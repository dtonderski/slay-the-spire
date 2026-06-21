use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulatorRng {
    state: u64,
    log: Vec<RngDraw>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RngDraw {
    pub stream: RngStream,
    pub call_site: String,
    pub bound: usize,
    pub value: usize,
}

/// Slay the Spire's target-game RNG wrapper for version `12-18-2022`.
///
/// The game class `com.megacrit.cardcrawl.random.Random` wraps libGDX
/// `RandomXS128`, increments `counter` once per public draw, and uses inclusive
/// integer bounds for `random(min, max)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StsRng {
    seed0: u64,
    seed1: u64,
    counter: u32,
}

/// Java `java.util.Random` compatibility helper.
///
/// Target relic pool initialization seeds a Java LCG with `relicRng.nextLong()`
/// and then calls `Collections.shuffle`, which is distinct from the game's
/// libGDX `RandomXS128` wrapper used by [StsRng].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaRng {
    seed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RngStream {
    Event,
    MapRoom,
    RewardCard,
    RewardRarity,
    Shuffle,
    Potion,
}

impl SimulatorRng {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed.max(1),
            log: Vec::new(),
        }
    }

    #[must_use]
    pub fn log(&self) -> &[RngDraw] {
        &self.log
    }

    pub fn next_usize(
        &mut self,
        stream: RngStream,
        call_site: &'static str,
        bound: usize,
    ) -> usize {
        assert!(bound > 0, "rng bound must be greater than zero");
        let value = (self.next_u64() as usize) % bound;
        self.log.push(RngDraw {
            stream,
            call_site: call_site.to_owned(),
            bound,
            value,
        });
        value
    }

    #[must_use]
    pub fn next_bool(&mut self, stream: RngStream, call_site: &'static str) -> bool {
        self.next_usize(stream, call_site, 2) == 0
    }

    #[must_use]
    pub fn seed_state(&self) -> u64 {
        self.state
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.state = value;
        value
    }
}

impl StsRng {
    const ZERO_SEED_REPLACEMENT: u64 = 0x8000_0000_0000_0000;
    const MURMUR_MULTIPLIER_1: u64 = 0xff51_afd7_ed55_8ccd;
    const MURMUR_MULTIPLIER_2: u64 = 0xc4ce_b9fe_1a85_ec53;

    #[must_use]
    pub fn new(seed: i64) -> Self {
        let seed = if seed == 0 {
            Self::ZERO_SEED_REPLACEMENT
        } else {
            seed as u64
        };
        let seed0 = Self::murmur_hash3(seed);
        let seed1 = Self::murmur_hash3(seed0);
        Self {
            seed0,
            seed1,
            counter: 0,
        }
    }

    #[must_use]
    pub fn with_counter(seed: i64, counter: u32) -> Self {
        let mut rng = Self::new(seed);
        rng.set_counter(counter);
        rng
    }

    #[must_use]
    pub fn counter(&self) -> u32 {
        self.counter
    }

    #[must_use]
    pub fn state(&self) -> (u64, u64) {
        (self.seed0, self.seed1)
    }

    pub fn set_counter(&mut self, target: u32) {
        assert!(
            target >= self.counter,
            "STS RNG counter cannot move backwards"
        );
        while self.counter < target {
            self.random_bool();
        }
    }

    pub fn random_int(&mut self, max_inclusive: i32) -> i32 {
        assert!(max_inclusive >= 0, "STS RNG max must be non-negative");
        self.counter += 1;
        self.next_int(max_inclusive + 1)
    }

    pub fn random_int_range(&mut self, min_inclusive: i32, max_inclusive: i32) -> i32 {
        assert!(
            max_inclusive >= min_inclusive,
            "STS RNG range must be ordered"
        );
        self.counter += 1;
        min_inclusive + self.next_int(max_inclusive - min_inclusive + 1)
    }

    pub fn random_bool(&mut self) -> bool {
        self.counter += 1;
        (self.next_long() & 1) != 0
    }

    pub fn random_float(&mut self) -> f32 {
        self.counter += 1;
        ((self.next_long() >> 40) as f64 * 5.960_464_477_539_063e-8) as f32
    }

    pub fn random_float_range(&mut self, min_inclusive: f32, max_inclusive: f32) -> f32 {
        assert!(max_inclusive >= min_inclusive, "STS RNG float range must be ordered");
        min_inclusive + self.random_float() * (max_inclusive - min_inclusive)
    }

    pub fn random_long(&mut self) -> i64 {
        self.counter += 1;
        self.next_long() as i64
    }

    pub fn raw_next_int(&mut self, bound_exclusive: i32) -> i32 {
        self.next_int(bound_exclusive)
    }

    /// Fisher-Yates shuffle matching Java `Collections.shuffle` with raw `RandomXS128`.
    pub fn collections_shuffle<T>(&mut self, items: &mut [T]) {
        for i in (2..=items.len()).rev() {
            let j = self.raw_next_int(i as i32) as usize;
            items.swap(i - 1, j);
        }
    }

