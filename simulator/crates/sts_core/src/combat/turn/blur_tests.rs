//! PC Blur.use, BlurPower.atEndOfRound, and GameActionManager block-loss order.
use super::*;
use crate::{
    action::{CombatAction, InternalAction},
    card::CardInstance,
    combat::transition::{apply_combat_action, process_internal_queue},
    content::cards::BLUR_ANY_COLOR_ID,
    ids::CardId,
    relic::Relic,
};
fn setup(upgraded: bool) -> CombatState {
    let mut s = CombatState::cultist_fixture();
    s.player.authority.relics.clear();
    s.monsters[0].hp = 500;
    s.monsters[0].max_hp = 500;
    let mut card = CardInstance::new(CardId::new(41), BLUR_ANY_COLOR_ID);
    card.upgrades = u8::from(upgraded);
    s.piles.hand = vec![card];
    s.piles.draw_pile.clear();
    s.piles.discard_pile.clear();
    s.piles.exhaust_pile.clear();
    s.validate().unwrap();
    s
}
fn play(s: &CombatState) -> CombatState {
    apply_combat_action(
        s,
        CombatAction::PlayCard {
            card_id: CardId::new(41),
            target: None,
        },
    )
    .unwrap()
}
#[test]
fn blur_identity_base_upgrade_modifiers_no_block_and_artifact() {
    assert_eq!(
        BLUR_ANY_COLOR_ID,
        crate::content::shop_pool::shop_card_content_id("BLUR")
    );
    for upgraded in [false, true] {
        for no_block in [false, true] {
            let mut s = setup(upgraded);
            s.player.powers.dexterity = 2;
            s.player.powers.frail = 1;
            s.player.powers.artifact = 1;
            s.player.cannot_draw = true;
            s.player.no_block_turns = i32::from(no_block);
            let before = serde_json::to_value(&s).unwrap();
            let _ = crate::combat::legal_combat_actions(&s);
            let n = play(&s);
            assert_eq!(
                n.player.block,
                if no_block {
                    0
                } else if upgraded {
                    7
                } else {
                    5
                }
            );
            assert_eq!(n.player.powers.blur, 1);
            assert_eq!(n.player.powers.artifact, 1); // BUFF, not an Artifact target
            assert_eq!(n.player.energy, 2);
            assert!(n.piles.discard_pile.iter().any(|c| c.id == CardId::new(41)));
            assert_eq!(n.rng, s.rng);
            assert_eq!(serde_json::to_value(&s).unwrap(), before);
        }
    }
}
#[test]
fn additive_duration_retains_before_expiring_and_then_allows_normal_loss() {
    let mut s = setup(false);
    s.player.block = 50;
    s.player.powers.blur = 2;
    for remaining in [1, 0] {
        apply_turn_transition_block_loss(&mut s);
        assert_eq!(s.player.block, 50);
        assert_eq!(s.player.powers.blur, remaining);
    }
    apply_turn_transition_block_loss(&mut s);
    assert_eq!(s.player.block, 0);
}
#[test]
fn calipers_and_barricade_do_not_prevent_blur_expiry() {
    for barricade in [false, true] {
        let mut s = setup(false);
        s.player.authority.relics.push(Relic::Calipers);
        s.player.powers.barricade = i32::from(barricade);
        s.player.powers.blur = 1;
        s.player.block = 50;
        apply_turn_transition_block_loss(&mut s);
        assert_eq!(s.player.block, 50);
        assert_eq!(s.player.powers.blur, 0);
        apply_turn_transition_block_loss(&mut s);
        assert_eq!(s.player.block, if barricade { 50 } else { 35 });
    }
}
#[test]
fn copied_blur_stacks_duration_but_moves_source_only_once() {
    for upgraded in [false, true] {
        let mut s = setup(upgraded);
        s.duplication_potion_pending = true;
        s.duplication_potion_stacks = 1;
        let n = play(&s);
        assert_eq!(n.player.powers.blur, 2);
        assert_eq!(n.player.block, if upgraded { 16 } else { 10 });
        assert_eq!(n.player.energy, 2);
        assert_eq!(
            n.piles
                .discard_pile
                .iter()
                .filter(|c| c.id == CardId::new(41))
                .count(),
            1
        );
    }
}
#[test]
fn block_is_prepared_before_callbacks_and_copy_reprepares_after_queued_cleanse() {
    {
        let mut s = setup(false);
        s.player.powers.frail = 1;
        let prepared = 3;
        // Source-backed action/card-lane boundary, not a claim about every
        // existing relic callback scheduler. Native cleanse precedes the copy.
        let n = process_internal_queue(
            &s,
            VecDeque::from([
                InternalAction::GainBlurBlock {
                    amount: prepared,
                    upgraded: false,
                },
                InternalAction::ApplyBlur { amount: 1 },
                InternalAction::ClearPlayerDebuffs,
                InternalAction::MoveCard {
                    card_id: CardId::new(41),
                    from: crate::action::CardPile::Hand,
                    to: crate::action::CardPile::DiscardPile,
                },
                InternalAction::PlayCardCopy {
                    card_id: CardId::new(41),
                    content_id: BLUR_ANY_COLOR_ID,
                },
                InternalAction::GainBlurBlock {
                    amount: prepared,
                    upgraded: false,
                },
                InternalAction::ApplyBlur { amount: 1 },
                InternalAction::EndCopiedCardEffects,
            ]),
        )
        .unwrap()
        .state;
        assert_eq!(n.player.block, 8);
        assert_eq!(n.player.powers.blur, 2);
        assert_eq!(n.player.powers.frail, 0);
        assert_eq!(n.player.no_block_turns, 0);
    }
    let mut s = setup(false);
    s.player.powers.frail = 1;
    let n = process_internal_queue(
        &s,
        VecDeque::from([
            InternalAction::ClearPlayerDebuffs,
            InternalAction::GainBlurBlock {
                amount: 3,
                upgraded: false,
            },
        ]),
    )
    .unwrap()
    .state;
    assert_eq!(n.player.block, 3); // already-prepared block is never recalculated at gain
}

