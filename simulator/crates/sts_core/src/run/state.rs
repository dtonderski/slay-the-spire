use crate::{
    card::{CardInstance, CardRarity, CardType},
    combat::state::BASE_PLAYER_ENERGY,
    combat::CombatState,
    content::ascension::AscensionConfig,
    content::cards::{
        ANGER_ID, BASH_ID, BATTLE_TRANCE_ID, BURNING_PACT_ID, CLEAVE_ID, DARK_EMBRACE_ID,
        DEFEND_R_ID, DRAMATIC_ENTRANCE_ID, DUAL_WIELD_ID, FEEL_NO_PAIN_ID, FLEX_ID, HAVOC_ID,
        INFLAME_ID, POMMEL_STRIKE_ID, SEARING_BLOW_ID, SEEING_RED_ID, SHRUG_IT_OFF_ID,
        SPOT_WEAKNESS_ID, STRIKE_R_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, WARCRY_ID, WHIRLWIND_ID,
    },
    content::character::IRONCLAD_A0_BASE_HP,
    ids::{CardId, ContentId, MonsterId},
    map::{milestone8_fixture, MapRunState},
    potion::{Potion, MAX_POTIONS},
    relic::{
        apply_start_of_combat_relics, initialize_ironclad_relic_pools, Relic, RelicKey,
        RelicPoolState, RelicSpawnContext, COFFEE_DRIPPER_ENERGY, STRAWBERRY_MAX_HP,
    },
    SimError, SimResult, StsRng,
};
use serde::{Deserialize, Serialize};

pub const STARTING_GOLD: i32 = 99;