    fn next_int(&mut self, bound_exclusive: i32) -> i32 {
        assert!(bound_exclusive > 0, "STS RNG bound must be positive");
        self.next_long_bound(bound_exclusive as u64) as i32
    }

    fn next_long_bound(&mut self, bound_exclusive: u64) -> u64 {
        loop {
            let bits = self.next_long() >> 1;
            let value = bits % bound_exclusive;
            if (bits.wrapping_sub(value).wrapping_add(bound_exclusive - 1) as i64) >= 0 {
                return value;
            }
        }
    }

    fn next_long(&mut self) -> u64 {
        let mut s1 = self.seed0;
        let s0 = self.seed1;
        self.seed0 = s0;
        s1 ^= s1 << 23;
        self.seed1 = s1 ^ s0 ^ (s1 >> 17) ^ (s0 >> 26);
        self.seed1.wrapping_add(s0)
    }

    fn murmur_hash3(mut value: u64) -> u64 {
        value ^= value >> 33;
        value = value.wrapping_mul(Self::MURMUR_MULTIPLIER_1);
        value ^= value >> 33;
        value = value.wrapping_mul(Self::MURMUR_MULTIPLIER_2);
        value ^= value >> 33;
        value
    }
}

impl JavaRng {
    const MULTIPLIER: u64 = 0x5DEECE66D;
    const ADDEND: u64 = 0xB;
    const MASK: u64 = (1_u64 << 48) - 1;

    #[must_use]
    pub fn new(seed: i64) -> Self {
        Self {
            seed: ((seed as u64) ^ Self::MULTIPLIER) & Self::MASK,
        }
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "Java Random bound must be positive");

        if (bound & -bound) == bound {
            return (((bound as i64) * (self.next_bits(31) as i64)) >> 31) as i32;
        }

        loop {
            let bits = self.next_bits(31) as i32;
            let value = bits % bound;
            if bits - value + (bound - 1) >= 0 {
                return value;
            }
        }
    }

    pub fn collections_shuffle<T>(&mut self, items: &mut [T]) {
        for i in (2..=items.len()).rev() {
            let j = self.next_int(i as i32) as usize;
            items.swap(i - 1, j);
        }
    }

    fn next_bits(&mut self, bits: u32) -> u32 {
        self.seed = self
            .seed
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::ADDEND)
            & Self::MASK;
        (self.seed >> (48 - bits)) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_rng_is_deterministic_and_logged() {
        let mut first = SimulatorRng::new(7);
        let mut second = SimulatorRng::new(7);

        assert_eq!(
            first.next_usize(RngStream::Shuffle, "test", 10),
            second.next_usize(RngStream::Shuffle, "test", 10)
        );
        assert_eq!(first.log().len(), 1);
    }

    #[test]
    fn zero_seed_is_mapped_to_nonzero_state() {
        let mut rng = SimulatorRng::new(0);

        let first = rng.next_usize(RngStream::Shuffle, "test", 10);
        let second = rng.next_usize(RngStream::Shuffle, "test", 10);

        assert_ne!(rng.seed_state(), 0);
        assert_ne!([first, second], [0, 0]);
    }

    #[test]
    fn sts_rng_matches_target_randomxs128_reference_outputs() {
        let mut rng = StsRng::new(22_079_335_079);

        assert_eq!(rng.random_int(99), 63);
        assert_eq!(rng.random_int(99), 25);
        assert_eq!(rng.random_int(99), 52);
        assert_eq!(rng.counter(), 3);
    }

    #[test]
    fn sts_rng_float_matches_target_randomxs128_reference_output() {
        let mut rng = StsRng::new(22_079_335_079);

        assert_eq!(rng.random_float().to_bits(), 0x396a_1000);
        assert_eq!(rng.counter(), 1);
    }

    #[test]
    fn sts_rng_inclusive_range_uses_target_random_reference_output() {
        let mut rng = StsRng::new(22_079_335_079);

        assert_eq!(rng.random_int_range(49, 54), 54);
        assert_eq!(rng.counter(), 1);
    }

    #[test]
    fn sts_rng_counter_constructor_advances_with_public_draw_semantics() {
        let mut stepped = StsRng::new(1_957_307_888_551);
        for _ in 0..5 {
            stepped.random_bool();
        }

        let advanced = StsRng::with_counter(1_957_307_888_551, 5);

        assert_eq!(advanced.counter(), 5);
        assert_eq!(advanced.state(), stepped.state());
        assert_eq!(
            advanced.clone().random_int(99),
            stepped.clone().random_int(99)
        );
    }

    #[test]
    fn java_rng_matches_reference_next_int_sequence() {
        let mut rng = JavaRng::new(0);

        assert_eq!(rng.next_int(10), 0);
        assert_eq!(rng.next_int(10), 8);
        assert_eq!(rng.next_int(10), 9);
        assert_eq!(rng.next_int(10), 7);
        assert_eq!(rng.next_int(10), 5);
    }

    #[test]
    fn java_collections_shuffle_matches_reference_order() {
        let mut values = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        JavaRng::new(0).collections_shuffle(&mut values);

        assert_eq!(values, vec![4, 8, 9, 6, 3, 5, 2, 1, 7, 0]);
    }
}
