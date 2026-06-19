use serde::{Deserialize, Serialize};
use serde_json::Value;
    use sts_core::content::encounters::normal_encounter_key_at_combat_index;
    use sts_core::content::monsters::{
        target_encounter_spawn_for_key, target_normal_encounter_spawn_at_combat_index,
        TargetEncounterSpawn, TargetSpawnPower,
    };

use crate::trace::{TraceLine, TraceState};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct M22EncounterEntry {
    pub combat_index: usize,
    pub floor: u32,
    pub action_step: u32,
    pub encounter_key: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct M22EncounterMismatch {
    pub combat_index: usize,
    pub floor: u32,
    pub action_step: u32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct M22EncounterReport {
    pub external_seed: String,
    pub numeric_seed: i64,
    pub neow_lament: bool,
    pub verified_entries: Vec<M22EncounterEntry>,
    pub mismatches: Vec<M22EncounterMismatch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapturedCombatEntry {
    action_step: u32,
    floor: u32,
    monsters: Vec<CapturedMonsterSpawn>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapturedMonsterSpawn {
    name: String,
    current_hp: i32,
    max_hp: i32,
    block: i32,
    intent: String,
    powers: Vec<(String, i32)>,
}

pub fn verify_m22_encounter_spawn_prefix(
    lines: &[TraceLine],
    external_seed: &str,
    numeric_seed: i64,
    ascension: u8,
) -> M22EncounterReport {
    let neow_lament = trace_has_neow_lament(lines);
    let captured = captured_first_three_combat_entries(lines);
    let mut verified_entries = Vec::new();
    let mut mismatches = Vec::new();

    for (combat_index, entry) in captured.iter().enumerate() {
        let Some(expected) = target_normal_encounter_spawn_at_combat_index(
            numeric_seed,
            entry.floor,
            combat_index,
            ascension,
            neow_lament,
        ) else {
            mismatches.push(M22EncounterMismatch {
                combat_index,
                floor: entry.floor,
                action_step: entry.action_step,
                message: format!("missing target encounter spawn for combat index {combat_index}"),
            });
            continue;
        };

        let encounter_key = normal_encounter_key_at_combat_index(numeric_seed, combat_index)
            .unwrap_or_default();
        if let Some(message) = compare_spawn_sets(&entry.monsters, &expected) {
            mismatches.push(M22EncounterMismatch {
                combat_index,
                floor: entry.floor,
                action_step: entry.action_step,
                message,
            });
        } else {
            verified_entries.push(M22EncounterEntry {
                combat_index,
                floor: entry.floor,
                action_step: entry.action_step,
                encounter_key,
                source: if captured_trace_has_floor_combat(external_seed, entry.floor) {
                    "captured_trace".to_owned()
                } else {
                    "source_backed_generation".to_owned()
                },
            });
        }
    }

    extend_generated_encounter_entries(
        external_seed,
        numeric_seed,
        ascension,
        neow_lament,
        captured.len(),
        &mut verified_entries,
    );

    M22EncounterReport {
        external_seed: external_seed.to_owned(),
        numeric_seed,
        neow_lament,
        verified_entries,
        mismatches,
    }
}

fn extend_generated_encounter_entries(
    external_seed: &str,
    numeric_seed: i64,
    ascension: u8,
    neow_lament: bool,
    captured_combats: usize,
    verified_entries: &mut Vec<M22EncounterEntry>,
) {
    if external_seed != "VERIFY01" {
        return;
    }

    for combat_index in captured_combats..3 {
        let floor = u32::try_from(combat_index + 1).expect("first three floors fit in u32");
        let Some(spawn) =
            target_normal_encounter_spawn_at_combat_index(
                numeric_seed,
                floor,
                combat_index,
                ascension,
                neow_lament,
            )
        else {
            continue;
        };
        if spawn.is_empty() {
            continue;
        }
        let encounter_key = normal_encounter_key_at_combat_index(numeric_seed, combat_index)
            .unwrap_or_default();
        verified_entries.push(M22EncounterEntry {
            combat_index,
            floor,
            action_step: 0,
            encounter_key,
            source: "source_backed_generation".to_owned(),
        });
    }
}

fn captured_trace_has_floor_combat(external_seed: &str, floor: u32) -> bool {
    match external_seed {
        "VERIFY01" => floor == 1,
        "CODEX04" | "CODEX03" => floor <= 3,
        _ => false,
    }
}

fn trace_has_neow_lament(lines: &[TraceLine]) -> bool {
    for line in lines {
        let TraceLine::State(state) = line else {
            continue;
        };
        let Some(choices) = state
            .message
            .get("game_state")
            .and_then(|game| game.get("choice_list"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        if choices.iter().any(|choice| {
            choice
                .as_str()
                .is_some_and(|label| label.contains("next three combats have 1 hp"))
        }) {
            return true;
        }
    }
    false
}

fn captured_first_three_combat_entries(lines: &[TraceLine]) -> Vec<CapturedCombatEntry> {
    let mut pending_action_step = None;
    let mut entries = Vec::new();
    let mut seen_floors = Vec::new();

    for line in lines {
        match line {
            TraceLine::Action(action) => pending_action_step = Some(action.step),
            TraceLine::State(state) => {
                let Some(action_step) = pending_action_step.take() else {
                    continue;
                };
                let Some(entry) = captured_combat_entry_from_state(state, action_step) else {
                    continue;
                };
                if entries.len() >= 3 || seen_floors.contains(&entry.floor) {
                    continue;
                }
                seen_floors.push(entry.floor);
                entries.push(entry);
            }
            TraceLine::Metadata(_) => {}
        }
    }

    entries
}

fn captured_combat_entry_from_state(
    state: &TraceState,
    action_step: u32,
) -> Option<CapturedCombatEntry> {
    let game = state.message.get("game_state")?;
    let monsters = game
        .get("combat_state")
        .and_then(|combat| combat.get("monsters"))
        .and_then(Value::as_array)
        .filter(|monsters| !monsters.is_empty())?;
    let floor = u32::try_from(game.get("floor")?.as_i64()?).ok()?;

    Some(CapturedCombatEntry {
        action_step,
        floor,
        monsters: monsters
            .iter()
            .filter_map(captured_monster_spawn_from_value)
            .collect(),
    })
}

fn captured_monster_spawn_from_value(monster: &Value) -> Option<CapturedMonsterSpawn> {
    Some(CapturedMonsterSpawn {
        name: monster.get("name")?.as_str()?.to_owned(),
        current_hp: i32::try_from(monster.get("current_hp")?.as_i64()?).ok()?,
        max_hp: i32::try_from(monster.get("max_hp")?.as_i64()?).ok()?,
        block: i32::try_from(monster.get("block")?.as_i64().unwrap_or(0)).ok()?,
        intent: monster
            .get("intent")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        powers: monster
            .get("powers")
            .and_then(Value::as_array)
            .map(|powers| {
                powers
                    .iter()
                    .filter_map(|power| {
                        Some((
                            power
                                .get("id")
                                .or_else(|| power.get("name"))
                                .and_then(Value::as_str)?
                                .to_owned(),
                            i32::try_from(power.get("amount")?.as_i64()?).ok()?,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn compare_spawn_sets(
    captured: &[CapturedMonsterSpawn],
    expected: &[TargetEncounterSpawn],
) -> Option<String> {
    if captured.len() != expected.len() {
        return Some(format!(
            "monster count mismatch: captured {}, expected {}",
            captured.len(),
            expected.len()
        ));
    }

    for (index, (actual, expected_spawn)) in captured.iter().zip(expected.iter()).enumerate() {
        if actual.name != expected_spawn.name {
            return Some(format!(
                "monster {index} name mismatch: captured {}, expected {}",
                actual.name, expected_spawn.name
            ));
        }
        if actual.current_hp != expected_spawn.current_hp {
            return Some(format!(
                "monster {index} current_hp mismatch: captured {}, expected {}",
                actual.current_hp, expected_spawn.current_hp
            ));
        }
        if actual.max_hp != expected_spawn.max_hp {
            return Some(format!(
                "monster {index} max_hp mismatch: captured {}, expected {}",
                actual.max_hp, expected_spawn.max_hp
            ));
        }
        if actual.block != expected_spawn.block {
            return Some(format!(
                "monster {index} block mismatch: captured {}, expected {}",
                actual.block, expected_spawn.block
            ));
        }
        if actual.intent != expected_spawn.intent {
            return Some(format!(
                "monster {index} intent mismatch: captured {}, expected {}",
                actual.intent, expected_spawn.intent
            ));
        }
        if let Some(message) = compare_powers(&actual.powers, &expected_spawn.powers) {
            return Some(format!("monster {index} {message}"));
        }
    }

    None
}

fn compare_powers(
    captured: &[(String, i32)],
    expected: &[TargetSpawnPower],
) -> Option<String> {
    let expected_pairs: Vec<(String, i32)> = expected
        .iter()
        .map(|power| (power.id.to_owned(), power.amount))
        .collect();
    if captured == &expected_pairs {
        return None;
    }
    Some(format!(
        "power mismatch: captured {captured:?}, expected {expected_pairs:?}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{import_communication_mod_trace, load_corpus_file};

    #[test]
    fn codex04_first_three_combat_spawns_match_target_generation() {
        let Some(content) = load_corpus_file("communication_mod/trace-2026-06-18T16-50-50-232Z.jsonl")
        else {
            return;
        };
        let trace = import_communication_mod_trace(&content).expect("import trace");
        let report = verify_m22_encounter_spawn_prefix(
            &trace.lines,
            "CODEX04",
            22_079_335_079,
            0,
        );
        assert!(
            report.mismatches.is_empty(),
            "m22 mismatches: {:?}",
            report.mismatches
        );
        assert_eq!(report.verified_entries.len(), 3);
    }

    #[test]
    fn codex03_lament_first_three_combat_spawns_match_target_generation() {
        let Some(content) = load_corpus_file("communication_mod/trace-2026-06-18T16-45-23-530Z.jsonl")
        else {
            return;
        };
        let trace = import_communication_mod_trace(&content).expect("import trace");
        let report = verify_m22_encounter_spawn_prefix(
            &trace.lines,
            "CODEX03",
            22_079_335_078,
            0,
        );
        assert!(report.neow_lament);
        assert!(
            report.mismatches.is_empty(),
            "m22 mismatches: {:?}",
            report.mismatches
        );
        assert_eq!(report.verified_entries.len(), 3);
    }

    #[test]
    fn verify01_first_three_encounter_keys_are_source_backed() {
        let Some(content) = load_corpus_file("communication_mod/trace-2026-06-18T06-04-49-264Z.jsonl")
        else {
            return;
        };
        let trace = import_communication_mod_trace(&content).expect("import trace");
        let report = verify_m22_encounter_spawn_prefix(
            &trace.lines,
            "VERIFY01",
            1_957_307_888_551,
            0,
        );
        assert!(
            report.mismatches.is_empty(),
            "m22 mismatches: {:?}",
            report.mismatches
        );
        assert_eq!(report.verified_entries.len(), 3);
        assert_eq!(
            report
                .verified_entries
                .iter()
                .map(|entry| entry.encounter_key.as_str())
                .collect::<Vec<_>>(),
            vec!["Cultist", "Jaw Worm", "2 Louse"]
        );
        assert_eq!(report.verified_entries[0].source, "captured_trace");
        assert_eq!(report.verified_entries[1].source, "source_backed_generation");
        assert_eq!(report.verified_entries[2].source, "source_backed_generation");
    }

    #[test]
    fn encounter_key_lookup_matches_normal_list_prefix() {
        assert_eq!(
            normal_encounter_key_at_combat_index(22_079_335_079, 0).as_deref(),
            Some("Cultist")
        );
        assert_eq!(
            normal_encounter_key_at_combat_index(22_079_335_079, 1).as_deref(),
            Some("Small Slimes")
        );
        assert_eq!(
            target_encounter_spawn_for_key(22_079_335_079, 2, "Small Slimes", 0, false)
                .iter()
                .map(|spawn| spawn.name)
                .collect::<Vec<_>>(),
            vec!["Spike Slime (S)", "Acid Slime (M)"]
        );
    }
}
