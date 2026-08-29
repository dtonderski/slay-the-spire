# Transform pool excludes upgraded sources by base cardID (FIDL00263)

## Rule

STS `AbstractDungeon.transformCard` excludes the source via shared `cardID`.
Upgraded copies must exclude the **base** content id from
`ironclad_transform_card_pool` / colorless transform pools.

## API

- `base_content_id(id)` — reverse of `upgrade_content_id`
- `ironclad_transform_card_pool` normalizes through `base_content_id` first
