use crate::model::{
    BlockedState, LegalAction, LegalActionKind, LiveError, LivePhase, LiveResult, LiveState,
    SlayTheDataAdvisorStep, SlayTheDataRunSummary, SlayTheDataSearchFilters,
    SlayTheDataSessionSnapshot,
};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row, ToSql};
use serde_json::json;
use std::{
    env,
    path::{Path, PathBuf},
};
use sts_verify::{
    import_slaythedata_run_json, slaythedata_replay_plan, slaythedata_replay_preflight,
    SlayTheDataBridgeDescriptor, SlayTheDataPreflightReport, SlayTheDataPreflightStatus,
    SlayTheDataPreflightStep,
};

pub const SLAYTHEDATA_DB_ENV: &str = "STS_LIVE_SLAYTHEDATA_DB";
pub const DEFAULT_SLAYTHEDATA_DB: &str = "slaythedata-chunks.sqlite3";

#[derive(Debug, Clone)]
pub struct SlayTheDataIndex {
    db_path: PathBuf,
}

impl SlayTheDataIndex {
    pub fn new(db_path: impl AsRef<Path>) -> Self {
        Self {
            db_path: db_path.as_ref().to_path_buf(),
        }
    }

    pub fn default_local() -> Self {
        Self::new(
            env::var(SLAYTHEDATA_DB_ENV)
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_SLAYTHEDATA_DB)),
        )
    }

    pub fn search(
        &self,
        filters: &SlayTheDataSearchFilters,
    ) -> LiveResult<Vec<SlayTheDataRunSummary>> {
        let conn = open_readonly(&self.db_path)?;
        require_tables(&conn, &["runs", "chunk_runs"])?;
        let tables = table_names(&conn)?;
        let run_columns = table_columns(&conn, "runs")?;
        let neow_bonus_expr = if run_columns.iter().any(|column| column == "neow_bonus") {
            "runs.neow_bonus"
        } else {
            "NULL"
        };
        let neow_cost_expr = if run_columns.iter().any(|column| column == "neow_cost") {
            "runs.neow_cost"
        } else {
            "NULL"
        };
        let materialized_expr = if tables.iter().any(|table| table == "run_materialized_json") {
            "EXISTS (SELECT 1 FROM run_materialized_json m WHERE m.run_id = runs.id)"
        } else {
            "0"
        };

        let mut clauses = vec![
            "runs.character_chosen = ?".to_owned(),
            "runs.floor_reached >= ?".to_owned(),
            "COALESCE(runs.is_daily, 0) = 0".to_owned(),
            "COALESCE(runs.is_endless, 0) = 0".to_owned(),
            "COALESCE(runs.is_trial, 0) = 0".to_owned(),
        ];
        let mut values: Vec<Box<dyn ToSql>> = vec![
            Box::new(filters.character.to_ascii_uppercase()),
            Box::new(i64::from(filters.min_floor_reached)),
        ];
        if let Some(ascension) = filters.ascension {
            clauses.push("runs.ascension_level = ?".to_owned());
            values.push(Box::new(i64::from(ascension)));
        }
        if let Some(max_floor) = filters.max_floor_reached {
            clauses.push("runs.floor_reached <= ?".to_owned());
            values.push(Box::new(i64::from(max_floor)));
        }
        if let Some(victory) = filters.victory {
            clauses.push("runs.victory = ?".to_owned());
            values.push(Box::new(if victory { 1_i64 } else { 0_i64 }));
        }
        if let Some(seed) = filters
            .seed_played
            .as_deref()
            .filter(|seed| !seed.trim().is_empty())
        {
            clauses.push("runs.seed_played = ?".to_owned());
            values.push(Box::new(seed.trim().to_owned()));
        }
        if filters.require_supported && run_columns.iter().any(|column| column == "unsupported_any")
        {
            clauses.push("COALESCE(runs.unsupported_any, 0) = 0".to_owned());
        }
        values.push(Box::new(i64::try_from(filters.limit.max(1)).unwrap_or(50)));

        let query = format!(
            r#"
            SELECT runs.id,
                   runs.seed_played,
                   runs.ascension_level,
                   runs.floor_reached,
                   COALESCE(runs.victory, 0),
                   runs.path_length,
                   runs.card_choice_count,
                   runs.event_choice_count,
                   runs.shop_purchase_count,
                   runs.potion_usage_count,
                   {neow_bonus_expr},
                   {neow_cost_expr},
                   COALESCE(runs.card_choice_count, 0)
                     + COALESCE(runs.event_choice_count, 0) * 2
                     + COALESCE(runs.shop_purchase_count, 0) * 3
                     + COALESCE(runs.potion_usage_count, 0) AS guided_score,
                   {materialized_expr} AS materialized
            FROM runs
            JOIN chunk_runs ON chunk_runs.run_id = runs.id
            WHERE {}
            ORDER BY COALESCE(runs.path_length, 0) DESC,
                     guided_score DESC,
                     COALESCE(runs.floor_reached, 0) DESC,
                     runs.id ASC
            LIMIT ?
            "#,
            clauses.join(" AND ")
        );
        let params = values
            .iter()
            .map(|value| value.as_ref())
            .collect::<Vec<_>>();
        let mut stmt = conn.prepare(&query).map_err(sql_error)?;
        let rows = stmt
            .query_map(&params[..], summary_from_row)
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        Ok(rows)
    }

    pub fn load_materialized_run(
        &self,
        run_id: i64,
    ) -> LiveResult<(SlayTheDataRunSummary, String)> {
        let conn = open_readonly(&self.db_path)?;
        require_tables(&conn, &["runs"])?;
        let summary = self
            .summary_by_id_with_conn(&conn, run_id)?
            .ok_or_else(|| LiveError::NotFound(format!("SlayTheData run {run_id}")))?;
        require_tables(&conn, &["run_materialized_json"])?;
        let raw = conn
            .query_row(
                "SELECT raw_event_json FROM run_materialized_json WHERE run_id = ?",
                params![run_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sql_error)?
            .ok_or_else(|| {
                LiveError::Blocked(format!(
                    "SlayTheData run {run_id} is not materialized; export it with chunk-export --store before attaching"
                ))
            })?;
        Ok((summary, raw))
    }

    fn summary_by_id_with_conn(
        &self,
        conn: &Connection,
        run_id: i64,
    ) -> LiveResult<Option<SlayTheDataRunSummary>> {
        let tables = table_names(conn)?;
        let run_columns = table_columns(conn, "runs")?;
        let neow_bonus_expr = if run_columns.iter().any(|column| column == "neow_bonus") {
            "runs.neow_bonus"
        } else {
            "NULL"
        };
        let neow_cost_expr = if run_columns.iter().any(|column| column == "neow_cost") {
            "runs.neow_cost"
        } else {
            "NULL"
        };
        let materialized_expr = if tables.iter().any(|table| table == "run_materialized_json") {
            "EXISTS (SELECT 1 FROM run_materialized_json m WHERE m.run_id = runs.id)"
        } else {
            "0"
        };
        let query = format!(
            r#"
            SELECT runs.id,
                   runs.seed_played,
                   runs.ascension_level,
                   runs.floor_reached,
                   COALESCE(runs.victory, 0),
                   runs.path_length,
                   runs.card_choice_count,
                   runs.event_choice_count,
                   runs.shop_purchase_count,
                   runs.potion_usage_count,
                   {neow_bonus_expr},
                   {neow_cost_expr},
                   COALESCE(runs.card_choice_count, 0)
                     + COALESCE(runs.event_choice_count, 0) * 2
                     + COALESCE(runs.shop_purchase_count, 0) * 3
                     + COALESCE(runs.potion_usage_count, 0) AS guided_score,
                   {materialized_expr} AS materialized
            FROM runs
            WHERE runs.id = ?
            "#
        );
        conn.query_row(&query, params![run_id], summary_from_row)
            .optional()
            .map_err(sql_error)
    }
}

