# Source-epoch bundle proposal

**Status:** proposal only. Do not implement without parent approval.
**Worktree:** `exp/experiment-infra-v1` (`/tmp/sts-experiment-infra-v1`)
**Constraint:** exact native/runtime identity stays blocking provenance. This proposal
archives the bytes that identity already requires. It does not add bypass flags,
does not demote the native digest to observational metadata, and does not make an
old checkpoint runnable by ignoring a digest mismatch.

## Problem

`sts_sim.rl.training._source_digest` attests two things together:

1. repository identity (`git_sha`, clean/dirty, dirty diff digest)
2. the exact loaded native extension bytes (`native_extension_digest` of
   `sts_sim._native.__file__`)

A later evaluator that has the git commit but not those native bytes cannot
reproduce the identity the checkpoint recorded. The current failure mode is
missing preservation, not permission to skip the native digest. Weakening the
check so an old checkpoint "runs" would mix source epochs.

Raw native bytes are behaviorally relevant. Compiler, manylinux image, link
flags, and the precise `.so`/`.pyd` contents can change combat transitions
without a Python-level git diff.

## Proposed artifact: `source-epoch-bundle-v1`

A write-once directory, content-addressed like other scientific experiment
artifacts, produced once per frozen source epoch **before** training
source-bound checkpoints.

```
source-epoch-bundle/
  manifest.json
  native/
    sts_sim._native.<ext>          # exact file bytes from import
    sts_sim._native.sha256         # lowercase SHA-256 of those bytes
  toolchain/
    rustc-version.txt
    cargo-version.txt
    python-version.txt
    platform.json
    maturin-build.json             # cargo target, features, profile
  source/
    git-sha.txt
    dirty-diff.patch               # omitted iff clean
    dirty-diff.sha256              # omitted iff clean
  contracts/
    encoder_contract_digest.txt
    vocabulary-fingerprint.txt     # if frozen with the epoch
    search-contract-digest.txt     # if frozen with the epoch
```

`manifest.json` names each component separately so a mismatch can say *which*
identity failed:

| Field | Role | Blocking? |
| --- | --- | --- |
| `git_sha` | checkout object | yes, diagnostic component |
| `dirty_diff_digest` | null iff clean | yes when dirty |
| `native_extension_digest` | SHA-256 of archived native bytes | **yes, blocking** |
| `runtime_identity_digest` | Python/numpy/torch/platform/lockfiles | yes, diagnostic component |
| `encoder_contract_digest` | encoder schema | yes, diagnostic component |
| `vocabulary_fingerprint` | frozen vocab | yes when the epoch froze one |
| `search_contract_digest` | teacher/search contract | yes when the epoch froze one |

The native digest in the manifest **must equal** the digest of `native/` bytes
and **must equal** `hashlib.sha256(Path(_native.__file__).read_bytes())` at
bundle creation. Later evaluation compares the loaded extension to that same
digest. A git SHA match with a native mismatch is a hard error naming the
native component; it is not a warning and not an observational footnote.

## Write-once and verification

Use the same exclusive scientific write path as experiment artifacts: atomic
create/link, identical-content idempotency, fsync of file and parent directory.
The bundle directory is an experiment input. Inventories must list the native
binary. Symlinks, absolute paths, and undeclared files follow the v1 strict
inventory policy.

Re-running evaluation against an old checkpoint requires the matching bundle.
If the bundle is missing, fail closed: "native bytes were not archived; cannot
re-attest source_digest". Do not evaluate by ignoring `native_extension_digest`.

## Out of scope (rejected)

- Flags such as `--allow-native-mismatch`, `--observational-native`, or
  "best-effort" source identity.
- Rebuilding the extension from git and treating the new `.so` as the old
  epoch without a byte-identical digest match.
- Projecting a missing native archive into a Python-only source digest.
- Mutating existing scale-v2 checkpoints, manifests, or reports to the new
  bundle schema.

## Adoption

1. Parent approves this proposal.
2. Implement bundle creation as a separate change on a frozen clean checkout.
3. Archive the bundle before the next source-bound training epoch.
4. Keep today's `_source_digest` check unchanged until the archive exists;
   then verify against archived bytes rather than hoping the current build
   still matches.

This strengthens artifact preservation. It does not change the meaning of
`source_digest` on existing records.
