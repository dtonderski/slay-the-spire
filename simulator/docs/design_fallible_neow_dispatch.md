# Fallible Neow Dispatch

## Problem

Several public Neow helpers accepted broad reward or drawback enums but panicked
when called with a variant owned by a different Neow mechanic. Simple drawback
application also mutated state infallibly and used arithmetic that could panic
or partially update divergent imported state.

## Decision

Card, rare-card, colorless-card, fixed-tier relic, simple drawback, and grid
dispatch APIs return `SimResult`. A reward or drawback handled by another
mechanic is an `IllegalAction`, not a process panic. RNG-taking generators
validate the reward family before consuming either RNG stream.

Simple drawbacks use clone-and-commit. Max-HP loss and percent damage use
checked arithmetic, and the curse variant explicitly directs callers to the
card-RNG-aware curse path. Core event code propagates these errors; verifier
helpers use precise assertions only after typed option-family predicates have
established the variant.

Private panics that remain in option-slot generation and static card-pool
selection represent closed internal invariants: public option generation always
uses slots `0..4`, and modeled reward pools are compile-time non-empty.

## Verification

Regression tests cover every invalid public dispatch family, exact run rollback,
zero RNG consumption for invalid RNG-backed rewards, and drawback arithmetic
overflow. The permanent corpus verifies unchanged valid-path sequencing.
