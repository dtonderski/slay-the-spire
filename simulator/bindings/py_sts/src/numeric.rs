//! Direct numeric transport of the combat policy's PUBLIC input fields.
//! Export consumes FairDecision only. Stepping indexes the current public legal list.
use crate::vocabulary::{
    card_id, counter_id, intent_id, monster_id, potion_id, power_id, relic_id,
    selection_catalog_id, slime_catalog_id,
};
use crate::{public_runtime_error, PyState};
use pyo3::{prelude::*, types::PyBytes};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use sts_env::{
    DecisionRevision, FairCard, FairCombatPhase, FairDecision, FairEnvironment, FairMonsterIntent,
    FairRunScreen, PublicChoice,
};

type Tables = BTreeMap<String, (usize, Py<PyBytes>)>;
type Batch = (u32, Vec<String>, Tables, Vec<usize>);
pub const NUMERIC_VERSION: u32 = 3;
pub const ACTION_ROW_WIDTH: usize = 12;
/// Fixed public kind order. Codes are vocabulary indices, not batch symbol positions.
pub const ACTION_KINDS: &[&str] = &[
    "play_hand_slot",
    "end_turn",
    "choose_event_option",
    "toggle_grid_card",
    "confirm_grid",
    "cancel_grid",
    "choose_map_node",
    "rest_heal",
    "rest_open_smith",
    "rest_open_remove",
    "rest_smith",
    "rest_remove_card",
    "rest_lift",
    "rest_dig",
    "rest_recall",
    "rest_proceed",
    "skip_reward",
    "close_card_reward",
    "take_card_reward",
    "take_singing_bowl_reward",
    "take_gold_reward",
    "take_stolen_gold_reward",
    "take_potion_reward",
    "take_relic_reward",
    "take_relic_reward_at",
    "take_sapphire_key",
    "take_emerald_key",
    "choose_boss_relic_reward",
    "proceed",
    "open_chest",
    "open_card_reward",
    "open_queued_card_reward",
    "skip_potion_reward",
    "buy_shop_card",
    "buy_shop_relic",
    "buy_shop_potion",
    "use_potion_slot",
    "discard_potion_slot",
    "toggle_visible_card",
    "choose_visible_option",
    "confirm_selection",
    "confirm_selection_without_retrieval",
    "skip_selection",
    "enter_shop",
    "leave_shop",
    "open_shop_remove",
];

fn kind_code(choice: PublicChoice) -> i64 {
    let code: i64 = match choice {
        PublicChoice::PlayHandSlot { .. } => 0,
        PublicChoice::EndTurn => 1,
        PublicChoice::ChooseEventOption { .. } => 2,
        PublicChoice::ToggleGridCard { .. } => 3,
        PublicChoice::ConfirmGrid => 4,
        PublicChoice::CancelGrid => 5,
        PublicChoice::ChooseMapNode { .. } => 6,
        PublicChoice::RestHeal => 7,
        PublicChoice::RestOpenSmith => 8,
        PublicChoice::RestOpenRemove => 9,
        PublicChoice::RestSmith { .. } => 10,
        PublicChoice::RestRemoveCard { .. } => 11,
        PublicChoice::RestLift => 12,
        PublicChoice::RestDig => 13,
        PublicChoice::RestRecall => 14,
        PublicChoice::RestProceed => 15,
        PublicChoice::SkipReward => 16,
        PublicChoice::CloseCardReward => 17,
        PublicChoice::TakeCardReward { .. } => 18,
        PublicChoice::TakeSingingBowlReward => 19,
        PublicChoice::TakeGoldReward => 20,
        PublicChoice::TakeStolenGoldReward => 21,
        PublicChoice::TakePotionReward { .. } => 22,
        PublicChoice::TakeRelicReward => 23,
        PublicChoice::TakeRelicRewardAt { .. } => 24,
        PublicChoice::TakeSapphireKey => 25,
        PublicChoice::TakeEmeraldKey => 26,
        PublicChoice::ChooseBossRelicReward { .. } => 27,
        PublicChoice::Proceed => 28,
        PublicChoice::OpenChest => 29,
        PublicChoice::OpenCardReward => 30,
        PublicChoice::OpenQueuedCardReward { .. } => 31,
        PublicChoice::SkipPotionReward => 32,
        PublicChoice::BuyShopCard { .. } => 33,
        PublicChoice::BuyShopRelic { .. } => 34,
        PublicChoice::BuyShopPotion { .. } => 35,
        PublicChoice::UsePotionSlot { .. } => 36,
        PublicChoice::DiscardPotionSlot { .. } => 37,
        PublicChoice::ToggleVisibleCard { .. } => 38,
        PublicChoice::ChooseVisibleOption { .. } => 39,
        PublicChoice::ConfirmSelection => 40,
        PublicChoice::ConfirmSelectionWithoutRetrieval => 41,
        PublicChoice::SkipSelection => 42,
        PublicChoice::EnterShop => 43,
        PublicChoice::LeaveShop => 44,
        PublicChoice::OpenShopRemove => 45,
    };
    debug_assert_eq!(ACTION_KINDS[code as usize], choice.kind());
    code
}

