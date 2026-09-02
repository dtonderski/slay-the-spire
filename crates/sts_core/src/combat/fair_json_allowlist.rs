//! Path-sensitive JSON allowlists for fair observation and belief serialization.
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
    ("rampage_damage_bonus", Schema::Leaf),
    ("ritual_dagger_damage_bonus", Schema::Leaf),
    ("windmill_retain_damage", Schema::Leaf),
    ("steam_barrier_block_reduction", Schema::Leaf),
    ("combat_cost_under_turn_override", Schema::Leaf),
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
                ("damage", Schema::Leaf),
                ("hits", Schema::Leaf),
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

const PLAYER_CHOICE: Schema = Schema::Tagged {
    tag: "kind",
    variants: &[
        (
            "play_hand_slot",
            Schema::Object(&[
                ("kind", Schema::Leaf),
                ("hand_slot", Schema::Leaf),
                ("target_slot", Schema::Leaf),
            ]),
        ),
        ("end_turn", Schema::Object(&[("kind", Schema::Leaf)])),
        (
            "use_potion_slot",
            Schema::Object(&[
                ("kind", Schema::Leaf),
                ("potion_slot", Schema::Leaf),
                ("target_slot", Schema::Leaf),
            ]),
        ),
        (
            "discard_potion_slot",
            Schema::Object(&[("kind", Schema::Leaf), ("potion_slot", Schema::Leaf)]),
        ),
        (
            "toggle_visible_card",
            Schema::Object(&[("kind", Schema::Leaf), ("option_slot", Schema::Leaf)]),
        ),
        (
            "choose_visible_option",
            Schema::Object(&[("kind", Schema::Leaf), ("option_slot", Schema::Leaf)]),
        ),
        (
            "confirm_selection",
            Schema::Object(&[("kind", Schema::Leaf)]),
        ),
        ("skip_selection", Schema::Object(&[("kind", Schema::Leaf)])),
        ("proceed", Schema::Object(&[("kind", Schema::Leaf)])),
    ],
};

const PUBLIC_EVENT: Schema = Schema::Tagged {
    tag: "kind",
    variants: &[
        (
            "card_drawn",
            Schema::Object(&[("kind", Schema::Leaf), ("card", CARD)]),
        ),
        (
            "card_played",
            Schema::Object(&[("kind", Schema::Leaf), ("hand_slot", Schema::Leaf)]),
        ),
        (
            "card_moved",
            Schema::Object(&[
                ("kind", Schema::Leaf),
                ("card", CARD),
                ("from", Schema::Leaf),
                ("to", Schema::Leaf),
            ]),
        ),
        ("pile_shuffled", Schema::Object(&[("kind", Schema::Leaf)])),
        (
            "monster_move_executed",
            Schema::Object(&[
                ("kind", Schema::Leaf),
                ("monster_slot", Schema::Leaf),
                ("category", Schema::Leaf),
            ]),
        ),
        (
            "turn_started",
            Schema::Object(&[("kind", Schema::Leaf), ("turn", Schema::Leaf)]),
        ),
    ],
};

const PUBLIC_STEP: Schema = Schema::Object(&[
    ("action", PLAYER_CHOICE),
    ("events", Schema::Array(&PUBLIC_EVENT)),
    ("observation", FAIR_COMBAT_OBSERVATION_SCHEMA),
]);

const KNOWLEDGE: Schema = Schema::Object(&[
    ("schema_version", Schema::Leaf),
    ("initial_observation", FAIR_COMBAT_OBSERVATION_SCHEMA),
    ("history", Schema::Array(&PUBLIC_STEP)),
]);

const PRIOR: Schema = Schema::Object(&[
    ("schema_version", Schema::Leaf),
    ("prior_version", Schema::Leaf),
]);

const NAMED_DRAW: Schema = Schema::Object(&[("name", Schema::Leaf), ("draw_count", Schema::Leaf)]);

const BELIEF_RNG: Schema = Schema::Object(&[
    ("seed", Schema::Leaf),
    ("named_draws", Schema::Array(&NAMED_DRAW)),
]);

pub(crate) const FAIR_BELIEF_SCHEMA: Schema = Schema::Object(&[
    ("schema_version", Schema::Leaf),
    ("knowledge", KNOWLEDGE),
    ("prior", PRIOR),
    ("belief_rng", BELIEF_RNG),
]);
