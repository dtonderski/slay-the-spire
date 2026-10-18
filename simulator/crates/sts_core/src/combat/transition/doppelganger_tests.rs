//! PC DoppelgangerAction / DrawCardNextTurnPower; FIDL01504/1176 is natural evidence.
use super::*;
use crate::content::cards::{DOPPELGANGER_ANY_COLOR_ID, HAVOC_ID, STRIKE_R_ID};
fn setup(energy: i32, upgraded: bool) -> CombatState {
    let mut s = CombatState::cultist_fixture();
    s.player.authority.relics.clear();
    s.player.energy = energy;
    s.monsters[0].hp = 500;
    s.monsters[0].max_hp = 500;
    let mut c = CardInstance::new(CardId::new(41), DOPPELGANGER_ANY_COLOR_ID);
    c.upgrades = u8::from(upgraded);
    s.piles.hand = vec![c];
    s.piles.draw_pile.clear();
    s.piles.discard_pile.clear();
    s.piles.exhaust_pile.clear();
    s.validate().unwrap();
    s
}
fn play(s: &CombatState) -> CombatTransition {
    apply_combat_action_with_events(
        s,
        CombatAction::PlayCard {
            card_id: CardId::new(41),
            target: None,
        },
    )
    .unwrap()
}
#[test]
fn base_upgrade_chemical_x_artifact_no_draw_energy_and_rng() {
    for energy in 0..=3 {
        for upgraded in [false, true] {
            for chemical in [false, true] {
                let mut s = setup(energy, upgraded);
                s.player.powers.artifact = 1;
                s.player.cannot_draw = true;
                if chemical {
                    s.player.authority.relics.push(Relic::ChemicalX);
                }
                let before = serde_json::to_value(&s).unwrap();
                let _ = crate::combat::legal_combat_actions(&s);
                let n = play(&s).state;
                let amount = energy + i32::from(upgraded) + if chemical { 2 } else { 0 };
                assert_eq!(n.player.energy, 0);
                assert_eq!(n.player.energy_next_turn, amount);
                assert_eq!(n.player.powers.draw_card, amount);
                assert_eq!(n.player.powers.artifact, 1);
                assert!(n.piles.hand.is_empty());
                assert_eq!(n.piles.exhaust_pile.len(), 1);
                assert_eq!(n.rng, s.rng);
                assert_eq!(serde_json::to_value(&s).unwrap(), before);
            }
        }
    }
}
#[test]
fn live_energy_spending_is_separate_from_reserved_x_and_free_autoplay() {
    for (reserved, upgraded, free, energy) in [
        (1, false, false, 0),
        (0, false, false, 5),
        (0, true, false, 0),
        (1, false, true, 5),
    ] {
        let s = setup(3, false);
        let n = process_internal_queue(
            &s,
            VecDeque::from([
                InternalAction::GainEnergy { amount: 2 },
                InternalAction::ResolveDoppelganger {
                    energy_on_use: reserved,
                    upgraded,
                    free_to_play_once: free,
                },
            ]),
        )
        .unwrap()
        .state;
        assert_eq!(n.player.energy, energy);
        assert_eq!(n.player.energy_next_turn, reserved + i32::from(upgraded));
        assert_eq!(n.player.powers.draw_card, reserved + i32::from(upgraded));
    }
}
#[test]
fn next_turn_draw_is_separate_from_reduced_opening_draw_and_then_expires() {
    for reduced in [false, true] {
        let mut s = setup(2, false);
        s.player.powers.draw_reduction = i32::from(reduced);
        s.piles.draw_pile = (50..65)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        let n = play(&s).state;
        let n = crate::combat::end_player_turn(&n).unwrap();
        assert_eq!(n.player.energy, 5);
        assert_eq!(n.player.energy_next_turn, 0);
        assert_eq!(n.player.powers.draw_card, 0);
        assert_eq!(n.piles.hand.len(), if reduced { 6 } else { 7 });
        if reduced {
            assert!(n.player.powers.draw_reduction_first_draw_seen);
        }
        let n = crate::combat::end_player_turn(&n).unwrap();
        assert_eq!(n.player.energy, 3);
        assert_eq!(n.piles.hand.len(), if reduced { 4 } else { 5 });
    }
}
#[test]
fn original_source_and_power_actions_finish_before_the_copy() {
    let mut s = setup(3, true);
    s.duplication_potion_pending = true;
    s.duplication_potion_stacks = 1;
    s.player.powers.feel_no_pain = 3;
    let n = play(&s);
    assert_eq!(n.state.player.energy, 0);
    assert_eq!(n.state.player.energy_next_turn, 8);
    assert_eq!(n.state.player.powers.draw_card, 8);
    assert_eq!(n.state.player.block, 3);
    assert_eq!(n.state.piles.exhaust_pile.len(), 1);
    let index = |f: fn(&InternalAction) -> bool| n.event_log.iter().position(f).unwrap();
    let source = index(|a| matches!(a, InternalAction::CardExhausted { .. }));
    let energized = index(|a| matches!(a, InternalAction::ApplyEnergized { .. }));
    let draw = index(|a| matches!(a, InternalAction::ApplyDrawCardNextTurn { .. }));
    let copied = index(|a| matches!(a, InternalAction::PlayCardCopy { .. }));
    assert!(
        source < energized && energized < draw && draw < copied,
        "{:?}",
        n.event_log
    );
    let wrappers: Vec<_> = n
        .event_log
        .iter()
        .filter_map(|a| {
            if let InternalAction::ResolveDoppelganger {
                free_to_play_once, ..
            } = a
            {
                Some(*free_to_play_once)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(wrappers, vec![false, true]);
}
#[test]
fn havoc_sundial_retains_reserved_x_and_does_not_spend_later_energy() {
    let mut s = setup(3, true);
    let c = s.piles.hand.pop().unwrap();
    s.piles
        .hand
        .push(CardInstance::new(CardId::new(99), HAVOC_ID));
    s.piles.discard_pile.push(c);
    s.player.authority.relics = vec![Relic::Sundial];
    s.sundial_shuffles = 2;
    let n = apply_combat_action(
        &s,
        CombatAction::PlayCard {
            card_id: CardId::new(99),
            target: None,
        },
    )
    .unwrap();
    assert_eq!(n.player.energy, 4);
    assert_eq!(n.player.energy_next_turn, 3);
    assert_eq!(n.player.powers.draw_card, 3);
}
#[test]
fn queued_powers_can_be_cancelled_after_wrapper_without_repairing_spent_energy() {
    let mut s = setup(2, false);
    s.monsters[0].hp = 1;
    let target = s.monsters[0].id;
    let n = process_internal_queue(
        &s,
        VecDeque::from([
            InternalAction::ResolveDoppelganger {
                energy_on_use: 2,
                upgraded: false,
                free_to_play_once: false,
            },
            InternalAction::DealUnmodifiedDamage { target, amount: 1 },
            InternalAction::MoveCard {
                card_id: CardId::new(41),
                from: CardPile::Hand,
                to: CardPile::ExhaustPile,
            },
        ]),
    )
    .unwrap()
    .state;
    assert_eq!(n.player.energy, 0);
    assert_eq!(n.player.energy_next_turn, 0);
    assert_eq!(n.player.powers.draw_card, 0);
    assert_eq!(n.piles.exhaust_pile.len(), 1);
    // Cleanup is point-in-time, not a global prohibition on later-created work.
    let late = process_internal_queue(
        &n,
        VecDeque::from([InternalAction::ResolveDoppelganger {
            energy_on_use: 1,
            upgraded: false,
            free_to_play_once: true,
        }]),
    )
    .unwrap()
    .state;
    assert_eq!(late.player.energy_next_turn, 1);
    assert_eq!(late.player.powers.draw_card, 1);
}
#[test]
fn energized_caps_both_creation_and_recharge_but_draw_amount_is_not_capped() {
    let mut s = setup(999, true);
    s.player.authority.relics.push(Relic::ChemicalX);
    s.piles.draw_pile = (50..70)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    let mut n = play(&s).state;
    assert_eq!(n.player.energy_next_turn, 999);
    assert_eq!(n.player.powers.draw_card, 1002);
    crate::combat::turn::start_player_turn(&mut n).unwrap();
    assert_eq!(n.player.energy, 999);
    assert_eq!(n.piles.hand.len(), 10);
    assert_eq!(n.player.powers.draw_card, 0);
}
#[test]
fn snapshots_keep_owned_pending_context_and_overflow_rejects_atomically() {
    let s = setup(3, true);
    let q = VecDeque::from([InternalAction::ResolveDoppelganger {
        energy_on_use: 2,
        upgraded: true,
        free_to_play_once: false,
    }]);
    let raw = serde_json::to_string(&(s.clone(), q.clone())).unwrap();
    let (r, rq): (CombatState, VecDeque<InternalAction>) = serde_json::from_str(&raw).unwrap();
    let a = process_internal_queue(&s, q).unwrap().state;
    let b = process_internal_queue(&r, rq).unwrap().state;
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(b).unwrap()
    );
    let old = serde_json::to_value(&s).unwrap();
    assert!(old["player"]["powers"].get("draw_card").is_none());
    let legacy: CombatState = serde_json::from_value(old).unwrap();
    assert_eq!(legacy.player.powers.draw_card, 0);
    let mut overflow = s;
    overflow.player.authority.relics.push(Relic::ChemicalX);
    let before = serde_json::to_value(&overflow).unwrap();
    assert!(process_internal_queue(
        &overflow,
        VecDeque::from([InternalAction::ResolveDoppelganger {
            energy_on_use: i32::MAX,
            upgraded: true,
            free_to_play_once: false
        }])
    )
    .is_err());
    assert_eq!(before, serde_json::to_value(&overflow).unwrap());
    overflow.player.powers.draw_card = i32::MAX;
    assert!(process_internal_queue(
        &overflow,
        VecDeque::from([InternalAction::ApplyDrawCardNextTurn { amount: 1 }])
    )
    .is_err());
    overflow.player.powers.draw_card = -1;
    assert!(overflow.validate().is_err());
}
