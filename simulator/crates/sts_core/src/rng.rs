use serde::{Deserialize, Serialize};
use serde::{Deserializer, Serializer};
use std::{
    cell::RefCell,
    panic::Location,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Authoritative RNG stream associated with a forensic trace event.
///
/// `Unknown` is intentional: callers that own an unclassified scratch RNG must
/// not guess a stream from its seed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RngTraceStream {
    #[default]
    Unknown,
    CardReward,
    CardRandom,
    Event,
    Merchant,
    Misc,
    Monster,
    MonsterHp,
    Potion,
    Relic,
    Shuffle,
    Treasure,
}

/// Replay command that was active when an RNG draw occurred.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct RngTraceContext {
    pub action_step: Option<u32>,
    pub command: Option<String>,
}

/// One target-compatible RNG operation and its returned value.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum RngTraceOperation {
    RandomInt {
        max_inclusive: i32,
        result: i32,
    },
    RandomIntRange {
        min_inclusive: i32,
        max_inclusive: i32,
        result: i32,
    },
    RandomBool {
        result: bool,
    },
    RandomFloat {
        result: f32,
    },
    RandomFloatRange {
        min_inclusive: f32,
        max_inclusive: f32,
        result: f32,
    },
    RandomLong {
        result: i64,
    },
    RawNextInt {
        bound_exclusive: i32,
        result: i32,
    },
    CollectionsShuffleSwap {
        item_count: usize,
        source_index: usize,
        target_index: usize,
    },
}

/// Structured, non-semantic record of one simulator RNG call.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct RngTraceEvent {
    pub sequence: u64,
    pub action_step: Option<u32>,
    pub command: Option<String>,
    pub stream: RngTraceStream,
    pub counter_before: u32,
    pub counter_after: u32,
    pub operation: RngTraceOperation,
    pub source_file: String,
    pub source_line: u32,
    pub source_column: u32,
}

static RNG_TRACE_ACTIVE: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static RNG_TRACE_EVENTS: RefCell<Option<Vec<RngTraceEvent>>> = const { RefCell::new(None) };
    static RNG_TRACE_CONTEXT: RefCell<RngTraceContext> = RefCell::new(RngTraceContext::default());
}

#[cfg(test)]
struct RngTraceCaptureGuard {
    previous_events: Option<Option<Vec<RngTraceEvent>>>,
    previous_context: Option<RngTraceContext>,
}

#[cfg(test)]
impl RngTraceCaptureGuard {
    fn start() -> Self {
        RNG_TRACE_ACTIVE.fetch_add(1, Ordering::Relaxed);
        let previous_events = RNG_TRACE_EVENTS.with(|events| events.replace(Some(Vec::new())));
        let previous_context =
            RNG_TRACE_CONTEXT.with(|context| context.replace(RngTraceContext::default()));
        Self {
            previous_events: Some(previous_events),
            previous_context: Some(previous_context),
        }
    }

    fn finish(mut self) -> Vec<RngTraceEvent> {
        let events = RNG_TRACE_EVENTS.with(|slot| {
            slot.replace(
                self.previous_events
                    .take()
                    .expect("RNG trace capture guard finishes once"),
            )
        });
        RNG_TRACE_CONTEXT.with(|slot| {
            slot.replace(
                self.previous_context
                    .take()
                    .expect("RNG trace context restores once"),
            );
        });
        RNG_TRACE_ACTIVE.fetch_sub(1, Ordering::Relaxed);
        events.expect("active RNG trace capture owns an event buffer")
    }
}

#[cfg(test)]
impl Drop for RngTraceCaptureGuard {
    fn drop(&mut self) {
        let Some(previous_events) = self.previous_events.take() else {
            return;
        };
        RNG_TRACE_ACTIVE.fetch_sub(1, Ordering::Relaxed);
        RNG_TRACE_EVENTS.with(|slot| {
            slot.replace(previous_events);
        });
        if let Some(previous_context) = self.previous_context.take() {
            RNG_TRACE_CONTEXT.with(|slot| {
                slot.replace(previous_context);
            });
        }
    }
}

/// Captures RNG calls made synchronously on the current thread while `f` runs.
/// Tracing is inactive outside this scope and does not affect RNG state.
#[cfg(test)]
pub(crate) fn capture_rng_trace<T>(f: impl FnOnce() -> T) -> (T, Vec<RngTraceEvent>) {
    let guard = RngTraceCaptureGuard::start();
    let value = f();
    let events = guard.finish();
    (value, events)
}

/// Sets the replay command attached to subsequent events in the active capture.
/// This is diagnostic context only and is ignored when tracing is inactive.
#[cfg(test)]
pub(crate) fn set_rng_trace_context(context: RngTraceContext) {
    RNG_TRACE_CONTEXT.with(|slot| {
        slot.replace(context);
    });
}

