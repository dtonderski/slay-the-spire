use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value;
use sts_verify::corpus_path;

#[test]
fn replay_cli_emits_json_artifact_with_final_snapshot() {
    let trace = corpus_path("permanent_traces/trace-session-8.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_sts_verify"))
        .args([
            "replay",
            "--json",
            "--at-step",
            "3322",
            trace.to_str().expect("trace path is valid UTF-8"),
        ])
        .output()
        .expect("replay CLI starts");

    assert!(output.status.success(), "{output:?}");
    let artifact: Value = serde_json::from_slice(&output.stdout).expect("artifact is JSON");
    assert_eq!(artifact["schema"], 1);
    assert_eq!(artifact["outcome"], "complete");
    assert_eq!(artifact["final_snapshot"]["schema_version"], 8);
    assert_eq!(artifact["selected_checkpoint"]["action_step"], 3322);
}

#[test]
fn replay_cli_rejects_trace_without_start_command() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sts_verify"))
        .args(["replay", "--json", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("replay CLI starts");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(b"{\"type\":\"metadata\",\"schema\":1}\n")
        .expect("trace writes to stdin");
    let output = child.wait_with_output().expect("replay CLI exits");

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(!output.stderr.is_empty(), "{output:?}");
}
