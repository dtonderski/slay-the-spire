use crate::{
    action::CombatAction,
    card::{CardDefinition, CardType, TargetRequirement},
    combat::{
        cost::effective_card_cost_with_corruption, transition::top_draw_card_definition,
        CombatDecisionState, CombatPhase, CombatState,
    },
    content::cards::{
        get_card_definition, CLASH_ID, CLASH_PLUS_ID, DUAL_WIELD_ID, DUAL_WIELD_PLUS_ID, HAVOC_ID,
        HAVOC_PLUS_ID, NORMALITY_ID, SECRET_TECHNIQUE_ID, SECRET_TECHNIQUE_PLUS_ID,
        SECRET_WEAPON_ID, SECRET_WEAPON_PLUS_ID, TRANSMUTATION_ID, TRANSMUTATION_PLUS_ID,
        WHIRLWIND_ID, WHIRLWIND_PLUS_ID,
    },
    ids::{CardId, MonsterId},
    relic::{can_play_card_with_relics, can_play_unplayable_card_with_relics},
    SimError, SimResult,
};

pub fn legal_combat_actions(state: &CombatState) -> SimResult<Vec<CombatAction>> {
    state.validate()?;
    if state.phase != CombatPhase::WaitingForPlayer {
        return Ok(Vec::new());
    }

    if state.decision.is_some() {
        return Ok(Vec::new());
    }

    let mut actions = Vec::new();
    if !can_play_card_with_relics(state) {
        actions.push(CombatAction::EndTurn);
        return Ok(actions);
    }

    for card in &state.piles.hand {
        let Some(definition) = get_card_definition(card.content_id) else {
            continue;
        };

        if definition.keywords.unplayable
            && !can_play_unplayable_card_with_relics(
                &state.relics,
                definition.card_type,
                card.content_id,
            )
        {
            continue;
        }

        if !is_affordable(state, card, definition)? {
            continue;
        }

        if normality_blocks_card_play(state) {
            continue;
        }

        if state.player.powers.entangled > 0 && definition.card_type == CardType::Attack {
            continue;
        }

        if is_clash(definition) && !hand_contains_only_attacks(state) {
            continue;
        }

        if definition.id == HAVOC_ID || definition.id == HAVOC_PLUS_ID {
            push_havoc_actions(&mut actions, state, card.id);
            continue;
        }

        if definition.id == DUAL_WIELD_ID || definition.id == DUAL_WIELD_PLUS_ID {
            if has_attack_or_power_in_hand(state, card.id) {
                actions.push(CombatAction::PlayCard {
                    card_id: card.id,
                    target: None,
                });
            }
            continue;
        }

        if (definition.id == SECRET_WEAPON_ID || definition.id == SECRET_WEAPON_PLUS_ID)
            && !has_attack_in_draw_pile(state)
        {
            continue;
        }

        if (definition.id == SECRET_TECHNIQUE_ID || definition.id == SECRET_TECHNIQUE_PLUS_ID)
            && !has_skill_in_draw_pile(state)
        {
            continue;
        }

        match definition.target {
            TargetRequirement::Enemy => {
                actions.extend(
                    living_monster_ids(state).map(|target| CombatAction::PlayCard {
                        card_id: card.id,
                        target: Some(target),
                    }),
                );
            }
            TargetRequirement::AllEnemies => {
                if has_living_monster(state) {
                    actions.push(CombatAction::PlayCard {
                        card_id: card.id,
                        target: None,
                    });
                }
            }
            TargetRequirement::None => {
                actions.push(CombatAction::PlayCard {
                    card_id: card.id,
                    target: None,
                });
            }
        }
    }

    actions.push(CombatAction::EndTurn);
    Ok(actions)
}

