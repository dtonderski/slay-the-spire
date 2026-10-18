//! Source-backed queue controls. FIDL00674/300->313 supplies natural evidence.
use super::*;
use crate::{
    content::cards::{DEFEND_R_ID, STRIKE_R_ID},
    CardId, CardInstance, Relic,
};
fn setup(draw: usize) -> CombatState {
    let mut s = CombatState::cultist_fixture();
    s.player.authority.relics = vec![
        Relic::MercuryHourglass,
        Relic::Sundial,
        Relic::TheAbacus,
        Relic::WarpedTongs,
        Relic::Pocketwatch,
    ];
    s.player.hp = 80;
    s.player.max_hp = 80;
    s.player.energy = 0;
    s.player.block = 0;
    s.monsters[0].hp = 3;
    s.monsters[0].max_hp = 500;
    s.monsters[0].block = 0;
    s.piles.hand.clear();
    s.piles.exhaust_pile.clear();
    s.piles.draw_pile = (1..=draw)
        .map(|i| CardInstance::new(CardId::new(i as u64), STRIKE_R_ID))
        .collect();
    s.piles.discard_pile = (100..114)
        .map(|i| CardInstance::new(CardId::new(i), DEFEND_R_ID))
        .collect();
    s.sundial_shuffles = 2;
    s.relic_counters.player_turns_started = 1;
    s
}
#[test]
fn final_mercury_kill_cancels_already_queued_draw_shuffle_and_post_draw_upgrade() {
    for draw in [0, 2, 5] {
        let mut s = setup(draw);
        let before = s.clone();
        start_player_turn(&mut s).unwrap();
        assert_eq!(s.phase, CombatPhase::Won);
        assert!(!s.monsters[0].alive);
        assert_eq!(s.sundial_shuffles, 2);
        assert_eq!(s.rng, before.rng);
        assert_eq!(
            s.piles, before.piles,
            "cancel before moving/upgrading any cards"
        );
        assert_eq!(s.player.block, 0, "Abacus constructor was never reached");
        assert_eq!(s.player.energy, 3);
    }
}
#[test]
fn nonlethal_mercury_still_allows_one_real_shuffle_sundial_and_abacus() {
    let mut s = setup(2);
    s.monsters[0].hp = 4;
    s.player.authority.relics = vec![Relic::MercuryHourglass, Relic::Sundial, Relic::TheAbacus];
    let before = s.clone();
    start_player_turn(&mut s).unwrap();
    assert_eq!(s.monsters[0].hp, 1);
    assert_eq!(s.piles.hand.len(), 5);
    assert_eq!(s.sundial_shuffles, 3);
    assert_eq!(s.player.energy, 5);
    assert_eq!(s.player.block, 6);
    assert_eq!(
        s.rng.shuffle_rng.counter(),
        before.rng.shuffle_rng.counter() + 1
    );
}
#[test]
fn cancelled_generators_and_buffs_do_not_draw_rng_but_brutality_damage_survives() {
    let mut s = setup(2);
    s.player.powers.brutality = 2;
    s.player.powers.draw_card = 4;
    s.player.powers.demon_form = 5;
    s.player.powers.magnetism = 2;
    s.player.powers.creative_ai = 2;
    s.player.powers.mayhem = 1;
    let before = s.clone();
    start_player_turn(&mut s).unwrap();
    assert_eq!(
        s.player.hp, 78,
        "queued LoseHPAction is DAMAGE; no generic compensation"
    );
    assert_eq!(s.player.powers.strength, 0);
    assert_eq!(
        s.player.powers.draw_card, 4,
        "RemoveSpecificPowerAction was cancelled too"
    );
    assert_eq!(s.piles, before.piles);
    assert_eq!(s.rng, before.rng);
    assert_eq!(s.sundial_shuffles, 2);
}
#[test]
fn killing_one_of_two_monsters_preserves_deferred_horn_after_the_base_draw() {
    let mut s = setup(2);
    let mut other = s.monsters[0].clone();
    other.id = MonsterId::new(2);
    other.hp = 100;
    s.monsters.push(other);
    s.player.authority.relics = vec![Relic::MercuryHourglass, Relic::GremlinHorn];
    start_player_turn(&mut s).unwrap();
    assert!(!s.monsters[0].alive);
    assert_eq!(s.monsters[1].hp, 97);
    assert_eq!(s.phase, CombatPhase::WaitingForPlayer);
    assert_eq!(s.piles.hand.len(), 6);
    assert_eq!(s.player.energy, 4);
}
#[test]
fn already_won_entry_is_not_a_blanket_ban_on_later_created_draw_work() {
    let mut s = setup(2);
    s.monsters[0].alive = false;
    s.monsters[0].hp = 0;
    s.phase = CombatPhase::Won;
    s.player.authority.relics = vec![Relic::MercuryHourglass, Relic::Sundial];
    let rng = s.rng.clone();
    start_player_turn(&mut s).unwrap();
    assert_eq!(s.piles.hand.len(), 2);
    assert_eq!(s.sundial_shuffles, 2);
    assert_eq!(s.rng, rng);
}
#[test]
fn awakened_one_first_death_does_not_cancel_the_opening_draw() {
    let mut s = setup(2);
    s.monsters = vec![crate::content::monsters::monster_state(
        &crate::content::monsters::AWAKENED_ONE_A0,
        MonsterId::new(1),
    )];
    s.monsters[0].hp = 3;
    s.player.authority.relics = vec![Relic::MercuryHourglass, Relic::Sundial];
    start_player_turn(&mut s).unwrap();
    assert!(awakened_one_is_half_dead(&s.monsters[0]));
    assert_eq!(s.phase, CombatPhase::WaitingForPlayer);
    assert_eq!(s.piles.hand.len(), 5);
    assert_eq!(s.sundial_shuffles, 3);
    assert_eq!(s.player.energy, 5);
}

#[test]
fn snapshot_round_trip_preserves_causal_cancellation_and_rng() {
    let mut a = setup(2);
    let mut b: CombatState = serde_json::from_value(serde_json::to_value(&a).unwrap()).unwrap();
    start_player_turn(&mut a).unwrap();
    start_player_turn(&mut b).unwrap();
    assert_eq!(a, b);
}
