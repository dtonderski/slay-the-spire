//! PC Perseverance / RestoreRetainedCardsAction, supplemental to FIDL00213.
use super::*;
use crate::content::cards::{HAVOC_ID, PERSEVERANCE_ANY_COLOR_ID, STRIKE_R_ID};
fn setup(upgraded: bool, growth: i32) -> CombatState {
    let mut s = CombatState::cultist_fixture();
    s.player.authority.relics.clear();
    s.player.energy = 3;
    s.monsters[0].hp = 500;
    s.monsters[0].max_hp = 500;
    let mut c = CardInstance::new(CardId::new(41), PERSEVERANCE_ANY_COLOR_ID);
    c.upgrades = u8::from(upgraded);
    c.perseverance_retain_block = growth;
    s.piles.hand = vec![c];
    s.piles.draw_pile.clear();
    s.piles.discard_pile.clear();
    s.piles.exhaust_pile.clear();
    s
}
fn play(s: &CombatState) -> SimResult<CombatState> {
    apply_combat_action(
        s,
        CombatAction::PlayCard {
            card_id: CardId::new(41),
            target: None,
        },
    )
}
#[test]
fn native_identity_cost_base_upgrade_modifiers_and_no_block() {
    assert_eq!(
        PERSEVERANCE_ANY_COLOR_ID,
        crate::content::shop_pool::shop_card_content_id("PERSEVERANCE")
    );
    for upgraded in [false, true] {
        for no_block in [false, true] {
            let mut s = setup(upgraded, 4);
            s.player.powers.dexterity = 2;
            s.player.powers.frail = 1;
            s.player.powers.artifact = 1;
            s.player.cannot_draw = true;
            s.player.no_block_turns = i32::from(no_block);
            let before = serde_json::to_value(&s).unwrap();
            let _ = crate::combat::legal_combat_actions(&s);
            let n = play(&s).unwrap();
            assert_eq!(
                n.player.block,
                if no_block {
                    0
                } else {
                    if upgraded {
                        9
                    } else {
                        8
                    }
                }
            );
            assert_eq!(n.player.energy, 2);
            assert_eq!(n.player.powers.artifact, 1);
            assert_eq!(n.piles.discard_pile[0].perseverance_retain_block, 4);
            assert_eq!(n.rng, s.rng);
            assert_eq!(serde_json::to_value(s).unwrap(), before);
        }
    }
}
#[test]
fn actual_retains_add_current_magic_and_upgrade_does_not_rescale_old_growth() {
    for mode in 0..3 {
        let mut s = setup(false, 0);
        s.piles
            .hand
            .push(CardInstance::new(CardId::new(42), STRIKE_R_ID));
        if mode == 1 {
            s.player.authority.relics.push(Relic::RunicPyramid);
        }
        if mode == 2 {
            s.player.retain_hand_next_turn = true;
        }
        let rng = s.rng.clone();
        crate::combat::hand::discard_end_of_turn_hand(&mut s).unwrap();
        assert_eq!(s.piles.hand[0].perseverance_retain_block, 2);
        s.piles.hand[0] = upgrade_card_instance(s.piles.hand[0]).unwrap().unwrap();
        assert_eq!(s.piles.hand[0].perseverance_retain_block, 2);
        crate::combat::hand::discard_end_of_turn_hand(&mut s).unwrap();
        assert_eq!(s.piles.hand[0].perseverance_retain_block, 5);
        assert_eq!(play(&s).unwrap().player.block, 12);
        assert_eq!(s.rng, rng);
    }
}
#[test]
fn copies_preserve_growth_original_identity_and_master_reality_adds_only_upgrade() {
    let mut s = setup(false, 4);
    s.player.powers.master_reality = true;
    let source = s.piles.hand[0];
    let n = process_internal_queue(
        &s,
        VecDeque::from([InternalAction::AddStatEquivalentCopyToPile {
            card: source,
            to: CardPile::Hand,
        }]),
    )
    .unwrap()
    .state;
    assert_eq!(n.piles.hand[0], source);
    let copy = n.piles.hand[1];
    assert_ne!(copy.id, source.id);
    assert_eq!(copy.perseverance_retain_block, 4);
    assert_eq!(copy.upgrades, 1);
    let n = apply_combat_action(
        &n,
        CombatAction::PlayCard {
            card_id: copy.id,
            target: None,
        },
    )
    .unwrap();
    assert_eq!(n.player.block, 11);
}
#[test]
fn copied_use_reprepares_modifiers_from_captured_base_block_not_old_amount() {
    let mut s = setup(false, 4);
    s.player.powers.frail = 1;
    let n = process_internal_queue(
        &s,
        VecDeque::from([
            InternalAction::ClearPlayerDebuffs,
            InternalAction::PlayCardCopy {
                card_id: CardId::new(41),
                content_id: PERSEVERANCE_ANY_COLOR_ID,
            },
            InternalAction::GainPerseveranceBlock {
                amount: 6,
                base_block: 9,
            },
            InternalAction::EndCopiedCardEffects,
        ]),
    )
    .unwrap()
    .state;
    assert_eq!(n.player.block, 9);
}
#[test]
fn queued_gain_block_survives_lethal_clear_as_native_gain_block_action() {
    let mut s = setup(false, 0);
    s.monsters[0].hp = 1;
    let n = process_internal_queue(
        &s,
        VecDeque::from([
            InternalAction::DealUnmodifiedDamage {
                target: s.monsters[0].id,
                amount: 1,
            },
            InternalAction::GainPerseveranceBlock {
                amount: 7,
                base_block: 7,
            },
        ]),
    )
    .unwrap()
    .state;
    assert_eq!(n.player.block, 7);
    assert_eq!(n.phase, CombatPhase::Won);
}
#[test]
fn havoc_autoplay_is_free_and_preserves_growth_on_source_exhaustion() {
    let mut s = setup(false, 4);
    let mut c = s.piles.hand[0];
    c.id = CardId::new(42);
    s.piles.hand = vec![CardInstance::new(CardId::new(41), HAVOC_ID)];
    s.piles.draw_pile = vec![c];
    let n = play(&s).unwrap();
    assert_eq!(n.player.energy, 2);
    assert_eq!(n.player.block, 9);
    assert_eq!(
        n.piles
            .exhaust_pile
            .iter()
            .find(|c| c.id == CardId::new(42))
            .unwrap()
            .perseverance_retain_block,
        4
    );
}
#[test]
fn negative_wrong_card_and_base_block_overflow_fail_closed_without_mutation() {
    let mut s = setup(false, -1);
    assert!(s.validate().is_err());
    s.piles.hand[0].perseverance_retain_block = 2;
    s.piles.hand[0].content_id = STRIKE_R_ID;
    assert!(s.validate().is_err());
    let s = setup(false, i32::MAX);
    let before = s.clone();
    assert!(play(&s).is_err());
    assert_eq!(s, before);
    let mut run = crate::run::RunState::seeded_ironclad(1, 0);
    let mut c = setup(false, 2).piles.hand[0];
    c.id = CardId::new(1000);
    assert!(run.add_deck_card(c).is_err());
}
#[test]
fn retain_overflow_is_atomic_at_the_combat_action_boundary() {
    let s = setup(false, i32::MAX);
    let before = s.clone();
    assert!(apply_combat_action(&s, CombatAction::EndTurn).is_err());
    assert_eq!(s, before);
}
#[test]
fn snapshots_and_legacy_zero_preserve_future_replay() {
    let s = setup(true, 5);
    let decoded: CombatState = serde_json::from_value(serde_json::to_value(&s).unwrap()).unwrap();
    assert_eq!(play(&s).unwrap(), play(&decoded).unwrap());
    let mut legacy = serde_json::to_value(setup(false, 0)).unwrap();
    legacy["piles"]["hand"][0]
        .as_object_mut()
        .unwrap()
        .remove("perseverance_retain_block");
    let legacy: CombatState = serde_json::from_value(legacy).unwrap();
    assert_eq!(legacy.piles.hand[0].perseverance_retain_block, 0);
    assert_eq!(play(&legacy).unwrap().player.block, 5);
}
