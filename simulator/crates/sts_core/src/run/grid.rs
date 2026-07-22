use super::event::{Event, EventChoice, EventScreen};
use crate::{
    card::{CardInstance, CardType},
    content::{
        cards::{
            card_instance_is_upgradeable, get_card_definition, is_pandoras_box_removed_starter,
            upgrade_card_instance, upgrade_content_id, CURSE_OF_THE_BELL_ID,
        },
        reward_pool::{ironclad_transform_card_content_id, ironclad_truly_random_card_pool},
    },
    rng::StsRng,
    RunPhase, RunState, SimError, SimResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GridPurpose {
    RestSmith,
    RestRemove,
    ShopRemove,
    EventRemove,
    EventRemoveReturnToEvent { event: Event },
    EventObtainCard,
    EventObtainCardReturnToEvent { event: Event },
    EventUpgrade,
    EventUpgradeReturnToEvent { event: Event },
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
    EventTransformReturnToEvent { event: Event, count: u8 },
    BonfireElementals,
    DesignerRemoveAndUpgrade,
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
        .filter(card_instance_is_upgradeable)
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

pub fn open_rest_remove_grid(run: &mut RunState) {
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
        purpose: GridPurpose::RestRemove,
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_shop_remove_grid(run: &mut RunState) {
    let cards = run
        .deck
        .iter()
        .copied()
        .filter(|card| !card.bottled && card.content_id != CURSE_OF_THE_BELL_ID)
        .collect::<Vec<_>>();

    run.card_grid = Some(CardGridScreen {
        cards,
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

pub fn open_bonfire_elementals_grid(run: &mut RunState) {
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
        purpose: GridPurpose::BonfireElementals,
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_designer_remove_and_upgrade_grid(run: &mut RunState) {
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
        purpose: GridPurpose::DesignerRemoveAndUpgrade,
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

pub fn open_falling_card_grid(run: &mut RunState, card_type: CardType) {
    let cards = run
        .deck
        .iter()
        .copied()
        .filter(|card| {
            !card.bottled
                && get_card_definition(card.content_id)
                    .is_some_and(|definition| definition.card_type == card_type)
        })
        .collect::<Vec<_>>();
    if cards.is_empty() {
        return;
    }

    let mut misc_rng = run.rng_for_stream(crate::run::state::RunRngStream::Misc);
    let selected = cards[misc_rng.random_int((cards.len() - 1) as i32) as usize];
    run.store_rng_counter(crate::run::state::RunRngStream::Misc, &misc_rng);
    run.card_grid = Some(CardGridScreen {
        cards: vec![selected],
        purpose: GridPurpose::EventRemoveReturnToEvent {
            event: Event::Falling,
        },
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

pub fn open_event_obtain_card_return_to_event_grid(
    run: &mut RunState,
    event: Event,
    cards: Vec<CardInstance>,
) {
    if cards.is_empty() {
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards,
        purpose: GridPurpose::EventObtainCardReturnToEvent { event },
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

pub fn open_event_transform_return_to_event_grid(run: &mut RunState, event: Event, count: u8) {
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
        purpose: GridPurpose::EventTransformReturnToEvent { event, count },
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_event_upgrade_grid(run: &mut RunState) {
    let cards = run
        .deck
        .iter()
        .copied()
        .filter(card_instance_is_upgradeable)
        .collect::<Vec<_>>();
    if cards.is_empty() {
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards,
        purpose: GridPurpose::EventUpgrade,
        selected: None,
        selected_indices: Vec::new(),
    });
}

pub fn open_event_upgrade_return_to_event_grid(run: &mut RunState, event: Event) {
    let cards = run
        .deck
        .iter()
        .copied()
        .filter(card_instance_is_upgradeable)
        .collect::<Vec<_>>();
    if cards.is_empty() {
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards,
        purpose: GridPurpose::EventUpgradeReturnToEvent { event },
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
        .filter(card_instance_is_upgradeable)
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

pub fn open_calling_bell_grid(run: &mut RunState) -> SimResult<()> {
    let next_card_id = run.next_card_instance_id()?;
    run.card_grid = Some(CardGridScreen {
        cards: vec![CardInstance::new(
            crate::ids::CardId::new(next_card_id),
            CURSE_OF_THE_BELL_ID,
        )],
        purpose: GridPurpose::CallingBellCurse,
        selected: None,
        selected_indices: Vec::new(),
    });
    Ok(())
}

pub fn open_pandoras_box_grid(run: &mut RunState) -> SimResult<()> {
    let mut next = run.clone();
    open_pandoras_box_grid_inner(&mut next)?;
    *run = next;
    Ok(())
}

fn open_pandoras_box_grid_inner(run: &mut RunState) -> SimResult<()> {
    let starter_count = run
        .deck
        .iter()
        .filter(|card| is_pandoras_box_removed_starter(card.content_id))
        .count();
    if starter_count == 0 {
        return Ok(());
    }

    run.deck
        .retain(|card| !is_pandoras_box_removed_starter(card.content_id));
    let pool = ironclad_truly_random_card_pool();
    let next_card_id = run.reserve_card_instance_ids(starter_count)?;
    let mut rng = run.card_random_rng();
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
    Ok(())
}

pub fn open_astrolabe_grid(run: &mut RunState) -> SimResult<()> {
    let cards = run.deck.clone();
    if cards.is_empty() {
        return Ok(());
    }
    if cards.len() <= ASTROLABE_TRANSFORM_COUNT {
        transform_astrolabe_cards(run, &cards)?;
        return Ok(());
    }

    run.card_grid = Some(CardGridScreen {
        cards,
        purpose: GridPurpose::Astrolabe,
        selected: None,
        selected_indices: Vec::new(),
    });
    Ok(())
}

const ASTROLABE_TRANSFORM_COUNT: usize = 3;

pub(crate) fn validate_grid_select(run: &RunState, index: usize) -> SimResult<()> {
    run.validate()?;
    let grid = run
        .card_grid
        .as_ref()
        .ok_or(SimError::IllegalAction("no card grid is open"))?;
    if index >= grid.cards.len() {
        return Err(SimError::IllegalAction("grid index out of range"));
    }
    Ok(())
}

pub fn select_grid_card(run: &RunState, index: usize) -> SimResult<RunState> {
    validate_grid_select(run, index)?;
    let grid = run.card_grid.as_ref().expect("validated card grid");

    if grid_multi_select_count(grid.purpose).is_some() {
        let mut next = run.clone();
        let grid = next.card_grid.as_mut().expect("grid present");
        if !grid.selected_indices.contains(&index) {
            grid.selected_indices.push(index);
        }
        return Ok(next);
    }

    let mut next = run.clone();
    let grid = next.card_grid.as_mut().expect("grid present");
    grid.selected = Some(index);
    if matches!(
        grid.purpose,
        GridPurpose::Bottle { .. }
            | GridPurpose::EventObtainCard
            | GridPurpose::EventObtainCardReturnToEvent { .. }
    ) {
        return confirm_grid(&next);
    }
    Ok(next)
}

pub(crate) fn validate_grid_cancel(run: &RunState) -> SimResult<()> {
    run.validate()?;
    if run.card_grid.is_none() {
        return Err(SimError::IllegalAction("no card grid is open"));
    }
    Ok(())
}

pub fn cancel_grid(run: &RunState) -> SimResult<RunState> {
    validate_grid_cancel(run)?;
    let mut next = run.clone();
    next.card_grid = None;
    Ok(next)
}

pub(crate) fn validate_grid_confirm(run: &RunState) -> SimResult<()> {
    run.validate()?;
    let grid = run
        .card_grid
        .as_ref()
        .ok_or(SimError::IllegalAction("no card grid is open"))?;

    match grid.purpose {
        GridPurpose::CallingBellCurse => {
            grid.cards
                .first()
                .ok_or(SimError::InvalidState("calling bell grid is empty"))?;
        }
        GridPurpose::PandorasBox => {}
        purpose if grid_multi_select_count(purpose).is_some() => {
            let required = grid_multi_select_count(purpose).expect("matched multi-select purpose");
            if grid.selected_indices.len() < required {
                return Err(SimError::IllegalAction("grid requires more selected cards"));
            }
            for index in grid.selected_indices.iter().take(required) {
                let card = grid
                    .cards
                    .get(*index)
                    .ok_or(SimError::IllegalAction("grid index out of range"))?;
                validate_grid_card_is_in_deck(run, *card)?;
            }
        }
        purpose => {
            let card = selected_grid_card(grid)?;
            if !matches!(
                purpose,
                GridPurpose::EventObtainCard | GridPurpose::EventObtainCardReturnToEvent { .. }
            ) {
                validate_grid_card_is_in_deck(run, card)?;
            }
            if matches!(
                purpose,
                GridPurpose::RestSmith
                    | GridPurpose::NeowUpgrade
                    | GridPurpose::EventUpgrade
                    | GridPurpose::EventUpgradeReturnToEvent { .. }
            ) && upgrade_card_instance(card).is_none()
            {
                return Err(SimError::IllegalAction("card cannot be upgraded"));
            }
            if purpose == GridPurpose::ShopRemove {
                let shop = run
                    .shop
                    .as_ref()
                    .ok_or(SimError::InvalidState("shop screen is missing"))?;
                if run.gold < shop.remove_cost {
                    return Err(SimError::IllegalAction("not enough gold"));
                }
            }
            if purpose == GridPurpose::BonfireElementals
                && !super::event::bonfire_card_is_supported(card.content_id)
            {
                return Err(SimError::UnsupportedMechanic(card.content_id));
            }
        }
    }
    Ok(())
}

pub fn confirm_grid(run: &RunState) -> SimResult<RunState> {
    validate_grid_confirm(run)?;
    let grid = run.card_grid.as_ref().expect("validated card grid");

    let mut next = run.clone();
    match grid.purpose {
        GridPurpose::CallingBellCurse => {
            let card = grid
                .cards
                .first()
                .copied()
                .ok_or(SimError::InvalidState("calling bell grid is empty"))?;
            next.card_grid = None;
            next.add_deck_card(card)?;
            // Calling Bell opens CombatRewardScreen while still in NeowRoom.
            // setupItemReward constructs the room's ordinary card reward first;
            // CallingBell.update then clears it and replaces it with three relics.
            super::reward::consume_hidden_neow_room_card_reward(&mut next)?;
            super::reward::enter_calling_bell_reward_screen(&mut next);
        }
        GridPurpose::PandorasBox => {
            // The target presents Pandora's Box results from the top of the
            // generated group and then appends them to the master deck in that
            // visible order, which is the reverse of our generation vector.
            for card in grid.cards.iter().rev() {
                next.add_deck_card(*card)?;
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
        GridPurpose::EventTransformReturnToEvent { event, count } => {
            confirm_event_transform_grid(&mut next, count)?;
            return_to_event_leave_screen(&mut next, event);
        }
        GridPurpose::RestSmith => {
            let card = selected_grid_card(grid)?;
            upgrade_deck_card(&mut next, card)?;
            next.card_grid = None;
            next.phase = RunPhase::Rest;
            next.rest_room_complete = true;
        }
        GridPurpose::RestRemove => {
            let card = selected_grid_card(grid)?;
            next.remove_deck_card(card.id)
                .expect("rest remove selected a deck card");
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
        GridPurpose::EventUpgrade => {
            let card = selected_grid_card(grid)?;
            upgrade_deck_card(&mut next, card)?;
            next.card_grid = None;
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        GridPurpose::EventUpgradeReturnToEvent { event } => {
            let card = selected_grid_card(grid)?;
            upgrade_deck_card(&mut next, card)?;
            next.card_grid = None;
            return_to_event_leave_screen(&mut next, event);
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
            let next_remove_count = next
                .shop_remove_count
                .checked_add(1)
                .ok_or(SimError::InvalidState("shop remove count overflows u32"))?;
            let remove_cost = super::shop::shop_remove_cost_for_count(&next, next_remove_count)?;
            next.gold -= cost;
            next.break_maw_bank_on_shop_spend();
            next.shop_remove_count = next_remove_count;
            next.remove_deck_card(card.id)
                .expect("shop remove selected a deck card");
            if let Some(shop) = next.shop.as_mut() {
                shop.remove_available = false;
                shop.remove_cost = remove_cost;
            }
            next.card_grid = None;
        }
        GridPurpose::EventRemove => {
            let card = selected_grid_card(grid)?;
            next.remove_deck_card(card.id)
                .expect("event remove selected a deck card");
            next.card_grid = None;
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        GridPurpose::EventRemoveReturnToEvent { event } => {
            let card = selected_grid_card(grid)?;
            next.remove_deck_card(card.id)
                .expect("event remove selected a deck card");
            next.card_grid = None;
            next.phase = RunPhase::Event;
            next.event = Some(EventScreen {
                event,
                choices: vec![EventChoice {
                    label: "Leave".to_owned(),
                }],
                stage: if event == Event::WheelOfChange { 3 } else { 2 },
                event_data: 0,
            });
        }
        GridPurpose::BonfireElementals => {
            let card = selected_grid_card(grid)?;
            super::event::complete_bonfire_elementals_card(&mut next, card)?;
        }
        GridPurpose::DesignerRemoveAndUpgrade => {
            let card = selected_grid_card(grid)?;
            super::event::complete_designer_remove_and_upgrade(&mut next, card)?;
        }
        GridPurpose::EventObtainCard => {
            let card = selected_grid_card(grid)?;
            next.add_deck_card(card)?;
            next.card_grid = None;
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        GridPurpose::EventObtainCardReturnToEvent { event } => {
            let card = selected_grid_card(grid)?;
            next.add_deck_card(card)?;
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
        GridPurpose::EmptyCage { remaining } => {
            let card = selected_grid_card(grid)?;
            remove_grid_card(&mut next, card, GridPurpose::EmptyCage { remaining });
        }
        GridPurpose::NeowRemove { remaining } => {
            if remaining > 1 {
                confirm_neow_remove_grid(&mut next, remaining)?;
            } else {
                let card = selected_grid_card(grid)?;
                remove_grid_card(&mut next, card, GridPurpose::NeowRemove { remaining });
                if next.card_grid.is_none() {
                    finish_neow_grid_reward(&mut next);
                }
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
            if reward_sequence_has_remaining_choices(&next) {
                super::reward::advance_pending_relic_offer(&mut next);
                next.phase = RunPhase::Reward;
            }
        }
        GridPurpose::DollysMirror => {
            let card = selected_grid_card(grid)?;
            let mut copy = card;
            copy.id = crate::ids::CardId::new(next.next_card_instance_id()?);
            copy.bottled = false;
            next.add_deck_card(copy)?;
            next.card_grid = None;
        }
    }

    Ok(next)
}

fn validate_grid_card_is_in_deck(run: &RunState, card: CardInstance) -> SimResult<()> {
    if run.deck.contains(&card) {
        Ok(())
    } else {
        Err(SimError::InvalidState(
            "grid card does not match the run deck",
        ))
    }
}

fn grid_multi_select_count(purpose: GridPurpose) -> Option<usize> {
    match purpose {
        GridPurpose::Astrolabe => Some(ASTROLABE_TRANSFORM_COUNT),
        GridPurpose::NeowRemove { remaining } if remaining > 1 => Some(usize::from(remaining)),
        GridPurpose::NeowTransform { count } => Some(usize::from(count)),
        GridPurpose::EventTransform { count } => Some(usize::from(count)),
        GridPurpose::EventTransformReturnToEvent { count, .. } => Some(usize::from(count)),
        _ => None,
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

fn reward_sequence_has_remaining_choices(run: &RunState) -> bool {
    run.reward.as_ref().is_some_and(|reward| {
        reward.gold_offer > 0
            || reward.stolen_gold_offer > 0
            || reward.potion_offer.is_some()
            || reward.relic_offer.is_some()
            || reward.pending_relic_offer.is_some()
            || !reward.queued_relic_offers.is_empty()
            || !reward.boss_relic_choices.is_empty()
            || reward.remaining_card_reward_count() > 0
            || !reward.choices.is_empty()
    })
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

fn return_to_event_leave_screen(run: &mut RunState, event: Event) {
    run.phase = RunPhase::Event;
    run.event = Some(EventScreen {
        event,
        choices: vec![EventChoice {
            label: "Leave".to_owned(),
        }],
        stage: if event == Event::Designer { 2 } else { 1 },
        event_data: 0,
    });
}

fn remove_grid_card(run: &mut RunState, card: CardInstance, purpose: GridPurpose) {
    let remaining = match purpose {
        GridPurpose::EmptyCage { remaining } | GridPurpose::NeowRemove { remaining } => remaining,
        _ => unreachable!("remove grid purpose required"),
    };
    run.remove_deck_card(card.id)
        .expect("remove grid selected a deck card");
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
    transform_astrolabe_cards(run, &cards)?;
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
    transform_neow_cards(run, &cards)?;
    run.card_grid = None;
    finish_neow_grid_reward(run);
    Ok(())
}

fn confirm_neow_remove_grid(run: &mut RunState, count: u8) -> SimResult<()> {
    let grid = run
        .card_grid
        .as_ref()
        .ok_or(SimError::IllegalAction("no card grid is open"))?;
    let required = usize::from(count);
    if grid.selected_indices.len() < required {
        return Err(SimError::IllegalAction(
            "Neow remove requires more selected cards",
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
        run.remove_deck_card(card.id)
            .expect("Neow remove selected a deck card");
    }
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
    transform_event_cards(run, &cards)?;
    run.card_grid = None;
    run.phase = RunPhase::Idle;
    run.event = None;
    Ok(())
}

fn transform_neow_cards(run: &mut RunState, cards: &[CardInstance]) -> SimResult<()> {
    let next_card_id = run.reserve_card_instance_ids(cards.len())?;
    let sources = cards.iter().map(|card| card.content_id).collect::<Vec<_>>();
    let reward =
        crate::run::neow::generate_neow_transform_reward(run.reward_rng_seed as i64, &sources);
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
        run.remove_deck_card(card.id)
            .expect("transform selected a deck card");
    }
    for card in transformed {
        run.add_deck_card(card)?;
    }
    Ok(())
}

fn transform_astrolabe_cards(run: &mut RunState, cards: &[CardInstance]) -> SimResult<()> {
    let next_card_id = run.reserve_card_instance_ids(cards.len())?;
    let mut rng = StsRng::with_counter(run.misc_rng_seed as i64, run.misc_rng_counter);
    let transformed = cards
        .iter()
        .enumerate()
        .map(|(index, card)| {
            let transformed = transform_card_content_id(card.content_id, &mut rng);
            let content_id = upgrade_content_id(transformed).unwrap_or(transformed);
            CardInstance::new(
                crate::ids::CardId::new(next_card_id + index as u64),
                content_id,
            )
        })
        .collect::<Vec<_>>();
    run.misc_rng_counter = rng.counter();

    for card in cards {
        run.remove_deck_card(card.id)
            .expect("transform selected a deck card");
    }
    for card in transformed {
        run.add_deck_card(card)?;
    }
    Ok(())
}

fn transform_event_cards(run: &mut RunState, cards: &[CardInstance]) -> SimResult<()> {
    let next_card_id = run.reserve_card_instance_ids(cards.len())?;
    let mut rng = StsRng::with_counter(run.misc_rng_seed as i64, run.misc_rng_counter);
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
        run.remove_deck_card(card.id)
            .expect("transform selected a deck card");
    }
    for card in transformed {
        run.add_deck_card(card)?;
    }
    Ok(())
}

fn transform_card_content_id(source: crate::ContentId, rng: &mut StsRng) -> crate::ContentId {
    ironclad_transform_card_content_id(source, rng)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        content::cards::{
            BITE_ID, BITE_PLUS_ID, CURSE_OF_THE_BELL_ID, RITUAL_DAGGER_ID, STRIKE_R_ID,
        },
        run::shop,
        RunState,
    };

    #[test]
    fn rest_smith_grid_includes_unupgraded_ritual_dagger() {
        let mut run = RunState::map_fixture();
        run.gain_deck_card(RITUAL_DAGGER_ID)
            .expect("Ritual Dagger gain succeeds");

        open_rest_smith_grid(&mut run);

        assert!(run
            .card_grid
            .as_ref()
            .expect("rest smith grid")
            .cards
            .iter()
            .any(|card| card.content_id == RITUAL_DAGGER_ID));
    }

    #[test]
    fn rest_smith_grid_includes_bites_and_upgrades_them() {
        let mut run = RunState::map_fixture();
        run.gain_deck_card(BITE_ID).expect("Bite gain succeeds");

        open_rest_smith_grid(&mut run);
        let bite_index = run
            .card_grid
            .as_ref()
            .expect("rest smith grid")
            .cards
            .iter()
            .position(|card| card.content_id == BITE_ID)
            .expect("Bite is upgradeable");
        let selected = select_grid_card(&run, bite_index).expect("Bite can be selected");
        let upgraded = confirm_grid(&selected).expect("Bite upgrade confirms");

        assert!(upgraded
            .deck
            .iter()
            .any(|card| card.content_id == BITE_PLUS_ID));
    }

    #[test]
    fn shop_remove_grid_excludes_curse_of_the_bell() {
        let mut run = RunState::map_fixture();
        run.gain_deck_card(CURSE_OF_THE_BELL_ID)
            .expect("Curse of the Bell gain succeeds");

        open_shop_remove_grid(&mut run);

        assert!(run
            .card_grid
            .as_ref()
            .expect("shop remove grid")
            .cards
            .iter()
            .all(|card| card.content_id != CURSE_OF_THE_BELL_ID));
    }

    #[test]
    fn shop_remove_price_overflow_leaves_grid_run_unchanged() {
        let mut run = RunState::map_fixture();
        run.shop_remove_count = shop::MAX_SHOP_REMOVE_COUNT;
        run.gold = i32::MAX;
        run.phase = RunPhase::Shop;
        let generated_shop = shop::generate_shop_screen(&mut run)
            .expect("maximum supported purge price generates a shop");
        run.shop = Some(generated_shop);
        run.shop_merchant_open = true;
        open_shop_remove_grid(&mut run);
        let selected = select_grid_card(&run, 0).expect("purge card can be selected");
        let before = selected.clone();

        assert_eq!(
            confirm_grid(&selected),
            Err(SimError::InvalidState("shop remove price overflows i32"))
        );
        assert_eq!(selected, before);
    }

    #[test]
    fn neow_remove_two_keeps_full_grid_until_two_cards_are_selected() {
        let mut run = RunState::map_fixture();
        open_neow_remove_grid(&mut run, 2);
        let original_deck = run.deck.clone();

        let first_selected = select_grid_card(&run, 0).expect("first select");
        let grid = first_selected
            .card_grid
            .as_ref()
            .expect("grid remains open");
        assert_eq!(grid.cards.len(), original_deck.len());
        assert_eq!(grid.selected_indices, vec![0]);
        assert_eq!(first_selected.deck, original_deck);
        assert!(confirm_grid(&first_selected).is_err());

        let second_selected = select_grid_card(&first_selected, 1).expect("second select");
        let confirmed = confirm_grid(&second_selected).expect("confirm two removals");

        assert!(confirmed.card_grid.is_none());
        assert_eq!(confirmed.deck.len(), original_deck.len() - 2);
        assert_eq!(
            confirmed
                .deck
                .iter()
                .filter(|card| card.content_id == STRIKE_R_ID)
                .count(),
            3
        );
    }

    #[test]
    fn empty_cage_removes_one_selected_card_per_confirm() {
        let mut run = RunState::map_fixture();
        open_empty_cage_grid(&mut run);
        let original_deck = run.deck.clone();

        let first_selected = select_grid_card(&run, 0).expect("first select");
        let first_confirmed = confirm_grid(&first_selected).expect("confirm first removal");
        assert_eq!(first_confirmed.deck.len(), original_deck.len() - 1);
        let second_selected = select_grid_card(&first_confirmed, 0).expect("second select");
        let second_confirmed = confirm_grid(&second_selected).expect("confirm second removal");

        assert!(second_confirmed.card_grid.is_none());
        assert_eq!(second_confirmed.deck.len(), original_deck.len() - 2);
        assert_eq!(
            second_confirmed
                .deck
                .iter()
                .filter(|card| card.content_id == STRIKE_R_ID)
                .count(),
            3
        );
    }

    #[test]
    fn only_astrolabe_upgrades_transformed_cards() {
        let run = RunState::map_fixture();
        let sources = run.deck.iter().copied().take(3).collect::<Vec<_>>();
        let mut event_run = run.clone();
        let mut astrolabe_run = run;

        transform_event_cards(&mut event_run, &sources).expect("event transforms allocate cards");
        transform_astrolabe_cards(&mut astrolabe_run, &sources)
            .expect("Astrolabe transforms allocate cards");

        let event_results = &event_run.deck[event_run.deck.len() - sources.len()..];
        let astrolabe_results = &astrolabe_run.deck[astrolabe_run.deck.len() - sources.len()..];
        assert_eq!(event_results.len(), astrolabe_results.len());
        for (event_card, astrolabe_card) in event_results.iter().zip(astrolabe_results) {
            assert_eq!(
                upgrade_content_id(event_card.content_id),
                Some(astrolabe_card.content_id)
            );
        }
    }
}
