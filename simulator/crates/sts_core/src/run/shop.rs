use crate::{
    card::{CardInstance, CardRarity, CardType},
    content::cards::ANGER_ID,
    content::shop_pool::{
        assign_random_class_card_excluding, random_class_card_of_type_and_rarity,
        random_colorless_from_pool, roll_card_rarity_shop, shop_card_price_rarity,
    },
    ids::CardId,
    potion::Potion,
    relic::{Relic, RelicKey, RelicTier},
    rng::StsRng,
    run::grid::open_shop_remove_grid,
    run::reward::target_random_potion,
    RunAction, RunPhase, RunState, SimError, SimResult,
};

pub const SHOP_ANGER_PRICE: i32 = 50;
pub const SHOP_VAJRA_PRICE: i32 = 150;
pub const SHOP_FIRE_POTION_PRICE: i32 = 50;
pub const SHOP_BASE_REMOVE_PRICE: i32 = 75;
pub const SHOP_REMOVE_PRICE_INCREASE: i32 = 25;

const SHOP_CARD_COMMON_PRICE: i32 = 50;
const SHOP_CARD_UNCOMMON_PRICE: i32 = 78;
const SHOP_CARD_RARE_PRICE: i32 = 102;
const SHOP_RELIC_COMMON_PRICE: i32 = 150;
const SHOP_RELIC_UNCOMMON_PRICE: i32 = 250;
const SHOP_RELIC_RARE_PRICE: i32 = 300;
const SHOP_RELIC_SHOP_PRICE: i32 = 150;
const SHOP_POTION_COMMON_PRICE: i32 = 50;
const SHOP_POTION_UNCOMMON_PRICE: i32 = 75;
const SHOP_POTION_RARE_PRICE: i32 = 100;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShopCardSlot {
    pub card: CardInstance,
    pub price: i32,
    pub sold: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShopRelicSlot {
    pub relic_key: RelicKey,
    pub price: i32,
    pub sold: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShopPotionSlot {
    pub potion: Potion,
    pub price: i32,
    pub sold: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShopScreen {
    pub cards: Vec<ShopCardSlot>,
    pub relics: Vec<ShopRelicSlot>,
    pub potions: Vec<ShopPotionSlot>,
    pub remove_cost: i32,
    #[serde(default)]
    pub sale_slot: Option<usize>,
}

pub fn shop_card_rarity_roll(rng: &mut StsRng, card_rarity_factor: i32) -> CardRarity {
    roll_card_rarity_shop(rng, card_rarity_factor)
}

pub fn shop_relic_tier_roll(rng: &mut StsRng) -> RelicTier {
    let roll = rng.random_int(99);
    if roll < 48 {
        RelicTier::Common
    } else if roll < 82 {
        RelicTier::Uncommon
    } else {
        RelicTier::Rare
    }
}

fn card_price_for_rarity(rarity: CardRarity, merchant_rng: &mut StsRng) -> i32 {
    let base = match rarity {
        CardRarity::Common => SHOP_CARD_COMMON_PRICE,
        CardRarity::Uncommon => SHOP_CARD_UNCOMMON_PRICE,
        CardRarity::Rare => SHOP_CARD_RARE_PRICE,
    };
    let factor = merchant_rng.random_float_range(0.9, 1.1);
    (base as f32 * factor) as i32
}

fn colorless_card_price_for_rarity(rarity: CardRarity, merchant_rng: &mut StsRng) -> i32 {
    // Target `AbstractCard.getPrice` bases, not shop class-card bases.
    let base = match rarity {
        CardRarity::Common => SHOP_CARD_COMMON_PRICE,
        CardRarity::Uncommon => SHOP_POTION_UNCOMMON_PRICE,
        CardRarity::Rare => 150,
    };
    let factor = merchant_rng.random_float_range(0.9, 1.1);
    (base as f32 * factor).round() as i32
}

fn relic_base_price(tier: RelicTier) -> i32 {
    match tier {
        RelicTier::Common => SHOP_RELIC_COMMON_PRICE,
        RelicTier::Uncommon => SHOP_RELIC_UNCOMMON_PRICE,
        RelicTier::Rare => SHOP_RELIC_RARE_PRICE,
        RelicTier::Shop => SHOP_RELIC_SHOP_PRICE,
        RelicTier::Boss => SHOP_RELIC_RARE_PRICE,
    }
}

fn relic_price(tier: RelicTier, merchant_rng: &mut StsRng) -> i32 {
    let factor = merchant_rng.random_float_range(0.95, 1.05);
    (relic_base_price(tier) as f32 * factor).round() as i32
}

fn potion_base_price(potion: Potion) -> i32 {
    match potion.rarity() {
        crate::potion::PotionRarity::Common => SHOP_POTION_COMMON_PRICE,
        crate::potion::PotionRarity::Uncommon => SHOP_POTION_UNCOMMON_PRICE,
        crate::potion::PotionRarity::Rare => SHOP_POTION_RARE_PRICE,
    }
}

fn potion_price(potion: Potion, merchant_rng: &mut StsRng) -> i32 {
    let factor = merchant_rng.random_float_range(0.95, 1.05);
    (potion_base_price(potion) as f32 * factor).round() as i32
}

#[must_use]
pub fn shop_remove_cost_for_run(run: &RunState) -> i32 {
    if owns_relic_key(run, RelicKey::SmilingMask) {
        return if has_membership_card(run) { 25 } else { 50 };
    }

    let base = SHOP_BASE_REMOVE_PRICE + SHOP_REMOVE_PRICE_INCREASE * run.shop_remove_count as i32;
    if has_membership_card(run) {
        base / 2
    } else {
        base
    }
}

fn has_membership_card(run: &RunState) -> bool {
    owns_relic_key(run, RelicKey::MembershipCard)
}

fn apply_membership_discount_to_shop(shop: &mut ShopScreen) {
    for offer in &mut shop.cards {
        if !offer.sold {
            offer.price = (offer.price + 1) / 2;
        }
    }
    for offer in &mut shop.relics {
        if !offer.sold {
            offer.price = (offer.price + 1) / 2;
        }
    }
    for offer in &mut shop.potions {
        if !offer.sold {
            offer.price = (offer.price + 1) / 2;
        }
    }
    shop.remove_cost = (shop.remove_cost + 1) / 2;
}

fn owns_relic_key(run: &RunState, key: RelicKey) -> bool {
    run.relic_keys.contains(&key) || run.relics.iter().any(|relic| relic.key() == key)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopPick {
    Purge,
    BuyCard(usize),
    BuyRelic(usize),
    BuyPotion(usize),
}

#[must_use]
pub fn affordable_shop_picks(run: &RunState) -> Vec<ShopPick> {
    let Some(shop) = run.shop.as_ref() else {
        return Vec::new();
    };
    if run.card_grid.is_some() {
        return Vec::new();
    }

    let mut picks = Vec::new();
    if run.shop_remove_count == 0 && run.gold >= shop.remove_cost {
        picks.push(ShopPick::Purge);
    }
    for (slot, offer) in shop.cards.iter().enumerate() {
        if !offer.sold && run.gold >= offer.price {
            picks.push(ShopPick::BuyCard(slot));
        }
    }
    for (slot, offer) in shop.relics.iter().enumerate() {
        if !offer.sold && run.gold >= offer.price && !owns_relic_key(run, offer.relic_key) {
            picks.push(ShopPick::BuyRelic(slot));
        }
    }
    for (slot, offer) in shop.potions.iter().enumerate() {
        if !offer.sold && run.gold >= offer.price && run.potions.len() < run.potion_capacity() {
            picks.push(ShopPick::BuyPotion(slot));
        }
    }
    picks
}

fn roll_shop_relic(run: &mut RunState, tier: RelicTier) -> RelicKey {
    run.ensure_ironclad_relic_pools();
    let context = run.relic_spawn_context(run.current_floor, true);
    run.relic_pools
        .as_mut()
        .expect("relic pools")
        .return_random_relic_end(tier, &context)
}

#[must_use]
pub fn generate_shop_screen(run: &mut RunState) -> ShopScreen {
    let mut next_card_id = run.next_card_instance_id();
    let mut card_rng = StsRng::with_counter(run.reward_rng_seed as i64, run.card_rng_counter);
    let mut potion_rng = StsRng::with_counter(run.potion_rng_seed as i64, run.potion_rng_counter);
    let mut merchant_rng =
        StsRng::with_counter(run.merchant_rng_seed as i64, run.merchant_rng_counter);

    let mut rarities = [CardRarity::Common; 5];
    let mut card_contents = [crate::ContentId::new(0); 7];
    let mut prices = [0i32; 7];

    rarities[0] = roll_card_rarity_shop(&mut card_rng, run.card_rarity_factor);
    card_contents[0] =
        random_class_card_of_type_and_rarity(&mut card_rng, CardType::Attack, rarities[0]);
    let (second_attack, second_attack_rarity) = assign_random_class_card_excluding(
        &mut card_rng,
        CardType::Attack,
        card_contents[0],
        run.card_rarity_factor,
    );
    card_contents[1] = second_attack;
    rarities[1] = second_attack_rarity;

    rarities[2] = roll_card_rarity_shop(&mut card_rng, run.card_rarity_factor);
    card_contents[2] =
        random_class_card_of_type_and_rarity(&mut card_rng, CardType::Skill, rarities[2]);
    let (second_skill, second_skill_rarity) = assign_random_class_card_excluding(
        &mut card_rng,
        CardType::Skill,
        card_contents[2],
        run.card_rarity_factor,
    );
    card_contents[3] = second_skill;
    rarities[3] = second_skill_rarity;

    rarities[4] = roll_card_rarity_shop(&mut card_rng, run.card_rarity_factor);
    if rarities[4] == CardRarity::Common {
        rarities[4] = CardRarity::Uncommon;
    }
    card_contents[4] =
        random_class_card_of_type_and_rarity(&mut card_rng, CardType::Power, rarities[4]);

    card_contents[5] = random_colorless_from_pool(&mut card_rng, CardRarity::Uncommon);
    card_contents[6] = random_colorless_from_pool(&mut card_rng, CardRarity::Rare);
    run.card_rng_counter = card_rng.counter();

    for i in 0..5 {
        prices[i] =
            card_price_for_rarity(shop_card_price_rarity(card_contents[i]), &mut merchant_rng);
    }
    prices[5] = (colorless_card_price_for_rarity(CardRarity::Uncommon, &mut merchant_rng) as f32
        * 1.2)
        .round() as i32;
    prices[6] = (colorless_card_price_for_rarity(CardRarity::Rare, &mut merchant_rng) as f32 * 1.2)
        .round() as i32;

    let sale_slot = merchant_rng.random_int(4) as usize;
    prices[sale_slot] /= 2;

    let mut relics = Vec::with_capacity(3);
    for _ in 0..2 {
        let tier = shop_relic_tier_roll(&mut merchant_rng);
        let key = roll_shop_relic(run, tier);
        relics.push(ShopRelicSlot {
            relic_key: key,
            price: relic_price(tier, &mut merchant_rng),
            sold: false,
        });
    }
    let key = roll_shop_relic(run, RelicTier::Shop);
    relics.push(ShopRelicSlot {
        relic_key: key,
        price: relic_price(RelicTier::Shop, &mut merchant_rng),
        sold: false,
    });

    let mut potions = Vec::with_capacity(3);
    for _ in 0..3 {
        let potion = target_random_potion(&mut potion_rng);
        potions.push(ShopPotionSlot {
            potion,
            price: potion_price(potion, &mut merchant_rng),
            sold: false,
        });
    }
    run.potion_rng_counter = potion_rng.counter();
    run.merchant_rng_counter = merchant_rng.counter();

    let cards = card_contents
        .into_iter()
        .zip(prices)
        .map(|(content_id, price)| {
            let card = CardInstance::new(CardId::new(next_card_id), content_id);
            next_card_id += 1;
            ShopCardSlot {
                card,
                price,
                sold: false,
            }
        })
        .collect();

    let mut shop = ShopScreen {
        cards,
        relics,
        potions,
        remove_cost: shop_remove_cost_for_run(run),
        sale_slot: Some(sale_slot),
    };
    if has_membership_card(run) {
        apply_membership_discount_to_shop(&mut shop);
    }
    shop
}

#[must_use]
pub fn fixed_shop_screen(next_card_id: u64) -> ShopScreen {
    ShopScreen {
        cards: vec![ShopCardSlot {
            card: CardInstance::new(CardId::new(next_card_id), ANGER_ID),
            price: SHOP_ANGER_PRICE,
            sold: false,
        }],
        relics: vec![ShopRelicSlot {
            relic_key: RelicKey::Vajra,
            price: SHOP_VAJRA_PRICE,
            sold: false,
        }],
        potions: vec![ShopPotionSlot {
            potion: Potion::Fire,
            price: SHOP_FIRE_POTION_PRICE,
            sold: false,
        }],
        remove_cost: SHOP_BASE_REMOVE_PRICE,
        sale_slot: None,
    }
}

pub fn enter_shop_room(run: &mut RunState) {
    run.phase = RunPhase::Shop;
    run.shop = None;
    run.card_grid = None;
}

pub fn open_shop_merchant(run: &mut RunState) {
    run.phase = RunPhase::Shop;
    run.shop = Some(if run.merchant_rng_seed == 0 {
        fixed_shop_screen(run.next_card_instance_id())
    } else {
        generate_shop_screen(run)
    });
    if run.relics.contains(&Relic::MealTicket) {
        run.player_hp = (run.player_hp + crate::relic::MEAL_TICKET_HEAL).min(run.player_max_hp);
    }
}

pub fn enter_shop_screen(run: &mut RunState) {
    open_shop_merchant(run);
}

pub fn leave_shop_merchant(run: &mut RunState) {
    run.shop = None;
    run.card_grid = None;
}

pub fn leave_shop_room(run: &mut RunState) {
    run.shop = None;
    run.card_grid = None;
    run.phase = RunPhase::Idle;
}

#[must_use]
pub fn shop_choice_labels(run: &RunState) -> Vec<String> {
    affordable_shop_picks(run)
        .into_iter()
        .map(|pick| match pick {
            ShopPick::Purge => "purge".to_owned(),
            ShopPick::BuyCard(slot) => {
                let shop = run.shop.as_ref().expect("shop pick without shop");
                format!("card{}", shop.cards[slot].card.content_id.get())
            }
            ShopPick::BuyRelic(slot) => {
                let shop = run.shop.as_ref().expect("shop pick without shop");
                format!("{:?}", shop.relics[slot].relic_key).to_ascii_lowercase()
            }
            ShopPick::BuyPotion(slot) => {
                let shop = run.shop.as_ref().expect("shop pick without shop");
                format!("{:?}", shop.potions[slot].potion).to_ascii_lowercase()
            }
        })
        .collect()
}

#[must_use]
pub fn legal_shop_actions(run: &RunState) -> Vec<RunAction> {
    if run.phase != RunPhase::Shop {
        return Vec::new();
    }

    if run.card_grid.is_some() {
        return Vec::new();
    }

    let Some(shop) = run.shop.as_ref() else {
        return vec![RunAction::EnterShop];
    };

    let mut actions = Vec::new();

    if run.gold >= shop.remove_cost {
        actions.push(RunAction::OpenShopRemove);
    }

    for (slot, offer) in shop.cards.iter().enumerate() {
        if !offer.sold && run.gold >= offer.price {
            actions.push(RunAction::BuyShopCard { slot });
        }
    }

    for (slot, offer) in shop.relics.iter().enumerate() {
        if !offer.sold && run.gold >= offer.price && !owns_relic_key(run, offer.relic_key) {
            actions.push(RunAction::BuyShopRelic { slot });
        }
    }

    for (slot, offer) in shop.potions.iter().enumerate() {
        if !offer.sold && run.gold >= offer.price && run.potions.len() < run.potion_capacity() {
            actions.push(RunAction::BuyShopPotion { slot });
        }
    }

    actions.push(RunAction::LeaveShop);
    actions
}

pub fn validate_shop_action(run: &RunState, action: RunAction) -> SimResult<()> {
    if run.phase != RunPhase::Shop {
        return Err(SimError::IllegalAction("shop actions require shop phase"));
    }

    match action {
        RunAction::EnterShop if run.shop.is_none() && run.card_grid.is_none() => Ok(()),
        RunAction::LeaveShop if run.shop.is_some() && run.card_grid.is_none() => Ok(()),
        RunAction::OpenShopRemove => {
            let shop = run
                .shop
                .as_ref()
                .ok_or(SimError::InvalidState("shop screen is missing"))?;
            if run.card_grid.is_some() {
                return Err(SimError::IllegalAction("grid already open"));
            }
            if run.gold < shop.remove_cost {
                return Err(SimError::IllegalAction("not enough gold"));
            }
            Ok(())
        }
        _ if run.card_grid.is_some() => Err(SimError::IllegalAction(
            "shop purchases unavailable while grid is open",
        )),
        _ => {
            let shop = run
                .shop
                .as_ref()
                .ok_or(SimError::InvalidState("shop screen is missing"))?;

            match action {
                RunAction::BuyShopCard { slot } => {
                    let offer = shop
                        .cards
                        .get(slot)
                        .ok_or(SimError::IllegalAction("shop slot is not available"))?;
                    if offer.sold {
                        return Err(SimError::IllegalAction("shop slot already sold"));
                    }
                    if run.gold < offer.price {
                        return Err(SimError::IllegalAction("not enough gold"));
                    }
                    Ok(())
                }
                RunAction::BuyShopRelic { slot } => {
                    let offer = shop
                        .relics
                        .get(slot)
                        .ok_or(SimError::IllegalAction("shop relic is not available"))?;
                    if offer.sold {
                        return Err(SimError::IllegalAction("shop relic already sold"));
                    }
                    if owns_relic_key(run, offer.relic_key) {
                        return Err(SimError::IllegalAction("relic already owned"));
                    }
                    if run.gold < offer.price {
                        return Err(SimError::IllegalAction("not enough gold"));
                    }
                    Ok(())
                }
                RunAction::BuyShopPotion { slot } => {
                    let offer = shop
                        .potions
                        .get(slot)
                        .ok_or(SimError::IllegalAction("shop potion is not available"))?;
                    if offer.sold {
                        return Err(SimError::IllegalAction("shop potion already sold"));
                    }
                    if run.potions.len() >= run.potion_capacity() {
                        return Err(SimError::IllegalAction("potion belt is full"));
                    }
                    if run.gold < offer.price {
                        return Err(SimError::IllegalAction("not enough gold"));
                    }
                    Ok(())
                }
                _ => Err(SimError::IllegalAction("not a shop action")),
            }
        }
    }
}

pub fn apply_shop_action(run: &RunState, action: RunAction) -> SimResult<RunState> {
    validate_shop_action(run, action)?;

    let mut next = run.clone();
    match action {
        RunAction::EnterShop => {
            open_shop_merchant(&mut next);
        }
        RunAction::LeaveShop => {
            leave_shop_merchant(&mut next);
        }
        RunAction::OpenShopRemove => {
            open_shop_remove_grid(&mut next);
        }
        RunAction::BuyShopCard { slot } => {
            let shop = next.shop.as_mut().expect("validated shop screen");
            let offer = shop.cards.get_mut(slot).expect("validated slot");
            let card = offer.card;
            let price = offer.price;
            offer.sold = true;
            next.gold -= price;
            next.add_deck_card(card);
        }
        RunAction::BuyShopRelic { slot } => {
            let shop = next.shop.as_mut().expect("validated shop screen");
            let offer = shop.relics.get_mut(slot).expect("validated relic offer");
            let key = offer.relic_key;
            let price = offer.price;
            offer.sold = true;
            next.gold -= price;
            next.gain_relic_key(key);
            if key == RelicKey::MembershipCard {
                if let Some(shop) = next.shop.as_mut() {
                    apply_membership_discount_to_shop(shop);
                }
            }
        }
        RunAction::BuyShopPotion { slot } => {
            let shop = next.shop.as_mut().expect("validated shop screen");
            let offer = shop.potions.get_mut(slot).expect("validated potion offer");
            let potion = offer.potion;
            let price = offer.price;
            offer.sold = true;
            next.gold -= price;
            next.potions.push(potion);
        }
        _ => unreachable!("validated shop action"),
    }

    Ok(next)
}

/// Map CommunicationMod `CHOOSE index` on `SHOP_SCREEN` to a shop action.
pub fn shop_action_for_choice_index(run: &RunState, choice_index: usize) -> SimResult<RunAction> {
    match affordable_shop_picks(run).get(choice_index) {
        Some(ShopPick::Purge) => Ok(RunAction::OpenShopRemove),
        Some(ShopPick::BuyCard(slot)) => Ok(RunAction::BuyShopCard { slot: *slot }),
        Some(ShopPick::BuyRelic(slot)) => Ok(RunAction::BuyShopRelic { slot: *slot }),
        Some(ShopPick::BuyPotion(slot)) => Ok(RunAction::BuyShopPotion { slot: *slot }),
        None => Err(SimError::IllegalAction("shop choice out of range")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        content::cards::ANGER_ID, map::RoomKind, MapAction, MapNodeId, Relic, VAJRA_STRENGTH,
    };

    fn shop_run() -> RunState {
        let mut run = RunState::map_fixture();
        for node_id in [MapNodeId::new(1), MapNodeId::new(3), MapNodeId::new(4)] {
            run = crate::apply_map_action_on_run(&run, MapAction::ChooseNode { node_id })
                .expect("reach shop");
        }
        open_shop_merchant(&mut run);
        run
    }

    #[test]
    fn entering_shop_room_exposes_fixed_anger_and_vajra_inventory() {
        let run = shop_run();

        assert_eq!(run.phase, RunPhase::Shop);
        assert_eq!(
            run.map
                .as_ref()
                .and_then(|map| map.map.node(map.current_node))
                .map(|node| node.room_kind),
            Some(RoomKind::Shop)
        );
        let shop = run.shop.expect("shop screen present");
        assert_eq!(shop.cards.len(), 1);
        assert_eq!(shop.cards[0].price, SHOP_ANGER_PRICE);
        assert_eq!(shop.cards[0].card.content_id, ANGER_ID);
        assert!(!shop.cards[0].sold);
        let relic = shop.relics[0];
        assert_eq!(relic.relic_key, RelicKey::Vajra);
        assert_eq!(relic.price, SHOP_VAJRA_PRICE);
        assert!(!relic.sold);
        let potion = shop.potions[0];
        assert_eq!(potion.potion, Potion::Fire);
        assert_eq!(potion.price, SHOP_FIRE_POTION_PRICE);
        assert!(!potion.sold);
    }

    #[test]
    fn merchant_shop_generation_is_deterministic_for_seed() {
        let mut first = RunState::map_fixture();
        let mut second = RunState::map_fixture();
        first.merchant_rng_seed = 22_079_335_079;
        second.merchant_rng_seed = 22_079_335_079;
        first.reward_rng_seed = 22_079_335_079;
        second.reward_rng_seed = 22_079_335_079;
        first.potion_rng_seed = 22_079_335_079;
        second.potion_rng_seed = 22_079_335_079;
        first.relic_rng_seed = 22_079_335_079;
        second.relic_rng_seed = 22_079_335_079;
        first.current_floor = 10;

        open_shop_merchant(&mut first);
        open_shop_merchant(&mut second);

        assert_eq!(first.shop, second.shop);
        assert_eq!(first.shop.as_ref().map(|shop| shop.cards.len()), Some(7));
        assert_eq!(first.shop.as_ref().map(|shop| shop.relics.len()), Some(3));
        assert_eq!(first.shop.as_ref().map(|shop| shop.potions.len()), Some(3));
        assert!(first.merchant_rng_counter > 0);
    }

    #[test]
    fn buy_shop_card_deducts_gold_and_adds_to_deck() {
        let run = shop_run();
        let gold_before = run.gold;
        let deck_len_before = run.deck.len();
        let anger_card_id = run.shop.as_ref().expect("shop").cards[0].card.id;

        let after = apply_shop_action(&run, RunAction::BuyShopCard { slot: 0 }).expect("buy anger");

        assert_eq!(after.phase, RunPhase::Shop);
        assert!(after.shop.is_some());
        assert_eq!(after.gold, gold_before - SHOP_ANGER_PRICE);
        assert_eq!(after.deck.len(), deck_len_before + 1);
        assert!(after.deck.iter().any(|card| card.id == anger_card_id));
        assert_eq!(after.count_content_in_deck(ANGER_ID), 1);
    }

    #[test]
    fn buy_shop_relic_deducts_gold_and_adds_vajra() {
        let mut run = shop_run();
        run.gold = SHOP_VAJRA_PRICE;

        let after =
            apply_shop_action(&run, RunAction::BuyShopRelic { slot: 0 }).expect("buy vajra");

        assert_eq!(after.phase, RunPhase::Shop);
        assert!(after.shop.is_some());
        assert_eq!(after.gold, 0);
        assert_eq!(after.relics, vec![Relic::Vajra]);
        let combat = after.init_combat(crate::CombatState::initial_fixture());
        assert_eq!(combat.player.powers.strength, VAJRA_STRENGTH);
    }

    #[test]
    fn membership_card_halves_remaining_shop_prices() {
        let mut run = shop_run();
        run.gold = 500;
        let card_price_before = run.shop.as_ref().expect("shop").cards[0].price;
        let mut shop = run.shop.take().expect("shop");
        shop.relics = vec![
            shop.relics[0],
            ShopRelicSlot {
                relic_key: RelicKey::MembershipCard,
                price: 100,
                sold: false,
            },
        ];
        run.shop = Some(shop);

        let after = apply_shop_action(&run, RunAction::BuyShopRelic { slot: 1 }).expect("buy card");
        let shop = after.shop.expect("shop");
        assert!(!shop.cards[0].sold);
        assert_eq!(shop.cards[0].price, (card_price_before + 1) / 2);
    }

    #[test]
    fn membership_card_modeled_relic_halves_shop_remove_cost() {
        let mut run = shop_run();
        run.relics.push(Relic::MembershipCard);
        let base = SHOP_BASE_REMOVE_PRICE + SHOP_REMOVE_PRICE_INCREASE;
        run.shop_remove_count = 1;

        assert_eq!(shop_remove_cost_for_run(&run), base / 2);
    }

    #[test]
    fn smiling_mask_caps_shop_remove_cost() {
        let mut run = shop_run();
        run.relics.push(Relic::SmilingMask);
        run.shop_remove_count = 3;

        assert_eq!(shop_remove_cost_for_run(&run), 50);
    }

    #[test]
    fn smiling_mask_and_membership_card_stack_on_remove_cost() {
        let mut run = shop_run();
        run.relics.push(Relic::SmilingMask);
        run.relics.push(Relic::MembershipCard);
        run.shop_remove_count = 3;

        assert_eq!(shop_remove_cost_for_run(&run), 25);
    }

    #[test]
    fn buying_shop_card_triggers_ceramic_fish_gold() {
        let mut run = shop_run();
        run.relics.push(Relic::CeramicFish);
        let gold_before = run.gold;

        let after = apply_shop_action(&run, RunAction::BuyShopCard { slot: 0 }).expect("buy anger");

        assert_eq!(
            after.gold,
            gold_before - SHOP_ANGER_PRICE + crate::relic::CERAMIC_FISH_GOLD
        );
    }

    #[test]
    fn buy_shop_relic_rejects_insufficient_gold() {
        let run = shop_run();

        let err = apply_shop_action(&run, RunAction::BuyShopRelic { slot: 0 })
            .expect_err("cannot afford");

        assert_eq!(err, SimError::IllegalAction("not enough gold"));
    }

    #[test]
    fn buy_shop_relic_rejects_duplicate_relic() {
        let mut run = shop_run();
        run.gold = SHOP_VAJRA_PRICE;
        run.relics.push(Relic::Vajra);

        let err = apply_shop_action(&run, RunAction::BuyShopRelic { slot: 0 })
            .expect_err("already owned");

        assert_eq!(err, SimError::IllegalAction("relic already owned"));
    }

    #[test]
    fn buy_shop_potion_deducts_gold_and_adds_fire_potion() {
        let run = shop_run();
        let gold_before = run.gold;
        let potions_before = run.potions.len();

        let after =
            apply_shop_action(&run, RunAction::BuyShopPotion { slot: 0 }).expect("buy potion");

        assert_eq!(after.phase, RunPhase::Shop);
        assert!(after.shop.is_some());
        assert_eq!(after.gold, gold_before - SHOP_FIRE_POTION_PRICE);
        assert_eq!(after.potions.len(), potions_before + 1);
        assert_eq!(after.potions.last(), Some(&Potion::Fire));
    }

    #[test]
    fn buy_shop_potion_rejects_full_belt() {
        let mut run = shop_run();
        run.potions = vec![Potion::Fire, Potion::Fire, Potion::Fire];

        let err =
            apply_shop_action(&run, RunAction::BuyShopPotion { slot: 0 }).expect_err("belt full");

        assert_eq!(err, SimError::IllegalAction("potion belt is full"));
    }

    #[test]
    fn buy_shop_potion_allows_extra_slots_with_potion_belt() {
        let mut run = shop_run();
        run.relics.push(crate::Relic::PotionBelt);
        run.potions = vec![Potion::Fire, Potion::Fire, Potion::Fire];

        let after =
            apply_shop_action(&run, RunAction::BuyShopPotion { slot: 0 }).expect("buy potion");

        assert_eq!(after.potions.len(), 4);
    }

    #[test]
    fn buy_shop_potion_rejects_insufficient_gold() {
        let mut run = shop_run();
        run.gold = SHOP_FIRE_POTION_PRICE - 1;

        let err = apply_shop_action(&run, RunAction::BuyShopPotion { slot: 0 })
            .expect_err("cannot afford");

        assert_eq!(err, SimError::IllegalAction("not enough gold"));
    }

    #[test]
    fn legal_shop_actions_include_affordable_card_and_potion_at_starting_gold() {
        let run = shop_run();

        assert!(legal_shop_actions(&run).contains(&RunAction::BuyShopCard { slot: 0 }));
        assert!(legal_shop_actions(&run).contains(&RunAction::BuyShopPotion { slot: 0 }));
    }

    #[test]
    fn meal_ticket_heals_when_shop_merchant_opens() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Shop;
        run.player_hp = 40;
        run.relics.push(Relic::MealTicket);

        open_shop_merchant(&mut run);

        assert_eq!(
            run.player_hp,
            (40 + crate::relic::MEAL_TICKET_HEAL).min(run.player_max_hp)
        );
        assert!(run.shop.is_some());
    }

    #[test]
    fn buy_shop_card_rejects_insufficient_gold() {
        let mut run = shop_run();
        run.gold = SHOP_ANGER_PRICE - 1;

        let err =
            apply_shop_action(&run, RunAction::BuyShopCard { slot: 0 }).expect_err("cannot afford");

        assert_eq!(err, SimError::IllegalAction("not enough gold"));
    }

    #[test]
    fn shop_action_is_illegal_outside_shop_phase() {
        let run = RunState::map_fixture();

        let err =
            apply_shop_action(&run, RunAction::BuyShopCard { slot: 0 }).expect_err("not at shop");

        assert_eq!(
            err,
            SimError::IllegalAction("shop actions require shop phase")
        );
    }

    #[test]
    fn test_seed_shop_inventory_matches_trace() {
        use crate::run::{
            event::{apply_event_action, enter_event_screen, Event},
            reward::{
                enter_chest_relic_reward_screen, enter_elite_combat_reward_screen,
                enter_normal_combat_reward_screen, setup_treasure_room,
            },
        };
        use crate::EventAction;

        const TEST: i64 = 1_218_623;
        let mut run = RunState::map_fixture();
        for field in [
            &mut run.reward_rng_seed,
            &mut run.treasure_rng_seed,
            &mut run.potion_rng_seed,
            &mut run.relic_rng_seed,
            &mut run.merchant_rng_seed,
            &mut run.event_rng_seed,
            &mut run.misc_rng_seed,
        ] {
            *field = TEST as u64;
        }

        run.event_rng_counter = 24;
        run.current_floor = 3;
        enter_event_screen(&mut run);
        assert_eq!(run.event.as_ref().unwrap().event, Event::ScrapOoze);
        run = apply_event_action(&run, EventAction::Choose { choice_index: 0 }).unwrap();
        run = apply_event_action(&run, EventAction::Choose { choice_index: 0 }).unwrap();
        run = apply_event_action(&run, EventAction::Choose { choice_index: 0 }).unwrap();

        run.current_floor = 4;
        enter_event_screen(&mut run);
        assert_eq!(run.event.as_ref().unwrap().event, Event::BigFish);
        run = apply_event_action(&run, EventAction::Choose { choice_index: 2 }).unwrap();
        run = apply_event_action(&run, EventAction::Choose { choice_index: 0 }).unwrap();

        let combats: [(i32, bool); 8] = [
            (1, false),
            (2, false),
            (5, false),
            (6, true),
            (8, false),
            (10, true),
            (11, false),
            (12, false),
        ];
        for (floor, elite) in combats {
            run.current_floor = floor;
            if floor == 6 {
                // The TEST trace uses the floor-1 Power Potion during Lagavulin before the elite reward.
                run.potions.clear();
            }
            if elite {
                enter_elite_combat_reward_screen(&mut run);
                if let Some(key) = run.reward.as_ref().and_then(|r| r.relic_key_offer) {
                    run.gain_relic_key(key);
                }
            } else {
                enter_normal_combat_reward_screen(&mut run);
            }
            if floor == 1 {
                run = crate::run::apply_run_action(&run, RunAction::TakePotionReward)
                    .expect("take floor-1 power potion");
            } else if run.reward.as_ref().and_then(|r| r.potion_offer).is_some() {
                run = crate::run::apply_run_action(&run, RunAction::SkipPotionReward)
                    .expect("skip later potion reward");
            }
            run.reward = None;
        }

        run.current_floor = 9;
        setup_treasure_room(&mut run);
        enter_chest_relic_reward_screen(&mut run);
        if let Some(key) = run.reward.as_ref().and_then(|r| r.relic_key_offer) {
            run.gain_relic_key(key);
        }
        run.reward = None;

        run.current_floor = 13;
        open_shop_merchant(&mut run);
        let shop = run.shop.expect("shop");

        let relic_keys: Vec<_> = shop.relics.iter().map(|offer| offer.relic_key).collect();
        assert_eq!(
            relic_keys,
            [
                RelicKey::Whetstone,
                RelicKey::Orichalcum,
                RelicKey::MembershipCard
            ]
        );

        let potions: Vec<_> = shop.potions.iter().map(|offer| offer.potion).collect();
        assert_eq!(
            potions,
            [Potion::EntropicBrew, Potion::Energy, Potion::Fear]
        );
    }
}
