use sts_core::{
    apply_combat_action,
    content::cards::{ASCENDERS_BANE_ID, STRIKE_R_ID},
    content::deck::ironclad_starter_deck_for_ascension,
    content::monsters::{
        boss_monsters_for_ascension, monster_state_for_ascension, CULTIST_A0, SLIME_BOSS_A0,
    },
    end_player_turn, generate_map_placeholder, CombatAction, CombatState, MonsterId, RoomKind,
    RunState,
};

#[test]
fn a2_increases_cultist_attack_damage() {
    let mut combat = CombatState::cultist_fixture();
    combat.ascension = 2;
    combat.monsters[0].moves_executed = 1;
    combat.monsters[0].intent = sts_core::MonsterIntent::Attack { damage: 6 };
    combat.player.hp = 80;
    combat.piles.draw_pile.clear();

    let next = end_player_turn(&combat);

    assert_eq!(next.player.hp, 80 - 8);
}

#[test]
fn a7_increases_cultist_starting_hp() {
    let monster = monster_state_for_ascension(&CULTIST_A0, MonsterId::new(1), 7);

    assert_eq!(monster.hp, 57);
}

#[test]
fn a10_starter_deck_includes_ascenders_bane() {
    let run = RunState::combat_fixture_with_ascension(10);

    assert_eq!(
        run.deck
            .iter()
            .filter(|card| card.content_id == ASCENDERS_BANE_ID)
            .count(),
        1
    );
}

#[test]
fn a17_adds_deadly_damage_on_top_of_a2() {
    let mut combat = CombatState::cultist_fixture();
    combat.ascension = 17;
    combat.monsters[0].moves_executed = 1;
    combat.monsters[0].intent = sts_core::MonsterIntent::Attack { damage: 6 };
    combat.player.hp = 80;
    combat.piles.draw_pile.clear();

    let next = end_player_turn(&combat);

    assert_eq!(next.player.hp, 80 - 9);
}

#[test]
fn a20_boss_encounter_spawns_two_monsters() {
    let monsters = boss_monsters_for_ascension(&SLIME_BOSS_A0, 20);

    assert_eq!(monsters.len(), 2);
}

#[test]
fn a1_generated_maps_can_contain_elite_nodes() {
    let has_elite = (0..32).any(|seed| {
        generate_map_placeholder(seed, 1)
            .0
            .nodes
            .iter()
            .any(|node| node.room_kind == RoomKind::Elite)
    });

    assert!(has_elite);
}

#[test]
fn ascension_round_trips_through_run_state_json() {
    let run = RunState::combat_fixture_with_ascension(12);

    let json = serde_json::to_string(&run).expect("run serializes");
    let restored: RunState = serde_json::from_str(&json).expect("run deserializes");

    assert_eq!(restored.ascension, 12);
    assert_eq!(
        restored.deck.len(),
        ironclad_starter_deck_for_ascension(12).len()
    );
}

#[test]
fn a0_strike_damage_unchanged_in_combat() {
    let run = RunState::combat_fixture_with_ascension(0);
    let combat = run.combat.expect("combat");
    let strike_id = combat
        .piles
        .hand
        .iter()
        .find(|card| card.content_id == STRIKE_R_ID)
        .expect("strike")
        .id;
    let monster_id = combat.monsters[0].id;
    let hp_before = combat.monsters[0].hp;

    let next = apply_combat_action(
        &combat,
        CombatAction::PlayCard {
            card_id: strike_id,
            target: Some(monster_id),
        },
    )
    .expect("strike");

    assert_eq!(next.monsters[0].hp, hp_before - 6);
}
