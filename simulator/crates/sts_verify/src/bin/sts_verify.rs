use std::{
    env, fs,
    fs::File,
    io::{stdin, BufReader},
    path::{Path, PathBuf},
    process::exit,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
    thread,
};

use sts_verify::{
    assess_verification, assess_verification_with_options, canonical_diff, corpus_path,
    import_communication_mod_trace, import_slaythedata_jsonl_line, import_slaythedata_run_json,
    load_corpus_file, minimize_communication_mod_trace, replay_communication_mod_trace_reader,
    slaythedata_replay_plan, slaythedata_replay_preflight, verify_communication_mod_trace,
    verify_communication_mod_trace_diagnostic_reader, verify_communication_mod_trace_reader,
    AssessmentOptions, MinimizeError, SimRealError, SimRealReport, SlayTheDataDiagnosticSeverity,
    VerificationOutcome, REPLAY_ARTIFACT_SCHEMA,
};

fn verify_trace_path(
    path: &str,
    diagnostic_early_exit: bool,
) -> Result<SimRealReport, SimRealError> {
    if path == "-" {
        let input = stdin();
        let reader = input.lock();
        if diagnostic_early_exit {
            verify_communication_mod_trace_diagnostic_reader(reader)
        } else {
            verify_communication_mod_trace_reader(reader)
        }
    } else {
        let reader = BufReader::new(File::open(path)?);
        if diagnostic_early_exit {
            verify_communication_mod_trace_diagnostic_reader(reader)
        } else {
            verify_communication_mod_trace_reader(reader)
        }
    }
}

fn replay_trace_path(
    path: &str,
    requested_step: Option<u32>,
) -> Result<sts_verify::ReplayResult, SimRealError> {
    if path == "-" {
        let input = stdin();
        replay_communication_mod_trace_reader(input.lock(), requested_step)
    } else {
        replay_communication_mod_trace_reader(BufReader::new(File::open(path)?), requested_step)
    }
}

