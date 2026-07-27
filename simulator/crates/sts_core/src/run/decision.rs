use crate::{
    action::{CombatAction, EventAction, RestAction},
    combat::{
        legal_combat_actions, validate_combat_action, CombatDecisionState, CombatState,
        ExhaustSelectPurpose,
    },
    map::MapAction,
    potion::Potion,
    RunPhase, SimError, SimResult,
};
use serde::{Deserialize, Serialize};

use super::{
    apply_combat_action_on_run, apply_event_action, apply_map_action_on_run, apply_rest_action,
    apply_run_action, cancel_grid, confirm_grid, legal_event_actions, legal_map_actions_on_run,
    legal_rest_actions, legal_shop_actions, select_grid_card, validate_event_action,
    validate_potion_action, validate_rest_action, validate_shop_action, RunAction, RunState,
};
use super::{
    grid::{validate_grid_cancel, validate_grid_confirm, validate_grid_select},
    map::validate_map_action_on_run,
};

/// One authoritative decision at any supported run boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunDecisionAction {
    Combat(CombatAction),
    Event(EventAction),
    GridSelect { index: usize },
    GridConfirm,
    GridCancel,
    Map(MapAction),
    Rest(RestAction),
    Run(RunAction),
}

/// Validates one top-level run decision without executing simulator mechanics.
pub fn validate_run_decision_action(run: &RunState, action: RunDecisionAction) -> SimResult<()> {
    run.validate()?;
    match action {
        RunDecisionAction::Combat(action) => {
            if run.phase != RunPhase::Combat {
                return Err(SimError::IllegalAction(
                    "combat actions require combat phase",
                ));
            }
            let combat = run
                .combat
                .as_ref()
                .ok_or(SimError::InvalidState("combat state is missing"))?;
            validate_combat_action(combat, action)
        }
        RunDecisionAction::Event(action) => validate_event_action(run, action),
        RunDecisionAction::GridSelect { index } => validate_grid_select(run, index),
        RunDecisionAction::GridConfirm => validate_grid_confirm(run),
        RunDecisionAction::GridCancel => validate_grid_cancel(run),
        RunDecisionAction::Map(action) => validate_map_action_on_run(run, action),
        RunDecisionAction::Rest(action) => validate_rest_action(run, action),
        RunDecisionAction::Run(action) => validate_run_action(run, action),
    }
}

/// Enumerates the complete supported decision boundary without executing transitions or RNG.
pub fn legal_run_decision_actions(run: &RunState) -> SimResult<Vec<RunDecisionAction>> {
    run.validate()?;
    let mut actions = Vec::new();

    if let Some(grid) = run.card_grid.as_ref() {
        actions.extend((0..grid.cards.len()).map(|index| RunDecisionAction::GridSelect { index }));
        actions.push(RunDecisionAction::GridConfirm);
        actions.push(RunDecisionAction::GridCancel);
        return validated_decision_candidates(run, actions);
    }

    match run.phase {
        RunPhase::Combat => {
            let combat = run
                .combat
                .as_ref()
                .ok_or(SimError::InvalidState("combat state is missing"))?;
            let select_actions = legal_combat_select_actions_on_run(run, combat)?;
            if !select_actions.is_empty() {
                actions.extend(select_actions.into_iter().map(RunDecisionAction::Run));
            } else {
                actions.extend(
                    legal_combat_actions(combat)?
                        .into_iter()
                        .map(RunDecisionAction::Combat),
                );
            }
            actions.extend(
                legal_potion_actions_on_run(run)?
                    .into_iter()
                    .map(RunDecisionAction::Run),
            );
        }
        RunPhase::Reward => {
            actions.extend(
                legal_reward_actions(run)?
                    .into_iter()
                    .map(RunDecisionAction::Run),
            );
            actions.extend(
                legal_potion_actions_on_run(run)?
                    .into_iter()
                    .map(RunDecisionAction::Run),
            );
        }
        RunPhase::Treasure => {
            actions.extend(
                [RunAction::OpenChest, RunAction::Proceed]
                    .into_iter()
                    .map(RunDecisionAction::Run),
            );
        }
        RunPhase::Idle => actions.extend(
            legal_map_actions_on_run(run)?
                .into_iter()
                .map(RunDecisionAction::Map),
        ),
        RunPhase::Rest => actions.extend(
            legal_rest_actions(run)?
                .into_iter()
                .map(RunDecisionAction::Rest),
        ),
        RunPhase::Event => actions.extend(
            legal_event_actions(run)?
                .into_iter()
                .map(RunDecisionAction::Event),
        ),
        RunPhase::Shop => actions.extend(
            legal_shop_actions(run)?
                .into_iter()
                .map(RunDecisionAction::Run),
        ),
        RunPhase::Complete => {}
    }

    validated_decision_candidates(run, actions)
}