fn optional_slot(value: Option<u16>) -> i64 {
    value.map(i64::from).unwrap_or(-1)
}

/// Public legal-list index plus visible slots. No internal action or instance ids.
fn action_row(
    owner: i64,
    legal_index: i64,
    revision: i64,
    choice: PublicChoice,
) -> [i64; ACTION_ROW_WIDTH] {
    let mut row = [-1; ACTION_ROW_WIDTH];
    row[0] = owner;
    row[1] = legal_index;
    row[2] = kind_code(choice);
    row[11] = revision;
    match choice {
        PublicChoice::PlayHandSlot {
            hand_slot,
            target_slot,
        } => {
            row[3] = i64::from(hand_slot);
            row[6] = optional_slot(target_slot);
        }
        PublicChoice::UsePotionSlot {
            potion_slot,
            target_slot,
        } => {
            row[4] = i64::from(potion_slot);
            row[6] = optional_slot(target_slot);
        }
        PublicChoice::DiscardPotionSlot { potion_slot } => row[4] = i64::from(potion_slot),
        PublicChoice::ChooseEventOption { option_slot }
        | PublicChoice::ToggleVisibleCard { option_slot }
        | PublicChoice::ChooseVisibleOption { option_slot } => row[5] = i64::from(option_slot),
        PublicChoice::ToggleGridCard { card_slot }
        | PublicChoice::RestSmith { card_slot }
        | PublicChoice::RestRemoveCard { card_slot } => row[7] = i64::from(card_slot),
        PublicChoice::ChooseMapNode { node_slot } => row[8] = i64::from(node_slot),
        PublicChoice::TakeCardReward { reward_slot }
        | PublicChoice::TakePotionReward { reward_slot }
        | PublicChoice::TakeRelicRewardAt { reward_slot }
        | PublicChoice::ChooseBossRelicReward { reward_slot }
        | PublicChoice::OpenQueuedCardReward { reward_slot } => row[9] = i64::from(reward_slot),
        PublicChoice::BuyShopCard { shop_slot }
        | PublicChoice::BuyShopRelic { shop_slot }
        | PublicChoice::BuyShopPotion { shop_slot } => row[10] = i64::from(shop_slot),
        _ => {}
    }
    row
}

