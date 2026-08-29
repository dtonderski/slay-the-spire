use crate::{
    bridge::BridgeManager,
    fidelity::FidelityChecker,
    model::{
        AutomationConfig, AutomationPolicy, BridgeId, Character, FidelityKind, LegalActionKind,
        LiveError, LivePhase, LiveResult, LiveState, RunConfig, RunSeed, SessionId,
        SessionSnapshot, SlayTheDataCollectionBlockerKind, SlayTheDataRepairPacket,
        SlayTheDataRunOutcome, SlayTheDataRunSummary, SlayTheDataSearchFilters,
    },
    replay::{replay_existing_trace, ReplayRequest},
    session::SessionStore,
};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use sts_verify::{
    parse_trace_jsonl_line, verify_communication_mod_trace_reader, SimRealReport,
    SlayTheDataReplayStepKind, TraceLine,
};

pub fn run_cli<B, F>(
    store: &mut SessionStore<B, F>,
    args: impl IntoIterator<Item = String>,
) -> LiveResult<Value>
where
    B: BridgeManager,
    F: FidelityChecker,
{
    let mut ignore_events = |_event: Value| {};
    run_cli_with_events(store, args, &mut ignore_events)
}

pub(crate) fn run_cli_with_events<B, F>(
    store: &mut SessionStore<B, F>,
    args: impl IntoIterator<Item = String>,
    emit: &mut dyn FnMut(Value),
) -> LiveResult<Value>
where
    B: BridgeManager,
    F: FidelityChecker,
{
    let args: Vec<String> = args.into_iter().collect();
    match args.as_slice() {
        [command, trace_path, rest @ ..] if command == "replay" => {
            let request = parse_replay_args(trace_path, rest)?;
            Ok(serde_json::to_value(replay_existing_trace(
                store, request,
            )?)?)
        }
        [area, command] if area == "bridges" && command == "list" => {
            Ok(json!({"bridges": store.list_bridges()?}))
        }
        [area, command, flag] if area == "bridges" && command == "kill" && flag == "--all" => {
            Ok(json!({"killed": store.kill_all_bridges()?}))
        }
        [area, command, bridge_id]
            if area == "bridges" && command == "kill" && bridge_id != "--all" =>
        {
            store.kill_bridge(&BridgeId(bridge_id.clone()))?;
            Ok(json!({"killed": 1, "bridge_id": bridge_id}))
        }
        [area, command, bridge_id] if area == "bridges" && command == "state" => Ok(
            serde_json::to_value(store.request_bridge_state(&BridgeId(bridge_id.clone()))?)?,
        ),
        [area, command, bridge_id] if area == "bridges" && command == "abandon" => Ok(
            serde_json::to_value(store.abandon_bridge_run(&BridgeId(bridge_id.clone()))?)?,
        ),
        [area, command] if area == "sessions" && command == "list" => {
            Ok(json!({"sessions": store.list_sessions()}))
        }
        [area, command, rest @ ..] if area == "sessions" && command == "start" => {
            let request = parse_start_args(rest)?;
            Ok(serde_json::to_value(
                store.start_run(request.bridge_id, request.config)?,
            )?)
        }
        [area, command, session_id] if area == "sessions" && command == "state" => Ok(
            serde_json::to_value(store.session_snapshot(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id] if area == "sessions" && command == "request-state" => Ok(
            serde_json::to_value(store.request_state(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id] if area == "sessions" && command == "abandon" => Ok(
            serde_json::to_value(
                store.abandon_run(&SessionId(session_id.clone()), "operator_cli")?,
            )?,
        ),
        [area, command, session_id] if area == "actions" && command == "list" => Ok(json!({
            "session_id": session_id,
            "legal_actions": store.actions(&SessionId(session_id.clone()))?,
        })),
        [area, command, session_id, action_id] if area == "actions" && command == "send" => {
            Ok(serde_json::to_value(store.send_action(
                &SessionId(session_id.clone()),
                &crate::model::ActionId(action_id.clone()),
            )?)?)
        }
        [area, command, rest @ ..] if area == "slaythedata" && command == "search" => {
            let filters = parse_slaythedata_filters(rest)?;
            Ok(json!({"runs": store.search_slaythedata_runs(filters)?}))
        }
        [area, command, run_id] if area == "slaythedata" && command == "json" => {
            let run_id = run_id
                .parse()
                .map_err(|err| LiveError::InvalidAction(format!("invalid run id: {err}")))?;
            store.slaythedata_run_json(run_id)
        }
        [area, command, rest @ ..] if area == "slaythedata" && command == "mark-illegal" => {
            run_slaythedata_mark_illegal(rest)
        }
        [area, command, run_id, reason @ ..]
            if area == "slaythedata" && command == "mark-broken" =>
        {
            let run_id = run_id
                .parse()
                .map_err(|err| LiveError::InvalidAction(format!("invalid run id: {err}")))?;
            let reason = (!reason.is_empty()).then(|| reason.join(" "));
            Ok(serde_json::to_value(
                store.mark_slaythedata_run_broken(run_id, reason.as_deref())?,
            )?)
        }
        [area, command, run_id] if area == "slaythedata" && command == "unmark-broken" => {
            let run_id = run_id
                .parse()
                .map_err(|err| LiveError::InvalidAction(format!("invalid run id: {err}")))?;
            Ok(json!({
                "run_id": run_id,
                "unmarked": store.unmark_slaythedata_run_broken(run_id)?,
            }))
        }
        [area, command, session_id, run_id] if area == "slaythedata" && command == "attach" => {
            let run_id = run_id
                .parse()
                .map_err(|err| LiveError::InvalidAction(format!("invalid run id: {err}")))?;
            Ok(serde_json::to_value(store.attach_slaythedata_run(
                &SessionId(session_id.clone()),
                run_id,
            )?)?)
        }
        [area, command, session_id] if area == "slaythedata" && command == "send-next" => {
            let session_id = SessionId(session_id.clone());
            store.ensure_slaythedata_attachment(&session_id)?;
            Ok(serde_json::to_value(
                store.slaythedata_send_next(&session_id)?,
            )?)
        }
        [area, command, session_id] if area == "slaythedata" && command == "skip-shop" => {
            let session_id = SessionId(session_id.clone());
            store.ensure_slaythedata_attachment(&session_id)?;
            if slaythedata_cli_needs_shop_block_recheck(&store.session_snapshot(&session_id)?) {
                store.slaythedata_send_next(&session_id)?;
            }
            Ok(serde_json::to_value(
                store.slaythedata_skip_shop(&session_id)?,
            )?)
        }
        [area, command, session_id] if area == "slaythedata" && command == "auto-play" => {
            let session_id = SessionId(session_id.clone());
            store.ensure_slaythedata_attachment(&session_id)?;
            Ok(serde_json::to_value(
                store.slaythedata_auto_play(&session_id)?,
            )?)
        }
        [area, command, session_id, rest @ ..] if area == "slaythedata" && command == "resume" => {
            let request = parse_slaythedata_resume_args(session_id, rest)?;
            Ok(run_slaythedata_resume(store, request, emit))
        }
        [area, command, rest @ ..] if area == "slaythedata" && command == "collect" => {
            let request = parse_slaythedata_collect_args(rest)?;
            Ok(run_slaythedata_collection(store, request, emit))
        }
        [area, command, session_id] if area == "automation" && command == "status" => Ok(
            serde_json::to_value(store.automation_status(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id, rest @ ..]
            if area == "automation" && command == "configure" =>
        {
            let config = parse_automation_config(rest)?;
            Ok(serde_json::to_value(store.configure_automation(
                &SessionId(session_id.clone()),
                config,
            )?)?)
        }
        [area, command, session_id] if area == "automation" && command == "plan" => Ok(
            serde_json::to_value(store.automation_plan(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id] if area == "automation" && command == "send-ready" => Ok(
            serde_json::to_value(store.automation_send_ready(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id]
            if area == "automation" && (command == "step" || command == "run-one") =>
        {
            Ok(serde_json::to_value(
                store.automation_step(&SessionId(session_id.clone()))?,
            )?)
        }
        [area, command, session_id] if area == "automation" && command == "auto-play" => Ok(
            serde_json::to_value(store.automation_auto_play(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id] if area == "automation" && command == "pause" => Ok(
            serde_json::to_value(store.automation_pause(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id] if area == "automation" && command == "resume" => Ok(
            serde_json::to_value(store.automation_resume(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id] if area == "automation" && command == "cancel" => Ok(
            serde_json::to_value(store.automation_cancel(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id] if area == "fidelity" && command == "status" => {
            let snapshot = store.session_snapshot(&SessionId(session_id.clone()))?;
            Ok(serde_json::to_value(snapshot.fidelity)?)
        }
        [area, command, session_id] if area == "trace" && command == "path" => {
            let snapshot = store.session_snapshot(&SessionId(session_id.clone()))?;
            Ok(json!({"session_id": session_id, "trace_path": snapshot.trace_path}))
        }
        [area, command, session_id] if area == "trace" && command == "verify" => {
            trace_verify_json(store, &SessionId(session_id.clone()))
        }
        [area, command, session_id, rest @ ..] if area == "trace" && command == "promote" => {
            let request = parse_trace_promote_args(rest)?;
            trace_promote_json(store, &SessionId(session_id.clone()), request)
        }
        _ => Err(LiveError::InvalidAction(usage())),
    }
}

struct StartRequest {
    bridge_id: BridgeId,
    config: RunConfig,
}

fn parse_replay_args(trace_path: &str, args: &[String]) -> LiveResult<ReplayRequest> {
    let mut bridge_id = BridgeId("communication-mod".to_owned());
    let mut reset_bridge = false;
    let mut max_actions = None;
    let mut dry_run = false;
    let mut action_template = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--bridge" => {
                index += 1;
                bridge_id = BridgeId(required(args, index, "--bridge")?.to_owned());
            }
            "--reset-bridge" => reset_bridge = true,
            "--max-actions" => {
                index += 1;
                max_actions = Some(required(args, index, "--max-actions")?.parse().map_err(
                    |err| LiveError::InvalidAction(format!("invalid --max-actions: {err}")),
                )?);
            }
            "--dry-run" => dry_run = true,
            "--action-template" => action_template = true,
            other => return Err(LiveError::InvalidAction(format!("unknown flag {other}"))),
        }
        index += 1;
    }
    Ok(ReplayRequest {
        source_path: PathBuf::from(trace_path),
        bridge_id,
        reset_bridge,
        max_actions,
        dry_run,
        action_template,
    })
}

fn parse_start_args(args: &[String]) -> LiveResult<StartRequest> {
    let mut bridge_id = BridgeId("fake-bridge-1".to_owned());
    let mut character = Character::Ironclad;
    let mut ascension = 0;
    let mut seed: Option<RunSeed> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--bridge" => {
                index += 1;
                bridge_id = BridgeId(required(args, index, "--bridge")?.to_owned());
            }
            "--character" => {
                index += 1;
                character = match required(args, index, "--character")?
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "ironclad" => Character::Ironclad,
                    other => {
                        return Err(LiveError::InvalidAction(format!(
                            "unsupported character {other}"
                        )))
                    }
                };
            }
            "--ascension" => {
                index += 1;
                ascension = required(args, index, "--ascension")?
                    .parse()
                    .map_err(|err| LiveError::InvalidAction(format!("invalid ascension: {err}")))?;
            }
            "--seed" => {
                index += 1;
                let value = required(args, index, "--seed")?;
                seed = Some(
                    value
                        .parse::<i64>()
                        .map_or_else(|_| RunSeed::External(value.to_owned()), RunSeed::Numeric),
                );
            }
            other => return Err(LiveError::InvalidAction(format!("unknown flag {other}"))),
        }
        index += 1;
    }
    let seed = seed.ok_or_else(|| LiveError::InvalidAction("--seed is required".to_owned()))?;
    Ok(StartRequest {
        bridge_id,
        config: RunConfig {
            character,
            ascension,
            seed,
            profile: None,
        },
    })
}

fn parse_automation_config(args: &[String]) -> LiveResult<AutomationConfig> {
    let mut config = AutomationConfig::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--policy" => {
                index += 1;
                config.policy = parse_automation_policy(required(args, index, "--policy")?)?;
            }
            "--depth" => {
                index += 1;
                config.depth = parse_usize(required(args, index, "--depth")?, "--depth")?;
            }
            "--width" => {
                index += 1;
                config.width = parse_usize(required(args, index, "--width")?, "--width")?;
            }
            "--auto-action-limit" => {
                index += 1;
                config.auto_action_limit = parse_usize(
                    required(args, index, "--auto-action-limit")?,
                    "--auto-action-limit",
                )?;
            }
            "--search-transition-budget" => {
                index += 1;
                config.search_transition_budget = parse_usize(
                    required(args, index, "--search-transition-budget")?,
                    "--search-transition-budget",
                )?;
            }
            "--search-time-budget-ms" => {
                index += 1;
                config.search_time_budget_ms = required(args, index, "--search-time-budget-ms")?
                    .parse()
                    .map_err(|err| {
                        LiveError::InvalidAction(format!("invalid --search-time-budget-ms: {err}"))
                    })?;
            }
            "--search-dedup" => config.deduplicate_search_states = true,
            "--potions" | "--potion-slots" => {
                index += 1;
                config.allowed_potion_slots =
                    parse_potion_slots(required(args, index, "--potions")?)?;
            }
            other => return Err(LiveError::InvalidAction(format!("unknown flag {other}"))),
        }
        index += 1;
    }
    Ok(config)
}

fn parse_slaythedata_filters(args: &[String]) -> LiveResult<SlayTheDataSearchFilters> {
    let mut filters = SlayTheDataSearchFilters {
        character: "IRONCLAD".to_owned(),
        ascension: Some(0),
        min_floor_reached: 1,
        max_floor_reached: None,
        victory: None,
        run_outcome: None,
        neow_bonus: None,
        seed_played: None,
        run_id: None,
        limit: 50,
        require_supported: true,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--character" => {
                index += 1;
                filters.character = required(args, index, "--character")?.to_ascii_uppercase();
            }
            "--ascension" => {
                index += 1;
                filters.ascension = Some(required(args, index, "--ascension")?.parse().map_err(
                    |err| LiveError::InvalidAction(format!("invalid --ascension: {err}")),
                )?);
            }
            "--any-ascension" => {
                filters.ascension = None;
            }
            "--min-floor" | "--min-floor-reached" => {
                index += 1;
                filters.min_floor_reached =
                    required(args, index, "--min-floor")?
                        .parse()
                        .map_err(|err| {
                            LiveError::InvalidAction(format!("invalid --min-floor: {err}"))
                        })?;
            }
            "--max-floor" | "--max-floor-reached" => {
                index += 1;
                filters.max_floor_reached = Some(
                    required(args, index, "--max-floor")?
                        .parse()
                        .map_err(|err| {
                            LiveError::InvalidAction(format!("invalid --max-floor: {err}"))
                        })?,
                );
            }
            "--victory" | "--win" => filters.run_outcome = Some(SlayTheDataRunOutcome::Win),
            "--loss" | "--defeat" => filters.run_outcome = Some(SlayTheDataRunOutcome::Loss),
            "--abandon" | "--abandoned" => {
                filters.run_outcome = Some(SlayTheDataRunOutcome::Abandon)
            }
            "--any-outcome" => {
                filters.victory = None;
                filters.run_outcome = None;
            }
            "--neow-bonus" | "--neow_bonus" => {
                index += 1;
                filters.neow_bonus = Some(required(args, index, "--neow-bonus")?.to_owned());
            }
            "--seed" => {
                index += 1;
                filters.seed_played = Some(required(args, index, "--seed")?.to_owned());
            }
            "--run-id" | "--id" => {
                index += 1;
                filters.run_id =
                    Some(required(args, index, "--run-id")?.parse().map_err(|err| {
                        LiveError::InvalidAction(format!("invalid --run-id: {err}"))
                    })?);
            }
            "--limit" => {
                index += 1;
                filters.limit = parse_usize(required(args, index, "--limit")?, "--limit")?;
            }
            "--include-unsupported" => filters.require_supported = false,
            other => return Err(LiveError::InvalidAction(format!("unknown flag {other}"))),
        }
        index += 1;
    }
    Ok(filters)
}

struct SlayTheDataMarkIllegalRequest {
    packet_path: PathBuf,
    source_path: PathBuf,
}

fn default_slaythedata_source_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/slaythedata.rs")
}

fn parse_slaythedata_mark_illegal_args(
    args: &[String],
) -> LiveResult<SlayTheDataMarkIllegalRequest> {
    let mut packet_path = None;
    let mut source_path = default_slaythedata_source_path();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--source" => {
                index += 1;
                source_path = PathBuf::from(required(args, index, "--source")?);
            }
            value if packet_path.is_none() => packet_path = Some(PathBuf::from(value)),
            other => return Err(LiveError::InvalidAction(format!("unknown flag {other}"))),
        }
        index += 1;
    }
    let packet_path = packet_path
        .ok_or_else(|| LiveError::InvalidAction("packet path is required".to_owned()))?;
    Ok(SlayTheDataMarkIllegalRequest {
        packet_path,
        source_path,
    })
}

fn run_slaythedata_mark_illegal(args: &[String]) -> LiveResult<Value> {
    let request = parse_slaythedata_mark_illegal_args(args)?;
    let packet: Value = serde_json::from_str(&fs::read_to_string(&request.packet_path)?)?;
    let entries = illegal_run_entries_from_packet(&packet);
    if entries.is_empty() {
        return Ok(json!({
            "source_path": request.source_path,
            "added_entries": [],
            "already_present_entries": [],
            "message": "no illegal run constant entries found",
        }));
    }
    let (added_entries, already_present_entries) =
        apply_illegal_run_entries_to_source(&request.source_path, &entries)?;
    Ok(json!({
        "source_path": request.source_path,
        "added_entries": added_entries,
        "already_present_entries": already_present_entries,
    }))
}

fn illegal_run_entries_from_packet(packet: &Value) -> Vec<String> {
    let mut entries = Vec::new();
    if let Some(entry) = packet
        .get("illegal_run_constant_entry")
        .and_then(Value::as_str)
        .filter(|entry| !entry.trim().is_empty())
    {
        entries.push(entry.to_owned());
    }
    if let Some(array) = packet
        .get("illegal_run_constant_entries")
        .and_then(Value::as_array)
    {
        for entry in array.iter().filter_map(Value::as_str) {
            if !entry.trim().is_empty() && !entries.iter().any(|seen| seen == entry) {
                entries.push(entry.to_owned());
            }
        }
    }
    entries
}

fn apply_illegal_run_entries_to_source(
    source_path: &Path,
    entries: &[String],
) -> LiveResult<(Vec<String>, Vec<String>)> {
    let source = fs::read_to_string(source_path)?;
    let constant_start = source
        .find("ILLEGAL_SLAYTHEDATA_RUN_IDS")
        .ok_or_else(|| LiveError::InvalidAction("illegal run constant not found".to_owned()))?;
    let insert_at = source[constant_start..]
        .find("];")
        .map(|offset| constant_start + offset)
        .ok_or_else(|| {
            LiveError::InvalidAction("illegal run constant terminator not found".to_owned())
        })?;
    let mut added = Vec::new();
    let mut already_present = Vec::new();
    for entry in entries {
        if source.contains(entry) {
            already_present.push(entry.clone());
        } else {
            added.push(entry.clone());
        }
    }
    if added.is_empty() {
        return Ok((added, already_present));
    }
    let mut updated =
        String::with_capacity(source.len() + added.iter().map(String::len).sum::<usize>());
    updated.push_str(&source[..insert_at]);
    for entry in &added {
        updated.push_str(entry);
        if !entry.ends_with('\n') {
            updated.push('\n');
        }
    }
    updated.push_str(&source[insert_at..]);
    fs::write(source_path, updated)?;
    Ok((added, already_present))
}

struct SlayTheDataCollectRequest {
    filters: SlayTheDataSearchFilters,
    include_corpus: bool,
    bridge_id: Option<BridgeId>,
    target_floor: u32,
    reset_bridge: bool,
    starting_hp: Option<i32>,
    repair_packet_path: Option<PathBuf>,
    mark_illegal_source_path: Option<PathBuf>,
    retry_journaled: bool,
    automation_config: AutomationConfig,
    output: CollectionOutputOptions,
}

#[derive(Clone)]
struct CollectionOutputOptions {
    journal_path: Option<PathBuf>,
    permanent_root: PathBuf,
    promote_floor: u32,
    promote: bool,
}

struct SlayTheDataResumeRequest {
    session_id: SessionId,
    target_floor: u32,
    automation_config: AutomationConfig,
    output: CollectionOutputOptions,
}

struct TracePromoteRequest {
    permanent_root: PathBuf,
    min_floor: u32,
}

#[derive(Clone, Copy)]
struct SlayTheDataCollectContext<'a> {
    target_floor: u32,
    starting_hp: Option<i32>,
    slaythedata_db_path: Option<&'a Path>,
    mark_illegal_source_path: Option<&'a Path>,
}

fn parse_slaythedata_collect_args(args: &[String]) -> LiveResult<SlayTheDataCollectRequest> {
    let mut bridge_id = None;
    let mut include_corpus = false;
    let mut target_floor = 60;
    let mut reset_bridge = true;
    let mut starting_hp = None;
    let mut repair_packet_path = None;
    let mut mark_illegal_source_path = None;
    let mut retry_journaled = false;
    let mut automation_config = AutomationConfig::default();
    let mut output = default_collection_output_options();
    let mut filter_args = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--reset-bridge" => {
                reset_bridge = true;
            }
            "--no-reset-bridge" => {
                reset_bridge = false;
            }
            "--bridge" => {
                index += 1;
                bridge_id = Some(BridgeId(required(args, index, "--bridge")?.to_owned()));
            }
            "--target-floor" => {
                index += 1;
                target_floor = required(args, index, "--target-floor")?
                    .parse()
                    .map_err(|err| {
                        LiveError::InvalidAction(format!("invalid --target-floor: {err}"))
                    })?;
            }
            "--starting-hp" => {
                index += 1;
                let parsed = required(args, index, "--starting-hp")?
                    .parse::<i32>()
                    .map_err(|err| {
                        LiveError::InvalidAction(format!("invalid --starting-hp: {err}"))
                    })?;
                if !(1..=1_000_000).contains(&parsed) {
                    return Err(LiveError::InvalidAction(format!(
                        "--starting-hp must be between 1 and 1000000, got {parsed}"
                    )));
                }
                starting_hp = Some(parsed);
            }
            "--repair-packet" => {
                index += 1;
                repair_packet_path = Some(PathBuf::from(required(args, index, "--repair-packet")?));
            }
            "--mark-illegal" => {
                mark_illegal_source_path = Some(default_slaythedata_source_path());
            }
            "--mark-illegal-source" => {
                index += 1;
                mark_illegal_source_path = Some(PathBuf::from(required(
                    args,
                    index,
                    "--mark-illegal-source",
                )?));
            }
            "--retry-journaled" => {
                retry_journaled = true;
            }
            "--include-corpus" => {
                include_corpus = true;
            }
            "--combat-search-transition-budget" => {
                index += 1;
                automation_config.search_transition_budget = parse_usize(
                    required(args, index, "--combat-search-transition-budget")?,
                    "--combat-search-transition-budget",
                )?;
            }
            "--combat-search-time-budget-ms" => {
                index += 1;
                automation_config.search_time_budget_ms =
                    required(args, index, "--combat-search-time-budget-ms")?
                        .parse()
                        .map_err(|err| {
                            LiveError::InvalidAction(format!(
                                "invalid --combat-search-time-budget-ms: {err}"
                            ))
                        })?;
            }
            "--combat-search-dedup" => {
                automation_config.deduplicate_search_states = true;
            }
            "--journal" => {
                index += 1;
                output.journal_path = Some(PathBuf::from(required(args, index, "--journal")?));
            }
            "--permanent-root" => {
                index += 1;
                output.permanent_root = PathBuf::from(required(args, index, "--permanent-root")?);
                output.promote = true;
            }
            "--promote-floor" => {
                index += 1;
                output.promote_floor =
                    required(args, index, "--promote-floor")?
                        .parse()
                        .map_err(|err| {
                            LiveError::InvalidAction(format!("invalid --promote-floor: {err}"))
                        })?;
            }
            "--no-promote" => {
                output.promote = false;
            }
            other => filter_args.push(other.to_owned()),
        }
        index += 1;
    }
    let filters = parse_slaythedata_filters(&filter_args)?;
    Ok(SlayTheDataCollectRequest {
        filters,
        include_corpus,
        bridge_id,
        target_floor,
        reset_bridge,
        starting_hp,
        repair_packet_path,
        mark_illegal_source_path,
        retry_journaled,
        automation_config,
        output,
    })
}

