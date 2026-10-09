//! Initialized source-backed rules tests, NOT new real-game parity evidence.
//! Authority: audited PC 12-18-2022 green/blue/purple card use/upgrade methods.
use sts_core::{
    action::CombatAction,
    adapter_internals::{CardId, CardInstance, ContentId, MonsterId, Relic},
    combat::{apply_combat_action, legal_combat_actions, start_player_turn, CombatOrb},
    content::{
        cards::{BURN_ID, DEFEND_R_ID, SHIV_ANY_COLOR_ID, STRIKE_R_ID},
        prismatic::*,
    },
    CombatState, SimError,
};

fn fixture(id: ContentId, upgraded: bool) -> CombatState {
    let mut state = CombatState::initial_fixture();
    let mut card = CardInstance::new(CardId::new(10), id);
    card.upgrades = u8::from(upgraded);
    state.piles.hand = vec![card];
    state.player.energy = 10;
    state.monsters[0].hp = 1000;
    state.monsters[0].max_hp = 1000;
    state
}

fn play(state: &CombatState, target: bool) -> CombatState {
    apply_combat_action(
        state,
        CombatAction::PlayCard {
            card_id: CardId::new(10),
            target: target.then_some(MonsterId::new(1)),
        },
    )
    .expect("source-backed card use")
}

#[test]
fn base_and_upgraded_backstab_deal_their_real_damage_and_exhaust() {
    for (upgraded, damage) in [(false, 11), (true, 15)] {
        let next = play(&fixture(BACKSTAB_ANY_COLOR_ID, upgraded), true);
        assert_eq!(next.monsters[0].hp, 1000 - damage);
        assert!(next.piles.hand.is_empty());
        assert!(next.piles.limbo.is_empty());
        assert_eq!(next.piles.exhaust_pile[0].content_id, BACKSTAB_ANY_COLOR_ID);
        assert_eq!(next.player.energy, 10);
    }
}

#[test]
fn source_leaves_hand_before_blade_dance_adds_shivs() {
    for (upgraded, count) in [(false, 3), (true, 4)] {
        let mut state = fixture(BLADE_DANCE_ANY_COLOR_ID, upgraded);
        state
            .piles
            .hand
            .extend((20..29).map(|id| CardInstance::new(CardId::new(id), DEFEND_R_ID)));
        let next = play(&state, false);
        assert_eq!(next.piles.hand.len(), 10);
        assert_eq!(
            next.piles
                .hand
                .iter()
                .filter(|c| c.content_id == SHIV_ANY_COLOR_ID)
                .count(),
            1
        );
        assert_eq!(
            next.piles
                .discard_pile
                .iter()
                .filter(|c| c.content_id == SHIV_ANY_COLOR_ID)
                .count(),
            count - 1
        );
        assert!(next.piles.limbo.is_empty());
        next.validate_unique_card_piles().unwrap();
    }
}

#[test]
fn copied_blade_dance_does_not_restage_the_physical_source() {
    let mut state = fixture(BLADE_DANCE_ANY_COLOR_ID, false);
    state.duplication_potion_pending = true;
    let next = play(&state, false);
    assert_eq!(
        next.piles
            .hand
            .iter()
            .filter(|c| c.content_id == SHIV_ANY_COLOR_ID)
            .count(),
        6
    );
    assert_eq!(
        next.piles
            .discard_pile
            .iter()
            .filter(|c| c.id == CardId::new(10))
            .count(),
        1
    );
    assert!(next.piles.limbo.is_empty());
    next.validate_unique_card_piles().unwrap();
}

#[test]
fn turbo_adds_real_void_and_adrenaline_draws_without_counting_source_in_hand() {
    let next = play(&fixture(TURBO_ANY_COLOR_ID, true), false);
    assert_eq!(next.player.energy, 13);
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        sts_core::content::cards::VOID_ID
    );
    let mut state = fixture(ADRENALINE_ANY_COLOR_ID, true);
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(20), DEFEND_R_ID),
        CardInstance::new(CardId::new(21), STRIKE_R_ID),
    ];
    let next = play(&state, false);
    assert_eq!(next.player.energy, 12);
    assert_eq!(next.piles.hand.len(), 2);
    assert_eq!(next.piles.exhaust_pile[0].id, CardId::new(10));
}

#[test]
fn overclock_draws_then_puts_a_burn_in_discard() {
    let mut state = fixture(OVERCLOCK_ANY_COLOR_ID, false);
    state.piles.draw_pile = (20..23)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    let next = play(&state, false);
    assert_eq!(next.piles.hand.len(), 2);
    assert_eq!(next.piles.draw_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].content_id, BURN_ID);
}

#[test]
fn glacier_gains_real_block_and_channels_two_frost_orbs() {
    for (upgraded, block) in [(false, 7), (true, 10)] {
        let mut state = fixture(GLACIER_ANY_COLOR_ID, upgraded);
        state.max_orbs = 2;
        let next = play(&state, false);
        assert_eq!(next.player.block, block);
        assert_eq!(next.orbs, vec![CombatOrb::Frost, CombatOrb::Frost]);
        assert_eq!(next.foreign.frost_channeled_this_combat, 2);
    }
}