#[pyfunction]
pub fn action_kind_vocabulary() -> Vec<&'static str> {
    ACTION_KINDS.to_vec()
}
#[derive(Default)]
struct Export {
    symbols: Vec<String>,
    ids: BTreeMap<String, i64>,
    tables: BTreeMap<&'static str, (usize, Vec<i64>)>,
}
impl Export {
    fn symbol(&mut self, key: &str) -> i64 {
        if let Some(&id) = self.ids.get(key) {
            return id;
        }
        let id = self.symbols.len() as i64;
        self.symbols.push(key.to_owned());
        self.ids.insert(key.to_owned(), id);
        id
    }
    fn category(&mut self, value: impl Serialize) -> i64 {
        let key = serde_json::to_value(value).expect("public enum serialization");
        self.symbol(key.as_str().expect("public enum key"))
    }
    fn row(&mut self, name: &'static str, values: &[i64]) -> i64 {
        debug_assert!(
            row_reference(name).is_some(),
            "numeric table {name} has no merge rule"
        );
        let (width, rows) = self
            .tables
            .entry(name)
            .or_insert_with(|| (values.len(), Vec::new()));
        assert_eq!(*width, values.len());
        let index = (rows.len() / *width) as i64;
        rows.extend_from_slice(values);
        index
    }
    fn card(&mut self, group: &'static str, owner: i64, card: &FairCard) -> Result<(), String> {
        let key = card_id(&card.content_key).ok_or_else(|| {
            format!(
                "card '{}' is absent from content vocabulary v1",
                card.content_key
            )
        })?;
        let d = &card.dynamic;
        let dynamic = [
            d.rampage_damage_bonus,
            d.ritual_dagger_damage_bonus,
            d.windmill_retain_damage,
            d.steam_barrier_block_reduction,
            d.combat_cost_under_turn_override,
        ];
        let mut row = [0; 18];
        row[..8].copy_from_slice(&[
            owner,
            key,
            i64::from(card.cost),
            i64::from(card.upgrade_level),
            i64::from(card.cost_is_modified),
            i64::from(card.cost_resets_next_turn),
            i64::from(card.bottled),
            i64::from(card.temporary),
        ]);
        for (index, value) in dynamic.into_iter().enumerate() {
            row[8 + index] = i64::from(value.unwrap_or(0));
            row[13 + index] = i64::from(value.is_some());
        }
        self.row(group, &row);
        Ok(())
    }
    fn enum_name(&self, value: impl Serialize) -> String {
        let key = serde_json::to_value(value).expect("public enum serialization");
        key.as_str().expect("public enum key").to_owned()
    }
    fn observation(&mut self, decision: &FairDecision, owner: i64) -> Result<bool, String> {
        let obs = &decision.observation;
        let kind = self.symbol(obs.screen.kind());
        let phase = self.category(obs.phase);
        let combat_phase = match &obs.screen {
            FairRunScreen::Combat(c) => self.category(c.phase),
            _ => -1,
        };
        self.row(
            "header",
            &[
                kind,
                phase,
                combat_phase,
                i64::from(obs.context.player_hp),
                i64::from(obs.context.player_max_hp),
            ],
        );
        let FairRunScreen::Combat(c) = &obs.screen else {
            return Ok(false);
        };
        if c.phase != FairCombatPhase::WaitingForPlayer {
            return Ok(false);
        }
        let p = &c.player;
        self.row(
            "player",
            &[
                i64::from(p.hp),
                i64::from(p.max_hp),
                i64::from(p.block),
                i64::from(p.energy),
                i64::from(p.max_energy),
                i64::from(obs.context.gold),
            ],
        );
        for power in &p.powers {
            let key = power_id(&power.key).ok_or_else(|| {
                format!("power '{}' is absent from content vocabulary v1", power.key)
            })?;
            self.row("player_powers", &[owner, key, i64::from(power.amount)]);
        }
        for entry in &c.hand {
            self.card("hand", owner, &entry.card)?;
        }
        for (group, pile) in [
            ("draw", &c.draw_pile),
            ("discard", &c.discard_pile),
            ("exhaust", &c.exhaust_pile),
        ] {
            for card in &pile.cards {
                self.card(group, owner, card)?;
            }
        }
        for monster in &c.monsters {
            let key = monster_id(&monster.content_key).ok_or_else(|| {
                format!(
                    "monster '{}' is absent from content vocabulary v1",
                    monster.content_key
                )
            })?;
            let slime_name = monster.slime_size.map(|size| self.enum_name(size));
            let slime = slime_catalog_id(slime_name.as_deref()).ok_or_else(|| {
                format!(
                    "slime size '{}' is absent from content vocabulary v1",
                    slime_name.unwrap_or_default()
                )
            })?;
            let (intent, damage, hits) = match monster.intent {
                FairMonsterIntent::Hidden => {
                    (intent_id("hidden").expect("hidden intent"), None, None)
                }
                FairMonsterIntent::None => (intent_id("none").expect("none intent"), None, None),
                FairMonsterIntent::Visible {
                    category,
                    damage,
                    hits,
                } => {
                    let name = self.enum_name(category);
                    let intent = intent_id(&name).ok_or_else(|| {
                        format!("intent '{name}' is absent from content vocabulary v1")
                    })?;
                    (intent, damage, hits)
                }
            };
            let enemy = self.row(
                "enemies",
                &[
                    owner,
                    key,
                    i64::from(monster.hp),
                    i64::from(monster.max_hp),
                    i64::from(monster.block),
                    i64::from(monster.alive),
                    slime,
                    intent,
                    i64::from(damage.unwrap_or(0)),
                    i64::from(hits.unwrap_or(0)),
                    i64::from(damage.is_some()),
                    i64::from(hits.is_some()),
                    i64::from(monster.escaped),
                    i64::from(monster.minion),
                    i64::from(monster.in_defensive_mode),
                    i64::from(monster.stolen_gold),
                    i64::from(monster.stasis_card.is_some()),
                    i64::from(monster.targetable),
                ],
            );
            for power in &monster.powers {
                let key = power_id(&power.key).ok_or_else(|| {
                    format!("power '{}' is absent from content vocabulary v1", power.key)
                })?;
                self.row("enemy_powers", &[enemy, key, i64::from(power.amount)]);
            }
            if let Some(card) = &monster.stasis_card {
                self.card("stasis", enemy, card)?;
            }
        }
        for relic in &obs.context.relics {
            let key = relic_id(&relic.content_key).ok_or_else(|| {
                format!(
                    "relic '{}' is absent from content vocabulary v1",
                    relic.content_key
                )
            })?;
            let index = self.row("relics", &[owner, key]);
            for counter in &relic.state {
                let key = counter_id(&counter.key).ok_or_else(|| {
                    format!(
                        "counter '{}' is absent from content vocabulary v1",
                        counter.key
                    )
                })?;
                self.row("relic_counters", &[index, key, counter.value]);
            }
        }
        for potion in &obs.context.potion_slots {
            let key = match potion.content_key.as_deref() {
                None => 0,
                Some(name) => potion_id(name).ok_or_else(|| {
                    format!("potion '{name}' is absent from content vocabulary v1")
                })?,
            };
            self.row("potions", &[owner, key, potion.slot as i64]);
        }
        let selection = match c.selection.as_ref() {
            None => 0,
            Some(selection) => {
                let name = self.enum_name(selection.kind);
                selection_catalog_id(Some(&name)).ok_or_else(|| {
                    format!("selection '{name}' is absent from content vocabulary v1")
                })?
            }
        };
        self.row("selection", &[selection]);
        if let Some(s) = &c.selection {
            for option in &s.options {
                self.card("selection_cards", owner, &option.card)?;
                self.row("selection_options", &[owner, option.slot as i64]);
            }
            for &slot in &s.selected_slots {
                self.row("selected_slots", &[owner, slot as i64]);
            }
        }
        Ok(true)
    }
}
/// Tables whose first column is a model-row owner.
const MODEL_OWNED_TABLES: [&str; 11] = [
    "player_powers",
    "hand",
    "draw",
    "discard",
    "exhaust",
    "selection_cards",
    "enemies",
    "relics",
    "potions",
    "selection_options",
    "selected_slots",
];
/// Header columns holding batch-local symbol ids (kind, phase, combat phase).
const HEADER_SYMBOL_COLUMNS: usize = 3;

