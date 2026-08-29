use crate::model::AutomationConfig;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use sts_core::{RunPhase, RunState};
use sts_search::benchmark_beam_search;
use sts_verify::{
    import_communication_mod_trace, serialize_communication_mod_trace,
    verify_seed_start_communication_mod_trace, TraceLine, TraceMetadata, TraceState,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RootEntry {
    id: String,
    lineage: String,
    source: String,
    source_line: usize,
    root_file: String,
    act: i32,
    floor: i32,
    encounter: String,
    room_type: String,
    low_hp: bool,
    potion_opportunity: bool,
    challenge: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RootManifest {
    schema: u32,
    split: String,
    generated_at_unix: u64,
    entries: Vec<RootEntry>,
    exclusions: Vec<RootExclusion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RootExclusion {
    source: String,
    source_line: usize,
    reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RootEvaluation {
    root_id: String,
    lineage: String,
    act: i32,
    floor: i32,
    encounter: String,
    low_hp: bool,
    potion_opportunity: bool,
    elapsed_ms: u128,
    result: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct BenchmarkReport {
    schema: u32,
    manifest: String,
    manifest_hash_fnv64: String,
    depth: usize,
    width: usize,
    transition_budget: usize,
    timeout_ms: u64,
    denominator: usize,
    wins: usize,
    losses: usize,
    nonterminal: usize,
    illegal: usize,
    timeouts: usize,
    errors: usize,
    p95_elapsed_ms: u128,
    evaluations: Vec<RootEvaluation>,
}

pub fn run(args: impl Iterator<Item = String>) -> Result<(), String> {
    let args = args.collect::<Vec<_>>();
    match args.as_slice() {
        [command, output] if command == "freeze" => freeze(Path::new(output)),
        [command, manifest, output, depth, width, transitions, timeout]
            if command == "evaluate" =>
        {
            evaluate(
                Path::new(manifest),
                Path::new(output),
                parse(depth, "depth")?,
                parse(width, "width")?,
                parse(transitions, "transition budget")?,
                parse(timeout, "timeout ms")?,
            )
        }
        [command, root, depth, width, transitions] if command == "eval-root" => {
            eval_root(
                Path::new(root),
                parse(depth, "depth")?,
                parse(width, "width")?,
                parse(transitions, "transition budget")?,
                true,
            )
        }
        [command, root, depth, width, transitions, deduplicate] if command == "eval-root" => {
            eval_root(
                Path::new(root),
                parse(depth, "depth")?,
                parse(width, "width")?,
                parse(transitions, "transition budget")?,
                parse(deduplicate, "deduplicate")?,
            )
        }
        [command, trace, line, output] if command == "verify-prefix" => {
            verify_prefix(
                Path::new(trace),
                parse(line, "line index")?,
                Path::new(output),
            )
        }
        _ => Err("usage: combat-research freeze OUT_DIR | evaluate MANIFEST OUTPUT DEPTH WIDTH TRANSITIONS TIMEOUT_MS | eval-root ROOT DEPTH WIDTH TRANSITIONS [DEDUPLICATE]".to_owned()),
    }
}

fn parse<T: std::str::FromStr>(value: &str, label: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("invalid {label}: {value}"))
}

fn freeze(output: &Path) -> Result<(), String> {
    if output.exists() {
        return Err(format!(
            "freeze output already exists: {}",
            output.display()
        ));
    }
    fs::create_dir_all(output.join("roots")).map_err(io_error)?;
    fs::create_dir_all(output.join("tmp")).map_err(io_error)?;
    let mut paths = corpus_trace_paths()?;
    let challenge_paths = challenge_trace_paths();
    paths.extend(challenge_paths.iter().cloned());
    paths.sort();
    paths.dedup();

    let mut roots = Vec::<RootEntry>::new();
    let mut root_indices = HashMap::<String, usize>::new();
    let mut exclusions = Vec::new();
    for path in paths {
        let challenge_source = challenge_paths.contains(&path);
        extract_roots(
            &path,
            challenge_source,
            output,
            &mut roots,
            &mut root_indices,
            &mut exclusions,
        );
    }
    let _ = fs::remove_dir(output.join("tmp"));

    roots.sort_by(|left, right| {
        left.lineage
            .cmp(&right.lineage)
            .then_with(|| left.floor.cmp(&right.floor))
            .then_with(|| left.id.cmp(&right.id))
    });
    let split_assignments = assign_lineage_splits(&roots);
    let generated_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    for split in ["development", "validation", "held_out", "challenge"] {
        let entries = roots
            .iter()
            .filter(|entry| match split {
                "challenge" => entry.challenge,
                _ if entry.challenge => false,
                _ => split_assignments
                    .get(&entry.lineage)
                    .is_some_and(|assigned| *assigned == split),
            })
            .cloned()
            .collect::<Vec<_>>();
        let manifest = RootManifest {
            schema: 1,
            split: split.to_owned(),
            generated_at_unix,
            entries,
            exclusions: exclusions.clone(),
        };
        write_new_json(&output.join(format!("{split}.json")), &manifest)?;
    }
    println!(
        "{}",
        serde_json::json!({
            "roots": roots.len(),
            "exclusions": exclusions.len(),
            "output": output,
        })
    );
    Ok(())
}

fn corpus_trace_paths() -> Result<Vec<PathBuf>, String> {
    let root = std::env::var_os("STS_PERMANENT_CORPUS_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| "STS_PERMANENT_CORPUS_DIR is required".to_owned())?;
    let entries = fs::read_dir(&root).map_err(io_error)?;
    Ok(entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .collect())
}

fn challenge_trace_paths() -> Vec<PathBuf> {
    let root = sts_verify::repo_root().join("live_traces");
    [
        "session-1187.jsonl",
        "session-1188.jsonl",
        "session-1189.jsonl",
    ]
    .into_iter()
    .map(|name| root.join(name))
    .filter(|path| path.exists())
    .collect()
}

fn extract_roots(
    path: &Path,
    challenge_source: bool,
    output: &Path,
    roots: &mut Vec<RootEntry>,
    root_indices: &mut HashMap<String, usize>,
    exclusions: &mut Vec<RootExclusion>,
) {
    let source = path.display().to_string();
    let Ok(content) = fs::read_to_string(path) else {
        exclusions.push(exclusion(&source, 0, "read_failed"));
        return;
    };
    let Ok(trace) = import_communication_mod_trace(&content) else {
        exclusions.push(exclusion(&source, 0, "parse_failed"));
        return;
    };
    let mut previous_phase = None;
    for (index, line) in trace.lines.iter().enumerate() {
        let TraceLine::State(state) = line else {
            continue;
        };
        let phase = observed_string(state, "room_phase");
        let starts_combat = phase == Some("COMBAT") && previous_phase != Some("COMBAT");
        previous_phase = phase;
        if !starts_combat {
            continue;
        }
        let root = match verify_prefix_subprocess(path, index, &output.join("tmp"), 5_000) {
            Ok(root) => root,
            Err(reason) => {
                exclusions.push(exclusion(&source, index, &reason));
                if reason == "verification_timeout" {
                    exclusions.push(exclusion(
                        &source,
                        index,
                        "later_prefixes_skipped_after_monotonic_timeout",
                    ));
                    break;
                }
                continue;
            }
        };
        if root.phase != RunPhase::Combat || root.combat.is_none() {
            exclusions.push(exclusion(&source, index, "not_combat_root"));
            continue;
        }
        let serialized = match serde_json::to_vec(&root) {
            Ok(value) => value,
            Err(_) => {
                exclusions.push(exclusion(&source, index, "root_serialize_failed"));
                continue;
            }
        };
        let id = format!("{:016x}", fnv64(&serialized));
        if let Some(existing) = root_indices.get(&id).copied() {
            roots[existing].challenge |= challenge_source;
            continue;
        }
        let lineage = observed_lineage(state, path);
        let root_file = format!("roots/{id}.json");
        if fs::write(output.join(&root_file), &serialized).is_err() {
            exclusions.push(exclusion(&source, index, "root_write_failed"));
            continue;
        }
        let combat = root.combat.as_ref().expect("checked combat root");
        let encounter = combat
            .monsters
            .iter()
            .map(|monster| format!("{:?}", monster.content_id))
            .collect::<Vec<_>>()
            .join("+");
        let current_hp = combat.player.hp;
        let max_hp = combat.player.max_hp;
        let entry = RootEntry {
            id: id.clone(),
            lineage,
            source: source.clone(),
            source_line: index,
            root_file,
            act: root.current_act,
            floor: root.current_floor,
            encounter,
            room_type: observed_string(state, "room_type")
                .unwrap_or("unknown")
                .to_owned(),
            low_hp: max_hp > 0 && current_hp * 2 < max_hp,
            potion_opportunity: !root.potions.is_empty(),
            challenge: challenge_source,
        };
        root_indices.insert(id, roots.len());
        roots.push(entry);
    }
}

fn observed_string<'a>(state: &'a TraceState, field: &str) -> Option<&'a str> {
    state
        .message
        .pointer(&format!("/game_state/{field}"))
        .and_then(serde_json::Value::as_str)
}

fn observed_lineage(state: &TraceState, path: &Path) -> String {
    let seed = state.message.pointer("/game_state/seed");
    let class = observed_string(state, "class").unwrap_or("unknown");
    let ascension = state
        .message
        .pointer("/game_state/ascension_level")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(-1);
    match seed {
        Some(serde_json::Value::String(value)) => format!("{class}:a{ascension}:seed:{value}"),
        Some(value) if value.is_number() => format!("{class}:a{ascension}:seed:{value}"),
        _ => format!("path:{}", path.display()),
    }
}

fn assign_lineage_splits(roots: &[RootEntry]) -> HashMap<String, &'static str> {
    let mut max_act_by_lineage = BTreeMap::<String, i32>::new();
    for root in roots.iter().filter(|root| !root.challenge) {
        max_act_by_lineage
            .entry(root.lineage.clone())
            .and_modify(|max_act| *max_act = (*max_act).max(root.act))
            .or_insert(root.act);
    }
    let mut lineages = max_act_by_lineage.into_iter().collect::<Vec<_>>();
    lineages.sort_by(|(left_lineage, left_act), (right_lineage, right_act)| {
        right_act
            .cmp(left_act)
            .then_with(|| fnv64(left_lineage.as_bytes()).cmp(&fnv64(right_lineage.as_bytes())))
            .then_with(|| left_lineage.cmp(right_lineage))
    });
    lineages
        .into_iter()
        .enumerate()
        .map(|(index, (lineage, _))| {
            let split = match index % 7 {
                0 => "validation",
                1 => "held_out",
                _ => "development",
            };
            (lineage, split)
        })
        .collect()
}

#[cfg(test)]
fn lineage_split_for_test(lineage: &str, max_act: i32) -> &'static str {
    let roots = [RootEntry {
        id: "root".to_owned(),
        lineage: lineage.to_owned(),
        source: "trace".to_owned(),
        source_line: 0,
        root_file: "root.json".to_owned(),
        act: max_act,
        floor: 1,
        encounter: "test".to_owned(),
        room_type: "MonsterRoom".to_owned(),
        low_hp: false,
        potion_opportunity: false,
        challenge: false,
    }];
    assign_lineage_splits(&roots)[lineage]
}

fn exclusion(source: &str, source_line: usize, reason: &str) -> RootExclusion {
    RootExclusion {
        source: source.to_owned(),
        source_line,
        reason: reason.to_owned(),
    }
}

fn default_metadata() -> TraceMetadata {
    TraceMetadata {
        schema: 1,
        source: "communication_mod".to_owned(),
        boundary_schema: None,
        client: None,
        mode: None,
        started_at: None,
        ended_at: None,
        event: None,
        boss_unlocks: None,
        run_config: None,
    }
}

fn verify_prefix_subprocess(
    trace: &Path,
    line_index: usize,
    temp_dir: &Path,
    timeout_ms: u64,
) -> Result<RunState, String> {
    let output = temp_dir.join(format!(
        "{:016x}-{line_index}.json",
        fnv64(trace.display().to_string().as_bytes())
    ));
    let executable = std::env::current_exe().map_err(io_error)?;
    let mut child = Command::new(executable)
        .arg("verify-prefix")
        .arg(trace)
        .arg(line_index.to_string())
        .arg(&output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "verification_spawn_failed".to_owned())?;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                let bytes =
                    fs::read(&output).map_err(|_| "verification_output_missing".to_owned())?;
                let _ = fs::remove_file(&output);
                return serde_json::from_slice(&bytes)
                    .map_err(|_| "verification_output_invalid".to_owned());
            }
            Ok(Some(_)) => {
                let _ = fs::remove_file(&output);
                return Err("verification_rejected".to_owned());
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&output);
                return Err("verification_timeout".to_owned());
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&output);
                return Err("verification_wait_failed".to_owned());
            }
        }
    }
}

