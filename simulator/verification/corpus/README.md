# Verification fixtures

Git stores only compact, reviewable fixtures. Full CommunicationMod payloads
are gitignored and must not be committed.

## Active authoritative corpus

`permanent_traces/` contains only traces captured under the current collection
epoch: fixed gameplay delta (`collection.2`) and boundary schema 6 exactly.
A later boundary schema requires an explicit verifier/corpus epoch update.
The current authoritative regression cohort contains 433 independently audited
traces. Its original 208 payloads declare `collection.2-schema6`; the additional
103 FIDL01880–FIDL02008 payloads retain a stale pre-schema-6 `source_version`
string, with the promotion record externally attesting their installed
SuperFastMode `1.0.9-collection.2` JAR and exact collection artifact hashes. A
third cohort contains 77 terminal FIDL02009–FIDL02106 captures and retains its
composite source-artifact metadata. The fourth promotion adds 43 reviewed
FIDL02107–FIDL02154 captures plus failure-driven regression traces FIDL02161 and
FIDL02166. The latter two were admitted unchanged while failing, then retained
after source-backed fixes for Steam Barrier identity and full-hand Exhume action
timing. FIDL02101 remains immutable outside the lock because it exposes a
pre-opening Colosseum publication without a pre-action scheduler input. The
current verifier completely passes all 433 traces with zero unsupported
mechanics or raw unexpected diffs. The earlier
independently audited exact-20 schema-6 pilot is retained unchanged under
`legacy_schema6_initial_pilot/permanent_traces/`.
The schema-3 pilot is non-authoritative because a deferred out-of-combat update
could complete the first combat card command before the queued card resolved.
The schema-4 pilot is non-authoritative because a late state from a preceding
choice could overtake execution and incorrectly complete the next command. The
schema-5 pilot proved its command fence but still published duration-dependent
`ObtainKeyEffect` and `ShowCardAndObtainEffect` mutations; its 20 immutable
payloads are retained under `legacy_schema5_pilot/permanent_traces/`.
`sts_verify status` defaults to this directory and fails when it contains no
authoritative traces. Override the path with `STS_PERMANENT_CORPUS_DIR` or
the worker count with `STS_VERIFY_JOBS`:

```bash
cd simulator
cargo run -q -p sts_verify --bin sts_verify -- status
cargo test -p sts_verify --test corpus \
  external_permanent_traces_are_structurally_replayable -- --ignored
```

Do not track a corpus inventory, outcome ledger, or status snapshot. Generate
status directly from the payloads and verifier revision being evaluated. Never
edit a captured trace to make replay pass.

## Legacy pre-collection.2 archive

The original 602 payloads predate the fixed-gameplay-delta collection fork.
They remain immutable investigation evidence but are not authoritative parity
inputs. Local payloads are stored under
`legacy_pre_collection_2/permanent_traces/`; their original quarantine metadata
is `legacy_pre_collection_2/quarantine.txt`. They no longer occupy the private
Hugging Face dataset. Do not copy this cohort back into `permanent_traces/`.

## External authoritative mirror

The private `dtonderski/sts-permanent-traces` Hugging Face dataset contains the
433 active schema-6 traces as deterministic `<trace>.jsonl.gz` files. It is the
external mirror used by Cursor Cloud and clean local checkouts; uploads remain
an explicit audited local operation.

## Repository fixtures

- `manual/`: tiny hand-authored parser or transition fixtures.
- `bugs/`: compact minimized regressions when a source-level fixture is clearer
  than invoking the external corpus.

A clean checkout and its ordinary test suite must not require any external
corpus.
