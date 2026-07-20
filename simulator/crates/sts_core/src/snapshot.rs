use crate::{CombatState, RunState, SimError, SimResult};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{error::Error, fmt};

pub const SNAPSHOT_SCHEMA_VERSION: u32 = 3;
pub const PREVIOUS_SNAPSHOT_SCHEMA_VERSION: u32 = 2;
pub const LEGACY_VALIDATED_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot<T = PlaceholderState> {
    pub schema_version: u32,
    pub state: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceholderState {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotHash(u64);

#[derive(Debug)]
pub enum SnapshotRestoreError {
    Json(serde_json::Error),
    InvalidDocument(&'static str),
    UnsupportedSchemaVersion(u32),
    InvalidState(SimError),
}

impl Snapshot<PlaceholderState> {
    #[must_use]
    pub const fn placeholder() -> Self {
        Self {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: PlaceholderState {},
        }
    }
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
            | PREVIOUS_SNAPSHOT_SCHEMA_VERSION
            | LEGACY_VALIDATED_SNAPSHOT_SCHEMA_VERSION
    ) {
        Ok(())
    } else {
        Err(SnapshotRestoreError::UnsupportedSchemaVersion(version))
    }
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

fn migrate_legacy_run_snapshot(value: &mut Value) -> Result<(), SnapshotRestoreError> {
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

pub fn restore_combat_snapshot_json(
    json: &str,
) -> Result<Snapshot<CombatState>, SnapshotRestoreError> {
    let value: Value = serde_json::from_str(json)?;
    validate_supported_schema(schema_version(&value)?)?;
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
    if version != SNAPSHOT_SCHEMA_VERSION {
        migrate_legacy_run_snapshot(&mut value)?;
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
    use crate::{CardRewardFlow, RunPhase};
    use serde_json::json;

    fn legacy_run_snapshot_json(version: u32, active: bool, pending: bool, count: u8) -> String {
        let snapshot = Snapshot {
            schema_version: version,
            state: RunState::seeded_ironclad(7, 0),
        };
        let mut value = serde_json::to_value(snapshot).expect("snapshot serializes");
        value["state"]["phase"] = Value::String("Reward".to_owned());
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

    #[test]
    fn same_snapshot_hashes_identically() {
        let first = Snapshot::placeholder();
        let second = Snapshot::placeholder();

        assert_eq!(
            first.hash().expect("first hashes"),
            second.hash().expect("second hashes")
        );
    }

    #[test]
    fn canonical_field_order_does_not_drift() {
        let snapshot = Snapshot::placeholder();

        assert_eq!(
            snapshot.canonical_json().expect("snapshot serializes"),
            r#"{"schema_version":3,"state":{}}"#
        );
    }

    #[test]
    fn snapshot_round_trip_preserves_hash() {
        let snapshot = Snapshot::placeholder();
        let before = snapshot.hash().expect("snapshot hashes");
        let json = snapshot.canonical_json().expect("snapshot serializes");
        let restored: Snapshot = serde_json::from_str(&json).expect("snapshot deserializes");

        assert_eq!(restored, snapshot);
        assert_eq!(restored.hash().expect("restored hashes"), before);
    }

    #[test]
    fn historical_run_snapshots_migrate_pending_card_reward_counts() {
        for version in [
            LEGACY_VALIDATED_SNAPSHOT_SCHEMA_VERSION,
            PREVIOUS_SNAPSHOT_SCHEMA_VERSION,
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
            PREVIOUS_SNAPSHOT_SCHEMA_VERSION,
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
            PREVIOUS_SNAPSHOT_SCHEMA_VERSION,
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
