//! Supplemental PC constructor controls; FIDL01364/1602 is immutable natural evidence.
use super::*;
use crate::content::cards::{
    DAZED_ID, HAVOC_ID, MASTER_REALITY_ANY_COLOR_ID, SEARING_BLOW_ID, SHRUG_IT_OFF_ID,
    SHRUG_IT_OFF_PLUS_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID, WOUND_ID,
};
fn setup(upgraded: bool) -> CombatState {
    let mut s = CombatState::cultist_fixture();
    s.player.authority.relics.clear();
    s.player.energy = 3;
    s.monsters[0].hp = 500;
    s.monsters[0].max_hp = 500;
    s.piles.draw_pile.clear();
    s.piles.discard_pile.clear();
    s.piles.exhaust_pile.clear();
    let mut c = CardInstance::new(CardId::new(41), MASTER_REALITY_ANY_COLOR_ID);
    c.upgrades = u8::from(upgraded);
    s.piles.hand = vec![c];
    s
}
#[test]
fn base_upgrade_payment_artifact_no_draw_source_removal_and_nonstacking_marker() {
    for upgraded in [false, true] {
        let mut s = setup(upgraded);
        s.player.powers.artifact = 2;
        s.player.cannot_draw = true;
        let n = apply_combat_action(
            &s,
            CombatAction::PlayCard {
                card_id: CardId::new(41),
                target: None,
            },
        )
        .unwrap();
        assert_eq!(n.player.energy, if upgraded { 3 } else { 2 });
        assert!(n.player.powers.master_reality);
        assert_eq!(n.player.powers.artifact, 2);
        assert!(n.player.cannot_draw);
        assert!(
            n.piles.hand.is_empty()
                && n.piles.discard_pile.is_empty()
                && n.piles.exhaust_pile.is_empty()
        );
        assert_eq!(n.rng, s.rng);
        let again = process_internal_queue(
            &n,
            VecDeque::from([
                InternalAction::GainMasterReality,
                InternalAction::GainMasterReality,
            ]),
        )
        .unwrap()
        .state;
        assert!(again.player.powers.master_reality);
        assert_eq!(again.player.energy, n.player.energy);
    }
}
#[test]
fn created_hand_discard_and_draw_cards_upgrade_without_changing_rng_or_moves() {
    for to in [CardPile::Hand, CardPile::DiscardPile, CardPile::DrawPile] {
        let mut s = setup(false);
        s.player.powers.master_reality = true;
        let n = process_internal_queue(
            &s,
            VecDeque::from([InternalAction::AddGeneratedCardToPile {
                content_id: STRIKE_R_ID,
                to,
                temp_cost: None,
                temp_cost_turn_only: false,
            }]),
        )
        .unwrap()
        .state;
        let c = match to {
            CardPile::Hand => n.piles.hand.last(),
            CardPile::DiscardPile => n.piles.discard_pile.last(),
            _ => n.piles.draw_pile.last(),
        }
        .unwrap();
        assert_eq!(c.content_id, STRIKE_R_PLUS_ID);
        assert_eq!(n.rng, s.rng);
    }
    let mut s = setup(false);
    s.player.powers.master_reality = true;
    s.piles.hand = vec![CardInstance::new(
        CardId::new(41),
        crate::content::cards::EXHUME_ID,
    )];
    s.piles.exhaust_pile = vec![CardInstance::new(CardId::new(42), STRIKE_R_ID)];
    let n = apply_combat_action(
        &s,
        CombatAction::PlayCard {
            card_id: CardId::new(41),
            target: None,
        },
    )
    .unwrap();
    assert_eq!(
        n.piles.hand.last().unwrap().content_id,
        STRIKE_R_ID,
        "physical returns are not creation constructors"
    );
}
#[test]
fn status_and_curse_do_not_upgrade_but_generic_regular_cards_do() {
    let mut s = setup(false);
    s.player.powers.master_reality = true;
    for id in [WOUND_ID, DAZED_ID, NECRONOMICURSE_ID] {
        let c = CardInstance::new(CardId::new(50), id);
        assert_eq!(upgrade_created_card_for_master_reality(&s, c).unwrap(), c);
    }
    let id = crate::content::shop_pool::shop_card_content_id("BARRAGE");
    assert!(get_card_definition(id).is_none());
    let c = CardInstance::new(CardId::new(50), id);
    let ((n, ()), events) = crate::rng::capture_rng_trace(|| {
        (upgrade_created_card_for_master_reality(&s, c).unwrap(), ())
    });
    assert_eq!(n.upgrades, 1);
    assert!(events.is_empty());
    assert_eq!(c.upgrades, 0);
    assert_eq!(upgrade_created_card_for_master_reality(&s, n).unwrap(), n);
}
#[test]
fn full_hand_creation_and_stat_equivalent_copy_upgrade_but_not_the_source() {
    let mut s = setup(false);
    s.player.powers.master_reality = true;
    s.piles.hand = (1..=10)
        .map(|i| CardInstance::new(CardId::new(i), STRIKE_R_ID))
        .collect();
    let source = s.piles.hand[0];
    let n = process_internal_queue(
        &s,
        VecDeque::from([
            InternalAction::AddStatEquivalentCopyToPile {
                card: source,
                to: CardPile::Hand,
            },
            InternalAction::AddGeneratedCardToPile {
                content_id: SHRUG_IT_OFF_ID,
                to: CardPile::Hand,
                temp_cost: None,
                temp_cost_turn_only: false,
            },
        ]),
    )
    .unwrap()
    .state;
    assert_eq!(n.piles.hand.len(), 10);
    assert!(n.piles.hand.iter().all(|c| c.content_id == STRIKE_R_ID));
    assert_eq!(
        n.piles
            .discard_pile
            .iter()
            .map(|c| c.content_id)
            .collect::<Vec<_>>(),
        vec![STRIKE_R_PLUS_ID, SHRUG_IT_OFF_PLUS_ID]
    );
}
#[test]
fn dual_wield_upgrades_the_created_copy_without_touching_the_original() {
    let mut s = setup(false);
    s.player.powers.master_reality = true;
    s.piles.hand = vec![
        CardInstance::new(CardId::new(41), crate::content::cards::DUAL_WIELD_ID),
        CardInstance::new(CardId::new(42), STRIKE_R_ID),
    ];
    let n = apply_combat_action(
        &s,
        CombatAction::PlayCard {
            card_id: CardId::new(41),
            target: None,
        },
    )
    .unwrap();
    assert_eq!(
        n.piles
            .hand
            .iter()
            .filter(|c| c.content_id == STRIKE_R_ID)
            .count(),
        1
    );
    assert_eq!(
        n.piles
            .hand
            .iter()
            .filter(|c| c.content_id == STRIKE_R_PLUS_ID)
            .count(),
        1
    );
    assert_eq!(n.rng, s.rng);
}

