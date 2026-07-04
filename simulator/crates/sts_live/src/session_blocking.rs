use crate::{
    model::{BlockedState, LiveResult, SessionLifecycle, TraceRecord},
    session_state::SessionData,
};

pub(super) fn record_blocked(
    session: &mut SessionData,
    reason_code: &str,
    message: &str,
) -> LiveResult<()> {
    session.lifecycle = SessionLifecycle::Blocked;
    session.blocked = Some(BlockedState {
        reason_code: reason_code.to_owned(),
        message: message.to_owned(),
    });
    append_error(session, reason_code, message)
}

fn append_error(session: &mut SessionData, reason_code: &str, message: &str) -> LiveResult<()> {
    session.trace_writer.append(&TraceRecord::Error {
        sequence: session
            .latest_state
            .as_ref()
            .map(|state| state.sequence)
            .unwrap_or_default(),
        reason_code: reason_code.to_owned(),
        message: message.to_owned(),
    })?;
    Ok(())
}