/// Applies one top-level run decision and validates both sides of the boundary.
pub fn apply_run_decision_action(run: &RunState, action: RunDecisionAction) -> SimResult<RunState> {
    validate_run_decision_action(run, action)?;
    let debug_potion_counter = run.potion_rng_counter;
    let next = match action {
        RunDecisionAction::Combat(action) => apply_combat_action_on_run(run, action),
        RunDecisionAction::Event(action) => apply_event_action(run, action),
        RunDecisionAction::GridSelect { index } => select_grid_card(run, index),
        RunDecisionAction::GridConfirm => confirm_grid(run),
        RunDecisionAction::GridCancel => cancel_grid(run),
        RunDecisionAction::Map(action) => apply_map_action_on_run(run, action),
        RunDecisionAction::Rest(action) => apply_rest_action(run, action),
        RunDecisionAction::Run(action) => apply_run_action(run, action),
    }?;
    if next.potion_rng_counter != debug_potion_counter {
        eprintln!(
            "DEBUG decision floor={} action={:?} before={} after={}",
            run.current_floor, action, debug_potion_counter, next.potion_rng_counter
        );
    }
    next.validate()?;
    Ok(next)
}

fn validate_run_action(run: &RunState, action: RunAction) -> SimResult<()> {
    match action {
        RunAction::OpenChest => super::reward::validate_treasure_action(run, action),
        RunAction::Proceed if run.phase == RunPhase::Reward => run.validate_reward_action(action),
        RunAction::Proceed if run.phase == RunPhase::Shop => validate_shop_action(run, action),
        RunAction::Proceed => super::reward::validate_treasure_action(run, action),
        RunAction::BuyShopCard { .. }
        | RunAction::BuyShopRelic { .. }
        | RunAction::BuyShopPotion { .. }
        | RunAction::EnterShop
        | RunAction::LeaveShop
        | RunAction::OpenShopRemove => validate_shop_action(run, action),
        RunAction::UsePotion { .. }
        | RunAction::DiscardPotion { .. }
        | RunAction::ChooseCombatCardReward { .. }
        | RunAction::SkipCombatCardReward
        | RunAction::ChooseHandSelect { .. }
        | RunAction::ConfirmHandSelect
        | RunAction::ChooseDrawSelect { .. }
        | RunAction::ConfirmDrawSelect
        | RunAction::ChooseDiscardSelect { .. }
        | RunAction::ConfirmDiscardSelect
        | RunAction::ChooseExhaustSelect { .. }
        | RunAction::ConfirmExhaustSelect => validate_potion_action(run, action),
        _ => run.validate_reward_action(action),
    }
}

