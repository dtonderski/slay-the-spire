//! Canonical snapshot diff helpers for parity comparison.

use serde_json::Value;

/// Compare two canonical JSON snapshots and return human-readable diff lines.
pub fn canonical_diff(left: &str, right: &str) -> Vec<String> {
    let left = serde_json::from_str::<Value>(left).unwrap_or(Value::Null);
    let right = serde_json::from_str::<Value>(right).unwrap_or(Value::Null);
    let mut diffs = Vec::new();
    diff_values("", &left, &right, &mut diffs);
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
}