pub fn validate_combat_action(state: &CombatState, action: CombatAction) -> SimResult<()> {
    state.validate()?;

    if state.phase != CombatPhase::WaitingForPlayer {
        return Err(SimError::IllegalAction(
            "combat is not waiting for player input",
        ));
    }

    if let Some(decision) = &state.decision {
        let reason = match decision {
            CombatDecisionState::HandSelect { .. } => "hand select is open",
            CombatDecisionState::DrawSelect { .. } => "draw select is open",
            CombatDecisionState::DiscardSelect { .. } => "discard select is open",
            CombatDecisionState::ExhaustSelect { .. } => "exhaust select is open",
            CombatDecisionState::PotionCardReward { .. }
            | CombatDecisionState::ToolboxCardReward { .. }
            | CombatDecisionState::DiscoveryCardReward { .. } => "combat card reward is open",
        };
        return Err(SimError::IllegalAction(reason));
    }

    match action {
        CombatAction::EndTurn => Ok(()),
        CombatAction::PlayCard { card_id, target } => {
            if !can_play_card_with_relics(state) {
                return Err(SimError::IllegalAction("card play limit reached"));
            }
            if normality_blocks_card_play(state) {
                return Err(SimError::IllegalAction("Normality card play limit reached"));
            }

            let definition = card_definition_for_hand_card(state, card_id)?;
            let card = state
                .piles
                .hand
                .iter()
                .find(|card| card.id == card_id)
                .ok_or(SimError::UnknownCard(card_id))?;

            if definition.keywords.unplayable
                && !can_play_unplayable_card_with_relics(
                    &state.relics,
                    definition.card_type,
                    card.content_id,
                )
            {
                return Err(SimError::IllegalAction("card is unplayable"));
            }

            if definition.id == HAVOC_ID || definition.id == HAVOC_PLUS_ID {
                if let Some(top_definition) = top_draw_card_definition(state) {
                    return validate_havoc_play(state, top_definition, target);
                }
                if !state.piles.discard_pile.is_empty() {
                    return match target {
                        Some(monster_id) if is_living_monster(state, monster_id) => Ok(()),
                        Some(_) => Err(SimError::IllegalAction("target is not a living monster")),
                        None => Ok(()),
                    };
                }
                return if target.is_none() {
                    Ok(())
                } else {
                    Err(SimError::IllegalAction(
                        "Havoc top card cannot have a target",
                    ))
                };
            }

            if definition.id == DUAL_WIELD_ID || definition.id == DUAL_WIELD_PLUS_ID {
                if target.is_some() {
                    return Err(SimError::IllegalAction(
                        "non-targeted card cannot have a target",
                    ));
                }
                if !has_attack_or_power_in_hand(state, card_id) {
                    return Err(SimError::IllegalAction(
                        "Dual Wield requires an attack or power",
                    ));
                }
                return Ok(());
            }

            if (definition.id == SECRET_WEAPON_ID || definition.id == SECRET_WEAPON_PLUS_ID)
                && !has_attack_in_draw_pile(state)
            {
                return Err(SimError::IllegalAction(
                    "Secret Weapon requires an attack in draw pile",
                ));
            }

            if (definition.id == SECRET_TECHNIQUE_ID || definition.id == SECRET_TECHNIQUE_PLUS_ID)
                && !has_skill_in_draw_pile(state)
            {
                return Err(SimError::IllegalAction(
                    "Secret Technique requires a skill in draw pile",
                ));
            }

            if !is_affordable(state, card, definition)? {
                return Err(SimError::IllegalAction("card is unaffordable"));
            }

            if state.player.powers.entangled > 0 && definition.card_type == CardType::Attack {
                return Err(SimError::IllegalAction("player is entangled"));
            }

            if is_clash(definition) && !hand_contains_only_attacks(state) {
                return Err(SimError::IllegalAction(
                    "Clash requires only attacks in hand",
                ));
            }

            match (definition.target, target) {
                (TargetRequirement::Enemy, Some(monster_id)) => {
                    if is_living_monster(state, monster_id) {
                        Ok(())
                    } else {
                        Err(SimError::IllegalAction("target is not a living monster"))
                    }
                }
                (TargetRequirement::Enemy, None) => {
                    Err(SimError::IllegalAction("targeted card requires a target"))
                }
                (TargetRequirement::AllEnemies, None) => {
                    if has_living_monster(state) {
                        Ok(())
                    } else {
                        Err(SimError::IllegalAction("no living monsters to hit"))
                    }
                }
                (TargetRequirement::AllEnemies, Some(_)) => Err(SimError::IllegalAction(
                    "all-enemies card cannot have a target",
                )),
                (TargetRequirement::None, Some(_)) => Err(SimError::IllegalAction(
                    "non-targeted card cannot have a target",
                )),
                (TargetRequirement::None, None) => Ok(()),
            }
        }
    }
}

fn card_definition_for_hand_card(
    state: &CombatState,
    card_id: CardId,
) -> SimResult<&'static CardDefinition> {
    let card = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .ok_or(SimError::UnknownCard(card_id))?;

    get_card_definition(card.content_id).ok_or(SimError::UnknownContent(card.content_id))
}

