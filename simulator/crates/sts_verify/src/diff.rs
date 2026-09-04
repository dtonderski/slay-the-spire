//! Canonical snapshot diff helpers for parity comparison.

use serde_json::Value;

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
