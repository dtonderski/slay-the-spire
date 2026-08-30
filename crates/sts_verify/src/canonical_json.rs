//! Canonical compact JSON and SHA-256 helpers for extractor-owned artifacts.

use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(HEX_DIGITS[(byte >> 4) as usize]);
        encoded.push(HEX_DIGITS[(byte & 0x0f) as usize]);
    }
    encoded
}

pub fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

pub fn canonical_json_bytes(value: &Value) -> Vec<u8> {
    let mut output = Vec::new();
    write_canonical(value, &mut output);
    output
}

const HEX_DIGITS: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

fn write_canonical(value: &Value, output: &mut Vec<u8>) {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(true) => output.extend_from_slice(b"true"),
        Value::Bool(false) => output.extend_from_slice(b"false"),
        Value::Number(number) => write_number(number, output),
        Value::String(text) => write_string(text, output),
        Value::Array(items) => {
            output.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_canonical(item, output);
            }
            output.push(b']');
        }
        Value::Object(fields) => write_object(fields, output),
    }
}

fn write_object(fields: &Map<String, Value>, output: &mut Vec<u8>) {
    let mut keys: Vec<&String> = fields.keys().collect();
    keys.sort_unstable();
    output.push(b'{');
    for (index, key) in keys.into_iter().enumerate() {
        if index > 0 {
            output.push(b',');
        }
        write_string(key, output);
        output.push(b':');
        write_canonical(&fields[key], output);
    }
    output.push(b'}');
}

fn write_number(number: &Number, output: &mut Vec<u8>) {
    output.extend_from_slice(number.to_string().as_bytes());
}

fn write_string(text: &str, output: &mut Vec<u8>) {
    output.push(b'"');
    for character in text.chars() {
        match character {
            '"' => output.extend_from_slice(br#"\""#),
            '\\' => output.extend_from_slice(br#"\\"#),
            '\u{08}' => output.extend_from_slice(br#"\b"#),
            '\u{0c}' => output.extend_from_slice(br#"\f"#),
            '\n' => output.extend_from_slice(br#"\n"#),
            '\r' => output.extend_from_slice(br#"\r"#),
            '\t' => output.extend_from_slice(br#"\t"#),
            character if (character as u32) < 0x20 => {
                let code = character as u32;
                output.extend_from_slice(br#"\u"#);
                output.push(HEX_DIGITS[((code >> 12) & 0xf) as usize] as u8);
                output.push(HEX_DIGITS[((code >> 8) & 0xf) as usize] as u8);
                output.push(HEX_DIGITS[((code >> 4) & 0xf) as usize] as u8);
                output.push(HEX_DIGITS[(code & 0xf) as usize] as u8);
            }
            character => {
                let mut buffer = [0u8; 4];
                output.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    output.push(b'"');
}

#[cfg(test)]
mod tests {
    use super::{canonical_json_bytes, is_sha256_hex, sha256_hex};
    use serde_json::json;

    #[test]
    fn canonical_json_sorts_object_keys_and_compacts() {
        let value = json!({"b": 1, "a": {"d": true, "c": [2, 3]}});
        assert_eq!(
            canonical_json_bytes(&value),
            br#"{"a":{"c":[2,3],"d":true},"b":1}"#
        );
    }

    #[test]
    fn sha256_hex_is_lowercase_and_fixed_width() {
        let digest = sha256_hex(b"abc");
        assert!(is_sha256_hex(&digest));
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