fn default_collection_output_options() -> CollectionOutputOptions {
    let permanent_root = configured_permanent_corpus_root();
    CollectionOutputOptions {
        journal_path: None,
        permanent_root: permanent_root.clone().unwrap_or_default(),
        promote_floor: 11,
        promote: permanent_root.is_some(),
    }
}

fn parse_slaythedata_resume_args(
    session_id: &str,
    args: &[String],
) -> LiveResult<SlayTheDataResumeRequest> {
    let mut target_floor = 60;
    let mut automation_config = AutomationConfig::default();
    let mut output = default_collection_output_options();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--target-floor" => {
                index += 1;
                target_floor = required(args, index, "--target-floor")?
                    .parse()
                    .map_err(|err| {
                        LiveError::InvalidAction(format!("invalid --target-floor: {err}"))
                    })?;
            }
            "--combat-search-transition-budget" => {
                index += 1;
                automation_config.search_transition_budget = parse_usize(
                    required(args, index, "--combat-search-transition-budget")?,
                    "--combat-search-transition-budget",
                )?;
            }
            "--combat-search-time-budget-ms" => {
                index += 1;
                automation_config.search_time_budget_ms =
                    required(args, index, "--combat-search-time-budget-ms")?
                        .parse()
                        .map_err(|err| {
                            LiveError::InvalidAction(format!(
                                "invalid --combat-search-time-budget-ms: {err}"
                            ))
                        })?;
            }
            "--combat-search-dedup" => {
                automation_config.deduplicate_search_states = true;
            }
            "--journal" => {
                index += 1;
                output.journal_path = Some(PathBuf::from(required(args, index, "--journal")?));
            }
            "--permanent-root" => {
                index += 1;
                output.permanent_root = PathBuf::from(required(args, index, "--permanent-root")?);
                output.promote = true;
            }
            "--promote-floor" => {
                index += 1;
                output.promote_floor =
                    required(args, index, "--promote-floor")?
                        .parse()
                        .map_err(|err| {
                            LiveError::InvalidAction(format!("invalid --promote-floor: {err}"))
                        })?;
            }
            "--no-promote" => output.promote = false,
            other => {
                return Err(LiveError::InvalidAction(format!(
                    "unsupported slaythedata resume argument {other}"
                )));
            }
        }
        index += 1;
    }
    Ok(SlayTheDataResumeRequest {
        session_id: SessionId(session_id.to_owned()),
        target_floor,
        automation_config,
        output,
    })
}

fn parse_trace_promote_args(args: &[String]) -> LiveResult<TracePromoteRequest> {
    let mut request = TracePromoteRequest {
        permanent_root: configured_permanent_corpus_root().unwrap_or_default(),
        min_floor: 11,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--permanent-root" => {
                index += 1;
                request.permanent_root = PathBuf::from(required(args, index, "--permanent-root")?);
            }
            "--min-floor" => {
                index += 1;
                request.min_floor =
                    required(args, index, "--min-floor")?
                        .parse()
                        .map_err(|err| {
                            LiveError::InvalidAction(format!("invalid --min-floor: {err}"))
                        })?;
            }
            other => {
                return Err(LiveError::InvalidAction(format!(
                    "unsupported trace promote argument {other}"
                )));
            }
        }
        index += 1;
    }
    if request.permanent_root.as_os_str().is_empty() {
        return Err(LiveError::InvalidAction(
            "set STS_PERMANENT_CORPUS_DIR or pass --permanent-root".to_owned(),
        ));
    }
    Ok(request)
}

fn configured_permanent_corpus_root() -> Option<PathBuf> {
    std::env::var_os("STS_PERMANENT_CORPUS_DIR").map(PathBuf::from)
}

struct StrictTraceAnalysis {
    report: SimRealReport,
    max_floor: Option<u32>,
    verified_prefix_max_floor: Option<u32>,
    first_failing_step: Option<u32>,
}

fn trace_verify_json<B, F>(store: &SessionStore<B, F>, session_id: &SessionId) -> LiveResult<Value>
where
    B: BridgeManager,
    F: FidelityChecker,
{
    let snapshot = store.session_snapshot(session_id)?;
    let analysis = strict_trace_analysis(Path::new(&snapshot.trace_path))?;
    let mut result = strict_trace_summary(&analysis);
    result["session_id"] = json!(session_id);
    result["trace_path"] = json!(snapshot.trace_path);
    result["report"] = serde_json::to_value(&analysis.report)?;
    Ok(result)
}

fn trace_promote_json<B, F>(
    store: &SessionStore<B, F>,
    session_id: &SessionId,
    request: TracePromoteRequest,
) -> LiveResult<Value>
where
    B: BridgeManager,
    F: FidelityChecker,
{
    let snapshot = store.session_snapshot(session_id)?;
    let analysis = strict_trace_analysis(Path::new(&snapshot.trace_path))?;
    promote_analyzed_trace(
        store,
        session_id,
        &analysis,
        &request.permanent_root,
        request.min_floor,
    )
}

fn strict_trace_analysis(path: &Path) -> LiveResult<StrictTraceAnalysis> {
    let mut trace = BufReader::new(File::open(path)?);
    let initial_metadata = trace.get_ref().metadata()?;
    let report = verify_communication_mod_trace_reader(&mut trace).map_err(|error| {
        LiveError::Blocked(format!(
            "strict seed-start verification failed for {}: {error}",
            path.display()
        ))
    })?;
    trace.seek(SeekFrom::Start(0))?;

    let first_failing_step = first_failing_step(&report);
    let mut max_floor = None;
    let mut verified_prefix_max_floor = None;
    let mut before_failure = true;
    let mut line_index = 0usize;
    let mut encoded = String::new();
    loop {
        encoded.clear();
        if trace.read_line(&mut encoded)? == 0 {
            break;
        }
        line_index += 1;
        let Some(line) = parse_trace_jsonl_line(&encoded).map_err(|error| {
            LiveError::Blocked(format!(
                "strict trace changed or became invalid at {}:{line_index}: {error}",
                path.display()
            ))
        })?
        else {
            continue;
        };
        if matches!(
            &line,
            TraceLine::Action(action) if Some(action.step) == first_failing_step
        ) {
            before_failure = false;
        }
        let TraceLine::State(state) = line else {
            continue;
        };
        let Some(floor) = trace_state_floor(&state.message) else {
            continue;
        };
        max_floor = Some(max_floor.map_or(floor, |current: u32| current.max(floor)));
        if before_failure {
            verified_prefix_max_floor =
                Some(verified_prefix_max_floor.map_or(floor, |current: u32| current.max(floor)));
        }
    }
    let final_metadata = trace.get_ref().metadata()?;
    if initial_metadata.len() != final_metadata.len()
        || initial_metadata.modified().ok() != final_metadata.modified().ok()
    {
        return Err(LiveError::Blocked(format!(
            "strict trace changed while it was being analyzed: {}",
            path.display()
        )));
    }
    Ok(StrictTraceAnalysis {
        report,
        max_floor,
        verified_prefix_max_floor,
        first_failing_step,
    })
}

fn first_failing_step(report: &SimRealReport) -> Option<u32> {
    report
        .unexpected_diffs
        .iter()
        .map(|diff| diff.action_step)
        .chain(
            report
                .unsupported
                .iter()
                .map(|transition| transition.action_step),
        )
        .min()
}

fn trace_state_floor(message: &Value) -> Option<u32> {
    [
        "/game_state/floor",
        "/game_state/floor_num",
        "/summary/floor",
        "/floor",
        "/sim_run_state/map/floor",
    ]
    .into_iter()
    .filter_map(|path| message.pointer(path).and_then(Value::as_u64))
    .max()
    .and_then(|floor| u32::try_from(floor).ok())
}