fn main() {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        eprintln!(
            "usage: sts_verify <trace|diff|parity|rng-trace|replay|minimize|status|corpus> ..."
        );
        exit(1);
    };

    match command.as_str() {
        "trace" => {
            let Some(path) = args.next() else {
                eprintln!("usage: sts_verify trace <trace.jsonl>");
                exit(1);
            };
            let content = fs::read_to_string(&path).unwrap_or_else(|err| {
                eprintln!("failed to read {path}: {err}");
                exit(1);
            });
            let trace = import_communication_mod_trace(&content).unwrap_or_else(|err| {
                eprintln!("failed to parse trace: {err}");
                exit(1);
            });
            let states = trace
                .lines
                .iter()
                .filter(|line| matches!(line, sts_verify::TraceLine::State(_)))
                .count();
            let actions = trace
                .lines
                .iter()
                .filter(|line| matches!(line, sts_verify::TraceLine::Action(_)))
                .count();
            println!(
                "source={}",
                trace.metadata.map(|m| m.source).unwrap_or_default()
            );
            println!("states={states} actions={actions}");
        }
        "diff" => {
            let Some(left_path) = args.next() else {
                eprintln!("usage: sts_verify diff <left.json> <right.json>");
                exit(1);
            };
            let Some(right_path) = args.next() else {
                eprintln!("usage: sts_verify diff <left.json> <right.json>");
                exit(1);
            };
            let left = fs::read_to_string(&left_path).unwrap_or_else(|err| {
                eprintln!("failed to read {left_path}: {err}");
                exit(1);
            });
            let right = fs::read_to_string(&right_path).unwrap_or_else(|err| {
                eprintln!("failed to read {right_path}: {err}");
                exit(1);
            });
            let diffs = canonical_diff(&left, &right);
            if diffs.is_empty() {
                println!("no differences");
            } else {
                for line in diffs {
                    println!("{line}");
                }
                exit(2);
            }
        }
        "rng-trace" => {
            let mut from_step = None;
            let mut through_step = None;
            let mut stream = None;
            let mut path = None;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--step" => {
                        let step = parse_rng_trace_step(args.next(), "--step");
                        from_step = Some(step);
                        through_step = Some(step);
                    }
                    "--from-step" => {
                        from_step = Some(parse_rng_trace_step(args.next(), "--from-step"));
                    }
                    "--through-step" => {
                        through_step = Some(parse_rng_trace_step(args.next(), "--through-step"));
                    }
                    "--stream" => {
                        let Some(value) = args.next() else {
                            rng_trace_usage_and_exit("--stream requires a value");
                        };
                        stream = Some(parse_rng_trace_stream(&value).unwrap_or_else(|| {
                            rng_trace_usage_and_exit(&format!("unknown RNG stream: {value}"))
                        }));
                    }
                    other if path.is_none() => path = Some(other.to_owned()),
                    other => {
                        rng_trace_usage_and_exit(&format!("unknown rng-trace argument: {other}"))
                    }
                }
            }
            let Some(path) = path else {
                rng_trace_usage_and_exit("missing trace path");
            };
            if from_step
                .zip(through_step)
                .is_some_and(|(from, through)| from > through)
            {
                rng_trace_usage_and_exit("--from-step must not exceed --through-step");
            }
            let content = fs::read_to_string(&path).unwrap_or_else(|err| {
                eprintln!("failed to read {path}: {err}");
                exit(1);
            });
            let (result, events) =
                sts_core::capture_rng_trace(|| verify_communication_mod_trace(&content));
            let mut emitted = 0usize;
            for event in events {
                if from_step.is_some_and(|step| event.action_step.is_none_or(|value| value < step))
                    || through_step
                        .is_some_and(|step| event.action_step.is_none_or(|value| value > step))
                    || stream.is_some_and(|selected| event.stream != selected)
                {
                    continue;
                }
                println!(
                    "{}",
                    serde_json::to_string(&event).expect("RNG trace event serializes")
                );
                emitted += 1;
            }
            match result {
                Ok(report) => {
                    eprintln!("rng_trace_events={emitted}");
                    eprintln!("unexpected_diffs={}", report.unexpected_diffs.len());
                    eprintln!("unsupported={}", report.unsupported.len());
                }
                Err(err) => {
                    eprintln!("rng_trace_events={emitted}");
                    eprintln!("failed to verify trace: {err}");
                    exit(1);
                }
            }
        }
        "parity" => {
            let mut require_terminal = false;
            let mut diagnostic_early_exit = false;
            let mut path = None;
            for arg in args.by_ref() {
                match arg.as_str() {
                    "--require-terminal" => require_terminal = true,
                    "--diagnostic-early-exit" => diagnostic_early_exit = true,
                    other if path.is_none() => path = Some(other.to_owned()),
                    other => {
                        eprintln!("unknown parity argument: {other}");
                        eprintln!(
                            "usage: sts_verify parity [--require-terminal] [--diagnostic-early-exit] <trace.jsonl>"
                        );
                        exit(1);
                    }
                }
            }
            let Some(path) = path else {
                eprintln!(
                    "usage: sts_verify parity [--require-terminal] [--diagnostic-early-exit] <trace.jsonl>"
                );
                exit(1);
            };
            let options = AssessmentOptions { require_terminal };
            let result = verify_trace_path(&path, diagnostic_early_exit);
            let report = match result {
                Ok(report) => report,
                Err(err) => {
                    let outcome = assess_verification_with_options(Err(&err), None, options);
                    print_verification_outcome(&outcome);
                    eprintln!("failed to verify trace: {err}");
                    exit(verification_outcome_exit_code(&outcome));
                }
            };
            let outcome = assess_verification_with_options(
                Ok(&report),
                report.action_integrity.as_ref(),
                options,
            );
            print_verification_outcome(&outcome);
            println!("total_actions={}", report.total_actions);
            println!("verified={}", report.verified.len());
            println!("unsupported={}", report.unsupported.len());
            println!("unexpected_diffs={}", report.unexpected_diffs.len());
            if let Some(integrity) = &report.action_integrity {
                println!("eof_validated={}", integrity.eof_validated);
                println!("applicable_actions={}", integrity.applicable_actions);
                println!("disposed_actions={}", integrity.disposed_actions);
                println!("target_rejected_actions={}", integrity.rejected_actions);
                println!(
                    "duplicate_dispositions={}",
                    integrity.duplicate_dispositions
                );
                println!(
                    "terminal_state_observed={}",
                    integrity.terminal_state_observed
                );
            }
            if let Some(seed_start) = &report.seed_start {
                println!("seed_start.failed={}", seed_start.failed);
                println!(
                    "seed_start.command=START {} {} {}",
                    seed_start.start_command.character,
                    seed_start.start_command.ascension,
                    seed_start.start_command.external_seed
                );
                println!(
                    "seed_start.numeric_seed={}",
                    seed_start.start_command.numeric_seed
                );
                println!(
                    "seed_start.first_boundary.path={}",
                    seed_start.first_boundary.path
                );
                println!(
                    "seed_start.first_boundary.category={}",
                    seed_start.first_boundary.category
                );
                println!(
                    "seed_start.first_boundary.reason={}",
                    seed_start.first_boundary.reason
                );
            }

            for verified in &report.verified {
                println!(
                    "verified step={} command=\"{}\" label=\"{}\"",
                    verified.action_step, verified.command, verified.label
                );
            }

            for unsupported in &report.unsupported {
                println!(
                    "unsupported step={} command=\"{}\" reason=\"{}\"",
                    unsupported.action_step, unsupported.command, unsupported.reason
                );
            }

            for diff in &report.unexpected_diffs {
                println!(
                    "unexpected_diff step={} command=\"{}\" label=\"{}\"",
                    diff.action_step, diff.command, diff.label
                );
                for line in &diff.diffs {
                    println!("  {line}");
                }
            }
            let exit_code = verification_outcome_exit_code(&outcome);
            if exit_code != 0 {
                exit(exit_code);
            }
        }
        "replay" => {
            let mut json_output = false;
            let mut timeline_output = false;
            let mut requested_step = None;
            let mut output_path = None;
            let mut path = None;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--json" => json_output = true,
                    "--timeline" => timeline_output = true,
                    "--at-step" => {
                        let Some(value) = args.next() else {
                            eprintln!(
                                "usage: sts_verify replay [--json|--timeline] [--at-step N] [-o path] <trace.jsonl>"
                            );
                            exit(1);
                        };
                        requested_step = Some(value.parse().unwrap_or_else(|err| {
                            eprintln!("invalid --at-step {value:?}: {err}");
                            exit(1);
                        }));
                    }
                    "-o" | "--output" => {
                        output_path = Some(args.next().unwrap_or_else(|| {
                            eprintln!(
                                "usage: sts_verify replay [--json|--timeline] [--at-step N] [-o path] <trace.jsonl>"
                            );
                            exit(1);
                        }));
                    }
                    "-" if path.is_none() => path = Some("-".to_owned()),
                    other if other.starts_with('-') => {
                        eprintln!("unknown replay flag: {other}");
                        exit(1);
                    }
                    other => {
                        if path.replace(other.to_owned()).is_some() {
                            eprintln!("replay accepts one trace path");
                            exit(1);
                        }
                    }
                }
            }
            if json_output && timeline_output {
                eprintln!("replay accepts either --json or --timeline, not both");
                exit(1);
            }
            let Some(path) = path else {
                eprintln!(
                    "usage: sts_verify replay [--json|--timeline] [--at-step N] [-o path] <trace.jsonl>"
                );
                exit(1);
            };
            let result = replay_trace_path(&path, requested_step).unwrap_or_else(|err| {
                eprintln!("failed to replay trace: {err}");
                exit(1);
            });
            let outcome = replay_outcome(&result.report);
            let final_snapshot_hash = result
                .final_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.hash().ok())
                .map(|hash| hash.to_string());

            let rendered = if timeline_output {
                result
                    .checkpoints
                    .iter()
                    .map(|checkpoint| {
                        serde_json::to_string(checkpoint).expect("checkpoint serializes")
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n"
            } else if json_output {
                serde_json::to_string_pretty(&serde_json::json!({
                    "schema": REPLAY_ARTIFACT_SCHEMA,
                    "outcome": outcome,
                    "report": &result.report,
                    "checkpoints": &result.checkpoints,
                    "final_snapshot_hash": &final_snapshot_hash,
                    "final_snapshot": &result.final_snapshot,
                    "selected_checkpoint": &result.selected_checkpoint,
                }))
                .expect("replay artifact serializes")
                    + "\n"
            } else {
                let final_hash = final_snapshot_hash.unwrap_or_default();
                let selected = result
                    .selected_checkpoint
                    .as_ref()
                    .map(|checkpoint| checkpoint.action_step.to_string())
                    .unwrap_or_default();
                let boundary = result
                    .report
                    .seed_start
                    .as_ref()
                    .map(|seed_start| {
                        format!(
                            "{}: {}",
                            seed_start.first_boundary.category, seed_start.first_boundary.reason
                        )
                    })
                    .unwrap_or_default();
                format!(
                    "outcome={outcome}\ntotal_actions={}\ncheckpoints={}\nfinal_snapshot_hash={final_hash}\nselected_step={selected}\nboundary={boundary}\n",
                    result.report.total_actions,
                    result.checkpoints.len(),
                )
            };

            if let Some(output_path) = output_path {
                if output_path == "-" {
                    print!("{rendered}");
                } else {
                    fs::write(&output_path, rendered).unwrap_or_else(|err| {
                        eprintln!("failed to write {output_path}: {err}");
                        exit(1);
                    });
                    eprintln!("replay.wrote={output_path}");
                }
            } else {
                print!("{rendered}");
            }
            if outcome != "complete" {
                exit(2);
            }
        }
        "slaythedata-plan" => {
            let mut line_index: Option<usize> = None;
            let mut json_output = false;
            let mut path: Option<String> = None;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--line-index" => {
                        let Some(value) = args.next() else {
                            eprintln!(
                                "usage: sts_verify slaythedata-plan [--line-index N] [--json] <run.json|run.jsonl>"
                            );
                            exit(1);
                        };
                        line_index = Some(value.parse().unwrap_or_else(|err| {
                            eprintln!("invalid --line-index {value:?}: {err}");
                            exit(1);
                        }));
                    }
                    "--json" => json_output = true,
                    other if other.starts_with('-') => {
                        eprintln!("unknown slaythedata-plan flag: {other}");
                        exit(1);
                    }
                    other => {
                        path = Some(other.to_owned());
                        break;
                    }
                }
            }
            let Some(path) = path else {
                eprintln!(
                    "usage: sts_verify slaythedata-plan [--line-index N] [--json] <run.json|run.jsonl>"
                );
                exit(1);
            };
            let content = if path == "-" {
                use std::io::Read;
                let mut buffer = String::new();
                std::io::stdin()
                    .read_to_string(&mut buffer)
                    .expect("read stdin");
                buffer
            } else {
                fs::read_to_string(&path).unwrap_or_else(|err| {
                    eprintln!("failed to read {path}: {err}");
                    exit(1);
                })
            };
            let imported = if let Some(line_index) = line_index {
                import_slaythedata_jsonl_line(&content, line_index)
            } else {
                import_slaythedata_run_json(&content)
            }
            .unwrap_or_else(|err| {
                eprintln!("failed to import SlayTheData run: {err}");
                exit(1);
            });
            let plan = slaythedata_replay_plan(&imported);
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&plan).expect("plan serializes")
                );
            } else {
                println!("schema={}", plan.schema);
                println!("source_kind={:?}", plan.source.kind);
                println!(
                    "run_id={}",
                    plan.source
                        .run_id
                        .map(|id| id.to_string())
                        .unwrap_or_default()
                );
                if let Some(start) = &plan.run_start {
                    println!(
                        "start={} {} {}",
                        start.character, start.ascension, start.seed_played
                    );
                } else {
                    println!("start=");
                }
                println!("ordering={:?}", plan.ordering);
                println!("steps={}", plan.steps.len());
                println!("checkpoints={}", plan.checkpoints.len());
                let errors = plan
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        diagnostic.severity == SlayTheDataDiagnosticSeverity::Error
                    })
                    .count();
                let warnings = plan
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        diagnostic.severity == SlayTheDataDiagnosticSeverity::Warning
                    })
                    .count();
                println!("diagnostics={}", plan.diagnostics.len());
                println!("diagnostic_errors={errors}");
                println!("diagnostic_warnings={warnings}");
                for diagnostic in &plan.diagnostics {
                    println!(
                        "diagnostic severity={:?} code={} path={} message=\"{}\"",
                        diagnostic.severity, diagnostic.code, diagnostic.path, diagnostic.message
                    );
                }
            }
        }
        "slaythedata-preflight" => {
            let mut line_index: Option<usize> = None;
            let mut json_output = false;
            let mut path: Option<String> = None;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--line-index" => {
                        let Some(value) = args.next() else {
                            eprintln!(
                                "usage: sts_verify slaythedata-preflight [--line-index N] [--json] <run.json|run.jsonl>"
                            );
                            exit(1);
                        };
                        line_index = Some(value.parse().unwrap_or_else(|err| {
                            eprintln!("invalid --line-index {value:?}: {err}");
                            exit(1);
                        }));
                    }
                    "--json" => json_output = true,
                    other if other.starts_with('-') => {
                        eprintln!("unknown slaythedata-preflight flag: {other}");
                        exit(1);
                    }
                    other => {
                        path = Some(other.to_owned());
                        break;
                    }
                }
            }
            let Some(path) = path else {
                eprintln!(
                    "usage: sts_verify slaythedata-preflight [--line-index N] [--json] <run.json|run.jsonl>"
                );
                exit(1);
            };
            let content = if path == "-" {
                use std::io::Read;
                let mut buffer = String::new();
                std::io::stdin()
                    .read_to_string(&mut buffer)
                    .expect("read stdin");
                buffer
            } else {
                fs::read_to_string(&path).unwrap_or_else(|err| {
                    eprintln!("failed to read {path}: {err}");
                    exit(1);
                })
            };
            let imported = if let Some(line_index) = line_index {
                import_slaythedata_jsonl_line(&content, line_index)
            } else {
                import_slaythedata_run_json(&content)
            }
            .unwrap_or_else(|err| {
                eprintln!("failed to import SlayTheData run: {err}");
                exit(1);
            });
            let plan = slaythedata_replay_plan(&imported);
            let report = slaythedata_replay_preflight(&plan);
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("preflight serializes")
                );
            } else {
                println!("schema={}", report.schema);
                println!("source_kind={:?}", report.source.kind);
                println!(
                    "numeric_seed={}",
                    report
                        .numeric_seed
                        .map(|seed| seed.to_string())
                        .unwrap_or_default()
                );
                println!("start_phase={}", report.start_phase.unwrap_or_default());
                println!("steps={}", report.steps.len());
                let checked = report
                    .steps
                    .iter()
                    .filter(|step| step.status == sts_verify::SlayTheDataPreflightStatus::Checked)
                    .count();
                let guided = report
                    .steps
                    .iter()
                    .filter(|step| step.status == sts_verify::SlayTheDataPreflightStatus::Guided)
                    .count();
                let blocked = report
                    .steps
                    .iter()
                    .filter(|step| step.status == sts_verify::SlayTheDataPreflightStatus::Blocked)
                    .count();
                println!("checked={checked}");
                println!("guided={guided}");
                println!("blocked={blocked}");
                println!("diagnostics={}", report.diagnostics.len());
                for step in &report.steps {
                    println!(
                        "step floor={} ordinal={} status={:?} code={} message=\"{}\"",
                        step.floor, step.ordinal, step.status, step.code, step.message
                    );
                }
            }
        }
        "minimize" => {
            let mut output_path: Option<String> = None;
            let mut path: Option<String> = None;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "-o" | "--output" => {
                        output_path = Some(args.next().unwrap_or_else(|| {
                            eprintln!("usage: sts_verify minimize [-o path] <trace.jsonl>");
                            exit(1);
                        }));
                    }
                    other if other.starts_with('-') => {
                        eprintln!("unknown minimize flag: {other}");
                        exit(1);
                    }
                    other => {
                        path = Some(other.to_owned());
                        break;
                    }
                }
            }
            let Some(path) = path else {
                eprintln!("usage: sts_verify minimize [-o path] <trace.jsonl>");
                exit(1);
            };
            let content = fs::read_to_string(&path).unwrap_or_else(|err| {
                eprintln!("failed to read {path}: {err}");
                exit(1);
            });
            let report =
                minimize_communication_mod_trace(&content).unwrap_or_else(|err| match err {
                    MinimizeError::NoFailure => {
                        eprintln!("minimize: {err}");
                        exit(0);
                    }
                    MinimizeError::Parse(parse_err) => {
                        eprintln!("failed to minimize trace: {parse_err}");
                        exit(1);
                    }
                });
            eprintln!("minimize.failure_kind={:?}", report.failure_kind);
            eprintln!("minimize.failure_step={}", report.failure_step);
            eprintln!("minimize.failure_command=\"{}\"", report.failure_command);
            eprintln!("minimize.failure_label=\"{}\"", report.failure_label);
            eprintln!(
                "minimize.actions={} (from {})",
                report.minimized_action_count, report.original_action_count
            );
            if let Some(category) = &report.boundary_category {
                eprintln!("minimize.boundary_category={category}");
            }
            if let Some(reason) = &report.boundary_reason {
                eprintln!("minimize.boundary_reason={reason}");
            }
            for line in &report.failure_diffs {
                eprintln!("minimize.diff {line}");
            }
            if let Some(out) = output_path {
                if out == "-" {
                    print!("{}", report.minimized_trace);
                } else {
                    fs::write(&out, &report.minimized_trace).unwrap_or_else(|err| {
                        eprintln!("failed to write {out}: {err}");
                        exit(1);
                    });
                    eprintln!("minimize.wrote={out}");
                }
            } else {
                print!("{}", report.minimized_trace);
            }
        }
        "corpus" => {
            let relative = args
                .next()
                .unwrap_or_else(|| "manual/milestone1.jsonl".to_owned());
            let path = corpus_path(&relative);
            println!("{}", path.display());
            if let Some(content) = load_corpus_file(relative) {
                println!("bytes={}", content.len());
            } else {
                eprintln!("corpus file missing");
                exit(1);
            }
        }
        "status" => {
            let mut markdown = false;
            let mut path: Option<String> = None;
            for arg in args {
                match arg.as_str() {
                    "--markdown" => markdown = true,
                    other if other.starts_with('-') => {
                        eprintln!("unknown status flag: {other}");
                        exit(1);
                    }
                    other => {
                        path = Some(other.to_owned());
                        break;
                    }
                }
            }

            let root = status_path(path.as_deref().unwrap_or("permanent_traces"));
            let entries = trace_status_entries(&root).unwrap_or_else(|err| {
                eprintln!("failed to build status for {}: {err}", root.display());
                exit(1);
            });
            let exit_code = trace_status_exit_code(&entries);
            print_trace_status(&entries, markdown);
            if exit_code != 0 {
                exit(exit_code);
            }
        }
        _ => {
            eprintln!("unknown command: {command}");
            exit(1);
        }
    }
}