/// What a table's first column refers to, which decides how `merge` shifts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowReference {
    /// One row per model row, in model-row order; nothing to shift.
    Aligned,
    /// Header rows: the first columns are batch-local symbol ids.
    HeaderSymbols,
    Decision,
    ModelRow,
    Enemy,
    Relic,
}

/// Every table `Export` writes must have a rule; `Export::row` checks this in debug builds.
fn row_reference(name: &str) -> Option<RowReference> {
    Some(match name {
        "header" => RowReference::HeaderSymbols,
        "player" | "selection" => RowReference::Aligned,
        "action_rows" => RowReference::Decision,
        "enemy_powers" | "stasis" => RowReference::Enemy,
        "relic_counters" => RowReference::Relic,
        name if MODEL_OWNED_TABLES.contains(&name) => RowReference::ModelRow,
        _ => return None,
    })
}

/// Tables and model rows for one contiguous run of decisions, indexed locally.
#[derive(Default)]
struct Part {
    out: Export,
    model_rows: Vec<usize>,
    decisions: usize,
}

fn build(decisions: Vec<FairDecision>) -> Result<Part, String> {
    let mut part = Part {
        decisions: decisions.len(),
        ..Part::default()
    };
    for (index, decision) in decisions.into_iter().enumerate() {
        if part
            .out
            .observation(&decision, part.model_rows.len() as i64)?
        {
            part.model_rows.push(index);
        }
        let revision = i64::try_from(decision.revision.get())
            .map_err(|_| "decision revision does not fit the numeric transport".to_owned())?;
        for (legal_index, choice) in decision.choices.into_iter().enumerate() {
            part.out.row(
                "action_rows",
                &action_row(index as i64, legal_index as i64, revision, choice),
            );
        }
    }
    Ok(part)
}

/// Concatenate parts in order. The result is identical to one serial `build`:
/// symbols are interned in first-appearance order and every row reference is
/// shifted by the rows that precede its part.
fn merge(parts: Vec<Part>) -> Part {
    let mut merged = Part::default();
    for part in parts {
        let remap: Vec<i64> = part
            .out
            .symbols
            .iter()
            .map(|symbol| merged.out.symbol(symbol))
            .collect();
        let row_count = |name| {
            merged
                .out
                .tables
                .get(name)
                .map_or(0, |(width, rows): &(usize, Vec<i64>)| rows.len() / width)
                as i64
        };
        let (enemies, relics) = (row_count("enemies"), row_count("relics"));
        let (decisions, models) = (merged.decisions as i64, merged.model_rows.len() as i64);
        for (name, (width, mut rows)) in part.out.tables {
            // Every row reference is the first column.
            let reference = row_reference(name)
                .unwrap_or_else(|| panic!("numeric table {name} has no merge rule"));
            let shift = match reference {
                RowReference::HeaderSymbols => {
                    for row in rows.chunks_exact_mut(width) {
                        for value in &mut row[..HEADER_SYMBOL_COLUMNS] {
                            if *value >= 0 {
                                *value = remap[*value as usize];
                            }
                        }
                    }
                    0
                }
                RowReference::Aligned => 0,
                RowReference::Decision => decisions,
                RowReference::Enemy => enemies,
                RowReference::Relic => relics,
                RowReference::ModelRow => models,
            };
            if shift != 0 {
                for row in rows.chunks_exact_mut(width) {
                    row[0] += shift;
                }
            }
            let (merged_width, merged_rows) = merged
                .out
                .tables
                .entry(name)
                .or_insert_with(|| (width, Vec::new()));
            assert_eq!(*merged_width, width);
            merged_rows.extend(rows);
        }
        merged.model_rows.extend(
            part.model_rows
                .into_iter()
                .map(|row| row + merged.decisions),
        );
        merged.decisions += part.decisions;
    }
    merged
}

fn to_python(py: Python<'_>, part: Part) -> PyResult<Batch> {
    let tables = part
        .out
        .tables
        .into_iter()
        .map(|(key, (width, values))| {
            let bytes = PyBytes::new_with(py, values.len() * 8, |buffer| {
                for (chunk, value) in buffer.as_chunks_mut::<8>().0.iter_mut().zip(values) {
                    chunk.copy_from_slice(&value.to_ne_bytes());
                }
                Ok(())
            })?;
            Ok((key.to_owned(), (width, bytes.unbind())))
        })
        .collect::<PyResult<Tables>>()?;
    Ok((NUMERIC_VERSION, part.out.symbols, tables, part.model_rows))
}

#[pyfunction]
pub fn numeric_decisions(py: Python<'_>, states: Vec<Py<PyState>>) -> PyResult<Batch> {
    let decisions = states
        .iter()
        .map(|state| {
            state
                .borrow(py)
                .env
                .decision()
                .map_err(public_runtime_error)
        })
        .collect::<PyResult<_>>()?;
    let part = build(decisions).map_err(pyo3::exceptions::PyValueError::new_err)?;
    to_python(py, part)
}

