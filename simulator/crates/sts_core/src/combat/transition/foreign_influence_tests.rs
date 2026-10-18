use super::*;
use crate::{
    content::cards::{
        CONCLUDE_ANY_COLOR_ID, FOREIGN_INFLUENCE_ANY_COLOR_ID, HAVOC_ID, SKEWER_ANY_COLOR_ID,
        STRIKE_R_ID, WHIRLWIND_ID,
    },
    run::state::RunRngStream,
    run::{apply_combat_action_on_run, apply_run_action, RunAction, RunState},
};
fn setup(upgraded: bool) -> RunState {
    let mut r = RunState::combat_fixture();
    r.relics.clear();
    let c = r.combat.as_mut().unwrap();
    c.player.energy = 3;
    c.monsters[0].hp = 500;
    c.monsters[0].max_hp = 500;
    c.piles.hand = vec![CardInstance {
        combat_only: true,
        upgrades: u8::from(upgraded),
        ..CardInstance::new(CardId::new(1000), FOREIGN_INFLUENCE_ANY_COLOR_ID)
    }];
    c.piles.draw_pile.clear();
    c.piles.discard_pile.clear();
    c.piles.exhaust_pile.clear();
    r
}
fn open(r: &RunState) -> RunState {
    apply_combat_action_on_run(
        r,
        CombatAction::PlayCard {
            card_id: CardId::new(1000),
            target: None,
        },
    )
    .unwrap()
}
fn ids(r: &RunState) -> Vec<ContentId> {
    r.combat
        .as_ref()
        .unwrap()
        .combat_card_reward_choices()
        .unwrap()
        .iter()
        .map(|c| c.content_id)
        .collect()
}
#[test]
fn native_identity_opens_three_distinct_choices_without_current_turn_draw_or_payment() {
    let mut r = setup(false);
    r.combat.as_mut().unwrap().player.cannot_draw = true;
    let before = serde_json::to_value(&r).unwrap();
    let n = open(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(
        crate::content::shop_pool::shop_card_content_id("FOREIGN_INFLUENCE"),
        FOREIGN_INFLUENCE_ANY_COLOR_ID
    );
    assert_eq!(
        ids(&n)
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
    assert_eq!(c.player.energy, 3);
    assert!(c.piles.hand.is_empty());
    assert!(c.piles.exhaust_pile.is_empty());
    assert_eq!(
        n.card_rng_counter,
        c.run_card_rng.as_ref().unwrap().counter()
    );
    assert_eq!(
        n.card_random_rng_counter - r.card_random_rng_counter,
        2 * (n.card_rng_counter - r.card_rng_counter)
    );
    assert_eq!(before, serde_json::to_value(r).unwrap());
}
#[test]
fn retrieval_has_no_discovery_tail_generation_and_only_upgrade_makes_non_x_cards_free() {
    for upgraded in [false, true] {
        for id in [CONCLUDE_ANY_COLOR_ID, SKEWER_ANY_COLOR_ID, WHIRLWIND_ID] {
            let mut n = open(&setup(upgraded));
            if let Some(CombatDecisionState::ForeignInfluenceCardReward { choices, .. }) =
                n.combat.as_mut().unwrap().decision.as_mut()
            {
                choices[0].content_id = id;
            }
            let counters = (n.card_rng_counter, n.card_random_rng_counter);
            let a = apply_run_action(&n, RunAction::ChooseCombatCardReward { index: 0 }).unwrap();
            let c = a.combat.as_ref().unwrap();
            let card = c
                .piles
                .hand
                .iter()
                .find(|card| card.content_id == id)
                .unwrap();
            assert_eq!(
                card.temp_cost,
                if upgraded && id == CONCLUDE_ANY_COLOR_ID {
                    Some(0)
                } else {
                    None
                }
            );
            assert_eq!((a.card_rng_counter, a.card_random_rng_counter), counters);
            assert_eq!(c.piles.exhaust_pile.len(), 1);
            assert!(c.decision.is_none());
            assert_eq!(c.player.energy, 3);
        }
    }
}
#[test]
fn full_hand_retrieves_to_discard_and_skip_does_not_allocate_or_roll_an_offer_again() {
    let mut r = open(&setup(false));
    let c = r.combat.as_mut().unwrap();
    c.piles.hand = (1100..1110)
        .map(|i| CardInstance {
            combat_only: true,
            ..CardInstance::new(CardId::new(i), STRIKE_R_ID)
        })
        .collect();
    let selected = ids(&r)[0];
    let before = (r.card_rng_counter, r.card_random_rng_counter);
    let chosen = apply_run_action(&r, RunAction::ChooseCombatCardReward { index: 0 }).unwrap();
    assert_eq!(chosen.combat.as_ref().unwrap().piles.hand.len(), 10);
    assert!(chosen
        .combat
        .as_ref()
        .unwrap()
        .piles
        .discard_pile
        .iter()
        .any(|c| c.content_id == selected));
    let skipped = apply_run_action(&r, RunAction::SkipCombatCardReward).unwrap();
    let c = skipped.combat.as_ref().unwrap();
    assert_eq!(c.piles.hand.len(), 10);
    assert_eq!(c.piles.exhaust_pile.len(), 1);
    assert!(c.piles.discard_pile.is_empty());
    assert_eq!(
        (skipped.card_rng_counter, skipped.card_random_rng_counter),
        before
    );
}
#[test]
fn source_exhaustion_callbacks_and_rng_precede_fresh_copied_offer() {
    let mut r = setup(false);
    r.relics.push(Relic::DeadBranch);
    r.combat.as_mut().unwrap().player.powers.feel_no_pain = 3;
    let solo = open(&r);
    let solo = apply_run_action(&solo, RunAction::SkipCombatCardReward).unwrap();
    let c = solo.combat.as_ref().unwrap();
    let mut cr = c.rng.card_random_rng.clone();
    let mut rr = c.run_card_rng.clone().unwrap();
    let expected = crate::content::foreign_influence::choices(&mut cr, &mut rr);
    r.combat.as_mut().unwrap().duplication_potion_pending = true;
    r.combat.as_mut().unwrap().duplication_potion_stacks = 1;
    let copied = open(&r);
    let copied = apply_run_action(&copied, RunAction::SkipCombatCardReward).unwrap();
    assert_eq!(ids(&copied), expected);
    let c = copied.combat.as_ref().unwrap();
    assert_eq!(c.player.block, 3);
    assert_eq!(c.piles.exhaust_pile.len(), 1);
    assert!(matches!(
        c.decision,
        Some(CombatDecisionState::ForeignInfluenceCardReward {
            source_card: None,
            ..
        })
    ));
    let done = apply_run_action(&copied, RunAction::SkipCombatCardReward).unwrap();
    assert_eq!(done.combat.as_ref().unwrap().player.block, 3);
    assert_eq!(done.combat.as_ref().unwrap().piles.exhaust_pile.len(), 1);
}
#[test]
fn later_potion_stays_after_the_native_source_use_card_boundary() {
    let mut r = open(&setup(false));
    r.gain_potion(crate::Potion::Energy).unwrap();
    let n = apply_run_action(
        &r,
        RunAction::UsePotion {
            slot: 0,
            target: None,
        },
    )
    .unwrap();
    let Some(CombatDecisionState::ForeignInfluenceCardReward {
        pending_actions, ..
    }) = &n.combat.as_ref().unwrap().decision
    else {
        panic!()
    };
    let source = pending_actions
        .iter()
        .position(|a| matches!(a, InternalAction::SettleForeignInfluenceSource))
        .unwrap();
    let potion = pending_actions
        .iter()
        .position(|a| matches!(a, InternalAction::GainEnergyFromPotion { .. }))
        .unwrap();
    assert!(source < potion);
    let n = apply_run_action(&n, RunAction::SkipCombatCardReward).unwrap();
    assert_eq!(n.combat.as_ref().unwrap().player.energy, 5);
}
#[test]
fn havoc_defers_physical_source_exhaustion_until_the_modal_closes() {
    let mut r = setup(false);
    let c = r.combat.as_mut().unwrap();
    let foreign = c.piles.hand.pop().unwrap();
    c.piles.draw_pile.push(foreign);
    c.piles.hand.push(CardInstance {
        combat_only: true,
        ..CardInstance::new(CardId::new(2000), HAVOC_ID)
    });
    let n = apply_combat_action_on_run(
        &r,
        CombatAction::PlayCard {
            card_id: CardId::new(2000),
            target: None,
        },
    )
    .unwrap();
    // Havoc itself discards (its exhaust flag belongs to the autoplayed card).
    assert!(n.combat.as_ref().unwrap().piles.exhaust_pile.is_empty());
    assert!(n
        .combat
        .as_ref()
        .unwrap()
        .piles
        .discard_pile
        .iter()
        .any(|c| c.content_id == HAVOC_ID));
    assert!(matches!(
        n.combat.as_ref().unwrap().decision,
        Some(CombatDecisionState::ForeignInfluenceCardReward {
            source_card: Some(_),
            ..
        })
    ));
    let n = apply_run_action(&n, RunAction::SkipCombatCardReward).unwrap();
    assert_eq!(n.combat.as_ref().unwrap().piles.exhaust_pile.len(), 1);
    assert_eq!(n.combat.as_ref().unwrap().player.energy, 2);
}
#[test]
fn snapshot_restore_preserves_owned_streams_pending_source_and_named_draw_attribution() {
    let mut r = setup(false);
    r.combat.as_mut().unwrap().duplication_potion_pending = true;
    r.combat.as_mut().unwrap().duplication_potion_stacks = 1;
    let r = open(&r);
    let restored: RunState = serde_json::from_value(serde_json::to_value(&r).unwrap()).unwrap();
    let a = apply_run_action(&r, RunAction::SkipCombatCardReward).unwrap();
    let (b, events) = crate::rng::capture_rng_trace(|| {
        apply_run_action(&restored, RunAction::SkipCombatCardReward).unwrap()
    });
    assert_eq!(
        serde_json::to_value(a).unwrap(),
        serde_json::to_value(&b).unwrap()
    );
    let events: Vec<_> = events
        .iter()
        .filter(|e| e.source_file.ends_with("content/foreign_influence.rs"))
        .collect();
    assert!(events
        .iter()
        .any(|e| e.stream == crate::rng::RngTraceStream::CardReward));
    assert!(events
        .iter()
        .any(|e| e.stream == crate::rng::RngTraceStream::CardRandom));
    assert!(events.iter().all(|e| e.source_line > 0));
    let before = serde_json::to_value(&b).unwrap();
    let (_, events) = crate::rng::capture_rng_trace(|| {
        let _ = crate::run::decision::legal_run_decision_actions(&b);
        let _ = serde_json::to_value(&b).unwrap();
    });
    assert!(events.is_empty());
    assert_eq!(before, serde_json::to_value(b).unwrap());
}
#[test]
fn distilled_chaos_keeps_both_owned_rng_counters_through_the_modal() {
    let mut r = setup(false);
    let c = r.combat.as_mut().unwrap();
    let source = c.piles.hand.pop().unwrap();
    c.piles.draw_pile = vec![
        CardInstance {
            combat_only: true,
            ..CardInstance::new(CardId::new(3000), STRIKE_R_ID)
        },
        CardInstance {
            combat_only: true,
            ..CardInstance::new(CardId::new(3001), STRIKE_R_ID)
        },
        source,
    ];
    r.gain_potion(crate::Potion::DistilledChaos).unwrap();
    let n = apply_run_action(
        &r,
        RunAction::UsePotion {
            slot: 0,
            target: None,
        },
    )
    .unwrap();
    assert!(n
        .combat
        .as_ref()
        .unwrap()
        .combat_card_reward_choices()
        .is_some());
    assert_eq!(
        n.card_rng_counter,
        n.combat
            .as_ref()
            .unwrap()
            .run_card_rng
            .as_ref()
            .unwrap()
            .counter()
    );
    assert!(n.card_rng_counter > r.card_rng_counter);
    let n = apply_run_action(&n, RunAction::SkipCombatCardReward).unwrap();
    assert!(n.combat.as_ref().unwrap().decision.is_none());
    // DistilledChaosPotion passes exhaustOnUseOnce=false; only the Skill
    // with its own Exhaust keyword exhausts, while both ordinary Strikes discard.
    assert_eq!(n.combat.as_ref().unwrap().piles.exhaust_pile.len(), 1);
    assert_eq!(n.combat.as_ref().unwrap().piles.discard_pile.len(), 2);
}
#[test]
fn standalone_combat_without_run_card_rng_fails_closed_without_mutation() {
    let r = setup(false);
    let c = r.combat.as_ref().unwrap();
    let before = serde_json::to_value(c).unwrap();
    assert!(apply_combat_action(
        c,
        CombatAction::PlayCard {
            card_id: CardId::new(1000),
            target: None
        }
    )
    .is_err());
    assert_eq!(before, serde_json::to_value(c).unwrap());
    let mut supplied = c.clone();
    supplied.run_card_rng = Some(r.rng_for_stream(RunRngStream::CardReward));
    assert!(apply_combat_action(
        &supplied,
        CombatAction::PlayCard {
            card_id: CardId::new(1000),
            target: None
        }
    )
    .is_ok());
}
