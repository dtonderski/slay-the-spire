use crate::{
    card::{CardInstance, CardRarity, CardType},
    content::shop_pool::{
        assign_random_class_card_excluding, class_card_pool_of_type_and_rarity_with_fallback,
        random_class_card_of_type_and_rarity, random_colorless_from_pool, roll_card_rarity_shop,
        shop_card_is_colorless, shop_card_price_rarity, shop_card_type,
    },
    ids::CardId,
    potion::Potion,
    relic::{Relic, RelicKey, RelicTier},
    rng::{ExternalRngKind, StsRng},
    run::grid::open_shop_remove_grid,
    run::reward::{
        enter_orrery_reward_screen, queue_orrery_card_reward_choices, target_random_potion,
        target_uniform_random_potion,
    },
    RunAction, RunPhase, RunState, SimError, SimResult,
};

pub const SHOP_BASE_REMOVE_PRICE: i32 = 75;
pub const SHOP_REMOVE_PRICE_INCREASE: i32 = 25;
pub(crate) const MAX_SHOP_REMOVE_COUNT: u32 =
    ((i32::MAX - SHOP_BASE_REMOVE_PRICE) / SHOP_REMOVE_PRICE_INCREASE) as u32;

const SHOP_CARD_COMMON_PRICE: i32 = 50;
const SHOP_CARD_UNCOMMON_PRICE: i32 = 75;
const SHOP_CARD_RARE_PRICE: i32 = 150;
const SHOP_RELIC_COMMON_PRICE: i32 = 150;
const SHOP_RELIC_UNCOMMON_PRICE: i32 = 250;
const SHOP_RELIC_RARE_PRICE: i32 = 300;
const SHOP_RELIC_SHOP_PRICE: i32 = 150;
const SHOP_POTION_COMMON_PRICE: i32 = 50;
const SHOP_POTION_UNCOMMON_PRICE: i32 = 75;
const SHOP_POTION_RARE_PRICE: i32 = 100;
const SHOP_COLORLESS_RARE_CHANCE: f32 = 0.3;

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
    #[serde(default = "default_shop_remove_available")]
    pub remove_available: bool,
    #[serde(default)]
    pub sale_slot: Option<usize>,
}