fn record_rng_trace_event(
    stream: RngTraceStream,
    counter_before: u32,
    counter_after: u32,
    operation: RngTraceOperation,
    caller: &'static Location<'static>,
) {
    // Normal simulation and corpus verification never touch thread-local trace
    // state beyond this predictable disabled branch.
    if RNG_TRACE_ACTIVE.load(Ordering::Relaxed) == 0 {
        return;
    }
    RNG_TRACE_EVENTS.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(events) = slot.as_mut() else {
            return;
        };
        let context = RNG_TRACE_CONTEXT.with(|context| context.borrow().clone());
        events.push(RngTraceEvent {
            sequence: events.len() as u64,
            action_step: context.action_step,
            command: context.command,
            stream,
            counter_before,
            counter_after,
            operation,
            source_file: caller.file().to_owned(),
            source_line: caller.line(),
            source_column: caller.column(),
        });
    });
}

/// Largest RNG draw counter representable by the target game's signed Java
/// `int` field.
pub(crate) const MAX_SUPPORTED_RNG_COUNTER: u32 = i32::MAX as u32;

#[must_use]
pub(crate) const fn rng_counter_is_supported(counter: u32) -> bool {
    counter <= MAX_SUPPORTED_RNG_COUNTER
}

/// Slay the Spire's target-game RNG wrapper for version `12-18-2022`.
///
/// The game class `com.megacrit.cardcrawl.random.Random` wraps libGDX
/// `RandomXS128`, increments `counter` once per public draw, and uses inclusive
/// integer bounds for `random(min, max)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StsRng {
    seed0: u64,
    seed1: u64,
    counter: u32,
    /// Diagnostic metadata only; snapshots and equality intentionally ignore it.
    #[serde(skip)]
    trace_stream: RngTraceStream,
}

impl PartialEq for StsRng {
    fn eq(&self, other: &Self) -> bool {
        self.seed0 == other.seed0 && self.seed1 == other.seed1 && self.counter == other.counter
    }
}

impl Eq for StsRng {}

/// Java `java.util.Random` compatibility helper.
///
/// Target relic pool initialization seeds a Java LCG with `relicRng.nextLong()`
/// and then calls `Collections.shuffle`, which is distinct from the game's
/// libGDX `RandomXS128` wrapper used by [StsRng].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaRng {
    seed: u64,
}

/// Call-time state of libGDX's process-global `MathUtils.random`.
///
/// Unlike [StsRng], this state is not derived from a run seed. Strict replay
/// receives it as explicit external input immediately before a gameplay draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MathUtilsRngState {
    #[serde(with = "hex_u64")]
    pub state0: u64,
    #[serde(with = "hex_u64")]
    pub state1: u64,
}

/// Instrumented process-global RNG call sites supported by strict replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalRngKind {
    CardGroupGetRandomCardByType,
}

/// One ordered, call-time external RNG input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalRngInput {
    pub kind: ExternalRngKind,
    pub state: MathUtilsRngState,
    /// Inclusive range passed to `MathUtils.random(int)`.
    pub range_inclusive: u32,
}

