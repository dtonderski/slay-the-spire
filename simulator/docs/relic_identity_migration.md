# Relic Identity Migration

The simulator uses `Relic` as its sole relic identity enum. The former
`RelicKey` enum is now a public type alias, preserving source compatibility and
serialized variant names without maintaining a second authority. Mark of the
Bloom, Spirit Poop, Odd Mushroom, and N'loth's Gift have canonical variants and
stable content IDs.

`RelicKey` remains temporarily as a public type alias so callers and serialized
enum variant names remain source- and wire-compatible. Compatibility
`key`/`from_key` methods are identity operations and must not decide whether a
relic can be owned.

Snapshot schema 4 merges historical `RunState.relic_keys` into `relics` and
folds paired reward offer fields into one `Relic` field at each stage. Schemas
1–3 migrate only at the validated snapshot boundary. Conflicting paired offers
and duplicate cross-store ownership fail closed. Current snapshots and raw run
state reject the retired fields rather than repairing them.

Static relic definitions will own content ID, tier, display/trace aliases, and
modeled-effect status. Unknown trace names remain unknown; recognizing a relic
identity must not imply that its gameplay effect is modeled.