fn default_energy_per_turn() -> i32 {
    BASE_PLAYER_ENERGY
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{ANGER_ID, FEEL_NO_PAIN_ID};

    #[test]
    fn ensure_ironclad_relic_pools_initializes_once_and_advances_counter() {
        let mut run = RunState::map_fixture();
        run.relic_rng_seed = 22_079_335_079;

        run.ensure_ironclad_relic_pools();
        let first = run.relic_pools.clone().expect("relic pools");

        assert_eq!(run.relic_rng_counter, 5);
        assert_eq!(first.common.first(), Some(&RelicKey::ToyOrnithopter));

        run.ensure_ironclad_relic_pools();

        assert_eq!(run.relic_rng_counter, 5);
        assert_eq!(run.relic_pools, Some(first));
    }

    #[test]
    fn relic_spawn_context_uses_deck_and_owned_relics() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::CoffeeDripper];
        run.deck.push(CardInstance::new(CardId::new(500), ANGER_ID));
        run.deck
            .push(CardInstance::new(CardId::new(501), FEEL_NO_PAIN_ID));

        let context = run.relic_spawn_context(12, true);

        assert!(context.shop_room);
        assert_eq!(context.floor_num, 12);
        assert!(context.owned_relics.contains(&RelicKey::CoffeeDripper));
        assert!(context.has_non_basic_attack);
        assert!(context.has_power);
        assert!(!context.has_non_basic_skill);
    }

    #[test]
    fn relic_keys_map_for_implemented_relics() {
        assert_eq!(Relic::from_key(Relic::Vajra.key()), Some(Relic::Vajra));
        assert_eq!(Relic::from_key(RelicKey::ToyOrnithopter), None);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunState {
    pub phase: RunPhase,
    pub deck: Vec<CardInstance>,
    pub player_hp: i32,
    pub player_max_hp: i32,
    pub gold: i32,
    #[serde(default = "default_energy_per_turn")]
    pub energy_per_turn: i32,
    pub map: Option<MapRunState>,
    pub combat: Option<CombatState>,
    pub reward: Option<RewardScreen>,
    #[serde(default)]
    pub event: Option<super::event::EventScreen>,
    pub shop: Option<super::shop::ShopScreen>,
    #[serde(default)]
    pub card_grid: Option<super::grid::CardGridScreen>,
    #[serde(default)]
    pub relics: Vec<Relic>,
    #[serde(default)]
    pub potions: Vec<Potion>,
    #[serde(default)]
    pub event_rng_seed: u64,
    #[serde(default)]
    pub reward_rng_seed: u64,
    #[serde(default)]
    pub card_rng_counter: u32,
    #[serde(default = "default_card_rarity_factor")]
    pub card_rarity_factor: i32,
    #[serde(default)]
    pub treasure_rng_seed: u64,
    #[serde(default)]
    pub treasure_rng_counter: u32,
    #[serde(default)]
    pub potion_rng_seed: u64,
    #[serde(default)]
    pub potion_rng_counter: u32,
    #[serde(default)]
    pub potion_chance: i32,
    #[serde(default)]
    pub relic_rng_seed: u64,
    #[serde(default)]
    pub relic_rng_counter: u32,
    #[serde(default)]
    pub relic_pools: Option<RelicPoolState>,
    #[serde(default)]
    pub relic_keys: Vec<RelicKey>,
    #[serde(default)]
    pub merchant_rng_seed: u64,
    #[serde(default)]
    pub merchant_rng_counter: u32,
    #[serde(default)]
    pub event_rng_counter: u32,
    #[serde(default)]
    pub misc_rng_seed: u64,
    #[serde(default)]
    pub misc_rng_counter: u32,
    #[serde(default)]
    pub current_floor: i32,
    #[serde(default)]
    pub current_act: i32,
    #[serde(default)]
    pub shop_remove_count: u32,
    #[serde(default)]
    pub act1_event_list: Vec<super::event::Event>,
    #[serde(default)]
    pub act1_shrine_list: Vec<super::event::Event>,
    #[serde(default)]
    pub ascension: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunPhase {
    Combat,
    Reward,
    Rest,
    Event,
    Shop,
    Idle,
}

pub const REWARD_GOLD_AMOUNT: i32 = 20;

fn default_card_rarity_factor() -> i32 {
    5
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardScreen {
    pub choices: Vec<CardInstance>,
    pub gold_offer: i32,
    pub potion_offer: Option<Potion>,
    pub relic_offer: Option<Relic>,
    #[serde(default)]
    pub relic_key_offer: Option<RelicKey>,
    #[serde(default)]
    pub card_reward_active: bool,
    /// Normal combat rewards defer card RNG until the player opens the card screen.
    #[serde(default)]
    pub card_reward_pending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunAction {
    SkipReward,
    TakeCardReward {
        card_id: CardId,
    },
    TakeGoldReward,
    TakePotionReward,
    TakeRelicReward,
    OpenCardReward,
    SkipPotionReward,
    BuyShopCard {
        slot: usize,
    },
    BuyShopRelic {
        slot: usize,
    },
    BuyShopPotion {
        slot: usize,
    },
    UsePotion {
        slot: usize,
        target: Option<MonsterId>,
    },
    DiscardPotion {
        slot: usize,
    },
    EnterShop,
    LeaveShop,
    OpenShopRemove,
}

impl RunState {
    #[must_use]
    pub fn init_combat(&self, base: CombatState) -> CombatState {
        let mut combat = base;
        combat.player.hp = self.player_hp;
        combat.player.max_hp = self.player_max_hp;
        combat.player.max_energy = self.energy_per_turn;
        combat.player.energy = self.energy_per_turn;
        combat.relics = self.relics.clone();
        combat.ascension = self.ascension;
        apply_start_of_combat_relics(&mut combat, &self.relics);
        combat
    }

    #[must_use]
    pub fn ascension_config(&self) -> AscensionConfig {
        AscensionConfig::new(self.ascension)
    }

    #[must_use]
    pub fn combat_fixture() -> Self {
        Self::combat_fixture_with_relics(Vec::new())
    }

    #[must_use]
    pub fn combat_fixture_with_relics(relics: Vec<Relic>) -> Self {
        Self::combat_fixture_with_options(relics, 0)
    }

    #[must_use]
    pub fn combat_fixture_with_ascension(ascension: u8) -> Self {
        Self::combat_fixture_with_options(Vec::new(), ascension)
    }

    #[must_use]
    pub fn combat_fixture_with_options(relics: Vec<Relic>, ascension: u8) -> Self {
        let deck = crate::content::deck::ironclad_starter_deck_for_ascension(ascension);
        let mut run = Self {
            phase: RunPhase::Combat,
            deck,
            player_hp: IRONCLAD_A0_BASE_HP,
            player_max_hp: IRONCLAD_A0_BASE_HP,
            gold: STARTING_GOLD,
            energy_per_turn: BASE_PLAYER_ENERGY,
            map: None,
            combat: None,
            reward: None,
            event: None,
            shop: None,
            card_grid: None,
            relics,
            potions: Vec::new(),
            event_rng_seed: 0,
            reward_rng_seed: 0,
            card_rng_counter: 0,
            card_rarity_factor: default_card_rarity_factor(),
            treasure_rng_seed: 0,
            treasure_rng_counter: 0,
            potion_rng_seed: 0,
            potion_rng_counter: 0,
            potion_chance: 0,
            relic_rng_seed: 0,
            relic_rng_counter: 0,
            relic_pools: None,
            relic_keys: Vec::new(),
            merchant_rng_seed: 0,
            merchant_rng_counter: 0,
            event_rng_counter: 0,
            misc_rng_seed: 0,
            misc_rng_counter: 0,
            current_floor: 0,
            current_act: 1,
            shop_remove_count: 0,
            act1_event_list: Vec::new(),
            act1_shrine_list: Vec::new(),
            ascension,
        };
        let combat = run.init_combat(CombatState::initial_fixture());
        run.player_hp = combat.player.hp;
        run.player_max_hp = combat.player.max_hp;
        run.combat = Some(combat);
        run
    }

    #[must_use]
    pub fn map_fixture() -> Self {
        Self {
            phase: RunPhase::Idle,
            deck: crate::content::deck::ironclad_starter_deck(),
            player_hp: IRONCLAD_A0_BASE_HP,
            player_max_hp: IRONCLAD_A0_BASE_HP,
            gold: STARTING_GOLD,
            energy_per_turn: BASE_PLAYER_ENERGY,
            map: Some(milestone8_fixture()),
            combat: None,
            reward: None,
            event: None,
            shop: None,
            card_grid: None,
            relics: Vec::new(),
            potions: Vec::new(),
            event_rng_seed: 0,
            reward_rng_seed: 0,
            card_rng_counter: 0,
            card_rarity_factor: default_card_rarity_factor(),
            treasure_rng_seed: 0,
            treasure_rng_counter: 0,
            potion_rng_seed: 0,
            potion_rng_counter: 0,
            potion_chance: 0,
            relic_rng_seed: 0,
            relic_rng_counter: 0,
            relic_pools: None,
            relic_keys: Vec::new(),
            merchant_rng_seed: 0,
            merchant_rng_counter: 0,
            event_rng_counter: 0,
            misc_rng_seed: 0,
            misc_rng_counter: 0,
            current_floor: 0,
            current_act: 1,
            shop_remove_count: 0,
            act1_event_list: Vec::new(),
            act1_shrine_list: Vec::new(),
            ascension: 0,
        }
    }

    pub fn ensure_ironclad_relic_pools(&mut self) {
        if self.relic_pools.is_none() {
            let mut rng = StsRng::with_counter(self.relic_rng_seed as i64, self.relic_rng_counter);
            self.relic_pools = Some(initialize_ironclad_relic_pools(&mut rng));
            self.relic_rng_counter = rng.counter();
        }
    }

    #[must_use]
    pub fn relic_spawn_context(&self, floor_num: i32, shop_room: bool) -> RelicSpawnContext {
        let mut owned_relics: Vec<_> = self.relics.iter().map(|relic| relic.key()).collect();
        owned_relics.extend(self.relic_keys.iter().copied());
        RelicSpawnContext {
            floor_num,
            shop_room,
            owned_relics,
            has_non_basic_attack: self.deck.iter().any(|card| {
                card_type_and_rarity(card.content_id).is_some_and(|(card_type, _)| {
                    card_type == CardType::Attack && !is_basic_starter_card(card.content_id)
                })
            }),
            has_non_basic_skill: self.deck.iter().any(|card| {
                card_type_and_rarity(card.content_id).is_some_and(|(card_type, _)| {
                    card_type == CardType::Skill && !is_basic_starter_card(card.content_id)
                })
            }),
            has_power: self.deck.iter().any(|card| {
                card_type_and_rarity(card.content_id)
                    .is_some_and(|(card_type, _)| card_type == CardType::Power)
            }),
        }
    }

    pub fn next_card_instance_id(&self) -> u64 {
        self.deck
            .iter()
            .map(|card| card.id.get())
            .max()
            .unwrap_or(0)
            + 1
    }

    pub fn gain_deck_card(&mut self, content_id: ContentId) {
        let id = CardId::new(self.next_card_instance_id());
        self.deck.push(CardInstance::new(id, content_id));
    }

    pub fn gain_relic_key(&mut self, key: RelicKey) {
        if let Some(relic) = Relic::from_key(key) {
            self.gain_relic(relic);
        } else {
            self.relic_keys.push(key);
        }
    }

    pub fn gain_relic(&mut self, relic: Relic) {
        self.relics.push(relic);
        match relic {
            Relic::Strawberry => {
                self.player_max_hp += STRAWBERRY_MAX_HP;
                self.player_hp += STRAWBERRY_MAX_HP;
            }
            Relic::CoffeeDripper => {
                self.energy_per_turn += COFFEE_DRIPPER_ENERGY;
            }
            Relic::Vajra
            | Relic::OddlySmoothStone
            | Relic::Anchor
            | Relic::InkBottle
            | Relic::OrnamentalFan
            | Relic::IceCream => {}
        }
    }

    pub fn validate_reward_action(&self, action: RunAction) -> SimResult<()> {
        if self.phase != RunPhase::Reward {
            return Err(SimError::IllegalAction(
                "reward actions require reward phase",
            ));
        }

        let reward = self
            .reward
            .as_ref()
            .ok_or(SimError::InvalidState("reward screen is missing"))?;

        match action {
            RunAction::SkipReward => Ok(()),
            RunAction::TakeGoldReward => {
                if reward.gold_offer > 0 {
                    Ok(())
                } else {
                    Err(SimError::IllegalAction("no gold reward offered"))
                }
            }
            RunAction::TakePotionReward => {
                if reward.potion_offer.is_none() {
                    return Err(SimError::IllegalAction("no potion reward offered"));
                }
                if self.potions.len() >= MAX_POTIONS {
                    return Err(SimError::IllegalAction("potion belt is full"));
                }
                Ok(())
            }
            RunAction::TakeRelicReward => {
                if reward.relic_offer.is_none() && reward.relic_key_offer.is_none() {
                    return Err(SimError::IllegalAction("no relic reward offered"));
                }
                if let Some(relic) = reward.relic_offer {
                    if self.relics.contains(&relic) {
                        return Err(SimError::IllegalAction("relic already owned"));
                    }
                }
                if let Some(key) = reward.relic_key_offer {
                    if self.relics.iter().any(|relic| relic.key() == key)
                        || self.relic_keys.contains(&key)
                    {
                        return Err(SimError::IllegalAction("relic already owned"));
                    }
                }
                Ok(())
            }
            RunAction::OpenCardReward => {
                if !reward.card_reward_pending {
                    return Err(SimError::IllegalAction("no card reward offered"));
                }
                if reward.card_reward_active {
                    return Err(SimError::IllegalAction("card reward already open"));
                }
                Ok(())
            }
            RunAction::SkipPotionReward => {
                if reward.potion_offer.is_none() {
                    return Err(SimError::IllegalAction("no potion reward offered"));
                }
                Ok(())
            }
            RunAction::TakeCardReward { card_id } => {
                if reward.choices.iter().any(|choice| choice.id == card_id) {
                    Ok(())
                } else {
                    Err(SimError::UnknownCard(card_id))
                }
            }
            RunAction::BuyShopCard { .. }
            | RunAction::BuyShopRelic { .. }
            | RunAction::BuyShopPotion { .. }
            | RunAction::EnterShop
            | RunAction::LeaveShop
            | RunAction::OpenShopRemove => Err(SimError::IllegalAction("not a reward action")),
            RunAction::UsePotion { .. } | RunAction::DiscardPotion { .. } => {
                Err(SimError::IllegalAction("not a reward action"))
            }
        }
    }

    pub fn count_content_in_deck(&self, content_id: ContentId) -> usize {
        self.deck
            .iter()
            .filter(|card| card.content_id == content_id)
            .count()
    }
}

impl Relic {
    #[must_use]
    pub fn key(self) -> RelicKey {
        match self {
            Relic::Vajra => RelicKey::Vajra,
            Relic::OddlySmoothStone => RelicKey::OddlySmoothStone,
            Relic::Strawberry => RelicKey::Strawberry,
            Relic::CoffeeDripper => RelicKey::CoffeeDripper,
            Relic::Anchor => RelicKey::Anchor,
            Relic::InkBottle => RelicKey::InkBottle,
            Relic::OrnamentalFan => RelicKey::OrnamentalFan,
            Relic::IceCream => RelicKey::IceCream,
        }
    }

    #[must_use]
    pub fn from_key(key: RelicKey) -> Option<Self> {
        match key {
            RelicKey::Vajra => Some(Relic::Vajra),
            RelicKey::OddlySmoothStone => Some(Relic::OddlySmoothStone),
            RelicKey::Strawberry => Some(Relic::Strawberry),
            RelicKey::CoffeeDripper => Some(Relic::CoffeeDripper),
            RelicKey::Anchor => Some(Relic::Anchor),
            RelicKey::InkBottle => Some(Relic::InkBottle),
            RelicKey::OrnamentalFan => Some(Relic::OrnamentalFan),
            RelicKey::IceCream => Some(Relic::IceCream),
            _ => None,
        }
    }
}

fn card_type_and_rarity(content_id: ContentId) -> Option<(CardType, CardRarity)> {
    match content_id {
        id if id == STRIKE_R_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == DEFEND_R_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == BASH_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == ANGER_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == CLEAVE_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == TWIN_STRIKE_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == SHRUG_IT_OFF_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == TRUE_GRIT_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == POMMEL_STRIKE_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == BATTLE_TRANCE_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == SEEING_RED_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == BURNING_PACT_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == FEEL_NO_PAIN_ID => Some((CardType::Power, CardRarity::Uncommon)),
        id if id == DARK_EMBRACE_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == INFLAME_ID => Some((CardType::Power, CardRarity::Uncommon)),
        id if id == FLEX_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == SPOT_WEAKNESS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == WHIRLWIND_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == HAVOC_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == WARCRY_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == DUAL_WIELD_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == SEARING_BLOW_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == DRAMATIC_ENTRANCE_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        _ => None,
    }
}

fn is_basic_starter_card(content_id: ContentId) -> bool {
    matches!(content_id, id if id == STRIKE_R_ID || id == DEFEND_R_ID || id == BASH_ID)
}
