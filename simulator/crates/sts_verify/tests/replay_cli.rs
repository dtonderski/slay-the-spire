use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value;
use sts_verify::corpus_path;

#[test]
fn replay_cli_emits_json_artifact_for_clean_strict_prefix() {
    let source =
        corpus_path("open_failures/FIDL01274-p1274-2026-08-07T13-08-53-551Z-2116632.jsonl");
    let content = fs::read_to_string(&source).expect("strict witness is readable");
    let prefix = content
        .lines()
        .take_while(|line| {
            let record: Value = serde_json::from_str(line).expect("trace line parses");
            record
                .get("step")
                .and_then(Value::as_u64)
                .is_none_or(|step| step <= 4)
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let path = std::env::temp_dir().join(format!(
        "sts-verify-strict-prefix-{}-{}.jsonl",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    fs::write(&path, prefix).expect("strict prefix writes");
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
    let path =
        std::env::temp_dir().join(format!("sts-verify-no-start-{}.jsonl", std::process::id()));
    fs::write(&path, "{\"type\":\"metadata\",\"schema\":1}\n").expect("invalid trace writes");
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
    let source =
        corpus_path("open_failures/FIDL01274-p1274-2026-08-07T13-08-53-551Z-2116632.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_sts_verify"))
        .args([
            "parity",
            "--diagnostic-early-exit",
            source.to_str().expect("trace path is valid UTF-8"),
        ])
        .output()
        .expect("parity CLI starts");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains("eof_validated=false"), "{stdout}");
    assert!(stdout.contains("TailNotValidated"), "{stdout}");
}

#[test]
fn file_and_stdin_parity_and_replay_are_identical() {
    let source =
        corpus_path("open_failures/FIDL01274-p1274-2026-08-07T13-08-53-551Z-2116632.jsonl");
    let content = fs::read_to_string(&source).expect("strict witness is readable");
    let prefix = content
        .lines()
        .take_while(|line| {
            let record: Value = serde_json::from_str(line).expect("trace line parses");
            record
                .get("step")
                .and_then(Value::as_u64)
                .is_none_or(|step| step <= 4)
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let path = std::env::temp_dir().join(format!("sts-verify-stdin-{}.jsonl", std::process::id()));
    fs::write(&path, &prefix).expect("strict prefix writes");

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
        let stdin = invoke(command, extra, "-", Some(&prefix));
        assert_eq!(file.status.code(), stdin.status.code(), "{command}");
        assert_eq!(file.stdout, stdin.stdout, "{command}");
    }
    fs::remove_file(path).ok();
}

#[test]
fn status_output_is_identical_for_single_and_multiple_workers() {
    let source =
        corpus_path("open_failures/FIDL01274-p1274-2026-08-07T13-08-53-551Z-2116632.jsonl");
    let content = fs::read_to_string(&source).expect("strict witness is readable");
    let prefix = content
        .lines()
        .take_while(|line| {
            let record: Value = serde_json::from_str(line).expect("trace line parses");
            record
                .get("step")
                .and_then(Value::as_u64)
                .is_none_or(|step| step <= 4)
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let root = std::env::temp_dir().join(format!("sts-verify-workers-{}", std::process::id()));
    fs::create_dir_all(&root).expect("temporary corpus directory creates");
    fs::write(root.join("a.jsonl"), &prefix).expect("first trace writes");
    fs::write(root.join("b.jsonl"), &prefix).expect("second trace writes");

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
