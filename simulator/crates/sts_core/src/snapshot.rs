use crate::{CombatState, RunState, SimError, SimResult};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{error::Error, fmt};

pub const SNAPSHOT_SCHEMA_VERSION: u32 = 8;
pub const LEGACY_NEOWS_LAMENT_RELIC_SNAPSHOT_SCHEMA_VERSION: u32 = 7;
pub const PREVIOUS_SNAPSHOT_SCHEMA_VERSION: u32 = 6;
pub const LEGACY_COMBAT_DECISION_SNAPSHOT_SCHEMA_VERSION: u32 = 5;
pub const LEGACY_COMBUST_SNAPSHOT_SCHEMA_VERSION: u32 = 4;
pub const LEGACY_RELIC_STORAGE_SNAPSHOT_SCHEMA_VERSION: u32 = 3;
pub const LEGACY_REWARD_FLOW_SNAPSHOT_SCHEMA_VERSION: u32 = 2;
pub const LEGACY_VALIDATED_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot<T> {
    pub schema_version: u32,
    pub state: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotHash(u64);

#[derive(Debug)]
pub enum SnapshotRestoreError {
    Json(serde_json::Error),
    InvalidDocument(&'static str),
    UnsupportedSchemaVersion(u32),
    InvalidState(SimError),
}

impl<T> Snapshot<T>
where
    T: Serialize,
{
    pub fn canonical_json(&self) -> SimResult<String> {
        serde_json::to_string(self)
            .map_err(|_| SimError::InvalidState("snapshot serialization failed"))
    }

    pub fn hash(&self) -> SimResult<SnapshotHash> {
        Ok(SnapshotHash(stable_hash64(
            self.canonical_json()?.as_bytes(),
        )))
    }
}

impl SnapshotHash {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for SnapshotHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl fmt::Display for SnapshotRestoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "invalid snapshot JSON: {error}"),
            Self::InvalidDocument(message) => write!(f, "invalid snapshot document: {message}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(f, "unsupported snapshot schema version: {version}")
            }
            Self::InvalidState(error) => write!(f, "invalid snapshot state: {error}"),
        }
    }
}

impl Error for SnapshotRestoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::InvalidState(error) => Some(error),
            Self::InvalidDocument(_) | Self::UnsupportedSchemaVersion(_) => None,
        }
    }
}

impl From<serde_json::Error> for SnapshotRestoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

fn schema_version(value: &Value) -> Result<u32, SnapshotRestoreError> {
    value
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .ok_or(SnapshotRestoreError::InvalidDocument(
            "schema_version must be a u32",
        ))
}

fn validate_supported_schema(version: u32) -> Result<(), SnapshotRestoreError> {
    if matches!(
        version,
        SNAPSHOT_SCHEMA_VERSION
            | LEGACY_NEOWS_LAMENT_RELIC_SNAPSHOT_SCHEMA_VERSION
            | PREVIOUS_SNAPSHOT_SCHEMA_VERSION
            | LEGACY_COMBAT_DECISION_SNAPSHOT_SCHEMA_VERSION
            | LEGACY_COMBUST_SNAPSHOT_SCHEMA_VERSION
            | LEGACY_RELIC_STORAGE_SNAPSHOT_SCHEMA_VERSION
            | LEGACY_REWARD_FLOW_SNAPSHOT_SCHEMA_VERSION
            | LEGACY_VALIDATED_SNAPSHOT_SCHEMA_VERSION
    ) {
        Ok(())
    } else {
        Err(SnapshotRestoreError::UnsupportedSchemaVersion(version))
    }
}

const LEGACY_COMBAT_DECISION_FIELDS: [&str; 10] = [
    "potion_card_reward",
    "potion_card_reward_kind",
    "toolbox_card_reward",
    "discovery_card_reward",
    "discovery_source_card",
    "hand_select",
    "pending_after_hand_select_actions",
    "draw_select",
    "discard_select",
    "exhaust_select",
];

fn take_optional_field(object: &mut Map<String, Value>, field: &str) -> Option<Value> {
    object.remove(field).filter(|value| !value.is_null())
}