fn strict_report_is_clean(report: &SimRealReport) -> bool {
    report.unexpected_diffs.is_empty()
        && report.unsupported.is_empty()
        && report
            .seed_start
            .as_ref()
            .is_none_or(|seed_start| !seed_start.failed)
}

fn strict_trace_summary(analysis: &StrictTraceAnalysis) -> Value {
    json!({
        "mode": "seed_start",
        "clean": strict_report_is_clean(&analysis.report),
        "total_actions": analysis.report.total_actions,
        "verified_actions": analysis.report.verified.len(),
        "unsupported_actions": analysis.report.unsupported.len(),
        "unexpected_diff_actions": analysis.report.unexpected_diffs.len(),
        "max_floor": analysis.max_floor,
        "verified_prefix_max_floor": analysis.verified_prefix_max_floor,
        "first_failing_step": analysis.first_failing_step,
        "first_boundary": analysis.report.seed_start.as_ref().map(|report| &report.first_boundary),
    })
}

fn promote_analyzed_trace<B, F>(
    store: &SessionStore<B, F>,
    session_id: &SessionId,
    analysis: &StrictTraceAnalysis,
    permanent_root: &Path,
    min_floor: u32,
) -> LiveResult<Value>
where
    B: BridgeManager,
    F: FidelityChecker,
{
    if analysis.first_failing_step.is_none() && !strict_report_is_clean(&analysis.report) {
        return Ok(json!({
            "promoted": false,
            "reason": "trace_has_unverified_tail_or_failed_seed_start_boundary",
            "min_floor": min_floor,
            "verified_prefix_max_floor": analysis.verified_prefix_max_floor,
        }));
    }
    if analysis.verified_prefix_max_floor.unwrap_or(0) < min_floor {
        return Ok(json!({
            "promoted": false,
            "reason": "verified_prefix_below_min_floor",
            "min_floor": min_floor,
            "verified_prefix_max_floor": analysis.verified_prefix_max_floor,
        }));
    }

    let destination = store.copy_verified_trace_to_permanent_corpus(session_id, permanent_root)?;
    let promoted = strict_trace_analysis(&destination)?;
    if !strict_report_is_clean(&promoted.report) {
        return Err(LiveError::Blocked(format!(
            "promoted trace is not strict seed-start clean: {}",
            destination.display()
        )));
    }
    if promoted.max_floor.unwrap_or(0) < min_floor {
        return Err(LiveError::Blocked(format!(
            "promoted trace only reaches floor {:?}, below required floor {min_floor}",
            promoted.max_floor
        )));
    }
    let run_id = store.attached_slaythedata_run_id(session_id)?;
    if let Some(run_id) = run_id {
        store.mark_slaythedata_run_in_corpus(run_id, &destination)?;
    }
    Ok(json!({
        "promoted": true,
        "path": destination,
        "run_id": run_id,
        "min_floor": min_floor,
        "max_floor": promoted.max_floor,
        "retained_prefix": analysis.first_failing_step.is_some(),
        "excluded_failure_step": analysis.first_failing_step,
    }))
}

fn append_collection_journal(path: &Path, record: &Value) -> LiveResult<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, record)?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

fn collection_journal_run_ids(path: &Path) -> LiveResult<HashSet<i64>> {
    if !path.exists() {
        return Ok(HashSet::new());
    }
    fs::read_to_string(path)?
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .try_fold(HashSet::new(), |mut ids, (index, line)| {
            let record: Value = serde_json::from_str(line).map_err(|error| {
                LiveError::InvalidAction(format!(
                    "invalid collection journal JSON at {}:{}: {error}",
                    path.display(),
                    index + 1
                ))
            })?;
            if let Some(run_id) = record.get("run_id").and_then(Value::as_i64) {
                ids.insert(run_id);
            }
            Ok(ids)
        })
}

fn select_unjournaled_runs(
    runs: Vec<SlayTheDataRunSummary>,
    journaled_run_ids: &HashSet<i64>,
    limit: usize,
) -> (Vec<SlayTheDataRunSummary>, usize) {
    let excluded = runs
        .iter()
        .filter(|run| journaled_run_ids.contains(&run.id))
        .count();
    let selected = runs
        .into_iter()
        .filter(|run| !journaled_run_ids.contains(&run.id))
        .take(limit)
        .collect();
    (selected, excluded)
}

fn finish_collection_attempt<B, F>(
    store: &SessionStore<B, F>,
    mut attempt: Value,
    elapsed: Duration,
    output: &CollectionOutputOptions,
) -> Value
where
    B: BridgeManager,
    F: FidelityChecker,
{
    attempt["journal_schema"] = json!(1);
    attempt["elapsed_ms"] = json!(elapsed.as_millis());
    attempt["finished_at_unix_ms"] = json!(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis());
    let journal_path = output
        .journal_path
        .clone()
        .unwrap_or_else(|| store.trace_root().join("slaythedata-collection.jsonl"));
    attempt["journal_path"] = json!(journal_path);

    if let Some(session_id) = attempt["session_id"].as_str() {
        let session_id = SessionId(session_id.to_owned());
        match store
            .session_snapshot(&session_id)
            .and_then(|snapshot| strict_trace_analysis(Path::new(&snapshot.trace_path)))
        {
            Ok(analysis) => {
                attempt["strict_verification"] = strict_trace_summary(&analysis);
                if output.promote {
                    match promote_analyzed_trace(
                        store,
                        &session_id,
                        &analysis,
                        &output.permanent_root,
                        output.promote_floor,
                    ) {
                        Ok(promotion) => attempt["promotion"] = promotion,
                        Err(error) => attempt["promotion_error"] = json!(error.to_string()),
                    }
                } else {
                    attempt["promotion"] = json!({
                        "promoted": false,
                        "reason": "promotion_disabled",
                    });
                }
            }
            Err(error) => attempt["strict_verification_error"] = json!(error.to_string()),
        }
    }
    if let Some(actions) = attempt
        .pointer("/strict_verification/total_actions")
        .and_then(Value::as_u64)
    {
        let seconds = elapsed.as_secs_f64();
        attempt["actions_per_second"] = json!(if seconds > 0.0 {
            actions as f64 / seconds
        } else {
            0.0
        });
    }
    if let Err(error) = append_collection_journal(&journal_path, &attempt) {
        attempt["journal_write_error"] = json!(error.to_string());
    }
    attempt
}

fn run_slaythedata_resume<B, F>(
    store: &mut SessionStore<B, F>,
    request: SlayTheDataResumeRequest,
    emit: &mut dyn FnMut(Value),
) -> Value
where
    B: BridgeManager,
    F: FidelityChecker,
{
    let started_at = Instant::now();
    let snapshot = match store.ensure_slaythedata_attachment(&request.session_id) {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => {
            return json!({
                "status": "blocked",
                "reason": "no_recorded_slaythedata_attachment",
                "session_id": request.session_id,
            });
        }
        Err(error) => {
            return json!({
                "status": "blocked",
                "reason": "restore_slaythedata_attachment_failed",
                "session_id": request.session_id,
                "message": error.to_string(),
            });
        }
    };
    let Some(run) = snapshot.slaythedata.attached_run.clone() else {
        return json!({
            "status": "blocked",
            "reason": "missing_attached_slaythedata_run",
            "session_id": request.session_id,
        });
    };
    let db_path = store.slaythedata_db_path().to_path_buf();
    let context = SlayTheDataCollectContext {
        target_floor: request.target_floor,
        starting_hp: None,
        slaythedata_db_path: Some(&db_path),
        mark_illegal_source_path: None,
    };
    let snapshot = match store
        .begin_fidelity_recheck(
            &request.session_id,
            "resume after verified simulator repair",
        )
        .and_then(|_| {
            store.configure_automation(&request.session_id, request.automation_config.clone())?;
            store.session_snapshot(&request.session_id)
        }) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return finish_collection_attempt(
                store,
                collect_attempt_json(
                    &run,
                    Some(&snapshot),
                    "blocked",
                    "repair_resume_prepare_failed",
                    Some(error),
                    context,
                ),
                started_at.elapsed(),
                &request.output,
            );
        }
    };
    emit_collection_progress(emit, "resume_started", &run, Some(&snapshot));
    let mut attempt =
        drive_attached_slaythedata_run(store, &request.session_id, &run, snapshot, context, emit);
    mark_confirmed_incompatible_attempt(store, &mut attempt);
    finish_collection_attempt(store, attempt, started_at.elapsed(), &request.output)
}

fn mark_confirmed_incompatible_attempt<B, F>(store: &SessionStore<B, F>, attempt: &mut Value)
where
    B: BridgeManager,
    F: FidelityChecker,
{
    if attempt.get("status").and_then(Value::as_str) != Some("incompatible_run") {
        return;
    }
    let Some(run_id) = attempt.get("run_id").and_then(Value::as_i64) else {
        attempt["mark_broken_error"] = json!("incompatible attempt has no run_id");
        return;
    };
    let reason = attempt
        .pointer("/repair_packet/first_simulator_diff_or_mapping_failure")
        .and_then(Value::as_str)
        .or_else(|| attempt.get("reason").and_then(Value::as_str))
        .unwrap_or("confirmed SlayTheData incompatibility")
        .to_owned();
    match store.mark_slaythedata_run_broken(run_id, Some(&reason)) {
        Ok(marked) => attempt["marked_broken"] = json!(marked),
        Err(error) => attempt["mark_broken_error"] = json!(error.to_string()),
    }
}

fn run_slaythedata_collection<B, F>(
    store: &mut SessionStore<B, F>,
    request: SlayTheDataCollectRequest,
    emit: &mut dyn FnMut(Value),
) -> Value
where
    B: BridgeManager,
    F: FidelityChecker,
{
    let repair_packet_path = request
        .repair_packet_path
        .clone()
        .unwrap_or_else(|| store.trace_root().join("slaythedata-repair.json"));
    let slaythedata_db_path = store.slaythedata_db_path().to_path_buf();
    let collect_context = SlayTheDataCollectContext {
        target_floor: request.target_floor,
        starting_hp: request.starting_hp,
        slaythedata_db_path: Some(&slaythedata_db_path),
        mark_illegal_source_path: request.mark_illegal_source_path.as_deref(),
    };
    let bridge_id = match request.bridge_id {
        Some(bridge_id) => bridge_id,
        None => match store.list_bridges() {
            Ok(bridges) => bridges
                .into_iter()
                .find(|bridge| bridge.connected)
                .map(|bridge| bridge.id)
                .unwrap_or_else(|| BridgeId("fake-bridge-1".to_owned())),
            Err(error) => {
                let mut result = collect_terminal_json(
                    "blocked",
                    "bridge_list_failed",
                    Some(error.to_string()),
                    Vec::new(),
                    Vec::new(),
                    collect_context,
                );
                write_requested_repair_packet(&mut result, Some(repair_packet_path));
                return result;
            }
        },
    };
    let journal_path = request
        .output
        .journal_path
        .clone()
        .unwrap_or_else(|| store.trace_root().join("slaythedata-collection.jsonl"));
    let journaled_run_ids = if request.retry_journaled {
        HashSet::new()
    } else {
        match collection_journal_run_ids(&journal_path) {
            Ok(run_ids) => run_ids,
            Err(error) => {
                let mut result = collect_terminal_json(
                    "blocked",
                    "journal_read_failed",
                    Some(error.to_string()),
                    Vec::new(),
                    Vec::new(),
                    collect_context,
                );
                write_requested_repair_packet(&mut result, Some(repair_packet_path));
                return result;
            }
        }
    };
    let requested_limit = request.filters.limit;
    let mut search_filters = request.filters.clone();
    search_filters.limit = requested_limit.saturating_add(journaled_run_ids.len());
    let runs = match if request.include_corpus {
        store.search_slaythedata_runs_with_corpus(search_filters)
    } else {
        store.search_slaythedata_runs(search_filters)
    } {
        Ok(runs) => runs,
        Err(error) => {
            let mut result = collect_terminal_json(
                "blocked",
                "slaythedata_search_failed",
                Some(error.to_string()),
                Vec::new(),
                Vec::new(),
                collect_context,
            );
            write_requested_repair_packet(&mut result, Some(repair_packet_path));
            return result;
        }
    };
    let (runs, excluded_journaled_runs) =
        select_unjournaled_runs(runs, &journaled_run_ids, requested_limit);

    let mut attempts = Vec::new();
    let mut illegal_run_ids_to_add = Vec::new();
    for run in runs {
        let started_at = Instant::now();
        emit_collection_progress(emit, "attempt_starting", &run, None);
        let mut attempt = collect_one_slaythedata_run(
            store,
            &bridge_id,
            &run,
            request.reset_bridge,
            &request.automation_config,
            collect_context,
            emit,
        );
        mark_confirmed_incompatible_attempt(store, &mut attempt);
        let attempt =
            finish_collection_attempt(store, attempt, started_at.elapsed(), &request.output);
        emit(json!({
            "type": "progress",
            "operation": "attempt_finished",
            "run_id": run.id,
            "session_id": attempt.get("session_id"),
            "floor": attempt.get("floor"),
            "status": attempt.get("status"),
            "reason": attempt.get("reason"),
        }));
        let blocker_kind = collect_attempt_blocker_kind(&attempt);
        if blocker_kind == Some(SlayTheDataCollectionBlockerKind::SlaythedataIllegalLog) {
            if let Some(run_id) = attempt["run_id"].as_i64() {
                illegal_run_ids_to_add.push(run_id);
            }
        }
        let should_continue = collect_should_continue_after_attempt(&attempt);
        attempts.push(attempt);
        if !should_continue {
            break;
        }
    }

    let status = attempts
        .last()
        .and_then(|attempt| attempt["status"].as_str())
        .unwrap_or("no_candidates")
        .to_owned();
    let last_repair_packet = attempts
        .last()
        .and_then(|attempt| attempt.get("repair_packet"))
        .cloned();
    let illegal_run_constant_entries = illegal_run_constant_entries(&attempts);
    let has_attempts = !attempts.is_empty();
    let mut result = if attempts.is_empty() {
        collect_terminal_json(
            &status,
            "no_candidates",
            None,
            illegal_run_ids_to_add,
            attempts,
            collect_context,
        )
    } else {
        json!({
            "status": status,
            "target_floor": request.target_floor,
            "processed_runs": attempts.len(),
            "promoted_traces": attempts.iter().filter(|attempt| {
                attempt.pointer("/promotion/promoted").and_then(Value::as_bool) == Some(true)
            }).count(),
            "act3_reached": attempts.iter().filter(|attempt| {
                attempt.get("floor").and_then(Value::as_u64).is_some_and(|floor| floor >= 34)
            }).count(),
            "illegal_run_ids_to_add": illegal_run_ids_to_add,
            "illegal_run_constant_entries": illegal_run_constant_entries,
            "attempts": attempts,
        })
    };
    if has_attempts {
        if let Some(packet) = last_repair_packet {
            result["blocker_kind"] = packet
                .get("blocker_kind")
                .cloned()
                .unwrap_or_else(|| json!("bridge_or_backend_error"));
            if let Some(command) = repair_packet_next_command(&packet) {
                result["next_command"] = json!(command);
            }
            result["repair_packet"] = packet;
        }
    }
    result["excluded_journaled_runs"] = json!(excluded_journaled_runs);
    result["retry_journaled"] = json!(request.retry_journaled);
    if let Some(source_path) = request.mark_illegal_source_path.as_ref() {
        let entries = result
            .get("illegal_run_constant_entries")
            .and_then(Value::as_array)
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        match apply_illegal_run_entries_to_source(source_path, &entries) {
            Ok((added, already_present)) => {
                let requires_rebuild = !added.is_empty();
                result["mark_illegal_source_path"] = json!(source_path);
                result["mark_illegal_added_entries"] = json!(added);
                result["mark_illegal_already_present_entries"] = json!(already_present);
                result["mark_illegal_requires_rebuild"] = json!(requires_rebuild);
                if requires_rebuild {
                    if let Some(next_command) = result.get("next_command").cloned() {
                        result["post_rebuild_command"] = next_command;
                    }
                }
            }
            Err(error) => {
                result["mark_illegal_source_path"] = json!(source_path);
                result["mark_illegal_error"] = json!(error.to_string());
            }
        }
    }
    write_requested_repair_packet(&mut result, Some(repair_packet_path));
    if let Some(command) = collect_followup_command(&result) {
        result["followup_command"] = json!(command);
    }
    result
}

