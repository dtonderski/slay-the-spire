use super::event::{Event, EventChoice, EventScreen};
use crate::{
    card::{CardInstance, CardType},
    content::{
        cards::{
            get_card_definition, is_pandoras_box_removed_starter, upgrade_card_instance,
            upgrade_content_id, CURSE_OF_THE_BELL_ID,
        },
        reward_pool::{ironclad_transform_card_content_id, ironclad_truly_random_card_pool},
    },
    rng::StsRng,
    RunPhase, RunState, SimError, SimResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GridPurpose {
    RestSmith,
    ShopRemove,
    EventRemove,
    EventRemoveReturnToEvent { event: Event },
    EventObtainCard,
    EmptyCage { remaining: u8 },
    NeowRemove { remaining: u8 },
    NeowUpgrade,
    Bottle { card_type: CardType },
    DollysMirror,
    CallingBellCurse,
    PandorasBox,
    Astrolabe,
    NeowTransform { count: u8 },
    EventTransform { count: u8 },
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
    let cards = run
        .deck
        .iter()
        .copied()
        .filter(|card| upgrade_content_id(card.content_id).is_some())
        .collect::<Vec<_>>();
    if cards.is_empty() {
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards,
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

pub fn open_event_remove_grid(run: &mut RunState) {
    let cards = run
        .deck
        .iter()
        .copied()
        .filter(|card| !card.bottled)
        .collect::<Vec<_>>();
    if cards.is_empty() {
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards,
        purpose: GridPurpose::EventRemove,
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_event_remove_return_to_event_grid(run: &mut RunState, event: Event) {
    let cards = run
        .deck
        .iter()
        .copied()
        .filter(|card| !card.bottled)
        .collect::<Vec<_>>();
    if cards.is_empty() {
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards,
        purpose: GridPurpose::EventRemoveReturnToEvent { event },
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_event_obtain_card_grid(run: &mut RunState, cards: Vec<CardInstance>) {
    if cards.is_empty() {
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards,
        purpose: GridPurpose::EventObtainCard,
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_event_transform_grid(run: &mut RunState, count: u8) {
    let cards = run
        .deck
        .iter()
        .copied()
        .filter(|card| !card.bottled)
        .collect::<Vec<_>>();
    if cards.is_empty() || count == 0 {
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards,
        purpose: GridPurpose::EventTransform { count },
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

pub fn open_neow_remove_grid(run: &mut RunState, count: u8) {
    if run.deck.is_empty() || count == 0 {
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards: run.deck.clone(),
        purpose: GridPurpose::NeowRemove { remaining: count },
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_neow_upgrade_grid(run: &mut RunState) {
    let cards = run
        .deck
        .iter()
        .copied()
        .filter(|card| upgrade_content_id(card.content_id).is_some())
        .collect::<Vec<_>>();
    if cards.is_empty() {
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards,
        purpose: GridPurpose::NeowUpgrade,
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_neow_transform_grid(run: &mut RunState, count: u8) {
    if run.deck.is_empty() || count == 0 {
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards: run.deck.clone(),
        purpose: GridPurpose::NeowTransform { count },
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_bottle_grid(run: &mut RunState, card_type: CardType) {
    let mut cards = run
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
    // STS uses CardGroup.getCardsOfType() for bottle relics, which appends with
    // addToBottom and therefore reverses the filtered master-deck order.
    cards.reverse();
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

    if let Some(required_count) = grid_multi_select_count(grid.purpose) {
        let mut next = run.clone();
        let selected_count = {
            let grid = next.card_grid.as_mut().expect("grid present");
            if grid.selected_indices.contains(&index) {
                return Ok(next);
            }
            grid.selected_indices.push(index);
            grid.selected_indices.len()
        };
        if selected_count >= required_count {
            confirm_multi_select_grid(&mut next)?;
        }
        return Ok(next);
    }

    let mut next = run.clone();
    let grid = next.card_grid.as_mut().expect("grid present");
    grid.selected = Some(index);
    if matches!(grid.purpose, GridPurpose::Bottle { .. }) {
        return confirm_grid(&next);
    }
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
        GridPurpose::NeowTransform { count } => {
            confirm_neow_transform_grid(&mut next, count)?;
        }
        GridPurpose::EventTransform { count } => {
            confirm_event_transform_grid(&mut next, count)?;
        }
        GridPurpose::RestSmith => {
            let card = selected_grid_card(grid)?;
            upgrade_deck_card(&mut next, card)?;
            next.card_grid = None;
            next.phase = RunPhase::Rest;
            next.rest_room_complete = true;
        }
        GridPurpose::NeowUpgrade => {
            let card = selected_grid_card(grid)?;
            upgrade_deck_card(&mut next, card)?;
            next.card_grid = None;
            finish_neow_grid_reward(&mut next);
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
                shop.remove_available = false;
                shop.remove_cost = remove_cost;
            }
            next.card_grid = None;
        }
        GridPurpose::EventRemove => {
            let card = selected_grid_card(grid)?;
            next.deck.retain(|deck_card| deck_card.id != card.id);
            next.card_grid = None;
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        GridPurpose::EventRemoveReturnToEvent { event } => {
            let card = selected_grid_card(grid)?;
            next.deck.retain(|deck_card| deck_card.id != card.id);
            next.card_grid = None;
            next.phase = RunPhase::Event;
            next.event = Some(EventScreen {
                event,
                choices: vec![EventChoice {
                    label: "Leave".to_owned(),
                }],
                stage: 2,
                event_data: 0,
            });
        }
        GridPurpose::EventObtainCard => {
            let card = selected_grid_card(grid)?;
            next.add_deck_card(card);
            next.card_grid = None;
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        GridPurpose::EmptyCage { remaining } => {
            let card = selected_grid_card(grid)?;
            remove_grid_card(&mut next, card, GridPurpose::EmptyCage { remaining });
        }
        GridPurpose::NeowRemove { remaining } => {
            let card = selected_grid_card(grid)?;
            remove_grid_card(&mut next, card, GridPurpose::NeowRemove { remaining });
            if next.card_grid.is_none() {
                finish_neow_grid_reward(&mut next);
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

fn grid_multi_select_count(purpose: GridPurpose) -> Option<usize> {
    match purpose {
        GridPurpose::Astrolabe => Some(ASTROLABE_TRANSFORM_COUNT),
        GridPurpose::NeowRemove { remaining } if remaining > 1 => Some(usize::from(remaining)),
        GridPurpose::NeowTransform { count } => Some(usize::from(count)),
        GridPurpose::EventTransform { count } => Some(usize::from(count)),
        _ => None,
    }
}

fn confirm_multi_select_grid(run: &mut RunState) -> SimResult<()> {
    let purpose = run
        .card_grid
        .as_ref()
        .ok_or(SimError::IllegalAction("no card grid is open"))?
        .purpose;
    match purpose {
        GridPurpose::Astrolabe => confirm_astrolabe_grid(run),
        GridPurpose::NeowRemove { remaining } if remaining > 1 => {
            confirm_multi_remove_grid(run, purpose)
        }
        GridPurpose::NeowTransform { count } => confirm_neow_transform_grid(run, count),
        GridPurpose::EventTransform { count } => confirm_event_transform_grid(run, count),
        _ => Err(SimError::IllegalAction("grid is not multi-select")),
    }
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

fn upgrade_deck_card(run: &mut RunState, card: CardInstance) -> SimResult<()> {
    let upgraded =
        upgrade_card_instance(card).ok_or(SimError::IllegalAction("card cannot be upgraded"))?;
    for deck_card in &mut run.deck {
        if deck_card.id == card.id {
            *deck_card = upgraded;
            break;
        }
    }
    Ok(())
}

fn remove_grid_card(run: &mut RunState, card: CardInstance, purpose: GridPurpose) {
    let remaining = match purpose {
        GridPurpose::EmptyCage { remaining } | GridPurpose::NeowRemove { remaining } => remaining,
        _ => unreachable!("remove grid purpose required"),
    };
    run.deck.retain(|deck_card| deck_card.id != card.id);
    if remaining > 1 && !run.deck.is_empty() {
        run.card_grid = Some(CardGridScreen {
            cards: run.deck.clone(),
            purpose: match purpose {
                GridPurpose::EmptyCage { .. } => GridPurpose::EmptyCage {
                    remaining: remaining - 1,
                },
                GridPurpose::NeowRemove { .. } => GridPurpose::NeowRemove {
                    remaining: remaining - 1,
                },
                _ => unreachable!("remove grid purpose required"),
            },
            selected: None,
            selected_indices: Vec::new(),
        });
    } else {
        run.card_grid = None;
    }
}

fn confirm_multi_remove_grid(run: &mut RunState, purpose: GridPurpose) -> SimResult<()> {
    let grid = run
        .card_grid
        .as_ref()
        .ok_or(SimError::IllegalAction("no card grid is open"))?;
    let required = match purpose {
        GridPurpose::EmptyCage { remaining } | GridPurpose::NeowRemove { remaining } => {
            usize::from(remaining)
        }
        _ => unreachable!("remove grid purpose required"),
    };
    if grid.selected_indices.len() < required {
        return Err(SimError::IllegalAction(
            "remove grid requires more selected cards",
        ));
    }
    let cards = grid
        .selected_indices
        .iter()
        .take(required)
        .map(|index| {
            grid.cards
                .get(*index)
                .copied()
                .ok_or(SimError::IllegalAction("grid index out of range"))
        })
        .collect::<SimResult<Vec<_>>>()?;

    for card in cards {
        run.deck.retain(|deck_card| deck_card.id != card.id);
    }
    run.card_grid = None;
    if matches!(purpose, GridPurpose::NeowRemove { .. }) {
        finish_neow_grid_reward(run);
    }
    Ok(())
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

fn confirm_neow_transform_grid(run: &mut RunState, count: u8) -> SimResult<()> {
    let grid = run
        .card_grid
        .as_ref()
        .ok_or(SimError::IllegalAction("no card grid is open"))?;
    let required = usize::from(count);
    if grid.selected_indices.len() < required {
        return Err(SimError::IllegalAction(
            "Neow transform requires more selected cards",
        ));
    }
    let cards = grid
        .selected_indices
        .iter()
        .take(required)
        .map(|index| {
            grid.cards
                .get(*index)
                .copied()
                .ok_or(SimError::IllegalAction("grid index out of range"))
        })
        .collect::<SimResult<Vec<_>>>()?;
    transform_neow_cards(run, &cards);
    run.card_grid = None;
    finish_neow_grid_reward(run);
    Ok(())
}

fn finish_neow_grid_reward(run: &mut RunState) {
    run.phase = RunPhase::Event;
    run.event = Some(super::event::neow_screen_for_stage(run, 2));
}

fn confirm_event_transform_grid(run: &mut RunState, count: u8) -> SimResult<()> {
    let grid = run
        .card_grid
        .as_ref()
        .ok_or(SimError::IllegalAction("no card grid is open"))?;
    let required = usize::from(count);
    if grid.selected_indices.len() < required {
        return Err(SimError::IllegalAction(
            "event transform requires more selected cards",
        ));
    }
    let cards = grid
        .selected_indices
        .iter()
        .take(required)
        .map(|index| {
            grid.cards
                .get(*index)
                .copied()
                .ok_or(SimError::IllegalAction("grid index out of range"))
        })
        .collect::<SimResult<Vec<_>>>()?;
    transform_event_cards(run, &cards);
    run.card_grid = None;
    run.phase = RunPhase::Idle;
    run.event = None;
    Ok(())
}

fn transform_neow_cards(run: &mut RunState, cards: &[CardInstance]) {
    let sources = cards.iter().map(|card| card.content_id).collect::<Vec<_>>();
    let reward =
        crate::run::neow::generate_neow_transform_reward(run.reward_rng_seed as i64, &sources);
    let next_card_id = run.next_card_instance_id();
    let transformed = reward
        .cards
        .into_iter()
        .enumerate()
        .map(|(index, content_id)| {
            CardInstance::new(
                crate::ids::CardId::new(next_card_id + index as u64),
                run.content_id_after_card_add_relics(content_id),
            )
        })
        .collect::<Vec<_>>();

    for card in cards {
        run.deck.retain(|deck_card| deck_card.id != card.id);
    }
    for card in transformed {
        run.add_deck_card(card);
    }
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

fn transform_event_cards(run: &mut RunState, cards: &[CardInstance]) {
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
    let content_id = ironclad_transform_card_content_id(source, rng);
    upgrade_content_id(content_id).unwrap_or(content_id)
}