fn tagged_decision(
    kind: &'static str,
    fields: impl IntoIterator<Item = (&'static str, Value)>,
) -> Value {
    let mut decision = Map::new();
    decision.insert("kind".to_owned(), Value::String(kind.to_owned()));
    for (field, value) in fields {
        decision.insert(field.to_owned(), value);
    }
    Value::Object(decision)
}

fn migrate_legacy_combat_decisions(combat: &mut Value) -> Result<(), SnapshotRestoreError> {
    let object = combat
        .as_object_mut()
        .ok_or(SnapshotRestoreError::InvalidDocument(
            "combat state must be an object",
        ))?;

    let potion_choices = take_optional_field(object, "potion_card_reward");
    let potion_kind = take_optional_field(object, "potion_card_reward_kind");
    let toolbox_choices = take_optional_field(object, "toolbox_card_reward");
    let discovery_choices = take_optional_field(object, "discovery_card_reward");
    let discovery_source = take_optional_field(object, "discovery_source_card");
    let hand_select = take_optional_field(object, "hand_select");
    let pending_actions = object
        .remove("pending_after_hand_select_actions")
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let draw_select = take_optional_field(object, "draw_select");
    let discard_select = take_optional_field(object, "discard_select");
    let exhaust_select = take_optional_field(object, "exhaust_select");

    if potion_choices.is_none() && potion_kind.is_some() {
        return Err(SnapshotRestoreError::InvalidDocument(
            "legacy potion reward kind has no reward choices",
        ));
    }
    if potion_choices.is_some() && potion_kind.is_none() {
        return Err(SnapshotRestoreError::InvalidDocument(
            "legacy potion reward choices have no reward kind",
        ));
    }
    if discovery_choices.is_none() && discovery_source.is_some() {
        return Err(SnapshotRestoreError::InvalidDocument(
            "legacy Discovery source has no reward choices",
        ));
    }
    let pending_actions_are_empty = pending_actions.as_array().is_some_and(Vec::is_empty);
    if hand_select.is_none() && !pending_actions_are_empty {
        return Err(SnapshotRestoreError::InvalidDocument(
            "legacy pending hand-select actions have no hand selection",
        ));
    }

    let mut decisions = Vec::new();
    if let (Some(choices), Some(reward_kind)) = (potion_choices, potion_kind) {
        decisions.push(tagged_decision(
            "potion_card_reward",
            [("choices", choices), ("reward_kind", reward_kind)],
        ));
    }
    if let Some(choices) = toolbox_choices {
        decisions.push(tagged_decision(
            "toolbox_card_reward",
            [("choices", choices)],
        ));
    }
    if let Some(choices) = discovery_choices {
        let mut fields = vec![("choices", choices)];
        if let Some(source) = discovery_source {
            fields.push(("source_card", source));
        }
        decisions.push(tagged_decision("discovery_card_reward", fields));
    }
    if let Some(state) = hand_select {
        decisions.push(tagged_decision(
            "hand_select",
            [("state", state), ("pending_actions", pending_actions)],
        ));
    }
    for (kind, state) in [
        ("draw_select", draw_select),
        ("discard_select", discard_select),
        ("exhaust_select", exhaust_select),
    ] {
        if let Some(state) = state {
            decisions.push(tagged_decision(kind, [("state", state)]));
        }
    }

    if decisions.is_empty() {
        return Ok(());
    }
    if object.get("decision").is_some_and(|value| !value.is_null())
        || object
            .get("queued_decisions")
            .is_some_and(|value| value.as_array().is_none_or(|queue| !queue.is_empty()))
    {
        return Err(SnapshotRestoreError::InvalidDocument(
            "snapshot contains both canonical and legacy combat decisions",
        ));
    }

    let mut decisions = decisions.into_iter();
    let active = decisions
        .next()
        .ok_or(SnapshotRestoreError::InvalidDocument(
            "legacy combat decision migration produced no active decision",
        ))?;
    object.insert("decision".to_owned(), active);
    object.insert(
        "queued_decisions".to_owned(),
        Value::Array(decisions.collect()),
    );
    Ok(())
}

fn reject_legacy_combat_decisions(combat: &Value) -> Result<(), SnapshotRestoreError> {
    let object = combat
        .as_object()
        .ok_or(SnapshotRestoreError::InvalidDocument(
            "combat state must be an object",
        ))?;
    if LEGACY_COMBAT_DECISION_FIELDS
        .iter()
        .any(|field| object.contains_key(*field))
    {
        return Err(SnapshotRestoreError::InvalidDocument(
            "current snapshot contains retired combat decision fields",
        ));
    }
    Ok(())
}

fn migrate_legacy_combust_damage(combat: &mut Value) -> Result<(), SnapshotRestoreError> {
    let Some(powers) = combat
        .pointer_mut("/player/powers")
        .and_then(Value::as_object_mut)
    else {
        return Ok(());
    };
    let Some(stacks) = powers.get("combust").and_then(Value::as_i64) else {
        return Ok(());
    };
    if stacks <= 0 {
        return Ok(());
    }
    let missing_or_zero = match powers.get("combust_damage") {
        None => true,
        Some(value) => value.as_i64() == Some(0),
    };
    if !missing_or_zero {
        return Ok(());
    }

    let damage = stacks
        .checked_mul(i64::from(crate::content::cards::COMBUST_DAMAGE))
        .ok_or(SnapshotRestoreError::InvalidDocument(
            "legacy Combust damage overflows i64",
        ))?;
    powers.insert("combust_damage".to_owned(), Value::from(damage));
    Ok(())
}

fn legacy_reward_count(
    reward: &mut Map<String, Value>,
    field: &'static str,
) -> Result<u8, SnapshotRestoreError> {
    let Some(value) = reward.remove(field) else {
        return Ok(0);
    };
    value
        .as_u64()
        .and_then(|count| u8::try_from(count).ok())
        .ok_or(SnapshotRestoreError::InvalidDocument(
            "legacy card reward count must be a u8",
        ))
}

fn legacy_reward_flag(
    reward: &mut Map<String, Value>,
    field: &'static str,
) -> Result<bool, SnapshotRestoreError> {
    let Some(value) = reward.remove(field) else {
        return Ok(false);
    };
    value.as_bool().ok_or(SnapshotRestoreError::InvalidDocument(
        "legacy card reward flag must be boolean",
    ))
}

fn migrate_legacy_reward_flow(value: &mut Value) -> Result<(), SnapshotRestoreError> {
    let reward = value
        .get_mut("state")
        .and_then(|state| state.get_mut("reward"));
    let Some(Value::Object(reward)) = reward else {
        return Ok(());
    };

    // A current-shape value relabeled by a caller as historical needs no value
    // conversion. Real version 1/2 snapshots contain the legacy fields below.
    if reward.contains_key("card_reward_flow") {
        return Ok(());
    }

    let active = legacy_reward_flag(reward, "card_reward_active")?;
    let pending = legacy_reward_flag(reward, "card_reward_pending")?;
    let legacy_count = legacy_reward_count(reward, "pending_card_reward_count")?;
    let remaining = if legacy_count > 0 {
        legacy_count
    } else if active || pending {
        1
    } else {
        0
    };

    let mut flow = Map::new();
    if active {
        flow.insert("state".to_owned(), Value::String("active".to_owned()));
        flow.insert("remaining".to_owned(), Value::from(remaining));
    } else if pending || remaining > 0 {
        flow.insert("state".to_owned(), Value::String("pending".to_owned()));
        flow.insert("remaining".to_owned(), Value::from(remaining));
    } else {
        flow.insert("state".to_owned(), Value::String("none".to_owned()));
    }
    reward.insert("card_reward_flow".to_owned(), Value::Object(flow));
    Ok(())
}

fn merge_optional_legacy_field(
    object: &mut Map<String, Value>,
    current: &'static str,
    legacy: &'static str,
    conflict: &'static str,
) -> Result<(), SnapshotRestoreError> {
    let Some(legacy_value) = object.remove(legacy) else {
        return Ok(());
    };
    if legacy_value.is_null() {
        return Ok(());
    }
    if object.get(current).is_some_and(|value| !value.is_null()) {
        return Err(SnapshotRestoreError::InvalidDocument(conflict));
    }
    object.insert(current.to_owned(), legacy_value);
    Ok(())
}

fn merge_legacy_relic_queue(reward: &mut Map<String, Value>) -> Result<(), SnapshotRestoreError> {
    let Some(legacy_value) = reward.remove("queued_relic_key_offers") else {
        return Ok(());
    };
    let Value::Array(legacy) = legacy_value else {
        return Err(SnapshotRestoreError::InvalidDocument(
            "legacy queued relic offers must be an array",
        ));
    };
    if legacy.is_empty() {
        return Ok(());
    }
    match reward.get("queued_relic_offers") {
        None | Some(Value::Null) => {
            reward.insert("queued_relic_offers".to_owned(), Value::Array(legacy));
            Ok(())
        }
        Some(Value::Array(current)) if current.is_empty() => {
            reward.insert("queued_relic_offers".to_owned(), Value::Array(legacy));
            Ok(())
        }
        Some(Value::Array(_)) => Err(SnapshotRestoreError::InvalidDocument(
            "snapshot contains both canonical and legacy queued relic offers",
        )),
        Some(_) => Err(SnapshotRestoreError::InvalidDocument(
            "queued relic offers must be an array",
        )),
    }
}

fn migrate_legacy_relic_storage(value: &mut Value) -> Result<(), SnapshotRestoreError> {
    let state = value
        .get_mut("state")
        .and_then(Value::as_object_mut)
        .ok_or(SnapshotRestoreError::InvalidDocument(
            "state must be an object",
        ))?;

    let legacy_relics = match state.remove("relic_keys") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(relics)) => relics,
        Some(_) => {
            return Err(SnapshotRestoreError::InvalidDocument(
                "legacy relic_keys must be an array",
            ));
        }
    };
    let relics = state
        .entry("relics".to_owned())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or(SnapshotRestoreError::InvalidDocument(
            "relics must be an array",
        ))?;
    for relic in legacy_relics {
        if relics.contains(&relic) {
            return Err(SnapshotRestoreError::InvalidDocument(
                "snapshot owns the same relic through both legacy stores",
            ));
        }
        relics.push(relic);
    }

    merge_optional_legacy_field(
        state,
        "pending_event_combat_relic_offer",
        "pending_event_combat_relic_key_offer",
        "snapshot contains both canonical and legacy pending event relic offers",
    )?;

    if let Some(Value::Object(reward)) = state.get_mut("reward") {
        merge_optional_legacy_field(
            reward,
            "relic_offer",
            "relic_key_offer",
            "snapshot contains both canonical and legacy relic offers",
        )?;
        merge_optional_legacy_field(
            reward,
            "pending_relic_offer",
            "pending_relic_key_offer",
            "snapshot contains both canonical and legacy pending relic offers",
        )?;
        merge_legacy_relic_queue(reward)?;
    }
    Ok(())
}

