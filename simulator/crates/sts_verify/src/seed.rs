//! Target-version seed string conversion helpers.

pub const STS_SEED_ALPHABET: &str = "0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedParseError {
    invalid: char,
}

impl std::fmt::Display for SeedParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid Slay the Spire seed character: {}",
            self.invalid
        )
    }
}

impl std::error::Error for SeedParseError {}

/// Convert a Slay the Spire user-facing seed string to the numeric seed used by
/// `SeedHelper.getLong` in the 12-18-2022 desktop jar.
pub fn try_sts_seed_string_to_long(seed: &str) -> Result<i64, SeedParseError> {
    let mut value = 0_i64;
    for ch in seed.to_uppercase().replace('O', "0").chars() {
        let digit = STS_SEED_ALPHABET
            .find(ch)
            .ok_or(SeedParseError { invalid: ch })? as i64;
        value = value
            .wrapping_mul(STS_SEED_ALPHABET.len() as i64)
            .wrapping_add(digit);
    }
    Ok(value)
}

/// Convert a Slay the Spire user-facing seed string to the numeric seed used by
/// `SeedHelper.getLong` in the 12-18-2022 desktop jar.
pub fn sts_seed_string_to_long(seed: &str) -> i64 {
    try_sts_seed_string_to_long(seed).unwrap_or_else(|error| panic!("{error}"))
}

/// Convert the numeric seed back to the user-facing seed text used by
/// `SeedHelper.getString(long)`. The target game first treats the signed Java
/// long as unsigned, then writes it in the same base-35 alphabet.
pub fn sts_seed_long_to_string(seed: i64) -> String {
    let mut value = seed as u64 as u128;
    if value == 0 {
        return String::new();
    }
    let alphabet = STS_SEED_ALPHABET.as_bytes();
    let radix = alphabet.len() as u128;
    let mut out = Vec::new();
    while value != 0 {
        let digit = (value % radix) as usize;
        value /= radix;
        out.push(alphabet[digit] as char);
    }
    out.iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_seed_strings_match_target_game_seedhelper() {
        assert_eq!(sts_seed_string_to_long("VERIFY01"), 1_957_307_888_551);
        assert_eq!(sts_seed_string_to_long("CODEX03"), 22_079_335_078);
        assert_eq!(sts_seed_string_to_long("CODEX04"), 22_079_335_079);
    }

    #[test]
    fn seed_conversion_is_case_insensitive_and_maps_o_to_zero() {
        assert_eq!(sts_seed_string_to_long("codex04"), 22_079_335_079);
        assert_eq!(sts_seed_string_to_long("O"), 0);
        assert_eq!(sts_seed_string_to_long("10"), 35);
    }

    #[test]
    fn fallible_seed_conversion_reports_invalid_characters() {
        assert_eq!(
            try_sts_seed_string_to_long("-123").unwrap_err().to_string(),
            "invalid Slay the Spire seed character: -"
        );
    }

    #[test]
    fn seed_long_to_string_matches_target_unsigned_encoding() {
        assert_eq!(sts_seed_long_to_string(1_957_307_888_551), "VERIFY01");
        assert_eq!(
            sts_seed_long_to_string(-3_574_229_841_928_219_368),
            "4E1F1EYL4U1M3"
        );
        assert_eq!(
            sts_seed_string_to_long(&sts_seed_long_to_string(-3_574_229_841_928_219_368)),
            -3_574_229_841_928_219_368
        );
    }
}
