//! Target-version seed string conversion helpers.

pub const STS_SEED_ALPHABET: &str = "0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ";

/// Convert a Slay the Spire user-facing seed string to the numeric seed used by
/// `SeedHelper.getLong` in the 12-18-2022 desktop jar.
pub fn sts_seed_string_to_long(seed: &str) -> i64 {
    let mut value = 0_i64;
    for ch in seed.to_uppercase().replace('O', "0").chars() {
        let digit = STS_SEED_ALPHABET.find(ch).unwrap_or_else(|| {
            panic!("invalid Slay the Spire seed character: {ch}");
        }) as i64;
        value = value
            .wrapping_mul(STS_SEED_ALPHABET.len() as i64)
            .wrapping_add(digit);
    }
    value
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
}