fn illegal_run_constant_entries(attempts: &[Value]) -> Vec<String> {
    attempts
        .iter()
        .filter(|attempt| attempt["status"] == "slaythedata_hard_break")
        .filter_map(|attempt| {
            let run_id = attempt["run_id"].as_i64()?;
            let reason = attempt
                .pointer("/repair_packet/first_simulator_diff_or_mapping_failure")
                .and_then(Value::as_str)
                .or_else(|| attempt["reason"].as_str())
                .unwrap_or("illegal SlayTheData log");
            Some(illegal_run_constant_entry(run_id, reason))
        })
        .collect()
}

fn collect_attempt_blocker_kind(attempt: &Value) -> Option<SlayTheDataCollectionBlockerKind> {
    serde_json::from_value(
        attempt
            .get("blocker_kind")
            .cloned()
            .or_else(|| attempt.pointer("/repair_packet/blocker_kind").cloned())?,
    )
    .ok()
}

fn collect_should_continue_after_attempt(attempt: &Value) -> bool {
    if attempt.get("promotion_error").is_some()
        || attempt.get("strict_verification_error").is_some()
        || attempt.get("journal_write_error").is_some()
        || attempt.get("mark_broken_error").is_some()
    {
        return false;
    }
    match attempt.get("status").and_then(Value::as_str) {
        Some("completed_trace") => true,
        Some("incompatible_run") => true,
        Some("slaythedata_hard_break") => {
            attempt.get("reason").and_then(Value::as_str) == Some("missing_seed")
        }
        Some("blocked") => {
            attempt.get("reason").and_then(Value::as_str) == Some("game_over_before_target")
        }
        _ => false,
    }
}

fn illegal_run_constant_entry(run_id: i64, reason: &str) -> String {
    format!(
        "    {}, // SlayTheData {}.",
        format_run_id(run_id),
        reason.replace('\n', " ")
    )
}

fn format_run_id(run_id: i64) -> String {
    let sign = if run_id < 0 { "-" } else { "" };
    let digits = run_id.unsigned_abs().to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push('_');
        }
        out.push(ch);
    }
    format!("{sign}{}", out.chars().rev().collect::<String>())
}

fn collect_terminal_json(
    status: &str,
    reason: &str,
    message: Option<String>,
    illegal_run_ids_to_add: Vec<i64>,
    attempts: Vec<Value>,
    context: SlayTheDataCollectContext<'_>,
) -> Value {
    let repair_packet = collect_repair_packet(
        &collector_run_summary(),
        None,
        status,
        reason,
        message.as_deref(),
        context,
    );
    json!({
        "status": status,
        "blocker_kind": repair_packet.blocker_kind.clone(),
        "next_command": repair_packet.reproduce_command.clone(),
        "followup_command": repair_packet.reproduce_command.clone(),
        "reason": reason,
        "message": message,
        "target_floor": context.target_floor,
        "illegal_run_ids_to_add": illegal_run_ids_to_add,
        "illegal_run_constant_entries": [],
        "attempts": attempts,
        "repair_packet": repair_packet,
    })
}

fn collector_run_summary() -> SlayTheDataRunSummary {
    SlayTheDataRunSummary {
        id: 0,
        seed_played: None,
        build_version: None,
        ascension_level: None,
        floor_reached: None,
        victory: false,
        run_outcome: SlayTheDataRunOutcome::Loss,
        path_length: None,
        card_choice_count: None,
        event_choice_count: None,
        shop_purchase_count: None,
        potion_usage_count: None,
        neow_bonus: None,
        neow_cost: None,
        guided_score: 0,
        materialized: false,
    }
}

fn emit_collection_progress(
    emit: &mut dyn FnMut(Value),
    operation: &str,
    run: &SlayTheDataRunSummary,
    snapshot: Option<&SessionSnapshot>,
) {
    let state = snapshot.and_then(|snapshot| snapshot.latest_state.as_ref());
    let summary = state.and_then(|state| state.raw.pointer("/summary"));
    emit(json!({
        "type": "progress",
        "operation": operation,
        "run_id": run.id,
        "seed": run.seed_played,
        "session_id": snapshot.map(|snapshot| &snapshot.session_id),
        "floor": snapshot.and_then(current_floor),
        "phase": state.map(|state| format!("{:?}", state.phase).to_lowercase()),
        "current_hp": summary.and_then(|summary| summary.get("current_hp")),
        "max_hp": summary.and_then(|summary| summary.get("max_hp")),
    }));
}

fn write_requested_repair_packet(result: &mut Value, repair_packet_path: Option<PathBuf>) {
    if let Some(path) = repair_packet_path {
        let packet = result.get("repair_packet").cloned().or_else(|| {
            result
                .pointer("/attempts")
                .and_then(Value::as_array)
                .and_then(|attempts| attempts.last())
                .and_then(|attempt| attempt.get("repair_packet"))
                .cloned()
        });
        match packet {
            Some(packet) => match serde_json::to_string_pretty(&packet)
                .map_err(LiveError::from)
                .and_then(|content| {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&path, content)?;
                    Ok(())
                }) {
                Ok(()) => result["repair_packet_path"] = json!(path),
                Err(error) => result["repair_packet_write_error"] = json!(error.to_string()),
            },
            None => result["repair_packet_write_error"] = json!("no repair packet was produced"),
        }
    }
}

fn collect_one_slaythedata_run<B, F>(
    store: &mut SessionStore<B, F>,
    bridge_id: &BridgeId,
    run: &SlayTheDataRunSummary,
    reset_bridge: bool,
    automation_config: &AutomationConfig,
    context: SlayTheDataCollectContext<'_>,
    emit: &mut dyn FnMut(Value),
) -> Value
where
    B: BridgeManager,
    F: FidelityChecker,
{
    let Some(seed) = run.seed_played.as_deref() else {
        return collect_attempt_json(
            run,
            None,
            "slaythedata_hard_break",
            "missing_seed",
            None,
            context,
        );
    };
    let config = RunConfig {
        character: Character::Ironclad,
        ascension: run.ascension_level.unwrap_or(0),
        seed: seed
            .parse::<i64>()
            .map_or_else(|_| RunSeed::External(seed.to_owned()), RunSeed::Numeric),
        profile: None,
    };
    if reset_bridge {
        emit_collection_progress(emit, "bridge_reset_started", run, None);
        if let Err(error) = reset_bridge_for_collection(store, bridge_id) {
            return collect_attempt_json(
                run,
                None,
                "blocked",
                "reset_bridge_failed",
                Some(error),
                context,
            );
        }
    }
    let started = match context.starting_hp {
        Some(starting_hp) => store.start_verification_run(bridge_id.clone(), config, starting_hp),
        None => store.start_run(bridge_id.clone(), config),
    };
    let started = match started {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return collect_attempt_json(
                run,
                None,
                "blocked",
                "start_failed",
                Some(error),
                context,
            );
        }
    };
    emit_collection_progress(emit, "run_started", run, Some(&started));
    let session_id = started.session_id.clone();
    if let Err(error) = store.configure_automation(&session_id, automation_config.clone()) {
        return collect_attempt_json(
            run,
            Some(&started),
            "blocked",
            "automation_configure_failed",
            Some(error),
            context,
        );
    }
    let snapshot = match store.attach_slaythedata_run(&session_id, run.id) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return collect_attempt_json(
                run,
                Some(&started),
                "blocked",
                "attach_failed",
                Some(error),
                context,
            );
        }
    };
    emit_collection_progress(emit, "guidance_attached", run, Some(&snapshot));
    drive_attached_slaythedata_run(store, &session_id, run, snapshot, context, emit)
}

fn drive_attached_slaythedata_run<B, F>(
    store: &mut SessionStore<B, F>,
    session_id: &SessionId,
    run: &SlayTheDataRunSummary,
    mut snapshot: SessionSnapshot,
    context: SlayTheDataCollectContext<'_>,
    emit: &mut dyn FnMut(Value),
) -> Value
where
    B: BridgeManager,
    F: FidelityChecker,
{
    let mut last_progress = None;
    for _ in 0..500 {
        snapshot = match store.request_state(session_id) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return collect_attempt_json(
                    run,
                    Some(&snapshot),
                    "blocked",
                    "request_state_failed",
                    Some(error),
                    context,
                );
            }
        };
        if snapshot.fidelity.kind == FidelityKind::Lost {
            return collect_attempt_json(
                run,
                Some(&snapshot),
                "simulator_mismatch",
                "fidelity_lost",
                None,
                context,
            );
        }
        if current_floor(&snapshot).is_some_and(|floor| floor >= context.target_floor) {
            return collect_attempt_json(
                run,
                Some(&snapshot),
                "completed_trace",
                "ok",
                None,
                context,
            );
        }
        let Some(state) = snapshot.latest_state.as_ref() else {
            return collect_attempt_json(
                run,
                Some(&snapshot),
                "blocked",
                "missing_state",
                None,
                context,
            );
        };
        let progress = (current_floor(&snapshot), state.phase.clone());
        if last_progress.as_ref() != Some(&progress) {
            emit_collection_progress(emit, "state", run, Some(&snapshot));
            last_progress = Some(progress);
        }
        if state.phase == LivePhase::GameOver {
            return collect_attempt_json(
                run,
                Some(&snapshot),
                "blocked",
                "game_over_before_target",
                None,
                context,
            );
        }
        snapshot = if state.phase == LivePhase::Combat {
            emit_collection_progress(emit, "combat_search_started", run, Some(&snapshot));
            match store.automation_auto_play(session_id) {
                Ok(snapshot) => {
                    emit_collection_progress(emit, "combat_search_finished", run, Some(&snapshot));
                    snapshot
                }
                Err(error) => {
                    return collect_attempt_json(
                        run,
                        Some(&snapshot),
                        "blocked",
                        "automation_failed",
                        Some(error),
                        context,
                    );
                }
            }
        } else {
            match store.slaythedata_send_next(session_id) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    return collect_attempt_json(
                        run,
                        Some(&snapshot),
                        "blocked",
                        "slaythedata_failed",
                        Some(error),
                        context,
                    );
                }
            }
        };
        if snapshot.fidelity.kind == FidelityKind::Lost {
            return collect_attempt_json(
                run,
                Some(&snapshot),
                "simulator_mismatch",
                "fidelity_lost",
                None,
                context,
            );
        }
        if let Some(blocked) = snapshot.slaythedata.blocked.as_ref() {
            let incompatible = slaythedata_confirmed_incompatible(&snapshot);
            let hard_break = slaythedata_hard_break(&snapshot);
            let diagnostic = (incompatible || hard_break).then(|| {
                LiveError::Blocked(
                    if incompatible {
                        "SlayTheData guidance is incompatible with the live run; the collector can safely exclude this run"
                    } else {
                        "SlayTheData guidance may be incompatible with the live run; the session was preserved for agent-selected recovery"
                    }
                    .to_owned(),
                )
            });
            return collect_attempt_json(
                run,
                Some(&snapshot),
                if incompatible {
                    "incompatible_run"
                } else if hard_break {
                    "slaythedata_hard_break"
                } else {
                    "blocked"
                },
                &blocked.reason_code,
                diagnostic,
                context,
            );
        }
        if snapshot.automation.blocked.is_some() {
            return collect_attempt_json(
                run,
                Some(&snapshot),
                "blocked",
                "automation_blocked",
                None,
                context,
            );
        }
    }
    collect_attempt_json(
        run,
        Some(&snapshot),
        "blocked",
        "collection_step_limit",
        None,
        context,
    )
}