#[test]
fn static_discharge_does_not_drop_plasma_evoke_energy_with_ice_cream() {
    let mut state = fixture(FUSION_ANY_COLOR_ID, false);
    state.player.authority.relics.push(Relic::IceCream);
    state.max_orbs = 1;
    state.player.powers.static_discharge = 1;
    let channeled = play(&state, false);
    assert_eq!(channeled.player.energy, 8);
    let next = apply_combat_action(&channeled, CombatAction::EndTurn).unwrap();
    assert_eq!(next.orbs, vec![CombatOrb::Lightning]);
    assert_eq!(next.player.energy, 13); // 8 carried + 2 Plasma evoke + 3 recharge
}

#[test]
fn fusion_plasma_ignores_focus_and_gives_energy_only_on_evoke_and_turn_start() {
    let mut state = fixture(FUSION_ANY_COLOR_ID, true);
    state.max_orbs = 1;
    state.player.powers.focus = -100;
    let mut next = play(&state, false);
    assert_eq!(next.player.energy, 9);
    assert_eq!(next.orbs, vec![CombatOrb::Plasma]);
    // New turn recharges 3 then queues Plasma's +1. No end-turn pulse exists.
    start_player_turn(&mut next).unwrap();
    assert_eq!(next.player.energy, 4);
    next.piles
        .hand
        .push(CardInstance::new(CardId::new(30), COLD_SNAP_ANY_COLOR_ID));
    let after = apply_combat_action(
        &next,
        CombatAction::PlayCard {
            card_id: CardId::new(30),
            target: Some(MonsterId::new(1)),
        },
    )
    .unwrap();
    assert_eq!(after.player.energy, 5); // spend 1, Plasma evokes for 2
    assert_eq!(after.orbs, vec![CombatOrb::Frost]);
}

#[test]
fn meteor_strike_with_one_slot_evokes_two_plasma_before_settlement() {
    let mut state = fixture(METEOR_STRIKE_ANY_COLOR_ID, false);
    state.max_orbs = 1;
    let next = play(&state, true);
    assert_eq!(next.monsters[0].hp, 976);
    assert_eq!(next.player.energy, 9); // -5, then two evokes of +2
    assert_eq!(next.orbs, vec![CombatOrb::Plasma]);
}

#[test]
fn lethal_cold_snap_does_not_channel_after_the_damage_clear() {
    let mut state = fixture(COLD_SNAP_ANY_COLOR_ID, false);
    state.max_orbs = 1;
    state.monsters[0].hp = 1;
    let next = play(&state, true);
    assert!(next.orbs.is_empty());
    assert!(next.piles.limbo.is_empty());
    assert_eq!(
        next.piles.discard_pile[0].content_id,
        COLD_SNAP_ANY_COLOR_ID
    );
}

#[test]
fn card_created_from_reality_cards_is_not_upgraded_with_the_source() {
    let next = play(&fixture(CARVE_REALITY_ANY_COLOR_ID, true), true);
    assert_eq!(next.piles.hand[0].content_id, SMITE_ID);
    assert_eq!(next.piles.hand[0].upgrades, 0);
    let next = play(&fixture(DECEIVE_REALITY_ANY_COLOR_ID, true), false);
    assert_eq!(next.piles.hand[0].content_id, SAFETY_ID);
    assert_eq!(next.piles.hand[0].upgrades, 0);
}

#[test]
fn mantra_threshold_changes_stance_and_preserves_calm_exit_energy() {
    let mut state = fixture(WORSHIP_ANY_COLOR_ID, false);
    state.player.powers.mantra = 5;
    state.player.powers.calm = 1;
    let next = play(&state, false);
    assert_eq!(next.player.powers.mantra, 0);
    assert_eq!(next.player.powers.calm, 0);
    assert_eq!(next.player.powers.divinity, 1);
    assert_eq!(next.player.energy, 13); // -2, +2 Calm exit, +3 Divinity entry
    assert_eq!(next.foreign.mantra_gained_this_combat, 5);
}

#[test]
fn signature_move_and_grand_finale_have_real_public_play_conditions() {
    let mut state = fixture(SIGNATURE_MOVE_ANY_COLOR_ID, false);
    assert!(legal_combat_actions(&state)
        .unwrap()
        .iter()
        .any(|a| matches!(a, CombatAction::PlayCard { .. })));
    state
        .piles
        .hand
        .push(CardInstance::new(CardId::new(30), STRIKE_R_ID));
    assert!(!legal_combat_actions(&state)
        .unwrap()
        .iter()
        .any(|a| matches!(a, CombatAction::PlayCard {card_id, ..} if *card_id == CardId::new(10))));
    assert!(apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(10),
            target: Some(MonsterId::new(1))
        }
    )
    .is_err());
    let mut state = fixture(GRAND_FINALE_ANY_COLOR_ID, false);
    assert!(!legal_combat_actions(&state)
        .unwrap()
        .iter()
        .any(|a| matches!(a, CombatAction::PlayCard { .. })));
    state.piles.draw_pile.clear();
    assert_eq!(play(&state, false).monsters[0].hp, 950);
}

