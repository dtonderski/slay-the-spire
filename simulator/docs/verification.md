# Verification

`sts_verify` has one job: replay current CommunicationMod traces against
`sts_core`.

```bash
cargo run -p sts_verify --bin sts_verify -- <schema-6-or-7-trace.jsonl-or-directory>
```

For each trace it:

1. requires one leading schema-6 or schema-7 metadata record;
2. initializes Ironclad from `START_VERIFY`, profile inputs, and boss unlocks;
3. validates each command against its schema's completion fence;
4. applies the command once to simulator-owned state;
5. compares the resulting observable projection with the recorded state; and
6. stops at the first rejection, unsupported command, transition error, or
   state difference.

The only post-start trace inputs consumed by simulation are explicitly typed
action-time environmental inputs: `playtime_seconds` and external RNG records.
Observed state is comparison output only and never repairs simulator state.
Schema-6 captures do not contain action-time playtime, so a timer-dependent
transition can expose that missing input as a divergence; schema 7 records it.

A directory is processed with at most 24 workers. Exit status is zero only when
every trace reaches game termination with no divergence. Nonterminal traces are
reported as incomplete; malformed traces are invalid. There are no exclusions,
quarantines, pre-schema-6 compatibility formats, minimizers, replay artifacts,
or alternate transition candidates in the verifier.

`simulator/verification/corpus/permanent_traces/` contains the restored reviewed schema-6
corpus. These payloads remain immutable evidence. New captures use schema 7;
never rewrite an old capture to make it pass.
