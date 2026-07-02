use crate::{
    map::TargetMapAct,
    rng::{JavaRng, StsRng},
};

pub const EXORDIUM_WEAK_ENCOUNTERS: [(&str, f32); 4] = [
    ("Cultist", 2.0),
    ("Jaw Worm", 2.0),
    ("2 Louse", 2.0),
    ("Small Slimes", 2.0),
];

pub const EXORDIUM_STRONG_ENCOUNTERS: [(&str, f32); 10] = [
    ("Blue Slaver", 2.0),
    ("Gremlin Gang", 1.0),
    ("Looter", 2.0),
    ("Large Slime", 2.0),
    ("Lots of Slimes", 1.0),
    ("Exordium Thugs", 1.5),
    ("Exordium Wildlife", 1.5),
    ("Red Slaver", 1.0),
    ("3 Louse", 2.0),
    ("2 Fungi Beasts", 2.0),
];

pub const EXORDIUM_ELITE_ENCOUNTERS: [(&str, f32); 3] =
    [("GremlinNob", 1.0), ("Lagavulin", 1.0), ("3 Sentries", 1.0)];

pub const CITY_WEAK_ENCOUNTERS: [(&str, f32); 5] = [
    ("Spheric Guardian", 2.0),
    ("Chosen", 2.0),
    ("Shell Parasite", 2.0),
    ("3 Byrds", 2.0),
    ("2 Thieves", 2.0),
];

pub const CITY_STRONG_ENCOUNTERS: [(&str, f32); 8] = [
    ("Chosen and Byrds", 2.0),
    ("Sentry and Sphere", 2.0),
    ("Snake Plant", 6.0),
    ("Snecko", 4.0),
    ("Centurion and Healer", 6.0),
    ("Cultist and Chosen", 3.0),
    ("3 Cultists", 3.0),
    ("Shelled Parasite and Fungi", 3.0),
];

pub const CITY_ELITE_ENCOUNTERS: [(&str, f32); 3] = [
    ("Gremlin Leader", 1.0),
    ("Slavers", 1.0),
    ("Book of Stabbing", 1.0),
];

pub const BEYOND_WEAK_ENCOUNTERS: [(&str, f32); 3] =
    [("3 Darklings", 2.0), ("Orb Walker", 2.0), ("3 Shapes", 2.0)];

pub const BEYOND_STRONG_ENCOUNTERS: [(&str, f32); 8] = [
    ("Spire Growth", 1.0),
    ("Transient", 1.0),
    ("4 Shapes", 1.0),
    ("Maw", 1.0),
    ("Sphere and 2 Shapes", 1.0),
    ("Jaw Worm Horde", 1.0),
    ("3 Darklings", 1.0),
    ("Writhing Mass", 1.0),
];

pub const BEYOND_ELITE_ENCOUNTERS: [(&str, f32); 3] =
    [("Giant Head", 2.0), ("Nemesis", 2.0), ("Reptomancer", 2.0)];

#[must_use]
pub fn generate_exordium_weak_encounters(seed: i64) -> Vec<String> {
    let mut rng = StsRng::new(seed);
    generate_exordium_weak_encounters_with_rng(&mut rng, 3)
}

#[must_use]
pub fn generate_exordium_normal_encounters(seed: i64) -> Vec<String> {
    let mut rng = StsRng::new(seed);
    let mut encounters = generate_exordium_weak_encounters_with_rng(&mut rng, 3);
    append_exordium_strong_encounters_with_rng(&mut rng, &mut encounters, 12);
    encounters
}

#[must_use]
pub fn generate_exordium_elite_encounters(seed: i64) -> Vec<String> {
    let mut rng = StsRng::new(seed);
    let mut normal_encounters = generate_exordium_weak_encounters_with_rng(&mut rng, 3);
    append_exordium_strong_encounters_with_rng(&mut rng, &mut normal_encounters, 12);
    generate_exordium_elite_encounters_with_rng(&mut rng, 10)
}

#[must_use]
pub fn target_exordium_act_one_boss(seed: i64) -> String {
    let mut rng = StsRng::new(seed);
    let mut normal_encounters = generate_exordium_weak_encounters_with_rng(&mut rng, 3);
    append_exordium_strong_encounters_with_rng(&mut rng, &mut normal_encounters, 12);
    let _elite_encounters = generate_exordium_elite_encounters_with_rng(&mut rng, 10);
    let mut bosses = ["The Guardian", "Hexaghost", "Slime Boss"];
    JavaRng::new(rng.random_long()).collections_shuffle(&mut bosses);
    bosses[0].to_owned()
}

