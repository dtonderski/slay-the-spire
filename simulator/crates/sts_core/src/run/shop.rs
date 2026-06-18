use crate::{
    card::CardInstance,
    content::cards::ANGER_ID,
    ids::CardId,
    potion::{Potion, MAX_POTIONS},
    Relic, RunAction, RunPhase, RunState, SimError, SimResult,
};

pub const SHOP_ANGER_PRICE: i32 = 50;
pub const SHOP_VAJRA_PRICE: i32 = 150;
pub const SHOP_FIRE_POTION_PRICE: i32 = 50;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShopCardSlot {
    pub card: CardInstance,
    pub price: i32,
    pub sold: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShopRelicSlot {
    pub relic: Relic,
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
    #[serde(default)]
    pub relic: Option<ShopRelicSlot>,
    #[serde(default)]
    pub potion: Option<ShopPotionSlot>,
}

#[must_use]
pub fn fixed_shop_screen(next_card_id: u64) -> ShopScreen {
    ShopScreen {
        cards: vec![ShopCardSlot {
            card: CardInstance::new(CardId::new(next_card_id), ANGER_ID),
            price: SHOP_ANGER_PRICE,
            sold: false,
        }],
        relic: Some(ShopRelicSlot {
            relic: Relic::Vajra,
            price: SHOP_VAJRA_PRICE,
            sold: false,
        }),
        potion: Some(ShopPotionSlot {
            potion: Potion::Fire,
            price: SHOP_FIRE_POTION_PRICE,
            sold: false,
        }),
    }
}

pub fn enter_shop_screen(run: &mut RunState) {
    let next_card_id = run.next_card_instance_id();
    run.phase = RunPhase::Shop;
    run.shop = Some(fixed_shop_screen(next_card_id));
}

#[must_use]
pub fn legal_shop_actions(run: &RunState) -> Vec<RunAction> {
    if run.phase != RunPhase::Shop {
        return Vec::new();
    }

    let Some(shop) = run.shop.as_ref() else {
        return Vec::new();
    };

    let mut actions = Vec::new();

    for (slot, offer) in shop.cards.iter().enumerate() {
        if !offer.sold && run.gold >= offer.price {
            actions.push(RunAction::BuyShopCard { slot });
        }
    }

    if let Some(offer) = shop.relic {
        if !offer.sold && run.gold >= offer.price && !run.relics.contains(&offer.relic) {
            actions.push(RunAction::BuyShopRelic);
        }
    }

    if let Some(offer) = shop.potion {
        if !offer.sold && run.gold >= offer.price && run.potions.len() < MAX_POTIONS {
            actions.push(RunAction::BuyShopPotion);
        }
    }

    actions
}

pub fn validate_shop_action(run: &RunState, action: RunAction) -> SimResult<()> {
    if run.phase != RunPhase::Shop {
        return Err(SimError::IllegalAction("shop actions require shop phase"));
    }

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
        RunAction::BuyShopRelic => {
            let offer = shop
                .relic
                .ok_or(SimError::IllegalAction("shop relic is not available"))?;
            if offer.sold {
                return Err(SimError::IllegalAction("shop relic already sold"));
            }
            if run.relics.contains(&offer.relic) {
                return Err(SimError::IllegalAction("relic already owned"));
            }
            if run.gold < offer.price {
                return Err(SimError::IllegalAction("not enough gold"));
            }
            Ok(())
        }
        RunAction::BuyShopPotion => {
            let offer = shop
                .potion
                .ok_or(SimError::IllegalAction("shop potion is not available"))?;
            if offer.sold {
                return Err(SimError::IllegalAction("shop potion already sold"));
            }
            if run.potions.len() >= MAX_POTIONS {
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

pub fn apply_shop_action(run: &RunState, action: RunAction) -> SimResult<RunState> {
    validate_shop_action(run, action)?;

    let mut next = run.clone();
    match action {
        RunAction::BuyShopCard { slot } => {
            let shop = next.shop.as_mut().expect("validated shop screen");
            let offer = shop.cards.get_mut(slot).expect("validated slot");
            let card = offer.card;
            let price = offer.price;
            offer.sold = true;
            next.gold -= price;
            next.deck.push(card);
            next.phase = RunPhase::Idle;
            next.shop = None;
        }
        RunAction::BuyShopRelic => {
            let shop = next.shop.as_mut().expect("validated shop screen");
            let offer = shop.relic.as_mut().expect("validated relic offer");
            let relic = offer.relic;
            let price = offer.price;
            offer.sold = true;
            next.gold -= price;
            next.gain_relic(relic);
            next.phase = RunPhase::Idle;
            next.shop = None;
        }
        RunAction::BuyShopPotion => {
            let shop = next.shop.as_mut().expect("validated shop screen");
            let offer = shop.potion.as_mut().expect("validated potion offer");
            let potion = offer.potion;
            let price = offer.price;
            offer.sold = true;
            next.gold -= price;
            next.potions.push(potion);
            next.phase = RunPhase::Idle;
            next.shop = None;
        }
        _ => unreachable!("validated shop action"),
    }

    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{content::cards::ANGER_ID, map::RoomKind, MapAction, MapNodeId, VAJRA_STRENGTH};

    fn shop_run() -> RunState {
        let mut run = RunState::map_fixture();
        for node_id in [MapNodeId::new(1), MapNodeId::new(3), MapNodeId::new(4)] {
            run = crate::apply_map_action_on_run(&run, MapAction::ChooseNode { node_id })
                .expect("reach shop");
        }
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
        let relic = shop.relic.expect("vajra offer present");
        assert_eq!(relic.relic, Relic::Vajra);
        assert_eq!(relic.price, SHOP_VAJRA_PRICE);
        assert!(!relic.sold);
        let potion = shop.potion.expect("fire potion offer present");
        assert_eq!(potion.potion, Potion::Fire);
        assert_eq!(potion.price, SHOP_FIRE_POTION_PRICE);
        assert!(!potion.sold);
    }

    #[test]
    fn buy_shop_card_deducts_gold_and_adds_to_deck() {
        let run = shop_run();
        let gold_before = run.gold;
        let deck_len_before = run.deck.len();
        let anger_card_id = run.shop.as_ref().expect("shop").cards[0].card.id;

        let after = apply_shop_action(&run, RunAction::BuyShopCard { slot: 0 }).expect("buy anger");

        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.shop.is_none());
        assert_eq!(after.gold, gold_before - SHOP_ANGER_PRICE);
        assert_eq!(after.deck.len(), deck_len_before + 1);
        assert!(after.deck.iter().any(|card| card.id == anger_card_id));
        assert_eq!(after.count_content_in_deck(ANGER_ID), 1);
    }

    #[test]
    fn buy_shop_relic_deducts_gold_and_adds_vajra() {
        let mut run = shop_run();
        run.gold = SHOP_VAJRA_PRICE;

        let after = apply_shop_action(&run, RunAction::BuyShopRelic).expect("buy vajra");

        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.shop.is_none());
        assert_eq!(after.gold, 0);
        assert_eq!(after.relics, vec![Relic::Vajra]);
        let combat = after.init_combat(crate::CombatState::initial_fixture());
        assert_eq!(combat.player.powers.strength, VAJRA_STRENGTH);
    }

    #[test]
    fn buy_shop_relic_rejects_insufficient_gold() {
        let run = shop_run();

        let err = apply_shop_action(&run, RunAction::BuyShopRelic).expect_err("cannot afford");

        assert_eq!(err, SimError::IllegalAction("not enough gold"));
    }

    #[test]
    fn buy_shop_relic_rejects_duplicate_relic() {
        let mut run = shop_run();
        run.gold = SHOP_VAJRA_PRICE;
        run.relics.push(Relic::Vajra);

        let err = apply_shop_action(&run, RunAction::BuyShopRelic).expect_err("already owned");

        assert_eq!(err, SimError::IllegalAction("relic already owned"));
    }

    #[test]
    fn buy_shop_potion_deducts_gold_and_adds_fire_potion() {
        let run = shop_run();
        let gold_before = run.gold;
        let potions_before = run.potions.len();

        let after = apply_shop_action(&run, RunAction::BuyShopPotion).expect("buy potion");

        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.shop.is_none());
        assert_eq!(after.gold, gold_before - SHOP_FIRE_POTION_PRICE);
        assert_eq!(after.potions.len(), potions_before + 1);
        assert_eq!(after.potions.last(), Some(&Potion::Fire));
    }

    #[test]
    fn buy_shop_potion_rejects_full_belt() {
        let mut run = shop_run();
        run.potions = vec![Potion::Fire, Potion::Fire, Potion::Fire];

        let err = apply_shop_action(&run, RunAction::BuyShopPotion).expect_err("belt full");

        assert_eq!(err, SimError::IllegalAction("potion belt is full"));
    }

    #[test]
    fn buy_shop_potion_rejects_insufficient_gold() {
        let mut run = shop_run();
        run.gold = SHOP_FIRE_POTION_PRICE - 1;

        let err = apply_shop_action(&run, RunAction::BuyShopPotion).expect_err("cannot afford");

        assert_eq!(err, SimError::IllegalAction("not enough gold"));
    }

    #[test]
    fn legal_shop_actions_include_affordable_card_and_potion_at_starting_gold() {
        let run = shop_run();

        assert_eq!(
            legal_shop_actions(&run),
            vec![RunAction::BuyShopCard { slot: 0 }, RunAction::BuyShopPotion,]
        );
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
}