fn is_affordable(
    state: &CombatState,
    card: &crate::CardInstance,
    definition: &CardDefinition,
) -> SimResult<bool> {
    if definition.id == WHIRLWIND_ID || definition.id == WHIRLWIND_PLUS_ID {
        return Ok(true);
    }
    if definition.id == TRANSMUTATION_ID || definition.id == TRANSMUTATION_PLUS_ID {
        return Ok(true);
    }
    Ok(state.player.energy
        >= effective_card_cost_with_corruption(card, state.player.powers.corruption > 0)?)
}

fn is_clash(definition: &CardDefinition) -> bool {
    definition.id == CLASH_ID || definition.id == CLASH_PLUS_ID
}

fn normality_blocks_card_play(state: &CombatState) -> bool {
    state
        .piles
        .hand
        .iter()
        .any(|card| card.content_id == NORMALITY_ID)
        && state.relic_counters.cards_played_this_turn >= 3
}

fn living_monster_ids(state: &CombatState) -> impl Iterator<Item = MonsterId> + '_ {
    state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster.id)
}

fn is_living_monster(state: &CombatState, monster_id: MonsterId) -> bool {
    state
        .monsters
        .iter()
        .any(|monster| monster.id == monster_id && monster.alive)
}

fn has_living_monster(state: &CombatState) -> bool {
    state.monsters.iter().any(|monster| monster.alive)
}

fn has_attack_or_power_in_hand(state: &CombatState, exclude_id: CardId) -> bool {
    state.piles.hand.iter().any(|card| {
        card.id != exclude_id
            && get_card_definition(card.content_id).is_some_and(|definition| {
                definition.card_type == CardType::Attack || definition.card_type == CardType::Power
            })
    })
}

fn has_attack_in_draw_pile(state: &CombatState) -> bool {
    state.piles.draw_pile.iter().any(|card| {
        get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == CardType::Attack)
    })
}

fn has_skill_in_draw_pile(state: &CombatState) -> bool {
    state.piles.draw_pile.iter().any(|card| {
        get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == CardType::Skill)
    })
}

fn hand_contains_only_attacks(state: &CombatState) -> bool {
    state.piles.hand.iter().all(|card| {
        get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == CardType::Attack)
    })
}

fn push_havoc_actions(actions: &mut Vec<CombatAction>, state: &CombatState, card_id: CardId) {
    let Some(top_definition) = top_draw_card_definition(state) else {
        actions.push(CombatAction::PlayCard {
            card_id,
            target: None,
        });
        return;
    };

    match top_definition.target {
        TargetRequirement::Enemy => {
            if has_living_monster(state) {
                actions.push(CombatAction::PlayCard {
                    card_id,
                    target: None,
                });
            }
        }
        TargetRequirement::AllEnemies => {
            if has_living_monster(state) {
                actions.push(CombatAction::PlayCard {
                    card_id,
                    target: None,
                });
            }
        }
        TargetRequirement::None => {
            actions.push(CombatAction::PlayCard {
                card_id,
                target: None,
            });
        }
    }
}

fn validate_havoc_play(
    state: &CombatState,
    top_definition: &CardDefinition,
    target: Option<MonsterId>,
) -> SimResult<()> {
    match top_definition.target {
        TargetRequirement::Enemy => match target {
            Some(monster_id) if is_living_monster(state, monster_id) => Ok(()),
            Some(_) => Err(SimError::IllegalAction("target is not a living monster")),
            None => Ok(()),
        },
        TargetRequirement::AllEnemies | TargetRequirement::None => {
            if target.is_none() {
                Ok(())
            } else {
                Err(SimError::IllegalAction(
                    "Havoc top card cannot have a target",
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_combat_state_is_not_reported_as_no_legal_actions() {
        let mut state = CombatState::initial_fixture();
        state.piles.draw_pile[0].id = state.piles.hand[0].id;

        assert_eq!(
            legal_combat_actions(&state),
            Err(SimError::InvalidState(
                "card instance appears in more than one pile"
            ))
        );
    }

    #[test]
    fn non_player_combat_phases_expose_and_accept_no_player_actions() {
        for phase in [
            CombatPhase::MonsterTurn,
            CombatPhase::Won,
            CombatPhase::Lost,
        ] {
            let mut state = CombatState::initial_fixture();
            state.phase = phase;

            assert!(
                legal_combat_actions(&state)
                    .expect("valid non-player combat state")
                    .is_empty(),
                "{phase:?}"
            );
            assert_eq!(
                validate_combat_action(&state, CombatAction::EndTurn),
                Err(SimError::IllegalAction(
                    "combat is not waiting for player input"
                )),
                "{phase:?}"
            );
        }
    }
}
