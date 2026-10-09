//! Source-backed synthetic testing; not dedicated real-game boundary trace parity.
//! Brimstone.atTurnStart applies StrengthPower(+2 player, +1 monsters),
//! whose stacking caps the current amount at +/-999.
use sts_core::adapter_internals::{
    apply_combat_action, CardId, CardInstance, CombatAction, CombatState, Relic, SimError,
};
use sts_core::content::cards::STRIKE_R_ID;

fn setup(player_strength: i32, monster_strength: i32) -> CombatState {
    let mut c = CombatState::initial_fixture();
    c.player.authority.relics = vec![Relic::Brimstone];
    c.player.hp = 10000;
    c.player.max_hp = 10000;
    c.player.powers.strength = player_strength;
    c.monsters[0].powers.strength = monster_strength;
    c.monsters[0].hp = 5000;
    c.monsters[0].max_hp = 5000;
    c.piles.draw_pile.clear();
    c.piles.discard_pile.clear();
    c.piles.exhaust_pile.clear();
    c.piles.hand = (1..=5)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    c.validate().unwrap();
    c
}

fn step(c: &CombatState, action: CombatAction) -> CombatState {
    let before = serde_json::to_value(c).unwrap();
    let restored: CombatState = serde_json::from_value(before.clone()).unwrap();
    let next = apply_combat_action(c, action).unwrap();
    let repeat = apply_combat_action(&restored, action).unwrap();
    assert_eq!(serde_json::to_value(c).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(repeat).unwrap()
    );
    next.validate().unwrap();
    println!(
        "brimstone_transition={}",
        serde_json::json!({"initial": before,"action": action,"result": next})
    );
    next
}

#[test]
fn brimstone_caps_player_strength_on_next_turn() {
    for strength in [998, 999] {
        let next = step(&setup(strength, 0), CombatAction::EndTurn);
        assert_eq!(next.player.powers.strength, 999);
        assert_eq!(next.monsters[0].powers.strength, 1);
    }
}

#[test]
fn brimstone_caps_monster_strength_on_next_turn() {
    let next = step(&setup(0, 999), CombatAction::EndTurn);
    assert_eq!(next.player.powers.strength, 2);
    assert_eq!(next.monsters[0].powers.strength, 999);
}

#[test]
fn repeated_turns_keep_both_powers_bounded() {
    let first = step(&setup(998, 999), CombatAction::EndTurn);
    let second = step(&first, CombatAction::EndTurn);
    assert_eq!(second.player.powers.strength, 999);
    assert_eq!(second.monsters[0].powers.strength, 999);
}

#[test]
fn player_bound_controls_following_strike_damage() {
    let next = step(&setup(998, 0), CombatAction::EndTurn);
    let attacked = step(
        &next,
        CombatAction::PlayCard {
            card_id: next.piles.hand[0].id,
            target: Some(next.monsters[0].id),
        },
    );
    assert_eq!(next.monsters[0].hp - attacked.monsters[0].hp, 1005);
}

#[test]
fn monster_bound_controls_following_attack_damage() {
    let first = step(&setup(0, 999), CombatAction::EndTurn);
    let second = step(&first, CombatAction::EndTurn);
    assert_eq!(first.player.hp - second.player.hp, 1005);
}

#[test]
fn ordinary_negative_and_artifact_controls_are_unchanged() {
    for (player, monster) in [(10, 3), (-20, -8)] {
        let mut c = setup(player, monster);
        c.player.powers.artifact = 2;
        c.monsters[0].powers.artifact = 1;
        let next = step(&c, CombatAction::EndTurn);
        assert_eq!(next.player.powers.strength, player + 2);
        assert_eq!(next.monsters[0].powers.strength, monster + 1);
        assert_eq!(next.player.powers.artifact, 2);
        assert_eq!(next.monsters[0].powers.artifact, 1);
    }
}

// Rule-level diagnostic: unlike the legal EndTurn prefixes above, this calls
// the turn-start hook directly to isolate split Strength bookkeeping.
#[test]
fn turn_start_hook_bounds_combined_strength_without_erasing_nominal_loss() {
    for (base, loss) in [(975, 24), (-1, 1000)] {
        let mut c = setup(base, 999);
        c.player.temp_strength = loss;
        let rng = c.rng.clone();
        sts_core::relic::apply_start_of_player_turn_relics(&mut c).unwrap();
        assert_eq!(c.player.powers.strength + c.player.temp_strength, 999);
        assert_eq!(c.player.powers.strength, base);
        assert_eq!(c.player.temp_strength, loss);
        assert_eq!(c.monsters[0].powers.strength, 999);
        assert_eq!(c.rng, rng);
        c.validate().unwrap();
    }
}

#[test]
fn malformed_player_overflow_rejects_with_existing_error_and_no_input_mutation() {
    let c = setup(i32::MAX, 0);
    let before = serde_json::to_value(&c).unwrap();
    assert_eq!(
        apply_combat_action(&c, CombatAction::EndTurn),
        Err(SimError::InvalidState(
            "combat integer addition overflows i32"
        ))
    );
    assert_eq!(serde_json::to_value(c).unwrap(), before);
}