fn default_shop_remove_available() -> bool {
    true
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

fn card_price_float_for_rarity(rarity: CardRarity, merchant_rng: &mut StsRng) -> f32 {
    // Target `AbstractCard.getPrice` bases, not shop class-card bases.
    let base = match rarity {
        CardRarity::Common => SHOP_CARD_COMMON_PRICE,
        CardRarity::Uncommon => SHOP_POTION_UNCOMMON_PRICE,
        CardRarity::Rare => 150,
    };
    base as f32 * merchant_rng.random_float_range(0.9, 1.1)
}

fn shop_colorless_card_price_for_rarity(rarity: CardRarity, merchant_rng: &mut StsRng) -> i32 {
    (card_price_float_for_rarity(rarity, merchant_rng) * 1.2) as i32
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

pub fn shop_remove_cost_for_run(run: &RunState) -> SimResult<i32> {
    shop_remove_cost_for_count(run, run.shop_remove_count)
}

pub(crate) fn shop_remove_cost_for_count(run: &RunState, count: u32) -> SimResult<i32> {
    let mut cost = shop_base_remove_cost(count)?;
    if owns_relic_key(run, RelicKey::SmilingMask) {
        return Ok(50);
    }

    if has_the_courier(run) {
        cost = round_discount(cost, 4, 5);
    }
    if has_membership_card(run) {
        cost = round_discount(cost, 1, 2);
    }

    Ok(cost)
}

fn shop_base_remove_cost(count: u32) -> SimResult<i32> {
    let count = i32::try_from(count)
        .map_err(|_| SimError::InvalidState("shop remove count exceeds i32"))?;
    SHOP_REMOVE_PRICE_INCREASE
        .checked_mul(count)
        .and_then(|increase| SHOP_BASE_REMOVE_PRICE.checked_add(increase))
        .ok_or(SimError::InvalidState("shop remove price overflows i32"))
}

fn has_membership_card(run: &RunState) -> bool {
    owns_relic_key(run, RelicKey::MembershipCard)
}

fn has_the_courier(run: &RunState) -> bool {
    owns_relic_key(run, RelicKey::TheCourier)
}

fn round_discount(price: i32, numerator: i32, denominator: i32) -> i32 {
    let rounded = (i64::from(price) * i64::from(numerator) + i64::from(denominator) / 2)
        / i64::from(denominator);
    i32::try_from(rounded).expect("static shop discount of an i32 price fits i32")
}

fn apply_discount_to_shop(shop: &mut ShopScreen, numerator: i32, denominator: i32) {
    for offer in &mut shop.cards {
        if !offer.sold {
            offer.price = round_discount(offer.price, numerator, denominator);
        }
    }
    for offer in &mut shop.relics {
        if !offer.sold {
            offer.price = round_discount(offer.price, numerator, denominator);
        }
    }
    for offer in &mut shop.potions {
        if !offer.sold {
            offer.price = round_discount(offer.price, numerator, denominator);
        }
    }
}

fn apply_courier_discount_to_shop(shop: &mut ShopScreen) {
    apply_discount_to_shop(shop, 4, 5);
    shop.remove_cost = round_discount(shop.remove_cost, 4, 5);
}

fn apply_membership_discount_to_shop(shop: &mut ShopScreen) {
    apply_discount_to_shop(shop, 1, 2);
    shop.remove_cost = round_discount(shop.remove_cost, 1, 2);
}

fn apply_relic_discounts_to_price(mut price: i32, run: &RunState) -> i32 {
    if has_the_courier(run) {
        price = round_discount(price, 4, 5);
    }
    if has_membership_card(run) {
        price = round_discount(price, 1, 2);
    }
    price
}

fn set_restocked_card_price(offer: &mut ShopCardSlot, run: &RunState, merchant_rng: &mut StsRng) {
    let mut price = if shop_card_is_colorless(offer.card.content_id) {
        shop_colorless_card_price_for_rarity(
            shop_card_price_rarity(offer.card.content_id),
            merchant_rng,
        )
    } else {
        card_price_for_rarity(shop_card_price_rarity(offer.card.content_id), merchant_rng)
    };
    if has_the_courier(run) {
        price = (price as f32 * 0.8) as i32;
    }
    if has_membership_card(run) {
        price = (price as f32 * 0.5) as i32;
    }
    offer.price = price;
}

fn owns_relic_key(run: &RunState, key: RelicKey) -> bool {
    run.relics.iter().any(|relic| relic.key() == key)
}

fn can_open_shop_remove(run: &RunState, shop: &ShopScreen) -> bool {
    shop.remove_available && run.gold >= shop.remove_cost
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
    if can_open_shop_remove(run, shop) {
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
        if !offer.sold
            && run.gold >= offer.price
            && run.open_potion_slots() > 0
            && run.can_gain_potions()
        {
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

fn courier_restock_relic_key(run: &mut RunState, merchant_rng: &mut StsRng) -> (RelicKey, i32) {
    loop {
        let tier = shop_relic_tier_roll(merchant_rng);
        let key = roll_shop_relic(run, tier);
        if matches!(
            key,
            RelicKey::OldCoin | RelicKey::SmilingMask | RelicKey::MawBank | RelicKey::TheCourier
        ) {
            continue;
        }
        let price = apply_relic_discounts_to_price(relic_price(tier, merchant_rng), run);
        return (key, price);
    }
}

fn restock_courier_card_slot(
    next: &mut RunState,
    slot: usize,
    purchased: CardInstance,
) -> SimResult<()> {
    let next_card_id = next.next_card_instance_id()?;
    let mut card_rng = StsRng::with_counter(next.reward_rng_seed as i64, next.card_rng_counter);
    let mut merchant_rng =
        StsRng::with_counter(next.merchant_rng_seed as i64, next.merchant_rng_counter);
    let content_id =
        if shop_card_is_colorless(purchased.content_id) {
            let rarity = if merchant_rng.random_float() < SHOP_COLORLESS_RARE_CHANCE {
                CardRarity::Rare
            } else {
                CardRarity::Uncommon
            };
            random_colorless_from_pool(&mut card_rng, rarity)
        } else {
            let card_type = shop_card_type(purchased.content_id)
                .ok_or(SimError::UnsupportedMechanic(purchased.content_id))?;
            loop {
                let rarity = roll_card_rarity_shop(&mut card_rng, next.card_rarity_factor);
                let pool = class_card_pool_of_type_and_rarity_with_fallback(card_type, rarity);
                let range_inclusive = u32::try_from(pool.len().saturating_sub(1))
                    .map_err(|_| SimError::InvalidState("Courier card pool exceeds u32"))?;
                let input = next.pending_external_rng.first().copied().ok_or(
                    SimError::MissingExternalRng("courier_colored_card_selection"),
                )?;
                if input.kind != ExternalRngKind::CardGroupGetRandomCardByType
                    || input.range_inclusive != range_inclusive
                {
                    return Err(SimError::ExternalRngMismatch(
                        "courier_colored_card_selection",
                    ));
                }
                next.pending_external_rng.remove(0);
                let mut math_utils_rng = input.state;
                let id = pool[math_utils_rng.random_int(range_inclusive) as usize];
                if !shop_card_is_colorless(id) {
                    break id;
                }
            }
        };
    next.card_rng_counter = card_rng.counter();

    let card = CardInstance::new(CardId::new(next_card_id), content_id);
    let mut offer = ShopCardSlot {
        card,
        price: 0,
        sold: false,
    };
    set_restocked_card_price(&mut offer, next, &mut merchant_rng);
    next.merchant_rng_counter = merchant_rng.counter();
    let shop = next.shop.as_mut().expect("validated shop screen");
    shop.cards[slot] = offer;
    Ok(())
}

fn restock_courier_relic_slot(next: &mut RunState, slot: usize) {
    let mut merchant_rng =
        StsRng::with_counter(next.merchant_rng_seed as i64, next.merchant_rng_counter);
    let (relic_key, price) = courier_restock_relic_key(next, &mut merchant_rng);
    next.merchant_rng_counter = merchant_rng.counter();
    let shop = next.shop.as_mut().expect("validated shop screen");
    shop.relics[slot] = ShopRelicSlot {
        relic_key,
        price,
        sold: false,
    };
}

fn restock_courier_potion_slot(next: &mut RunState, slot: usize) {
    let mut potion_rng = StsRng::with_counter(next.potion_rng_seed as i64, next.potion_rng_counter);
    let mut merchant_rng =
        StsRng::with_counter(next.merchant_rng_seed as i64, next.merchant_rng_counter);
    let potion = target_uniform_random_potion(&mut potion_rng);
    let price = apply_relic_discounts_to_price(potion_price(potion, &mut merchant_rng), next);
    next.potion_rng_counter = potion_rng.counter();
    next.merchant_rng_counter = merchant_rng.counter();
    let shop = next.shop.as_mut().expect("validated shop screen");
    shop.potions[slot] = ShopPotionSlot {
        potion,
        price,
        sold: false,
    };
}

pub fn generate_shop_screen(run: &mut RunState) -> SimResult<ShopScreen> {
    let remove_cost = shop_base_remove_cost(run.shop_remove_count)?;
    let mut next_card_id = run.reserve_card_instance_ids(7)?;
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
    prices[5] = shop_colorless_card_price_for_rarity(CardRarity::Uncommon, &mut merchant_rng);
    prices[6] = shop_colorless_card_price_for_rarity(CardRarity::Rare, &mut merchant_rng);

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
        remove_cost,
        remove_available: true,
        sale_slot: Some(sale_slot),
    };
    if has_the_courier(run) {
        apply_courier_discount_to_shop(&mut shop);
    }
    if has_membership_card(run) {
        apply_membership_discount_to_shop(&mut shop);
    }
    if owns_relic_key(run, RelicKey::SmilingMask) {
        shop.remove_cost = 50;
    }
    Ok(shop)
}

pub fn enter_shop_room(run: &mut RunState) -> SimResult<()> {
    let mut next = run.clone();
    next.phase = RunPhase::Shop;
    // ShopRoom.onPlayerEntry constructs Merchant, whose constructor initializes
    // ShopScreen before the merchant UI is opened.
    next.shop = Some(generate_shop_screen(&mut next)?);
    next.shop_merchant_open = false;
    next.card_grid = None;
    if next.relics.contains(&Relic::MealTicket) {
        next.heal_player(crate::relic::MEAL_TICKET_HEAL)?;
    }
    *run = next;
    Ok(())
}

pub fn open_shop_merchant(run: &mut RunState) -> SimResult<()> {
    run.phase = RunPhase::Shop;
    if run.shop.is_none() {
        run.shop = Some(generate_shop_screen(run)?);
    }
    run.shop_merchant_open = true;
    Ok(())
}

pub fn enter_shop_screen(run: &mut RunState) -> SimResult<()> {
    open_shop_merchant(run)
}

pub fn leave_shop_merchant(run: &mut RunState) {
    run.shop_merchant_open = false;
    run.card_grid = None;
}

pub fn leave_shop_room(run: &mut RunState) {
    run.shop = None;
    run.shop_merchant_open = false;
    run.card_grid = None;
    run.phase = RunPhase::Idle;
}

pub fn legal_shop_actions(run: &RunState) -> SimResult<Vec<RunAction>> {
    run.validate()?;
    if run.phase != RunPhase::Shop {
        return Ok(Vec::new());
    }

    if run.card_grid.is_some() {
        return Ok(Vec::new());
    }

    if !run.shop_merchant_open {
        let mut actions = vec![RunAction::EnterShop];
        if run.shop.is_some() {
            actions.push(RunAction::Proceed);
        }
        return Ok(actions);
    }

    let Some(shop) = run.shop.as_ref() else {
        return Err(SimError::InvalidState("shop screen is missing"));
    };

    let mut actions = Vec::new();

    if can_open_shop_remove(run, shop) {
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
        if !offer.sold
            && run.gold >= offer.price
            && run.open_potion_slots() > 0
            && run.can_gain_potions()
        {
            actions.push(RunAction::BuyShopPotion { slot });
        }
    }

    actions.push(RunAction::LeaveShop);
    Ok(actions)
}

pub fn validate_shop_action(run: &RunState, action: RunAction) -> SimResult<()> {
    run.validate()?;

    if run.phase != RunPhase::Shop {
        return Err(SimError::IllegalAction("shop actions require shop phase"));
    }

    match action {
        RunAction::EnterShop if !run.shop_merchant_open && run.card_grid.is_none() => Ok(()),
        RunAction::LeaveShop
            if run.shop_merchant_open && run.shop.is_some() && run.card_grid.is_none() =>
        {
            Ok(())
        }
        RunAction::Proceed
            if !run.shop_merchant_open && run.shop.is_some() && run.card_grid.is_none() =>
        {
            Ok(())
        }
        RunAction::OpenShopRemove => {
            if !run.shop_merchant_open {
                return Err(SimError::IllegalAction("shop merchant is not open"));
            }
            let shop = run
                .shop
                .as_ref()
                .ok_or(SimError::InvalidState("shop screen is missing"))?;
            if run.card_grid.is_some() {
                return Err(SimError::IllegalAction("grid already open"));
            }
            if !shop.remove_available {
                return Err(SimError::IllegalAction("shop remove already used"));
            }
            if !can_open_shop_remove(run, shop) {
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
                    if !run.can_gain_potions() {
                        return Err(SimError::IllegalAction("potions cannot be obtained"));
                    }
                    if run.open_potion_slots() == 0 {
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
            open_shop_merchant(&mut next)?;
        }
        RunAction::LeaveShop => {
            leave_shop_merchant(&mut next);
        }
        RunAction::Proceed => {
            leave_shop_room(&mut next);
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
            next.break_maw_bank_on_shop_spend();
            next.add_deck_card(card)?;
            if has_the_courier(&next) {
                restock_courier_card_slot(&mut next, slot, card)?;
            }
        }
        RunAction::BuyShopRelic { slot } => {
            let shop = next.shop.as_mut().expect("validated shop screen");
            let offer = shop.relics.get_mut(slot).expect("validated relic offer");
            let key = offer.relic_key;
            let price = offer.price;
            offer.sold = true;
            next.gold -= price;
            next.break_maw_bank_on_shop_spend();
            if key == RelicKey::Orrery {
                enter_orrery_reward_screen(&mut next);
            }
            next.gain_relic_key(key)?;
            if key == RelicKey::Orrery {
                queue_orrery_card_reward_choices(&mut next)?;
            }
            if key == RelicKey::MembershipCard {
                if let Some(shop) = next.shop.as_mut() {
                    apply_membership_discount_to_shop(shop);
                }
            }
            if key == RelicKey::TheCourier || has_the_courier(&next) {
                restock_courier_relic_slot(&mut next, slot);
            }
        }
        RunAction::BuyShopPotion { slot } => {
            let shop = next.shop.as_mut().expect("validated shop screen");
            let offer = shop.potions.get_mut(slot).expect("validated potion offer");
            let potion = offer.potion;
            let price = offer.price;
            offer.sold = true;
            next.gold -= price;
            next.break_maw_bank_on_shop_spend();
            next.gain_potion(potion)?;
            if has_the_courier(&next) {
                restock_courier_potion_slot(&mut next, slot);
            }
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

    #[test]
    fn shop_remove_prices_are_checked_without_changing_valid_discounts() {
        let mut run = RunState::map_fixture();
        assert_eq!(shop_remove_cost_for_run(&run), Ok(75));

        run.shop_remove_count = 1;
        assert_eq!(shop_remove_cost_for_run(&run), Ok(100));

        run.relics = vec![Relic::TheCourier];
        assert_eq!(shop_remove_cost_for_run(&run), Ok(80));

        run.relics.push(Relic::MembershipCard);
        assert_eq!(shop_remove_cost_for_run(&run), Ok(40));

        run.relics = vec![Relic::MembershipCard];
        assert_eq!(shop_remove_cost_for_run(&run), Ok(50));

        run.relics = vec![Relic::TheCourier, Relic::MembershipCard, Relic::SmilingMask];
        assert_eq!(shop_remove_cost_for_run(&run), Ok(50));

        run.relics.clear();
        run.shop_remove_count = MAX_SHOP_REMOVE_COUNT;
        assert_eq!(shop_remove_cost_for_run(&run), Ok(2_147_483_625));
    }

    #[test]
    fn invalid_shop_remove_count_fails_before_shop_generation_mutates_run() {
        let mut run = RunState::map_fixture();
        run.shop_remove_count = MAX_SHOP_REMOVE_COUNT + 1;
        let before = run.clone();

        assert_eq!(
            run.validate(),
            Err(SimError::InvalidState(
                "shop remove count exceeds the supported price range"
            ))
        );
        assert_eq!(
            shop_remove_cost_for_run(&run),
            Err(SimError::InvalidState("shop remove price overflows i32"))
        );
        assert_eq!(
            generate_shop_screen(&mut run),
            Err(SimError::InvalidState("shop remove price overflows i32"))
        );
        assert_eq!(run, before);
    }

    #[test]
    fn non_shop_card_offer_is_rejected_before_courier_restock() {
        let mut run = RunState::seeded_ironclad(7, 10);
        run.phase = RunPhase::Shop;
        run.event = None;
        run.gold = 999;
        run.relics.push(Relic::TheCourier);
        let mut shop = generate_shop_screen(&mut run).expect("shop fixture allocation is valid");
        shop.cards[0].card.content_id = crate::content::cards::ASCENDERS_BANE_ID;
        shop.cards[0].price = 0;
        run.shop = Some(shop);
        run.shop_merchant_open = true;

        assert_eq!(
            apply_shop_action(&run, RunAction::BuyShopCard { slot: 0 }),
            Err(SimError::UnsupportedMechanic(
                crate::content::cards::ASCENDERS_BANE_ID
            ))
        );
    }

    #[test]
    fn entered_shop_room_can_leave_without_opening_merchant() {
        let mut run = RunState::map_fixture();
        enter_shop_room(&mut run).expect("shop entry succeeds");

        assert!(run.shop.is_some());
        assert_eq!(
            legal_shop_actions(&run),
            Ok(vec![RunAction::EnterShop, RunAction::Proceed])
        );
        assert_eq!(validate_shop_action(&run, RunAction::EnterShop), Ok(()));
        let left = apply_shop_action(&run, RunAction::Proceed).expect("shop room can close");
        assert_eq!(left.phase, RunPhase::Idle);
        assert!(left.shop.is_none());
    }

    #[test]
    fn leaving_merchant_preserves_inventory_until_shop_room_exit() {
        let mut run = RunState::map_fixture();
        run.gold = 999;
        enter_shop_room(&mut run).expect("shop entry succeeds");

        let opened = apply_shop_action(&run, RunAction::EnterShop).expect("shop opens");
        assert!(opened.shop_merchant_open);
        let inventory = opened.shop.clone().expect("generated inventory");
        assert_eq!(inventory.cards.len(), 7);

        let closed = apply_shop_action(&opened, RunAction::LeaveShop).expect("merchant closes");
        assert!(!closed.shop_merchant_open);
        assert_eq!(closed.shop.as_ref(), Some(&inventory));
        assert_eq!(
            legal_shop_actions(&closed).expect("valid closed merchant state"),
            vec![RunAction::EnterShop, RunAction::Proceed]
        );

        let reopened = apply_shop_action(&closed, RunAction::EnterShop).expect("merchant reopens");
        assert!(reopened.shop_merchant_open);
        assert_eq!(reopened.shop.as_ref(), Some(&inventory));

        let left_room = apply_shop_action(&closed, RunAction::Proceed).expect("shop room closes");
        assert_eq!(left_room.phase, RunPhase::Idle);
        assert!(left_room.shop.is_none());
        assert!(!left_room.shop_merchant_open);
    }
    #[test]
    fn zero_seed_shop_uses_generated_inventory_and_rng_streams() {
        let mut run = RunState::map_fixture();
        assert_eq!(run.merchant_rng_seed, 0);
        enter_shop_room(&mut run).expect("zero-seed shop entry succeeds");

        let shop = run.shop.as_ref().expect("zero-seed shop is generated");
        assert_eq!(shop.cards.len(), 7);
        assert_eq!(shop.relics.len(), 3);
        assert_eq!(shop.potions.len(), 3);
        assert!(shop.sale_slot.is_some());
        assert!(run.card_rng_counter > 0);
        assert!(run.merchant_rng_counter > 0);
        assert!(run.potion_rng_counter > 0);
    }

    #[test]
    fn shop_potions_use_one_uniform_potion_rng_draw_each() {
        let mut run = RunState::map_fixture();
        enter_shop_room(&mut run).expect("shop entry succeeds");

        let before_restock = run.potion_rng_counter;
        run.relics.push(Relic::TheCourier);
        restock_courier_potion_slot(&mut run, 0);
        assert_eq!(run.potion_rng_counter, before_restock + 1);
    }

    #[test]
    fn courier_restock_allocates_above_unsold_shop_card_ids() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.phase = RunPhase::Shop;
        run.event = None;
        run.gold = 999;
        run.relics.push(Relic::TheCourier);
        run.shop = Some(generate_shop_screen(&mut run).expect("shop generates"));
        run.shop_merchant_open = true;
        run.shop.as_mut().expect("shop").cards[0].card.content_id =
            crate::content::cards::DARK_SHACKLES_ID;
        run.shop.as_mut().expect("shop").cards[0].price = 0;
        let initial_shop_ids = run
            .shop
            .as_ref()
            .expect("shop")
            .cards
            .iter()
            .map(|offer| offer.card.id)
            .collect::<Vec<_>>();
        let initial_max = initial_shop_ids
            .iter()
            .map(|id| id.get())
            .max()
            .expect("shop has cards");

        let next = apply_shop_action(&run, RunAction::BuyShopCard { slot: 0 })
            .expect("Courier restock succeeds");
        let replacement_id = next.shop.as_ref().expect("shop").cards[0].card.id;
        assert!(replacement_id.get() > initial_max);
        assert!(
            next.deck
                .iter()
                .chain(
                    next.shop
                        .as_ref()
                        .expect("shop")
                        .cards
                        .iter()
                        .map(|offer| &offer.card)
                )
                .map(|card| card.id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == next.deck.len() + next.shop.as_ref().expect("shop").cards.len()
        );
    }

    #[test]
    fn courier_colored_restock_uses_card_rng_only_for_rarity() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.phase = RunPhase::Shop;
        run.event = None;
        run.gold = 999;
        run.relics.push(Relic::TheCourier);
        run.shop = Some(generate_shop_screen(&mut run).expect("shop generates"));
        run.shop_merchant_open = true;
        let purchased = CardInstance::new(
            run.shop.as_ref().unwrap().cards[0].card.id,
            crate::content::cards::HAVOC_ID,
        );
        run.shop.as_mut().unwrap().cards[0].card = purchased;
        run.shop.as_mut().unwrap().cards[0].price = 0;

        let card_rng_before = run.card_rng_counter;
        let mut rarity_rng = StsRng::with_counter(run.reward_rng_seed as i64, card_rng_before);
        let rarity = roll_card_rarity_shop(&mut rarity_rng, run.card_rarity_factor);
        let pool = class_card_pool_of_type_and_rarity_with_fallback(CardType::Skill, rarity);
        let range_inclusive = (pool.len() - 1) as u32;
        let state = crate::rng::MathUtilsRngState {
            state0: 0x0123_4567_89ab_cdef,
            state1: 0xfedc_ba98_7654_3210,
        };
        let mut expected_rng = state;
        let expected = pool[expected_rng.random_int(range_inclusive) as usize];
        run.pending_external_rng.push(crate::rng::ExternalRngInput {
            kind: ExternalRngKind::CardGroupGetRandomCardByType,
            state,
            range_inclusive,
        });

        let next = apply_shop_action(&run, RunAction::BuyShopCard { slot: 0 })
            .expect("instrumented Courier restock succeeds");

        assert_eq!(next.card_rng_counter, card_rng_before + 1);
        assert_eq!(
            next.shop.as_ref().unwrap().cards[0].card.content_id,
            expected
        );
        assert!(next.pending_external_rng.is_empty());
    }

    #[test]
    fn courier_colored_restock_fails_closed_without_external_rng() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.phase = RunPhase::Shop;
        run.event = None;
        run.gold = 999;
        run.relics.push(Relic::TheCourier);
        run.shop = Some(generate_shop_screen(&mut run).expect("shop generates"));
        run.shop_merchant_open = true;
        run.shop.as_mut().unwrap().cards[0].card.content_id = crate::content::cards::HAVOC_ID;
        run.shop.as_mut().unwrap().cards[0].price = 0;
        let before = run.clone();

        assert_eq!(
            apply_shop_action(&run, RunAction::BuyShopCard { slot: 0 }),
            Err(SimError::MissingExternalRng(
                "courier_colored_card_selection"
            ))
        );
        assert_eq!(run, before);
    }

    #[test]
    fn shop_generation_id_exhaustion_does_not_consume_rng_or_mutate_run() {
        let mut run = RunState::map_fixture();
        run.deck[0].id = CardId::new(crate::ids::MAX_SUPPORTED_CARD_INSTANCE_ID - 6);
        let before = run.clone();

        assert_eq!(
            generate_shop_screen(&mut run),
            Err(SimError::InvalidState(
                "card instance ID allocation exceeds the supported domain"
            ))
        );
        assert_eq!(run, before);
    }

    #[test]
    fn entering_shop_room_generates_inventory_before_merchant_is_opened() {
        let mut run = RunState::seeded_ironclad(3_840_209_149_409_335_969, 0);
        let card_rng_before = run.card_rng_counter;

        enter_shop_room(&mut run).expect("shop entry succeeds");

        assert!(run.shop.is_some());
        assert!(!run.shop_merchant_open);
        assert!(run.card_rng_counter > card_rng_before);
        let counters_after_entry = (
            run.card_rng_counter,
            run.merchant_rng_counter,
            run.potion_rng_counter,
            run.relic_rng_counter,
        );

        open_shop_merchant(&mut run).expect("merchant opens");

        assert!(run.shop_merchant_open);
        assert_eq!(
            (
                run.card_rng_counter,
                run.merchant_rng_counter,
                run.potion_rng_counter,
                run.relic_rng_counter,
            ),
            counters_after_entry
        );
    }

    #[test]
    fn buying_cauldron_opens_five_potion_rewards_then_returns_to_shop() {
        let mut run = RunState::seeded_ironclad(3_840_209_149_409_335_969, 0);
        run.phase = RunPhase::Shop;
        run.event = None;
        run.gold = 999;
        let shop = generate_shop_screen(&mut run).expect("shop fixture allocation is valid");
        run.shop = Some(shop);
        run.shop_merchant_open = true;
        run.shop.as_mut().unwrap().relics[0].relic_key = RelicKey::Cauldron;
        run.shop.as_mut().unwrap().relics[0].price = 0;
        let potion_count_before = run.potions.len();
        assert_eq!(run.card_rng_counter, 12);
        assert_eq!(run.card_rarity_factor, 5);
        let mut expected_potion_rng = run.rng_for_stream(crate::run::state::RunRngStream::Potion);
        let expected_potions = (0..crate::relic::CAULDRON_POTIONS)
            .map(|_| target_uniform_random_potion(&mut expected_potion_rng))
            .collect::<Vec<_>>();

        let mut next = apply_shop_action(&run, RunAction::BuyShopRelic { slot: 0 })
            .expect("Cauldron purchase succeeds");
        assert_eq!(next.phase, RunPhase::Reward);
        assert_eq!(next.card_rng_counter, 21);
        assert_eq!(next.card_rarity_factor, 3);
        assert_eq!(next.potions.len(), potion_count_before);
        assert_eq!(next.potion_rng_counter, expected_potion_rng.counter());
        assert_eq!(
            next.reward
                .as_ref()
                .expect("Cauldron reward screen")
                .continuation,
            crate::run::RewardContinuation::Shop
        );
        assert_eq!(
            next.reward
                .as_ref()
                .expect("Cauldron reward screen")
                .potion_offers,
            expected_potions
        );
        assert!(!next.shop_merchant_open);

        next = crate::run::reward::apply_run_action(&next, RunAction::SkipReward)
            .expect("skipping Cauldron rewards returns to shop room");
        assert_eq!(next.phase, RunPhase::Shop);
        assert!(next.reward.is_none());
        assert!(!next.shop_merchant_open);

        next = apply_shop_action(&next, RunAction::Proceed).expect("shop room closes");
        assert_eq!(next.phase, RunPhase::Idle);
        assert!(next.shop.is_none());
    }

    #[test]
    fn buying_orrery_opens_five_card_rewards_then_returns_to_shop() {
        let mut run = RunState::seeded_ironclad(3_840_209_149_409_335_969, 0);
        run.phase = RunPhase::Shop;
        run.event = None;
        run.gold = 999;
        let shop = generate_shop_screen(&mut run).expect("shop fixture allocation is valid");
        run.shop = Some(shop);
        run.shop_merchant_open = true;
        run.shop.as_mut().unwrap().relics[0].relic_key = RelicKey::Orrery;

        let mut next = apply_shop_action(&run, RunAction::BuyShopRelic { slot: 0 })
            .expect("Orrery purchase succeeds");
        assert_eq!(next.phase, RunPhase::Reward);
        assert_eq!(
            next.reward.as_ref().unwrap().remaining_card_reward_count(),
            crate::relic::ORRERY_CARD_REWARDS
        );
        // Merchant UI closes while the Orrery combat-reward overlay is up.
        assert!(!next.shop_merchant_open);

        let fifth_reward = next
            .reward
            .as_ref()
            .expect("Orrery reward screen")
            .queued_card_rewards[4]
            .clone();
        next = crate::run::reward::apply_run_action(
            &next,
            RunAction::OpenQueuedCardReward { index: 4 },
        )
        .expect("Orrery can open the selected fifth reward");
        assert_eq!(
            next.reward.as_ref().expect("active reward").choices,
            fifth_reward
        );
        let card_id = next.reward.as_ref().unwrap().choices[0].id;
        next = crate::run::reward::apply_run_action(&next, RunAction::TakeCardReward { card_id })
            .expect("selected Orrery reward can be taken");
        assert_eq!(
            next.reward.as_ref().unwrap().remaining_card_reward_count(),
            crate::relic::ORRERY_CARD_REWARDS - 1
        );

        for remaining in (0..crate::relic::ORRERY_CARD_REWARDS - 1).rev() {
            next = crate::run::reward::apply_run_action(&next, RunAction::OpenCardReward)
                .expect("Orrery card reward opens");
            let card_id = next.reward.as_ref().unwrap().choices[0].id;
            next =
                crate::run::reward::apply_run_action(&next, RunAction::TakeCardReward { card_id })
                    .expect("Orrery card reward can be taken");
            assert_eq!(next.phase, RunPhase::Reward);
            assert_eq!(
                next.reward.as_ref().unwrap().remaining_card_reward_count(),
                remaining
            );
        }

        // Final pick leaves an empty combat-reward frame (merchant still closed).
        assert_eq!(next.phase, RunPhase::Reward);
        assert!(next
            .reward
            .as_ref()
            .is_some_and(crate::run::reward::reward_is_empty));
        assert!(!next.shop_merchant_open);

        next = crate::run::reward::apply_run_action(&next, RunAction::SkipReward)
            .expect("empty Orrery reward SKIP returns to shop room");
        assert_eq!(next.phase, RunPhase::Shop);
        assert!(next.reward.is_none());
        assert!(!next.shop_merchant_open);

        next = apply_shop_action(&next, RunAction::EnterShop).expect("re-open merchant");
        assert!(next.shop_merchant_open);
    }
}
