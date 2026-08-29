use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value;

const STRICT_PREFIX: &str = include_str!("fixtures/strict_cli_prefix.jsonl");

fn temporary_trace(label: &str, content: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "sts-verify-{label}-{}-{}.jsonl",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    fs::write(&path, content).expect("temporary trace writes");
    path
}

fn strict_prefix_with_final_mismatch() -> String {
    let mut records = STRICT_PREFIX
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("fixture line parses"))
        .collect::<Vec<_>>();
    let final_state = records.last_mut().expect("fixture has a final state");
    final_state["message"]["game_state"]["gold"] = Value::from(100);
    records
        .into_iter()
        .map(|record| serde_json::to_string(&record).expect("fixture record serializes"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[test]
fn replay_cli_emits_json_artifact_for_clean_strict_prefix() {
    let path = temporary_trace("strict-prefix", STRICT_PREFIX);
    let output = Command::new(env!("CARGO_BIN_EXE_sts_verify"))
        .args([
            "replay",
            "--json",
            "--at-step",
            "4",
            path.to_str().expect("trace path is valid UTF-8"),
        ])
        .output()
        .expect("replay CLI starts");
    fs::remove_file(path).ok();

    assert!(output.status.success(), "{output:?}");
    let artifact: Value = serde_json::from_slice(&output.stdout).expect("artifact is JSON");
    assert_eq!(artifact["schema"], 1);
    assert_eq!(artifact["outcome"], "complete");
    assert_eq!(artifact["final_snapshot"]["schema_version"], 8);
    assert_eq!(artifact["selected_checkpoint"]["action_step"], 4);
}

#[test]
fn replay_cli_rejects_trace_without_start_command() {
    let path = temporary_trace("no-start", "{\"type\":\"metadata\",\"schema\":1}\n");
    let output = Command::new(env!("CARGO_BIN_EXE_sts_verify"))
        .args([
            "replay",
            "--json",
            path.to_str().expect("trace path is valid UTF-8"),
        ])
        .output()
        .expect("replay CLI starts");
    fs::remove_file(path).ok();

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(!output.stderr.is_empty(), "{output:?}");
}

#[test]
fn diagnostic_parity_is_explicitly_not_eof_validated() {
    let path = temporary_trace("diagnostic", &strict_prefix_with_final_mismatch());
    let output = Command::new(env!("CARGO_BIN_EXE_sts_verify"))
        .args([
            "parity",
            "--diagnostic-early-exit",
            path.to_str().expect("trace path is valid UTF-8"),
        ])
        .output()
        .expect("parity CLI starts");
    fs::remove_file(path).ok();
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains("eof_validated=false"), "{stdout}");
    assert!(stdout.contains("TailNotValidated"), "{stdout}");
}

#[test]
fn file_and_stdin_parity_and_replay_are_identical() {
    let path = temporary_trace("stdin", STRICT_PREFIX);

    let invoke = |command: &str, extra: &[&str], input_path: &str, stdin_content: Option<&str>| {
        let mut process = Command::new(env!("CARGO_BIN_EXE_sts_verify"));
        process.arg(command).args(extra).arg(input_path);
        if stdin_content.is_some() {
            process.stdin(Stdio::piped());
        }
        let mut child = process
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("CLI starts");
        if let Some(content) = stdin_content {
            child
                .stdin
                .take()
                .expect("stdin is piped")
                .write_all(content.as_bytes())
                .expect("stdin writes");
        }
        child.wait_with_output().expect("CLI completes")
    };

    for (command, extra) in [("parity", &[][..]), ("replay", &["--json"][..])] {
        let file = invoke(command, extra, path.to_str().expect("path is UTF-8"), None);
        let stdin = invoke(command, extra, "-", Some(STRICT_PREFIX));
        assert_eq!(file.status.code(), stdin.status.code(), "{command}");
        assert_eq!(file.stdout, stdin.stdout, "{command}");
    }
    fs::remove_file(path).ok();
}

#[test]
fn status_output_is_identical_for_single_and_multiple_workers() {
    let root = std::env::temp_dir().join(format!("sts-verify-workers-{}", std::process::id()));
    fs::create_dir_all(&root).expect("temporary corpus directory creates");
    fs::write(root.join("a.jsonl"), STRICT_PREFIX).expect("first trace writes");
    fs::write(root.join("b.jsonl"), STRICT_PREFIX).expect("second trace writes");

    let run = |jobs: &str| {
        Command::new(env!("CARGO_BIN_EXE_sts_verify"))
            .env("STS_VERIFY_JOBS", jobs)
            .args([
                "status",
                "--markdown",
                root.to_str().expect("path is UTF-8"),
            ])
            .output()
            .expect("status CLI starts")
    };
    let single = run("1");
    let multiple = run("2");
    fs::remove_dir_all(root).ok();
    assert_eq!(single.status.code(), multiple.status.code());
    assert_eq!(single.stdout, multiple.stdout);
}
