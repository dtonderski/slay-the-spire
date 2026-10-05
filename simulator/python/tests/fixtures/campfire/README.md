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
legitimate simulator behavior can invalidate a prefix. First investigate and
confirm the intended simulator change; never edit an existing journal's commands,
observations or suffix to manufacture replay success.

After a confirmed legitimate change, replacing an obsolete fixture with a **newly
generated simulator journal** is allowed. Use a new fixture identity and SHA-256,
record its source and prefix length in `metadata.json`, update both binding and RL
tests as needed, and document the replacement in the commit. Keep the old payload
and provenance available in Git history; do not present the replacement as a
successful replay of the old fixture. This exception applies only to these
simulator-generated infrastructure fixtures, never to captured real-game traces
or reviewed corpus payloads.
