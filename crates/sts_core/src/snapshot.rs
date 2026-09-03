use crate::{CombatState, RunState, SimError, SimResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{error::Error, fmt};

pub const SNAPSHOT_SCHEMA_VERSION: u32 = 8;

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

fn require_current_schema(version: u32) -> Result<(), SnapshotRestoreError> {
    if version == SNAPSHOT_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(SnapshotRestoreError::UnsupportedSchemaVersion(version))
    }
}

const RETIRED_COMBAT_DECISION_FIELDS: [&str; 10] = [
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

fn reject_retired_combat_decision_fields(combat: &Value) -> Result<(), SnapshotRestoreError> {
    let object = combat
        .as_object()
        .ok_or(SnapshotRestoreError::InvalidDocument(
            "combat state must be an object",
        ))?;
    if RETIRED_COMBAT_DECISION_FIELDS
        .iter()
        .any(|field| object.contains_key(*field))
    {
        return Err(SnapshotRestoreError::InvalidDocument(
            "current snapshot contains retired combat decision fields",
        ));
    }
    Ok(())
}

pub fn restore_combat_snapshot_json(
    json: &str,
) -> Result<Snapshot<CombatState>, SnapshotRestoreError> {
    let value: Value = serde_json::from_str(json)?;
    require_current_schema(schema_version(&value)?)?;
    if let Some(state) = value.get("state") {
        reject_retired_combat_decision_fields(state)?;
    }
    let snapshot: Snapshot<CombatState> = serde_json::from_value(value)?;
    snapshot
        .state
        .validate()
        .map_err(SnapshotRestoreError::InvalidState)?;
    if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return Err(SnapshotRestoreError::UnsupportedSchemaVersion(
            snapshot.schema_version,
        ));
    }
    Ok(snapshot)
}

