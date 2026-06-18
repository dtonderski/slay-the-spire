use sts_core::{
    apply_combat_action, apply_combat_action_on_run, apply_rest_action, apply_run_action,
    content::cards::{DEFEND_R_ID, STRIKE_R_ID},
    end_player_turn, legal_rest_actions, CardId, CardInstance, CombatAction, Relic, RestAction,
    RunAction, RunPhase, RunState, ANCHOR_BLOCK, BASE_PLAYER_ENERGY, COFFEE_DRIPPER_ENERGY,
    INK_BOTTLE_THRESHOLD, ODDLY_SMOOTH_STONE_DEXTERITY, STRAWBERRY_MAX_HP, VAJRA_STRENGTH,
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
fn anchor_grants_block_when_combat_starts_from_run() {
    let run = RunState::combat_fixture_with_relics(vec![Relic::Anchor]);
    let combat = run.combat.expect("combat initialized");

    assert_eq!(combat.player.block, ANCHOR_BLOCK);
}

#[test]
fn anchor_block_stacks_with_defend() {
    let run = RunState::combat_fixture_with_relics(vec![Relic::Anchor]);
    let combat = run.combat.expect("combat initialized");
    let defend_id = combat
        .piles
        .hand
        .iter()
        .find(|card| card.content_id == sts_core::content::cards::DEFEND_R_ID)
        .expect("defend in hand")
        .id;

    let next = sts_core::apply_combat_action(
        &combat,
        CombatAction::PlayCard {
            card_id: defend_id,
            target: None,
        },
    )
    .expect("defend applies");

    assert_eq!(next.player.block, ANCHOR_BLOCK + 5);
}

#[test]
fn ink_bottle_counter_tracks_card_plays_in_combat() {
    let run = RunState::combat_fixture_with_relics(vec![Relic::InkBottle]);
    let combat = run.combat.expect("combat initialized");
    let defend_id = combat
        .piles
        .hand
        .iter()
        .find(|card| card.content_id == DEFEND_R_ID)
        .expect("defend in hand")
        .id;

    let next = apply_combat_action(
        &combat,
        CombatAction::PlayCard {
            card_id: defend_id,
            target: None,
        },
    )
    .expect("defend applies");

    assert_eq!(next.relic_counters.ink_bottle_cards_played, 1);
}

#[test]
fn ink_bottle_draws_card_on_tenth_play() {
    let run = RunState::combat_fixture_with_relics(vec![Relic::InkBottle]);
    let mut combat = run.combat.expect("combat initialized");
    combat.player.energy = 30;
    combat.relic_counters.ink_bottle_cards_played = INK_BOTTLE_THRESHOLD - 1;
    for index in 0..6 {
        combat
            .piles
            .hand
            .push(CardInstance::new(CardId::new(100 + index), DEFEND_R_ID));
    }
    let draw_pile_size = combat.piles.draw_pile.len();
    let hand_size_before = combat.piles.hand.len();

    let defend_id = combat
        .piles
        .hand
        .iter()
        .find(|card| card.content_id == DEFEND_R_ID)
        .expect("defend in hand")
        .id;
    let next = apply_combat_action(
        &combat,
        CombatAction::PlayCard {
            card_id: defend_id,
            target: None,
        },
    )
    .expect("defend triggers ink bottle");

    assert_eq!(next.relic_counters.ink_bottle_cards_played, 0);
    assert_eq!(next.piles.hand.len(), hand_size_before - 1 + draw_pile_size);
}

#[test]
fn ink_bottle_counters_round_trip_through_combat_json() {
    let run = RunState::combat_fixture_with_relics(vec![Relic::InkBottle]);
    let mut combat = run.combat.expect("combat initialized");
    combat.relic_counters.ink_bottle_cards_played = 4;

    let json = serde_json::to_string(&combat).expect("combat serializes");
    let restored: sts_core::CombatState = serde_json::from_str(&json).expect("combat deserializes");

    assert_eq!(restored.relics, vec![Relic::InkBottle]);
    assert_eq!(restored.relic_counters.ink_bottle_cards_played, 4);
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

#[test]
fn strawberry_pickup_increases_current_and_max_hp() {
    let mut run = RunState::map_fixture();
    run.player_hp = 40;
    run.player_max_hp = 80;

    run.gain_relic(Relic::Strawberry);

    assert_eq!(run.relics, vec![Relic::Strawberry]);
    assert_eq!(run.player_max_hp, 80 + STRAWBERRY_MAX_HP);
    assert_eq!(run.player_hp, 40 + STRAWBERRY_MAX_HP);
}

#[test]
fn strawberry_hp_bonus_applies_to_next_combat() {
    let mut run = RunState::map_fixture();
    run.player_hp = 40;
    run.player_max_hp = 80;
    run.gain_relic(Relic::Strawberry);

    let combat = run.init_combat(sts_core::CombatState::initial_fixture());

    assert_eq!(combat.player.max_hp, 80 + STRAWBERRY_MAX_HP);
    assert_eq!(combat.player.hp, 40 + STRAWBERRY_MAX_HP);
}

#[test]
fn strawberry_round_trips_through_run_state_json() {
    let mut run = RunState::map_fixture();
    run.gain_relic(Relic::Strawberry);

    let json = serde_json::to_string(&run).expect("run serializes");
    let restored: RunState = serde_json::from_str(&json).expect("run deserializes");

    assert_eq!(restored.relics, vec![Relic::Strawberry]);
    assert_eq!(restored.player_max_hp, 80 + STRAWBERRY_MAX_HP);
}

#[test]
fn coffee_dripper_pickup_increases_energy_per_turn() {
    let mut run = RunState::map_fixture();

    run.gain_relic(Relic::CoffeeDripper);

    assert_eq!(run.relics, vec![Relic::CoffeeDripper]);
    assert_eq!(
        run.energy_per_turn,
        BASE_PLAYER_ENERGY + COFFEE_DRIPPER_ENERGY
    );
}

#[test]
fn coffee_dripper_energy_applies_to_combat_and_next_turn_refill() {
    let mut run = RunState::map_fixture();
    run.gain_relic(Relic::CoffeeDripper);

    let mut combat = run.init_combat(sts_core::CombatState::initial_fixture());
    assert_eq!(combat.player.max_energy, 4);
    assert_eq!(combat.player.energy, 4);

    combat.player.energy = 0;
    let combat = end_player_turn(&combat);

    assert_eq!(combat.player.energy, 4);
}

#[test]
fn coffee_dripper_disables_rest_heal() {
    let mut run = RunState::map_fixture();
    run.phase = RunPhase::Rest;
    run.player_hp = 40;
    run.gain_relic(Relic::CoffeeDripper);

    assert!(!legal_rest_actions(&run).contains(&RestAction::Heal));
    let err = apply_rest_action(&run, RestAction::Heal).expect_err("coffee dripper blocks rest");

    assert_eq!(
        err,
        sts_core::SimError::IllegalAction("heal is not available")
    );
}

#[test]
fn coffee_dripper_energy_round_trips_through_run_state_json() {
    let mut run = RunState::map_fixture();
    run.gain_relic(Relic::CoffeeDripper);

    let json = serde_json::to_string(&run).expect("run serializes");
    let restored: RunState = serde_json::from_str(&json).expect("run deserializes");

    assert_eq!(restored.relics, vec![Relic::CoffeeDripper]);
    assert_eq!(
        restored.energy_per_turn,
        BASE_PLAYER_ENERGY + COFFEE_DRIPPER_ENERGY
    );
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