#[derive(Debug, Clone)]
pub struct AttachedSlayTheDataRun {
    pub summary: SlayTheDataRunSummary,
    pub report: SlayTheDataPreflightReport,
    pub next_step_index: usize,
    pub blocked: Option<BlockedState>,
    pub last_message: Option<String>,
}

impl AttachedSlayTheDataRun {
    pub fn from_raw(summary: SlayTheDataRunSummary, raw_run_json: &str) -> LiveResult<Self> {
        let imported = import_slaythedata_run_json(raw_run_json).map_err(|error| {
            LiveError::InvalidAction(format!("SlayTheData import failed: {error}"))
        })?;
        let plan = slaythedata_replay_plan(&imported);
        let report = slaythedata_replay_preflight(&plan);
        Ok(Self {
            summary,
            report,
            next_step_index: 0,
            blocked: None,
            last_message: Some("SlayTheData run attached".to_owned()),
        })
    }

    pub fn snapshot(&self, state: Option<&LiveState>) -> SlayTheDataSessionSnapshot {
        SlayTheDataSessionSnapshot {
            attached_run: Some(self.summary.clone()),
            advisor: self.advisor_step(state),
            next_step_index: self.next_step_index,
            blocked: self.blocked.clone(),
            last_message: self.last_message.clone(),
        }
    }

