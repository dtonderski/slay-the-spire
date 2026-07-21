use std::{
    env, fs,
    path::{Path, PathBuf},
    process::exit,
};

use sts_verify::{
    assess_verification, canonical_diff, corpus_path, import_communication_mod_trace,
    import_slaythedata_jsonl_line, import_slaythedata_run_json, load_corpus_file,
    minimize_communication_mod_trace, slaythedata_replay_plan, slaythedata_replay_preflight,
    verify_communication_mod_trace, MinimizeError, SlayTheDataDiagnosticSeverity,
    VerificationCorpusManifest, VerificationExpectation, VerificationOutcome,
    VERIFICATION_CORPUS_MANIFEST_SCHEMA,
};

fn main() {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        eprintln!("usage: sts_verify <trace|diff|parity|minimize|status|corpus> ...");
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
        "parity" => {
            let Some(path) = args.next() else {
                eprintln!("usage: sts_verify parity <trace.jsonl>");
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
            let expectation = VerificationExpectation::Complete;
            let result = verify_communication_mod_trace(&content);
            let report = match result {
                Ok(report) => report,
                Err(err) => {
                    let outcome = assess_verification(Err(&err), &expectation, None);
                    print_verification_outcome(&outcome);
                    eprintln!("failed to verify trace: {err}");
                    exit(verification_outcome_exit_code(&outcome));
                }
            };
            let outcome =
                assess_verification(Ok(&report), &expectation, report.action_integrity.as_ref());
            print_verification_outcome(&outcome);
            println!("total_actions={}", report.total_actions);
            println!("ignored_tail_actions={}", report.ignored_tail_actions);
            println!("verified={}", report.verified.len());
            println!("unsupported={}", report.unsupported.len());
            println!("unexpected_diffs={}", report.unexpected_diffs.len());
            if let Some(integrity) = &report.action_integrity {
                println!("applicable_actions={}", integrity.applicable_actions);
                println!("disposed_actions={}", integrity.disposed_actions);
                println!("target_rejected_actions={}", integrity.rejected_actions);
                println!(
                    "duplicate_dispositions={}",
                    integrity.duplicate_dispositions
                );
                println!(
                    "unresolved_transient_assertions={}",
                    integrity.unresolved_transient_assertions
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
    expectation: String,
    verified_floor: u32,
    total_actions: usize,
    verified: usize,
    raw_diffs: usize,
    unsupported: usize,
    ignored_tail: usize,
    applicable_actions: usize,
    disposed_actions: usize,
    rejected_actions: usize,
    duplicate_dispositions: usize,
    unresolved_transient_assertions: usize,
    status: String,
    boundary: String,
    frontier: String,
}

struct StatusTraceInput {
    path: PathBuf,
    trace: String,
    expectation: VerificationExpectation,
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
    let mut entries = Vec::new();
    for input in inputs {
        let expectation = expectation_label(&input.expectation);
        let content = match fs::read_to_string(&input.path) {
            Ok(content) => content,
            Err(err) => {
                entries.push(trace_error_entry(
                    input.trace,
                    expectation,
                    format!("read error: {err}"),
                ));
                continue;
            }
        };
        let report = match verify_communication_mod_trace(&content) {
            Ok(report) => report,
            Err(err) => {
                entries.push(trace_error_entry(
                    input.trace,
                    expectation,
                    format!("parse error: {err}"),
                ));
                continue;
            }
        };
        let outcome = assess_verification(
            Ok(&report),
            &input.expectation,
            report.action_integrity.as_ref(),
        );
        let integrity = report.action_integrity.unwrap_or_default();
        let boundary = report.seed_start.as_ref().map(|seed_start| {
            format!(
                "{} at {}",
                seed_start.first_boundary.category, seed_start.first_boundary.path
            )
        });
        entries.push(TraceStatusEntry {
            trace: input.trace,
            expectation,
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
            ignored_tail: report.ignored_tail_actions,
            applicable_actions: integrity.applicable_actions,
            disposed_actions: integrity.disposed_actions,
            rejected_actions: integrity.rejected_actions,
            duplicate_dispositions: integrity.duplicate_dispositions,
            unresolved_transient_assertions: integrity.unresolved_transient_assertions,
            status: outcome_status(&outcome).to_owned(),
            boundary: boundary.unwrap_or_else(|| "-".to_owned()),
            frontier: trace_frontier(&report, &outcome),
        });
    }

    Ok(entries)
}

fn status_trace_inputs(root: &Path) -> Result<Vec<StatusTraceInput>, String> {
    let manifest_path = root.with_extension("json");
    if manifest_path.exists() {
        let content = fs::read_to_string(&manifest_path).map_err(|err| err.to_string())?;
        let manifest: VerificationCorpusManifest =
            serde_json::from_str(&content).map_err(|err| err.to_string())?;
        if manifest.schema != VERIFICATION_CORPUS_MANIFEST_SCHEMA {
            return Err(format!(
                "unsupported verification corpus manifest schema {} in {}",
                manifest.schema,
                manifest_path.display()
            ));
        }
        let mut actual = fs::read_dir(root)
            .map_err(|err| err.to_string())?
            .map(|entry| {
                entry
                    .map(|entry| entry.path())
                    .map_err(|err| err.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        actual.retain(|path| {
            path.extension().and_then(|extension| extension.to_str()) == Some("jsonl")
        });
        let mut actual_names = actual
            .iter()
            .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        actual_names.sort();
        let mut declared_names = manifest
            .entries
            .iter()
            .map(|entry| entry.trace.clone())
            .collect::<Vec<_>>();
        declared_names.sort();
        if declared_names != actual_names {
            return Err(format!(
                "manifest {} does not exactly match trace directory {}",
                manifest_path.display(),
                root.display()
            ));
        }
        return manifest
            .entries
            .into_iter()
            .map(|entry| {
                let path = Path::new(&entry.trace);
                if path.file_name().and_then(|name| name.to_str()) != Some(entry.trace.as_str()) {
                    return Err(format!(
                        "manifest trace must be a filename, found {:?}",
                        entry.trace
                    ));
                }
                Ok(StatusTraceInput {
                    path: root.join(&entry.trace),
                    trace: entry.trace,
                    expectation: entry.expectation,
                })
            })
            .collect();
    }

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
            expectation: VerificationExpectation::Complete,
        })
        .collect())
}

fn expectation_label(expectation: &VerificationExpectation) -> String {
    match expectation {
        VerificationExpectation::Complete => "complete".to_owned(),
        VerificationExpectation::RetainedPrefix { endpoint } => format!(
            "retained_prefix through step {} ({})",
            endpoint.action_step, endpoint.label
        ),
        VerificationExpectation::ExpectedBoundary { boundary } => {
            format!(
                "expected_boundary {} at {}",
                boundary.category, boundary.path
            )
        }
    }
}

fn outcome_status(outcome: &VerificationOutcome) -> &'static str {
    match outcome {
        VerificationOutcome::CompletePass => "complete_pass",
        VerificationOutcome::RetainedPrefixPass { .. } => "retained_prefix_pass",
        VerificationOutcome::ExpectedBoundary { .. } => "expected_boundary",
        VerificationOutcome::InvalidInput { .. } => "invalid_input",
        VerificationOutcome::Failed { .. } => "failed",
    }
}

fn verification_outcome_exit_code(outcome: &VerificationOutcome) -> i32 {
    match outcome {
        VerificationOutcome::CompletePass
        | VerificationOutcome::RetainedPrefixPass { .. }
        | VerificationOutcome::ExpectedBoundary { .. } => 0,
        VerificationOutcome::InvalidInput { .. } => 1,
        VerificationOutcome::Failed { .. } => 2,
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
        VerificationOutcome::CompletePass
        | VerificationOutcome::RetainedPrefixPass { .. }
        | VerificationOutcome::ExpectedBoundary { .. } => {}
    }
}

fn trace_error_entry(trace: String, expectation: String, error: String) -> TraceStatusEntry {
    TraceStatusEntry {
        trace,
        expectation,
        verified_floor: 0,
        total_actions: 0,
        verified: 0,
        raw_diffs: 0,
        unsupported: 0,
        ignored_tail: 0,
        applicable_actions: 0,
        disposed_actions: 0,
        rejected_actions: 0,
        duplicate_dispositions: 0,
        unresolved_transient_assertions: 0,
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
        .filter(|entry| {
            matches!(
                entry.status.as_str(),
                "complete_pass" | "retained_prefix_pass" | "expected_boundary"
            )
        })
        .count();
    let errors = entries
        .iter()
        .filter(|entry| entry.status == "invalid_input")
        .count();
    let complete_passes = entries
        .iter()
        .filter(|entry| entry.status == "complete_pass")
        .count();
    let prefix_passes = entries
        .iter()
        .filter(|entry| entry.status == "retained_prefix_pass")
        .count();
    let expected_boundaries = entries
        .iter()
        .filter(|entry| entry.status == "expected_boundary")
        .count();
    let raw_diffs: usize = entries.iter().map(|entry| entry.raw_diffs).sum();
    let unsupported: usize = entries.iter().map(|entry| entry.unsupported).sum();
    let verified: usize = entries.iter().map(|entry| entry.verified).sum();
    let ignored_tail: usize = entries.iter().map(|entry| entry.ignored_tail).sum();
    let applicable_actions: usize = entries.iter().map(|entry| entry.applicable_actions).sum();
    let disposed_actions: usize = entries.iter().map(|entry| entry.disposed_actions).sum();
    let rejected_actions: usize = entries.iter().map(|entry| entry.rejected_actions).sum();
    let duplicate_dispositions: usize = entries
        .iter()
        .map(|entry| entry.duplicate_dispositions)
        .sum();
    let unresolved_transient_assertions: usize = entries
        .iter()
        .map(|entry| entry.unresolved_transient_assertions)
        .sum();
    println!("traces={}", entries.len());
    println!("trace_failures={failures}");
    println!("trace_errors={errors}");
    println!("passing_traces={passing}");
    println!("complete_passes={complete_passes}");
    println!("retained_prefix_passes={prefix_passes}");
    println!("expected_boundaries={expected_boundaries}");
    println!("raw_unexpected_diffs={raw_diffs}");
    println!("unsupported_transitions={unsupported}");
    println!("verified_transitions={verified}");
    println!("ignored_tail_actions={ignored_tail}");
    println!("applicable_actions={applicable_actions}");
    println!("disposed_actions={disposed_actions}");
    println!("target_rejected_actions={rejected_actions}");
    println!("duplicate_dispositions={duplicate_dispositions}");
    println!("unresolved_transient_assertions={unresolved_transient_assertions}");

    if markdown {
        println!();
        println!("| Trace | Expectation | Floor | Actions | Disposed | Rejected | Verified | Status | Raw diffs | Unsupported | Ignored tail | Duplicates | Unresolved transient | Boundary | Frontier |");
        println!("|---|---|---:|---:|---:|---:|---:|---|---:|---:|---:|---:|---:|---|---|");
        for entry in entries {
            println!(
                "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | `{}` | {} |",
                escape_markdown_cell(&entry.trace),
                escape_markdown_cell(&entry.expectation),
                entry.verified_floor,
                entry.total_actions,
                entry.disposed_actions,
                entry.rejected_actions,
                entry.verified,
                entry.status,
                entry.raw_diffs,
                entry.unsupported,
                entry.ignored_tail,
                entry.duplicate_dispositions,
                entry.unresolved_transient_assertions,
                escape_markdown_cell(&entry.boundary),
                escape_markdown_cell(&entry.frontier)
            );
        }
    } else {
        for entry in entries {
            println!(
                "trace=\"{}\" expectation=\"{}\" floor={} actions={} applicable={} disposed={} rejected={} verified={} status={} raw_diffs={} unsupported={} ignored_tail={} duplicates={} unresolved_transient={} boundary=\"{}\" frontier=\"{}\"",
                entry.trace,
                entry.expectation,
                entry.verified_floor,
                entry.total_actions,
                entry.applicable_actions,
                entry.disposed_actions,
                entry.rejected_actions,
                entry.verified,
                entry.status,
                entry.raw_diffs,
                entry.unsupported,
                entry.ignored_tail,
                entry.duplicate_dispositions,
                entry.unresolved_transient_assertions,
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
            verification_outcome_exit_code(&VerificationOutcome::RetainedPrefixPass {
                endpoint: sts_verify::RetainedPrefixEndpoint {
                    action_step: 12,
                    label: "retained endpoint".to_owned(),
                },
            }),
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
        let passing = trace_error_entry(
            "pass.jsonl".to_owned(),
            "complete".to_owned(),
            String::new(),
        );
        let mut passing = passing;
        passing.status = "complete_pass".to_owned();
        assert_eq!(trace_status_exit_code(&[passing]), 0);

        let failed = TraceStatusEntry {
            status: "failed".to_owned(),
            ..trace_error_entry(
                "failed.jsonl".to_owned(),
                "complete".to_owned(),
                String::new(),
            )
        };
        assert_eq!(trace_status_exit_code(&[failed]), 2);

        let invalid = trace_error_entry(
            "invalid.jsonl".to_owned(),
            "complete".to_owned(),
            "parse error".to_owned(),
        );
        assert_eq!(trace_status_exit_code(&[invalid]), 1);
    }
}
