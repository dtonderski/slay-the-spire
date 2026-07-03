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
}