#[derive(Debug)]
struct TraceStatusEntry {
    trace: String,
    verified_floor: u32,
    total_actions: usize,
    verified: usize,
    raw_diffs: usize,
    unsupported: usize,
    applicable_actions: usize,
    disposed_actions: usize,
    rejected_actions: usize,
    duplicate_dispositions: usize,
    status: String,
    boundary: String,
    frontier: String,
}

struct StatusTraceInput {
    path: PathBuf,
    trace: String,
}

fn status_path(input: &str) -> PathBuf {
    let path = PathBuf::from(input);
    if path.is_absolute() || path.exists() {
        path
    } else {
        corpus_path(input)
    }
}

fn trace_status_entries(root: &Path) -> Result<Vec<TraceStatusEntry>, String> {
    let inputs = status_trace_inputs(root)?;
    if inputs.is_empty() {
        return Ok(Vec::new());
    }

    let worker_count = status_worker_count(inputs.len());
    eprintln!(
        "status: {} traces across {worker_count} workers",
        inputs.len()
    );
    let next_input = AtomicUsize::new(0);
    let results = Mutex::new((0..inputs.len()).map(|_| None).collect::<Vec<_>>());

    thread::scope(|scope| {
        for _ in 0..worker_count {
            let inputs = &inputs;
            let next_input = &next_input;
            let results = &results;
            scope.spawn(move || loop {
                let index = next_input.fetch_add(1, Ordering::Relaxed);
                let Some(input) = inputs.get(index) else {
                    break;
                };
                let entry = trace_status_entry(input);
                results.lock().expect("status result mutex")[index] = Some(entry);
            });
        }
    });

    Ok(results
        .into_inner()
        .expect("status result mutex")
        .into_iter()
        .map(|entry| entry.expect("status worker produced one result per trace"))
        .collect())
}