#[test]
fn full_round_and_revival_boundary_preserve_once_before_expiry() {
    let n = play(&setup(true));
    let first = end_player_turn(&n).unwrap(); // Cultist's opening Incantation does no damage
    assert_eq!(first.player.block, 8);
    assert_eq!(first.player.powers.blur, 0);
    let second = end_player_turn(&first).unwrap();
    assert_eq!(second.player.block, 0);
    let mut revival = setup(false);
    revival.player.block = 12;
    revival.player.powers.blur = 1;
    finish_monster_turn_after_player_revival(&mut revival).unwrap();
    assert_eq!(revival.player.block, 12);
    assert_eq!(revival.player.powers.blur, 0);
}
#[test]
fn snapshots_preserve_duration_and_pending_application_fail_closed_on_overflow() {
    let s = play(&setup(false));
    let raw = serde_json::to_string(&s).unwrap();
    let mut restored: CombatState = serde_json::from_str(&raw).unwrap();
    let mut original = s.clone();
    apply_turn_transition_block_loss(&mut original);
    apply_turn_transition_block_loss(&mut restored);
    assert_eq!(
        serde_json::to_value(original).unwrap(),
        serde_json::to_value(restored).unwrap()
    );
    let pending = VecDeque::from([InternalAction::ApplyBlur { amount: 1 }]);
    let queued_json = serde_json::to_string(&(s.clone(), pending.clone())).unwrap();
    let (queued_state, restored_queue): (CombatState, VecDeque<InternalAction>) =
        serde_json::from_str(&queued_json).unwrap();
    let a = process_internal_queue(&s, pending).unwrap().state;
    let b = process_internal_queue(&queued_state, restored_queue)
        .unwrap()
        .state;
    assert_eq!(a.player.powers.blur, 2);
    assert_eq!(
        serde_json::to_value(a).unwrap(),
        serde_json::to_value(b).unwrap()
    );
    let before = serde_json::to_value(&s).unwrap();
    assert!(process_internal_queue(
        &s,
        VecDeque::from([InternalAction::ApplyBlur { amount: i32::MAX }])
    )
    .is_err());
    assert_eq!(before, serde_json::to_value(&s).unwrap());
    let mut invalid = s;
    invalid.player.powers.blur = -1;
    assert!(invalid.validate().is_err());
    let old = serde_json::to_value(setup(false)).unwrap();
    assert!(old["player"]["powers"].get("blur").is_none());
    let legacy: CombatState = serde_json::from_value(old).unwrap();
    assert_eq!(legacy.player.powers.blur, 0);
}
#[test]
fn queued_blur_application_is_cancelled_by_native_post_lethal_clear() {
    let mut s = setup(false);
    s.monsters[0].hp = 1;
    let target = s.monsters[0].id;
    let n = process_internal_queue(
        &s,
        VecDeque::from([
            InternalAction::DealUnmodifiedDamage { target, amount: 1 },
            InternalAction::ApplyBlur { amount: 1 },
        ]),
    )
    .unwrap()
    .state;
    assert_eq!(n.player.powers.blur, 0);
}