fn migrate_legacy_neows_lament_relic(value: &mut Value) -> Result<(), SnapshotRestoreError> {
    let state = value
        .get_mut("state")
        .and_then(Value::as_object_mut)
        .ok_or(SnapshotRestoreError::InvalidDocument(
            "state must be an object",
        ))?;
    let remaining = match state.get("neow_lament_combats_remaining") {
        None => 0,
        Some(value) => value
            .as_u64()
            .filter(|remaining| u32::try_from(*remaining).is_ok())
            .ok_or(SnapshotRestoreError::InvalidDocument(
                "neow_lament_combats_remaining must be a u32",
            ))?,
    };
    if remaining == 0 {
        return Ok(());
    }
    let relics = state
        .get_mut("relics")
        .and_then(Value::as_array_mut)
        .ok_or(SnapshotRestoreError::InvalidDocument(
            "relics must be an array",
        ))?;
    let identity = Value::String("NeowsLament".to_owned());
    if !relics.contains(&identity) {
        relics.insert(usize::min(1, relics.len()), identity);
    }
    Ok(())
}

fn migrate_legacy_reward_continuation(value: &mut Value) -> Result<(), SnapshotRestoreError> {
    let state = value
        .get_mut("state")
        .and_then(Value::as_object_mut)
        .ok_or(SnapshotRestoreError::InvalidDocument(
            "state must be an object",
        ))?;
    let Some(reward) = state.get("reward").filter(|reward| !reward.is_null()) else {
        return Ok(());
    };
    let reward = reward
        .as_object()
        .ok_or(SnapshotRestoreError::InvalidDocument(
            "reward screen must be an object",
        ))?;

    match reward.get("continuation") {
        Some(Value::String(continuation)) if continuation != "None" => return Ok(()),
        None | Some(Value::String(_)) => {}
        Some(_) => {
            return Err(SnapshotRestoreError::InvalidDocument(
                "reward continuation must be a string",
            ));
        }
    }

    let mut owners = Vec::new();
    if let Some(event) = state.get("event").filter(|event| !event.is_null()) {
        let neow_exit = event.get("event").and_then(Value::as_str) == Some("Neow")
            && event.get("stage").and_then(Value::as_u64) == Some(2)
            && state
                .get("relics")
                .and_then(Value::as_array)
                .is_some_and(|relics| relics.contains(&Value::String("TinyHouse".to_owned())));
        owners.push(if neow_exit { "Neow" } else { "Event" });
    }
    if state.get("shop").is_some_and(|shop| !shop.is_null())
        && state
            .get("shop_merchant_open")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        owners.push("Shop");
    }
    if state
        .get("rest_room_complete")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        owners.push("Rest");
    }
    if state
        .get("treasure_room")
        .is_some_and(|treasure| !treasure.is_null())
    {
        owners.push("Map");
    }
    if state
        .get("boss_chest_opened")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        owners.push("Treasure");
    }

    match owners.as_slice() {
        [] | [_] => {}
        _ => {
            return Err(SnapshotRestoreError::InvalidDocument(
                "legacy reward has multiple possible continuation owners",
            ));
        }
    }
    let reward = state
        .get_mut("reward")
        .and_then(Value::as_object_mut)
        .ok_or(SnapshotRestoreError::InvalidDocument(
            "reward screen must be an object",
        ))?;
    if let [owner] = owners.as_slice() {
        reward.insert(
            "continuation".to_owned(),
            Value::String((*owner).to_owned()),
        );
    }
    Ok(())
}

