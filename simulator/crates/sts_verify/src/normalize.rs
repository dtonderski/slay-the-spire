//! Canonical observed-state normalization for CommunicationMod exports.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalMonsterObservation {
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub intent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalCombatObservation {
    pub player_hp: i32,
    pub player_max_hp: i32,
    pub player_block: i32,
    pub player_energy: i32,
    pub hand_size: usize,
    pub draw_pile_size: usize,
    pub discard_pile_size: usize,
    pub monsters: Vec<CanonicalMonsterObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalRunObservation {
    pub ascension: u32,
    pub floor: u32,
    pub gold: i32,
    pub current_hp: i32,
    pub max_hp: i32,
    pub deck_size: usize,
    pub in_combat: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub combat: Option<CanonicalCombatObservation>,
}

/// Normalize a CommunicationMod `message` value into canonical run observation.
#[must_use]
pub fn normalize_communication_mod_message(message: &Value) -> Option<CanonicalRunObservation> {
    let game_state = message.get("game_state")?;
    let combat = game_state
        .get("combat_state")
        .and_then(normalize_combat_state);

    Some(CanonicalRunObservation {
        ascension: game_state
            .get("ascension_level")
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32,
        floor: game_state.get("floor").and_then(Value::as_u64).unwrap_or(0) as u32,
        gold: game_state.get("gold").and_then(Value::as_i64).unwrap_or(0) as i32,
        current_hp: game_state
            .get("current_hp")
            .and_then(Value::as_i64)
            .unwrap_or(0) as i32,
        max_hp: game_state
            .get("max_hp")
            .and_then(Value::as_i64)
            .unwrap_or(0) as i32,
        deck_size: game_state
            .get("deck")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        in_combat: combat.is_some(),
        combat,
    })
}

#[must_use]
pub fn normalize_combat_state(combat: &Value) -> Option<CanonicalCombatObservation> {
    let player = combat.get("player")?;
    let monsters = combat
        .get("monsters")
        .and_then(Value::as_array)
        .map(|monsters| {
            monsters
                .iter()
                .filter_map(normalize_monster)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(CanonicalCombatObservation {
        player_hp: player
            .get("current_hp")
            .and_then(Value::as_i64)
            .unwrap_or(0) as i32,
        player_max_hp: player.get("max_hp").and_then(Value::as_i64).unwrap_or(0) as i32,
        player_block: player.get("block").and_then(Value::as_i64).unwrap_or(0) as i32,
        player_energy: player.get("energy").and_then(Value::as_i64).unwrap_or(0) as i32,
        hand_size: combat
            .get("hand")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        draw_pile_size: combat
            .get("draw_pile")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        discard_pile_size: combat
            .get("discard_pile")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        monsters,
    })
}

fn normalize_monster(monster: &Value) -> Option<CanonicalMonsterObservation> {
    Some(CanonicalMonsterObservation {
        name: monster
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned(),
        hp: monster
            .get("current_hp")
            .and_then(Value::as_i64)
            .unwrap_or(0) as i32,
        max_hp: monster.get("max_hp").and_then(Value::as_i64).unwrap_or(0) as i32,
        block: monster.get("block").and_then(Value::as_i64).unwrap_or(0) as i32,
        intent: monster
            .get("intent")
            .and_then(Value::as_str)
            .unwrap_or("UNKNOWN")
            .to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_cultist_combat_from_fixture_shape() {
        let message = json!({
            "game_state": {
                "ascension_level": 0,
                "floor": 1,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": [{}, {}, {}],
                "combat_state": {
                    "player": { "current_hp": 80, "max_hp": 80, "block": 0, "energy": 1 },
                    "hand": [{}, {}, {}, {}],
                    "draw_pile": [{}, {}, {}, {}, {}],
                    "discard_pile": [{}],
                    "monsters": [{
                        "name": "Cultist",
                        "current_hp": 41,
                        "max_hp": 49,
                        "block": 0,
                        "intent": "BUFF"
                    }]
                }
            }
        });

        let observed = normalize_communication_mod_message(&message).expect("normalized");
        let combat = observed.combat.expect("combat");
        assert_eq!(combat.player_energy, 1);
        assert_eq!(combat.monsters[0].hp, 41);
        assert_eq!(combat.discard_pile_size, 1);
    }
}