fn validated_decision_candidates(
    run: &RunState,
    candidates: Vec<RunDecisionAction>,
) -> SimResult<Vec<RunDecisionAction>> {
    let mut legal = Vec::new();
    for action in candidates {
        match validate_run_decision_action(run, action) {
            Ok(()) => legal.push(action),
            Err(SimError::IllegalAction(_)) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(legal)
}

fn validated_run_action_candidates(
    run: &RunState,
    candidates: impl IntoIterator<Item = RunAction>,
) -> SimResult<Vec<RunAction>> {
    let mut legal = Vec::new();
    for action in candidates {
        match validate_run_action(run, action) {
            Ok(()) => legal.push(action),
            Err(SimError::IllegalAction(_)) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(legal)
}

fn legal_combat_select_actions_on_run(
    run: &RunState,
    combat: &CombatState,
) -> SimResult<Vec<RunAction>> {
    let mut candidates = Vec::new();
    match combat.decision.as_ref() {
        Some(CombatDecisionState::PotionCardReward { choices, .. }) => {
            candidates.extend(
                (0..choices.len()).map(|index| RunAction::ChooseCombatCardReward { index }),
            );
            candidates.push(RunAction::SkipCombatCardReward);
        }
        Some(CombatDecisionState::ToolboxCardReward { choices })
        | Some(CombatDecisionState::DiscoveryCardReward { choices, .. }) => {
            candidates.extend(
                (0..choices.len()).map(|index| RunAction::ChooseCombatCardReward { index }),
            );
        }
        Some(CombatDecisionState::HandSelect { .. }) => {
            candidates.extend(
                (0..combat.piles.hand.len()).map(|index| RunAction::ChooseHandSelect { index }),
            );
            candidates.push(RunAction::ConfirmHandSelect);
        }
        Some(CombatDecisionState::DrawSelect { .. }) => {
            candidates.extend(
                (0..combat.piles.draw_pile.len())
                    .map(|index| RunAction::ChooseDrawSelect { index }),
            );
            candidates.push(RunAction::ConfirmDrawSelect);
        }
        Some(CombatDecisionState::DiscardSelect { .. }) => {
            candidates.extend(
                (0..combat.piles.discard_pile.len())
                    .map(|index| RunAction::ChooseDiscardSelect { index }),
            );
            candidates.push(RunAction::ConfirmDiscardSelect);
        }
        Some(CombatDecisionState::ExhaustSelect { state: select }) => {
            let choice_count = if select.purpose == ExhaustSelectPurpose::ExhumeReturnToHand {
                combat.piles.exhaust_pile.len()
            } else {
                combat.piles.hand.len()
            };
            candidates
                .extend((0..choice_count).map(|index| RunAction::ChooseExhaustSelect { index }));
            candidates.push(RunAction::ConfirmExhaustSelect);
        }
        None => {}
    }
    validated_run_action_candidates(run, candidates)
}

fn legal_reward_actions(run: &RunState) -> SimResult<Vec<RunAction>> {
    let mut candidates = vec![
        RunAction::SkipReward,
        RunAction::CloseCardReward,
        RunAction::TakeGoldReward,
        RunAction::TakeStolenGoldReward,
        RunAction::TakeRelicReward,
        RunAction::Proceed,
        RunAction::OpenCardReward,
        RunAction::SkipPotionReward,
        RunAction::TakeSingingBowlReward,
    ];
    if let Some(reward) = run.reward.as_ref() {
        let potion_offer_count = reward
            .potion_offers
            .len()
            .max(usize::from(reward.potion_offer.is_some()));
        candidates
            .extend((0..potion_offer_count).map(|index| RunAction::TakePotionReward { index }));
        candidates.extend(
            (0..reward.boss_relic_choices.len())
                .map(|index| RunAction::ChooseBossRelicReward { index }),
        );
        let relic_offer_count = usize::from(reward.relic_offer.is_some())
            + usize::from(reward.pending_relic_offer.is_some())
            + reward.queued_relic_offers.len();
        candidates
            .extend((0..relic_offer_count).map(|index| RunAction::TakeRelicRewardAt { index }));
        candidates.extend(
            (0..reward.queued_card_rewards.len())
                .map(|index| RunAction::OpenQueuedCardReward { index }),
        );
        candidates.extend(
            reward
                .choices
                .iter()
                .map(|choice| RunAction::TakeCardReward { card_id: choice.id }),
        );
    }
    validated_run_action_candidates(run, candidates)
}

fn legal_potion_actions_on_run(run: &RunState) -> SimResult<Vec<RunAction>> {
    let candidates = run
        .occupied_potion_slots()
        .into_iter()
        .flat_map(|(slot, potion)| {
            potion_use_candidates(slot, potion, run.combat.as_ref())
                .into_iter()
                .chain(std::iter::once(RunAction::DiscardPotion { slot }))
        })
        .collect::<Vec<_>>();
    validated_run_action_candidates(run, candidates)
}

fn potion_use_candidates(
    slot: usize,
    potion: Potion,
    combat: Option<&CombatState>,
) -> Vec<RunAction> {
    if potion.requires_target() {
        let Some(combat) = combat else {
            return Vec::new();
        };
        return combat
            .monsters
            .iter()
            .filter(|monster| monster.alive)
            .map(|monster| RunAction::UsePotion {
                slot,
                target: Some(monster.id),
            })
            .collect();
    }
    vec![RunAction::UsePotion { slot, target: None }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        combat::{
            CombatPhase, DrawSelectPurpose, DrawSelectState, HandSelectPurpose, HandSelectState,
        },
        content::cards::DEFEND_R_ID,
        legal_map_actions_on_run, CardGridScreen, CardId, CardInstance, GridPurpose,
    };

    #[test]
    fn top_level_map_step_matches_the_specialized_transition() {
        let run = RunState::map_fixture();
        let action = legal_map_actions_on_run(&run).expect("valid map fixture")[0];

        assert_eq!(
            apply_run_decision_action(&run, RunDecisionAction::Map(action)),
            apply_map_action_on_run(&run, action)
        );
    }

    #[test]
    fn top_level_step_rejects_malformed_pre_state_before_routing() {
        let mut run = RunState::seeded_ironclad(22_079_335_079, 0);
        run.phase = RunPhase::Shop;
        run.event = None;
        run.shop = None;

        assert_eq!(
            apply_run_decision_action(&run, RunDecisionAction::Run(RunAction::Proceed)),
            Err(SimError::InvalidState("shop phase has no shop screen"))
        );
    }

    #[test]
    fn top_level_legal_actions_are_validator_backed() {
        let run = RunState::map_fixture();
        let actions = legal_run_decision_actions(&run).expect("valid map fixture");

        assert!(!actions.is_empty());
        assert!(actions
            .iter()
            .all(|action| validate_run_decision_action(&run, *action).is_ok()));
        assert_eq!(
            actions,
            legal_map_actions_on_run(&run)
                .expect("valid map fixture")
                .into_iter()
                .map(RunDecisionAction::Map)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn top_level_legal_actions_reject_malformed_state() {
        let mut run = RunState::seeded_ironclad(22_079_335_079, 0);
        run.phase = RunPhase::Shop;
        run.event = None;
        run.shop = None;

        assert_eq!(
            legal_run_decision_actions(&run),
            Err(SimError::InvalidState("shop phase has no shop screen"))
        );
    }

    #[test]
    fn top_level_legal_actions_do_not_hide_invalid_candidate_state() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Shop;
        run.card_grid = Some(CardGridScreen {
            cards: vec![run.deck[0]],
            purpose: GridPurpose::ShopRemove,
            selected: Some(0),
            selected_indices: Vec::new(),
        });

        assert_eq!(
            legal_run_decision_actions(&run),
            Err(SimError::InvalidState("shop phase has no shop screen"))
        );
    }

    #[test]
    fn top_level_legal_actions_reject_duplicate_grid_selections() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Treasure;
        run.current_room_override = Some(crate::RoomKind::Boss);
        run.boss_chest_opened = true;
        run.relics.push(crate::Relic::Astrolabe);
        run.card_grid = Some(CardGridScreen {
            cards: vec![run.deck[0]],
            purpose: GridPurpose::Astrolabe,
            selected: None,
            selected_indices: vec![0, 0, 0],
        });

        assert_eq!(
            legal_run_decision_actions(&run),
            Err(SimError::InvalidState(
                "card grid selection indices contain duplicates"
            ))
        );
    }

    #[test]
    fn top_level_legal_actions_respect_mandatory_confirmation_grids() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::event_screen_for_run(
            &run,
            crate::Event::Neow,
        ));
        run.gain_relic(crate::Relic::CallingBell)
            .expect("Calling Bell opens its confirmation grid");

        assert_eq!(
            legal_run_decision_actions(&run),
            Ok(vec![RunDecisionAction::GridConfirm])
        );
    }

    #[test]
    fn top_level_legal_actions_reject_fabricated_grid_counts() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Treasure;
        run.current_room_override = Some(crate::RoomKind::Boss);
        run.boss_chest_opened = true;
        run.relics.push(crate::Relic::EmptyCage);
        run.card_grid = Some(CardGridScreen {
            cards: run.deck.clone(),
            purpose: GridPurpose::EmptyCage { remaining: 3 },
            selected: None,
            selected_indices: Vec::new(),
        });

        assert_eq!(
            legal_run_decision_actions(&run),
            Err(SimError::InvalidState(
                "card grid removal count is outside its authoritative range"
            ))
        );
    }

    #[test]
    fn top_level_legal_actions_omit_single_select_noops() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::Energy];
        run.empty_potion_slots = vec![1, 2];
        let combat = run.combat.as_mut().expect("combat fixture");
        let source_card_id = combat.piles.hand[0].id;
        combat.decision = Some(CombatDecisionState::HandSelect {
            state: HandSelectState {
                purpose: HandSelectPurpose::WarcryPutOnDraw,
                source_card_id,
                selected_hand_index: None,
                selected_hand_indices: Vec::new(),
            },
            pending_actions: Default::default(),
        });

        let selected = apply_run_decision_action(
            &run,
            RunDecisionAction::Run(RunAction::ChooseHandSelect { index: 0 }),
        )
        .expect("valid hand selection");
        let actions = legal_run_decision_actions(&selected).expect("valid selected state");

        assert!(
            !actions.contains(&RunDecisionAction::Run(RunAction::ChooseHandSelect {
                index: 0,
            }))
        );
        assert!(actions.contains(&RunDecisionAction::Run(RunAction::ConfirmHandSelect)));
        let use_energy_potion = RunDecisionAction::Run(RunAction::UsePotion {
            slot: 0,
            target: None,
        });
        assert!(actions.contains(&use_energy_potion));

        let after_potion = apply_run_decision_action(&selected, use_energy_potion)
            .expect("potion use remains legal during hand selection");
        assert!(matches!(
            after_potion
                .combat
                .as_ref()
                .expect("combat remains active")
                .decision,
            Some(CombatDecisionState::HandSelect { .. })
        ));
        assert_eq!(after_potion.potion_at_slot(0), None);
    }

    #[test]
    fn top_level_legal_actions_omit_selected_draw_choice() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let source_card_id = combat.piles.hand[0].id;
        combat.piles.draw_pile = vec![CardInstance::new(CardId::new(900), DEFEND_R_ID)];
        combat.decision = Some(CombatDecisionState::DrawSelect {
            state: DrawSelectState {
                purpose: DrawSelectPurpose::SecretTechniqueSkillToHand,
                source_card_id,
                selectable_card_ids: Vec::new(),
                selected_draw_index: None,
            },
        });

        let selected = apply_run_decision_action(
            &run,
            RunDecisionAction::Run(RunAction::ChooseDrawSelect { index: 0 }),
        )
        .expect("valid draw selection");
        let actions = legal_run_decision_actions(&selected).expect("valid selected state");

        assert!(
            !actions.contains(&RunDecisionAction::Run(RunAction::ChooseDrawSelect {
                index: 0,
            }))
        );
        assert!(actions.contains(&RunDecisionAction::Run(RunAction::ConfirmDrawSelect)));
    }

    #[test]
    fn top_level_legal_actions_omit_potions_outside_player_combat_phase() {
        let use_potion = RunDecisionAction::Run(RunAction::UsePotion {
            slot: 0,
            target: None,
        });
        let discard_potion = RunDecisionAction::Run(RunAction::DiscardPotion { slot: 0 });
        let phases = [
            (CombatPhase::WaitingForPlayer, true),
            (CombatPhase::MonsterTurn, false),
            (CombatPhase::Won, false),
            (CombatPhase::Lost, false),
        ];

        for (phase, player_can_act) in phases {
            let mut run = RunState::combat_fixture();
            run.potions = vec![Potion::Energy];
            run.empty_potion_slots = vec![1, 2];
            run.combat.as_mut().expect("combat fixture").phase = phase;
            let actions = legal_run_decision_actions(&run).expect("legal actions enumerate");

            assert_eq!(
                actions.contains(&use_potion),
                player_can_act,
                "use-potion legality for {phase:?}"
            );
            assert_eq!(
                actions.contains(&discard_potion),
                player_can_act,
                "discard-potion legality for {phase:?}"
            );

            if player_can_act {
                assert_eq!(validate_run_decision_action(&run, use_potion), Ok(()));
                assert_eq!(validate_run_decision_action(&run, discard_potion), Ok(()));
                continue;
            }

            for action in [use_potion, discard_potion] {
                assert_eq!(
                    validate_run_decision_action(&run, action),
                    Err(SimError::IllegalAction(
                        "combat is not waiting for player input"
                    )),
                    "validation for {action:?} in {phase:?}"
                );
                let before = run.clone();
                assert_eq!(
                    apply_run_decision_action(&run, action),
                    Err(SimError::IllegalAction(
                        "combat is not waiting for player input"
                    )),
                    "application for {action:?} in {phase:?}"
                );
                assert_eq!(run, before, "failed {action:?} mutated {phase:?}");
            }
        }

        let mut selecting = RunState::combat_fixture();
        selecting.potions = vec![Potion::Energy];
        selecting.empty_potion_slots = vec![1, 2];
        let source_card_id = selecting
            .combat
            .as_ref()
            .expect("combat fixture")
            .piles
            .hand[0]
            .id;
        selecting.combat.as_mut().expect("combat fixture").decision =
            Some(CombatDecisionState::HandSelect {
                state: HandSelectState {
                    purpose: HandSelectPurpose::WarcryPutOnDraw,
                    source_card_id,
                    selected_hand_index: None,
                    selected_hand_indices: Vec::new(),
                },
                pending_actions: Default::default(),
            });
        let selecting_actions = legal_run_decision_actions(&selecting).expect("selection actions");
        assert!(
            selecting_actions.contains(&RunDecisionAction::Run(RunAction::ChooseHandSelect {
                index: 0
            }))
        );
        assert!(selecting_actions.contains(&use_potion));
        assert!(selecting_actions.contains(&discard_potion));

        let selected = apply_run_decision_action(
            &selecting,
            RunDecisionAction::Run(RunAction::ChooseHandSelect { index: 0 }),
        )
        .expect("hand selection applies");
        let selected_actions =
            legal_run_decision_actions(&selected).expect("selected actions enumerate");
        assert!(selected_actions.contains(&RunDecisionAction::Run(RunAction::ConfirmHandSelect)));
        assert!(selected_actions.contains(&use_potion));
        assert!(selected_actions.contains(&discard_potion));
    }
}