/// Parallelism for streaming trace jobs. The default uses at most four
/// workers; explicit overrides remain bounded by CPU count and a hard ceiling.
fn heavy_trace_worker_count(trace_count: usize) -> usize {
    const DEFAULT_CAP: usize = 4;
    const HARD_CAP: usize = 8;
    let cpus = thread::available_parallelism().map_or(1, usize::from);
    let requested = env::var("STS_VERIFY_JOBS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value >= 1)
        .unwrap_or(DEFAULT_CAP);
    bounded_worker_count(trace_count, requested, cpus, HARD_CAP)
}

fn bounded_worker_count(
    trace_count: usize,
    requested: usize,
    cpus: usize,
    hard_cap: usize,
) -> usize {
    let cap = requested.min(cpus).clamp(1, hard_cap);
    trace_count.min(cap).max(1)
}

fn status_worker_count(trace_count: usize) -> usize {
    heavy_trace_worker_count(trace_count)
}

fn trace_status_entry(input: &StatusTraceInput) -> TraceStatusEntry {
    let file = match File::open(&input.path) {
        Ok(file) => file,
        Err(err) => {
            return trace_error_entry(input.trace.clone(), format!("read error: {err}"));
        }
    };
    let report = match verify_communication_mod_trace_reader(BufReader::new(file)) {
        Ok(report) => report,
        Err(err) => {
            return trace_error_entry(input.trace.clone(), format!("parse error: {err}"));
        }
    };
    let outcome = assess_verification(Ok(&report), report.action_integrity.as_ref());
    let integrity = report.action_integrity.unwrap_or_default();
    let boundary = report.seed_start.as_ref().map(|seed_start| {
        format!(
            "{} at {}",
            seed_start.first_boundary.category, seed_start.first_boundary.path
        )
    });
    TraceStatusEntry {
        trace: input.trace.clone(),
        verified_floor: report
            .seed_start
            .as_ref()
            .and_then(|seed_start| seed_start.sim_run_state.as_ref())
            .and_then(|state| state.map.as_ref())
            .map(|map| map.floor)
            .unwrap_or(0),
        total_actions: report.total_actions,
        verified: report.verified.len(),
        raw_diffs: report.unexpected_diffs.len(),
        unsupported: report.unsupported.len(),
        applicable_actions: integrity.applicable_actions,
        disposed_actions: integrity.disposed_actions,
        rejected_actions: integrity.rejected_actions,
        duplicate_dispositions: integrity.duplicate_dispositions,
        status: outcome_status(&outcome).to_owned(),
        boundary: boundary.unwrap_or_else(|| "-".to_owned()),
        frontier: trace_frontier(&report, &outcome),
    }
}

fn status_trace_inputs(root: &Path) -> Result<Vec<StatusTraceInput>, String> {
    let mut paths = fs::read_dir(root)
        .map_err(|err| err.to_string())?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|err| err.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.extension().and_then(|extension| extension.to_str()) == Some("jsonl"));
    paths.sort();
    Ok(paths
        .into_iter()
        .map(|path| StatusTraceInput {
            trace: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("<unknown>")
                .to_owned(),
            path,
        })
        .collect())
}

