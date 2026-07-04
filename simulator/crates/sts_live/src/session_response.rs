use crate::{
    model::{LiveResult, LiveState, TraceRecord},
    session_state::SessionData,
};
use serde_json::json;

pub(super) fn append_bridge_response(
    session: &mut SessionData,
    command: &str,
    state: &LiveState,
) -> LiveResult<()> {
    session.trace_writer.append(&TraceRecord::Response {
        sequence: state.sequence,
        response: json!({
            "kind": "bridge_command_result",
            "command": command,
            "state_sequence": state.sequence,
            "phase": state.phase,
        }),
    })?;
    Ok(())
}

pub(super) fn append_bridge_response_and_state(
    session: &mut SessionData,
    command: &str,
    state: &LiveState,
) -> LiveResult<()> {
    append_bridge_response(session, command, state)?;
    session.trace_writer.append(&TraceRecord::State {
        sequence: state.sequence,
        state: state.clone(),
    })?;
    Ok(())
}
