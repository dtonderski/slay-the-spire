# Immutable Courier journal fixture

`courier-strict-failure.jsonl.gz` is a byte-identical copy of the original
`eval-4-26.jsonl.gz` experiment journal (SHA256
`d67c72072b7a7f584f3d7ca08f02934286a9ecc1901f1a22c7333da967993bc3`).
It is a simulator experiment journal, **not** a real-game parity trace.

Tests reconstruct its initial state and accepted action prefix. Strict mode must
still reject the unsupported process-global Courier draw atomically. Explicit
private environmental inputs permit replacement generation and clone-repeat.
Additional environmental seeds are independent test inputs, not edits to the
original captured journal or evidence of vanilla global-RNG sequence parity.
