use std::{env, fs, process::exit};

use sts_verify::{
    canonical_diff, corpus_path, import_communication_mod_trace, load_corpus_file,
    observations_from_trace,
};

fn main() {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        eprintln!("usage: sts_verify <trace|diff|parity> ...");
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
            let observations = observations_from_trace(&content).unwrap_or_else(|err| {
                eprintln!("failed to import trace: {err}");
                exit(1);
            });
            println!("imported_steps={}", observations.len());
            if let Some(step) = observations.iter().find(|step| step.observation.in_combat) {
                println!(
                    "first_combat_step={} player_hp={}",
                    step.step,
                    step.observation
                        .combat
                        .as_ref()
                        .map(|combat| combat.player_hp)
                        .unwrap_or(0)
                );
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
