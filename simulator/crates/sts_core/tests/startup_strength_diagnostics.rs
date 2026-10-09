//! Explicit direct-initializer diagnostics, NOT legal gameplay prefixes.
use sts_core::adapter_internals::{CombatState, Relic, RunState};
#[test]
fn initializer_bounds_philosopher_stone_monster_strength() {
    let mut r = RunState::map_fixture();
    r.relics = vec![Relic::PhilosophersStone];
    let mut base = CombatState::initial_fixture();
    base.monsters[0].powers.strength = 999;
    base.monsters[0].powers.artifact = 1;
    let before = serde_json::to_value(&r).unwrap();
    let c = r.init_combat(base.clone()).unwrap();
    c.validate().unwrap();
    assert_eq!(c.monsters[0].powers.strength, 999);
    assert_eq!(c.monsters[0].powers.artifact, 1);
    assert_eq!(c.rng, base.rng);
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
    let restored: RunState = serde_json::from_value(before).unwrap();
    assert_eq!(
        serde_json::to_value(c).unwrap(),
        serde_json::to_value(restored.init_combat(base).unwrap()).unwrap()
    );
}
#[test]
fn initializer_vajra_caps_combined_strength_without_erasing_loss() {
    for (strength, loss) in [(998, 1), (-1001, 2000)] {
        let mut r = RunState::map_fixture();
        r.relics = vec![Relic::Vajra];
        let mut base = CombatState::initial_fixture();
        base.player.powers.strength = strength;
        base.player.temp_strength = loss;
        base.player.powers.artifact = 2;
        let c = r.init_combat(base.clone()).unwrap();
        c.validate().unwrap();
        assert_eq!(c.player.powers.strength + loss, 999);
        assert_eq!(c.player.temp_strength, loss);
        assert_eq!(c.player.powers.artifact, 2);
        assert_eq!(c.rng, base.rng);
    }
}
#[test]
fn initializer_malformed_overflow_keeps_existing_error_and_run_input() {
    let mut r = RunState::map_fixture();
    r.relics = vec![Relic::Vajra];
    let mut base = CombatState::initial_fixture();
    base.player.powers.strength = i32::MAX;
    let before = serde_json::to_value(&r).unwrap();
    assert!(matches!(
        r.init_combat(base),
        Err(sts_core::SimError::InvalidState(
            "combat integer addition overflows i32"
        ))
    ));
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}