#[must_use]
pub fn target_city_act_two_boss(seed: i64) -> String {
    let mut rng = StsRng::new(seed);
    let mut normal_encounters = generate_city_weak_encounters_with_rng(&mut rng, 2);
    append_city_strong_encounters_with_rng(&mut rng, &mut normal_encounters, 12);
    let _elite_encounters = generate_city_elite_encounters_with_rng(&mut rng, 10);
    let mut bosses = ["Automaton", "Collector", "Champ"];
    JavaRng::new(rng.random_long()).collections_shuffle(&mut bosses);
    bosses[0].to_owned()
}

#[must_use]
pub fn target_beyond_act_three_boss(seed: i64) -> String {
    let mut rng = StsRng::new(seed);
    advance_exordium_content_generation_rng(&mut rng);
    let _ = generate_city_encounter_lists_with_rng(&mut rng);
    let mut normal_encounters = generate_beyond_weak_encounters_with_rng(&mut rng, 2);
    append_beyond_strong_encounters_with_rng(&mut rng, &mut normal_encounters, 12);
    let _elite_encounters = generate_beyond_elite_encounters_with_rng(&mut rng, 10);
    let mut bosses = ["Awakened One", "Time Eater", "Donu and Deca"];
    JavaRng::new(rng.random_long()).collections_shuffle(&mut bosses);
    bosses[0].to_owned()
}

#[must_use]
pub fn generate_city_weak_encounters(seed: i64) -> Vec<String> {
    let mut rng = StsRng::new(seed);
    generate_city_weak_encounters_with_rng(&mut rng, 2)
}

#[must_use]
pub fn generate_city_normal_encounters(seed: i64) -> Vec<String> {
    let mut rng = StsRng::new(seed);
    let mut encounters = generate_city_weak_encounters_with_rng(&mut rng, 2);
    append_city_strong_encounters_with_rng(&mut rng, &mut encounters, 12);
    encounters
}

#[must_use]
pub fn generate_city_elite_encounters(seed: i64) -> Vec<String> {
    let mut rng = StsRng::new(seed);
    let mut normal_encounters = generate_city_weak_encounters_with_rng(&mut rng, 2);
    append_city_strong_encounters_with_rng(&mut rng, &mut normal_encounters, 12);
    generate_city_elite_encounters_with_rng(&mut rng, 10)
}

#[must_use]
pub fn generate_beyond_normal_encounters(seed: i64) -> Vec<String> {
    let mut rng = StsRng::new(seed);
    advance_exordium_content_generation_rng(&mut rng);
    let _ = generate_city_encounter_lists_with_rng(&mut rng);
    let (normal, _) = generate_beyond_encounter_lists_with_rng(&mut rng);
    normal
}

#[must_use]
pub fn generate_beyond_elite_encounters(seed: i64) -> Vec<String> {
    let mut rng = StsRng::new(seed);
    advance_exordium_content_generation_rng(&mut rng);
    let _ = generate_city_encounter_lists_with_rng(&mut rng);
    let (_, elite) = generate_beyond_encounter_lists_with_rng(&mut rng);
    elite
}

pub fn advance_exordium_content_generation_rng(rng: &mut StsRng) {
    let mut normal_encounters = generate_exordium_weak_encounters_with_rng(rng, 3);
    append_exordium_strong_encounters_with_rng(rng, &mut normal_encounters, 12);
    let _elite_encounters = generate_exordium_elite_encounters_with_rng(rng, 10);
    let _boss_shuffle_seed = rng.random_long();
}

pub fn generate_city_encounter_lists_with_rng(rng: &mut StsRng) -> (Vec<String>, Vec<String>) {
    let mut normal_encounters = generate_city_weak_encounters_with_rng(rng, 2);
    append_city_strong_encounters_with_rng(rng, &mut normal_encounters, 12);
    let elite_encounters = generate_city_elite_encounters_with_rng(rng, 10);
    (normal_encounters, elite_encounters)
}

pub fn generate_beyond_encounter_lists_with_rng(rng: &mut StsRng) -> (Vec<String>, Vec<String>) {
    let mut normal_encounters = generate_beyond_weak_encounters_with_rng(rng, 2);
    append_beyond_strong_encounters_with_rng(rng, &mut normal_encounters, 12);
    let elite_encounters = generate_beyond_elite_encounters_with_rng(rng, 10);
    let _boss_shuffle_seed = rng.random_long();
    (normal_encounters, elite_encounters)
}

