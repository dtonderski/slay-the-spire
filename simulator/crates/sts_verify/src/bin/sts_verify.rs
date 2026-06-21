use std::{env, fs, process::exit};

use sts_verify::{
    canonical_diff, corpus_path, import_communication_mod_trace, load_corpus_file,
    minimize_communication_mod_trace, verify_communication_mod_trace_with_mode, MinimizeError,
    VerificationMode,
};

fn main() {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        eprintln!("usage: sts_verify <trace|diff|parity|minimize|corpus> ...");
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
            let mut mode = VerificationMode::ObservedState;
            let Some(mut path) = args.next() else {
                eprintln!(
                    "usage: sts_verify parity [--mode observed-state|seed-start] <trace.jsonl>"
                );
                exit(1);
            };
            if path == "--mode" {
                let Some(mode_name) = args.next() else {
                    eprintln!(
                        "usage: sts_verify parity [--mode observed-state|seed-start] <trace.jsonl>"
                    );
                    exit(1);
                };
                mode = match mode_name.as_str() {
                    "observed-state" => VerificationMode::ObservedState,
                    "seed-start" => VerificationMode::SeedStart,
                    _ => {
                        eprintln!("unknown parity mode: {mode_name}");
                        exit(1);
                    }
                };
                let Some(next_path) = args.next() else {
                    eprintln!(
                        "usage: sts_verify parity [--mode observed-state|seed-start] <trace.jsonl>"
                    );
                    exit(1);
                };
                path = next_path;
            }
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
            let report =
                verify_communication_mod_trace_with_mode(&content, mode).unwrap_or_else(|err| {
                    eprintln!("failed to verify trace: {err}");
                    exit(1);
                });
            println!("mode={:?}", report.mode);
            println!("total_actions={}", report.total_actions);
            println!("verified={}", report.verified.len());
            println!("unsupported={}", report.unsupported.len());
            println!("unexpected_diffs={}", report.unexpected_diffs.len());
            if let Some(seed_start) = &report.seed_start {
                println!(
                    "seed_start.expected_failure={}",
                    seed_start.expected_failure
                );
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
                for boundary in &seed_start.rng_boundaries {
                    println!(
                        "rng_boundary stream=\"{}\" status=\"{}\" save_counter=\"{}\" reason=\"{}\"",
                        boundary.stream,
                        boundary.status,
                        boundary.save_counter.as_deref().unwrap_or(""),
                        boundary.reason
                    );
                }
                if let Some(m22) = &seed_start.m22_encounter_report {
                    println!(
                        "seed_start.m22.verified_entries={}",
                        m22.verified_entries.len()
                    );
                    println!(
                        "seed_start.m22.predicted_entries={}",
                        m22.predicted_entries.len()
                    );
                    println!("seed_start.m22.mismatches={}", m22.mismatches.len());
                    for entry in &m22.verified_entries {
                        println!(
                            "m22_verified combat_index={} floor={} step={} encounter=\"{}\" source=\"{}\"",
                            entry.combat_index,
                            entry.floor,
                            entry.action_step,
                            entry.encounter_key,
                            entry.source
                        );
                    }
                    for entry in &m22.predicted_entries {
                        println!(
                            "m22_predicted combat_index={} floor={} encounter=\"{}\" source=\"{}\"",
                            entry.combat_index, entry.floor, entry.encounter_key, entry.source
                        );
                    }
                    for mismatch in &m22.mismatches {
                        println!(
                            "m22_mismatch combat_index={} floor={} step={} message=\"{}\"",
                            mismatch.combat_index,
                            mismatch.floor,
                            mismatch.action_step,
                            mismatch.message
                        );
                    }
                }
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

            if !report.unexpected_diffs.is_empty() {
                for diff in &report.unexpected_diffs {
                    println!(
                        "unexpected_diff step={} command=\"{}\" label=\"{}\"",
                        diff.action_step, diff.command, diff.label
                    );
                    for line in &diff.diffs {
                        println!("  {line}");
                    }
                }
                exit(2);
            }
        }
        "minimize" => {
            let mut mode = VerificationMode::SeedStart;
            let mut output_path: Option<String> = None;
            let mut path: Option<String> = None;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--mode" => {
                        let Some(mode_name) = args.next() else {
                            eprintln!(
                                "usage: sts_verify minimize [--mode observed-state|seed-start] [-o path] <trace.jsonl>"
                            );
                            exit(1);
                        };
                        mode = match mode_name.as_str() {
                            "observed-state" => VerificationMode::ObservedState,
                            "seed-start" => VerificationMode::SeedStart,
                            _ => {
                                eprintln!("unknown minimize mode: {mode_name}");
                                exit(1);
                            }
                        };
                    }
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
                eprintln!(
                    "usage: sts_verify minimize [--mode observed-state|seed-start] [-o path] <trace.jsonl>"
                );
                exit(1);
            };
            let content = fs::read_to_string(&path).unwrap_or_else(|err| {
                eprintln!("failed to read {path}: {err}");
                exit(1);
            });
            let report =
                minimize_communication_mod_trace(&content, mode).unwrap_or_else(|err| match err {
                    MinimizeError::NoFailure => {
                        eprintln!("minimize: {err}");
                        exit(0);
                    }
                    MinimizeError::Parse(parse_err) => {
                        eprintln!("failed to minimize trace: {parse_err}");
                        exit(1);
                    }
                });
            eprintln!("minimize.mode={:?}", report.mode);
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
        _ => {
            eprintln!("unknown command: {command}");
            exit(1);
        }
    }
}
