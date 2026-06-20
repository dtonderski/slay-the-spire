use crate::action::InternalAction;
use crate::card::CardType;
use crate::combat::CombatState;
use serde::{Deserialize, Serialize};

use crate::ids::ContentId;

/// Strength granted by [Relic::Vajra] at combat start.
pub const VAJRA_STRENGTH: i32 = 1;
/// Dexterity granted by [Relic::OddlySmoothStone] at combat start.
pub const ODDLY_SMOOTH_STONE_DEXTERITY: i32 = 1;
/// Max HP granted by [Relic::Strawberry] on pickup.
pub const STRAWBERRY_MAX_HP: i32 = 7;
/// Energy per turn granted by [Relic::CoffeeDripper] on pickup.
pub const COFFEE_DRIPPER_ENERGY: i32 = 1;
/// Block granted by [Relic::Anchor] at combat start.
pub const ANCHOR_BLOCK: i32 = 10;
/// Cards played before [Relic::InkBottle] draws a card.
pub const INK_BOTTLE_THRESHOLD: u32 = 10;
/// Attacks played in one turn before [Relic::OrnamentalFan] grants block.
pub const ORNAMENTAL_FAN_THRESHOLD: u32 = 3;
/// Block granted by [Relic::OrnamentalFan] every third attack in a turn.
pub const ORNAMENTAL_FAN_BLOCK: i32 = 4;