fn verify_prefix(trace_path: &Path, line_index: usize, output: &Path) -> Result<(), String> {
    let content = fs::read_to_string(trace_path).map_err(io_error)?;
    let trace = import_communication_mod_trace(&content).map_err(|error| error.to_string())?;
    if line_index >= trace.lines.len() {
        return Err("line index is outside trace".to_owned());
    }
    let metadata = trace.metadata.unwrap_or_else(default_metadata);
    let prefix = serialize_communication_mod_trace(&metadata, &trace.lines[..=line_index]);
    let report =
        verify_seed_start_communication_mod_trace(&prefix).map_err(|error| error.to_string())?;
    if !report.unexpected_diffs.is_empty() {
        return Err("prefix has fidelity diff".to_owned());
    }
    if !report.unsupported.is_empty() {
        return Err("prefix has unsupported transition".to_owned());
    }
    let root = report
        .seed_start
        .and_then(|seed| seed.sim_run_state)
        .ok_or_else(|| "prefix has no simulator root".to_owned())?;
    fs::write(
        output,
        serde_json::to_vec(&root).map_err(|error| error.to_string())?,
    )
    .map_err(io_error)
}

fn write_new_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    if path.exists() {
        return Err(format!("refusing to overwrite {}", path.display()));
    }
    let content = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, content).map_err(io_error)
}