#[test]
fn incomplete_card_is_not_replaced_filtered_or_a_fake_success() {
    let state = fixture(ECHO_FORM_ANY_COLOR_ID, false);
    let action = CombatAction::PlayCard {
        card_id: CardId::new(10),
        target: None,
    };
    assert!(legal_combat_actions(&state).unwrap().contains(&action));
    let before = state.clone();
    assert_eq!(
        apply_combat_action(&state, action),
        Err(SimError::UnsupportedMechanic(ECHO_FORM_ANY_COLOR_ID))
    );
    assert_eq!(state, before);
}

#[test]
fn auto_shields_copy_rechecks_live_block_instead_of_replaying_the_gain() {
    let mut state = fixture(AUTO_SHIELDS_ANY_COLOR_ID, false);
    state.duplication_potion_pending = true;
    let next = play(&state, false);
    assert_eq!(next.player.block, 11);
    assert!(next.piles.limbo.is_empty());
}

#[test]
fn stack_copy_recomputes_after_the_original_enters_discard() {
    let mut state = fixture(STACK_ANY_COLOR_ID, false);
    state.duplication_potion_pending = true;
    let next = play(&state, false);
    assert_eq!(next.player.block, 1); // original 0; copied use sees original in discard
}

#[test]
fn catalyst_still_applies_poison_through_artifact() {
    for (upgraded, amount) in [(false, 10), (true, 15)] {
        let mut state = fixture(CATALYST_ANY_COLOR_ID, upgraded);
        state.monsters[0].powers.poison = 5;
        assert_eq!(play(&state, true).monsters[0].powers.poison, amount);
        state.monsters[0].powers.artifact = 1;
        let blocked = play(&state, true);
        assert_eq!(blocked.monsters[0].powers.poison, 5);
        assert_eq!(blocked.monsters[0].powers.artifact, 0);
    }
}

#[test]
fn claw_copies_keep_copy_stats_and_grow_only_live_claws_not_exhausted_cards() {
    let mut state = fixture(CLAW_ANY_COLOR_ID, false);
    state.duplication_potion_pending = true;
    state
        .piles
        .hand
        .push(CardInstance::new(CardId::new(20), CLAW_ANY_COLOR_ID));
    state
        .piles
        .exhaust_pile
        .push(CardInstance::new(CardId::new(30), CLAW_ANY_COLOR_ID));
    let next = play(&state, true);
    assert_eq!(next.monsters[0].hp, 994);
    assert_eq!(next.piles.hand[0].base_damage_delta, 4);
    assert_eq!(next.piles.discard_pile[0].base_damage_delta, 4);
    assert_eq!(next.piles.exhaust_pile[0].base_damage_delta, 0);
}

#[test]
fn miracle_and_meteor_strike_use_fixed_energy_and_strike_tag_not_magic_defaults() {
    for (upgraded, energy) in [(false, 11), (true, 12)] {
        assert_eq!(
            play(&fixture(MIRACLE_ID, upgraded), false).player.energy,
            energy
        );
    }
    let mut state = fixture(METEOR_STRIKE_ANY_COLOR_ID, false);
    state.player.authority.relics.push(Relic::StrikeDummy);
    assert_eq!(play(&state, true).monsters[0].hp, 973);
}

#[test]
fn expertise_draws_to_the_real_hand_size_including_original_copy_settlement() {
    let mut state = fixture(EXPERTISE_ANY_COLOR_ID, false);
    state.piles.draw_pile = (20..29)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    assert_eq!(play(&state, false).piles.hand.len(), 6);
}

#[test]
fn reprogram_focus_debuff_uses_artifact_without_blocking_later_buffs() {
    let mut state = fixture(REPROGRAM_ANY_COLOR_ID, true);
    state.player.powers.artifact = 1;
    let next = play(&state, false);
    assert_eq!(next.player.powers.focus, 0);
    assert_eq!(next.player.powers.artifact, 0);
    assert_eq!(next.player.powers.strength, 2);
    assert_eq!(next.player.powers.dexterity, 2);
}

#[test]
fn force_field_cost_changes_follow_real_power_plays() {
    let mut state = fixture(FOOTWORK_ANY_COLOR_ID, false);
    state
        .piles
        .hand
        .push(CardInstance::new(CardId::new(20), FORCE_FIELD_ANY_COLOR_ID));
    let next = play(&state, false);
    assert_eq!(next.foreign.powers_played_this_combat, 1);
    assert_eq!(next.piles.hand[0].temp_cost, Some(3));
    assert!(
        next.piles.discard_pile.is_empty(),
        "played Footwork power leaves combat"
    );
}

#[test]
fn duplication_and_source_clones_produce_identical_real_orbs_and_draws() {
    let mut state = fixture(RAINBOW_ANY_COLOR_ID, true);
    state.max_orbs = 3;
    state.duplication_potion_pending = true;
    state.player.authority.relics = vec![Relic::PrismaticShard];
    assert_eq!(play(&state, false), play(&state.clone(), false));
}
