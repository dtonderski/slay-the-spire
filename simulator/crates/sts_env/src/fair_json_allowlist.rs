//! Path-sensitive JSON allowlists for fair observation serialization.
//!
//! A flat union of key names is not an allowlist: hidden fields could reuse
//! already-allowed names such as `seed`, `state`, or `value` at a different path.
//! Each object path has its own permitted keys and nested schemas.

#![cfg(test)]

use serde_json::Value;

#[derive(Clone, Copy)]
pub(crate) enum Schema {
    Leaf,
    Optional(&'static Schema),
    Object(&'static [(&'static str, Schema)]),
    Array(&'static Schema),
    Tagged {
        tag: &'static str,
        variants: &'static [(&'static str, Schema)],
    },
}

pub(crate) fn check_schema(value: &Value, schema: &Schema, path: &str) -> Result<(), String> {
    match (schema, value) {
        (Schema::Leaf, Value::Object(_) | Value::Array(_)) => Err(format!(
            "{path}: expected a JSON leaf, got {}",
            value_kind(value)
        )),
        (Schema::Leaf, _) => Ok(()),
        (Schema::Optional(_), Value::Null) => Ok(()),
        (Schema::Optional(inner), other) => check_schema(other, inner, path),
        (Schema::Array(item), Value::Array(items)) => items
            .iter()
            .enumerate()
            .try_for_each(|(index, child)| check_schema(child, item, &format!("{path}[{index}]"))),
        (Schema::Array(_), other) => Err(format!(
            "{path}: expected a JSON array, got {}",
            value_kind(other)
        )),
        (Schema::Object(fields), Value::Object(map)) => check_object(map, fields, path),
        (Schema::Object(_), other) => Err(format!(
            "{path}: expected a JSON object, got {}",
            value_kind(other)
        )),
        (Schema::Tagged { tag, variants }, Value::Object(map)) => {
            let discriminant = map
                .get(*tag)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{path}: missing tagged field `{tag}`"))?;
            let variant = variants
                .iter()
                .find(|(name, _)| *name == discriminant)
                .ok_or_else(|| format!("{path}: unknown `{tag}` variant `{discriminant}`"))?;
            match variant.1 {
                Schema::Object(fields) => check_object(map, fields, path),
                other => check_schema(value, &other, path),
            }
        }
        (Schema::Tagged { .. }, other) => Err(format!(
            "{path}: expected a tagged JSON object, got {}",
            value_kind(other)
        )),
    }
}