/// Returns the normal encounter key for the `combat_index`-th Act 1 combat room entered.
/// Target `AbstractDungeon.monsterList` is populated once at run start; normal rooms consume
/// entries sequentially from this list.
#[must_use]
pub fn normal_encounter_key_at_combat_index(seed: i64, combat_index: usize) -> Option<String> {
    generate_exordium_normal_encounters(seed)
        .into_iter()
        .nth(combat_index)
}

/// Returns the normal encounter key for the `combat_index`-th combat room entered in the City.
#[must_use]
pub fn city_normal_encounter_key_at_combat_index(seed: i64, combat_index: usize) -> Option<String> {
    generate_city_normal_encounters(seed)
        .into_iter()
        .nth(combat_index)
}

#[must_use]
pub fn exordium_elite_encounter_key_at_combat_index(
    seed: i64,
    combat_index: usize,
) -> Option<String> {
    generate_exordium_elite_encounters(seed)
        .into_iter()
        .nth(combat_index)
}

#[must_use]
pub fn city_elite_encounter_key_at_combat_index(seed: i64, combat_index: usize) -> Option<String> {
    generate_city_elite_encounters(seed)
        .into_iter()
        .nth(combat_index)
}

#[must_use]
pub fn target_normal_encounter_key_at_combat_index(
    seed: i64,
    act: TargetMapAct,
    combat_index: usize,
) -> Option<String> {
    match act {
        TargetMapAct::Exordium => normal_encounter_key_at_combat_index(seed, combat_index),
        TargetMapAct::City => city_normal_encounter_key_at_combat_index(seed, combat_index),
        TargetMapAct::Beyond => generate_beyond_normal_encounters(seed)
            .into_iter()
            .nth(combat_index),
    }
}

pub fn generate_exordium_weak_encounters_with_rng(rng: &mut StsRng, count: usize) -> Vec<String> {
    let pool = normalized_monster_weights(&EXORDIUM_WEAK_ENCOUNTERS);
    let mut encounters = Vec::with_capacity(count);

    populate_monster_list(&pool, rng, &mut encounters, count);
    encounters
}

pub fn generate_city_weak_encounters_with_rng(rng: &mut StsRng, count: usize) -> Vec<String> {
    let pool = normalized_monster_weights(&CITY_WEAK_ENCOUNTERS);
    let mut encounters = Vec::with_capacity(count);

    populate_monster_list(&pool, rng, &mut encounters, count);
    encounters
}

pub fn generate_beyond_weak_encounters_with_rng(rng: &mut StsRng, count: usize) -> Vec<String> {
    let pool = normalized_monster_weights(&BEYOND_WEAK_ENCOUNTERS);
    let mut encounters = Vec::with_capacity(count);

    populate_monster_list(&pool, rng, &mut encounters, count);
    encounters
}

pub fn append_exordium_strong_encounters_with_rng(
    rng: &mut StsRng,
    encounters: &mut Vec<String>,
    count: usize,
) {
    let pool = normalized_monster_weights(&EXORDIUM_STRONG_ENCOUNTERS);
    let exclusions = first_strong_exclusions(encounters.last().map(String::as_str));
    populate_first_strong_enemy(&pool, rng, encounters, &exclusions);
    populate_monster_list(&pool, rng, encounters, count);
}

pub fn append_city_strong_encounters_with_rng(
    rng: &mut StsRng,
    encounters: &mut Vec<String>,
    count: usize,
) {
    let pool = normalized_monster_weights(&CITY_STRONG_ENCOUNTERS);
    let exclusions = city_first_strong_exclusions(encounters.last().map(String::as_str));
    populate_first_strong_enemy(&pool, rng, encounters, &exclusions);
    populate_monster_list(&pool, rng, encounters, count);
}

pub fn append_beyond_strong_encounters_with_rng(
    rng: &mut StsRng,
    encounters: &mut Vec<String>,
    count: usize,
) {
    let pool = normalized_monster_weights(&BEYOND_STRONG_ENCOUNTERS);
    let exclusions = beyond_first_strong_exclusions(encounters.last().map(String::as_str));
    populate_first_strong_enemy(&pool, rng, encounters, &exclusions);
    populate_monster_list(&pool, rng, encounters, count);
}