    pub fn advisor_step(&self, state: Option<&LiveState>) -> Option<SlayTheDataAdvisorStep> {
        for (index, step) in self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(self.next_step_index)
        {
            if is_combat_only_guidance(&step.code) {
                continue;
            }
            let mut advisor = SlayTheDataAdvisorStep {
                floor: step.floor,
                ordinal: step.ordinal,
                status: status_name(step.status).to_owned(),
                code: step.code.clone(),
                message: step.message.clone(),
                command: step
                    .bridge_command
                    .as_ref()
                    .map(|hint| hint.command.clone()),
                action_id: None,
                action_label: None,
            };
            if index < self.next_step_index {
                continue;
            }
            if let (Some(state), Some(_command)) = (state, advisor.command.as_deref()) {
                if let Ok(action) = bind_step_to_live_action(state, step) {
                    advisor.action_id = Some(action.id.clone());
                    advisor.action_label = Some(action.label.clone());
                }
            }
            return Some(advisor);
        }
        None
    }

    pub fn ready_action(&self, state: &LiveState) -> Result<(usize, LegalAction), BlockedState> {
        if let Some(blocked) = self.blocked.clone() {
            return Err(blocked);
        }
        let Some((index, step)) = self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(self.next_step_index)
            .find(|(_, step)| !is_combat_only_guidance(&step.code))
        else {
            return Err(blocked(
                "slaythedata_done",
                "SlayTheData plan has no remaining guided step",
            ));
        };
        if step.status == SlayTheDataPreflightStatus::Blocked {
            return Err(blocked(&step.code, &step.message));
        }
        if step.bridge_command.is_none() {
            return Err(blocked(
                &step.code,
                "next SlayTheData step is guidance-only and has no unique bridge command",
            ));
        }
        let action = bind_step_to_live_action(state, step)
            .map_err(|message| blocked("slaythedata_action_mismatch", &message))?;
        Ok((index, action.clone()))
    }

    pub fn mark_sent(&mut self, index: usize) {
        self.next_step_index = index.saturating_add(1);
        self.blocked = None;
        self.last_message = Some("SlayTheData guided action sent".to_owned());
    }

    pub fn mark_blocked(&mut self, blocked: BlockedState) {
        self.last_message = Some(blocked.message.clone());
        self.blocked = Some(blocked);
    }
}

pub fn bind_command_to_live_action<'a>(
    state: &'a LiveState,
    expected_command: &str,
) -> Result<&'a LegalAction, String> {
    bind_matching_live_action(state, expected_command, |_| true)
}

pub fn bind_step_to_live_action<'a>(
    state: &'a LiveState,
    step: &SlayTheDataPreflightStep,
) -> Result<&'a LegalAction, String> {
    let Some(hint) = step.bridge_command.as_ref() else {
        return Err("SlayTheData step has no bridge command".to_owned());
    };
    let expected = expected_live_context(step, &hint.descriptor);
    bind_matching_live_action(state, &hint.command, |action| {
        expected
            .as_ref()
            .is_none_or(|context| context.matches(state, action))
    })
    .map_err(|message| {
        if let Some(context) = expected {
            format!(
                "{message}; expected live context phase {:?} and action kind {:?} for SlayTheData step {}",
                context.phase, context.kind, step.code
            )
        } else {
            message
        }
    })
}

