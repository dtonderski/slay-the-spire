use crate::rng::StsRng;

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

/// Returns the normal encounter key for the `combat_index`-th Act 1 combat room entered.
/// Target `AbstractDungeon.monsterList` is populated once at run start; normal rooms consume
/// entries sequentially from this list.
#[must_use]
pub fn normal_encounter_key_at_combat_index(seed: i64, combat_index: usize) -> Option<String> {
    generate_exordium_normal_encounters(seed)
        .into_iter()
        .nth(combat_index)
}

pub fn generate_exordium_weak_encounters_with_rng(rng: &mut StsRng, count: usize) -> Vec<String> {
    let pool = normalized_monster_weights(&EXORDIUM_WEAK_ENCOUNTERS);
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
}