fn outcome_status(outcome: &VerificationOutcome) -> &'static str {
    match outcome {
        VerificationOutcome::CompletePass => "complete_pass",
        VerificationOutcome::InvalidInput { .. } => "invalid_input",
        VerificationOutcome::Failed { .. } => "failed",
    }
}

fn verification_outcome_exit_code(outcome: &VerificationOutcome) -> i32 {
    match outcome {
        VerificationOutcome::CompletePass => 0,
        VerificationOutcome::InvalidInput { .. } => 1,
        VerificationOutcome::Failed { .. } => 2,
    }
}

fn rng_trace_usage_and_exit(reason: &str) -> ! {
    eprintln!("{reason}");
    eprintln!(
        "usage: sts_verify rng-trace [--step N | --from-step N --through-step N] \\\n         [--stream STREAM] <trace.jsonl>"
    );
    eprintln!(
        "streams: unknown, card_reward, card_random, event, merchant, misc, monster, \\\n         monster_hp, potion, relic, shuffle, treasure"
    );
    exit(1);
}

fn parse_rng_trace_step(value: Option<String>, flag: &str) -> u32 {
    let Some(value) = value else {
        rng_trace_usage_and_exit(&format!("{flag} requires a value"));
    };
    value
        .parse()
        .unwrap_or_else(|_| rng_trace_usage_and_exit(&format!("invalid {flag} value: {value}")))
}

