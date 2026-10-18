//! PC EnergyManager.recharge -> EnergyPanel.addEnergy versus setEnergy.
//! Natural boundary: QLFO00012 step941,997 retained +4 recharge becomes999.
use sts_core::adapter_internals::{combat::start_player_turn, CombatState, Relic};
fn fixture(ice_cream: bool, energy: i32, max_energy: i32) -> CombatState {
    let mut s = CombatState::cultist_fixture();
    s.piles.draw_pile.clear();
    s.piles.discard_pile.clear();
    s.player.energy = energy;
    s.player.max_energy = max_energy;
    if ice_cream {
        s.player.authority.relics.push(Relic::IceCream);
    }
    s
}
#[test]
fn ice_cream_recharge_adds_then_caps_while_ordinary_recharge_sets() {
    for (energy, base, expected) in [(0, 3, 3), (995, 4, 999), (997, 4, 999), (999, 3, 999)] {
        let mut s = fixture(true, energy, base);
        start_player_turn(&mut s).unwrap();
        assert_eq!(s.player.energy, expected);
    }
    let mut s = fixture(false, 997, 4);
    start_player_turn(&mut s).unwrap();
    assert_eq!(s.player.energy, 4);
}
#[test]
fn ordinary_set_energy_is_not_given_the_add_energy_cap() {
    let mut s = fixture(false, 0, 1005);
    start_player_turn(&mut s).unwrap();
    assert_eq!(s.player.energy, 1005);
}
#[test]
fn overflow_remains_a_whole_transition_rejection_not_a_repair() {
    let mut s = fixture(true, i32::MAX, 3);
    let before = serde_json::to_value(&s).unwrap();
    assert!(start_player_turn(&mut s).is_err());
    assert_eq!(serde_json::to_value(&s).unwrap(), before);
}
