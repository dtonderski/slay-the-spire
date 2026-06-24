use crate::{
    card::{CardInstance, CardType},
    content::{
        cards::{
            get_card_definition, is_pandoras_box_removed_starter, upgrade_content_id,
            CURSE_OF_THE_BELL_ID,
        },
        reward_pool::{ironclad_reward_content_ids, ironclad_truly_random_card_pool},
    },
    rng::StsRng,
    RunPhase, RunState, SimError, SimResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GridPurpose {
    RestSmith,
    ShopRemove,
    EmptyCage { remaining: u8 },
    Bottle { card_type: CardType },
    DollysMirror,
    CallingBellCurse,
    PandorasBox,
    Astrolabe,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CardGridScreen {
    pub cards: Vec<CardInstance>,
    pub purpose: GridPurpose,
    #[serde(default)]
    pub selected: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_indices: Vec<usize>,
}

pub fn open_rest_smith_grid(run: &mut RunState) {
    run.card_grid = Some(CardGridScreen {
        cards: run.deck.clone(),
        purpose: GridPurpose::RestSmith,
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_shop_remove_grid(run: &mut RunState) {
    run.card_grid = Some(CardGridScreen {
        cards: run.deck.clone(),
        purpose: GridPurpose::ShopRemove,
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_empty_cage_grid(run: &mut RunState) {
    if run.deck.is_empty() {
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards: run.deck.clone(),
        purpose: GridPurpose::EmptyCage { remaining: 2 },
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_bottle_grid(run: &mut RunState, card_type: CardType) {
    let cards = run
        .deck
        .iter()
        .copied()
        .filter(|card| {
            !card.bottled
                && get_card_definition(card.content_id)
                    .map(|definition| definition.card_type == card_type)
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    if cards.is_empty() {
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards,
        purpose: GridPurpose::Bottle { card_type },
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_dollys_mirror_grid(run: &mut RunState) {
    if run.deck.is_empty() {
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards: run.deck.clone(),
        purpose: GridPurpose::DollysMirror,
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_calling_bell_grid(run: &mut RunState) {
    run.card_grid = Some(CardGridScreen {
        cards: vec![CardInstance::new(
            crate::ids::CardId::new(run.next_card_instance_id()),
            CURSE_OF_THE_BELL_ID,
        )],
        purpose: GridPurpose::CallingBellCurse,
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_pandoras_box_grid(run: &mut RunState) {
    let starter_count = run
        .deck
        .iter()
        .filter(|card| is_pandoras_box_removed_starter(card.content_id))
        .count();
    if starter_count == 0 {
        return;
    }

    run.deck
        .retain(|card| !is_pandoras_box_removed_starter(card.content_id));
    let pool = ironclad_truly_random_card_pool();
    let mut rng = run.card_random_rng();
    let next_card_id = run.next_card_instance_id();
    let cards = (0..starter_count)
        .map(|index| {
            let pick = rng.random_int((pool.len() - 1) as i32) as usize;
            let content_id = run.content_id_after_card_add_relics(pool[pick]);
            CardInstance::new(
                crate::ids::CardId::new(next_card_id + index as u64),
                content_id,
            )
        })
        .collect();
    run.card_random_rng_counter = rng.counter();
    run.card_grid = Some(CardGridScreen {
        cards,
        purpose: GridPurpose::PandorasBox,
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_astrolabe_grid(run: &mut RunState) {
    let cards = run.deck.clone();
    if cards.is_empty() {
        return;
    }
    if cards.len() <= ASTROLABE_TRANSFORM_COUNT {
        transform_astrolabe_cards(run, &cards);
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards,
        purpose: GridPurpose::Astrolabe,
        selected: None,
        selected_indices: Vec::new(),
    });
}

const ASTROLABE_TRANSFORM_COUNT: usize = 3;

pub fn select_grid_card(run: &RunState, index: usize) -> SimResult<RunState> {
    let grid = run
        .card_grid
        .as_ref()
        .ok_or(SimError::IllegalAction("no card grid is open"))?;
    if index >= grid.cards.len() {
        return Err(SimError::IllegalAction("grid index out of range"));
    }

    if grid.purpose == GridPurpose::Astrolabe {
        let mut next = run.clone();
        let selected_count = {
            let grid = next.card_grid.as_mut().expect("grid present");
            if grid.selected_indices.contains(&index) {
                return Ok(next);
            }
            grid.selected_indices.push(index);
            grid.selected_indices.len()
        };
        if selected_count >= ASTROLABE_TRANSFORM_COUNT {
            confirm_astrolabe_grid(&mut next)?;
        }
        return Ok(next);
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

    let mut next = run.clone();
    match grid.purpose {
        GridPurpose::CallingBellCurse => {
            let card = grid
                .cards
                .first()
                .copied()
                .ok_or(SimError::InvalidState("calling bell grid is empty"))?;
            next.card_grid = None;
            next.add_deck_card(card);
            super::reward::enter_calling_bell_reward_screen(&mut next);
        }
        GridPurpose::PandorasBox => {
            for card in &grid.cards {
                next.add_deck_card(*card);
            }
            next.card_grid = None;
        }
        GridPurpose::Astrolabe => {
            confirm_astrolabe_grid(&mut next)?;
        }
        GridPurpose::RestSmith => {
            let card = selected_grid_card(grid)?;
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
            let card = selected_grid_card(grid)?;
            let shop = next
                .shop
                .as_ref()
                .ok_or(SimError::InvalidState("shop screen is missing"))?;
            let cost = shop.remove_cost;
            if next.gold < cost {
                return Err(SimError::IllegalAction("not enough gold"));
            }
            next.gold -= cost;
            next.break_maw_bank_on_shop_spend();
            next.shop_remove_count += 1;
            next.deck.retain(|deck_card| deck_card.id != card.id);
            let remove_cost = super::shop::shop_remove_cost_for_run(&next);
            if let Some(shop) = next.shop.as_mut() {
                shop.remove_cost = remove_cost;
            }
            next.card_grid = None;
        }
        GridPurpose::EmptyCage { remaining } => {
            let card = selected_grid_card(grid)?;
            next.deck.retain(|deck_card| deck_card.id != card.id);
            if remaining > 1 && !next.deck.is_empty() {
                next.card_grid = Some(CardGridScreen {
                    cards: next.deck.clone(),
                    purpose: GridPurpose::EmptyCage {
                        remaining: remaining - 1,
                    },
                    selected: None,
                    selected_indices: Vec::new(),
                });
            } else {
                next.card_grid = None;
            }
        }
        GridPurpose::Bottle { .. } => {
            let card = selected_grid_card(grid)?;
            for deck_card in &mut next.deck {
                if deck_card.id == card.id {
                    deck_card.bottled = true;
                    break;
                }
            }
            next.card_grid = None;
        }
        GridPurpose::DollysMirror => {
            let card = selected_grid_card(grid)?;
            let mut copy = card;
            copy.id = crate::ids::CardId::new(next.next_card_instance_id());
            copy.bottled = false;
            next.add_deck_card(copy);
            next.card_grid = None;
        }
    }

    Ok(next)
}

fn selected_grid_card(grid: &CardGridScreen) -> SimResult<CardInstance> {
    let selected = grid
        .selected
        .ok_or(SimError::IllegalAction("no card selected in grid"))?;
    grid.cards
        .get(selected)
        .copied()
        .ok_or(SimError::IllegalAction("grid index out of range"))
}

fn confirm_astrolabe_grid(run: &mut RunState) -> SimResult<()> {
    let grid = run
        .card_grid
        .as_ref()
        .ok_or(SimError::IllegalAction("no card grid is open"))?;
    if grid.selected_indices.len() < ASTROLABE_TRANSFORM_COUNT {
        return Err(SimError::IllegalAction(
            "Astrolabe requires three selected cards",
        ));
    }
    let cards = grid
        .selected_indices
        .iter()
        .take(ASTROLABE_TRANSFORM_COUNT)
        .map(|index| {
            grid.cards
                .get(*index)
                .copied()
                .ok_or(SimError::IllegalAction("grid index out of range"))
        })
        .collect::<SimResult<Vec<_>>>()?;
    transform_astrolabe_cards(run, &cards);
    run.card_grid = None;
    Ok(())
}

fn transform_astrolabe_cards(run: &mut RunState, cards: &[CardInstance]) {
    let mut rng = StsRng::with_counter(run.misc_rng_seed as i64, run.misc_rng_counter);
    let next_card_id = run.next_card_instance_id();
    let transformed = cards
        .iter()
        .enumerate()
        .map(|(index, card)| {
            let content_id = transform_card_content_id(card.content_id, &mut rng);
            CardInstance::new(
                crate::ids::CardId::new(next_card_id + index as u64),
                run.content_id_after_card_add_relics(content_id),
            )
        })
        .collect::<Vec<_>>();
    run.misc_rng_counter = rng.counter();

    for card in cards {
        run.deck.retain(|deck_card| deck_card.id != card.id);
    }
    for card in transformed {
        run.add_deck_card(card);
    }
}

fn transform_card_content_id(source: crate::ContentId, rng: &mut StsRng) -> crate::ContentId {
    let pool = ironclad_reward_content_ids()
        .into_iter()
        .filter(|content_id| *content_id != source)
        .collect::<Vec<_>>();
    let pick = rng.random_int((pool.len() - 1) as i32) as usize;
    upgrade_content_id(pool[pick]).unwrap_or(pool[pick])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        content::cards::{ANGER_ID, FEEL_NO_PAIN_ID, STRIKE_R_PLUS_ID},
        RunState,
    };

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

    #[test]
    fn shop_remove_grid_breaks_maw_bank() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Shop;
        run.gold = 100;
        run.relics.push(crate::Relic::MawBank);
        run.shop = Some(super::super::shop::fixed_shop_screen(
            run.next_card_instance_id(),
        ));
        open_shop_remove_grid(&mut run);

        let selected = select_grid_card(&run, 0).expect("select");
        let after = confirm_grid(&selected).expect("confirm");

        assert!(after.maw_bank_broken);
    }

    #[test]
    fn empty_cage_grid_removes_two_selected_cards() {
        let mut run = RunState::map_fixture();
        open_empty_cage_grid(&mut run);
        let first_id = run.deck[0].id;
        let second_id = run.deck[1].id;
        let deck_len = run.deck.len();

        let selected = select_grid_card(&run, 0).expect("select first");
        let after_first = confirm_grid(&selected).expect("confirm first");

        assert!(!after_first.deck.iter().any(|card| card.id == first_id));
        assert_eq!(after_first.deck.len(), deck_len - 1);
        assert_eq!(
            after_first.card_grid.as_ref().map(|grid| grid.purpose),
            Some(GridPurpose::EmptyCage { remaining: 1 })
        );

        let selected = select_grid_card(&after_first, 0).expect("select second");
        let after_second = confirm_grid(&selected).expect("confirm second");

        assert!(!after_second.deck.iter().any(|card| card.id == second_id));
        assert_eq!(after_second.deck.len(), deck_len - 2);
        assert!(after_second.card_grid.is_none());
    }

    #[test]
    fn bottle_grid_filters_by_type_and_marks_selected_card() {
        let mut run = RunState::map_fixture();
        run.gain_deck_card(ANGER_ID);
        run.gain_deck_card(FEEL_NO_PAIN_ID);
        open_bottle_grid(&mut run, CardType::Power);

        let grid = run.card_grid.as_ref().expect("bottle grid");
        assert_eq!(grid.cards.len(), 1);
        assert_eq!(grid.cards[0].content_id, FEEL_NO_PAIN_ID);

        let selected = select_grid_card(&run, 0).expect("select");
        let after = confirm_grid(&selected).expect("confirm");

        assert!(after.card_grid.is_none());
        assert!(after
            .deck
            .iter()
            .any(|card| card.content_id == FEEL_NO_PAIN_ID && card.bottled));
        assert!(!after
            .deck
            .iter()
            .any(|card| card.content_id == ANGER_ID && card.bottled));
    }

    #[test]
    fn dollys_mirror_grid_duplicates_selected_card_as_new_instance() {
        let mut run = RunState::map_fixture();
        let source_id = run.deck[0].id;
        run.deck[0].bottled = true;
        open_dollys_mirror_grid(&mut run);
        let deck_len = run.deck.len();

        let selected = select_grid_card(&run, 0).expect("select");
        let after = confirm_grid(&selected).expect("confirm");

        assert!(after.card_grid.is_none());
        assert_eq!(after.deck.len(), deck_len + 1);
        assert_eq!(after.deck[0].id, source_id);
        let copy = after.deck.last().expect("copy");
        assert_ne!(copy.id, source_id);
        assert_eq!(copy.content_id, after.deck[0].content_id);
        assert!(!copy.bottled);
    }

    #[test]
    fn calling_bell_grid_confirms_curse_without_selection_and_opens_relic_rewards() {
        let mut run = RunState::map_fixture();
        run.reward_rng_seed = 1_218_623;
        run.gain_relic(crate::Relic::CallingBell);

        let grid = run.card_grid.as_ref().expect("calling bell grid");
        assert_eq!(grid.purpose, GridPurpose::CallingBellCurse);
        assert_eq!(grid.cards[0].content_id, CURSE_OF_THE_BELL_ID);

        let after = confirm_grid(&run).expect("confirm bell curse");

        assert!(after.card_grid.is_none());
        assert!(after
            .deck
            .iter()
            .any(|card| card.content_id == CURSE_OF_THE_BELL_ID));
        let reward = after.reward.as_ref().expect("calling bell rewards");
        assert!(reward.relic_offer.is_some() || reward.relic_key_offer.is_some());
        assert_eq!(reward.queued_relic_key_offers.len(), 2);
    }

    #[test]
    fn pandoras_box_grid_replaces_starter_strikes_and_defends_with_random_cards() {
        let mut run = RunState::map_fixture();
        run.reward_rng_seed = 1_218_623;
        let expected_removed = run
            .deck
            .iter()
            .filter(|card| is_pandoras_box_removed_starter(card.content_id))
            .count();

        run.gain_relic(crate::Relic::PandorasBox);

        assert_eq!(expected_removed, 9);
        assert!(run.relics.contains(&crate::Relic::PandorasBox));
        assert!(!run
            .deck
            .iter()
            .any(|card| is_pandoras_box_removed_starter(card.content_id)));
        let grid = run.card_grid.as_ref().expect("pandora grid");
        assert_eq!(grid.purpose, GridPurpose::PandorasBox);
        assert_eq!(grid.cards.len(), expected_removed);
        assert_eq!(run.card_random_rng_counter, expected_removed as u32);

        let after = confirm_grid(&run).expect("confirm pandora");

        assert!(after.card_grid.is_none());
        assert_eq!(after.deck.len(), 1 + expected_removed);
        assert!(after
            .deck
            .iter()
            .any(|card| card.content_id == crate::content::cards::BASH_ID));
        assert!(!after
            .deck
            .iter()
            .any(|card| is_pandoras_box_removed_starter(card.content_id)));
    }

    #[test]
    fn astrolabe_selects_three_cards_then_transforms_and_upgrades_them() {
        let mut run = RunState::map_fixture();
        run.misc_rng_seed = 1_218_623;
        let removed = [run.deck[0], run.deck[1], run.deck[2]];

        run.gain_relic(crate::Relic::Astrolabe);

        assert!(run.relics.contains(&crate::Relic::Astrolabe));
        assert_eq!(
            run.card_grid.as_ref().expect("astrolabe grid").purpose,
            GridPurpose::Astrolabe
        );
        let after_first = select_grid_card(&run, 0).expect("select first");
        assert!(after_first.card_grid.is_some());
        assert_eq!(after_first.deck, run.deck);

        let after_second = select_grid_card(&after_first, 1).expect("select second");
        assert!(after_second.card_grid.is_some());

        let after_third = select_grid_card(&after_second, 2).expect("select third");

        assert!(after_third.card_grid.is_none());
        for card in removed {
            assert!(!after_third
                .deck
                .iter()
                .any(|deck_card| deck_card.id == card.id));
        }
        assert_eq!(after_third.deck.len(), run.deck.len());
        assert_eq!(after_third.misc_rng_counter, 3);
    }

    #[test]
    fn astrolabe_with_three_or_fewer_cards_transforms_without_grid() {
        let mut run = RunState::map_fixture();
        run.deck.truncate(3);
        let old_ids = run.deck.iter().map(|card| card.id).collect::<Vec<_>>();
        run.misc_rng_seed = 1_218_623;

        run.gain_relic(crate::Relic::Astrolabe);

        assert!(run.card_grid.is_none());
        assert_eq!(run.deck.len(), 3);
        assert!(old_ids
            .iter()
            .all(|id| !run.deck.iter().any(|card| card.id == *id)));
        assert_eq!(run.misc_rng_counter, 3);
    }
}
