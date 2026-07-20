# Relic Identity Migration

The simulator currently has two enums for one domain identity. `RelicKey`
contains every known relic, while `Relic` omits Mark of the Bloom, Spirit Poop,
Odd Mushroom, and N'loth's Gift. Run ownership and reward offers consequently
split modeled relics from those four identities, and verifier name conversion
maintains parallel tables.

The migration uses `Relic` as the sole identity enum. The four missing variants
receive stable content IDs and an explicit modeled-effect classification.
`RelicKey` remains temporarily as a public type alias so callers and serialized
enum variant names remain source- and wire-compatible while storage fields are
migrated. Compatibility `key`/`from_key` methods become identity operations and
must not decide whether a relic can be owned.

The next snapshot schema merges historical `RunState.relic_keys` into `relics`
and folds the paired reward offer fields into one `Relic` field at each stage.
Historical snapshots are migrated only at the validated snapshot boundary.
Current raw state does not repair dual or contradictory ownership.

Static relic definitions will own content ID, tier, display/trace aliases, and
modeled-effect status. Unknown trace names remain unknown; recognizing a relic
identity must not imply that its gameplay effect is modeled.
