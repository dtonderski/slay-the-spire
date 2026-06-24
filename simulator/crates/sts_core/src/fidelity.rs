//! Vocabulary for simulator fidelity boundaries.
//!
//! These labels keep hard-coded behavior honest. Some constants and branches are
//! source-backed target-game behavior; others are compatibility fixtures or
//! verifier-only trace scaffolding. New code should choose the narrowest label
//! that describes its evidence instead of using an ambiguous "fixed" or
//! "fallback" name.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FidelityCategory {
    /// Decoded from the target game or verified against captured traces.
    SourceBacked,
    /// Deterministic early-milestone fixture kept for compatibility/tests.
    LegacyFixed,
    /// Simulator-only provisional behavior with no parity claim.
    Placeholder,
    /// Behavior pinned to a known captured branch rather than general RNG.
    CapturedBranch,
    /// Verification-only observed-state sync or trace repair scaffolding.
    VerifierOnly,
}
