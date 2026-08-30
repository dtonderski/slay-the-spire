use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sts_verify::canonical_json::{canonical_json_bytes, sha256_hex};

fn temporary_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "sts-real-trace-audit-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    fs::create_dir_all(&path).expect("temp dir");
    path
}

fn schema7_start_trace() -> String {
    let metadata = json!({
        "type": "metadata",
        "schema": 1,
        "source": "communication_mod",
        "boundary_schema": 7,
        "run_config": {"profile": {"note_card": "Strike", "note_upgrades": 0}}
    });
    let action = json!({
        "type": "action",
        "step": 1,
        "command": "START IRONCLAD 0 1",
        "command_meta": {
            "command_id": "start-1",
            "source_command_execution_seq": 0,
            "source_command_settlement_seq": 0
        }
    });
    let state = json!({
        "type": "state",
        "step": 1,
        "message": {
            "boundary_schema": 7,
            "boundary_kind": "quiescent",
            "end_turn_queued": false,
            "game_update_seq": 1,
            "dungeon_update_seq": 1,
            "command_execution_seq": 1,
            "command_settlement_seq": 1,
            "command_response_id": "start-1",
            "command_response_kind": "settled",
            "transaction_pending": false,
            "effects_size": 0,
            "top_level_effects_size": 0,
            "queued_top_level_effects_size": 0,
            "actions_queued": 0,
            "card_queue_size": 0,
            "pre_turn_actions_size": 0,
            "current_action": null,
            "ready_for_command": true,
            "in_game": false
        }
    });
    [metadata, action, state]
        .into_iter()
        .map(|value| serde_json::to_string(&value).expect("record serializes"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn write_challenge(dir: &Path, traces: &[(&str, &str)]) -> PathBuf {
    let mut entries = traces
        .iter()
        .map(|(relative, content)| {
            json!({
                "relative_path": relative,
                "sha256": sha256_hex(content.as_bytes()),
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left["relative_path"]
            .as_str()
            .cmp(&right["relative_path"].as_str())
    });
    let challenge = json!({
        "challenge_version": 1,
        "challenge_id": "real-trace-audit-v1",
        "purpose": "real_trace_audit",
        "source": "communication_mod",
        "boundary_schema": 7,
        "collection_epoch": "schema7",
        "source_artifact_digests": {
            "CommunicationMod": sha256_hex(b"mod"),
            "SuperFastMode": sha256_hex(b"sfm")
        },
        "traces": entries
    });
    let path = dir.join("challenge.json");
    fs::write(&path, canonical_json_bytes(&challenge)).expect("write");
    path
}

fn run_extract(traces: &Path, challenge: &Path, output: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_sts_verify"))
        .args([
            "real-trace-audit",
            "extract",
            "--traces",
            traces.to_str().expect("utf8"),
            "--challenge",
            challenge.to_str().expect("utf8"),
            "--output",
            output.to_str().expect("utf8"),
        ])
        .output()
        .expect("extract starts")
}

#[test]
fn empty_clean_schema7_cohort_writes_failed_nonempty_gate() {
    let root = temporary_dir("empty");
    let traces_dir = root.join("traces");
    let output_dir = root.join("out");
    fs::create_dir_all(&traces_dir).unwrap();
    let content = schema7_start_trace();
    fs::write(traces_dir.join("start.jsonl"), &content).unwrap();
    let challenge = write_challenge(&root, &[("start.jsonl", &content)]);
    let output = run_extract(&traces_dir, &challenge, &output_dir);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("root_count=0"), "{stdout}");
    assert!(stdout.contains("gate.verdict=fail"), "{stdout}");
    let manifest: Value = serde_json::from_slice(
        &fs::read(output_dir.join("real-trace-audit-manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["gate"]["verdict"], "fail");
    assert_eq!(manifest["gate"]["root_count"], 0);
    assert_eq!(manifest["gate"]["all_sources_accounted"], true);
    assert_eq!(manifest["exclusions"].as_array().unwrap().len(), 1);
    fs::remove_dir_all(root).ok();
}

#[test]
fn malformed_challenge_and_digest_mismatch_exit_one() {
    let root = temporary_dir("invalid");
    let traces_dir = root.join("traces");
    let output_dir = root.join("out");
    fs::create_dir_all(&traces_dir).unwrap();
    let content = schema7_start_trace();
    fs::write(traces_dir.join("start.jsonl"), &content).unwrap();
    let challenge_path = root.join("bad.json");
    fs::write(&challenge_path, b"{\"challenge_version\":1}").unwrap();
    let output = run_extract(&traces_dir, &challenge_path, &output_dir);
    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let challenge = write_challenge(&root, &[("start.jsonl", &content)]);
    let mut parsed: Value = serde_json::from_slice(&fs::read(&challenge).unwrap()).unwrap();
    let noncanonical = root.join("noncanonical.json");
    fs::write(&noncanonical, serde_json::to_vec_pretty(&parsed).unwrap()).unwrap();
    let output = run_extract(&traces_dir, &noncanonical, &root.join("out-noncanonical"));
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("canonical compact"), "{stderr}");

    let mut unknown = parsed.clone();
    unknown["traces"][0]["extra"] = json!(true);
    let unknown_path = root.join("unknown-nested.json");
    fs::write(&unknown_path, canonical_json_bytes(&unknown)).unwrap();
    let output = run_extract(&traces_dir, &unknown_path, &root.join("out-unknown"));
    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let mut missing_artifact = parsed.clone();
    missing_artifact["source_artifact_digests"]
        .as_object_mut()
        .unwrap()
        .remove("SuperFastMode");
    let missing_artifact_path = root.join("missing-artifact.json");
    fs::write(
        &missing_artifact_path,
        canonical_json_bytes(&missing_artifact),
    )
    .unwrap();
    let output = run_extract(
        &traces_dir,
        &missing_artifact_path,
        &root.join("out-missing-artifact"),
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");

    parsed["traces"][0]["sha256"] = json!(sha256_hex(b"not-the-file"));
    fs::write(&challenge, canonical_json_bytes(&parsed)).unwrap();
    let output = run_extract(&traces_dir, &challenge, &root.join("out2"));
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("source digest mismatch"), "{stderr}");
    fs::remove_dir_all(root).ok();
}

#[test]
fn nonempty_output_and_path_traversal_exit_one() {
    let root = temporary_dir("refuse");
    let traces_dir = root.join("traces");
    let output_dir = root.join("out");
    fs::create_dir_all(&traces_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();
    fs::write(output_dir.join("already.txt"), b"nope").unwrap();
    let content = schema7_start_trace();
    fs::write(traces_dir.join("start.jsonl"), &content).unwrap();
    let challenge = write_challenge(&root, &[("start.jsonl", &content)]);
    let output = run_extract(&traces_dir, &challenge, &output_dir);
    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let mut parsed: Value = serde_json::from_slice(&fs::read(&challenge).unwrap()).unwrap();
    parsed["traces"][0]["relative_path"] = json!("../outside.jsonl");
    let traversal = root.join("traversal.json");
    fs::write(&traversal, canonical_json_bytes(&parsed)).unwrap();
    let output = run_extract(&traces_dir, &traversal, &root.join("out2"));
    assert_eq!(output.status.code(), Some(1), "{output:?}");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let outside = root.join("outside.jsonl");
        fs::write(&outside, &content).unwrap();
        symlink(&outside, traces_dir.join("link.jsonl")).unwrap();
        let challenge = write_challenge(&root, &[("link.jsonl", &content)]);
        let output = run_extract(&traces_dir, &challenge, &root.join("out-symlink"));
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("outside the traces directory"), "{stderr}");
    }
    fs::remove_dir_all(root).ok();
}

#[test]
fn malformed_tail_is_a_typed_exclusion_and_regeneration_is_stable() {
    let root = temporary_dir("tail");
    let traces_dir = root.join("traces");
    fs::create_dir_all(&traces_dir).unwrap();
    let mut content = schema7_start_trace();
    content.push_str("{not-json\n");
    fs::write(traces_dir.join("broken.jsonl"), &content).unwrap();
    let challenge = write_challenge(&root, &[("broken.jsonl", &content)]);
    let first_out = root.join("out-a");
    let second_out = root.join("out-b");
    let first = run_extract(&traces_dir, &challenge, &first_out);
    let second = run_extract(&traces_dir, &challenge, &second_out);
    assert_eq!(first.status.code(), Some(2), "{first:?}");
    assert_eq!(second.status.code(), Some(2), "{second:?}");
    let first_stdout = String::from_utf8(first.stdout).unwrap();
    let second_stdout = String::from_utf8(second.stdout).unwrap();
    let membership = |stdout: &str| {
        stdout
            .lines()
            .find(|line| line.starts_with("membership_digest="))
            .expect("membership")
            .to_owned()
    };
    assert_eq!(membership(&first_stdout), membership(&second_stdout));
    let manifest: Value = serde_json::from_slice(
        &fs::read(first_out.join("real-trace-audit-manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["exclusions"][0]["reason"], "invalid_input");
    assert_eq!(manifest["gate"]["all_sources_accounted"], true);
    fs::remove_dir_all(root).ok();
}

#[test]
fn extra_files_are_not_globbed_and_schema7_identity_still_fails_closed() {
    let root = temporary_dir("noglob");
    let traces_dir = root.join("traces");
    fs::create_dir_all(&traces_dir).unwrap();
    let selected = schema7_start_trace();
    let ignored = schema7_start_trace().replace("start-1", "ignored-1");
    fs::write(traces_dir.join("selected.jsonl"), &selected).unwrap();
    fs::write(traces_dir.join("ignored.jsonl"), ignored).unwrap();
    let challenge = write_challenge(&root, &[("selected.jsonl", &selected)]);
    let output = run_extract(&traces_dir, &challenge, &root.join("out"));
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("source_trace_count=1"), "{stdout}");

    let mut bad = schema7_start_trace();
    bad = bad.replace(
        "\"command_response_id\":\"start-1\"",
        "\"command_response_id\":\"other\"",
    );
    fs::write(traces_dir.join("bad.jsonl"), &bad).unwrap();
    let challenge = write_challenge(&root, &[("bad.jsonl", &bad)]);
    let output = run_extract(&traces_dir, &challenge, &root.join("out-bad"));
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let manifest: Value = serde_json::from_slice(
        &fs::read(root.join("out-bad/real-trace-audit-manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["exclusions"][0]["reason"], "invalid_input");
    fs::remove_dir_all(root).ok();
}
