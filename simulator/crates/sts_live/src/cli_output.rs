use crate::{
    error_payload::error_payload,
    model::{LiveError, LiveResult},
};
use serde_json::Value;

pub fn format_cli_success(value: &Value) -> LiveResult<String> {
    Ok(serde_json::to_string(value)?)
}

pub fn format_cli_error(error: &LiveError) -> String {
    error_payload(error).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn success_output_is_compact_json() {
        let output = format_cli_success(&json!({"answer": 42, "items": [1, 2]})).unwrap();
        assert_eq!(output, r#"{"answer":42,"items":[1,2]}"#);
    }

    #[test]
    fn error_output_is_structured_json() {
        let output = format_cli_error(&LiveError::InvalidAction("bad flag".to_owned()));
        let value: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(value["error"]["kind"], "invalid_action");
        assert_eq!(value["error"]["message"], "invalid action: bad flag");
    }
}
