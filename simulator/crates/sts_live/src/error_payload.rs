use crate::model::LiveError;
use serde_json::{json, Value};

pub fn error_payload(error: &LiveError) -> Value {
    json!({
        "error": {
            "kind": error_kind(error),
            "message": error.to_string(),
        }
    })
}

pub fn error_kind(error: &LiveError) -> &'static str {
    match error {
        LiveError::Bridge(_) => "bridge",
        LiveError::InvalidAction(_) => "invalid_action",
        LiveError::Io(_) => "io",
        LiveError::Json(_) => "json",
        LiveError::NotFound(_) => "not_found",
        LiveError::TraceExists(_) => "trace_exists",
        LiveError::Blocked(_) => "blocked",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_payload_is_structured() {
        let value = error_payload(&LiveError::NotFound("session s".to_owned()));

        assert_eq!(value["error"]["kind"], "not_found");
        assert_eq!(value["error"]["message"], "not found: session s");
    }
}
