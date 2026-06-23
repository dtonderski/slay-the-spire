use crate::{
    card::CardInstance, content::cards::upgrade_content_id, RunPhase, RunState, SimError, SimResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GridPurpose {
    RestSmith,
    ShopRemove,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CardGridScreen {
    pub cards: Vec<CardInstance>,
    pub purpose: GridPurpose,
    #[serde(default)]
    pub selected: Option<usize>,
}

pub fn open_rest_smith_grid(run: &mut RunState) {
    run.card_grid = Some(CardGridScreen {
        cards: run.deck.clone(),
        purpose: GridPurpose::RestSmith,
        selected: None,
    });
}

pub fn open_shop_remove_grid(run: &mut RunState) {
    run.card_grid = Some(CardGridScreen {
        cards: run.deck.clone(),
        purpose: GridPurpose::ShopRemove,
        selected: None,
    });
}

pub fn select_grid_card(run: &RunState, index: usize) -> SimResult<RunState> {
    let grid = run
        .card_grid
        .as_ref()
        .ok_or(SimError::IllegalAction("no card grid is open"))?;
    if index >= grid.cards.len() {
        return Err(SimError::IllegalAction("grid index out of range"));
    }

    let mut next = run.clone();
    let grid = next.card_grid.as_mut().expect("grid present");
    grid.selected = Some(index);
    Ok(next)
}

pub fn cancel_grid(run: &RunState) -> SimResult<RunState> {
    if run.card_grid.is_none() {
        return Err(SimError::IllegalAction("no card grid is open"));
    }
    let mut next = run.clone();
    next.card_grid = None;
    Ok(next)
}

pub fn confirm_grid(run: &RunState) -> SimResult<RunState> {
    let grid = run
        .card_grid
        .as_ref()
        .ok_or(SimError::IllegalAction("no card grid is open"))?;
    let selected = grid
        .selected
        .ok_or(SimError::IllegalAction("no card selected in grid"))?;
    let card = grid.cards[selected];

    let mut next = run.clone();
    match grid.purpose {
        GridPurpose::RestSmith => {
            let upgraded = upgrade_content_id(card.content_id)
                .ok_or(SimError::IllegalAction("card cannot be upgraded"))?;
            for deck_card in &mut next.deck {
                if deck_card.id == card.id {
                    deck_card.content_id = upgraded;
                    break;
                }
            }
            next.card_grid = None;
            next.phase = RunPhase::Idle;
        }
        GridPurpose::ShopRemove => {
            let shop = next
                .shop
                .as_ref()
                .ok_or(SimError::InvalidState("shop screen is missing"))?;
            let cost = shop.remove_cost;
            if next.gold < cost {
                return Err(SimError::IllegalAction("not enough gold"));
            }
            next.gold -= cost;
            next.shop_remove_count += 1;
            next.deck.retain(|deck_card| deck_card.id != card.id);
            let remove_cost = super::shop::shop_remove_cost_for_run(&next);
            if let Some(shop) = next.shop.as_mut() {
                shop.remove_cost = remove_cost;
            }
            next.card_grid = None;
        }
    }

    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{content::cards::STRIKE_R_PLUS_ID, RunState};

    #[test]
    fn rest_smith_grid_upgrades_selected_card() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        open_rest_smith_grid(&mut run);
        let strike_id = run.deck[0].id;

        let selected = select_grid_card(&run, 0).expect("select");
        let after = confirm_grid(&selected).expect("confirm");

        assert!(after.card_grid.is_none());
        assert_eq!(after.phase, RunPhase::Idle);
        assert_eq!(after.deck[0].content_id, STRIKE_R_PLUS_ID);
        assert_eq!(after.deck[0].id, strike_id);
    }

    #[test]
    fn shop_remove_grid_removes_card_and_charges_gold() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Shop;
        run.gold = 100;
        run.shop = Some(super::super::shop::fixed_shop_screen(
            run.next_card_instance_id(),
        ));
        open_shop_remove_grid(&mut run);
        let strike_id = run.deck[0].id;
        let deck_len = run.deck.len();

        let selected = select_grid_card(&run, 0).expect("select");
        let after = confirm_grid(&selected).expect("confirm");

        assert!(after.card_grid.is_none());
        assert_eq!(after.deck.len(), deck_len - 1);
        assert!(!after.deck.iter().any(|card| card.id == strike_id));
        assert_eq!(after.gold, 100 - super::super::shop::SHOP_BASE_REMOVE_PRICE);
        assert_eq!(after.shop_remove_count, 1);
    }
}