fn reset_bridge_for_collection<B, F>(
    store: &mut SessionStore<B, F>,
    bridge_id: &BridgeId,
) -> LiveResult<()>
where
    B: BridgeManager,
    F: FidelityChecker,
{
    for _ in 0..40 {
        let state = match store.request_bridge_state(bridge_id) {
            Ok(state) => state,
            Err(error) if bridge_reset_error_is_retryable(&error) => {
                std::thread::sleep(std::time::Duration::from_millis(250));
                continue;
            }
            Err(error) => return Err(error),
        };
        if start_command_available(&state) {
            return Ok(());
        }
        if command_available(&state, "proceed") {
            let sent = store.send_bridge_command(
                bridge_id,
                &state,
                "PROCEED",
                LegalActionKind::Confirm,
                "Proceed",
            );
            if let Err(error) = sent {
                if !bridge_reset_error_is_retryable(&error) {
                    return Err(error);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
            continue;
        }
        if command_available(&state, "abandon") {
            if let Err(error) = store.abandon_bridge_run(bridge_id) {
                if !bridge_reset_error_is_retryable(&error) {
                    return Err(error);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
            continue;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    Err(LiveError::Bridge(
        "bridge did not return to start-ready state after abandon".to_owned(),
    ))
}

fn bridge_reset_error_is_retryable(error: &LiveError) -> bool {
    matches!(
        error,
        LiveError::Bridge(message)
            if message.contains("bridge is not ready for a command")
                || message.contains("stale bridge action rejected")
    )
}

fn start_command_available(state: &LiveState) -> bool {
    command_available(state, "start")
}

fn command_available(state: &LiveState, expected: &str) -> bool {
    commands_contain(state.raw.pointer("/summary/available_commands"), expected)
        || commands_contain(
            state
                .raw
                .pointer("/current_state/message/available_commands"),
            expected,
        )
}

fn commands_contain(commands: Option<&Value>, expected: &str) -> bool {
    match commands {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .any(|command| command.eq_ignore_ascii_case(expected)),
        Some(Value::String(commands)) => commands
            .split_whitespace()
            .any(|command| command.eq_ignore_ascii_case(expected)),
        _ => false,
    }
}

fn slaythedata_hard_break(snapshot: &SessionSnapshot) -> bool {
    let blocked = match snapshot.slaythedata.blocked.as_ref() {
        Some(blocked) => blocked,
        None => return false,
    };
    let advisor_code = snapshot
        .slaythedata
        .advisor
        .as_ref()
        .map(|advisor| advisor.code.as_str())
        .unwrap_or_default();
    blocked.reason_code == "map_symbol_unmatched"
        || blocked.reason_code == "neow_option_not_available"
        || blocked.reason_code == "shop_purchase_rng_critical_unavailable"
        || advisor_code.starts_with("legal_neow")
        || advisor_code == "legal_card_reward"
        || advisor_code == "guided_card_reward"
        || advisor_code == "pending_card_reward"
        || advisor_code == "legal_map_room"
        || is_guided_event_code(advisor_code)
        || guided_campfire_has_no_live_action(snapshot)
        || guided_event_choice_in_non_event_room(snapshot)
        || guided_event_choice_event_name_mismatch(snapshot)
}

fn slaythedata_confirmed_incompatible(snapshot: &SessionSnapshot) -> bool {
    let Some(blocked) = snapshot.slaythedata.blocked.as_ref() else {
        return false;
    };
    if blocked.reason_code == "shop_purchase_rng_critical_unavailable" {
        return true;
    }
    matches!(
        blocked.reason_code.as_str(),
        "pending_card_reward" | "guided_card_reward" | "legal_card_reward"
    ) && blocked.message.contains("has no live grid label match")
        && snapshot
            .latest_state
            .as_ref()
            .is_some_and(|state| state.phase == LivePhase::Reward)
}

fn slaythedata_cli_needs_shop_block_recheck(snapshot: &SessionSnapshot) -> bool {
    snapshot.slaythedata.blocked.is_none()
        && snapshot
            .latest_state
            .as_ref()
            .is_some_and(|state| state.phase == LivePhase::Shop)
        && snapshot
            .slaythedata
            .advisor
            .as_ref()
            .is_some_and(|advisor| {
                advisor.code == "guided_shop_purchase" && advisor.action_id.is_none()
            })
}

fn guided_campfire_has_no_live_action(snapshot: &SessionSnapshot) -> bool {
    let Some(advisor) = snapshot.slaythedata.advisor.as_ref() else {
        return false;
    };
    advisor.code == "guided_campfire"
        && advisor.action_id.is_none()
        && snapshot
            .slaythedata
            .blocked
            .as_ref()
            .is_some_and(|blocked| blocked.reason_code == "guided_campfire")
}

fn guided_event_choice_in_non_event_room(snapshot: &SessionSnapshot) -> bool {
    let Some(advisor) = snapshot.slaythedata.advisor.as_ref() else {
        return false;
    };
    if !is_guided_event_code(&advisor.code) {
        return false;
    }
    let Some(current_floor) = current_floor(snapshot) else {
        return false;
    };
    if current_floor < advisor.floor {
        return false;
    }
    snapshot
        .latest_state
        .as_ref()
        .and_then(|state| state.raw.pointer("/summary/room_type"))
        .and_then(Value::as_str)
        .is_some_and(|room_type| room_type != "EventRoom")
}

fn guided_event_choice_event_name_mismatch(snapshot: &SessionSnapshot) -> bool {
    let Some(advisor) = snapshot.slaythedata.advisor.as_ref() else {
        return false;
    };
    if !is_guided_event_code(&advisor.code) {
        return false;
    }
    let Some(expected) = advisor_event_name(advisor) else {
        return false;
    };
    let Some(live) = snapshot
        .latest_state
        .as_ref()
        .and_then(live_event_name_from_state)
    else {
        return false;
    };
    !event_names_match(expected, live)
}

fn is_guided_event_code(code: &str) -> bool {
    matches!(code, "guided_event_choice" | "guided_event_sequence")
}

fn advisor_event_name(advisor: &crate::model::SlayTheDataAdvisorStep) -> Option<&str> {
    match advisor.intent.as_ref()? {
        SlayTheDataReplayStepKind::EventChoice { event_name, .. } => event_name.as_deref(),
        _ => None,
    }
}

fn live_event_name_from_state(state: &LiveState) -> Option<&str> {
    state
        .raw
        .pointer("/summary/screen_state/event_id")
        .or_else(|| state.raw.pointer("/summary/screen_state/event_name"))
        .or_else(|| {
            state
                .raw
                .pointer("/current_state/message/game_state/screen_state/event_id")
        })
        .or_else(|| {
            state
                .raw
                .pointer("/current_state/message/game_state/screen_state/event_name")
        })
        .and_then(Value::as_str)
}

fn event_names_match(expected: &str, live: &str) -> bool {
    normalize_event_name(expected) == normalize_event_name(live)
}

fn normalize_event_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn collect_attempt_json(
    run: &SlayTheDataRunSummary,
    snapshot: Option<&SessionSnapshot>,
    status: &str,
    reason: &str,
    error: Option<LiveError>,
    context: SlayTheDataCollectContext<'_>,
) -> Value {
    let message = error.as_ref().map(ToString::to_string);
    let repair_packet =
        collect_repair_packet(run, snapshot, status, reason, message.as_deref(), context);
    json!({
        "run_id": run.id,
        "seed_played": run.seed_played,
        "session_id": snapshot.map(|snapshot| snapshot.session_id.0.clone()),
        "trace_path": snapshot.map(|snapshot| snapshot.trace_path.clone()),
        "status": status,
        "blocker_kind": repair_packet.blocker_kind.clone(),
        "next_command": repair_packet.reproduce_command.clone(),
        "followup_command": repair_packet.reproduce_command.clone(),
        "reason": reason,
        "floor": snapshot.and_then(current_floor),
        "phase": snapshot
            .and_then(|snapshot| snapshot.latest_state.as_ref())
            .map(|state| format!("{:?}", state.phase)),
        "fidelity": snapshot.map(|snapshot| snapshot.fidelity.clone()),
        "slaythedata_blocked": snapshot.and_then(|snapshot| snapshot.slaythedata.blocked.clone()),
        "automation_blocked": snapshot.and_then(|snapshot| snapshot.automation.blocked.clone()),
        "actions_command": snapshot.map(|snapshot| format!(
            "cargo run -p sts_live --bin live-trace -- actions list {}",
            shell_arg(&snapshot.session_id.0)
        )),
        "resume_command": snapshot.map(|snapshot| format!(
            "cargo run -p sts_live --bin live-trace -- slaythedata resume {} --target-floor {}",
            shell_arg(&snapshot.session_id.0),
            context.target_floor
        )),
        "message": message,
        "repair_packet": repair_packet,
    })
}

fn repair_packet_next_command(packet: &Value) -> Option<&str> {
    packet.get("reproduce_command").and_then(Value::as_str)
}

fn collect_followup_command(result: &Value) -> Option<String> {
    match result.get("blocker_kind").and_then(Value::as_str)? {
        "slaythedata_illegal_log" => {
            if result
                .get("mark_illegal_requires_rebuild")
                .and_then(Value::as_bool)
                == Some(true)
            {
                return Some("cargo test -p sts_live".to_owned());
            }
            if result.get("mark_illegal_source_path").is_some()
                && result.get("mark_illegal_error").is_none()
            {
                return result
                    .get("next_command")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
            }
            let packet_path = result.get("repair_packet_path").and_then(Value::as_str)?;
            Some(format!(
                "cargo run -p sts_live --bin live-trace -- slaythedata mark-illegal {}",
                shell_arg(packet_path)
            ))
        }
        "simulator_fidelity_break" | "slaythedata_mapping_gap" | "bridge_or_backend_error" => {
            result
                .get("next_command")
                .and_then(Value::as_str)
                .map(str::to_owned)
        }
        "completed_trace" | "run_ended_before_target" => None,
        _ => None,
    }
}

fn collect_repair_packet(
    run: &SlayTheDataRunSummary,
    snapshot: Option<&SessionSnapshot>,
    status: &str,
    reason: &str,
    message: Option<&str>,
    context: SlayTheDataCollectContext<'_>,
) -> SlayTheDataRepairPacket {
    let blocker_kind = collect_blocker_kind(status, reason);
    let legal_live_actions = snapshot
        .and_then(|snapshot| snapshot.latest_state.as_ref())
        .map(|state| state.legal_actions.clone())
        .unwrap_or_default();
    let failure_message = collect_failure_message(snapshot, reason, message);
    let illegal_run_constant_entry = (blocker_kind
        == SlayTheDataCollectionBlockerKind::SlaythedataIllegalLog
        && run.id > 0)
        .then(|| illegal_run_constant_entry(run.id, failure_message.as_deref().unwrap_or(reason)));
    SlayTheDataRepairPacket {
        blocker_kind,
        run_id: run.id,
        seed: run.seed_played.clone(),
        session_id: snapshot.map(|snapshot| snapshot.session_id.clone()),
        trace_path: snapshot.map(|snapshot| snapshot.trace_path.clone()),
        current_live_state_summary: snapshot
            .and_then(|snapshot| snapshot.latest_state.as_ref())
            .map(live_state_repair_summary),
        slaythedata_step: snapshot.and_then(|snapshot| snapshot.slaythedata.advisor.clone()),
        legal_live_actions,
        first_simulator_diff_or_mapping_failure: failure_message,
        reproduce_command: collect_reproduce_command(run, snapshot, status, reason, context),
        illegal_run_constant_entry,
    }
}

fn collect_reproduce_command(
    run: &SlayTheDataRunSummary,
    snapshot: Option<&SessionSnapshot>,
    status: &str,
    reason: &str,
    context: SlayTheDataCollectContext<'_>,
) -> Option<String> {
    if status == "simulator_mismatch" {
        return snapshot.map(|snapshot| {
            format!(
                "uv run -- cargo run -p sts_verify --bin sts_verify -- parity {}",
                shell_arg(&snapshot.trace_path)
            )
        });
    }

    if reason == "game_over_before_target" {
        return None;
    }

    if let Some(snapshot) = snapshot {
        if reason == "shop_purchase_unavailable" {
            return Some(format!(
                "cargo run -p sts_live --bin live-trace -- slaythedata skip-shop {}",
                shell_arg(&snapshot.session_id.0)
            ));
        }
        return Some(format!(
            "cargo run -p sts_live --bin live-trace -- actions list {}",
            shell_arg(&snapshot.session_id.0)
        ));
    }

    let mut command = "cargo run -p sts_live --bin live-trace --".to_owned();
    if let Some(db_path) = context.slaythedata_db_path {
        command.push_str(" --slaythedata-db ");
        command.push_str(&shell_arg(&db_path.to_string_lossy()));
    }
    command.push_str(&format!(
        " slaythedata collect --target-floor {} --limit 1",
        context.target_floor
    ));
    if let Some(seed) = run.seed_played.as_deref() {
        command.push_str(" --seed ");
        command.push_str(&shell_arg(seed));
    }
    if run.id > 0 {
        command.push_str(" --run-id ");
        command.push_str(&run.id.to_string());
    }
    if let Some(ascension) = run.ascension_level {
        command.push_str(" --ascension ");
        command.push_str(&ascension.to_string());
    }
    if let Some(source_path) = context.mark_illegal_source_path {
        if source_path == default_slaythedata_source_path() {
            command.push_str(" --mark-illegal");
        } else {
            command.push_str(" --mark-illegal-source ");
            command.push_str(&shell_arg(&source_path.to_string_lossy()));
        }
    }
    command.push_str(" --repair-packet slaythedata-repair.json");
    Some(command)
}

fn shell_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '\\' | '/'))
    {
        value.to_owned()
    } else {
        format!("\"{}\"", value.replace('"', "\\\""))
    }
}

fn collect_blocker_kind(status: &str, reason: &str) -> SlayTheDataCollectionBlockerKind {
    match status {
        "completed_trace" | "target_reached" => SlayTheDataCollectionBlockerKind::CompletedTrace,
        "simulator_mismatch" => SlayTheDataCollectionBlockerKind::SimulatorFidelityBreak,
        "incompatible_run" => SlayTheDataCollectionBlockerKind::SlaythedataIncompatibleRun,
        "slaythedata_hard_break" => SlayTheDataCollectionBlockerKind::SlaythedataIllegalLog,
        "blocked" if reason == "game_over_before_target" => {
            SlayTheDataCollectionBlockerKind::RunEndedBeforeTarget
        }
        "blocked" if reason == "slaythedata_send_failed" => {
            SlayTheDataCollectionBlockerKind::BridgeOrBackendError
        }
        "blocked"
            if reason.starts_with("slaythedata")
                || reason.starts_with("guided")
                || reason.contains("map_symbol")
                || reason.contains("neow_option")
                || reason.contains("shop_purchase")
                || reason.contains("card_reward")
                || reason.contains("reward_rng") =>
        {
            SlayTheDataCollectionBlockerKind::SlaythedataMappingGap
        }
        "blocked" => SlayTheDataCollectionBlockerKind::BridgeOrBackendError,
        _ => SlayTheDataCollectionBlockerKind::BridgeOrBackendError,
    }
}

fn collect_failure_message(
    snapshot: Option<&SessionSnapshot>,
    reason: &str,
    message: Option<&str>,
) -> Option<String> {
    if let Some(snapshot) = snapshot {
        if snapshot.fidelity.kind == FidelityKind::Lost {
            if !snapshot.fidelity.compact_diff.is_empty() {
                return Some(snapshot.fidelity.compact_diff.join("\n"));
            }
            if let Some(message) = snapshot.fidelity.message.as_deref() {
                return Some(message.to_owned());
            }
        }
        if let Some(blocked) = snapshot.slaythedata.blocked.as_ref() {
            return Some(format!("{}: {}", blocked.reason_code, blocked.message));
        }
        if let Some(blocked) = snapshot.automation.blocked.as_ref() {
            return Some(format!("{}: {}", blocked.reason_code, blocked.message));
        }
        if let Some(blocked) = snapshot.blocked.as_ref() {
            return Some(format!("{}: {}", blocked.reason_code, blocked.message));
        }
    }
    message
        .map(str::to_owned)
        .or_else(|| (reason != "ok").then(|| reason.to_owned()))
}

fn live_state_repair_summary(state: &LiveState) -> Value {
    json!({
        "sequence": state.sequence,
        "phase": state.phase,
        "summary": state.raw.get("summary").cloned(),
        "screen_type": state.raw.pointer("/current_state/message/game_state/screen_type").cloned(),
        "room_phase": state.raw.pointer("/current_state/message/game_state/room_phase").cloned(),
        "floor": state.raw
            .pointer("/summary/floor")
            .or_else(|| state.raw.pointer("/current_state/message/game_state/floor"))
            .cloned(),
    })
}

fn current_floor(snapshot: &SessionSnapshot) -> Option<u32> {
    let raw = &snapshot.latest_state.as_ref()?.raw;
    [
        "/summary/floor",
        "/summary/floor_num",
        "/summary/floor_number",
        "/current_state/message/game_state/floor",
        "/sim_run_state/map/floor",
    ]
    .into_iter()
    .filter_map(|path| raw.pointer(path).and_then(|value| value.as_u64()))
    .max()
    .and_then(|floor| u32::try_from(floor).ok())
}

fn parse_automation_policy(value: &str) -> LiveResult<AutomationPolicy> {
    match value {
        "fake-play-first-card" | "fake_play_first_card" => Ok(AutomationPolicy::FakePlayFirstCard),
        "greedy-search" | "greedy_search" | "greedy" => Ok(AutomationPolicy::GreedySearch),
        "beam-search" | "beam_search" | "beam" => Ok(AutomationPolicy::BeamSearch),
        other => Err(LiveError::InvalidAction(format!(
            "unsupported automation policy {other}"
        ))),
    }
}

fn parse_usize(value: &str, flag: &str) -> LiveResult<usize> {
    value
        .parse()
        .map_err(|err| LiveError::InvalidAction(format!("invalid {flag}: {err}")))
}

fn parse_potion_slots(value: &str) -> LiveResult<Vec<usize>> {
    if value.trim().is_empty() || value.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|slot| {
            slot.trim()
                .parse()
                .map_err(|err| LiveError::InvalidAction(format!("invalid potion slot: {err}")))
        })
        .collect()
}

fn required<'a>(args: &'a [String], index: usize, flag: &str) -> LiveResult<&'a str> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| LiveError::InvalidAction(format!("{flag} requires a value")))
}