pub fn restore_run_snapshot_json(json: &str) -> Result<Snapshot<RunState>, SnapshotRestoreError> {
    let value: Value = serde_json::from_str(json)?;
    require_current_schema(schema_version(&value)?)?;
    if let Some(combat) = value
        .pointer("/state/combat")
        .filter(|combat| !combat.is_null())
    {
        reject_retired_combat_decision_fields(combat)?;
    }
    let snapshot: Snapshot<RunState> = serde_json::from_value(value)?;
    snapshot
        .state
        .validate()
        .map_err(SnapshotRestoreError::InvalidState)?;
    if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return Err(SnapshotRestoreError::UnsupportedSchemaVersion(
            snapshot.schema_version,
        ));
    }
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
    use crate::{CardRewardFlow, Relic, RewardContinuation, RewardScreen, RoomKind, RunPhase};
    use serde_json::{json, Value};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct EmptySnapshotState {}

    fn empty_snapshot() -> Snapshot<EmptySnapshotState> {
        Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: EmptySnapshotState {},
        }
    }

    fn reward_snapshot_value() -> Value {
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
            schema_version: SNAPSHOT_SCHEMA_VERSION,
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
    fn historical_snapshot_schemas_are_rejected() {
        let json = Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: RunState::map_fixture(),
        }
        .canonical_json()
        .expect("current snapshot serializes");
        let mut value: Value = serde_json::from_str(&json).expect("current snapshot parses");
        for version in 0..=7 {
            value["schema_version"] = Value::from(version);
            let error = restore_run_snapshot_json(
                &serde_json::to_string(&value).expect("relabeled snapshot serializes"),
            )
            .expect_err("historical snapshot schemas are unsupported");
            assert!(matches!(
                error,
                SnapshotRestoreError::UnsupportedSchemaVersion(rejected) if rejected == version
            ));
        }
    }

    #[test]
    fn run_snapshot_round_trip_preserves_missing_note_card() {
        let mut run = RunState::map_fixture();
        run.note_card_content_id = None;
        run.note_card_upgrades = 0;
        let snapshot = Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: run,
        };
        let json = snapshot
            .canonical_json()
            .expect("missing Note card snapshot serializes");

        assert!(json.contains(r#""note_card_content_id":null"#));
        let restored =
            restore_run_snapshot_json(&json).expect("missing Note card snapshot restores");
        assert_eq!(restored.state.note_card_content_id, None);
        assert_eq!(restored, snapshot);
    }

    #[test]
    fn run_snapshot_round_trip_preserves_pending_external_rng() {
        let mut run = RunState::map_fixture();
        run.pending_external_rng.push(crate::ExternalRngInput {
            kind: crate::ExternalRngKind::CardGroupGetRandomCardByType,
            state: crate::MathUtilsRngState {
                state0: 0xfedc_ba98_7654_3210,
                state1: 0x0123_4567_89ab_cdef,
            },
            range_inclusive: 16,
        });
        let snapshot = Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: run,
        };
        let json = snapshot
            .canonical_json()
            .expect("external RNG snapshot serializes");

        assert!(json.contains(r#""state0":"fedcba9876543210""#));
        assert!(json.contains(r#""state1":"0123456789abcdef""#));
        let restored =
            restore_run_snapshot_json(&json).expect("external RNG snapshot restores exactly");
        assert_eq!(restored, snapshot);
    }

    #[test]
    fn run_snapshot_round_trip_preserves_pending_combat_obtain_cards() {
        let mut run = RunState::combat_fixture();
        run.pending_combat_obtain_cards
            .push(crate::content::cards::PARASITE_ID);
        let snapshot = Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: run,
        };
        let json = snapshot
            .canonical_json()
            .expect("combat obtain snapshot serializes");

        assert!(json.contains("pending_combat_obtain_cards"));
        let restored =
            restore_run_snapshot_json(&json).expect("combat obtain snapshot restores exactly");
        assert_eq!(restored, snapshot);
    }

    #[test]
    fn run_snapshot_round_trip_preserves_pending_obtain_provenance() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.current_floor = 7;
        run.current_room_override = Some(RoomKind::Event);
        run.relics = vec![Relic::Omamori];
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::event_screen(
            crate::Event::HypnotizingColoredMushrooms,
        ));
        let run = crate::run::event::apply_event_action(
            &run,
            crate::EventAction::Choose { choice_index: 1 },
        )
        .expect("Mushroom obtain provenance is generated");
        let snapshot = Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: run,
        };
        let json = snapshot
            .canonical_json()
            .expect("obtain provenance snapshot serializes");

        assert!(json.contains("pending_obtain_provenance"));
        let restored =
            restore_run_snapshot_json(&json).expect("obtain provenance snapshot restores exactly");
        assert_eq!(restored, snapshot);
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
    fn current_reward_snapshot_rejects_retained_owner_without_continuation() {
        let mut value = reward_snapshot_value();
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
    fn current_snapshot_rejects_ownerless_pending_obtain_cards() {
        let mut run = RunState::map_fixture();
        run.pending_obtain_cards
            .push(crate::content::cards::INJURY_ID);
        let value = serde_json::to_value(Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: run,
        })
        .expect("run snapshot serializes");

        let error = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("pending obtain cards require an authoritative event owner");

        assert!(matches!(error, SnapshotRestoreError::InvalidState(_)));
    }

    #[test]
    fn current_snapshot_rejects_event_grid_after_its_owner_stage() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::event_screen_for_run(
            &run,
            crate::Event::Purifier,
        ));
        let opened = crate::run::event::apply_event_action(
            &run,
            crate::EventAction::Choose { choice_index: 0 },
        )
        .expect("Purifier opens its remove grid");
        let grid = opened.card_grid.clone().expect("remove grid");
        let selected = crate::select_grid_card(&opened, 0).expect("remove card can be selected");
        let mut stale = crate::confirm_grid(&selected).expect("Purifier remove confirms");
        stale.card_grid = Some(grid);
        let value = serde_json::to_value(Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: stale,
        })
        .expect("run snapshot serializes");

        let error = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("event grid cannot outlive its authoritative stage");

        assert!(matches!(error, SnapshotRestoreError::InvalidState(_)));
    }

    #[test]
    fn current_snapshot_rejects_fabricated_duplicator_grid() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.current_act = 2;
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::event_screen_for_run(
            &run,
            crate::Event::Duplicator,
        ));
        let mut opened = crate::run::event::apply_event_action(
            &run,
            crate::EventAction::Choose { choice_index: 0 },
        )
        .expect("Duplicator opens its copy grid");
        opened.card_grid.as_mut().expect("Duplicator grid").cards[0].content_id =
            crate::content::cards::BASH_ID;
        let value = serde_json::to_value(Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: opened,
        })
        .expect("run snapshot serializes");

        let error = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("Duplicator cannot import a fabricated card copy");

        assert!(matches!(error, SnapshotRestoreError::InvalidState(_)));
    }

    #[test]
    fn current_snapshot_rejects_fabricated_pandoras_box_grid() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::event_screen_for_run(
            &run,
            crate::Event::Neow,
        ));
        run.gain_relic(crate::Relic::PandorasBox)
            .expect("Pandora's Box opens its generated grid");
        run.card_grid.as_mut().expect("Pandora's Box grid").cards[0].content_id =
            crate::content::cards::BASH_ID;
        let value = serde_json::to_value(Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: run,
        })
        .expect("run snapshot serializes");

        let error = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("Pandora's Box cannot import fabricated generated cards");

        assert!(matches!(error, SnapshotRestoreError::InvalidState(_)));
    }

    #[test]
    fn current_snapshot_rejects_incomplete_deck_derived_grid() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::event_screen_for_run(
            &run,
            crate::Event::Purifier,
        ));
        let mut opened = crate::run::event::apply_event_action(
            &run,
            crate::EventAction::Choose { choice_index: 0 },
        )
        .expect("Purifier opens its remove grid");
        opened
            .card_grid
            .as_mut()
            .expect("Purifier grid")
            .cards
            .pop();
        let value = serde_json::to_value(Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: opened,
        })
        .expect("run snapshot serializes");

        let error = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("deck-derived grid cannot hide an authoritative choice");

        assert!(matches!(error, SnapshotRestoreError::InvalidState(_)));
    }

    #[test]
    fn current_snapshot_preserves_falling_preselection_rng_state() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::event_screen_for_run(
            &run,
            crate::Event::Falling,
        ));
        let intro = crate::run::event::apply_event_action(
            &run,
            crate::EventAction::Choose { choice_index: 0 },
        )
        .expect("Falling intro opens card-type choices");
        let value = serde_json::to_value(Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: intro.clone(),
        })
        .expect("run snapshot serializes");

        let restored = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect("Falling preselection snapshot restores");
        assert_eq!(restored.state, intro);
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