fn bind_matching_live_action<'a>(
    state: &'a LiveState,
    expected_command: &str,
    action_filter: impl Fn(&LegalAction) -> bool,
) -> Result<&'a LegalAction, String> {
    let matches = state
        .legal_actions
        .iter()
        .filter(|action| action.enabled)
        .filter(|action| action_filter(action))
        .filter(|action| {
            action
                .command
                .get("command")
                .and_then(|value| value.as_str())
                .is_some_and(|command| command.eq_ignore_ascii_case(expected_command))
                || action.command == json!({"kind": "choose_neow", "choice": 0})
                    && expected_command.eq_ignore_ascii_case("CHOOSE 0")
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [action] => Ok(action),
        [] => Err(format!(
            "no current live legal action matches SlayTheData command {expected_command:?}"
        )),
        _ => Err(format!(
            "SlayTheData command {expected_command:?} matches multiple live actions"
        )),
    }
}

#[derive(Debug, Clone)]
struct ExpectedLiveContext {
    phase: LivePhase,
    kind: LegalActionKind,
}

impl ExpectedLiveContext {
    fn matches(&self, state: &LiveState, action: &LegalAction) -> bool {
        state.phase == self.phase && action.kind == self.kind
    }
}

fn expected_live_context(
    step: &SlayTheDataPreflightStep,
    descriptor: &SlayTheDataBridgeDescriptor,
) -> Option<ExpectedLiveContext> {
    match (step.code.as_str(), descriptor) {
        (
            "legal_neow_talk" | "legal_neow_bonus" | "legal_neow_leave",
            SlayTheDataBridgeDescriptor::ChooseVisibleOption { .. },
        ) => Some(ExpectedLiveContext {
            phase: LivePhase::Neow,
            kind: LegalActionKind::ChooseNeow,
        }),
        ("legal_map_room", SlayTheDataBridgeDescriptor::ChooseVisibleOption { .. }) => {
            Some(ExpectedLiveContext {
                phase: LivePhase::Map,
                kind: LegalActionKind::ChooseMapNode,
            })
        }
        ("legal_card_reward", SlayTheDataBridgeDescriptor::ChooseVisibleOption { .. }) => {
            Some(ExpectedLiveContext {
                phase: LivePhase::Reward,
                kind: LegalActionKind::ChooseReward,
            })
        }
        ("legal_card_reward", SlayTheDataBridgeDescriptor::SkipVisibleReward) => {
            Some(ExpectedLiveContext {
                phase: LivePhase::Reward,
                kind: LegalActionKind::SkipReward,
            })
        }
        _ => None,
    }
}

fn open_readonly(path: &Path) -> LiveResult<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(sql_error)
}

fn require_tables(conn: &Connection, required: &[&str]) -> LiveResult<()> {
    let tables = table_names(conn)?;
    let missing = required
        .iter()
        .filter(|name| !tables.iter().any(|table| table == **name))
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(LiveError::Blocked(format!(
            "SlayTheData database is missing required table(s): {}",
            missing.join(", ")
        )))
    }
}

fn table_names(conn: &Connection) -> LiveResult<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
        .map_err(sql_error)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(rows)
}

fn table_columns(conn: &Connection, table: &str) -> LiveResult<Vec<String>> {
    if !table
        .chars()
        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return Err(LiveError::InvalidAction(format!(
            "unsafe table name {table}"
        )));
    }
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(sql_error)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(rows)
}

fn summary_from_row(row: &Row<'_>) -> rusqlite::Result<SlayTheDataRunSummary> {
    Ok(SlayTheDataRunSummary {
        id: row.get(0)?,
        seed_played: row.get(1)?,
        ascension_level: optional_u8(row, 2)?,
        floor_reached: optional_u32(row, 3)?,
        victory: row.get::<_, i64>(4)? != 0,
        path_length: optional_u32(row, 5)?,
        card_choice_count: optional_u32(row, 6)?,
        event_choice_count: optional_u32(row, 7)?,
        shop_purchase_count: optional_u32(row, 8)?,
        potion_usage_count: optional_u32(row, 9)?,
        neow_bonus: row.get(10)?,
        neow_cost: row.get(11)?,
        guided_score: row.get(12)?,
        materialized: row.get::<_, i64>(13)? != 0,
    })
}

