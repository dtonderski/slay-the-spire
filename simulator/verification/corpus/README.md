# Verification fixtures

Git stores only compact, reviewable fixtures in this directory. Full
CommunicationMod trace payloads are external data and must not be added to the
repository.

## External corpus

Set `STS_PERMANENT_CORPUS_DIR` to a directory containing the immutable JSONL
captures before running a corpus-wide command:

```bash
export STS_PERMANENT_CORPUS_DIR=/path/to/permanent_traces
cd simulator
cargo run -q -p sts_verify --bin sts_verify -- status "$STS_PERMANENT_CORPUS_DIR"
cargo test -p sts_verify --test corpus \
  external_permanent_traces_are_structurally_replayable -- --ignored
```

The repository does not track a corpus manifest, inventory, outcome ledger, or
status snapshot. Generate status directly from the external files and the
verifier revision being evaluated. Never edit a captured trace to make replay
pass; repair simulator behavior and replay the unchanged payload.

## Repository fixtures

- `manual/`: tiny hand-authored parser or transition fixtures.
- `bugs/`: compact minimized regressions when a source-level fixture is clearer
  than invoking the external corpus.

A clean checkout and its ordinary test suite must not require the external
corpus.