#[test]
fn already_upgraded_ordinary_card_stays_plus_but_searing_blow_copy_gets_another_upgrade() {
    let mut s = setup(false);
    s.player.powers.master_reality = true;
    let c = CardInstance::new(CardId::new(50), STRIKE_R_PLUS_ID);
    assert_eq!(upgrade_created_card_for_master_reality(&s, c).unwrap(), c);
    let c = upgrade_card_instance(CardInstance::new(CardId::new(50), SEARING_BLOW_ID))
        .unwrap()
        .unwrap();
    assert_eq!(c.searing_blow_upgrades, 1);
    let n = upgrade_created_card_for_master_reality(&s, c).unwrap();
    assert_eq!(n.searing_blow_upgrades, 2);
    assert_eq!(c.searing_blow_upgrades, 1);
}
#[test]
fn constructor_upgrade_preserves_free_cost_then_corruption_applies_afterward() {
    let mut s = setup(false);
    s.player.powers.master_reality = true;
    let n = process_internal_queue(
        &s,
        VecDeque::from([InternalAction::AddGeneratedCardToPile {
            content_id: HAVOC_ID,
            to: CardPile::Hand,
            temp_cost: Some(0),
            temp_cost_turn_only: true,
        }]),
    )
    .unwrap()
    .state;
    let c = n.piles.hand.last().unwrap();
    assert_eq!(c.content_id, crate::content::cards::HAVOC_PLUS_ID);
    assert_eq!(c.temp_cost, Some(0));
    assert!(c.temp_cost_turn_only);
    s.player.powers.corruption = 1;
    let n = process_internal_queue(
        &s,
        VecDeque::from([InternalAction::AddGeneratedCardToPile {
            content_id: SHRUG_IT_OFF_ID,
            to: CardPile::Hand,
            temp_cost: None,
            temp_cost_turn_only: false,
        }]),
    )
    .unwrap()
    .state;
    let c = n.piles.hand.last().unwrap();
    assert_eq!(c.content_id, SHRUG_IT_OFF_PLUS_ID);
    assert_eq!(c.temp_cost, Some(0));
}
#[test]
fn master_reality_upgrade_base_cost_uses_live_written_cost_and_turn_delta() {
    for (cost, turn_only, under, expected) in [
        (0, false, None, 0),
        (2, false, None, 0),
        (2, true, None, 1),
        (0, true, None, 0),
        (1, true, Some(2), 0),
    ] {
        let mut c = CardInstance::new(CardId::new(50), MASTER_REALITY_ANY_COLOR_ID);
        c.temp_cost = Some(cost);
        c.temp_cost_turn_only = turn_only;
        c.combat_cost_under_turn_override = under;
        let n = upgrade_card_instance(c).unwrap().unwrap();
        assert_eq!(n.upgrades, 1);
        assert_eq!(n.temp_cost, Some(expected));
        assert_eq!(n.combat_cost_under_turn_override, under.map(|_| 0));
        assert_eq!(
            upgrade_card_instance(n).unwrap(),
            None,
            "ordinary card upgrade is not repeatable"
        );
    }
}

