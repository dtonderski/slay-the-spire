# Synthetic campfire API fixtures

These are **unchanged simulator-generated training journals**, not captured
real-game traces or parity evidence. `metadata-current.json` selects the current
pair; `metadata.json` preserves the original pair and their provenance. Each
entry gives its SHA-256 and accepted-prefix length ending at a natural unused
Act-1 floor-15 campfire. Tests construct a natural state from the journal's seed
and replay checked public actions; they never hydrate from recorded observations.
The full original payloads, including suffixes unused by the fixture tests, remain
intact. HP variants are new independent synthetic initial states.

Owner integration confirmed that `validation.jsonl.gz` is obsolete after the
correct grid-preview fence in #95: step 51 requests another toggle while only
Confirm/Cancel are legal. Its bytes and original metadata remain unchanged, and
negative tests assert rejection rather than replay repair. The current validation
fixture is the byte-for-byte original `eval-440-103.jsonl.gz` from the same
simulator training run, under a new fixture filename/identity. Its 276-action
natural prefix is independently verified; this does not make the old prefix pass.

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