fn usage() -> String {
    "usage: live-trace [--slaythedata-db PATH] replay TRACE [--bridge ID] [--reset-bridge] [--max-actions N] [--dry-run] [--action-template]; live-trace bridges list|kill [--all|bridge-id]|state BRIDGE|abandon BRIDGE; live-trace sessions list|start|state|request-state|abandon; live-trace actions list SESSION; live-trace actions send SESSION ACTION; live-trace automation status|configure|plan|send-ready|step|run-one|auto-play|pause|resume|cancel SESSION; live-trace slaythedata search [filters]|json RUN_ID|attach SESSION RUN_ID|send-next SESSION|skip-shop SESSION|auto-play SESSION|resume SESSION [--target-floor N] [--journal PATH] [--permanent-root PATH] [--promote-floor N] [--no-promote] [--combat-search-transition-budget N] [--combat-search-time-budget-ms N] [--combat-search-dedup]|agent [collect options]|collect [filters] [--bridge ID] [--target-floor N] [--starting-hp N] [--reset-bridge|--no-reset-bridge] [--journal PATH] [--retry-journaled] [--include-corpus] [--permanent-root PATH] [--promote-floor N] [--no-promote] [--repair-packet PATH] [--combat-search-transition-budget N] [--combat-search-time-budget-ms N] [--combat-search-dedup] [--mark-illegal|--mark-illegal-source PATH]|mark-broken RUN_ID [REASON]|unmark-broken RUN_ID|mark-illegal PACKET_JSON [--source PATH]; live-trace fidelity status SESSION; live-trace trace path|verify SESSION; live-trace trace promote SESSION [--permanent-root PATH] [--min-floor N]".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fidelity::FidelityChecker;
    use crate::model::{
        ActionId, AutomationJobSnapshot, BlockedState, BridgeStatus, FidelityStatus, LegalAction,
        SessionLifecycle, SlayTheDataAdvisorStep, SlayTheDataCollectionBlockerKind,
        SlayTheDataSessionSnapshot,
    };
    use crate::{bridge::FakeBridgeManager, fidelity::TraceFidelityChecker};
    use rusqlite::Connection;
    use std::{fs, time::SystemTime};

    fn test_advisor_event(event_name: &str, player_choice: &str) -> SlayTheDataAdvisorStep {
        SlayTheDataAdvisorStep {
            floor: 0,
            ordinal: 0,
            intent: Some(SlayTheDataReplayStepKind::EventChoice {
                event_name: Some(event_name.to_owned()),
                player_choice: Some(player_choice.to_owned()),
                cards_obtained: Vec::new(),
                cards_removed: Vec::new(),
                cards_transformed: Vec::new(),
                cards_upgraded: Vec::new(),
                relics_obtained: Vec::new(),
                relics_lost: Vec::new(),
            }),
            status: "guided".to_owned(),
            code: "guided_event_choice".to_owned(),
            message: "display-only test message".to_owned(),
            command: None,
            action_id: None,
            action_label: None,
        }
    }

    #[test]
    fn guided_event_choice_reads_typed_advisor_event_name() {
        let advisor = test_advisor_event("Liars Game", "Ignored");

        assert_eq!(advisor_event_name(&advisor), Some("Liars Game"));
    }

    #[test]
    fn event_name_matching_ignores_case_and_spacing_only() {
        assert!(event_names_match("Upgrade Shrine", "upgradeShrine"));
        assert!(!event_names_match("Liars Game", "Big Fish"));
    }

    #[test]
    fn blocked_guided_event_choice_is_slaythedata_hard_break() {
        let mut snapshot = SessionSnapshot {
            session_id: SessionId("session-1".to_owned()),
            bridge_id: BridgeId("bridge-1".to_owned()),
            lifecycle: SessionLifecycle::Blocked,
            trace_path: "trace.jsonl".to_owned(),
            run_config: None,
            latest_state: None,
            fidelity: FidelityStatus {
                kind: FidelityKind::Ok,
                first_divergent_step: None,
                compact_diff: Vec::new(),
                message: None,
            },
            blocked: None,
            automation: AutomationJobSnapshot::default(),
            slaythedata: SlayTheDataSessionSnapshot {
                attached_run: None,
                advisor: Some(SlayTheDataAdvisorStep {
                    floor: 5,
                    ordinal: 18,
                    intent: test_advisor_event("We Meet Again!", "Attack").intent,
                    status: "guided".to_owned(),
                    code: "guided_event_choice".to_owned(),
                    message: "event Some(\"We Meet Again!\") choice Some(\"Attack\") is high-level guidance".to_owned(),
                    command: None,
                    action_id: None,
                    action_label: None,
                }),
                next_step_index: 18,
                blocked: Some(BlockedState {
                    reason_code: "guided_event_choice".to_owned(),
                    message: "next SlayTheData step is guidance-only and has no unique bridge command".to_owned(),
                }),
                last_message: None,
                auto_play_paused: false,
            },
        };

        assert!(slaythedata_hard_break(&snapshot));
        snapshot.slaythedata.advisor = None;
        snapshot.slaythedata.blocked = Some(BlockedState {
            reason_code: "shop_purchase_rng_critical_unavailable".to_owned(),
            message: "Dream Catcher is absent from the live shop".to_owned(),
        });
        assert!(slaythedata_hard_break(&snapshot));
        assert!(slaythedata_confirmed_incompatible(&snapshot));
        snapshot.slaythedata.blocked = Some(BlockedState {
            reason_code: "shop_purchase_unavailable".to_owned(),
            message: "Lantern is absent from the live shop; use Skip shop to continue".to_owned(),
        });
        assert!(!slaythedata_hard_break(&snapshot));
        assert!(!slaythedata_confirmed_incompatible(&snapshot));
        snapshot.latest_state = Some(LiveState {
            sequence: 1,
            phase: LivePhase::Reward,
            legal_actions: Vec::new(),
            raw: json!({}),
        });
        snapshot.slaythedata.blocked = Some(BlockedState {
            reason_code: "pending_card_reward".to_owned(),
            message: "pending card reward target \"Flame Barrier+1\" has no live grid label match"
                .to_owned(),
        });
        assert!(slaythedata_confirmed_incompatible(&snapshot));
        snapshot.latest_state.as_mut().unwrap().phase = LivePhase::Shop;
        snapshot.slaythedata.advisor = Some(SlayTheDataAdvisorStep {
            floor: 4,
            ordinal: 12,
            intent: Some(SlayTheDataReplayStepKind::ShopPurchase {
                item: "Blood for Blood".to_owned(),
                base_item: "Blood for Blood".to_owned(),
            }),
            status: "guided".to_owned(),
            code: "guided_shop_purchase".to_owned(),
            message: "shop purchase \"Blood for Blood\" is high-level guidance".to_owned(),
            command: None,
            action_id: None,
            action_label: None,
        });
        snapshot.slaythedata.blocked = None;
        assert!(slaythedata_cli_needs_shop_block_recheck(&snapshot));
    }

    #[test]
    fn blocked_guided_campfire_is_slaythedata_hard_break() {
        let snapshot = SessionSnapshot {
            session_id: SessionId("session-1".to_owned()),
            bridge_id: BridgeId("bridge-1".to_owned()),
            lifecycle: SessionLifecycle::Blocked,
            trace_path: "trace.jsonl".to_owned(),
            run_config: None,
            latest_state: None,
            fidelity: FidelityStatus {
                kind: FidelityKind::Ok,
                first_divergent_step: None,
                compact_diff: Vec::new(),
                message: None,
            },
            blocked: None,
            automation: AutomationJobSnapshot::default(),
            slaythedata: SlayTheDataSessionSnapshot {
                attached_run: None,
                advisor: Some(SlayTheDataAdvisorStep {
                    floor: 7,
                    ordinal: 28,
                    intent: Some(SlayTheDataReplayStepKind::Campfire {
                        key: Some("SMITH".to_owned()),
                        target_card: None,
                    }),
                    status: "guided".to_owned(),
                    code: "guided_campfire".to_owned(),
                    message:
                        "campfire key Some(\"SMITH\") target Some(\"Inflame\") is high-level guidance"
                            .to_owned(),
                    command: None,
                    action_id: None,
                    action_label: None,
                }),
                next_step_index: 28,
                blocked: Some(BlockedState {
                    reason_code: "guided_campfire".to_owned(),
                    message:
                        "next SlayTheData step is guidance-only and has no unique bridge command"
                            .to_owned(),
                }),
                last_message: None,
                auto_play_paused: false,
            },
        };

        assert!(slaythedata_hard_break(&snapshot));
    }

    #[test]
    fn cli_lists_bridges_and_starts_run() {
        let root = temp_dir("cli-start");
        let mut store = fake_store(&root);
        let bridges = run_cli(&mut store, strings(["bridges", "list"])).unwrap();
        assert_eq!(bridges["bridges"].as_array().unwrap().len(), 1);

        let session = run_cli(
            &mut store,
            strings([
                "sessions",
                "start",
                "--character",
                "ironclad",
                "--ascension",
                "0",
                "--seed",
                "CODEX04",
            ]),
        )
        .unwrap();
        assert_eq!(session["session_id"], "session-1");

        let actions = run_cli(&mut store, strings(["actions", "list", "session-1"])).unwrap();
        assert!(!actions["legal_actions"].as_array().unwrap().is_empty());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn cli_reads_and_abandons_bridge_state_without_a_session() {
        let root = temp_dir("cli-bridge-state");
        let mut store = fake_store(&root);

        let menu = run_cli(&mut store, strings(["bridges", "state", "fake-bridge-1"])).unwrap();
        assert_eq!(menu["phase"], "menu");

        run_cli(
            &mut store,
            strings(["sessions", "start", "--seed", "123", "--ascension", "0"]),
        )
        .unwrap();
        let abandoned =
            run_cli(&mut store, strings(["bridges", "abandon", "fake-bridge-1"])).unwrap();
        assert_eq!(abandoned["phase"], "menu");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn cli_sends_action_with_same_backend_store() {
        let root = temp_dir("cli-send");
        let mut store = fake_store(&root);
        run_cli(
            &mut store,
            strings(["sessions", "start", "--seed", "123", "--ascension", "0"]),
        )
        .unwrap();
        let sent = run_cli(
            &mut store,
            strings(["actions", "send", "session-1", "talk"]),
        )
        .unwrap();
        assert_eq!(sent["latest_state"]["phase"], "combat");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn cli_exposes_skip_shop_recovery_command() {
        let root = temp_dir("cli-skip-shop");
        let mut store = fake_store(&root);

        let error = run_cli(
            &mut store,
            strings(["slaythedata", "skip-shop", "missing-session"]),
        )
        .unwrap_err();

        assert!(matches!(error, LiveError::NotFound(_)));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn cli_can_continue_recovered_trace_session() {
        let root = temp_dir("cli-recover");
        {
            let mut store = fake_store(&root);
            run_cli(
                &mut store,
                strings(["sessions", "start", "--seed", "123", "--ascension", "0"]),
            )
            .unwrap();
        }

        let mut recovered = fake_store(&root);
        recovered.recover_existing_sessions().unwrap();
        let sessions = run_cli(&mut recovered, strings(["sessions", "list"])).unwrap();
        assert_eq!(sessions["sessions"][0]["session_id"], "session-1");

        let actions = run_cli(&mut recovered, strings(["actions", "list", "session-1"])).unwrap();
        assert!(actions["legal_actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|action| action["id"] == "talk"));

        let sent = run_cli(
            &mut recovered,
            strings(["actions", "send", "session-1", "talk"]),
        )
        .unwrap();
        assert_eq!(sent["latest_state"]["phase"], "combat");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn cli_abandons_active_session() {
        let root = temp_dir("cli-abandon");
        let mut store = fake_store(&root);
        run_cli(
            &mut store,
            strings(["sessions", "start", "--seed", "123", "--ascension", "0"]),
        )
        .unwrap();
        let abandoned = run_cli(&mut store, strings(["sessions", "abandon", "session-1"])).unwrap();
        assert_eq!(abandoned["lifecycle"], "ended");
        assert_eq!(abandoned["latest_state"]["phase"], "menu");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn cli_searches_slaythedata_index_with_filters() {
        let root = temp_dir("cli-slaythedata-search");
        fs::create_dir_all(&root).unwrap();
        let db = root.join("slaythedata.sqlite3");
        write_slaythedata_locator_db(&db);
        let mut store = fake_store(&root).with_slaythedata_index(crate::SlayTheDataIndex::new(&db));

        let result = run_cli(
            &mut store,
            strings([
                "slaythedata",
                "search",
                "--ascension",
                "0",
                "--min-floor",
                "20",
                "--victory",
                "--neow-bonus",
                "THREE_CARDS",
                "--limit",
                "5",
            ]),
        )
        .unwrap();

        let runs = result["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0]["id"], 11);
        assert!(!runs.iter().any(|run| run["id"] == 270662));
        assert_eq!(runs[0]["victory"], true);
        assert_eq!(runs[0]["materialized"], false);

        let by_id = run_cli(
            &mut store,
            strings(["slaythedata", "search", "--run-id", "11", "--limit", "5"]),
        )
        .unwrap();
        assert_eq!(by_id["runs"].as_array().unwrap().len(), 1);
        assert_eq!(by_id["runs"][0]["id"], 11);

        let illegal_by_id = run_cli(
            &mut store,
            strings([
                "slaythedata",
                "search",
                "--run-id",
                "270662",
                "--limit",
                "5",
            ]),
        )
        .unwrap();
        assert_eq!(illegal_by_id["runs"].as_array().unwrap().len(), 0);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn cli_marks_and_unmarks_incompatible_slaythedata_runs() {
        let root = temp_dir("cli-slaythedata-mark-broken");
        fs::create_dir_all(&root).unwrap();
        let db = root.join("slaythedata.sqlite3");
        write_slaythedata_locator_db(&db);
        let mut store = fake_store(&root).with_slaythedata_index(crate::SlayTheDataIndex::new(&db));

        let marked = run_cli(
            &mut store,
            strings([
                "slaythedata",
                "mark-broken",
                "11",
                "card",
                "reward",
                "drift",
            ]),
        )
        .unwrap();
        assert_eq!(marked["run_id"], 11);
        assert_eq!(marked["reason"], "card reward drift");

        let hidden = run_cli(
            &mut store,
            strings(["slaythedata", "search", "--run-id", "11"]),
        )
        .unwrap();
        assert!(hidden["runs"].as_array().unwrap().is_empty());

        let unmarked =
            run_cli(&mut store, strings(["slaythedata", "unmark-broken", "11"])).unwrap();
        assert_eq!(unmarked["unmarked"], true);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn cli_exposes_materialized_slaythedata_run_json_for_recovery() {
        let root = temp_dir("cli-slaythedata-json");
        let db = root.join("slaythedata.sqlite3");
        write_slaythedata_materialized_db(&db);
        let mut store = fake_store(&root).with_slaythedata_index(crate::SlayTheDataIndex::new(&db));

        let run = run_cli(&mut store, strings(["slaythedata", "json", "7"])).unwrap();

        assert_eq!(run["seed_played"], "COMPLETE");
        assert_eq!(run["build_version"], "2022-12-18");
        assert_eq!(run["floor_reached"], 20);
        fs::remove_dir_all(root).ok();
    }

    fn fake_store(root: &std::path::Path) -> SessionStore<FakeBridgeManager, TraceFidelityChecker> {
        SessionStore::new(
            FakeBridgeManager::with_default_bridge(),
            TraceFidelityChecker,
            root,
        )
    }

    #[derive(Default)]
    struct AlwaysOkFidelity;

    impl FidelityChecker for AlwaysOkFidelity {
        fn check_trace(&self, _path: &std::path::Path) -> LiveResult<FidelityStatus> {
            Ok(FidelityStatus {
                kind: FidelityKind::Ok,
                first_divergent_step: None,
                compact_diff: Vec::new(),
                message: None,
            })
        }
    }

    #[derive(Default)]
    struct CompletedFloorBridge {
        state: Option<LiveState>,
    }

    impl CompletedFloorBridge {
        fn completed_state(sequence: u64) -> LiveState {
            LiveState {
                sequence,
                phase: LivePhase::Map,
                legal_actions: vec![LegalAction {
                    id: ActionId("request-state".to_owned()),
                    kind: LegalActionKind::RequestState,
                    label: "Request state".to_owned(),
                    enabled: true,
                    command: json!({"kind": "request_state"}),
                    disabled_reason: None,
                }],
                raw: json!({"summary": {"floor": 20}, "screen": "map"}),
            }
        }
    }

    impl BridgeManager for CompletedFloorBridge {
        fn list_bridges(&self) -> LiveResult<Vec<BridgeStatus>> {
            Ok(vec![BridgeStatus {
                id: BridgeId("completed-bridge".to_owned()),
                process_id: Some(2001),
                client_id: Some("completed-test".to_owned()),
                connected: true,
                last_heartbeat_ms: Some(0),
            }])
        }

        fn start_run(
            &mut self,
            _bridge_id: &BridgeId,
            _config: &RunConfig,
        ) -> LiveResult<LiveState> {
            let state = Self::completed_state(1);
            self.state = Some(state.clone());
            Ok(state)
        }

        fn abandon_run(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
            let state = LiveState {
                sequence: 2,
                phase: LivePhase::Menu,
                legal_actions: Vec::new(),
                raw: json!({"screen": "menu"}),
            };
            self.state = Some(state.clone());
            Ok(state)
        }

        fn request_state(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
            let state = self
                .state
                .clone()
                .unwrap_or_else(|| Self::completed_state(1));
            Ok(Self::completed_state(state.sequence + 1))
        }

        fn send_action(
            &mut self,
            _bridge_id: &BridgeId,
            _action: &LegalAction,
        ) -> LiveResult<LiveState> {
            self.request_state(&BridgeId("completed-bridge".to_owned()))
        }

        fn kill_bridge(&mut self, _bridge_id: &BridgeId) -> LiveResult<()> {
            Ok(())
        }

        fn kill_all(&mut self) -> LiveResult<usize> {
            Ok(1)
        }
    }

    fn strings<const N: usize>(items: [&str; N]) -> Vec<String> {
        items.into_iter().map(str::to_owned).collect()
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("sts-live-cli-{name}-{nonce}"))
    }

    fn write_slaythedata_locator_db(path: &std::path::Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE runs (
                id INTEGER PRIMARY KEY,
                character_chosen TEXT,
                ascension_level INTEGER,
                floor_reached INTEGER,
                is_daily INTEGER,
                is_endless INTEGER,
                is_trial INTEGER,
                unsupported_any INTEGER,
                seed_played TEXT,
                build_version TEXT,
                victory INTEGER,
                path_length INTEGER,
                card_choice_count INTEGER,
                event_choice_count INTEGER,
                shop_purchase_count INTEGER,
                potion_usage_count INTEGER,
                neow_bonus TEXT,
                neow_cost TEXT
            );
            CREATE TABLE chunk_runs (run_id INTEGER PRIMARY KEY);
            "#,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (11, 'IRONCLAD', 0, 50, 0, 0, 0, 0, 'WIN', '2020-07-30', 1, 50, 5, 1, 1, 0, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (12, 'IRONCLAD', 0, 19, 0, 0, 0, 0, 'EARLY', '2020-07-30', 1, 19, 5, 1, 1, 0, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (13, 'IRONCLAD', 0, 56, 0, 0, 0, 0, NULL, '2020-07-30', 0, 56, 5, 1, 1, 0, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (270662, 'IRONCLAD', 0, 80, 0, 0, 0, 0, 'ILLEGAL', '2020-07-30', 1, 80, 5, 1, 1, 0, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO chunk_runs VALUES (11)", [])
            .unwrap();
        conn.execute("INSERT INTO chunk_runs VALUES (12)", [])
            .unwrap();
        conn.execute("INSERT INTO chunk_runs VALUES (13)", [])
            .unwrap();
        conn.execute("INSERT INTO chunk_runs VALUES (270662)", [])
            .unwrap();
    }

    fn write_slaythedata_materialized_db(path: &std::path::Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE runs (
                id INTEGER PRIMARY KEY,
                character_chosen TEXT,
                ascension_level INTEGER,
                floor_reached INTEGER,
                is_daily INTEGER,
                is_endless INTEGER,
                is_trial INTEGER,
                unsupported_any INTEGER,
                seed_played TEXT,
                build_version TEXT,
                victory INTEGER,
                path_length INTEGER,
                card_choice_count INTEGER,
                event_choice_count INTEGER,
                shop_purchase_count INTEGER,
                potion_usage_count INTEGER,
                neow_bonus TEXT,
                neow_cost TEXT
            );
            CREATE TABLE chunk_runs (run_id INTEGER PRIMARY KEY);
            CREATE TABLE run_materialized_json (
                run_id INTEGER PRIMARY KEY,
                raw_event_json TEXT NOT NULL
            );
            "#,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (7, 'IRONCLAD', 0, 20, 0, 0, 0, 0, 'COMPLETE', '2020-07-30', 0, 20, 0, 0, 0, 0, 'TEN_PERCENT_HP_BONUS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO chunk_runs VALUES (7)", [])
            .unwrap();
        let raw_run_json = json!({
            "character_chosen": "IRONCLAD",
            "ascension_level": 0,
            "seed_played": "COMPLETE",
            "build_version": "2022-12-18",
            "neow_bonus": "TEN_PERCENT_HP_BONUS",
            "neow_cost": "NONE",
            "path_taken": [],
            "path_per_floor": [],
            "floor_reached": 20,
            "victory": false
        })
        .to_string();
        conn.execute(
            "INSERT INTO run_materialized_json VALUES (7, ?)",
            [&raw_run_json],
        )
        .unwrap();
    }

    #[test]
    fn slaythedata_collect_parser_accepts_reset_bridge() {
        let request = parse_slaythedata_collect_args(&strings([
            "--reset-bridge",
            "--target-floor",
            "17",
            "--starting-hp",
            "10000",
            "--repair-packet",
            "packet.json",
            "--mark-illegal",
            "--retry-journaled",
            "--include-corpus",
            "--permanent-root",
            "/tmp/external-traces",
            "--combat-search-transition-budget",
            "25000",
            "--combat-search-time-budget-ms",
            "12000",
            "--combat-search-dedup",
            "--run-id",
            "11",
            "--ascension",
            "0",
        ]))
        .unwrap();

        assert!(request.reset_bridge);
        assert_eq!(request.target_floor, 17);
        assert_eq!(request.starting_hp, Some(10_000));
        assert_eq!(
            request.repair_packet_path,
            Some(PathBuf::from("packet.json"))
        );
        assert_eq!(
            request.mark_illegal_source_path,
            Some(default_slaythedata_source_path())
        );
        assert_eq!(request.filters.ascension, Some(0));
        assert_eq!(request.filters.run_id, Some(11));
        assert_eq!(request.filters.min_floor_reached, 1);
        assert!(request.retry_journaled);
        assert!(request.include_corpus);
        assert_eq!(request.automation_config.search_transition_budget, 25_000);
        assert_eq!(request.automation_config.search_time_budget_ms, 12_000);
        assert!(request.automation_config.deduplicate_search_states);
        assert!(request.output.promote);
        assert_eq!(
            request.output.permanent_root,
            PathBuf::from("/tmp/external-traces")
        );
        assert_eq!(request.output.promote_floor, 11);
    }

    #[test]
    fn slaythedata_collect_parser_rejects_invalid_starting_hp() {
        let error = parse_slaythedata_collect_args(&strings(["--starting-hp", "1000001"]))
            .err()
            .expect("starting HP above the bridge limit must fail");

        assert!(error
            .to_string()
            .contains("--starting-hp must be between 1 and 1000000"));
    }

    #[test]
    fn slaythedata_resume_parser_preserves_fast_combat_budget() {
        let request = parse_slaythedata_resume_args(
            "session-7",
            &strings([
                "--target-floor",
                "51",
                "--combat-search-transition-budget",
                "20000",
                "--combat-search-time-budget-ms",
                "1000",
                "--combat-search-dedup",
                "--no-promote",
            ]),
        )
        .unwrap();

        assert_eq!(request.session_id, SessionId("session-7".to_owned()));
        assert_eq!(request.target_floor, 51);
        assert_eq!(request.automation_config.search_transition_budget, 20_000);
        assert_eq!(request.automation_config.search_time_budget_ms, 1_000);
        assert!(request.automation_config.deduplicate_search_states);
        assert!(!request.output.promote);
    }

    #[test]
    fn slaythedata_collect_defaults_to_fresh_full_run_without_implicit_promotion() {
        let request =
            parse_slaythedata_collect_args(&strings(["--ascension", "0", "--victory"])).unwrap();

        assert!(request.reset_bridge);
        assert_eq!(request.target_floor, 60);
        assert_eq!(request.filters.min_floor_reached, 1);
        assert_eq!(request.filters.ascension, Some(0));
        assert_eq!(
            request.filters.run_outcome,
            Some(SlayTheDataRunOutcome::Win)
        );
        assert!(!request.retry_journaled);
        assert!(!request.include_corpus);
        assert!(!request.output.promote);
        assert!(request.output.permanent_root.as_os_str().is_empty());
        assert_eq!(request.output.promote_floor, 11);
    }

    #[test]
    fn trace_floor_analysis_reads_communication_mod_game_state() {
        assert_eq!(
            trace_state_floor(&json!({"game_state": {"floor": 44}})),
            Some(44)
        );
        assert_eq!(trace_state_floor(&json!({"floor": 11})), Some(11));
        assert_eq!(trace_state_floor(&json!({"game_state": {}})), None);
    }

    #[test]
    fn collection_journal_appends_machine_readable_jsonl() {
        let root = temp_dir("cli-collection-journal");
        let path = root.join("journal.jsonl");
        append_collection_journal(&path, &json!({"run_id": 7, "status": "blocked"})).unwrap();
        append_collection_journal(&path, &json!({"run_id": 8, "status": "completed_trace"}))
            .unwrap();

        let records = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["run_id"], 7);
        assert_eq!(records[1]["run_id"], 8);
        assert_eq!(
            collection_journal_run_ids(&path).unwrap(),
            HashSet::from([7, 8])
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn journal_exclusion_happens_before_the_requested_limit() {
        let runs = [7, 8, 9]
            .into_iter()
            .map(|id| SlayTheDataRunSummary {
                id,
                ..collector_run_summary()
            })
            .collect();

        let (selected, excluded) = select_unjournaled_runs(runs, &HashSet::from([7]), 1);

        assert_eq!(excluded, 1);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, 8);
    }

    fn assert_collect_terminal_contract(result: &Value) {
        assert!(result.get("status").and_then(Value::as_str).is_some());
        assert!(result.get("blocker_kind").and_then(Value::as_str).is_some());
        assert!(result.get("repair_packet").is_some());
        assert_eq!(
            result["blocker_kind"],
            result["repair_packet"]["blocker_kind"]
        );
        assert!(result
            .get("repair_packet_path")
            .and_then(Value::as_str)
            .is_some());
        assert!(result["repair_packet"]
            .get("first_simulator_diff_or_mapping_failure")
            .is_some());
        assert!(result["repair_packet"].get("legal_live_actions").is_some());
        assert!(result["repair_packet"].get("reproduce_command").is_some());
        match result["blocker_kind"].as_str().unwrap() {
            "completed_trace" | "run_ended_before_target" => {
                assert!(result.get("followup_command").is_none())
            }
            _ => assert!(result
                .get("followup_command")
                .and_then(Value::as_str)
                .is_some()),
        }
    }

    #[test]
    fn slaythedata_collect_classifies_repair_packet_blockers() {
        assert_eq!(
            collect_blocker_kind("completed_trace", "ok"),
            SlayTheDataCollectionBlockerKind::CompletedTrace
        );
        assert_eq!(
            collect_blocker_kind("simulator_mismatch", "fidelity_lost"),
            SlayTheDataCollectionBlockerKind::SimulatorFidelityBreak
        );
        assert_eq!(
            collect_blocker_kind("incompatible_run", "pending_card_reward"),
            SlayTheDataCollectionBlockerKind::SlaythedataIncompatibleRun
        );
        assert_eq!(
            collect_blocker_kind("slaythedata_hard_break", "legal_card_reward"),
            SlayTheDataCollectionBlockerKind::SlaythedataIllegalLog
        );
        assert_eq!(
            collect_blocker_kind("blocked", "guided_event_choice"),
            SlayTheDataCollectionBlockerKind::SlaythedataMappingGap
        );
        assert_eq!(
            collect_blocker_kind("blocked", "map_symbol_unmatched"),
            SlayTheDataCollectionBlockerKind::SlaythedataMappingGap
        );
        assert_eq!(
            collect_blocker_kind("blocked", "shop_purchase_unavailable"),
            SlayTheDataCollectionBlockerKind::SlaythedataMappingGap
        );
        assert_eq!(
            collect_blocker_kind("blocked", "request_state_failed"),
            SlayTheDataCollectionBlockerKind::BridgeOrBackendError
        );
        assert_eq!(
            collect_blocker_kind("blocked", "slaythedata_send_failed"),
            SlayTheDataCollectionBlockerKind::BridgeOrBackendError
        );
        assert_eq!(
            collect_blocker_kind("blocked", "game_over_before_target"),
            SlayTheDataCollectionBlockerKind::RunEndedBeforeTarget
        );
        let packet = collect_repair_packet(
            &collector_run_summary(),
            None,
            "blocked",
            "game_over_before_target",
            None,
            SlayTheDataCollectContext {
                target_floor: 60,
                starting_hp: None,
                slaythedata_db_path: None,
                mark_illegal_source_path: None,
            },
        );
        assert_eq!(
            packet.blocker_kind,
            SlayTheDataCollectionBlockerKind::RunEndedBeforeTarget
        );
        assert!(packet.reproduce_command.is_none());
    }

    #[test]
    fn slaythedata_collect_continues_only_after_terminal_attempts() {
        assert!(collect_should_continue_after_attempt(
            &json!({"status": "completed_trace", "reason": "ok"})
        ));
        assert!(collect_should_continue_after_attempt(
            &json!({"status": "incompatible_run", "reason": "pending_card_reward"})
        ));
        assert!(collect_should_continue_after_attempt(
            &json!({"status": "blocked", "reason": "game_over_before_target"})
        ));
        assert!(collect_should_continue_after_attempt(
            &json!({"status": "slaythedata_hard_break", "reason": "missing_seed"})
        ));
        assert!(!collect_should_continue_after_attempt(
            &json!({"status": "blocked", "reason": "guided_event_choice"})
        ));
        assert!(!collect_should_continue_after_attempt(&json!({
            "status": "completed_trace",
            "reason": "ok",
            "promotion_error": "disk full"
        })));
        assert!(!collect_should_continue_after_attempt(&json!({
            "status": "incompatible_run",
            "reason": "pending_card_reward",
            "mark_broken_error": "database locked"
        })));
    }

    #[test]
    fn slaythedata_collect_attempt_emits_machine_readable_repair_packet() {
        let run = SlayTheDataRunSummary {
            id: 5043653,
            seed_played: Some("1UL6PRJWQ2GWU".to_owned()),
            build_version: None,
            ascension_level: Some(0),
            floor_reached: Some(202),
            victory: true,
            run_outcome: SlayTheDataRunOutcome::Win,
            path_length: Some(202),
            card_choice_count: Some(10),
            event_choice_count: Some(4),
            shop_purchase_count: Some(1),
            potion_usage_count: Some(0),
            neow_bonus: Some("TRANSFORM_CARD".to_owned()),
            neow_cost: Some("NONE".to_owned()),
            guided_score: 18,
            materialized: true,
        };
        let snapshot = SessionSnapshot {
            session_id: SessionId("session-1".to_owned()),
            bridge_id: BridgeId("bridge-1".to_owned()),
            lifecycle: SessionLifecycle::FidelityLost,
            trace_path: "D:\\dev\\slay-the-spire\\live_traces_active\\session-1.jsonl".to_owned(),
            run_config: None,
            latest_state: Some(LiveState {
                sequence: 24,
                phase: LivePhase::Event,
                legal_actions: vec![LegalAction {
                    id: crate::model::ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "Leave".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                }],
                raw: json!({"summary": {"floor": 2, "room_type": "EventRoom"}}),
            }),
            fidelity: FidelityStatus {
                kind: FidelityKind::Lost,
                first_divergent_step: Some(23),
                compact_diff: vec!["choices[2]: \"card3\" != \"limit break\"".to_owned()],
                message: Some("unexpected simulator diff".to_owned()),
            },
            blocked: None,
            automation: AutomationJobSnapshot::default(),
            slaythedata: SlayTheDataSessionSnapshot {
                advisor: Some(SlayTheDataAdvisorStep {
                    floor: 2,
                    ordinal: 7,
                    intent: test_advisor_event("Match and Keep!", "Played").intent,
                    status: "guided".to_owned(),
                    code: "guided_event_choice".to_owned(),
                    message: "event Some(\"Match and Keep!\")".to_owned(),
                    command: None,
                    action_id: None,
                    action_label: None,
                }),
                ..SlayTheDataSessionSnapshot::default()
            },
        };

        let attempt = collect_attempt_json(
            &run,
            Some(&snapshot),
            "simulator_mismatch",
            "fidelity_lost",
            None,
            SlayTheDataCollectContext {
                target_floor: 60,
                starting_hp: None,
                slaythedata_db_path: None,
                mark_illegal_source_path: None,
            },
        );

        assert_eq!(
            attempt["repair_packet"]["blocker_kind"],
            "simulator_fidelity_break"
        );
        assert_eq!(attempt["repair_packet"]["run_id"], 5043653);
        assert_eq!(attempt["blocker_kind"], "simulator_fidelity_break");
        assert_eq!(
            attempt["next_command"],
            attempt["repair_packet"]["reproduce_command"]
        );
        assert_eq!(attempt["followup_command"], attempt["next_command"]);
        assert_eq!(attempt["repair_packet"]["seed"], "1UL6PRJWQ2GWU");
        assert_eq!(
            attempt["repair_packet"]["slaythedata_step"]["code"],
            "guided_event_choice"
        );
        assert_eq!(
            attempt["repair_packet"]["first_simulator_diff_or_mapping_failure"],
            "choices[2]: \"card3\" != \"limit break\""
        );
        assert!(attempt["repair_packet"]["reproduce_command"]
            .as_str()
            .unwrap()
            .contains("sts_verify --bin sts_verify -- parity"));
        assert!(!attempt["repair_packet"]["reproduce_command"]
            .as_str()
            .unwrap()
            .contains("slaythedata collect"));

        let mut shop_snapshot = snapshot.clone();
        shop_snapshot.fidelity = FidelityStatus {
            kind: FidelityKind::Ok,
            first_divergent_step: None,
            compact_diff: Vec::new(),
            message: None,
        };
        let shop_attempt = collect_attempt_json(
            &run,
            Some(&shop_snapshot),
            "blocked",
            "shop_purchase_unavailable",
            None,
            SlayTheDataCollectContext {
                target_floor: 60,
                starting_hp: None,
                slaythedata_db_path: None,
                mark_illegal_source_path: None,
            },
        );
        assert_eq!(
            shop_attempt["repair_packet"]["blocker_kind"],
            "slaythedata_mapping_gap"
        );
        assert!(shop_attempt["repair_packet"]["illegal_run_constant_entry"].is_null());
        assert_eq!(
            shop_attempt["next_command"],
            "cargo run -p sts_live --bin live-trace -- slaythedata skip-shop session-1"
        );
    }

    #[test]
    fn slaythedata_mark_illegal_applies_packet_entry_to_source_constant() {
        let root = temp_dir("cli-slaythedata-mark-illegal");
        fs::create_dir_all(&root).unwrap();
        let packet = root.join("packet.json");
        let source = root.join("slaythedata.rs");
        fs::write(
            &packet,
            serde_json::to_string(&json!({
                "illegal_run_constant_entry": "    13, // SlayTheData missing_seed."
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            &source,
            "pub const ILLEGAL_SLAYTHEDATA_RUN_IDS: &[i64] = &[\n    7, // existing.\n];\n",
        )
        .unwrap();
        let mut store = fake_store(&root);

        let result = run_cli(
            &mut store,
            vec![
                "slaythedata".to_owned(),
                "mark-illegal".to_owned(),
                packet.to_string_lossy().into_owned(),
                "--source".to_owned(),
                source.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();

        assert_eq!(
            result["added_entries"],
            json!(["    13, // SlayTheData missing_seed."])
        );
        let source_text = fs::read_to_string(&source).unwrap();
        assert!(
            source_text.contains("    7, // existing.\n    13, // SlayTheData missing_seed.\n];")
        );

        let second = run_cli(
            &mut store,
            vec![
                "slaythedata".to_owned(),
                "mark-illegal".to_owned(),
                packet.to_string_lossy().into_owned(),
                "--source".to_owned(),
                source.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();
        assert_eq!(second["added_entries"], json!([]));
        assert_eq!(
            second["already_present_entries"],
            json!(["    13, // SlayTheData missing_seed."])
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn slaythedata_mark_illegal_reads_result_entry_array() {
        let root = temp_dir("cli-slaythedata-mark-illegal-result");
        fs::create_dir_all(&root).unwrap();
        let packet = root.join("result.json");
        let source = root.join("slaythedata.rs");
        fs::write(
            &packet,
            serde_json::to_string(&json!({
                "illegal_run_constant_entries": [
                    "    13, // SlayTheData missing_seed.",
                    "    14, // SlayTheData map_symbol_unmatched."
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            &source,
            "pub const ILLEGAL_SLAYTHEDATA_RUN_IDS: &[i64] = &[\n];\n",
        )
        .unwrap();
        let mut store = fake_store(&root);

        let result = run_cli(
            &mut store,
            vec![
                "slaythedata".to_owned(),
                "mark-illegal".to_owned(),
                packet.to_string_lossy().into_owned(),
                "--source".to_owned(),
                source.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();

        assert_eq!(
            result["added_entries"],
            json!([
                "    13, // SlayTheData missing_seed.",
                "    14, // SlayTheData map_symbol_unmatched."
            ])
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn slaythedata_collect_writes_repair_packet_for_illegal_run() {
        let root = temp_dir("cli-slaythedata-collect-repair-packet");
        fs::create_dir_all(&root).unwrap();
        let db = root.join("slaythedata.sqlite3");
        write_slaythedata_locator_db(&db);
        let packet_path = root.join("repair-packet.json");
        let mut store = fake_store(&root).with_slaythedata_index(crate::SlayTheDataIndex::new(&db));
        let result = run_cli(
            &mut store,
            vec![
                "slaythedata".to_owned(),
                "collect".to_owned(),
                "--target-floor".to_owned(),
                "20".to_owned(),
                "--limit".to_owned(),
                "1".to_owned(),
                "--repair-packet".to_owned(),
                packet_path.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();

        assert_collect_terminal_contract(&result);
        assert_eq!(result["status"], "slaythedata_hard_break");
        assert_eq!(
            result["repair_packet_path"],
            packet_path.to_string_lossy().as_ref()
        );
        assert_eq!(
            result["repair_packet"]["blocker_kind"],
            "slaythedata_illegal_log"
        );
        assert_eq!(result["blocker_kind"], "slaythedata_illegal_log");
        assert_eq!(
            result["next_command"],
            result["repair_packet"]["reproduce_command"]
        );
        assert!(result["followup_command"]
            .as_str()
            .unwrap()
            .contains("slaythedata mark-illegal"));
        assert!(result["followup_command"]
            .as_str()
            .unwrap()
            .contains("repair-packet.json"));
        assert_eq!(result["repair_packet"]["run_id"], 13);
        assert_eq!(result["illegal_run_ids_to_add"], json!([13]));
        assert_eq!(
            result["illegal_run_constant_entries"],
            json!(["    13, // SlayTheData missing_seed."])
        );

        let packet: Value =
            serde_json::from_str(&fs::read_to_string(&packet_path).unwrap()).unwrap();
        assert_eq!(packet["blocker_kind"], "slaythedata_illegal_log");
        assert_eq!(packet["run_id"], 13);
        assert_eq!(packet["seed"], Value::Null);
        assert_eq!(
            packet["illegal_run_constant_entry"],
            "    13, // SlayTheData missing_seed."
        );
        assert!(packet["reproduce_command"]
            .as_str()
            .unwrap()
            .contains("live-trace -- --slaythedata-db"));
        assert!(packet["reproduce_command"]
            .as_str()
            .unwrap()
            .contains("slaythedata collect --target-floor 20 --limit 1"));
        assert!(packet["reproduce_command"]
            .as_str()
            .unwrap()
            .contains("--run-id 13"));
        assert_eq!(
            packet["first_simulator_diff_or_mapping_failure"],
            "missing_seed"
        );
        assert_eq!(packet["legal_live_actions"], json!([]));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn slaythedata_collect_can_apply_illegal_entries_to_source() {
        let root = temp_dir("cli-slaythedata-collect-mark-illegal");
        fs::create_dir_all(&root).unwrap();
        let db = root.join("slaythedata.sqlite3");
        let source = root.join("slaythedata.rs");
        write_slaythedata_locator_db(&db);
        fs::write(
            &source,
            "pub const ILLEGAL_SLAYTHEDATA_RUN_IDS: &[i64] = &[\n];\n",
        )
        .unwrap();
        let mut store = fake_store(&root).with_slaythedata_index(crate::SlayTheDataIndex::new(&db));
        let result = run_cli(
            &mut store,
            vec![
                "slaythedata".to_owned(),
                "collect".to_owned(),
                "--target-floor".to_owned(),
                "20".to_owned(),
                "--limit".to_owned(),
                "1".to_owned(),
                "--mark-illegal-source".to_owned(),
                source.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();

        assert_collect_terminal_contract(&result);
        assert_eq!(result["status"], "slaythedata_hard_break");
        assert_eq!(
            result["mark_illegal_added_entries"],
            json!(["    13, // SlayTheData missing_seed."])
        );
        assert_eq!(result["mark_illegal_already_present_entries"], json!([]));
        assert_eq!(result["mark_illegal_requires_rebuild"], json!(true));
        assert_eq!(result["followup_command"], "cargo test -p sts_live");
        assert_eq!(result["post_rebuild_command"], result["next_command"]);
        assert!(result["next_command"]
            .as_str()
            .unwrap()
            .contains("--mark-illegal-source"));
        assert!(result["next_command"]
            .as_str()
            .unwrap()
            .contains(&shell_arg(&source.to_string_lossy())));
        assert!(fs::read_to_string(&source)
            .unwrap()
            .contains("    13, // SlayTheData missing_seed.\n];"));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn slaythedata_collect_reuses_existing_illegal_entries_without_rebuild() {
        let root = temp_dir("cli-slaythedata-collect-mark-illegal-existing");
        fs::create_dir_all(&root).unwrap();
        let db = root.join("slaythedata.sqlite3");
        let source = root.join("slaythedata.rs");
        write_slaythedata_locator_db(&db);
        fs::write(
            &source,
            "pub const ILLEGAL_SLAYTHEDATA_RUN_IDS: &[i64] = &[\n    13, // SlayTheData missing_seed.\n];\n",
        )
        .unwrap();
        let mut store = fake_store(&root).with_slaythedata_index(crate::SlayTheDataIndex::new(&db));
        let result = run_cli(
            &mut store,
            vec![
                "slaythedata".to_owned(),
                "collect".to_owned(),
                "--target-floor".to_owned(),
                "20".to_owned(),
                "--limit".to_owned(),
                "1".to_owned(),
                "--mark-illegal-source".to_owned(),
                source.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();

        assert_collect_terminal_contract(&result);
        assert_eq!(result["status"], "slaythedata_hard_break");
        assert_eq!(result["mark_illegal_added_entries"], json!([]));
        assert_eq!(
            result["mark_illegal_already_present_entries"],
            json!(["    13, // SlayTheData missing_seed."])
        );
        assert_eq!(result["mark_illegal_requires_rebuild"], json!(false));
        assert_eq!(result["followup_command"], result["next_command"]);
        assert!(result.get("post_rebuild_command").is_none());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn slaythedata_collect_writes_repair_packet_for_completed_trace() {
        let root = temp_dir("cli-slaythedata-collect-completed");
        fs::create_dir_all(&root).unwrap();
        let db = root.join("slaythedata.sqlite3");
        write_slaythedata_materialized_db(&db);
        let packet_path = root.join("completed-packet.json");
        let mut store = SessionStore::new(CompletedFloorBridge::default(), AlwaysOkFidelity, &root)
            .with_slaythedata_index(crate::SlayTheDataIndex::new(&db));
        let result = run_cli(
            &mut store,
            vec![
                "slaythedata".to_owned(),
                "collect".to_owned(),
                "--target-floor".to_owned(),
                "20".to_owned(),
                "--run-id".to_owned(),
                "7".to_owned(),
                "--no-reset-bridge".to_owned(),
                "--no-promote".to_owned(),
                "--repair-packet".to_owned(),
                packet_path.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();

        assert_collect_terminal_contract(&result);
        assert_eq!(result["status"], "completed_trace");
        assert_eq!(result["blocker_kind"], "completed_trace");
        assert!(result.get("followup_command").is_none());
        assert_eq!(result["repair_packet"]["run_id"], 7);
        assert_eq!(result["repair_packet"]["seed"], "COMPLETE");
        assert_eq!(
            result["repair_packet"]["current_live_state_summary"]["floor"],
            20
        );
        assert_eq!(
            result["repair_packet"]["first_simulator_diff_or_mapping_failure"],
            Value::Null
        );
        assert_eq!(result["attempts"][0]["status"], "completed_trace");
        let packet: Value =
            serde_json::from_str(&fs::read_to_string(&packet_path).unwrap()).unwrap();
        assert_eq!(packet, result["repair_packet"]);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn slaythedata_collect_writes_repair_packet_when_no_candidate_exists() {
        let root = temp_dir("cli-slaythedata-collect-no-candidates");
        fs::create_dir_all(&root).unwrap();
        let db = root.join("slaythedata.sqlite3");
        write_slaythedata_locator_db(&db);
        let packet_path = root.join("repair-packet.json");
        let mut store = fake_store(&root).with_slaythedata_index(crate::SlayTheDataIndex::new(&db));
        let result = run_cli(
            &mut store,
            vec![
                "slaythedata".to_owned(),
                "collect".to_owned(),
                "--target-floor".to_owned(),
                "20".to_owned(),
                "--seed".to_owned(),
                "NOT_IN_INDEX".to_owned(),
                "--repair-packet".to_owned(),
                packet_path.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();

        assert_collect_terminal_contract(&result);
        assert_eq!(result["status"], "no_candidates");
        assert_eq!(result["reason"], "no_candidates");
        assert_eq!(
            result["repair_packet"]["blocker_kind"],
            "bridge_or_backend_error"
        );
        assert_eq!(result["blocker_kind"], "bridge_or_backend_error");
        assert_eq!(
            result["next_command"],
            result["repair_packet"]["reproduce_command"]
        );
        assert_eq!(result["followup_command"], result["next_command"]);
        assert_eq!(result["repair_packet"]["run_id"], 0);
        assert_eq!(result["illegal_run_constant_entries"], json!([]));
        assert!(result["repair_packet"]["reproduce_command"]
            .as_str()
            .unwrap()
            .contains("live-trace -- --slaythedata-db"));
        assert!(result["repair_packet"]["reproduce_command"]
            .as_str()
            .unwrap()
            .contains("slaythedata collect --target-floor 20 --limit 1"));
        assert_eq!(
            result["repair_packet"]["first_simulator_diff_or_mapping_failure"],
            "no_candidates"
        );

        let packet: Value =
            serde_json::from_str(&fs::read_to_string(&packet_path).unwrap()).unwrap();
        assert_eq!(packet, result["repair_packet"]);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn slaythedata_collect_writes_default_repair_packet_path() {
        let root = temp_dir("cli-slaythedata-collect-default-repair-packet");
        let db = root.join("slaythedata.sqlite3");
        fs::create_dir_all(db.parent().unwrap()).unwrap();
        write_slaythedata_locator_db(&db);
        let mut store = fake_store(&root).with_slaythedata_index(crate::SlayTheDataIndex::new(&db));
        let result = run_cli(
            &mut store,
            vec![
                "slaythedata".to_owned(),
                "collect".to_owned(),
                "--target-floor".to_owned(),
                "20".to_owned(),
                "--seed".to_owned(),
                "NOT_IN_INDEX".to_owned(),
            ],
        )
        .unwrap();

        assert_collect_terminal_contract(&result);
        let packet_path = root.join("slaythedata-repair.json");
        assert_eq!(
            result["repair_packet_path"],
            packet_path.to_string_lossy().as_ref()
        );
        let packet: Value =
            serde_json::from_str(&fs::read_to_string(&packet_path).unwrap()).unwrap();
        assert_eq!(packet["blocker_kind"], "bridge_or_backend_error");
        assert_eq!(packet["illegal_run_constant_entry"], Value::Null);
        assert_eq!(packet, result["repair_packet"]);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn start_command_available_reads_summary_and_protocol_state() {
        assert!(start_command_available(&crate::model::LiveState {
            sequence: 1,
            phase: crate::model::LivePhase::Menu,
            legal_actions: Vec::new(),
            raw: json!({"summary": {"available_commands": ["start", "state"]}}),
        }));
        assert!(start_command_available(&crate::model::LiveState {
            sequence: 1,
            phase: crate::model::LivePhase::Menu,
            legal_actions: Vec::new(),
            raw: json!({"current_state": {"message": {"available_commands": ["state", "start"]}}}),
        }));
        assert!(!start_command_available(&crate::model::LiveState {
            sequence: 1,
            phase: crate::model::LivePhase::Combat,
            legal_actions: Vec::new(),
            raw: json!({"summary": {"available_commands": ["play", "end", "abandon"]}}),
        }));
    }

    #[test]
    fn bridge_reset_retries_transient_not_ready_error() {
        assert!(bridge_reset_error_is_retryable(&LiveError::Bridge(
            "bridge is not ready for a command".to_owned()
        )));
        assert!(bridge_reset_error_is_retryable(&LiveError::Bridge(
            "stale bridge action rejected".to_owned()
        )));
        assert!(!bridge_reset_error_is_retryable(&LiveError::Bridge(
            "CommunicationMod disconnected".to_owned()
        )));
    }
}
