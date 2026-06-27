use crate::{map::TargetMapAct, rng::StsRng};

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
    generate_city_elite_encounters_with_rng(&mut rng, 10)
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
pub fn target_normal_encounter_key_at_combat_index(
    seed: i64,
    act: TargetMapAct,
    combat_index: usize,
) -> Option<String> {
    match act {
        TargetMapAct::Exordium => normal_encounter_key_at_combat_index(seed, combat_index),
        TargetMapAct::City => city_normal_encounter_key_at_combat_index(seed, combat_index),
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

pub fn generate_city_elite_encounters_with_rng(rng: &mut StsRng, count: usize) -> Vec<String> {
    let pool = normalized_monster_weights(&CITY_ELITE_ENCOUNTERS);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exordium_weak_encounters_match_captured_seed_prefixes() {
        assert_eq!(
            generate_exordium_weak_encounters(1_957_307_888_551),
            vec!["Cultist", "Jaw Worm", "2 Louse"]
        );
        assert_eq!(
            generate_exordium_weak_encounters(22_079_335_079),
            vec!["Cultist", "Small Slimes", "2 Louse"]
        );
        assert_eq!(
            generate_exordium_weak_encounters(22_079_335_078),
            vec!["Jaw Worm", "Cultist", "2 Louse"]
        );
        assert_eq!(
            generate_exordium_weak_encounters(1_218_623),
            vec!["2 Louse", "Small Slimes", "Cultist"]
        );
    }

    #[test]
    fn exordium_weak_encounters_do_not_repeat_previous_two_entries() {
        for seed in 0..256 {
            let encounters = generate_exordium_weak_encounters(seed);
            assert_ne!(encounters[0], encounters[1]);
            assert_ne!(encounters[0], encounters[2]);
            assert_ne!(encounters[1], encounters[2]);
        }
    }

    #[test]
    fn normal_encounter_keys_follow_combat_index_for_captured_prefixes() {
        assert_eq!(
            normal_encounter_key_at_combat_index(1_957_307_888_551, 0).as_deref(),
            Some("Cultist")
        );
        assert_eq!(
            normal_encounter_key_at_combat_index(1_957_307_888_551, 1).as_deref(),
            Some("Jaw Worm")
        );
        assert_eq!(
            normal_encounter_key_at_combat_index(1_957_307_888_551, 2).as_deref(),
            Some("2 Louse")
        );
        assert_eq!(
            normal_encounter_key_at_combat_index(22_079_335_079, 2).as_deref(),
            Some("2 Louse")
        );
        assert_eq!(
            target_normal_encounter_key_at_combat_index(
                1_957_307_888_551,
                TargetMapAct::Exordium,
                0
            )
            .as_deref(),
            Some("Cultist")
        );
    }

    #[test]
    fn exordium_normal_encounters_preserve_captured_weak_prefixes() {
        let verify01 = generate_exordium_normal_encounters(1_957_307_888_551);
        assert_eq!(verify01.len(), 16);
        assert_eq!(&verify01[..3], ["Cultist", "Jaw Worm", "2 Louse"]);

        let codex04 = generate_exordium_normal_encounters(22_079_335_079);
        assert_eq!(codex04.len(), 16);
        assert_eq!(&codex04[..3], ["Cultist", "Small Slimes", "2 Louse"]);

        let codex03 = generate_exordium_normal_encounters(22_079_335_078);
        assert_eq!(codex03.len(), 16);
        assert_eq!(&codex03[..3], ["Jaw Worm", "Cultist", "2 Louse"]);
    }

    #[test]
    fn exordium_first_strong_enemy_uses_target_exclusions() {
        let pool = normalized_monster_weights(&EXORDIUM_STRONG_ENCOUNTERS);
        for seed in 0..4_096 {
            let mut encounters = vec!["Cultist".to_owned(), "Small Slimes".to_owned()];
            let mut rng = StsRng::new(seed);

            populate_first_strong_enemy(
                &pool,
                &mut rng,
                &mut encounters,
                &first_strong_exclusions(Some("Small Slimes")),
            );

            assert!(!matches!(
                encounters.last().map(String::as_str),
                Some("Large Slime" | "Lots of Slimes")
            ));
        }
    }

    #[test]
    fn city_encounter_pools_match_target_the_city_source() {
        assert_eq!(
            CITY_WEAK_ENCOUNTERS,
            [
                ("Spheric Guardian", 2.0),
                ("Chosen", 2.0),
                ("Shell Parasite", 2.0),
                ("3 Byrds", 2.0),
                ("2 Thieves", 2.0),
            ]
        );
        assert_eq!(
            CITY_STRONG_ENCOUNTERS,
            [
                ("Chosen and Byrds", 2.0),
                ("Sentry and Sphere", 2.0),
                ("Snake Plant", 6.0),
                ("Snecko", 4.0),
                ("Centurion and Healer", 6.0),
                ("Cultist and Chosen", 3.0),
                ("3 Cultists", 3.0),
                ("Shelled Parasite and Fungi", 3.0),
            ]
        );
        assert_eq!(
            CITY_ELITE_ENCOUNTERS,
            [
                ("Gremlin Leader", 1.0),
                ("Slavers", 1.0),
                ("Book of Stabbing", 1.0),
            ]
        );
    }

    #[test]
    fn city_normal_encounters_use_two_weak_then_thirteen_strong_entries() {
        let encounters = generate_city_normal_encounters(1_218_623);

        assert_eq!(encounters.len(), 15);
        assert!(encounters[..2]
            .iter()
            .all(|key| CITY_WEAK_ENCOUNTERS.iter().any(|(name, _)| name == key)));
        assert!(encounters[2..]
            .iter()
            .all(|key| CITY_STRONG_ENCOUNTERS.iter().any(|(name, _)| name == key)));
    }

    #[test]
    fn city_normal_encounter_key_lookup_uses_city_list() {
        let encounters = generate_city_normal_encounters(1_218_623);

        assert_eq!(
            city_normal_encounter_key_at_combat_index(1_218_623, 0).as_deref(),
            Some(encounters[0].as_str())
        );
        assert_eq!(
            target_normal_encounter_key_at_combat_index(1_218_623, TargetMapAct::City, 1)
                .as_deref(),
            Some(encounters[1].as_str())
        );
    }

    #[test]
    fn city_normal_encounters_do_not_repeat_previous_two_entries() {
        for seed in 0..1_024 {
            let encounters = generate_city_normal_encounters(seed);
            for window in encounters.windows(3) {
                assert_ne!(window[0], window[1], "seed {seed} repeated adjacent");
                assert_ne!(window[0], window[2], "seed {seed} repeated two-back");
                assert_ne!(window[1], window[2], "seed {seed} repeated adjacent");
            }
        }
    }

    #[test]
    fn city_first_strong_enemy_uses_target_exclusions() {
        let pool = normalized_monster_weights(&CITY_STRONG_ENCOUNTERS);
        for seed in 0..4_096 {
            let mut rng = StsRng::new(seed);

            let mut spheric = vec!["Spheric Guardian".to_owned()];
            populate_first_strong_enemy(
                &pool,
                &mut rng,
                &mut spheric,
                &city_first_strong_exclusions(Some("Spheric Guardian")),
            );
            assert_ne!(
                spheric.last().map(String::as_str),
                Some("Sentry and Sphere")
            );

            let mut byrds = vec!["3 Byrds".to_owned()];
            populate_first_strong_enemy(
                &pool,
                &mut rng,
                &mut byrds,
                &city_first_strong_exclusions(Some("3 Byrds")),
            );
            assert_ne!(byrds.last().map(String::as_str), Some("Chosen and Byrds"));

            let mut chosen = vec!["Chosen".to_owned()];
            populate_first_strong_enemy(
                &pool,
                &mut rng,
                &mut chosen,
                &city_first_strong_exclusions(Some("Chosen")),
            );
            assert!(!matches!(
                chosen.last().map(String::as_str),
                Some("Chosen and Byrds" | "Cultist and Chosen")
            ));
        }
    }

    #[test]
    fn city_elite_encounters_only_avoid_adjacent_repeats() {
        for seed in 0..1_024 {
            let encounters = generate_city_elite_encounters(seed);
            assert_eq!(encounters.len(), 10);
            for pair in encounters.windows(2) {
                assert_ne!(pair[0], pair[1], "seed {seed} repeated adjacent elite");
            }
            assert!(encounters
                .iter()
                .all(|key| CITY_ELITE_ENCOUNTERS.iter().any(|(name, _)| name == key)));
        }
    }
}
