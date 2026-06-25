use crate::{
    action::CombatAction,
    card::{CardDefinition, CardType, TargetRequirement},
    combat::{transition::top_draw_card_definition, CombatState},
    content::cards::{
        get_card_definition, CLASH_ID, DUAL_WIELD_ID, DUAL_WIELD_PLUS_ID, HAVOC_ID, HAVOC_PLUS_ID,
        WHIRLWIND_ID, WHIRLWIND_PLUS_ID,
    },
    ids::{CardId, MonsterId},
    relic::{can_play_card_with_relics, can_play_unplayable_card_with_relics, Relic},
    SimError, SimResult,
};

#[must_use]
pub fn legal_combat_actions(state: &CombatState) -> Vec<CombatAction> {
    if state.hand_select.is_some()
        || state.discard_select.is_some()
        || state.exhaust_select.is_some()
        || state.potion_card_reward.is_some()
        || state.toolbox_card_reward.is_some()
    {
        return Vec::new();
    }

    let mut actions = Vec::new();
    if !can_play_card_with_relics(state) {
        actions.push(CombatAction::EndTurn);
        return actions;
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

        if !is_affordable(state, card.id, definition) {
            continue;
        }

        if definition.id == CLASH_ID && !hand_contains_only_attacks(state) {
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
    actions
}

pub fn validate_combat_action(state: &CombatState, action: CombatAction) -> SimResult<()> {
    if state.hand_select.is_some() {
        return Err(SimError::IllegalAction("hand select is open"));
    }
    if state.discard_select.is_some() {
        return Err(SimError::IllegalAction("discard select is open"));
    }
    if state.exhaust_select.is_some() {
        return Err(SimError::IllegalAction("exhaust select is open"));
    }
    if state.potion_card_reward.is_some() {
        return Err(SimError::IllegalAction("combat card reward is open"));
    }
    if state.toolbox_card_reward.is_some() {
        return Err(SimError::IllegalAction("combat card reward is open"));
    }

    match action {
        CombatAction::EndTurn => Ok(()),
        CombatAction::PlayCard { card_id, target } => {
            if !can_play_card_with_relics(state) {
                return Err(SimError::IllegalAction("card play limit reached"));
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
                if state.piles.draw_pile.is_empty() {
                    return Err(SimError::IllegalAction("Havoc requires a draw pile card"));
                }
                let top_definition = top_draw_card_definition(state)
                    .ok_or(SimError::IllegalAction("Havoc requires a draw pile card"))?;
                return validate_havoc_play(top_definition, target);
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

            if !is_affordable(state, card_id, definition) {
                return Err(SimError::IllegalAction("card is unaffordable"));
            }

            if definition.id == CLASH_ID && !hand_contains_only_attacks(state) {
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

fn is_affordable(state: &CombatState, card_id: CardId, definition: &CardDefinition) -> bool {
    if is_x_cost(definition) {
        return state.player.energy >= 1 || state.relics.contains(&Relic::ChemicalX);
    }
    state.player.energy >= effective_hand_card_cost(state, card_id)
}

fn effective_hand_card_cost(state: &CombatState, card_id: CardId) -> i32 {
    let card = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .expect("hand card");
    if let Some(cost) = card.temp_cost {
        return i32::from(cost);
    }
    get_card_definition(card.content_id)
        .map(|definition| i32::from(definition.cost))
        .unwrap_or(i32::MAX)
}

fn is_x_cost(definition: &CardDefinition) -> bool {
    definition.id == WHIRLWIND_ID || definition.id == WHIRLWIND_PLUS_ID
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

fn hand_contains_only_attacks(state: &CombatState) -> bool {
    state.piles.hand.iter().all(|card| {
        get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == CardType::Attack)
    })
}

fn push_havoc_actions(actions: &mut Vec<CombatAction>, state: &CombatState, card_id: CardId) {
    let Some(top_definition) = top_draw_card_definition(state) else {
        return;
    };

    match top_definition.target {
        TargetRequirement::Enemy => {
            actions.extend(
                living_monster_ids(state).map(|target| CombatAction::PlayCard {
                    card_id,
                    target: Some(target),
                }),
            );
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
    top_definition: &CardDefinition,
    target: Option<MonsterId>,
) -> SimResult<()> {
    match top_definition.target {
        TargetRequirement::Enemy => {
            if target.is_some() {
                Ok(())
            } else {
                Err(SimError::IllegalAction("Havoc top card requires a target"))
            }
        }
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
    use crate::{
        content::cards::{
            ANGER_ID, ANGER_PLUS_ID, BASH_ID, BATTLE_TRANCE_ID, BATTLE_TRANCE_PLUS_ID,
            BLOODLETTING_ID, BLUDGEON_ID, BODY_SLAM_ID, BURNING_PACT_ID, CARNAGE_ID, CLASH_ID,
            CLEAVE_ID, CLEAVE_PLUS_ID, CLOTHESLINE_ID, DARK_EMBRACE_ID, DEFEND_R_ID, DUAL_WIELD_ID,
            FEEL_NO_PAIN_ID, FLEX_ID, FLEX_PLUS_ID, GHOSTLY_ARMOR_ID, HAVOC_ID, HEAVY_BLADE_ID,
            HEMOKINESIS_ID, IMPERVIOUS_ID, INFLAME_ID, INFLAME_PLUS_ID, INTIMIDATE_ID,
            IRON_WAVE_ID, PERFECTED_STRIKE_ID, POMMEL_STRIKE_ID, POMMEL_STRIKE_PLUS_ID,
            POWER_THROUGH_ID, PUMMEL_ID, RECKLESS_CHARGE_ID, REGRET_ID, SEARING_BLOW_ID,
            SEEING_RED_ID, SEEING_RED_PLUS_ID, SENTINEL_ID, SHRUG_IT_OFF_ID, SPOT_WEAKNESS_ID,
            SPOT_WEAKNESS_PLUS_ID, STRIKE_R_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, TWIN_STRIKE_PLUS_ID,
            WHIRLWIND_ID, WHIRLWIND_PLUS_ID, WILD_STRIKE_ID, WOUND_ID,
        },
        CardInstance, Relic,
    };

    #[test]
    fn strike_is_legal_with_living_monster_target() {
        let state = CombatState::initial_fixture();

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: hand_card_id(&state, STRIKE_R_ID),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn defend_is_legal_without_target() {
        let state = CombatState::initial_fixture();

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: hand_card_id(&state, DEFEND_R_ID),
                target: None,
            })
        );
    }

    #[test]
    fn blue_candle_makes_unplayable_curses_legal() {
        let mut state = hand_with_card(REGRET_ID);
        state.relics = vec![Relic::BlueCandle];

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn medical_kit_makes_unplayable_statuses_legal() {
        let mut state = hand_with_card(WOUND_ID);
        state.relics = vec![Relic::MedicalKit];

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn medical_kit_does_not_make_curses_legal() {
        let mut state = hand_with_card(REGRET_ID);
        state.relics = vec![Relic::MedicalKit];

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn bash_is_illegal_at_one_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: hand_card_id(&state, BASH_ID),
                target: Some(MonsterId::new(1)),
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: hand_card_id(&state, BASH_ID),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn velvet_choker_blocks_card_play_after_six_cards_this_turn() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::VelvetChoker);
        state.relic_counters.cards_played_this_turn = crate::relic::VELVET_CHOKER_CARD_LIMIT;

        let strike = CombatAction::PlayCard {
            card_id: hand_card_id(&state, STRIKE_R_ID),
            target: Some(MonsterId::new(1)),
        };

        assert_eq!(legal_combat_actions(&state), vec![CombatAction::EndTurn]);
        assert_eq!(
            validate_combat_action(&state, strike),
            Err(SimError::IllegalAction("card play limit reached"))
        );
    }

    #[test]
    fn dead_monster_cannot_be_targeted() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].alive = false;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: hand_card_id(&state, STRIKE_R_ID),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn targeted_cards_reject_missing_targets() {
        let state = CombatState::initial_fixture();

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: hand_card_id(&state, STRIKE_R_ID),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("targeted card requires a target"))
        );
    }

    #[test]
    fn non_targeted_cards_reject_targets() {
        let state = CombatState::initial_fixture();

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: hand_card_id(&state, DEFEND_R_ID),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn legal_action_generation_does_not_mutate_state_hash() {
        let state = CombatState::initial_fixture();
        let before = state.snapshot().hash().expect("state hashes before");

        let _actions = legal_combat_actions(&state);

        assert_eq!(state.snapshot().hash().expect("state hashes after"), before);
    }

    #[test]
    fn anger_is_legal_at_zero_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 0;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), ANGER_ID)];

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn cleave_is_legal_without_target() {
        let state = hand_with_card(CLEAVE_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn cleave_rejects_target() {
        let state = hand_with_card(CLEAVE_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "all-enemies card cannot have a target"
            ))
        );
    }

    #[test]
    fn cleave_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(CLEAVE_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn twin_strike_is_legal_with_target() {
        let state = hand_with_card(TWIN_STRIKE_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn twin_strike_rejects_missing_target() {
        let state = hand_with_card(TWIN_STRIKE_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("targeted card requires a target"))
        );
    }

    #[test]
    fn iron_wave_is_legal_with_target() {
        let state = hand_with_card(IRON_WAVE_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn iron_wave_rejects_missing_target() {
        let state = hand_with_card(IRON_WAVE_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("targeted card requires a target"))
        );
    }

    #[test]
    fn body_slam_is_legal_with_target() {
        let state = hand_with_card(BODY_SLAM_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn body_slam_rejects_missing_target() {
        let state = hand_with_card(BODY_SLAM_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("targeted card requires a target"))
        );
    }

    #[test]
    fn body_slam_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(BODY_SLAM_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn clash_is_legal_with_target_when_hand_contains_only_attacks() {
        let mut state = hand_with_card(CLASH_ID);
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(21), STRIKE_R_ID));
        state.player.energy = 0;

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn clash_rejects_missing_target() {
        let state = hand_with_card(CLASH_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("targeted card requires a target"))
        );
    }

    #[test]
    fn clash_is_unplayable_with_skill_in_hand() {
        let mut state = hand_with_card(CLASH_ID);
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(21), DEFEND_R_ID));

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "Clash requires only attacks in hand"
            ))
        );
    }

    #[test]
    fn clash_is_unplayable_with_curse_in_hand_even_if_blue_candle_can_play_it() {
        let mut state = hand_with_card(CLASH_ID);
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(21), REGRET_ID));
        state.relics.push(Relic::BlueCandle);

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn wild_strike_is_legal_with_target() {
        let state = hand_with_card(WILD_STRIKE_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn wild_strike_rejects_missing_target() {
        let state = hand_with_card(WILD_STRIKE_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("targeted card requires a target"))
        );
    }

    #[test]
    fn wild_strike_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(WILD_STRIKE_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn heavy_blade_is_legal_with_target() {
        let state = hand_with_card(HEAVY_BLADE_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn heavy_blade_rejects_missing_target() {
        let state = hand_with_card(HEAVY_BLADE_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("targeted card requires a target"))
        );
    }

    #[test]
    fn heavy_blade_is_illegal_at_one_energy() {
        let mut state = hand_with_card(HEAVY_BLADE_ID);
        state.player.energy = 1;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn perfected_strike_is_legal_with_target() {
        let state = hand_with_card(PERFECTED_STRIKE_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn perfected_strike_rejects_missing_target() {
        let state = hand_with_card(PERFECTED_STRIKE_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("targeted card requires a target"))
        );
    }

    #[test]
    fn perfected_strike_is_illegal_at_one_energy() {
        let mut state = hand_with_card(PERFECTED_STRIKE_ID);
        state.player.energy = 1;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn power_through_is_legal_without_target() {
        let state = hand_with_card(POWER_THROUGH_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn power_through_rejects_target() {
        let state = hand_with_card(POWER_THROUGH_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn power_through_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(POWER_THROUGH_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn ghostly_armor_is_legal_without_target() {
        let state = hand_with_card(GHOSTLY_ARMOR_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn ghostly_armor_rejects_target() {
        let state = hand_with_card(GHOSTLY_ARMOR_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn ghostly_armor_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(GHOSTLY_ARMOR_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn reckless_charge_is_legal_with_target_at_zero_energy() {
        let mut state = hand_with_card(RECKLESS_CHARGE_ID);
        state.player.energy = 0;

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Ok(())
        );
    }

    #[test]
    fn reckless_charge_rejects_missing_target() {
        let state = hand_with_card(RECKLESS_CHARGE_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("targeted card requires a target"))
        );
    }

    #[test]
    fn pummel_is_legal_with_target() {
        let state = hand_with_card(PUMMEL_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn pummel_rejects_missing_target() {
        let state = hand_with_card(PUMMEL_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("targeted card requires a target"))
        );
    }

    #[test]
    fn pummel_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(PUMMEL_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn bludgeon_is_legal_with_target() {
        let state = hand_with_card(BLUDGEON_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn bludgeon_rejects_missing_target() {
        let state = hand_with_card(BLUDGEON_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("targeted card requires a target"))
        );
    }

    #[test]
    fn bludgeon_is_illegal_at_two_energy() {
        let mut state = hand_with_card(BLUDGEON_ID);
        state.player.energy = 2;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn carnage_is_legal_with_target_at_two_energy() {
        let mut state = hand_with_card(CARNAGE_ID);
        state.player.energy = 2;

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Ok(())
        );
    }

    #[test]
    fn carnage_rejects_missing_target() {
        let state = hand_with_card(CARNAGE_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("targeted card requires a target"))
        );
    }

    #[test]
    fn carnage_is_illegal_at_one_energy() {
        let mut state = hand_with_card(CARNAGE_ID);
        state.player.energy = 1;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn hemokinesis_is_legal_with_target_at_one_energy() {
        let mut state = hand_with_card(HEMOKINESIS_ID);
        state.player.energy = 1;

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Ok(())
        );
    }

    #[test]
    fn hemokinesis_rejects_missing_target() {
        let state = hand_with_card(HEMOKINESIS_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("targeted card requires a target"))
        );
    }

    #[test]
    fn hemokinesis_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(HEMOKINESIS_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn impervious_is_legal_without_target() {
        let state = hand_with_card(IMPERVIOUS_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn impervious_rejects_target() {
        let state = hand_with_card(IMPERVIOUS_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn impervious_is_illegal_at_one_energy() {
        let mut state = hand_with_card(IMPERVIOUS_ID);
        state.player.energy = 1;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn clothesline_is_legal_with_target() {
        let state = hand_with_card(CLOTHESLINE_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn clothesline_rejects_missing_target() {
        let state = hand_with_card(CLOTHESLINE_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("targeted card requires a target"))
        );
    }

    #[test]
    fn intimidate_is_legal_without_target_at_zero_energy() {
        let mut state = hand_with_card(INTIMIDATE_ID);
        state.player.energy = 0;

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn intimidate_rejects_target() {
        let state = hand_with_card(INTIMIDATE_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn anger_plus_is_legal_at_zero_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 0;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), ANGER_PLUS_ID)];

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn cleave_plus_is_legal_without_target() {
        let state = hand_with_card(CLEAVE_PLUS_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn twin_strike_plus_is_legal_with_target() {
        let state = hand_with_card(TWIN_STRIKE_PLUS_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn shrug_it_off_is_legal_without_target() {
        let state = hand_with_card(SHRUG_IT_OFF_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn shrug_it_off_rejects_target() {
        let state = hand_with_card(SHRUG_IT_OFF_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn true_grit_is_legal_without_target() {
        let state = hand_with_card(TRUE_GRIT_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn true_grit_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(TRUE_GRIT_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn burning_pact_is_legal_without_target() {
        let state = hand_with_card(BURNING_PACT_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn sentinel_is_legal_without_target() {
        let state = hand_with_card(SENTINEL_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn sentinel_rejects_target() {
        let state = hand_with_card(SENTINEL_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn sentinel_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(SENTINEL_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn feel_no_pain_is_legal_without_target() {
        let state = hand_with_card(FEEL_NO_PAIN_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn dark_embrace_is_legal_without_target() {
        let state = hand_with_card(DARK_EMBRACE_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn pommel_strike_is_legal_with_target() {
        let state = hand_with_card(POMMEL_STRIKE_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn pommel_strike_rejects_missing_target() {
        let state = hand_with_card(POMMEL_STRIKE_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("targeted card requires a target"))
        );
    }

    #[test]
    fn battle_trance_is_legal_at_zero_energy() {
        let mut state = hand_with_card(BATTLE_TRANCE_ID);
        state.player.energy = 0;

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn bloodletting_is_legal_at_zero_energy_without_target() {
        let mut state = hand_with_card(BLOODLETTING_ID);
        state.player.energy = 0;

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn bloodletting_rejects_target() {
        let state = hand_with_card(BLOODLETTING_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn seeing_red_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(SEEING_RED_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn seeing_red_plus_is_legal_at_zero_energy() {
        let mut state = hand_with_card(SEEING_RED_PLUS_ID);
        state.player.energy = 0;

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn pommel_strike_plus_is_legal_with_target() {
        let state = hand_with_card(POMMEL_STRIKE_PLUS_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn battle_trance_plus_is_legal_at_zero_energy() {
        let mut state = hand_with_card(BATTLE_TRANCE_PLUS_ID);
        state.player.energy = 0;

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn inflame_is_legal_without_target() {
        let state = hand_with_card(INFLAME_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn inflame_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(INFLAME_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn inflame_plus_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(INFLAME_PLUS_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn flex_is_legal_at_zero_energy() {
        let mut state = hand_with_card(FLEX_ID);
        state.player.energy = 0;

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn flex_plus_is_legal_at_zero_energy() {
        let mut state = hand_with_card(FLEX_PLUS_ID);
        state.player.energy = 0;

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn spot_weakness_is_legal_without_target() {
        let state = hand_with_card(SPOT_WEAKNESS_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn spot_weakness_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(SPOT_WEAKNESS_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn spot_weakness_plus_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(SPOT_WEAKNESS_PLUS_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn whirlwind_is_legal_without_target() {
        let state = hand_with_card(WHIRLWIND_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn whirlwind_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(WHIRLWIND_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn whirlwind_is_legal_at_zero_energy_with_chemical_x() {
        let mut state = hand_with_card(WHIRLWIND_ID);
        state.player.energy = 0;
        state.relics.push(Relic::ChemicalX);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: None,
                },
            ),
            Ok(())
        );
    }

    #[test]
    fn whirlwind_plus_is_illegal_at_zero_energy() {
        let mut state = hand_with_card(WHIRLWIND_PLUS_ID);
        state.player.energy = 0;

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn havoc_is_illegal_with_empty_draw_pile() {
        let mut state = hand_with_card(HAVOC_ID);
        state.piles.draw_pile.clear();

        assert!(!legal_combat_actions(&state)
            .iter()
            .any(|action| matches!(action, CombatAction::PlayCard { card_id, .. } if *card_id == CardId::new(20))));
    }

    #[test]
    fn havoc_is_legal_when_top_of_draw_is_strike() {
        let mut state = hand_with_card(HAVOC_ID);
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    #[test]
    fn dual_wield_is_illegal_without_attack_or_power() {
        let mut state = hand_with_card(DUAL_WIELD_ID);
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DUAL_WIELD_ID)];

        assert!(
            !legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn searing_blow_is_legal_with_target() {
        let state = hand_with_card(SEARING_BLOW_ID);

        assert!(
            legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            })
        );
    }

    fn hand_with_card(content_id: crate::ContentId) -> CombatState {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), content_id)];
        state
    }

    fn hand_card_id(state: &CombatState, content_id: crate::ContentId) -> CardId {
        state
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == content_id)
            .expect("card is in hand")
            .id
    }
}