fn evaluate(
    manifest_path: &Path,
    output: &Path,
    depth: usize,
    width: usize,
    transition_budget: usize,
    timeout_ms: u64,
) -> Result<(), String> {
    if output.exists() {
        return Err(format!("refusing to overwrite {}", output.display()));
    }
    let manifest_bytes = fs::read(manifest_path).map_err(io_error)?;
    let manifest: RootManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let executable = std::env::current_exe().map_err(io_error)?;
    let mut evaluations = Vec::new();
    for entry in &manifest.entries {
        let root = manifest_dir.join(&entry.root_file);
        let started = Instant::now();
        let result = evaluate_child(
            &executable,
            &root,
            depth,
            width,
            transition_budget,
            timeout_ms,
        );
        evaluations.push(RootEvaluation {
            root_id: entry.id.clone(),
            lineage: entry.lineage.clone(),
            act: entry.act,
            floor: entry.floor,
            encounter: entry.encounter.clone(),
            low_hp: entry.low_hp,
            potion_opportunity: entry.potion_opportunity,
            elapsed_ms: started.elapsed().as_millis(),
            result,
        });
    }
    let mut wins = 0;
    let mut losses = 0;
    let mut nonterminal = 0;
    let mut illegal = 0;
    let mut timeouts = 0;
    let mut errors = 0;
    let mut elapsed = Vec::new();
    for evaluation in &evaluations {
        elapsed.push(evaluation.elapsed_ms);
        match evaluation
            .result
            .get("outcome")
            .and_then(serde_json::Value::as_str)
        {
            Some("won") => wins += 1,
            Some("lost") => losses += 1,
            Some("illegal") => illegal += 1,
            Some("timeout") => timeouts += 1,
            Some("nonterminal" | "escaped") => nonterminal += 1,
            _ => errors += 1,
        }
    }
    elapsed.sort_unstable();
    let p95_elapsed_ms = elapsed
        .get(
            elapsed
                .len()
                .saturating_mul(95)
                .div_ceil(100)
                .saturating_sub(1),
        )
        .copied()
        .unwrap_or(0);
    let report = BenchmarkReport {
        schema: 1,
        manifest: manifest_path.display().to_string(),
        manifest_hash_fnv64: format!("{:016x}", fnv64(&manifest_bytes)),
        depth,
        width,
        transition_budget,
        timeout_ms,
        denominator: evaluations.len(),
        wins,
        losses,
        nonterminal,
        illegal,
        timeouts,
        errors,
        p95_elapsed_ms,
        evaluations,
    };
    write_new_json(output, &report)?;
    println!(
        "{}",
        serde_json::json!({
            "denominator": report.denominator,
            "wins": report.wins,
            "losses": report.losses,
            "nonterminal": report.nonterminal,
            "illegal": report.illegal,
            "timeouts": report.timeouts,
            "errors": report.errors,
            "p95_elapsed_ms": report.p95_elapsed_ms,
            "output": output,
        })
    );
    Ok(())
}