pub fn generate_exordium_elite_encounters_with_rng(rng: &mut StsRng, count: usize) -> Vec<String> {
    let pool = normalized_monster_weights(&EXORDIUM_ELITE_ENCOUNTERS);
    let mut encounters = Vec::with_capacity(count);

    populate_elite_monster_list(&pool, rng, &mut encounters, count);
    encounters
}

pub fn generate_city_elite_encounters_with_rng(rng: &mut StsRng, count: usize) -> Vec<String> {
    let pool = normalized_monster_weights(&CITY_ELITE_ENCOUNTERS);
    let mut encounters = Vec::with_capacity(count);

    populate_elite_monster_list(&pool, rng, &mut encounters, count);
    encounters
}

pub fn generate_beyond_elite_encounters_with_rng(rng: &mut StsRng, count: usize) -> Vec<String> {
    let pool = normalized_monster_weights(&BEYOND_ELITE_ENCOUNTERS);
    let mut encounters = Vec::with_capacity(count);

    populate_elite_monster_list(&pool, rng, &mut encounters, count);
    encounters
}

fn populate_monster_list(
    pool: &[(&str, f32)],
    rng: &mut StsRng,
    encounters: &mut Vec<String>,
    count: usize,
) {
    let target_len = encounters.len() + count;
    while encounters.len() < target_len {
        let candidate = roll_monster_info(&pool, rng.random_float());
        if encounters.last().is_some_and(|last| last == candidate)
            || encounters
                .len()
                .checked_sub(2)
                .and_then(|index| encounters.get(index))
                .is_some_and(|two_back| two_back == candidate)
        {
            continue;
        }
        encounters.push(candidate.to_owned());
    }
}

fn populate_elite_monster_list(
    pool: &[(&str, f32)],
    rng: &mut StsRng,
    encounters: &mut Vec<String>,
    count: usize,
) {
    let target_len = encounters.len() + count;
    while encounters.len() < target_len {
        let candidate = roll_monster_info(pool, rng.random_float());
        if encounters.last().is_some_and(|last| last == candidate) {
            continue;
        }
        encounters.push(candidate.to_owned());
    }
}

fn populate_first_strong_enemy(
    pool: &[(&str, f32)],
    rng: &mut StsRng,
    encounters: &mut Vec<String>,
    exclusions: &[&str],
) {
    loop {
        let candidate = roll_monster_info(pool, rng.random_float());
        if !exclusions.contains(&candidate) {
            encounters.push(candidate.to_owned());
            return;
        }
    }
}

fn first_strong_exclusions(last_weak: Option<&str>) -> Vec<&'static str> {
    match last_weak {
        Some("Looter") => vec!["Exordium Thugs"],
        Some("Blue Slaver") => vec!["Red Slaver", "Exordium Thugs"],
        Some("2 Louse") => vec!["3 Louse"],
        Some("Small Slimes") => vec!["Large Slime", "Lots of Slimes"],
        _ => Vec::new(),
    }
}

fn city_first_strong_exclusions(last_weak: Option<&str>) -> Vec<&'static str> {
    match last_weak {
        Some("Spheric Guardian") => vec!["Sentry and Sphere"],
        Some("3 Byrds") => vec!["Chosen and Byrds"],
        Some("Chosen") => vec!["Chosen and Byrds", "Cultist and Chosen"],
        _ => Vec::new(),
    }
}

fn beyond_first_strong_exclusions(last_weak: Option<&str>) -> Vec<&'static str> {
    match last_weak {
        Some("3 Darklings") => vec!["3 Darklings"],
        Some("Orb Walker") => vec!["Orb Walker"],
        Some("3 Shapes") => vec!["4 Shapes"],
        _ => Vec::new(),
    }
}

fn normalized_monster_weights(entries: &[(&'static str, f32)]) -> Vec<(&'static str, f32)> {
    let mut entries = entries.to_vec();
    entries.sort_by(|(_, left_weight), (_, right_weight)| left_weight.total_cmp(right_weight));
    let total: f32 = entries.iter().map(|(_, weight)| *weight).sum();
    entries
        .iter()
        .map(|(name, weight)| (*name, *weight / total))
        .collect()
}

fn roll_monster_info<'a>(entries: &'a [(&'a str, f32)], roll: f32) -> &'a str {
    let mut cumulative = 0.0;
    for (name, weight) in entries {
        cumulative += *weight;
        if roll < cumulative {
            return name;
        }
    }
    "ERROR"
}
