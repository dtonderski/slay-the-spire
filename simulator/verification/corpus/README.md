# Verification fixtures

Git stores only compact, reviewable fixtures. Full CommunicationMod payloads
live in the working tree at `permanent_traces/` but are gitignored and must not
be committed.

## Permanent corpus

`sts_verify status` defaults to this directory and uses 24 workers when the
machine has that many CPUs. Override the path with `STS_PERMANENT_CORPUS_DIR`
or the worker count with `STS_VERIFY_JOBS`:

```bash
cd simulator
cargo run -q -p sts_verify --bin sts_verify -- status
cargo test -p sts_verify --test corpus \
  external_permanent_traces_are_structurally_replayable -- --ignored
```

Do not track a corpus manifest, inventory, outcome ledger, or status snapshot.
Generate status directly from the payloads and the verifier revision being
evaluated. Never edit a captured trace to make replay pass; repair simulator
behavior and replay the unchanged payload.

## Repository fixtures

- `manual/`: tiny hand-authored parser or transition fixtures.
- `bugs/`: compact minimized regressions when a source-level fixture is clearer
  than invoking the external corpus.

A clean checkout and its ordinary test suite must not require the external
corpus.