/// Every worker gets at least this many states; below it, thread startup costs more than it saves.
const MIN_STATES_PER_WORKER: usize = 64;

/// Contiguous chunk sizes, one per worker: as even as possible, each at least
/// `MIN_STATES_PER_WORKER` when there is more than one, and never more than
/// `max_workers` chunks. A batch too small to split is one chunk.
fn chunk_sizes(states: usize, max_workers: usize) -> Vec<usize> {
    let workers = max_workers.min(states / MIN_STATES_PER_WORKER).max(1);
    let (base, extra) = (states / workers, states % workers);
    (0..workers)
        .map(|index| base + usize::from(index < extra))
        .collect()
}

/// Most worker threads one batch may use.
///
/// Stepping clones whole run states and is memory-bound. On a 12-core/24-thread
/// Ryzen 9 7900X, more than one worker per physical core was slower, so the default
/// is half the logical CPUs. `STS_NUMERIC_THREADS` overrides it (read once), e.g. when
/// several training processes share a machine. The count never changes results.
fn max_workers() -> Result<usize, String> {
    static WORKERS: OnceLock<Result<usize, String>> = OnceLock::new();
    WORKERS
        .get_or_init(|| match std::env::var("STS_NUMERIC_THREADS") {
            Ok(value) => match value.trim().parse::<usize>() {
                Ok(count) if count > 0 => Ok(count),
                _ => Err(format!(
                    "STS_NUMERIC_THREADS must be a positive integer, got {value:?}"
                )),
            },
            Err(std::env::VarError::NotPresent) => Ok(std::thread::available_parallelism()
                .map_or(1, usize::from)
                .div_ceil(2)),
            Err(error) => Err(format!("STS_NUMERIC_THREADS is unreadable: {error}")),
        })
        .clone()
}

fn step(
    environment: &mut FairEnvironment,
    (revision, index): (u64, i64),
) -> Result<FairDecision, String> {
    if index < 0 {
        return Err("choice is invalid".to_owned());
    }
    // One public legality scan, then the existing successor projection.
    // No second observation contract.
    environment
        .step_at_public_index(DecisionRevision::new(revision), index as usize)
        .map_err(|error| error.to_string())
}

/// Step and export one chunk. The outer error is a step error, the inner one an
/// export error, so step errors can take precedence across chunks as they do serially.
fn step_chunk(
    chunk: Vec<(&mut FairEnvironment, (u64, i64))>,
) -> Result<Result<Part, String>, String> {
    let decisions = chunk
        .into_iter()
        .map(|(environment, action)| step(environment, action))
        .collect::<Vec<_>>();
    // Every state in the chunk is attempted before an error is reported.
    let decisions = decisions.into_iter().collect::<Result<Vec<_>, _>>()?;
    Ok(build(decisions))
}

/// Step independent environments and export their successors, in input order.
///
/// Environments share no gameplay state, and nothing a step reads is thread-local
/// in production: RNG trace capture and the clone/validation counters exist only in
/// test builds, and the outer-transaction depth guard is balanced within each step,
/// starting from zero on every thread. A worker therefore produces the same decision
/// as the calling thread, and `merge` reproduces the serial tables.
fn step_independent(
    environments: Vec<&mut FairEnvironment>,
    actions: Vec<(u64, i64)>,
    max_workers: usize,
) -> Result<Part, String> {
    let mut jobs = environments.into_iter().zip(actions);
    let chunks: Vec<Vec<_>> = chunk_sizes(jobs.len(), max_workers)
        .into_iter()
        .map(|size| jobs.by_ref().take(size).collect())
        .collect();
    let results = if chunks.len() <= 1 {
        chunks.into_iter().map(step_chunk).collect::<Vec<_>>()
    } else {
        std::thread::scope(|scope| {
            let handles: Vec<_> = chunks
                .into_iter()
                .map(|chunk| scope.spawn(move || step_chunk(chunk)))
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("numeric step worker panicked"))
                .collect()
        })
    };
    // Any step error wins over any export error, matching step-all-then-export.
    let exported = results.into_iter().collect::<Result<Vec<_>, _>>()?;
    let mut parts = exported.into_iter().collect::<Result<Vec<_>, _>>()?;
    // One part is already the serial export; merging it would only copy it.
    Ok(if parts.len() == 1 {
        parts.pop().expect("one part")
    } else {
        merge(parts)
    })
}

