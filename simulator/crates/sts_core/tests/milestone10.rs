use sts_core::{
    apply_combat_action_on_run, apply_run_action, content::cards::STRIKE_R_ID, CombatAction, Relic,
    RunAction, RunState, ODDLY_SMOOTH_STONE_DEXTERITY, VAJRA_STRENGTH,
};

#[test]
fn vajra_grants_strength_when_combat_starts_from_run() {
    let run = RunState::combat_fixture_with_relics(vec![Relic::Vajra]);
    let combat = run.combat.expect("combat initialized");

    assert_eq!(combat.player.powers.strength, VAJRA_STRENGTH);
}

#[test]
fn combat_fixture_without_relics_has_zero_strength() {
    let run = RunState::combat_fixture();
    let combat = run.combat.expect("combat initialized");

    assert_eq!(combat.player.powers.strength, 0);
}

#[test]
fn vajra_strength_boosts_strike_damage_in_combat() {
    let mut run = RunState::combat_fixture_with_relics(vec![Relic::Vajra]);
    let combat = run.combat.as_mut().expect("combat initialized");
    combat.monsters[0].hp = 50;

    let strike_id = combat
        .piles
        .hand
        .iter()
        .find(|card| card.content_id == STRIKE_R_ID)
        .expect("strike in hand")
        .id;
    let monster_id = combat.monsters[0].id;

    let next = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: strike_id,
            target: Some(monster_id),
        },
    )
    .expect("strike applies");

    let combat = next.combat.expect("combat continues");
    assert_eq!(combat.monsters[0].hp, 50 - (6 + VAJRA_STRENGTH));
}

#[test]
fn oddly_smooth_stone_grants_dexterity_when_combat_starts_from_run() {
    let run = RunState::combat_fixture_with_relics(vec![Relic::OddlySmoothStone]);
    let combat = run.combat.expect("combat initialized");

    assert_eq!(combat.player.powers.dexterity, ODDLY_SMOOTH_STONE_DEXTERITY);
}

#[test]
fn relic_reward_applies_on_next_combat_start() {
    let run = win_fixture_combat();
    let run = apply_run_action(&run, RunAction::TakeRelicReward).expect("take relic");
    let mut run = run;
    let combat = run.init_combat(sts_core::CombatState::initial_fixture());
    run.combat = Some(combat);
    let combat = run.combat.expect("combat initialized");

    assert_eq!(combat.player.powers.dexterity, ODDLY_SMOOTH_STONE_DEXTERITY);
}

fn win_fixture_combat() -> RunState {
    let mut run = RunState::combat_fixture();
    let combat = run.combat.as_mut().expect("combat fixture");
    combat.monsters[0].hp = 1;

    let strike_id = combat
        .piles
        .hand
        .iter()
        .find(|card| card.content_id == STRIKE_R_ID)
        .expect("strike in hand")
        .id;
    let monster_id = combat.monsters[0].id;

    apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: strike_id,
            target: Some(monster_id),
        },
    )
    .expect("strike wins combat")
}

#[test]
fn run_state_relics_round_trip_through_json() {
    let run = RunState::combat_fixture_with_relics(vec![Relic::Vajra]);

    let json = serde_json::to_string(&run).expect("run serializes");
    let restored: RunState = serde_json::from_str(&json).expect("run deserializes");

    assert_eq!(restored.relics, vec![Relic::Vajra]);
    assert_eq!(
        restored
            .combat
            .expect("combat restored")
            .player
            .powers
            .strength,
        VAJRA_STRENGTH
    );
}
