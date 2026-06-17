use crate::{
    action::CombatAction,
    card::{CardDefinition, TargetRequirement},
    combat::CombatState,
    content::cards::get_card_definition,
    ids::{CardId, MonsterId},
    SimError, SimResult,
};

#[must_use]
pub fn legal_combat_actions(state: &CombatState) -> Vec<CombatAction> {
    let mut actions = Vec::new();

    for card in &state.piles.hand {
        let Some(definition) = get_card_definition(card.content_id) else {
            continue;
        };

        if !is_affordable(state, definition) {
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
    match action {
        CombatAction::EndTurn => Ok(()),
        CombatAction::PlayCard { card_id, target } => {
            let definition = card_definition_for_hand_card(state, card_id)?;

            if !is_affordable(state, definition) {
                return Err(SimError::IllegalAction("card is unaffordable"));
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

fn is_affordable(state: &CombatState, definition: &CardDefinition) -> bool {
    state.player.energy >= i32::from(definition.cost)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{BASH_ID, DEFEND_R_ID, STRIKE_R_ID};

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