/// Content id for [Relic::Vajra].
pub const VAJRA_ID: ContentId = ContentId::new(300);
/// Content id for [Relic::OddlySmoothStone].
pub const ODDLY_SMOOTH_STONE_ID: ContentId = ContentId::new(301);
/// Content id for [Relic::Strawberry].
pub const STRAWBERRY_ID: ContentId = ContentId::new(302);
/// Content id for [Relic::CoffeeDripper].
pub const COFFEE_DRIPPER_ID: ContentId = ContentId::new(303);
/// Content id for [Relic::Anchor].
pub const ANCHOR_ID: ContentId = ContentId::new(304);
/// Content id for [Relic::InkBottle].
pub const INK_BOTTLE_ID: ContentId = ContentId::new(305);
/// Content id for [Relic::OrnamentalFan].
pub const ORNAMENTAL_FAN_ID: ContentId = ContentId::new(306);
/// Content id for [Relic::IceCream].
pub const ICE_CREAM_ID: ContentId = ContentId::new(307);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RelicCounters {
    #[serde(default)]
    pub ink_bottle_cards_played: u32,
    #[serde(default)]
    pub ornamental_fan_attacks_this_turn: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelicTier {
    Common,
    Uncommon,
    Rare,
    Boss,
    Shop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Relic {
    Vajra,
    OddlySmoothStone,
    Strawberry,
    CoffeeDripper,
    Anchor,
    InkBottle,
    OrnamentalFan,
    IceCream,
}

impl Relic {
    #[must_use]
    pub fn content_id(self) -> ContentId {
        match self {
            Relic::Vajra => VAJRA_ID,
            Relic::OddlySmoothStone => ODDLY_SMOOTH_STONE_ID,
            Relic::Strawberry => STRAWBERRY_ID,
            Relic::CoffeeDripper => COFFEE_DRIPPER_ID,
            Relic::Anchor => ANCHOR_ID,
            Relic::InkBottle => INK_BOTTLE_ID,
            Relic::OrnamentalFan => ORNAMENTAL_FAN_ID,
            Relic::IceCream => ICE_CREAM_ID,
        }
    }

    #[must_use]
    pub fn from_content_id(id: ContentId) -> Option<Self> {
        match id {
            id if id == VAJRA_ID => Some(Relic::Vajra),
            id if id == ODDLY_SMOOTH_STONE_ID => Some(Relic::OddlySmoothStone),
            id if id == STRAWBERRY_ID => Some(Relic::Strawberry),
            id if id == COFFEE_DRIPPER_ID => Some(Relic::CoffeeDripper),
            id if id == ANCHOR_ID => Some(Relic::Anchor),
            id if id == INK_BOTTLE_ID => Some(Relic::InkBottle),
            id if id == ORNAMENTAL_FAN_ID => Some(Relic::OrnamentalFan),
            id if id == ICE_CREAM_ID => Some(Relic::IceCream),
            _ => None,
        }
    }
}

pub fn apply_start_of_combat_relics(combat: &mut CombatState, relics: &[Relic]) {
    for relic in relics {
        match relic {
            Relic::Vajra => {
                combat.player.powers.strength += VAJRA_STRENGTH;
            }
            Relic::OddlySmoothStone => {
                combat.player.powers.dexterity += ODDLY_SMOOTH_STONE_DEXTERITY;
            }
            Relic::Strawberry => {}
            Relic::CoffeeDripper => {}
            Relic::Anchor => {
                combat.player.block += ANCHOR_BLOCK;
            }
            Relic::InkBottle => {}
            Relic::OrnamentalFan => {}
            Relic::IceCream => {}
        }
    }
}

/// Whether player energy should carry over instead of refilling at turn start.
#[must_use]
pub fn preserves_energy_between_turns(relics: &[Relic]) -> bool {
    relics.contains(&Relic::IceCream)
}

pub fn reset_turn_relic_counters(state: &mut CombatState) {
    state.relic_counters.ornamental_fan_attacks_this_turn = 0;
}

#[must_use]
pub fn apply_on_card_play_relics(
    state: &mut CombatState,
    card_type: CardType,
) -> Vec<InternalAction> {
    let mut follow_ups = Vec::new();

    if state.relics.contains(&Relic::InkBottle) {
        state.relic_counters.ink_bottle_cards_played += 1;
        if state.relic_counters.ink_bottle_cards_played >= INK_BOTTLE_THRESHOLD {
            state.relic_counters.ink_bottle_cards_played = 0;
            follow_ups.push(InternalAction::DrawCards { count: 1 });
        }
    }

    if state.relics.contains(&Relic::OrnamentalFan) && card_type == CardType::Attack {
        state.relic_counters.ornamental_fan_attacks_this_turn += 1;
        if state.relic_counters.ornamental_fan_attacks_this_turn >= ORNAMENTAL_FAN_THRESHOLD {
            state.relic_counters.ornamental_fan_attacks_this_turn = 0;
            follow_ups.push(InternalAction::GainBlock {
                amount: ORNAMENTAL_FAN_BLOCK,
            });
        }
    }

    follow_ups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::CombatState;

    #[test]
    fn vajra_grants_one_strength_at_combat_start() {
        let mut combat = CombatState::initial_fixture();

        apply_start_of_combat_relics(&mut combat, &[Relic::Vajra]);

        assert_eq!(combat.player.powers.strength, VAJRA_STRENGTH);
    }

    #[test]
    fn start_of_combat_relics_without_vajra_leaves_strength_unchanged() {
        let mut combat = CombatState::initial_fixture();

        apply_start_of_combat_relics(&mut combat, &[]);

        assert_eq!(combat.player.powers.strength, 0);
    }

    #[test]
    fn relic_round_trips_through_json() {
        let relic = Relic::Vajra;

        let json = serde_json::to_string(&relic).expect("relic serializes");
        let restored: Relic = serde_json::from_str(&json).expect("relic deserializes");

        assert_eq!(restored, relic);
    }

    #[test]
    fn oddly_smooth_stone_grants_one_dexterity_at_combat_start() {
        let mut combat = CombatState::initial_fixture();

        apply_start_of_combat_relics(&mut combat, &[Relic::OddlySmoothStone]);

        assert_eq!(combat.player.powers.dexterity, ODDLY_SMOOTH_STONE_DEXTERITY);
    }

    #[test]
    fn relic_content_ids_map_both_ways() {
        assert_eq!(Relic::Vajra.content_id(), VAJRA_ID);
        assert_eq!(Relic::OddlySmoothStone.content_id(), ODDLY_SMOOTH_STONE_ID);
        assert_eq!(Relic::Strawberry.content_id(), STRAWBERRY_ID);
        assert_eq!(Relic::CoffeeDripper.content_id(), COFFEE_DRIPPER_ID);
        assert_eq!(Relic::Anchor.content_id(), ANCHOR_ID);
        assert_eq!(Relic::InkBottle.content_id(), INK_BOTTLE_ID);
        assert_eq!(Relic::OrnamentalFan.content_id(), ORNAMENTAL_FAN_ID);
        assert_eq!(Relic::IceCream.content_id(), ICE_CREAM_ID);
        assert_eq!(Relic::from_content_id(VAJRA_ID), Some(Relic::Vajra));
        assert_eq!(
            Relic::from_content_id(ODDLY_SMOOTH_STONE_ID),
            Some(Relic::OddlySmoothStone)
        );
        assert_eq!(
            Relic::from_content_id(STRAWBERRY_ID),
            Some(Relic::Strawberry)
        );
        assert_eq!(
            Relic::from_content_id(COFFEE_DRIPPER_ID),
            Some(Relic::CoffeeDripper)
        );
        assert_eq!(Relic::from_content_id(ANCHOR_ID), Some(Relic::Anchor));
        assert_eq!(
            Relic::from_content_id(INK_BOTTLE_ID),
            Some(Relic::InkBottle)
        );
        assert_eq!(
            Relic::from_content_id(ORNAMENTAL_FAN_ID),
            Some(Relic::OrnamentalFan)
        );
        assert_eq!(Relic::from_content_id(ICE_CREAM_ID), Some(Relic::IceCream));
        assert_eq!(Relic::from_content_id(ContentId::new(999)), None);
    }

    #[test]
    fn ice_cream_preserves_energy_between_turns_flag() {
        assert!(!preserves_energy_between_turns(&[]));
        assert!(preserves_energy_between_turns(&[Relic::IceCream]));
    }

    #[test]
    fn ink_bottle_increments_counter_on_card_play() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::InkBottle];

        let follow_ups = apply_on_card_play_relics(&mut combat, CardType::Skill);

        assert!(follow_ups.is_empty());
        assert_eq!(combat.relic_counters.ink_bottle_cards_played, 1);
    }

    #[test]
    fn ink_bottle_draws_after_ten_card_plays() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::InkBottle];
        combat.relic_counters.ink_bottle_cards_played = INK_BOTTLE_THRESHOLD - 1;

        let follow_ups = apply_on_card_play_relics(&mut combat, CardType::Attack);

        assert_eq!(follow_ups, vec![InternalAction::DrawCards { count: 1 }]);
        assert_eq!(combat.relic_counters.ink_bottle_cards_played, 0);
    }

    #[test]
    fn ornamental_fan_increments_attack_counter() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::OrnamentalFan];

        let follow_ups = apply_on_card_play_relics(&mut combat, CardType::Attack);

        assert!(follow_ups.is_empty());
        assert_eq!(combat.relic_counters.ornamental_fan_attacks_this_turn, 1);
    }

    #[test]
    fn ornamental_fan_ignores_non_attack_cards() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::OrnamentalFan];

        let follow_ups = apply_on_card_play_relics(&mut combat, CardType::Skill);

        assert!(follow_ups.is_empty());
        assert_eq!(combat.relic_counters.ornamental_fan_attacks_this_turn, 0);
    }

    #[test]
    fn ornamental_fan_grants_block_on_third_attack() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::OrnamentalFan];
        combat.relic_counters.ornamental_fan_attacks_this_turn = ORNAMENTAL_FAN_THRESHOLD - 1;

        let follow_ups = apply_on_card_play_relics(&mut combat, CardType::Attack);

        assert_eq!(
            follow_ups,
            vec![InternalAction::GainBlock {
                amount: ORNAMENTAL_FAN_BLOCK
            }]
        );
        assert_eq!(combat.relic_counters.ornamental_fan_attacks_this_turn, 0);
    }

    #[test]
    fn reset_turn_relic_counters_clears_ornamental_fan_attacks() {
        let mut combat = CombatState::initial_fixture();
        combat.relic_counters.ornamental_fan_attacks_this_turn = 2;

        reset_turn_relic_counters(&mut combat);

        assert_eq!(combat.relic_counters.ornamental_fan_attacks_this_turn, 0);
    }

    #[test]
    fn relic_counters_round_trip_through_json() {
        let counters = RelicCounters {
            ink_bottle_cards_played: 7,
            ornamental_fan_attacks_this_turn: 2,
        };

        let json = serde_json::to_string(&counters).expect("counters serialize");
        let restored: RelicCounters = serde_json::from_str(&json).expect("counters deserialize");

        assert_eq!(restored, counters);
    }

    #[test]
    fn anchor_grants_ten_block_at_combat_start() {
        let mut combat = CombatState::initial_fixture();

        apply_start_of_combat_relics(&mut combat, &[Relic::Anchor]);

        assert_eq!(combat.player.block, ANCHOR_BLOCK);
    }
}