fn parse_rng_trace_stream(value: &str) -> Option<sts_core::RngTraceStream> {
    use sts_core::RngTraceStream;
    match value {
        "unknown" => Some(RngTraceStream::Unknown),
        "card_reward" => Some(RngTraceStream::CardReward),
        "card_random" => Some(RngTraceStream::CardRandom),
        "event" => Some(RngTraceStream::Event),
        "merchant" => Some(RngTraceStream::Merchant),
        "misc" => Some(RngTraceStream::Misc),
        "monster" => Some(RngTraceStream::Monster),
        "monster_hp" => Some(RngTraceStream::MonsterHp),
        "potion" => Some(RngTraceStream::Potion),
        "relic" => Some(RngTraceStream::Relic),
        "shuffle" => Some(RngTraceStream::Shuffle),
        "treasure" => Some(RngTraceStream::Treasure),
        _ => None,
    }
}

fn replay_outcome(report: &sts_verify::SimRealReport) -> &'static str {
    match report.seed_start.as_ref() {
        Some(seed_start) if !seed_start.failed => "complete",
        Some(_) => "boundary",
        None => "boundary",
    }
}

fn print_verification_outcome(outcome: &VerificationOutcome) {
    println!("outcome={}", outcome_status(outcome));
    match outcome {
        VerificationOutcome::InvalidInput { reason } => println!("invalid_input={reason}"),
        VerificationOutcome::Failed { failures } => {
            for failure in failures {
                println!("failure={failure:?}");
            }
        }
        VerificationOutcome::CompletePass => {}
    }
}

