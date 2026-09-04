use crate::{RunState, SimError, SimResult};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{error::Error, fmt};

pub const SNAPSHOT_SCHEMA_VERSION: u32 = 9;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot<T> {
    pub schema_version: u32,
    pub state: T,
}

#[derive(Deserialize)]
struct SnapshotDocument<T> {
    schema_version: u32,
    state: T,
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

fn require_canonical_encoded_snapshot<T: Serialize>(
    original: &Value,
    snapshot: &Snapshot<T>,
) -> Result<(), SnapshotRestoreError> {
    // Compare JSON documents, not `to_value`: serde_json widens f32 to f64 in
    // `Value`, which disagrees with the decimal text the serializer emits for
    // current `EventRoomChance` snapshots.
    let encoded: Value = serde_json::from_str(&serde_json::to_string(snapshot)?)?;
    if &encoded != original {
        return Err(SnapshotRestoreError::InvalidDocument(
            "snapshot is not canonical for the current schema",
        ));
    }
    Ok(())
}

pub fn restore_run_snapshot_json(json: &str) -> Result<Snapshot<RunState>, SnapshotRestoreError> {
    restore_snapshot_document(json, RunState::validate)
}

fn restore_snapshot_document<T>(
    json: &str,
    validate: impl FnOnce(&T) -> SimResult<()>,
) -> Result<Snapshot<T>, SnapshotRestoreError>
where
    T: Serialize + DeserializeOwned,
{
    let original: Value = serde_json::from_str(json)?;
    require_current_schema(schema_version(&original)?)?;
    let document: SnapshotDocument<T> = serde_json::from_value(original.clone())?;
    validate(&document.state).map_err(SnapshotRestoreError::InvalidState)?;
    if document.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return Err(SnapshotRestoreError::UnsupportedSchemaVersion(
            document.schema_version,
        ));
    }
    let snapshot = Snapshot {
        schema_version: document.schema_version,
        state: document.state,
    };
    require_canonical_encoded_snapshot(&original, &snapshot)?;
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
        CardRewardFlow, CombatState, Relic, RewardContinuation, RewardScreen, RoomKind, RunPhase,
    };
    use serde_json::{json, Value};

    fn restore_combat_snapshot_json(
        json: &str,
    ) -> Result<Snapshot<CombatState>, SnapshotRestoreError> {
        restore_snapshot_document(json, CombatState::validate)
    }

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
            r#"{"schema_version":9,"state":{}}"#
        );
    }

    #[test]
    fn snapshot_round_trip_preserves_hash() {
        let snapshot = empty_snapshot();
        let before = snapshot.hash().expect("snapshot hashes");
        let json = snapshot.canonical_json().expect("snapshot serializes");
        let restored: Snapshot<EmptySnapshotState> =
            restore_snapshot_document(&json, |_| Ok(())).expect("snapshot deserializes");

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
        for version in 0..=8 {
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
    fn current_snapshot_rejects_unknown_nested_fields() {
        let combat = CombatState::initial_fixture();
        let mut value = serde_json::to_value(Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: combat,
        })
        .expect("combat snapshot serializes");
        value["state"]["future_hidden_field"] = json!(1);

        let error = restore_combat_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("unknown nested snapshot fields are rejected");
        assert!(matches!(
            error,
            SnapshotRestoreError::InvalidDocument(_) | SnapshotRestoreError::Json(_)
        ));
    }

    #[test]
    fn current_snapshot_rejects_explicit_skipped_defaults() {
        let combat = CombatState::initial_fixture();
        let mut value = serde_json::to_value(Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: combat,
        })
        .expect("combat snapshot serializes");
        value["state"]["mark_of_bloom"] = json!(false);

        let error = restore_combat_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("explicit skipped defaults are noncanonical");
        assert!(matches!(error, SnapshotRestoreError::InvalidDocument(_)));
    }

    #[test]
    fn current_snapshot_rejects_potion_name_aliases() {
        let run = RunState::seeded_ironclad(7, 0);
        let mut value = serde_json::to_value(Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: run,
        })
        .expect("run snapshot serializes");
        value["state"]["potions"] = json!(["Gamble"]);

        let error = restore_run_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("potion aliases are not current schema");
        assert!(matches!(
            error,
            SnapshotRestoreError::InvalidDocument(_) | SnapshotRestoreError::Json(_)
        ));
    }

    #[test]
    fn current_snapshot_rejects_retired_elixir_fields() {
        let combat = CombatState::initial_fixture();
        let mut value = serde_json::to_value(Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: combat,
        })
        .expect("combat snapshot serializes");
        value["state"]["pending_elixir_exhaust_card_ids"] = json!([1]);
        value["state"]["pending_elixir_exhaust_turns_remaining"] = json!(1);

        let error = restore_combat_snapshot_json(
            &serde_json::to_string(&value).expect("snapshot value serializes"),
        )
        .expect_err("retired elixir snapshot fields are rejected");
        assert!(matches!(
            error,
            SnapshotRestoreError::InvalidDocument(_) | SnapshotRestoreError::Json(_)
        ));
    }

    #[test]
    fn combat_fixture_snapshot_is_canonical_for_current_schema() {
        let combat = CombatState::initial_fixture();
        let snapshot = Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: combat,
        };
        let json = snapshot
            .canonical_json()
            .expect("combat fixture snapshot serializes");
        let restored =
            restore_combat_snapshot_json(&json).expect("canonical combat fixture restores");
        assert_eq!(restored, snapshot);
    }

    #[test]
    fn schema9_run_snapshots_encode_exactly_one_player_owner() {
        let noncombat = Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: RunState::map_fixture(),
        };
        let noncombat_json = noncombat
            .canonical_json()
            .expect("noncombat snapshot serializes");
        let noncombat_value: Value =
            serde_json::from_str(&noncombat_json).expect("noncombat snapshot parses");
        assert!(noncombat_value["state"].get("run_player").is_some());
        assert!(noncombat_value["state"]["combat"].is_null());
        assert_eq!(
            restore_run_snapshot_json(&noncombat_json).expect("noncombat snapshot restores"),
            noncombat
        );

        let combat = Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: RunState::combat_fixture(),
        };
        let combat_json = combat.canonical_json().expect("combat snapshot serializes");
        let combat_value: Value =
            serde_json::from_str(&combat_json).expect("combat snapshot parses");
        assert!(combat_value["state"].get("run_player").is_none());
        assert!(combat_value["state"]["combat"].is_object());
        assert_eq!(
            restore_run_snapshot_json(&combat_json).expect("combat snapshot restores"),
            combat
        );
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
        let snapshot = Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: intro.clone(),
        };
        let json = snapshot
            .canonical_json()
            .expect("Falling snapshot serializes");

        let restored =
            restore_run_snapshot_json(&json).expect("Falling preselection snapshot restores");
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