fn evaluate_child(
    executable: &Path,
    root: &Path,
    depth: usize,
    width: usize,
    transitions: usize,
    timeout_ms: u64,
) -> serde_json::Value {
    let mut child = match Command::new(executable)
        .arg("eval-root")
        .arg(root)
        .arg(depth.to_string())
        .arg(width.to_string())
        .arg(transitions.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return serde_json::json!({"outcome": "error", "error": error.to_string()}),
    };
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                if let Some(mut pipe) = child.stdout.take() {
                    let _ = pipe.read_to_string(&mut stdout);
                }
                let mut stderr = String::new();
                if let Some(mut pipe) = child.stderr.take() {
                    let _ = pipe.read_to_string(&mut stderr);
                }
                if !status.success() {
                    return serde_json::json!({"outcome": "error", "error": stderr});
                }
                return serde_json::from_str(stdout.trim()).unwrap_or_else(
                    |error| serde_json::json!({"outcome": "error", "error": error.to_string()}),
                );
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return serde_json::json!({"outcome": "timeout"});
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return serde_json::json!({"outcome": "error", "error": error.to_string()});
            }
        }
    }
}

fn eval_root(
    root: &Path,
    depth: usize,
    width: usize,
    transition_budget: usize,
    deduplicate_search_states: bool,
) -> Result<(), String> {
    let bytes = fs::read(root).map_err(io_error)?;
    let expected_id = root
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let actual_id = format!("{:016x}", fnv64(&bytes));
    if expected_id != actual_id {
        return Err(format!(
            "root hash mismatch: expected {expected_id}, got {actual_id}"
        ));
    }
    let state: RunState = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let config = AutomationConfig {
        depth,
        width,
        deduplicate_search_states,
        ..AutomationConfig::default()
    };
    let result = benchmark_beam_search(&state, &config.search_config(), transition_budget);
    println!(
        "{}",
        serde_json::to_string(&result).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lineage_split_is_stable_and_total() {
        for lineage in ["a", "b", "seed:42", "IRONCLAD:a0:seed:7"] {
            assert_eq!(
                lineage_split_for_test(lineage, 1),
                lineage_split_for_test(lineage, 1)
            );
        }
    }

    #[test]
    fn later_act_lineages_reach_both_sealed_splits() {
        let roots = (0..14)
            .map(|index| RootEntry {
                id: format!("root-{index}"),
                lineage: format!("lineage-{index}"),
                source: "trace".to_owned(),
                source_line: index,
                root_file: format!("root-{index}.json"),
                act: if index < 4 { 3 } else { 1 },
                floor: 1,
                encounter: "test".to_owned(),
                room_type: "MonsterRoom".to_owned(),
                low_hp: false,
                potion_opportunity: false,
                challenge: false,
            })
            .collect::<Vec<_>>();
        let splits = assign_lineage_splits(&roots);

        assert!(roots
            .iter()
            .any(|root| root.act == 3 && splits.get(&root.lineage) == Some(&"validation")));
        assert!(roots
            .iter()
            .any(|root| root.act == 3 && splits.get(&root.lineage) == Some(&"held_out")));
    }

    #[test]
    fn fnv_root_id_is_content_sensitive() {
        assert_eq!(fnv64(b"same"), fnv64(b"same"));
        assert_ne!(fnv64(b"same"), fnv64(b"different"));
    }
}