fn check_object(
    map: &serde_json::Map<String, Value>,
    fields: &[(&str, Schema)],
    path: &str,
) -> Result<(), String> {
    for (key, schema) in fields {
        if !matches!(schema, Schema::Optional(_)) && !map.contains_key(*key) {
            return Err(format!("{path}: missing required field `{key}`"));
        }
    }
    for (key, child) in map {
        let schema = fields
            .iter()
            .find(|(allowed, _)| *allowed == key)
            .map(|(_, schema)| schema)
            .ok_or_else(|| format!("{path}.{key}: key is not allowlisted at this path"))?;
        check_schema(child, schema, &format!("{path}.{key}"))?;
    }
    Ok(())
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

const COUNTER: Schema = Schema::Object(&[("key", Schema::Leaf), ("value", Schema::Leaf)]);
const POWER: Schema = Schema::Object(&[("key", Schema::Leaf), ("amount", Schema::Leaf)]);
const DYNAMIC: Schema = Schema::Object(&[
    ("rampage_damage_bonus", Schema::Optional(&Schema::Leaf)),
    (
        "ritual_dagger_damage_bonus",
        Schema::Optional(&Schema::Leaf),
    ),
    ("windmill_retain_damage", Schema::Optional(&Schema::Leaf)),
    (
        "steam_barrier_block_reduction",
        Schema::Optional(&Schema::Leaf),
    ),
    (
        "combat_cost_under_turn_override",
        Schema::Optional(&Schema::Leaf),
    ),
]);
const CARD: Schema = Schema::Object(&[
    ("content_key", Schema::Leaf),
    ("cost", Schema::Leaf),
    ("cost_is_modified", Schema::Leaf),
    ("cost_resets_next_turn", Schema::Leaf),
    ("upgrade_level", Schema::Leaf),
    ("bottled", Schema::Leaf),
    ("temporary", Schema::Leaf),
    ("dynamic", DYNAMIC),
]);
const HAND_CARD: Schema = Schema::Object(&[("slot", Schema::Leaf), ("card", CARD)]);
const PILE: Schema = Schema::Object(&[
    ("count", Schema::Leaf),
    ("cards", Schema::Array(&CARD)),
    ("known_order", Schema::Array(&CARD)),
]);
const ORB: Schema = Schema::Tagged {
    tag: "type",
    variants: &[
        ("lightning", Schema::Object(&[("type", Schema::Leaf)])),
        ("frost", Schema::Object(&[("type", Schema::Leaf)])),
        (
            "dark",
            Schema::Object(&[("type", Schema::Leaf), ("evoke", Schema::Leaf)]),
        ),
    ],
};
const ORB_SLOT: Schema = Schema::Object(&[("slot", Schema::Leaf), ("orb", Schema::Optional(&ORB))]);
const INTENT: Schema = Schema::Tagged {
    tag: "visibility",
    variants: &[
        ("hidden", Schema::Object(&[("visibility", Schema::Leaf)])),
        ("none", Schema::Object(&[("visibility", Schema::Leaf)])),
        (
            "visible",
            Schema::Object(&[
                ("visibility", Schema::Leaf),
                ("category", Schema::Leaf),
                ("damage", Schema::Optional(&Schema::Leaf)),
                ("hits", Schema::Optional(&Schema::Leaf)),
            ]),
        ),
    ],
};
const MONSTER: Schema = Schema::Object(&[
    ("slot", Schema::Leaf),
    ("content_key", Schema::Leaf),
    ("slime_size", Schema::Optional(&Schema::Leaf)),
    ("hp", Schema::Leaf),
    ("max_hp", Schema::Leaf),
    ("block", Schema::Leaf),
    ("powers", Schema::Array(&POWER)),
    ("stolen_gold", Schema::Leaf),
    ("stasis_card", Schema::Optional(&CARD)),
    ("intent", INTENT),
    ("alive", Schema::Leaf),
    ("escaped", Schema::Leaf),
    ("minion", Schema::Leaf),
    ("targetable", Schema::Leaf),
    ("in_defensive_mode", Schema::Leaf),
]);
const RELIC: Schema = Schema::Object(&[
    ("slot", Schema::Leaf),
    ("content_key", Schema::Leaf),
    ("state", Schema::Array(&COUNTER)),
]);
const POTION_SLOT: Schema = Schema::Object(&[
    ("slot", Schema::Leaf),
    ("content_key", Schema::Optional(&Schema::Leaf)),
]);
const SELECTION_OPTION: Schema = Schema::Object(&[("slot", Schema::Leaf), ("card", CARD)]);
const SELECTION: Schema = Schema::Object(&[
    ("kind", Schema::Leaf),
    ("options", Schema::Array(&SELECTION_OPTION)),
    ("selected_slots", Schema::Array(&Schema::Leaf)),
]);
const PLAYER: Schema = Schema::Object(&[
    ("hp", Schema::Leaf),
    ("max_hp", Schema::Leaf),
    ("block", Schema::Leaf),
    ("energy", Schema::Leaf),
    ("max_energy", Schema::Leaf),
    ("powers", Schema::Array(&POWER)),
]);
const CONTEXT: Schema = Schema::Object(&[
    ("ascension", Schema::Leaf),
    ("act", Schema::Leaf),
    ("floor", Schema::Leaf),
    ("gold", Schema::Leaf),
]);

pub(crate) const FAIR_COMBAT_OBSERVATION_SCHEMA: Schema = Schema::Object(&[
    ("schema_version", Schema::Leaf),
    ("context", CONTEXT),
    ("phase", Schema::Leaf),
    ("player", PLAYER),
    ("orb_slots", Schema::Array(&ORB_SLOT)),
    ("hand", Schema::Array(&HAND_CARD)),
    ("draw_pile", PILE),
    ("discard_pile", PILE),
    ("exhaust_pile", PILE),
    ("monsters", Schema::Array(&MONSTER)),
    ("relics", Schema::Array(&RELIC)),
    ("potion_slots", Schema::Array(&POTION_SLOT)),
    ("selection", Schema::Optional(&SELECTION)),
    ("public_counters", Schema::Array(&COUNTER)),
]);

const RUN_RELIC: Schema = Schema::Object(&[("slot", Schema::Leaf), ("content_key", Schema::Leaf)]);
const RUN_CONTEXT: Schema = Schema::Object(&[
    ("ascension", Schema::Leaf),
    ("act", Schema::Leaf),
    ("floor", Schema::Leaf),
    ("gold", Schema::Leaf),
    ("player_hp", Schema::Leaf),
    ("player_max_hp", Schema::Leaf),
    ("deck", Schema::Array(&CARD)),
    ("relics", Schema::Array(&RUN_RELIC)),
    ("potion_slots", Schema::Array(&POTION_SLOT)),
]);
const CARD_SLOT: Schema = Schema::Object(&[("slot", Schema::Leaf), ("card", CARD)]);
const MAP_NODE: Schema = Schema::Object(&[
    ("slot", Schema::Leaf),
    ("act", Schema::Leaf),
    ("room_kind", Schema::Leaf),
    ("children", Schema::Array(&Schema::Leaf)),
]);
const MAP_SCREEN: Schema = Schema::Object(&[
    ("act", Schema::Leaf),
    ("floor", Schema::Leaf),
    ("current_node", Schema::Leaf),
    ("reachable_nodes", Schema::Array(&Schema::Leaf)),
    ("nodes", Schema::Array(&MAP_NODE)),
]);
const EVENT_CHOICE: Schema = Schema::Object(&[("slot", Schema::Leaf), ("label", Schema::Leaf)]);
const MATCH_CARD: Schema = Schema::Object(&[
    ("content_key", Schema::Optional(&Schema::Leaf)),
    ("revealed", Schema::Leaf),
    ("matched", Schema::Leaf),
]);
const EVENT_SCREEN: Schema = Schema::Object(&[
    ("event", Schema::Leaf),
    ("choices", Schema::Array(&EVENT_CHOICE)),
    (
        "match_and_keep",
        Schema::Optional(&Schema::Array(&MATCH_CARD)),
    ),
]);
const QUEUED_REWARD: Schema =
    Schema::Object(&[("slot", Schema::Leaf), ("choice_count", Schema::Leaf)]);
const REWARD_SCREEN: Schema = Schema::Object(&[
    ("cards", Schema::Array(&CARD_SLOT)),
    ("queued_card_rewards", Schema::Array(&QUEUED_REWARD)),
    ("gold_offer", Schema::Leaf),
    ("stolen_gold_offer", Schema::Leaf),
    ("potion_offer", Schema::Optional(&Schema::Leaf)),
    ("potion_offers", Schema::Array(&Schema::Leaf)),
    ("relic_offer", Schema::Optional(&Schema::Leaf)),
    ("boss_relic_choices", Schema::Array(&Schema::Leaf)),
    ("card_reward_flow", Schema::Leaf),
]);
const TREASURE_SCREEN: Schema =
    Schema::Object(&[("chest_size", Schema::Leaf), ("opened", Schema::Leaf)]);
const REST_OPTION: Schema = Schema::Tagged {
    tag: "kind",
    variants: &[
        ("heal", Schema::Object(&[("kind", Schema::Leaf)])),
        ("open_smith", Schema::Object(&[("kind", Schema::Leaf)])),
        ("open_remove", Schema::Object(&[("kind", Schema::Leaf)])),
        (
            "smith",
            Schema::Object(&[("kind", Schema::Leaf), ("card_slot", Schema::Leaf)]),
        ),
        (
            "remove_card",
            Schema::Object(&[("kind", Schema::Leaf), ("card_slot", Schema::Leaf)]),
        ),
        ("lift", Schema::Object(&[("kind", Schema::Leaf)])),
        ("dig", Schema::Object(&[("kind", Schema::Leaf)])),
        ("recall", Schema::Object(&[("kind", Schema::Leaf)])),
        ("proceed", Schema::Object(&[("kind", Schema::Leaf)])),
    ],
};
const REST_SCREEN: Schema = Schema::Object(&[
    ("complete", Schema::Leaf),
    ("options", Schema::Array(&REST_OPTION)),
]);
const SHOP_CARD: Schema = Schema::Object(&[
    ("slot", Schema::Leaf),
    ("content_key", Schema::Leaf),
    ("price", Schema::Leaf),
    ("sold", Schema::Leaf),
]);
const SHOP_SCREEN: Schema = Schema::Object(&[
    ("merchant_open", Schema::Leaf),
    ("remove_cost", Schema::Optional(&Schema::Leaf)),
    ("cards", Schema::Array(&SHOP_CARD)),
    ("relics", Schema::Array(&SHOP_CARD)),
    ("potions", Schema::Array(&SHOP_CARD)),
]);
const GRID_SCREEN: Schema = Schema::Object(&[
    ("purpose", Schema::Leaf),
    ("cards", Schema::Array(&CARD_SLOT)),
    ("selected", Schema::Optional(&Schema::Leaf)),
    ("selected_indices", Schema::Array(&Schema::Leaf)),
]);
const RUN_SCREEN: Schema = Schema::Tagged {
    tag: "kind",
    variants: &[
        (
            "combat",
            Schema::Object(&[
                ("kind", Schema::Leaf),
                ("value", FAIR_COMBAT_OBSERVATION_SCHEMA),
            ]),
        ),
        (
            "map",
            Schema::Object(&[("kind", Schema::Leaf), ("value", MAP_SCREEN)]),
        ),
        (
            "event",
            Schema::Object(&[("kind", Schema::Leaf), ("value", EVENT_SCREEN)]),
        ),
        (
            "reward",
            Schema::Object(&[("kind", Schema::Leaf), ("value", REWARD_SCREEN)]),
        ),
        (
            "treasure",
            Schema::Object(&[("kind", Schema::Leaf), ("value", TREASURE_SCREEN)]),
        ),
        (
            "rest",
            Schema::Object(&[("kind", Schema::Leaf), ("value", REST_SCREEN)]),
        ),
        (
            "shop",
            Schema::Object(&[("kind", Schema::Leaf), ("value", SHOP_SCREEN)]),
        ),
        (
            "grid",
            Schema::Object(&[("kind", Schema::Leaf), ("value", GRID_SCREEN)]),
        ),
        ("idle", Schema::Object(&[("kind", Schema::Leaf)])),
        ("complete", Schema::Object(&[("kind", Schema::Leaf)])),
    ],
};
pub(crate) const FAIR_RUN_OBSERVATION_SCHEMA: Schema = Schema::Object(&[
    ("schema_version", Schema::Leaf),
    ("phase", Schema::Leaf),
    ("context", RUN_CONTEXT),
    ("screen", RUN_SCREEN),
]);
