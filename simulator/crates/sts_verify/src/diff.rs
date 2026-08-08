//! Canonical snapshot diff helpers for parity comparison.

use serde_json::Value;

/// Compare two canonical JSON snapshots and return human-readable diff lines.
pub fn canonical_diff(left: &str, right: &str) -> Vec<String> {
    let left = serde_json::from_str::<Value>(left).unwrap_or(Value::Null);
    let right = serde_json::from_str::<Value>(right).unwrap_or(Value::Null);
    canonical_value_diff(&left, &right)
}

/// Compare already typed JSON values without serializing and parsing them
/// again. This preserves the exact canonical path and rendering contract used
/// by [`canonical_diff`].
pub fn canonical_value_diff(left: &Value, right: &Value) -> Vec<String> {
    let mut diffs = Vec::new();
    diff_values("", left, right, &mut diffs);
    diffs
}

fn diff_values(path: &str, left: &Value, right: &Value, diffs: &mut Vec<String>) {
    if left == right {
        return;
    }

    match (left, right) {
        (Value::Object(left_map), Value::Object(right_map)) => {
            let mut keys: Vec<_> = left_map.keys().chain(right_map.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                diff_values(
                    &child_path,
                    left_map.get(key).unwrap_or(&Value::Null),
                    right_map.get(key).unwrap_or(&Value::Null),
                    diffs,
                );
            }
        }
        (Value::Array(left_items), Value::Array(right_items)) => {
            let max_len = left_items.len().max(right_items.len());
            for index in 0..max_len {
                let child_path = format!("{path}[{index}]");
                diff_values(
                    &child_path,
                    left_items.get(index).unwrap_or(&Value::Null),
                    right_items.get(index).unwrap_or(&Value::Null),
                    diffs,
                );
            }
        }
        _ => diffs.push(format!("{path}: {left} != {right}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_diff_reports_field_mismatch() {
        let left = r#"{"player_hp":80,"monsters":[{"hp":41}]}"#;
        let right = r#"{"player_hp":78,"monsters":[{"hp":41}]}"#;
        let diffs = canonical_diff(left, right);
        assert_eq!(diffs, vec!["player_hp: 80 != 78"]);
    }

    #[test]
    fn typed_value_diff_preserves_canonical_paths_order_and_rendering() {
        let left = serde_json::json!({"z": [1, 2], "a": {"x": true}, "missing": null});
        let right = serde_json::json!({"z": [1, 3, 4], "a": {"x": false}});
        let typed = canonical_value_diff(&left, &right);
        let roundtrip = canonical_diff(
            &serde_json::to_string(&left).unwrap(),
            &serde_json::to_string(&right).unwrap(),
        );
        assert_eq!(typed, roundtrip);
        assert_eq!(
            typed,
            vec!["a.x: true != false", "z[1]: 2 != 3", "z[2]: null != 4"]
        );
    }
}
