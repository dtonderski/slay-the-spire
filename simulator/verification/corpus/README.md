# Verification corpus

Full CommunicationMod captures are immutable and gitignored.

`permanent_traces/` contains the restored reviewed schema-6 cohort. The lean
verifier accepts both schema 6 and schema 7; new captures use schema 7's stronger
command-settlement fences. Do not edit, truncate, or convert old payloads to make
them pass. Verify a cohort directly:

```bash
cargo run -p sts_verify --bin sts_verify -- /path/to/schema-6-or-7-traces
```

The command must fail if any trace diverges, is rejected, is malformed, or ends
before game termination. There are no quarantine or exclusion lists.