fn trace_error_entry(trace: String, error: String) -> TraceStatusEntry {
    TraceStatusEntry {
        trace,
        verified_floor: 0,
        total_actions: 0,
        verified: 0,
        raw_diffs: 0,
        unsupported: 0,
        applicable_actions: 0,
        disposed_actions: 0,
        rejected_actions: 0,
        duplicate_dispositions: 0,
        status: "invalid_input".to_owned(),
        boundary: "-".to_owned(),
        frontier: error,
    }
}

fn trace_status_exit_code(entries: &[TraceStatusEntry]) -> i32 {
    if entries.iter().any(|entry| entry.status == "invalid_input") {
        1
    } else if entries.iter().any(|entry| entry.status == "failed") {
        2
    } else {
        0
    }
}

fn trace_frontier(report: &sts_verify::SimRealReport, outcome: &VerificationOutcome) -> String {
    if let Some(diff) = report.unexpected_diffs.first() {
        let first_line = diff
            .diffs
            .iter()
            .find(|line| line.starts_with("event_id:"))
            .or_else(|| diff.diffs.first())
            .map(String::as_str)
            .unwrap_or("unexpected diff");
        return format!(
            "step {} `{}` {}: {}",
            diff.action_step, diff.command, diff.label, first_line
        );
    }

    if let Some(unsupported) = report.unsupported.first() {
        return format!(
            "step {} `{}` unsupported: {}",
            unsupported.action_step, unsupported.command, unsupported.reason
        );
    }

    if let VerificationOutcome::Failed { failures } = outcome {
        return failures
            .first()
            .map(|failure| format!("typed outcome failure: {failure:?}"))
            .unwrap_or_else(|| "typed outcome failed without a reason".to_owned());
    }

    if let Some(seed_start) = &report.seed_start {
        if seed_start.first_boundary.category == "none" {
            return "all verifiable transitions passed".to_owned();
        }
        return format!(
            "{}: {}",
            seed_start.first_boundary.category, seed_start.first_boundary.reason
        );
    }

    "no seed-start report".to_owned()
}