pub fn restore_combat_snapshot_json(
    json: &str,
) -> Result<Snapshot<CombatState>, SnapshotRestoreError> {
    let mut value: Value = serde_json::from_str(json)?;
    let version = schema_version(&value)?;
    validate_supported_schema(version)?;
    if let Some(state) = value.get_mut("state") {
        if version <= LEGACY_COMBAT_DECISION_SNAPSHOT_SCHEMA_VERSION {
            migrate_legacy_combat_decisions(state)?;
        } else {
            reject_legacy_combat_decisions(state)?;
        }
        if version <= LEGACY_COMBUST_SNAPSHOT_SCHEMA_VERSION {
            migrate_legacy_combust_damage(state)?;
        }
    }
    let mut snapshot: Snapshot<CombatState> = serde_json::from_value(value)?;
    snapshot
        .state
        .validate()
        .map_err(SnapshotRestoreError::InvalidState)?;
    snapshot.schema_version = SNAPSHOT_SCHEMA_VERSION;
    Ok(snapshot)
}

pub fn restore_run_snapshot_json(json: &str) -> Result<Snapshot<RunState>, SnapshotRestoreError> {
    let mut value: Value = serde_json::from_str(json)?;
    let version = schema_version(&value)?;
    validate_supported_schema(version)?;
    if version <= PREVIOUS_SNAPSHOT_SCHEMA_VERSION {
        migrate_legacy_reward_continuation(&mut value)?;
    }
    if version <= LEGACY_RELIC_STORAGE_SNAPSHOT_SCHEMA_VERSION {
        migrate_legacy_reward_flow(&mut value)?;
        migrate_legacy_relic_storage(&mut value)?;
    }
    if version <= LEGACY_NEOWS_LAMENT_RELIC_SNAPSHOT_SCHEMA_VERSION {
        migrate_legacy_neows_lament_relic(&mut value)?;
    }
    if let Some(combat) = value
        .pointer_mut("/state/combat")
        .filter(|combat| !combat.is_null())
    {
        if version <= LEGACY_COMBAT_DECISION_SNAPSHOT_SCHEMA_VERSION {
            migrate_legacy_combat_decisions(combat)?;
        } else {
            reject_legacy_combat_decisions(combat)?;
        }
        if version <= LEGACY_COMBUST_SNAPSHOT_SCHEMA_VERSION {
            migrate_legacy_combust_damage(combat)?;
        }
    }
    let mut snapshot: Snapshot<RunState> = serde_json::from_value(value)?;
    snapshot
        .state
        .validate()
        .map_err(SnapshotRestoreError::InvalidState)?;
    snapshot.schema_version = SNAPSHOT_SCHEMA_VERSION;
    Ok(snapshot)
}

