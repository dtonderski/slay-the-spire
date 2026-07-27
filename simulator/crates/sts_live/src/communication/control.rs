use crate::model::{LiveError, LiveResult, LiveState};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpStream, ToSocketAddrs},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use super::{
    actions::{
        bridge_files_from_protocol_state, live_state_from_files, live_state_from_protocol_state,
    },
    files::BridgeFiles,
};

#[derive(Debug, Clone)]
pub(crate) struct ControlAddress {
    host: String,
    port: u16,
}

pub(crate) fn control_address(status: &Value) -> Option<ControlAddress> {
    let control = status.get("control")?;
    if control.get("protocol").and_then(Value::as_str)? != "tcp-jsonl" {
        return None;
    }
    Some(ControlAddress {
        host: control
            .get("host")
            .and_then(Value::as_str)
            .unwrap_or("127.0.0.1")
            .to_owned(),
        port: control.get("port").and_then(Value::as_u64)? as u16,
    })
}

pub(crate) fn control_is_reachable(control: &ControlAddress, timeout: Duration) -> bool {
    let Ok(mut addresses) = (control.host.as_str(), control.port).to_socket_addrs() else {
        return false;
    };
    addresses.any(|address| TcpStream::connect_timeout(&address, timeout).is_ok())
}

pub(crate) fn control_request(
    control: &ControlAddress,
    payload: &Value,
    timeout: Duration,
) -> LiveResult<Value> {
    let mut stream = TcpStream::connect((control.host.as_str(), control.port))?;
    stream.set_read_timeout(Some(timeout + Duration::from_secs(1)))?;
    stream.set_write_timeout(Some(timeout + Duration::from_secs(1)))?;
    serde_json::to_writer(&mut stream, payload)?;
    stream.write_all(b"\n")?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    if line.trim().is_empty() {
        return Err(LiveError::Bridge(
            "bridge control returned no response".to_owned(),
        ));
    }
    Ok(serde_json::from_str(&line)?)
}

pub(crate) fn request_control_files(
    control: &ControlAddress,
    fallback_status: &Value,
    timeout: Duration,
) -> LiveResult<BridgeFiles> {
    let response = control_request(control, &json!({"type": "state"}), timeout)?;
    if response.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(LiveError::Bridge(
            response
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("bridge control state request rejected")
                .to_owned(),
        ));
    }
    let mut files = bridge_files_from_protocol_state(&response);
    if files.status.is_null() {
        files.status = fallback_status.clone();
    }
    Ok(files)
}

pub(crate) fn send_guarded_command(
    control: &ControlAddress,
    command: &str,
    files: &BridgeFiles,
    stale_after: Duration,
    timeout: Duration,
) -> LiveResult<LiveState> {
    let owner_token = acquire_control(control, stale_after, timeout)?;
    let response = control_request(
        control,
        &json!({
            "type": "command",
            "command_id": format!("sts-live-{}-{}", std::process::id(), now_ms()),
            "command": command,
            "expected_state_id": files.summary.get("state_id"),
            "expected_state_seq": files.summary.get("state_seq"),
            "owner_token": owner_token,
            "wait_for_state_update": true,
            "update_timeout_ms": timeout.as_millis() as u64,
            "metadata": {"source": "sts_live"},
        }),
        timeout,
    );
    let _ = release_control(control, &owner_token, timeout);
    let response = response?;
    let state = observed_response(response, "bridge control command rejected", files)?;
    if state_is_newer_than_command_source(&state, files) {
        return Ok(state);
    }
    wait_for_newer_state(control, files, timeout)
}

pub(crate) fn send_abandon_run(
    control: &ControlAddress,
    files: &BridgeFiles,
    stale_after: Duration,
    timeout: Duration,
) -> LiveResult<LiveState> {
    let owner_token = acquire_control(control, stale_after, timeout)?;
    let response = control_request(
        control,
        &json!({
            "type": "abandon_run",
            "command_id": format!("sts-live-{}-{}", std::process::id(), now_ms()),
            "owner_token": owner_token,
            "wait_for_state_update": true,
            "update_timeout_ms": timeout.as_millis() as u64,
            "metadata": {"source": "sts_live"},
        }),
        timeout,
    );
    let _ = release_control(control, &owner_token, timeout);
    observed_response(response?, "bridge abandon_run rejected", files)
}

fn acquire_control(
    control: &ControlAddress,
    stale_after: Duration,
    timeout: Duration,
) -> LiveResult<String> {
    let owner = control_request(
        control,
        &json!({
            "type": "acquire",
            "owner_id": format!("sts-live-{}", std::process::id()),
            "takeover_if_stale_after_ms": stale_after.as_millis() as u64,
        }),
        timeout,
    )?;
    owner
        .get("owner_token")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| LiveError::Bridge("bridge control did not return owner_token".to_owned()))
}

fn release_control(
    control: &ControlAddress,
    owner_token: &str,
    timeout: Duration,
) -> LiveResult<()> {
    control_request(
        control,
        &json!({"type": "release", "owner_token": owner_token}),
        timeout,
    )
    .map(|_| ())
    .map_err(|err| LiveError::Bridge(format!("failed to release bridge control: {err}")))
}

fn observed_response(
    response: Value,
    rejection_message: &str,
    fallback_files: &BridgeFiles,
) -> LiveResult<LiveState> {
    if response.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(LiveError::Bridge(
            response
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or(rejection_message)
                .to_owned(),
        ));
    }
    if let Some(update) = response.get("observed_update") {
        if update.get("ok").and_then(Value::as_bool) == Some(false) {
            return Err(LiveError::Bridge(
                update
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("timed out waiting for observed state update")
                    .to_owned(),
            ));
        }
    }
    if let Some(protocol_state) = response
        .get("observed_update")
        .and_then(|update| update.get("state"))
        .filter(|state| state.is_object())
    {
        return Ok(live_state_from_protocol_state(protocol_state));
    }
    Ok(live_state_from_files(fallback_files))
}

fn wait_for_newer_state(
    control: &ControlAddress,
    files: &BridgeFiles,
    timeout: Duration,
) -> LiveResult<LiveState> {
    let deadline = Instant::now() + timeout;
    let poll_interval = Duration::from_millis(100);
    loop {
        let fresh_files = request_control_files(control, &files.status, timeout)?;
        let fresh = live_state_from_files(&fresh_files);
        if state_is_newer_than_command_source(&fresh, files) {
            return Ok(fresh);
        }
        if Instant::now() >= deadline {
            return Err(LiveError::Bridge(
                "timed out waiting for a newer observed state".to_owned(),
            ));
        }
        thread::sleep(poll_interval);
    }
}

fn state_is_newer_than_command_source(state: &LiveState, files: &BridgeFiles) -> bool {
    let expected_state_id = files.summary.get("state_id").and_then(Value::as_str);
    let actual_state_id = state
        .raw
        .pointer("/summary/state_id")
        .or_else(|| state.raw.pointer("/current_state/state_id"))
        .and_then(Value::as_str);
    if let (Some(expected), Some(actual)) = (expected_state_id, actual_state_id) {
        return actual != expected;
    }
    if actual_state_id.is_none() && state.sequence == 0 {
        return true;
    }

    let expected_seq = files.summary.get("state_seq").and_then(Value::as_u64);
    if let Some(expected) = expected_seq {
        if state.sequence > 0 {
            return state.sequence > expected;
        }
    }

    true
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