#[test]
fn snapshots_legacy_default_and_autoplay_preserve_the_nonstacking_marker() {
    let mut s = setup(false);
    s.player.powers.master_reality = true;
    let encoded = serde_json::to_value(&s).unwrap();
    let decoded: CombatState = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(decoded, s);
    let mut legacy = encoded;
    legacy["player"]["powers"]
        .as_object_mut()
        .unwrap()
        .remove("master_reality");
    let old: CombatState = serde_json::from_value(legacy).unwrap();
    assert!(!old.player.powers.master_reality);
    let mut s = setup(false);
    s.piles.hand = vec![CardInstance::new(CardId::new(41), HAVOC_ID)];
    s.piles.draw_pile = vec![CardInstance::new(
        CardId::new(42),
        MASTER_REALITY_ANY_COLOR_ID,
    )];
    let n = apply_combat_action(
        &s,
        CombatAction::PlayCard {
            card_id: CardId::new(41),
            target: None,
        },
    )
    .unwrap();
    assert_eq!(n.player.energy, 2);
    assert!(n.player.powers.master_reality);
}
#[test]
fn point_in_time_lethal_clear_cancels_already_queued_application_not_later_created_work() {
    let mut s = setup(false);
    s.monsters[0].hp = 1;
    let n = process_internal_queue(
        &s,
        VecDeque::from([
            InternalAction::DealUnmodifiedDamage {
                target: s.monsters[0].id,
                amount: 1,
            },
            InternalAction::GainMasterReality,
        ]),
    )
    .unwrap()
    .state;
    assert!(!n.player.powers.master_reality);
    let n = process_internal_queue(&n, VecDeque::from([InternalAction::GainMasterReality]))
        .unwrap()
        .state;
    assert!(n.player.powers.master_reality);
}
