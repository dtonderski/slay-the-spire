# Synthetic campfire API fixtures

These are two **unchanged simulator-generated training journals**, not captured
real-game traces or parity evidence. `metadata.json` identifies each original
journal, its SHA-256 and the accepted-prefix length ending at a natural unused
Act-1 floor-15 campfire. Tests construct a natural state from the journal's seed
and replay checked public actions; they never hydrate from recorded observations.
The full original payloads, including suffixes unused by the fixture tests, remain
intact. HP variants are new independent synthetic initial states.

They cover source immutability, revision fencing, deterministic variant creation,
HP-only configuration, and root-bank seed-split/provenance validation. Changes in
legitimate simulator behavior can invalidate a prefix; investigate that rather
than rewriting either original journal to make replay pass.
