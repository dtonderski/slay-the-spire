use std::collections::BTreeSet;

use sts_core::{
    content::{
        cards::ALL_CARDS,
        encounters::{EXORDIUM_STRONG_ENCOUNTERS, EXORDIUM_WEAK_ENCOUNTERS},
        monsters::{
            ACID_SLIME_A0, CULTIST_A0, FIXED_SIMPLE_MONSTER, GREEN_LOUSE_A0, GREMLIN_NOB_A0,
            GUARDIAN_A0, HEXAGHOST_A0, JAW_WORM_A0, LAGAVULIN_A0, RED_LOUSE_A0, SENTRY_A0,
            SLIME_BOSS_A0, SPIKE_SLIME_A0,
        },
        reward_pool::IRONCLAD_REWARD_ENTRIES,
        shop_pool::{
            colorless_discovery_pool, ironclad_combat_attack_discovery_pool,
            ironclad_combat_power_discovery_pool, ironclad_combat_skill_discovery_pool,
        },
    },
    potion::IRONCLAD_POTION_POOL,
    relic::{
        RelicKey, IRONCLAD_BOSS_RELIC_POOL, IRONCLAD_COMMON_RELIC_POOL, IRONCLAD_RARE_RELIC_POOL,
        IRONCLAD_SHOP_RELIC_POOL, IRONCLAD_UNCOMMON_RELIC_POOL,
    },
    Event, RoomKind,
};

const CARD_MATRIX: &str = include_str!("../../../docs/m32a_cards_matrix.md");
const RELIC_POTION_MATRIX: &str = include_str!("../../../docs/m32a_relic_potion_matrix.md");
const RUN_WORLD_MATRIX: &str = include_str!("../../../docs/m32a_run_world_matrix.md");

fn assert_matrix_contains(matrix: &str, token: impl AsRef<str>) {
    let token = token.as_ref();
    assert!(
        matrix.contains(token),
        "M32A matrix is missing expected token `{token}`"
    );
}

fn content_id_token(id: sts_core::ContentId) -> String {
    format!("ContentId({})", id.get())
}

#[test]
fn card_matrix_covers_known_card_content_ids() {
    let mut ids = BTreeSet::new();
    for definition in ALL_CARDS {
        ids.insert(definition.id);
        assert_matrix_contains(CARD_MATRIX, definition.key);
    }
    for entry in IRONCLAD_REWARD_ENTRIES {
        ids.insert(entry.content_id);
    }
    for id in ironclad_combat_attack_discovery_pool()
        .into_iter()
        .chain(ironclad_combat_skill_discovery_pool())
        .chain(ironclad_combat_power_discovery_pool().iter().copied())
        .chain(colorless_discovery_pool())
    {
        ids.insert(id);
    }

    for id in ids {
        assert_matrix_contains(CARD_MATRIX, content_id_token(id));
    }
}

#[test]
fn relic_potion_matrix_covers_known_relic_and_potion_surfaces() {
    let relics = [
        &[RelicKey::BurningBlood][..],
        &[RelicKey::CrackedCore],
        &[RelicKey::RingOfTheSnake],
        &[RelicKey::PureWater],
        &IRONCLAD_COMMON_RELIC_POOL,
        &IRONCLAD_UNCOMMON_RELIC_POOL,
        &IRONCLAD_RARE_RELIC_POOL,
        &IRONCLAD_SHOP_RELIC_POOL,
        &IRONCLAD_BOSS_RELIC_POOL,
        &[RelicKey::FrozenCore],
        &[RelicKey::RingOfTheSerpent],
        &[RelicKey::HolyWater],
        &[RelicKey::Circlet],
        &[RelicKey::RedCirclet],
    ]
    .into_iter()
    .flatten();

    for relic in relics {
        assert_matrix_contains(RELIC_POTION_MATRIX, format!("RelicKey::{relic:?}"));
    }

    for potion in IRONCLAD_POTION_POOL {
        assert_matrix_contains(RELIC_POTION_MATRIX, format!("Potion::{potion:?}"));
    }
    for surface in [
        "RelicPoolState",
        "RelicSpawnContext",
        "reward acquisition",
        "shop acquisition",
        "Potion-relic interactions",
        "Entropic Brew fill behavior",
    ] {
        assert_matrix_contains(RELIC_POTION_MATRIX, surface);
    }
}

#[test]
fn run_world_matrix_covers_known_run_world_surfaces() {
    for definition in [
        FIXED_SIMPLE_MONSTER,
        CULTIST_A0,
        JAW_WORM_A0,
        GREMLIN_NOB_A0,
        RED_LOUSE_A0,
        GREEN_LOUSE_A0,
        LAGAVULIN_A0,
        SENTRY_A0,
        HEXAGHOST_A0,
        SLIME_BOSS_A0,
        GUARDIAN_A0,
        SPIKE_SLIME_A0,
        ACID_SLIME_A0,
    ] {
        assert_matrix_contains(RUN_WORLD_MATRIX, content_id_token(definition.content_id));
        assert_matrix_contains(RUN_WORLD_MATRIX, definition.name);
    }

    for encounter in EXORDIUM_WEAK_ENCOUNTERS
        .iter()
        .chain(EXORDIUM_STRONG_ENCOUNTERS.iter())
    {
        assert_matrix_contains(RUN_WORLD_MATRIX, encounter.0);
    }

    for event in [
        Event::GoldenShrine,
        Event::BigFish,
        Event::TheCleric,
        Event::DeadAdventurer,
        Event::GoldenIdol,
        Event::WingStatue,
        Event::WorldOfGoop,
        Event::TheSsssserpent,
        Event::LivingWall,
        Event::HypnotizingColoredMushrooms,
        Event::ScrapOoze,
        Event::ShiningLight,
        Event::Transmorgrifier,
        Event::Purifier,
        Event::UpgradeShrine,
        Event::WheelOfChange,
        Event::MatchAndKeep,
    ] {
        assert_matrix_contains(RUN_WORLD_MATRIX, format!("{event:?}"));
    }

    for room_kind in [
        RoomKind::Combat,
        RoomKind::Elite,
        RoomKind::Rest,
        RoomKind::Shop,
        RoomKind::Event,
        RoomKind::Treasure,
        RoomKind::Boss,
    ] {
        assert_matrix_contains(RUN_WORLD_MATRIX, format!("{room_kind:?}"));
    }

    for rest_action in ["Heal", "OpenSmith", "Smith", "RemoveCard", "Lift", "Dig"] {
        assert_matrix_contains(RUN_WORLD_MATRIX, rest_action);
    }

    for surface in [
        "reward_screen",
        "shop_screen",
        "map_topology",
        "ascension_delta",
        "verifier_trace",
        "corpus_boundary",
    ] {
        assert_matrix_contains(RUN_WORLD_MATRIX, surface);
    }
}
