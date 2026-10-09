# Immutable simulator-only boundary journals

These are byte-identical diagnostic journals from the stopped, pinned A0 Act-1
experiment `a0-week-20261008` (native source commit `24839c89291361f5cea0880094d0965be687074d`).
They are not captured real-game traces and do not establish gameplay parity.
No original payload, action, failure, or setup was corrected or regenerated.

| Copy | Original journal | SHA256 |
| --- | --- | --- |
| `courier-strict-failure.jsonl.gz` | `eval-4-26.jsonl.gz` | `d67c72072b7a7f584f3d7ca08f02934286a9ecc1901f1a22c7333da967993bc3` |
| `prismatic-reward-failure.jsonl.gz` | `train-9-8.jsonl.gz` | `3449d3142b6575c106b7d1c96eab0430e2bb50111bd279672db1da40b86a4d44` |
| `prismatic-egg-reward-failure.jsonl.gz` | `train-663-49.jsonl.gz` | `c229dd21f3376566dc1a60202744bde6b989362fb8813df60bf16e2f8afd3751` |

Tests construct fresh natural states and replay only accepted actions, checking
revision, original legal-list index and complete public action descriptors.
Strict Courier inputs must still fail atomically. Additional environmental seeds
are explicitly declared diagnostic initial inputs, never written into the original
journal or inferred from observed restocks. Prismatic's original failed legal
kill now publishes the actual reward preview; taking an unimplemented foreign
card is still tested to fail atomically rather than inventing its gameplay.
