//! Direct numeric transport of the combat policy's PUBLIC input fields.
//! Export consumes FairDecision only. Stepping indexes the current public legal list.
use crate::{public_runtime_error, PyState};
use pyo3::{prelude::*, types::PyBytes};
use serde::Serialize;
use std::collections::BTreeMap;
use sts_env::{
    DecisionRevision, FairCard, FairCombatPhase, FairDecision, FairMonsterIntent, FairRunScreen,
    PublicChoice, PublicChoiceRequest,
};

type Tables = BTreeMap<String, (usize, Py<PyBytes>)>;
type Batch = (u32, Vec<String>, Tables, Vec<usize>);
pub const NUMERIC_VERSION: u32 = 2;
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
        let (width, rows) = self
            .tables
            .entry(name)
            .or_insert_with(|| (values.len(), Vec::new()));
        assert_eq!(*width, values.len());
        let index = (rows.len() / *width) as i64;
        rows.extend_from_slice(values);
        index
    }
    fn card(&mut self, group: &'static str, owner: i64, card: &FairCard) {
        let key = self.symbol(&card.content_key);
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
    }
    fn observation(&mut self, decision: &FairDecision, owner: i64) -> bool {
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
            return false;
        };
        if c.phase != FairCombatPhase::WaitingForPlayer {
            return false;
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
            let key = self.symbol(&power.key);
            self.row("player_powers", &[owner, key, i64::from(power.amount)]);
        }
        for entry in &c.hand {
            self.card("hand", owner, &entry.card);
        }
        for (group, pile) in [
            ("draw", &c.draw_pile),
            ("discard", &c.discard_pile),
            ("exhaust", &c.exhaust_pile),
        ] {
            for card in &pile.cards {
                self.card(group, owner, card);
            }
        }
        for monster in &c.monsters {
            let key = self.symbol(&monster.content_key);
            let slime = monster.slime_size.map_or(-1, |size| self.category(size));
            let (intent, damage, hits) = match monster.intent {
                FairMonsterIntent::Hidden => (self.symbol("hidden"), None, None),
                FairMonsterIntent::None => (self.symbol("none"), None, None),
                FairMonsterIntent::Visible {
                    category,
                    damage,
                    hits,
                } => (self.category(category), damage, hits),
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
                let key = self.symbol(&power.key);
                self.row("enemy_powers", &[enemy, key, i64::from(power.amount)]);
            }
            if let Some(card) = &monster.stasis_card {
                self.card("stasis", enemy, card);
            }
        }
        for relic in &obs.context.relics {
            let key = self.symbol(&relic.content_key);
            let index = self.row("relics", &[owner, key]);
            for counter in &relic.state {
                let key = self.symbol(&counter.key);
                self.row("relic_counters", &[index, key, counter.value]);
            }
        }
        for potion in &obs.context.potion_slots {
            let key = potion.content_key.as_ref().map_or(-1, |k| self.symbol(k));
            self.row("potions", &[owner, key, potion.slot as i64]);
        }
        let selection = c.selection.as_ref().map_or(-1, |s| self.category(s.kind));
        self.row("selection", &[selection]);
        if let Some(s) = &c.selection {
            for option in &s.options {
                self.card("selection_cards", owner, &option.card);
                self.row("selection_options", &[owner, option.slot as i64]);
            }
            for &slot in &s.selected_slots {
                self.row("selected_slots", &[owner, slot as i64]);
            }
        }
        true
    }
}
fn export(py: Python<'_>, decisions: Vec<FairDecision>) -> PyResult<Batch> {
    let mut out = Export::default();
    let mut model_rows = Vec::new();
    for (index, decision) in decisions.into_iter().enumerate() {
        if out.observation(&decision, model_rows.len() as i64) {
            model_rows.push(index);
        }
        let revision = i64::try_from(decision.revision.get()).map_err(|_| {
            pyo3::exceptions::PyValueError::new_err(
                "decision revision does not fit the numeric transport",
            )
        })?;
        for (legal_index, choice) in decision.choices.into_iter().enumerate() {
            out.row(
                "action_rows",
                &action_row(index as i64, legal_index as i64, revision, choice),
            );
        }
    }
    let tables = out
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
    Ok((NUMERIC_VERSION, out.symbols, tables, model_rows))
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
    export(py, decisions)
}
fn resolve_public_index(
    env: &sts_env::FairEnvironment,
    index: i64,
    revision: u64,
) -> PyResult<PublicChoice> {
    // Reject the exported revision before interpreting the index against a newer list.
    if env.revision().get() != revision {
        return Err(pyo3::exceptions::PyValueError::new_err("decision is stale"));
    }
    if index < 0 {
        return Err(pyo3::exceptions::PyValueError::new_err("choice is invalid"));
    }
    let choices = env
        .legal_choices()
        .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
    choices
        .get(index as usize)
        .copied()
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("choice is invalid"))
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
    let decisions = states
        .iter()
        .zip(indices)
        .zip(revisions)
        .map(|((state, index), revision)| {
            let choice = resolve_public_index(&state.borrow(py).env, index, revision)?;
            state
                .borrow_mut(py)
                .env
                .step(PublicChoiceRequest {
                    revision: DecisionRevision::new(revision),
                    choice,
                })
                .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))
        })
        .collect::<PyResult<_>>()?;
    export(py, decisions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::{action_row, ACTION_KINDS};
    use sts_env::{
        FairCardDynamicValues, FairEnvironment, FairIntentCategory, FairSelection,
        FairSelectionKind, FairSelectionOption, PublicChoice,
    };

    fn card() -> FairCard {
        FairCard {
            content_key: "strike".to_owned(),
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
        out.card("hand", 4, &card());
        assert_eq!(
            out.tables["hand"].1,
            [4, 0, -1, 2, 1, 0, 1, 0, 17, 0, 31, 5, 0, 1, 1, 1, 1, 1]
        );
        let mut absent = card();
        absent.dynamic = FairCardDynamicValues::default();
        out.card("draw", 4, &absent);
        assert_eq!(&out.tables["draw"].1[8..], &[0; 10]);
        assert_eq!(out.symbols, ["strike"]);
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
        assert!(out.observation(&decision, 0));
        assert_eq!(&out.tables["enemies"].1[8..12], &[0, 0, 1, 0]);
        assert_eq!(out.tables["enemies"].1[2], 0);
        assert_eq!(out.tables["enemies"].1[5], 0);
        assert_eq!(out.tables["enemies"].1[16], 1);
        assert_eq!(out.tables["stasis"].1[0], 0);
        assert_eq!(out.tables["selection_options"].1, [0, 0]);
        assert_eq!(out.tables["selected_slots"].1, [0, 0]);
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
