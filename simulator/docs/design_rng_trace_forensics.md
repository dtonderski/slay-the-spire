# RNG Trace Forensics

## Problem

CommunicationMod traces preserve commands and visible states but not the simulator's hidden RNG call history. A visible mismatch can therefore appear long after one extra or missing draw. Debugging by comparing only final counters does not identify the responsible operation.

## Design

Add opt-in, non-semantic tracing to `StsRng` and expose it through `sts_verify rng-trace`.

Each captured call records:

- the trace action step and command currently being replayed;
- the named RNG stream, when the owning state identifies it;
- counter values before and after the call;
- the operation, arguments, and returned value;
- the Rust source file, line, and column of the call.

Tracing is thread-local and inactive by default. Trace metadata is not serialized, hashed, or compared as authoritative RNG state. Constructing an RNG at an existing counter is restoration, not gameplay, and must not emit synthetic historical draws. The verifier supplies action context without changing replay behavior or consuming observations as simulator input.

The CLI emits JSON Lines so traces can be filtered and compared with ordinary command-line tools. Step and stream filters affect output only; replay remains complete from the seed.

## Constraints

- No RNG output may change when tracing is enabled.
- No observation may repair or hydrate simulator state.
- Existing JSONL traces remain unchanged and require no recollection.
- Unknown stream labels remain explicit rather than inferred from seed values.
- The facility is diagnostic infrastructure, not a parity comparison exemption.