fn optional_u8(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u8>> {
    Ok(row
        .get::<_, Option<i64>>(index)?
        .and_then(|value| u8::try_from(value).ok()))
}

fn optional_u32(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u32>> {
    Ok(row
        .get::<_, Option<i64>>(index)?
        .and_then(|value| u32::try_from(value).ok()))
}

fn is_combat_only_guidance(code: &str) -> bool {
    matches!(code, "combat_encounter_evidence" | "guided_potion_budget")
}

fn status_name(status: SlayTheDataPreflightStatus) -> &'static str {
    match status {
        SlayTheDataPreflightStatus::Checked => "checked",
        SlayTheDataPreflightStatus::Guided => "guided",
        SlayTheDataPreflightStatus::Blocked => "blocked",
    }
}

fn blocked(reason_code: &str, message: &str) -> BlockedState {
    BlockedState {
        reason_code: reason_code.to_owned(),
        message: message.to_owned(),
    }
}

fn sql_error(error: rusqlite::Error) -> LiveError {
    LiveError::Blocked(format!("SlayTheData SQLite error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ActionId, LegalActionKind, LivePhase};
    use rusqlite::Connection;
    use std::{
        sync::{Mutex, OnceLock},
        time::SystemTime,
    };

    #[test]
    fn search_filters_exportable_supported_runs() {
        let db = temp_db("search");
        create_locator_schema(&db);
        let conn = Connection::open(&db).unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (1, 'IRONCLAD', 0, 20, 0, 0, 0, 0, 'A', 0, 20, 3, 2, 1, 0, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (2, 'IRONCLAD', 0, 40, 0, 0, 0, 1, 'B', 1, 40, 9, 4, 2, 1, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO chunk_runs VALUES (1)", [])
            .unwrap();
        conn.execute("INSERT INTO chunk_runs VALUES (2)", [])
            .unwrap();
        drop(conn);

        let rows = SlayTheDataIndex::new(&db)
            .search(&SlayTheDataSearchFilters {
                ascension: Some(0),
                min_floor_reached: 1,
                ..SlayTheDataSearchFilters {
                    character: "IRONCLAD".to_owned(),
                    ascension: None,
                    min_floor_reached: 1,
                    max_floor_reached: None,
                    victory: None,
                    seed_played: None,
                    limit: 10,
                    require_supported: true,
                }
            })
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 1);
        assert_eq!(rows[0].guided_score, 10);
        assert!(!rows[0].materialized);
        std::fs::remove_file(db).ok();
    }

    #[test]
    fn command_binding_requires_unique_enabled_action() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Inflame".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Skip".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };

        let action = bind_command_to_live_action(&state, "choose 0").unwrap();
        assert_eq!(action.id.0, "choose-0");
        assert!(bind_command_to_live_action(&state, "choose 2").is_err());
    }

    #[test]
    fn default_local_uses_env_configured_database_path() {
        let _guard = env_lock().lock().unwrap();
        let db = temp_db("env-config");
        let previous = std::env::var_os(SLAYTHEDATA_DB_ENV);
        std::env::set_var(SLAYTHEDATA_DB_ENV, &db);

        assert_eq!(SlayTheDataIndex::default_local().db_path, db);

        if let Some(previous) = previous {
            std::env::set_var(SLAYTHEDATA_DB_ENV, previous);
        } else {
            std::env::remove_var(SLAYTHEDATA_DB_ENV);
        }
    }

    fn create_locator_schema(path: &Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE runs (
                id INTEGER PRIMARY KEY,
                character_chosen TEXT,
                ascension_level INTEGER,
                floor_reached INTEGER,
                is_daily INTEGER,
                is_endless INTEGER,
                is_trial INTEGER,
                unsupported_any INTEGER,
                seed_played TEXT,
                victory INTEGER,
                path_length INTEGER,
                card_choice_count INTEGER,
                event_choice_count INTEGER,
                shop_purchase_count INTEGER,
                potion_usage_count INTEGER,
                neow_bonus TEXT,
                neow_cost TEXT
            );
            CREATE TABLE chunk_runs (run_id INTEGER PRIMARY KEY);
            "#,
        )
        .unwrap();
    }

    fn temp_db(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("sts-live-slaythedata-{name}-{nonce}.sqlite3"))
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }
}