/// Derives the target game's per-floor RNG seed with Java `long` overflow
/// semantics, which must be identical in debug and release builds.
#[must_use]
pub(crate) fn seed_for_floor(seed: i64, floor_num: impl Into<i64>) -> i64 {
    seed.wrapping_add(floor_num.into())
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
            trace_stream: RngTraceStream::Unknown,
        }
    }

    #[must_use]
    pub fn with_counter(seed: i64, counter: u32) -> Self {
        let mut rng = Self::new(seed);
        rng.set_counter(counter);
        rng
    }

    /// Constructs a restored RNG with an explicit non-semantic trace label.
    #[must_use]
    pub fn with_counter_for_stream(seed: i64, counter: u32, trace_stream: RngTraceStream) -> Self {
        let mut rng = Self::with_counter(seed, counter);
        rng.trace_stream = trace_stream;
        rng
    }

    /// Attaches an explicit non-semantic trace label to this RNG.
    #[must_use]
    pub fn for_stream(mut self, trace_stream: RngTraceStream) -> Self {
        self.trace_stream = trace_stream;
        self
    }

    /// Restore a mid-stream RandomXS128 state (seed pair + public draw counter).
    /// Used by seed-start diagnostics and tests that continue from a snapshot.
    #[must_use]
    pub fn from_raw_state(seed0: u64, seed1: u64, counter: u32) -> Self {
        Self {
            seed0,
            seed1,
            counter,
            trace_stream: RngTraceStream::Unknown,
        }
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
            rng_counter_is_supported(target),
            "STS RNG counter exceeds the target signed range"
        );
        assert!(
            target >= self.counter,
            "STS RNG counter cannot move backwards"
        );
        // Restoring a counter reconstructs hidden state; it is not a new
        // gameplay draw and therefore must not emit synthetic trace events.
        while self.counter < target {
            self.counter += 1;
            let _ = self.next_long();
        }
    }

    #[track_caller]
    pub fn random_int(&mut self, max_inclusive: i32) -> i32 {
        assert!(max_inclusive >= 0, "STS RNG max must be non-negative");
        let counter_before = self.counter;
        self.counter += 1;
        let result = self.next_int(max_inclusive + 1);
        record_rng_trace_event(
            self.trace_stream,
            counter_before,
            self.counter,
            RngTraceOperation::RandomInt {
                max_inclusive,
                result,
            },
            Location::caller(),
        );
        result
    }

    #[track_caller]
    pub fn random_int_range(&mut self, min_inclusive: i32, max_inclusive: i32) -> i32 {
        assert!(
            max_inclusive >= min_inclusive,
            "STS RNG range must be ordered"
        );
        let counter_before = self.counter;
        self.counter += 1;
        let result = min_inclusive + self.next_int(max_inclusive - min_inclusive + 1);
        record_rng_trace_event(
            self.trace_stream,
            counter_before,
            self.counter,
            RngTraceOperation::RandomIntRange {
                min_inclusive,
                max_inclusive,
                result,
            },
            Location::caller(),
        );
        result
    }

    #[track_caller]
    pub fn random_bool(&mut self) -> bool {
        let counter_before = self.counter;
        self.counter += 1;
        let result = (self.next_long() & 1) != 0;
        record_rng_trace_event(
            self.trace_stream,
            counter_before,
            self.counter,
            RngTraceOperation::RandomBool { result },
            Location::caller(),
        );
        result
    }

    #[track_caller]
    pub fn random_float(&mut self) -> f32 {
        let counter_before = self.counter;
        self.counter += 1;
        let result = ((self.next_long() >> 40) as f64 * 5.960_464_477_539_063e-8) as f32;
        record_rng_trace_event(
            self.trace_stream,
            counter_before,
            self.counter,
            RngTraceOperation::RandomFloat { result },
            Location::caller(),
        );
        result
    }

    #[track_caller]
    pub fn random_float_range(&mut self, min_inclusive: f32, max_inclusive: f32) -> f32 {
        assert!(
            max_inclusive >= min_inclusive,
            "STS RNG float range must be ordered"
        );
        let counter_before = self.counter;
        self.counter += 1;
        let unit = ((self.next_long() >> 40) as f64 * 5.960_464_477_539_063e-8) as f32;
        let result = min_inclusive + unit * (max_inclusive - min_inclusive);
        record_rng_trace_event(
            self.trace_stream,
            counter_before,
            self.counter,
            RngTraceOperation::RandomFloatRange {
                min_inclusive,
                max_inclusive,
                result,
            },
            Location::caller(),
        );
        result
    }

    #[track_caller]
    pub fn random_long(&mut self) -> i64 {
        let counter_before = self.counter;
        self.counter += 1;
        let result = self.next_long() as i64;
        record_rng_trace_event(
            self.trace_stream,
            counter_before,
            self.counter,
            RngTraceOperation::RandomLong { result },
            Location::caller(),
        );
        result
    }

    #[track_caller]
    pub fn raw_next_int(&mut self, bound_exclusive: i32) -> i32 {
        let result = self.next_int(bound_exclusive);
        record_rng_trace_event(
            self.trace_stream,
            self.counter,
            self.counter,
            RngTraceOperation::RawNextInt {
                bound_exclusive,
                result,
            },
            Location::caller(),
        );
        result
    }

    /// Fisher-Yates shuffle matching Java `Collections.shuffle` with raw `RandomXS128`.
    #[track_caller]
    pub fn collections_shuffle<T>(&mut self, items: &mut [T]) {
        let item_count = items.len();
        for i in (2..=item_count).rev() {
            let j = self.next_int(i as i32) as usize;
            record_rng_trace_event(
                self.trace_stream,
                self.counter,
                self.counter,
                RngTraceOperation::CollectionsShuffleSwap {
                    item_count,
                    source_index: i - 1,
                    target_index: j,
                },
                Location::caller(),
            );
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
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
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

impl MathUtilsRngState {
    /// Match libGDX `MathUtils.random(maxInclusive)`.
    #[must_use]
    pub fn random_int(&mut self, max_inclusive: u32) -> u32 {
        self.next_long_bound(u64::from(max_inclusive) + 1) as u32
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
        let mut s1 = self.state0;
        let s0 = self.state1;
        self.state0 = s0;
        s1 ^= s1 << 23;
        self.state1 = s1 ^ s0 ^ (s1 >> 17) ^ (s0 >> 26);
        self.state1.wrapping_add(s0)
    }
}

mod hex_u64 {
    use super::*;
    use serde::de::Error as _;

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{value:016x}"))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        if encoded.len() != 16 || !encoded.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(D::Error::custom(
                "RandomXS128 state word must be exactly 16 hexadecimal digits",
            ));
        }
        u64::from_str_radix(&encoded, 16).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn rng_trace_records_named_stream_context_result_and_call_site() {
        let ((first, second), events) = capture_rng_trace(|| {
            set_rng_trace_context(RngTraceContext {
                action_step: Some(41),
                command: Some("END".to_owned()),
            });
            let mut rng =
                StsRng::with_counter_for_stream(22_079_335_079, 2, RngTraceStream::CardRandom);
            (rng.random_int(99), rng.random_bool())
        });

        assert_eq!((first, second), (52, true));
        assert_eq!(events.len(), 2, "counter restoration must not emit draws");
        assert_eq!(events[0].sequence, 0);
        assert_eq!(events[0].action_step, Some(41));
        assert_eq!(events[0].command.as_deref(), Some("END"));
        assert_eq!(events[0].stream, RngTraceStream::CardRandom);
        assert_eq!((events[0].counter_before, events[0].counter_after), (2, 3));
        assert!(matches!(
            events[0].operation,
            RngTraceOperation::RandomInt {
                max_inclusive: 99,
                result: 52
            }
        ));
        assert!(events[0].source_file.ends_with("rng.rs"));
        assert!(events[0].source_line > 0);
    }

    #[test]
    fn rng_trace_capture_restores_disabled_fast_path_after_panic() {
        let result = std::panic::catch_unwind(|| {
            let _ = capture_rng_trace(|| {
                let mut rng = StsRng::new(1);
                let _ = rng.random_bool();
                panic!("test panic");
            });
        });

        assert!(result.is_err());
        assert_eq!(RNG_TRACE_ACTIVE.load(Ordering::Relaxed), 0);
        let (_, events) = capture_rng_trace(|| StsRng::new(2).random_bool());
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn trace_label_is_nonsemantic_and_not_serialized() {
        let unknown = StsRng::new(7);
        let named = unknown.clone().for_stream(RngTraceStream::Monster);

        assert_eq!(unknown, named);
        assert_eq!(
            serde_json::to_value(&unknown).expect("serialize unknown RNG"),
            serde_json::to_value(&named).expect("serialize named RNG")
        );
    }

    #[test]
    fn target_rng_counter_range_is_signed_java_int() {
        assert!(rng_counter_is_supported(0));
        assert!(rng_counter_is_supported(i32::MAX as u32));
        assert!(!rng_counter_is_supported(i32::MAX as u32 + 1));
        assert!(!rng_counter_is_supported(u32::MAX));
    }

    #[test]
    #[should_panic(expected = "STS RNG counter exceeds the target signed range")]
    fn counter_reconstruction_rejects_unrepresentable_target_before_advancing() {
        let _ = StsRng::with_counter(1, i32::MAX as u32 + 1);
    }

    #[test]
    fn floor_seed_wraps_with_java_long_semantics() {
        assert_eq!(seed_for_floor(i64::MAX, 1), i64::MIN);
        assert_eq!(seed_for_floor(i64::MAX - 1, 3), i64::MIN + 1);
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

    #[test]
    fn math_utils_state_matches_target_randomxs128_bounded_draw() {
        let mut rng = MathUtilsRngState {
            state0: 0x0123_4567_89ab_cdef,
            state1: 0xfedc_ba98_7654_3210,
        };

        // Verified against the target desktop jar's RandomXS128 and
        // MathUtils.random(16).
        assert_eq!(rng.random_int(16), 10);
        assert_eq!(
            rng,
            MathUtilsRngState {
                state0: 0xfedc_ba98_7654_3210,
                state1: 0x4c3b_7355_7711_e6f7,
            }
        );
    }

    #[test]
    fn math_utils_state_serializes_losslessly_as_hex_strings() {
        let input = ExternalRngInput {
            kind: ExternalRngKind::CardGroupGetRandomCardByType,
            state: MathUtilsRngState {
                state0: u64::MAX,
                state1: 0x8000_0000_0000_0001,
            },
            range_inclusive: 16,
        };

        let json = serde_json::to_string(&input).expect("external RNG input serializes");
        assert!(json.contains(r#""state0":"ffffffffffffffff""#));
        assert!(json.contains(r#""state1":"8000000000000001""#));
        assert_eq!(
            serde_json::from_str::<ExternalRngInput>(&json)
                .expect("external RNG input deserializes"),
            input
        );
    }
}