fn stable_hash64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        combat::{ExhaustSelectPurpose, ExhaustSelectState, HandSelectState},
        content::cards::STRIKE_R_ID,
        CardId, CardInstance, CardRewardFlow, CombatDecisionState, Relic, RewardContinuation,
        RewardScreen, RoomKind, RunPhase,
    };
    use serde_json::json;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct EmptySnapshotState {}

    fn empty_snapshot() -> Snapshot<EmptySnapshotState> {
        Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: EmptySnapshotState {},
        }
    }

    #[test]
    fn schema_seven_active_neows_lament_migrates_to_owned_relic_order() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::BurningBlood];
        run.relics.push(Relic::NeowsLament);
        run.relics.push(Relic::Lantern);
        run.neow_lament_combats_remaining = 2;
        let snapshot = Snapshot {
            schema_version: LEGACY_NEOWS_LAMENT_RELIC_SNAPSHOT_SCHEMA_VERSION,
            state: run,
        };
        let mut value = serde_json::to_value(snapshot).expect("snapshot serializes");
        value["state"]["relics"]
            .as_array_mut()
            .expect("relics are an array")
            .retain(|relic| relic != "NeowsLament");

        let restored = restore_run_snapshot_json(&value.to_string())
            .expect("schema-seven Neow's Lament identity migrates");

        assert_eq!(restored.schema_version, SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(
            restored.state.relics,
            vec![Relic::BurningBlood, Relic::NeowsLament, Relic::Lantern]
        );
    }

    fn legacy_run_snapshot_json(version: u32, active: bool, pending: bool, count: u8) -> String {
        let snapshot = Snapshot {
            schema_version: version,
            state: RunState::seeded_ironclad(7, 0),
        };
        let mut value = serde_json::to_value(snapshot).expect("snapshot serializes");
        value["state"]["phase"] = Value::String("Reward".to_owned());
        value["state"]["event"] = Value::Null;
        let choices = if active {
            vec![value["state"]["deck"][0].clone()]
        } else {
            Vec::new()
        };
        value["state"]["reward"] = json!({
            "choices": choices,
            "gold_offer": 0,
            "potion_offer": null,
            "relic_offer": null,
            "card_reward_active": active,
            "card_reward_pending": pending,
            "pending_card_reward_count": count,
        });
        serde_json::to_string(&value).expect("legacy snapshot serializes")
    }

    fn reward_snapshot_value(version: u32) -> Value {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.phase = RunPhase::Reward;
        run.current_room_override = None;
        run.event = None;
        run.shop = None;
        run.shop_merchant_open = false;
        run.treasure_room = None;
        run.boss_chest_opened = false;
        run.rest_room_complete = false;
        run.reward = Some(RewardScreen {
            continuation: RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 0,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: None,
            pending_relic_offer: None,
            queued_relic_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_flow: CardRewardFlow::None,
        });
        serde_json::to_value(Snapshot {
            schema_version: version,
            state: run,
        })
        .expect("reward snapshot serializes")
    }

    #[test]
    fn same_snapshot_hashes_identically() {
        let first = empty_snapshot();
        let second = empty_snapshot();

        assert_eq!(
            first.hash().expect("first hashes"),
            second.hash().expect("second hashes")
        );
    }

    #[test]
    fn canonical_field_order_does_not_drift() {
        let snapshot = empty_snapshot();

        assert_eq!(
            snapshot.canonical_json().expect("snapshot serializes"),
            r#"{"schema_version":8,"state":{}}"#
        );
    }

    #[test]
    fn snapshot_round_trip_preserves_hash() {
        let snapshot = empty_snapshot();
        let before = snapshot.hash().expect("snapshot hashes");
        let json = snapshot.canonical_json().expect("snapshot serializes");
        let restored: Snapshot<EmptySnapshotState> =
            serde_json::from_str(&json).expect("snapshot deserializes");

        assert_eq!(restored, snapshot);
        assert_eq!(restored.hash().expect("restored hashes"), before);
    }

    #[test]
    fn schema_four_combat_snapshot_migrates_missing_combust_damage() {
        let mut combat = CombatState::initial_fixture();
        combat.player.powers.combust = 2;
        combat.player.powers.combust_damage = 10;
        let mut value = serde_json::to_value(Snapshot {
            schema_version: LEGACY_COMBUST_SNAPSHOT_SCHEMA_VERSION,
            state: combat,
        })
        .expect("combat snapshot serializes");
        value["state"]["player"]["powers"]
            .as_object_mut()
            .expect("powers object")
            .remove("combust_damage");

        let restored = restore_combat_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect("schema-four Combust state migrates");

        assert_eq!(restored.schema_version, SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(restored.state.player.powers.combust, 2);
        assert_eq!(restored.state.player.powers.combust_damage, 10);
    }

    #[test]
    fn schema_four_run_snapshot_migrates_nested_combust_damage() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        let mut combat = run
            .init_combat(CombatState::initial_fixture())
            .expect("combat initializes");
        combat.player.powers.combust = 1;
        combat.player.powers.combust_damage = 5;
        run.combat = Some(combat);
        let mut value = serde_json::to_value(Snapshot {
            schema_version: LEGACY_COMBUST_SNAPSHOT_SCHEMA_VERSION,
            state: run,
        })
        .expect("run snapshot serializes");
        value["state"]["combat"]["player"]["powers"]
            .as_object_mut()
            .expect("nested powers object")
            .remove("combust_damage");

        let restored = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect("schema-four nested Combust state migrates");

        assert_eq!(restored.schema_version, SNAPSHOT_SCHEMA_VERSION);
        let combat = restored.state.combat.expect("combat restored");
        assert_eq!(combat.player.powers.combust, 1);
        assert_eq!(combat.player.powers.combust_damage, 5);
    }

    #[test]
    fn current_combat_snapshot_rejects_missing_combust_damage() {
        let mut combat = CombatState::initial_fixture();
        combat.player.powers.combust = 1;
        combat.player.powers.combust_damage = 5;
        let mut value = serde_json::to_value(Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: combat,
        })
        .expect("combat snapshot serializes");
        value["state"]["player"]["powers"]
            .as_object_mut()
            .expect("powers object")
            .remove("combust_damage");

        let error = restore_combat_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("current Combust state must be canonical");

        assert!(matches!(error, SnapshotRestoreError::InvalidState(_)));
    }

    #[test]
    fn schema_five_combat_snapshot_migrates_hand_selection() {
        let combat = CombatState::initial_fixture();
        let source_card_id = combat.piles.hand[0].id;
        let mut value = serde_json::to_value(Snapshot {
            schema_version: LEGACY_COMBAT_DECISION_SNAPSHOT_SCHEMA_VERSION,
            state: combat,
        })
        .expect("combat snapshot serializes");
        value["state"]["hand_select"] = serde_json::to_value(HandSelectState {
            purpose: Default::default(),
            source_card_id,
            selected_hand_index: None,
            selected_hand_indices: Vec::new(),
        })
        .expect("hand selection serializes");

        let restored = restore_combat_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect("schema-five hand selection migrates");

        assert_eq!(restored.schema_version, SNAPSHOT_SCHEMA_VERSION);
        assert!(matches!(
            restored.state.decision,
            Some(CombatDecisionState::HandSelect { .. })
        ));
        assert!(restored.state.queued_decisions.is_empty());
    }

    #[test]
    fn schema_five_multiple_decisions_preserve_legacy_priority_in_queue() {
        let combat = CombatState::initial_fixture();
        let mut value = serde_json::to_value(Snapshot {
            schema_version: LEGACY_COMBAT_DECISION_SNAPSHOT_SCHEMA_VERSION,
            state: combat,
        })
        .expect("combat snapshot serializes");
        value["state"]["toolbox_card_reward"] =
            json!([CardInstance::new(CardId::new(900), STRIKE_R_ID)]);
        value["state"]["exhaust_select"] = serde_json::to_value(ExhaustSelectState {
            purpose: ExhaustSelectPurpose::GamblingChip,
            source_card_id: None,
            source_card: None,
            selected_hand_indices: Vec::new(),
        })
        .expect("exhaust selection serializes");

        let restored = restore_combat_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect("schema-five decisions migrate");

        assert!(matches!(
            restored.state.decision,
            Some(CombatDecisionState::ToolboxCardReward { .. })
        ));
        assert!(matches!(
            restored.state.queued_decisions.front(),
            Some(CombatDecisionState::ExhaustSelect { .. })
        ));
    }

    #[test]
    fn schema_five_run_snapshot_migrates_nested_combat_decision() {
        let run = RunState::combat_fixture();
        let source_card_id = run.combat.as_ref().expect("combat fixture").piles.hand[0].id;
        let mut value = serde_json::to_value(Snapshot {
            schema_version: LEGACY_COMBAT_DECISION_SNAPSHOT_SCHEMA_VERSION,
            state: run,
        })
        .expect("run snapshot serializes");
        value["state"]["combat"]["hand_select"] = serde_json::to_value(HandSelectState {
            purpose: Default::default(),
            source_card_id,
            selected_hand_index: None,
            selected_hand_indices: Vec::new(),
        })
        .expect("hand selection serializes");

        let restored = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect("nested schema-five decision migrates");

        assert!(matches!(
            restored.state.combat.expect("combat restored").decision,
            Some(CombatDecisionState::HandSelect { .. })
        ));
    }

    #[test]
    fn current_snapshot_rejects_retired_combat_decision_fields() {
        let combat = CombatState::initial_fixture();
        let mut value = serde_json::to_value(Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: combat,
        })
        .expect("combat snapshot serializes");
        value["state"]["hand_select"] = Value::Null;

        let error = restore_combat_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("current snapshots reject retired decision fields");

        assert!(matches!(error, SnapshotRestoreError::InvalidDocument(_)));
    }

    #[test]
    fn schema_six_reward_snapshots_derive_each_unambiguous_continuation() {
        let cases = [
            (
                RewardContinuation::Event,
                json!({
                    "event": crate::EventScreen {
                        event: crate::Event::SensoryStone,
                        choices: vec![crate::EventChoice { label: "Leave".to_owned() }],
                        stage: 2,
                        event_data: 0,
                    },
                }),
            ),
            (
                RewardContinuation::Shop,
                json!({
                    "shop": {
                        "cards": [],
                        "relics": [],
                        "potions": [],
                        "remove_cost": 75,
                        "remove_available": true,
                        "sale_slot": null
                    },
                    "shop_merchant_open": true,
                }),
            ),
            (
                RewardContinuation::Rest,
                json!({
                    "current_room_override": "Rest",
                    "rest_room_complete": true,
                }),
            ),
            (
                RewardContinuation::Map,
                json!({
                    "current_room_override": "Treasure",
                    "treasure_room": {
                        "chest_size": "Small",
                        "relic_tier": "Common",
                        "have_gold": false
                    },
                }),
            ),
            (
                RewardContinuation::Treasure,
                json!({
                    "current_room_override": "Boss",
                    "boss_chest_opened": true,
                }),
            ),
        ];

        for (expected, owner_fields) in cases {
            let mut value = reward_snapshot_value(PREVIOUS_SNAPSHOT_SCHEMA_VERSION);
            let state = value["state"].as_object_mut().expect("state object");
            state.extend(owner_fields.as_object().expect("owner fields").clone());

            let restored = restore_run_snapshot_json(
                &serde_json::to_string(&value).expect("snapshot value serializes"),
            )
            .expect("schema-six continuation migrates");

            assert_eq!(
                restored.state.reward.expect("reward restored").continuation,
                expected
            );
        }
    }

    #[test]
    fn schema_six_reward_snapshot_rejects_ambiguous_continuation_owner() {
        let mut value = reward_snapshot_value(PREVIOUS_SNAPSHOT_SCHEMA_VERSION);
        value["state"]["event"] =
            serde_json::to_value(crate::run::event::event_screen(crate::Event::Neow))
                .expect("event serializes");
        value["state"]["current_room_override"] = Value::String("Rest".to_owned());
        value["state"]["rest_room_complete"] = Value::Bool(true);

        let error = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("ambiguous legacy owner must fail closed");

        assert!(matches!(error, SnapshotRestoreError::InvalidDocument(_)));
    }

    #[test]
    fn schema_six_neow_tiny_house_reward_migrates_to_explicit_exit() {
        let mut value = reward_snapshot_value(PREVIOUS_SNAPSHOT_SCHEMA_VERSION);
        value["state"]["event"] = serde_json::to_value(crate::run::event::neow_screen_for_stage(
            &RunState::seeded_ironclad(7, 0),
            2,
        ))
        .expect("Neow leave screen serializes");
        value["state"]["relics"]
            .as_array_mut()
            .expect("relic array")
            .push(Value::String("TinyHouse".to_owned()));

        let restored = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect("schema-six Tiny House continuation migrates");
        assert_eq!(
            restored
                .state
                .reward
                .as_ref()
                .expect("reward restored")
                .continuation,
            RewardContinuation::Neow
        );

        let settled =
            crate::run::reward::apply_run_action(&restored.state, crate::RunAction::Proceed)
                .expect("Neow reward proceeds to the map");
        assert_eq!(settled.phase, RunPhase::Idle);
        assert!(settled.event.is_none());
        assert!(settled.reward.is_none());
    }

    #[test]
    fn current_reward_snapshot_rejects_retained_owner_without_continuation() {
        let mut value = reward_snapshot_value(SNAPSHOT_SCHEMA_VERSION);
        value["state"]["event"] = serde_json::to_value(crate::EventScreen {
            event: crate::Event::SensoryStone,
            choices: vec![crate::EventChoice {
                label: "Leave".to_owned(),
            }],
            stage: 2,
            event_data: 0,
        })
        .expect("event serializes");

        let error = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("current reward owner requires a typed continuation");

        assert!(matches!(error, SnapshotRestoreError::InvalidState(_)));
    }

    #[test]
    fn historical_run_snapshots_migrate_pending_card_reward_counts() {
        for version in [
            LEGACY_VALIDATED_SNAPSHOT_SCHEMA_VERSION,
            LEGACY_REWARD_FLOW_SNAPSHOT_SCHEMA_VERSION,
            LEGACY_RELIC_STORAGE_SNAPSHOT_SCHEMA_VERSION,
        ] {
            let restored =
                restore_run_snapshot_json(&legacy_run_snapshot_json(version, false, true, 2))
                    .expect("legacy pending reward migrates");

            assert_eq!(restored.schema_version, SNAPSHOT_SCHEMA_VERSION);
            assert_eq!(restored.state.phase, RunPhase::Reward);
            assert_eq!(
                restored.state.reward.expect("reward").card_reward_flow,
                CardRewardFlow::pending(2)
            );
        }
    }

    #[test]
    fn historical_active_flag_migrates_to_one_active_reward() {
        let restored = restore_run_snapshot_json(&legacy_run_snapshot_json(
            LEGACY_RELIC_STORAGE_SNAPSHOT_SCHEMA_VERSION,
            true,
            false,
            0,
        ))
        .expect("legacy active reward migrates");

        assert_eq!(
            restored.state.reward.expect("reward").card_reward_flow,
            CardRewardFlow::active(1)
        );
    }

    #[test]
    fn historical_nonzero_count_remains_authoritative() {
        let restored = restore_run_snapshot_json(&legacy_run_snapshot_json(
            LEGACY_RELIC_STORAGE_SNAPSHOT_SCHEMA_VERSION,
            false,
            false,
            3,
        ))
        .expect("legacy count migrates");

        assert_eq!(
            restored.state.reward.expect("reward").card_reward_flow,
            CardRewardFlow::pending(3)
        );
    }

    #[test]
    fn current_schema_rejects_legacy_reward_fields() {
        let error = restore_run_snapshot_json(&legacy_run_snapshot_json(
            SNAPSHOT_SCHEMA_VERSION,
            false,
            true,
            1,
        ))
        .expect_err("current snapshots must use the current reward shape");

        assert!(matches!(error, SnapshotRestoreError::Json(_)));
    }

    #[test]
    fn schema_three_merges_relic_ownership_and_offer_fields() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.relics.push(Relic::SpiritPoop);
        run.pending_event_combat_relic_offer = Some(Relic::OddMushroom);
        run.phase = RunPhase::Reward;
        run.event = None;
        run.reward = Some(RewardScreen {
            continuation: RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 0,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: Some(Relic::MarkOfBloom),
            pending_relic_offer: Some(Relic::NlothsGift),
            queued_relic_offers: vec![Relic::TheBoot],
            boss_relic_choices: Vec::new(),
            card_reward_flow: CardRewardFlow::None,
        });
        let mut value = serde_json::to_value(Snapshot {
            schema_version: LEGACY_RELIC_STORAGE_SNAPSHOT_SCHEMA_VERSION,
            state: run,
        })
        .expect("snapshot serializes");

        let relics = value["state"]["relics"]
            .as_array_mut()
            .expect("relics are an array");
        let spirit_poop = relics.pop().expect("event relic is present");
        value["state"]["relic_keys"] = Value::Array(vec![spirit_poop]);
        let pending_event = value["state"]
            .as_object_mut()
            .expect("state object")
            .remove("pending_event_combat_relic_offer")
            .expect("pending event offer");
        value["state"]["pending_event_combat_relic_key_offer"] = pending_event;
        let reward = value["state"]["reward"]
            .as_object_mut()
            .expect("reward object");
        for (current, legacy) in [
            ("relic_offer", "relic_key_offer"),
            ("pending_relic_offer", "pending_relic_key_offer"),
            ("queued_relic_offers", "queued_relic_key_offers"),
        ] {
            let field = reward.remove(current).expect("current reward field");
            reward.insert(legacy.to_owned(), field);
        }

        let restored = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect("schema-three relic storage migrates");
        assert_eq!(restored.schema_version, SNAPSHOT_SCHEMA_VERSION);
        assert!(restored.state.relics.contains(&Relic::SpiritPoop));
        assert_eq!(
            restored.state.pending_event_combat_relic_offer,
            Some(Relic::OddMushroom)
        );
        let reward = restored.state.reward.expect("reward restored");
        assert_eq!(reward.relic_offer, Some(Relic::MarkOfBloom));
        assert_eq!(reward.pending_relic_offer, Some(Relic::NlothsGift));
        assert_eq!(reward.queued_relic_offers, vec![Relic::TheBoot]);
    }

    #[test]
    fn schema_three_rejects_paired_relic_offer_authorities() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.phase = RunPhase::Reward;
        run.reward = Some(RewardScreen {
            continuation: RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 0,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: Some(Relic::TheBoot),
            pending_relic_offer: None,
            queued_relic_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_flow: CardRewardFlow::None,
        });
        let mut value = serde_json::to_value(Snapshot {
            schema_version: LEGACY_RELIC_STORAGE_SNAPSHOT_SCHEMA_VERSION,
            state: run,
        })
        .expect("snapshot serializes");
        value["state"]["reward"]["relic_key_offer"] =
            serde_json::to_value(Relic::Pantograph).expect("relic serializes");

        let error = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("paired relic authorities must fail closed");
        assert!(matches!(error, SnapshotRestoreError::InvalidDocument(_)));
    }

    #[test]
    fn schema_three_rejects_duplicate_relic_ownership_across_stores() {
        let run = RunState::seeded_ironclad(7, 0);
        let mut value = serde_json::to_value(Snapshot {
            schema_version: LEGACY_RELIC_STORAGE_SNAPSHOT_SCHEMA_VERSION,
            state: run,
        })
        .expect("snapshot serializes");
        value["state"]["relic_keys"] = json!(["BurningBlood"]);

        let error = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("duplicate legacy ownership must fail closed");
        assert!(matches!(error, SnapshotRestoreError::InvalidDocument(_)));
    }

    #[test]
    fn current_schema_rejects_retired_relic_storage_fields() {
        let run = RunState::seeded_ironclad(7, 0);
        let mut value = serde_json::to_value(Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: run,
        })
        .expect("snapshot serializes");
        value["state"]["relic_keys"] = json!(["SpiritPoop"]);

        let error = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("current schema must reject retired relic fields");
        assert!(matches!(error, SnapshotRestoreError::Json(_)));
    }

    #[test]
    fn current_snapshot_rejects_choices_without_a_reward_flow() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.phase = RunPhase::Reward;
        let card = run.deck[0];
        let mut value = serde_json::to_value(Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: run,
        })
        .expect("snapshot serializes");
        value["state"]["reward"] = json!({
            "choices": [card],
            "gold_offer": 0,
            "potion_offer": null,
            "relic_offer": null,
        });

        let error = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("orphaned card reward choices must fail validation");

        assert!(matches!(error, SnapshotRestoreError::InvalidState(_)));
    }
}