fn print_trace_status(entries: &[TraceStatusEntry], markdown: bool) {
    let failures = entries
        .iter()
        .filter(|entry| entry.status == "failed")
        .count();
    let passing = entries
        .iter()
        .filter(|entry| entry.status == "complete_pass")
        .count();
    let errors = entries
        .iter()
        .filter(|entry| entry.status == "invalid_input")
        .count();
    let complete_passes = passing;
    let raw_diffs: usize = entries.iter().map(|entry| entry.raw_diffs).sum();
    let unsupported: usize = entries.iter().map(|entry| entry.unsupported).sum();
    let verified: usize = entries.iter().map(|entry| entry.verified).sum();
    let applicable_actions: usize = entries.iter().map(|entry| entry.applicable_actions).sum();
    let disposed_actions: usize = entries.iter().map(|entry| entry.disposed_actions).sum();
    let rejected_actions: usize = entries.iter().map(|entry| entry.rejected_actions).sum();
    let duplicate_dispositions: usize = entries
        .iter()
        .map(|entry| entry.duplicate_dispositions)
        .sum();
    println!("traces={}", entries.len());
    println!("trace_failures={failures}");
    println!("trace_errors={errors}");
    println!("passing_traces={passing}");
    println!("complete_passes={complete_passes}");
    println!("raw_unexpected_diffs={raw_diffs}");
    println!("unsupported_transitions={unsupported}");
    println!("verified_transitions={verified}");
    println!("applicable_actions={applicable_actions}");
    println!("disposed_actions={disposed_actions}");
    println!("target_rejected_actions={rejected_actions}");
    println!("duplicate_dispositions={duplicate_dispositions}");

    if markdown {
        println!();
        println!("| Trace | Floor | Actions | Disposed | Rejected | Verified | Status | Raw diffs | Unsupported | Duplicates | Boundary | Frontier |");
        println!("|---|---:|---:|---:|---:|---:|---|---:|---:|---:|---|---|");
        for entry in entries {
            println!(
                "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | `{}` | {} |",
                escape_markdown_cell(&entry.trace),
                entry.verified_floor,
                entry.total_actions,
                entry.disposed_actions,
                entry.rejected_actions,
                entry.verified,
                entry.status,
                entry.raw_diffs,
                entry.unsupported,
                entry.duplicate_dispositions,
                escape_markdown_cell(&entry.boundary),
                escape_markdown_cell(&entry.frontier)
            );
        }
    } else {
        for entry in entries {
            println!(
                "trace=\"{}\" floor={} actions={} applicable={} disposed={} rejected={} verified={} status={} raw_diffs={} unsupported={} duplicates={} boundary=\"{}\" frontier=\"{}\"",
                entry.trace,
                entry.verified_floor,
                entry.total_actions,
                entry.applicable_actions,
                entry.disposed_actions,
                entry.rejected_actions,
                entry.verified,
                entry.status,
                entry.raw_diffs,
                entry.unsupported,
                entry.duplicate_dispositions,
                entry.boundary,
                entry.frontier
            );
        }
    }
}

fn escape_markdown_cell(value: &str) -> String {
    value.replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_outcomes_have_stable_process_exit_codes() {
        assert_eq!(
            verification_outcome_exit_code(&VerificationOutcome::CompletePass),
            0
        );
        assert_eq!(
            verification_outcome_exit_code(&VerificationOutcome::InvalidInput {
                reason: "bad trace".to_owned(),
            }),
            1
        );
        assert_eq!(
            verification_outcome_exit_code(&VerificationOutcome::Failed {
                failures: vec![sts_verify::VerificationFailure::MissingActionIntegrity],
            }),
            2
        );
    }

    #[test]
    fn status_exit_code_distinguishes_failed_and_invalid_traces() {
        let mut passing = trace_error_entry("pass.jsonl".to_owned(), String::new());
        passing.status = "complete_pass".to_owned();
        assert_eq!(trace_status_exit_code(&[passing]), 0);

        let failed = TraceStatusEntry {
            status: "failed".to_owned(),
            ..trace_error_entry("failed.jsonl".to_owned(), String::new())
        };
        assert_eq!(trace_status_exit_code(&[failed]), 2);

        let invalid = trace_error_entry("invalid.jsonl".to_owned(), "parse error".to_owned());
        assert_eq!(trace_status_exit_code(&[invalid]), 1);
    }

    #[test]
    fn status_worker_count_is_bounded_by_trace_count() {
        assert_eq!(status_worker_count(1), 1);
        assert!(status_worker_count(3) >= 1);
        assert!(status_worker_count(100) <= 8);
        if std::env::var_os("STS_VERIFY_JOBS").is_none() {
            assert!(status_worker_count(100) <= 4);
        }
        assert_eq!(bounded_worker_count(100, 1_000, 64, 8), 8);
        assert_eq!(bounded_worker_count(3, 1_000, 64, 8), 3);
        assert_eq!(bounded_worker_count(100, 1_000, 2, 8), 2);
    }
}
