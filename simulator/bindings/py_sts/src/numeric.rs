//! Direct numeric transport of the combat policy's PUBLIC input fields.
//! Consumes FairDecision only; has no access to authoritative state or RL code.
use crate::{public_runtime_error, py_action, PyAction, PyState};
use pyo3::{prelude::*, types::PyBytes};
use serde::Serialize;
use std::collections::BTreeMap;
use sts_env::{
    FairCard, FairCombatPhase, FairDecision, FairMonsterIntent, FairRunScreen, PublicChoiceRequest,
};

type Tables = BTreeMap<String, (usize, Py<PyBytes>)>;
type Batch = (u32, Vec<String>, Tables, Vec<Vec<PyAction>>, Vec<usize>);
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
    let mut actions = Vec::new();
    let mut model_rows = Vec::new();
    for (index, decision) in decisions.into_iter().enumerate() {
        if out.observation(&decision, model_rows.len() as i64) {
            model_rows.push(index);
        }
        actions.push(
            decision
                .choices
                .into_iter()
                .map(|choice| py_action(choice, decision.revision))
                .collect(),
        );
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
    Ok((1, out.symbols, tables, actions, model_rows))
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
#[pyfunction]
pub fn numeric_steps(
    py: Python<'_>,
    states: Vec<Py<PyState>>,
    actions: Vec<Py<PyAction>>,
) -> PyResult<Batch> {
    if states.len() != actions.len() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "State/action batch lengths differ",
        ));
    }
    let decisions = states
        .iter()
        .zip(actions)
        .map(|(state, action)| {
            let action = action.borrow(py);
            state
                .borrow_mut(py)
                .env
                .step(PublicChoiceRequest {
                    revision: action.revision,
                    choice: action.choice,
                })
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
        })
        .collect::<PyResult<_>>()?;
    export(py, decisions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts_env::{
        FairCardDynamicValues, FairEnvironment, FairIntentCategory, FairSelection,
        FairSelectionKind, FairSelectionOption,
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
}