#[pyfunction]
pub fn numeric_steps(
    py: Python<'_>,
    states: Vec<Py<PyState>>,
    indices: Vec<i64>,
    revisions: Vec<u64>,
) -> PyResult<Batch> {
    if states.len() != indices.len() || states.len() != revisions.len() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "State/action batch lengths differ",
        ));
    }
    // Every borrow is held for the whole batch, so a repeated state is rejected
    // before any state is stepped.
    let mut borrowed = states
        .iter()
        .map(|state| {
            state.try_borrow_mut(py).map_err(|_| {
                pyo3::exceptions::PyValueError::new_err(
                    "state is repeated in the batch or already borrowed",
                )
            })
        })
        .collect::<PyResult<Vec<_>>>()?;
    let workers = max_workers().map_err(pyo3::exceptions::PyValueError::new_err)?;
    let environments = borrowed.iter_mut().map(|state| &mut state.env).collect();
    let actions = revisions.into_iter().zip(indices).collect();
    // Not atomic: every state is attempted and the first error in input order is
    // reported. Callers discard the whole batch on any error.
    let part = py
        .detach(|| step_independent(environments, actions, workers))
        .map_err(pyo3::exceptions::PyValueError::new_err)?;
    drop(borrowed);
    to_python(py, part)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::{action_row, card_id, ACTION_KINDS};
    use sts_env::{
        FairCardDynamicValues, FairEnvironment, FairIntentCategory, FairSelection,
        FairSelectionKind, FairSelectionOption, PublicChoice, PublicChoiceRequest,
    };

    fn card() -> FairCard {
        FairCard {
            content_key: "Strike_R".to_owned(),
            cost: -1,
            upgrade_level: 2,
            cost_is_modified: true,
            cost_resets_next_turn: false,
            bottled: true,
            temporary: false,
            dynamic: FairCardDynamicValues {
                rampage_damage_bonus: Some(17),
                ritual_dagger_damage_bonus: Some(0),
                windmill_retain_damage: Some(31),
                steam_barrier_block_reduction: Some(5),
                combat_cost_under_turn_override: Some(0),
            },
        }
    }

    #[test]
    fn raw_card_columns_preserve_signed_values_and_optional_zero() {
        let mut out = Export::default();
        out.card("hand", 4, &card()).unwrap();
        let expected = vec![
            4,
            card_id("Strike_R").unwrap(),
            -1,
            2,
            1,
            0,
            1,
            0,
            17,
            0,
            31,
            5,
            0,
            1,
            1,
            1,
            1,
            1,
        ];
        assert_eq!(out.tables["hand"].1, expected);
        let mut absent = card();
        absent.dynamic = FairCardDynamicValues::default();
        out.card("draw", 4, &absent).unwrap();
        assert_eq!(&out.tables["draw"].1[8..], &[0; 10]);
        assert!(out.symbols.is_empty());
    }

    #[test]
    fn public_fixture_stasis_selection_and_dead_slots_keep_their_references() {
        let mut env =
            FairEnvironment::new_ironclad(sts_env::parse_seed("HUMAN1").unwrap(), 0).unwrap();
        let mut decision = env.decision().unwrap();
        for _ in 0..100 {
            if matches!(decision.observation.screen, FairRunScreen::Combat(_)) {
                break;
            }
            decision = env
                .step(PublicChoiceRequest {
                    revision: decision.revision,
                    choice: decision.choices[0],
                })
                .unwrap();
        }
        // Modify a PUBLIC DTO fixture, never simulator state or an observed trace.
        let FairRunScreen::Combat(c) = &mut decision.observation.screen else {
            panic!("fixture did not reach combat")
        };
        c.monsters[0].stasis_card = Some(card());
        c.monsters[0].hp = 0;
        c.monsters[0].alive = false;
        c.monsters[0].intent = FairMonsterIntent::Visible {
            category: FairIntentCategory::Attack,
            damage: Some(0),
            hits: None,
        };
        c.selection = Some(FairSelection {
            kind: FairSelectionKind::DiscoveryReward,
            options: vec![FairSelectionOption {
                slot: 0,
                card: card(),
            }],
            selected_slots: vec![0],
        });
        let mut out = Export::default();
        assert!(out.observation(&decision, 0).unwrap());
        assert_eq!(&out.tables["enemies"].1[8..12], &[0, 0, 1, 0]);
        assert_eq!(out.tables["enemies"].1[2], 0);
        assert_eq!(out.tables["enemies"].1[5], 0);
        assert_eq!(out.tables["enemies"].1[16], 1);
        assert_eq!(out.tables["stasis"].1[0], 0);
        assert_eq!(out.tables["selection_options"].1, [0, 0]);
        assert_eq!(out.tables["selected_slots"].1, [0, 0]);
    }

    /// Environments walked by a fixed index pattern, one per seed, with their
    /// visited decisions. Covers map/event screens as well as combat.
    fn walked(steps: usize) -> (Vec<FairEnvironment>, Vec<FairDecision>) {
        let mut environments = Vec::new();
        let mut decisions = Vec::new();
        for seed in 1..=12_u64 {
            let mut env = FairEnvironment::new_ironclad(seed, 0).unwrap();
            let mut decision = env.decision().unwrap();
            for step in 0..steps {
                decisions.push(decision.clone());
                if decision.choices.is_empty() {
                    break;
                }
                let index = (step * 7 + seed as usize) % decision.choices.len();
                decision = env.step_at_public_index(decision.revision, index).unwrap();
            }
            environments.push(env);
        }
        (environments, decisions)
    }

    fn assert_same(left: &Part, right: &Part) {
        assert_eq!(left.out.symbols, right.out.symbols);
        assert_eq!(left.out.tables, right.out.tables);
        assert_eq!(left.model_rows, right.model_rows);
        assert_eq!(left.decisions, right.decisions);
    }

    #[test]
    fn merged_chunks_equal_one_serial_export() {
        let (_, decisions) = walked(120);
        let serial = build(decisions.clone()).unwrap();
        for tables in ["enemies", "enemy_powers", "relics", "action_rows"] {
            assert!(
                serial.out.tables.contains_key(tables),
                "fixture lacks {tables}"
            );
        }
        assert!(!serial.model_rows.is_empty() && serial.model_rows.len() < decisions.len());
        for size in [1, 7, 64, decisions.len()] {
            let parts = decisions
                .chunks(size)
                .map(|chunk| build(chunk.to_vec()).unwrap())
                .collect();
            assert_same(&serial, &merge(parts));
        }
    }

    #[test]
    fn parallel_steps_match_serial_steps() {
        let (environments, _) = walked(40);
        // Enough states that the batch is split across workers.
        let mut serial: Vec<_> = (0..MIN_STATES_PER_WORKER * 4)
            .map(|index| environments[index % environments.len()].clone())
            .collect();
        let actions: Vec<_> = serial
            .iter()
            .enumerate()
            .map(|(index, env)| {
                let decision = env.decision().unwrap();
                let count = decision.choices.len().max(1);
                (decision.revision.get(), (index % count) as i64)
            })
            .collect();
        let expected = build(
            serial
                .iter_mut()
                .zip(actions.iter().copied())
                .map(|(env, action)| step(env, action).unwrap())
                .collect(),
        )
        .unwrap();
        // 1 is the calling-thread path; the others split the batch unevenly.
        for workers in [1, 2, 3, 8] {
            let mut parallel = environments_before(&environments, MIN_STATES_PER_WORKER * 4);
            let actual =
                step_independent(parallel.iter_mut().collect(), actions.clone(), workers).unwrap();
            assert_same(&expected, &actual);
            for (left, right) in serial.iter().zip(&parallel) {
                assert_eq!(left.decision().unwrap(), right.decision().unwrap());
            }
        }
    }

    fn environments_before(environments: &[FairEnvironment], count: usize) -> Vec<FairEnvironment> {
        (0..count)
            .map(|index| environments[index % environments.len()].clone())
            .collect()
    }

    #[test]
    fn chunks_are_balanced_and_never_below_the_minimum() {
        assert_eq!(chunk_sizes(0, 12), [0]);
        assert_eq!(chunk_sizes(127, 12), [127]);
        assert_eq!(chunk_sizes(128, 12), [64, 64]);
        // div_ceil sizing would have left a final chunk of 57 here.
        assert_eq!(
            chunk_sizes(651, 10),
            [66, 65, 65, 65, 65, 65, 65, 65, 65, 65]
        );
        for states in 0..3000 {
            for max_workers in 1..=24 {
                let sizes = chunk_sizes(states, max_workers);
                assert_eq!(sizes.iter().sum::<usize>(), states);
                assert!(sizes.len() <= max_workers);
                if sizes.len() > 1 {
                    assert!(sizes.iter().all(|&size| size >= MIN_STATES_PER_WORKER));
                    assert!(sizes.iter().max().unwrap() - sizes.iter().min().unwrap() <= 1);
                }
            }
        }
    }

    #[test]
    fn merge_shifts_every_row_reference() {
        // Two hand-built parts that write every table; the first column of each row is
        // a local reference, the second a marker that must survive unchanged.
        fn part(symbols: &[&str], decisions: usize) -> Part {
            let mut part = Part {
                decisions,
                ..Part::default()
            };
            let ids: Vec<i64> = symbols.iter().map(|s| part.out.symbol(s)).collect();
            for decision in 0..decisions as i64 {
                part.out.row("header", &[ids[0], ids[1], -1, 7, 9]);
                part.out.row("action_rows", &[decision, 100 + decision]);
            }
            part.model_rows = (0..decisions).collect();
            for table in MODEL_OWNED_TABLES {
                part.out.row(table, &[1, 200]);
            }
            for table in ["player", "selection"] {
                part.out.row(table, &[300, 301]);
            }
            for (table, reference) in [("enemy_powers", 0), ("stasis", 0), ("relic_counters", 0)] {
                part.out.row(table, &[reference, 400]);
            }
            part
        }
        let first = part(&["combat", "turn"], 2);
        let second = part(&["reward", "combat"], 3);
        for name in first.out.tables.keys() {
            assert!(row_reference(name).is_some(), "{name}");
        }
        let merged = merge(vec![first, second]);
        assert_eq!(merged.out.symbols, ["combat", "turn", "reward"]);
        assert_eq!(merged.decisions, 5);
        assert_eq!(merged.model_rows, [0, 1, 2, 3, 4]);
        let column = |name: &str, column: usize| -> Vec<i64> {
            let (width, rows) = &merged.out.tables[name];
            rows.chunks_exact(*width).map(|row| row[column]).collect()
        };
        assert_eq!(column("header", 0), [0, 0, 2, 2, 2]);
        assert_eq!(column("header", 1), [1, 1, 0, 0, 0]);
        assert_eq!(column("header", 2), [-1; 5]);
        assert_eq!(column("action_rows", 0), [0, 1, 2, 3, 4]);
        assert_eq!(column("action_rows", 1), [100, 101, 100, 101, 102]);
        for table in MODEL_OWNED_TABLES {
            // Shifted by the first part's two model rows.
            assert_eq!(column(table, 0), [1, 3], "{table}");
            assert_eq!(column(table, 1), [200, 200], "{table}");
        }
        assert_eq!(column("player", 0), [300, 300]);
        // One enemy and one relic row in the first part.
        assert_eq!(column("enemy_powers", 0), [0, 1]);
        assert_eq!(column("stasis", 0), [0, 1]);
        assert_eq!(column("relic_counters", 0), [0, 1]);
    }

    #[test]
    fn parallel_step_errors_report_the_first_state_in_input_order() {
        let (environments, _) = walked(10);
        let mut states: Vec<_> = (0..MIN_STATES_PER_WORKER * 3)
            .map(|index| environments[index % environments.len()].clone())
            .collect();
        let mut actions: Vec<_> = states
            .iter()
            .map(|env| (env.decision().unwrap().revision.get(), 0))
            .collect();
        actions[MIN_STATES_PER_WORKER * 2].1 = -1;
        actions[MIN_STATES_PER_WORKER + 1].0 += 1;
        let error = step_independent(states.iter_mut().collect(), actions, 4)
            .err()
            .unwrap();
        assert!(error.contains("stale"), "{error}");
    }

    #[test]
    fn action_kind_codes_match_public_names_and_slots() {
        let samples = [
            PublicChoice::PlayHandSlot {
                hand_slot: 3,
                target_slot: Some(1),
            },
            PublicChoice::EndTurn,
            PublicChoice::ChooseEventOption { option_slot: 2 },
            PublicChoice::ToggleGridCard { card_slot: 4 },
            PublicChoice::ConfirmGrid,
            PublicChoice::CancelGrid,
            PublicChoice::ChooseMapNode { node_slot: 5 },
            PublicChoice::RestHeal,
            PublicChoice::RestOpenSmith,
            PublicChoice::RestOpenRemove,
            PublicChoice::RestSmith { card_slot: 6 },
            PublicChoice::RestRemoveCard { card_slot: 7 },
            PublicChoice::RestLift,
            PublicChoice::RestDig,
            PublicChoice::RestRecall,
            PublicChoice::RestProceed,
            PublicChoice::SkipReward,
            PublicChoice::CloseCardReward,
            PublicChoice::TakeCardReward { reward_slot: 1 },
            PublicChoice::TakeSingingBowlReward,
            PublicChoice::TakeGoldReward,
            PublicChoice::TakeStolenGoldReward,
            PublicChoice::TakePotionReward { reward_slot: 2 },
            PublicChoice::TakeRelicReward,
            PublicChoice::TakeRelicRewardAt { reward_slot: 3 },
            PublicChoice::TakeSapphireKey,
            PublicChoice::TakeEmeraldKey,
            PublicChoice::ChooseBossRelicReward { reward_slot: 0 },
            PublicChoice::Proceed,
            PublicChoice::OpenChest,
            PublicChoice::OpenCardReward,
            PublicChoice::OpenQueuedCardReward { reward_slot: 8 },
            PublicChoice::SkipPotionReward,
            PublicChoice::BuyShopCard { shop_slot: 1 },
            PublicChoice::BuyShopRelic { shop_slot: 2 },
            PublicChoice::BuyShopPotion { shop_slot: 3 },
            PublicChoice::UsePotionSlot {
                potion_slot: 1,
                target_slot: None,
            },
            PublicChoice::DiscardPotionSlot { potion_slot: 2 },
            PublicChoice::ToggleVisibleCard { option_slot: 0 },
            PublicChoice::ChooseVisibleOption { option_slot: 1 },
            PublicChoice::ConfirmSelection,
            PublicChoice::ConfirmSelectionWithoutRetrieval,
            PublicChoice::SkipSelection,
            PublicChoice::EnterShop,
            PublicChoice::LeaveShop,
            PublicChoice::OpenShopRemove,
        ];
        assert_eq!(samples.len(), ACTION_KINDS.len());
        for (expected, choice) in samples.into_iter().enumerate() {
            let row = action_row(9, 4, 7, choice);
            assert_eq!(row[0], 9);
            assert_eq!(row[1], 4);
            assert_eq!(row[2], expected as i64);
            assert_eq!(ACTION_KINDS[expected], choice.kind());
            assert_eq!(row[11], 7);
        }
        let targeted = action_row(
            0,
            0,
            1,
            PublicChoice::PlayHandSlot {
                hand_slot: 3,
                target_slot: Some(1),
            },
        );
        assert_eq!(targeted[3], 3);
        assert_eq!(targeted[6], 1);
        let untargeted = action_row(
            0,
            0,
            1,
            PublicChoice::UsePotionSlot {
                potion_slot: 2,
                target_slot: None,
            },
        );
        assert_eq!(untargeted[4], 2);
        assert_eq!(untargeted[6], -1);
    }
}
