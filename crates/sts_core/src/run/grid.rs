use super::{
    event::{Event, EventChoice, EventScreen},
    state::{PendingAstrolabeTransform, PendingEventTransform, RunRngStream},
};
use crate::{
    card::{CardInstance, CardType},
    content::{
        cards::{
            card_instance_is_upgradeable, get_card_definition, is_curse_content_id,
            is_pandoras_box_removed_starter, is_purgeable_card, is_purgeable_card_content,
            upgrade_card_instance, CURSE_OF_THE_BELL_ID,
        },
        reward_pool::{
            ironclad_transform_card_content_id, ironclad_truly_random_card_pool,
            IRONCLAD_REWARD_ENTRIES,
        },
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

pub(super) fn event_grid_has_authoritative_owner(run: &RunState, purpose: GridPurpose) -> bool {
    let Some(screen) = run.event.as_ref().filter(|_| run.phase == RunPhase::Event) else {
        return false;
    };

    match purpose {
        GridPurpose::EventRemove => matches!((screen.event, screen.stage), (Event::Beggar, 2)),
        GridPurpose::EventObtainCard
        | GridPurpose::EventUpgrade
        | GridPurpose::EventTransform { .. } => false,
        GridPurpose::EventRemoveReturnToEvent { event } => {
            event == screen.event
                && match (screen.event, screen.stage) {
                    (Event::WingStatue, 2) => screen.event_data == 0,
                    (
                        Event::TheCleric
                        | Event::LivingWall
                        | Event::Purifier
                        | Event::BackToBasics,
                        0,
                    )
                    | (Event::Designer | Event::NoteForYourself | Event::Falling, 1) => true,
                    (Event::WheelOfChange, 2) => screen.event_data == 4,
                    _ => false,
                }
        }
        GridPurpose::EventObtainCardReturnToEvent { event } => {
            event == screen.event
                && matches!(
                    (screen.event, screen.stage),
                    (Event::TheLibrary | Event::Duplicator, 0)
                )
        }
        GridPurpose::EventUpgradeReturnToEvent { event } => {
            event == screen.event
                && matches!(
                    (screen.event, screen.stage),
                    (Event::Designer, 1)
                        | (
                            Event::AccursedBlacksmith | Event::UpgradeShrine | Event::LivingWall,
                            0
                        )
                )
        }
        GridPurpose::EventTransformReturnToEvent { event, count } => {
            event == screen.event
                && matches!(
                    (screen.event, screen.stage, count),
                    (Event::Designer, 1, 2)
                        | (Event::Transmorgrifier | Event::LivingWall, 0, 1)
                        | (Event::DrugDealer, 1, 2)
                )
        }
        GridPurpose::BonfireElementals => {
            screen.event == Event::BonfireElementals && screen.stage == 1
        }
        GridPurpose::DesignerRemoveAndUpgrade => {
            screen.event == Event::Designer && screen.stage == 1
        }
        GridPurpose::RestSmith
        | GridPurpose::RestRemove
        | GridPurpose::ShopRemove
        | GridPurpose::EmptyCage { .. }
        | GridPurpose::NeowRemove { .. }
        | GridPurpose::NeowUpgrade
        | GridPurpose::Bottle { .. }
        | GridPurpose::DollysMirror
        | GridPurpose::CallingBellCurse
        | GridPurpose::PandorasBox
        | GridPurpose::Astrolabe
        | GridPurpose::NeowTransform { .. } => false,
    }
}

pub(super) fn validate_grid_payload_authority(
    run: &RunState,
    grid: &CardGridScreen,
) -> SimResult<()> {
    match grid.purpose {
        GridPurpose::EventObtainCardReturnToEvent {
            event: Event::TheLibrary,
        } => validate_library_grid_payload(run, grid),
        GridPurpose::EventObtainCardReturnToEvent {
            event: Event::Duplicator,
        } => validate_duplicator_grid_payload(run, grid),
        GridPurpose::CallingBellCurse => validate_calling_bell_grid_payload(run, grid),
        GridPurpose::PandorasBox => validate_pandoras_box_grid_payload(run, grid),
        _ => validate_deck_derived_grid_payload(run, grid),
    }
}

fn validate_calling_bell_grid_payload(run: &RunState, grid: &CardGridScreen) -> SimResult<()> {
    let expected = CardInstance::new(
        crate::ids::CardId::new(run.next_card_instance_id()?),
        CURSE_OF_THE_BELL_ID,
    );
    if grid.cards.as_slice() != [expected]
        || grid.selected.is_some()
        || !grid.selected_indices.is_empty()
    {
        return Err(SimError::InvalidState(
            "Calling Bell grid does not match generated curse authority",
        ));
    }
    Ok(())
}

fn validate_pandoras_box_grid_payload(run: &RunState, grid: &CardGridScreen) -> SimResult<()> {
    let draw_count = u32::try_from(grid.cards.len())
        .map_err(|_| SimError::InvalidState("Pandora's Box grid is too large"))?;
    let stream = run.rng_stream_state(RunRngStream::CardRandom);
    let prior_counter = stream
        .counter
        .checked_sub(draw_count)
        .ok_or(SimError::InvalidState(
            "Pandora's Box grid has no preceding card RNG draws",
        ))?;
    let first_id = run.reserve_card_instance_ids(grid.cards.len())?;
    let pool = ironclad_truly_random_card_pool();
    let mut rng = StsRng::with_counter(stream.seed as i64, prior_counter);
    let expected = (0..grid.cards.len())
        .map(|index| -> SimResult<CardInstance> {
            let pick = rng.random_int((pool.len() - 1) as i32) as usize;
            let content_id = run.content_id_after_card_add_relics(pool[pick])?;
            Ok(CardInstance::new(
                crate::ids::CardId::new(first_id + index as u64),
                content_id,
            ))
        })
        .collect::<SimResult<Vec<_>>>()?;
    if expected != grid.cards
        || rng.counter() != stream.counter
        || grid.cards.is_empty()
        || grid.selected.is_some()
        || !grid.selected_indices.is_empty()
    {
        return Err(SimError::InvalidState(
            "Pandora's Box grid does not match generated card authority",
        ));
    }
    Ok(())
}

fn validate_deck_derived_grid_payload(run: &RunState, grid: &CardGridScreen) -> SimResult<()> {
    if matches!(
        grid.purpose,
        GridPurpose::EventRemoveReturnToEvent {
            event: Event::Falling
        }
    ) {
        return validate_falling_grid_payload(run, grid);
    }

    let expected = match grid.purpose {
        GridPurpose::RestSmith
        | GridPurpose::NeowUpgrade
        | GridPurpose::EventUpgrade
        | GridPurpose::EventUpgradeReturnToEvent { .. } => Some(
            run.deck
                .iter()
                .copied()
                .filter(card_instance_is_upgradeable)
                .collect::<Vec<_>>(),
        ),
        GridPurpose::RestRemove
        | GridPurpose::EventRemove
        | GridPurpose::DesignerRemoveAndUpgrade => Some(
            run.deck
                .iter()
                .filter(|card| is_purgeable_card(card))
                .copied()
                .collect::<Vec<_>>(),
        ),
        GridPurpose::EventTransform { .. } => Some(
            run.deck
                .iter()
                .filter(|card| is_purgeable_card(card))
                .copied()
                .collect::<Vec<_>>(),
        ),
        // Drug Dealer's transform grid is `masterDeck.getPurgeableCards()` with
        // no bottled filter (FIDL01718 includes bottled Bash). Living Wall and
        // Transmogrifier wrap that group in `getGroupWithoutBottledCards`.
        GridPurpose::EventTransformReturnToEvent {
            event: Event::DrugDealer,
            ..
        } => Some(
            run.deck
                .iter()
                .filter(|card| is_purgeable_card_content(card.content_id))
                .copied()
                .collect::<Vec<_>>(),
        ),
        GridPurpose::EventTransformReturnToEvent { .. } => Some(
            run.deck
                .iter()
                .filter(|card| is_purgeable_card(card))
                .copied()
                .collect::<Vec<_>>(),
        ),
        // BonfireElementals uses the same purgeable-card authority as the
        // event's card-removal grid; soulbound curses such as Necronomicurse
        // are not selectable even though ordinary curses are.
        GridPurpose::BonfireElementals => Some(
            run.deck
                .iter()
                .filter(|card| is_purgeable_card(card))
                .copied()
                .collect::<Vec<_>>(),
        ),
        GridPurpose::EventRemoveReturnToEvent {
            event: Event::Falling,
        } => None,
        GridPurpose::EventRemoveReturnToEvent { .. } => Some(
            run.deck
                .iter()
                .filter(|card| is_purgeable_card(card))
                .copied()
                .collect::<Vec<_>>(),
        ),
        GridPurpose::ShopRemove => Some(
            run.deck
                .iter()
                .filter(|card| is_purgeable_card(card))
                .copied()
                .collect::<Vec<_>>(),
        ),
        // Empty Cage opens `getPurgeableCards` only. Bottled cards stay
        // eligible; special unremovable curses do not (FIDL01565).
        GridPurpose::EmptyCage { .. } => Some(
            run.deck
                .iter()
                .filter(|card| is_purgeable_card_content(card.content_id))
                .copied()
                .collect::<Vec<_>>(),
        ),
        GridPurpose::NeowRemove { .. }
        | GridPurpose::NeowTransform { .. }
        | GridPurpose::DollysMirror => Some(run.deck.clone()),
        GridPurpose::Astrolabe => Some(
            run.deck
                .iter()
                .filter(|card| is_purgeable_card_content(card.content_id))
                .copied()
                .collect::<Vec<_>>(),
        ),
        GridPurpose::Bottle { card_type } => {
            let mut cards = run
                .deck
                .iter()
                .copied()
                .filter(|card| {
                    !card.bottled
                        && get_card_definition(card.content_id)
                            .is_some_and(|definition| definition.card_type == card_type)
                })
                .collect::<Vec<_>>();
            cards.reverse();
            Some(cards)
        }
        GridPurpose::EventObtainCard
        | GridPurpose::EventObtainCardReturnToEvent { .. }
        | GridPurpose::CallingBellCurse
        | GridPurpose::PandorasBox => None,
    };

    let matches_deferred_neow_curse = matches!(grid.purpose, GridPurpose::NeowTransform { .. })
        && run.deck.len() == grid.cards.len() + 1
        && run.deck[..grid.cards.len()] == grid.cards
        && run
            .deck
            .last()
            .is_some_and(|card| is_curse_content_id(card.content_id));
    if expected.as_ref().is_some_and(|cards| cards != &grid.cards) && !matches_deferred_neow_curse {
        return Err(SimError::InvalidState(
            "card grid payload does not match its deck-derived authority",
        ));
    }
    Ok(())
}

fn validate_falling_grid_payload(run: &RunState, grid: &CardGridScreen) -> SimResult<()> {
    let Some(card) = grid
        .cards
        .first()
        .copied()
        .filter(|_| grid.cards.len() == 1)
    else {
        return Err(SimError::InvalidState(
            "Falling grid does not match its RNG-selected card authority",
        ));
    };
    let card_type = get_card_definition(card.content_id)
        .map(|definition| definition.card_type)
        .ok_or(SimError::UnknownContent(card.content_id))?;
    if !matches!(
        card_type,
        CardType::Attack | CardType::Skill | CardType::Power
    ) {
        return Err(SimError::InvalidState(
            "Falling grid does not match its RNG-selected card authority",
        ));
    }
    let eligible = run
        .deck
        .iter()
        .copied()
        .filter(|candidate| {
            !candidate.bottled
                && get_card_definition(candidate.content_id)
                    .is_some_and(|definition| definition.card_type == card_type)
        })
        .collect::<Vec<_>>();
    let previous_counter = run
        .misc_rng_counter
        .checked_sub(1)
        .ok_or(SimError::InvalidState(
            "Falling grid has no preceding misc RNG draw",
        ))?;
    let max_index = eligible
        .len()
        .checked_sub(1)
        .and_then(|index| i32::try_from(index).ok())
        .ok_or(SimError::InvalidState(
            "Falling grid has no supported card candidates",
        ))?;
    let mut misc_rng = StsRng::with_counter(run.misc_rng_seed as i64, previous_counter);
    let expected = eligible
        .get(misc_rng.random_int(max_index) as usize)
        .copied();
    if expected != Some(card) || misc_rng.counter() != run.misc_rng_counter {
        return Err(SimError::InvalidState(
            "Falling grid does not match its RNG-selected card authority",
        ));
    }
    Ok(())
}

fn validate_library_grid_payload(run: &RunState, grid: &CardGridScreen) -> SimResult<()> {
    let valid_selection = grid.selected.is_none() && grid.selected_indices.is_empty();
    let valid_count = grid.cards.len() == super::event::THE_LIBRARY_READ_CARD_COUNT;
    let first_id = run.reserve_card_instance_ids(super::event::THE_LIBRARY_READ_CARD_COUNT)?;
    let mut content_ids = Vec::with_capacity(grid.cards.len());
    let valid_cards = grid.cards.iter().enumerate().all(|(index, card)| {
        let expected_id = first_id + (grid.cards.len() - 1 - index) as u64;
        let canonical = CardInstance::new(card.id, card.content_id);
        content_ids.push(card.content_id);
        card.id == crate::ids::CardId::new(expected_id)
            && *card == canonical
            && IRONCLAD_REWARD_ENTRIES.iter().any(|entry| {
                entry.content_id == card.content_id
                    || run
                        .content_id_after_card_add_relics(entry.content_id)
                        .is_ok_and(|content_id| content_id == card.content_id)
            })
    });
    content_ids.sort_unstable();
    let unique_content = content_ids.windows(2).all(|pair| pair[0] != pair[1]);
    if valid_selection && valid_count && valid_cards && unique_content {
        Ok(())
    } else {
        Err(SimError::InvalidState(
            "Library grid does not match generated offer authority",
        ))
    }
}

fn validate_duplicator_grid_payload(run: &RunState, grid: &CardGridScreen) -> SimResult<()> {
    let valid_selection = grid.selected.is_none() && grid.selected_indices.is_empty();
    let first_id = run.reserve_card_instance_ids(run.deck.len())?;
    let valid_cards = grid.cards.len() == run.deck.len()
        && grid
            .cards
            .iter()
            .zip(&run.deck)
            .enumerate()
            .all(|(index, (card, source))| {
                let mut expected = *source;
                expected.id = crate::ids::CardId::new(first_id + index as u64);
                expected.bottled = false;
                *card == expected
            });
    if valid_selection && valid_cards {
        Ok(())
    } else {
        Err(SimError::InvalidState(
            "Duplicator grid does not match deck-copy authority",
        ))
    }
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
        .filter(|card| is_purgeable_card(card))
        .copied()
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
        .filter(|card| is_purgeable_card(card))
        .copied()
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
        .filter(|card| is_purgeable_card(card))
        .copied()
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
        .filter(|card| is_purgeable_card(card))
        .copied()
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
        .filter(|card| is_purgeable_card(card))
        .copied()
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
        .filter(|card| is_purgeable_card(card))
        .copied()
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
        .filter(|card| is_purgeable_card(card))
        .copied()
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
        .filter(|card| match event {
            Event::DrugDealer => is_purgeable_card_content(card.content_id),
            _ => is_purgeable_card(card),
        })
        .copied()
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

    let cards = run
        .deck
        .iter()
        .filter(|card| is_purgeable_card_content(card.content_id))
        .copied()
        .collect::<Vec<_>>();
    if cards.is_empty() {
        return;
    }

    run.card_grid = Some(CardGridScreen {
        cards,
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
        .map(|index| -> SimResult<CardInstance> {
            let pick = rng.random_int((pool.len() - 1) as i32) as usize;
            let content_id = run.content_id_after_card_add_relics(pool[pick])?;
            Ok(CardInstance::new(
                crate::ids::CardId::new(next_card_id + index as u64),
                content_id,
            ))
        })
        .collect::<SimResult<Vec<_>>>()?;
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
    // Astrolabe.onEquip builds its group from masterDeck.getPurgeableCards().
    // Bottled cards remain eligible, but soulbound curses do not.
    let cards = run
        .deck
        .iter()
        .filter(|card| is_purgeable_card_content(card.content_id))
        .copied()
        .collect::<Vec<_>>();
    if cards.is_empty() {
        return Ok(());
    }
    if cards.len() <= ASTROLABE_TRANSFORM_COUNT {
        transform_astrolabe_cards(run, &cards, astrolabe_obtain_is_pending(run))?;
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

pub(crate) const ASTROLABE_TRANSFORM_COUNT: usize = 3;

pub(crate) fn validate_grid_select(run: &RunState, index: usize) -> SimResult<()> {
    run.validate()?;
    let grid = run
        .card_grid
        .as_ref()
        .ok_or(SimError::IllegalAction("no card grid is open"))?;
    if matches!(
        grid.purpose,
        GridPurpose::CallingBellCurse | GridPurpose::PandorasBox
    ) {
        return Err(SimError::IllegalAction(
            "confirmation-only grid does not accept card selection",
        ));
    }
    if index >= grid.cards.len() {
        return Err(SimError::IllegalAction("grid index out of range"));
    }
    Ok(())
}

pub fn select_grid_card(run: &RunState, index: usize) -> SimResult<RunState> {
    validate_grid_select(run, index)?;
    let grid = run.card_grid.as_ref().expect("validated card grid");

    if let Some(required) = grid_multi_select_count(grid.purpose) {
        let mut next = run.clone();
        let grid = next.card_grid.as_mut().expect("grid present");
        if let Some(selected_position) = grid
            .selected_indices
            .iter()
            .position(|selected_index| *selected_index == index)
        {
            // CardGridSelectScreen toggles a selected card when it is clicked
            // again. This matters for Astrolabe and the other multi-select
            // grids because the target permits changing the selection before
            // confirming it.
            grid.selected_indices.remove(selected_position);
        } else {
            grid.selected_indices.push(index);
        }
        // Empty Cage, Astrolabe, multi-card Neow rewards, and multi-card event
        // transforms resolve as soon as the required cards have been selected;
        // none of these target grids exposes a separate confirmation click.
        // Other multi-select grids retain their selections until the explicit
        // GridConfirm action.
        if (matches!(
            grid.purpose,
            GridPurpose::EmptyCage { .. } | GridPurpose::Astrolabe | GridPurpose::NeowRemove { .. }
        ) || matches!(grid.purpose, GridPurpose::NeowTransform { count } if count > 1)
            || matches!(grid.purpose, GridPurpose::EventTransformReturnToEvent { count, .. } if count > 1))
            && grid.selected_indices.len() >= required
        {
            return apply_validated_grid_confirmation(&next);
        }
        return Ok(next);
    }

    let mut next = run.clone();
    let grid = next.card_grid.as_mut().expect("grid present");
    grid.selected = Some(index);
    if matches!(
        grid.purpose,
        GridPurpose::Bottle { .. }
            | GridPurpose::DollysMirror
            | GridPurpose::EventObtainCard
            | GridPurpose::EventObtainCardReturnToEvent { .. }
    ) {
        // Bottle and Dolly's Mirror open a one-card GridCardSelectScreen with
        // no confirm button (CommunicationMod `confirm_up=false`). Selecting a
        // card equips/duplicates immediately and returns to the owning screen.
        return apply_validated_grid_confirmation(&next);
    }
    // Note For Yourself and Back to Basics/Elegance open a one-card
    // GridCardSelectScreen without a confirm button. Their event updates
    // consume the selected card immediately and then expose Leave.
    // CommunicationMod records a single CHOOSE (no CONFIRM) for both.
    if matches!(
        grid.purpose,
        GridPurpose::EventRemoveReturnToEvent {
            event: Event::NoteForYourself | Event::BackToBasics
        }
    ) {
        return apply_validated_grid_confirmation(&next);
    }
    Ok(next)
}

pub(crate) fn validate_grid_cancel(run: &RunState) -> SimResult<()> {
    run.validate()?;
    let grid = run
        .card_grid
        .as_ref()
        .ok_or(SimError::IllegalAction("no card grid is open"))?;
    if !matches!(
        grid.purpose,
        GridPurpose::RestSmith | GridPurpose::RestRemove | GridPurpose::ShopRemove
    ) {
        return Err(SimError::IllegalAction("card grid cannot be cancelled"));
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
            ) && upgrade_card_instance(card)?.is_none()
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
    apply_validated_grid_confirmation(run)
}

fn apply_validated_grid_confirmation(run: &RunState) -> SimResult<RunState> {
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
            super::reward::consume_hidden_room_card_reward(&mut next)?;
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
            confirm_event_transform_grid(&mut next, count, false)?;
        }
        GridPurpose::EventTransformReturnToEvent { event, count } => {
            // The target's transform result is shown through
            // ShowCardAndObtainEffect. The source is removed at grid confirm,
            // while the replacement commits when the returned Leave screen is
            // selected.
            confirm_event_transform_grid(&mut next, count, true)?;
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
            // Back to Basics leave is stage 1 (same as Simplicity). Most other
            // remove-return events use stage 2; Wheel of Change uses stage 3.
            let leave_stage = match event {
                Event::WheelOfChange => 3,
                Event::BackToBasics => 1,
                _ => 2,
            };
            next.event = Some(EventScreen {
                event,
                choices: vec![EventChoice {
                    label: "Leave".to_owned(),
                }],
                stage: leave_stage,
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
            // Library / Duplicator: CM keeps deck and Ceramic Fish gold unchanged
            // until Leave. Queue the obtain and flush on the leave choice.
            next.queue_pending_obtain_card(card.content_id);
            next.card_grid = None;
            next.phase = RunPhase::Event;
            next.event = Some(EventScreen {
                event,
                choices: vec![EventChoice {
                    label: "Leave".to_owned(),
                }],
                // Duplicator leave is stage 2; Library leave is stage 1.
                stage: if event == Event::Duplicator { 2 } else { 1 },
                event_data: 0,
            });
        }
        GridPurpose::EmptyCage { remaining } => {
            if remaining > 1 {
                confirm_empty_cage_grid(&mut next, remaining)?;
            } else {
                let card = selected_grid_card(grid)?;
                remove_grid_card(&mut next, card, GridPurpose::EmptyCage { remaining });
            }
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
        GridPurpose::EmptyCage { remaining } if remaining > 1 => Some(usize::from(remaining)),
        GridPurpose::NeowRemove { remaining } if remaining > 1 => Some(usize::from(remaining)),
        GridPurpose::NeowTransform { count } => Some(usize::from(count)),
        GridPurpose::EventTransform { count } => Some(usize::from(count)),
        GridPurpose::EventTransformReturnToEvent { count, .. } => Some(usize::from(count)),
        _ => None,
    }
}

fn confirm_empty_cage_grid(run: &mut RunState, remaining: u8) -> SimResult<()> {
    let grid = run
        .card_grid
        .as_ref()
        .ok_or(SimError::IllegalAction("no card grid is open"))?;
    let required = usize::from(remaining);
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
            .expect("Empty Cage selected a deck card");
    }
    run.card_grid = None;
    Ok(())
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
        upgrade_card_instance(card)?.ok_or(SimError::IllegalAction("card cannot be upgraded"))?;
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
    transform_astrolabe_cards(run, &cards, astrolabe_obtain_is_pending(run))?;
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
    transform_neow_cards(run, &cards, true)?;
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

fn confirm_event_transform_grid(
    run: &mut RunState,
    count: u8,
    defer_obtains: bool,
) -> SimResult<()> {
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
    transform_event_cards(run, &cards, defer_obtains)?;
    run.card_grid = None;
    run.phase = RunPhase::Idle;
    run.event = None;
    Ok(())
}

fn transform_neow_cards(
    run: &mut RunState,
    cards: &[CardInstance],
    defer_obtains: bool,
) -> SimResult<()> {
    let sources = cards.iter().map(|card| card.content_id).collect::<Vec<_>>();
    let reward =
        crate::run::neow::generate_neow_transform_reward(run.reward_rng_seed as i64, &sources);

    if defer_obtains {
        for card in cards {
            run.remove_deck_card(card.id)
                .expect("transform selected a deck card");
        }
        // Single- and multi-card Neow transforms remove their sources at grid
        // confirmation. The target's ShowCardAndObtainEffect results remain
        // pending until Leave (FIDL01250 and the 28-trace CONFIRM cluster).
        for content_id in reward.cards {
            run.queue_pending_obtain_card(content_id);
        }
    } else {
        let next_card_id = run.reserve_card_instance_ids(cards.len())?;
        let transformed = reward
            .cards
            .into_iter()
            .enumerate()
            .map(|(index, content_id)| -> SimResult<CardInstance> {
                Ok(CardInstance::new(
                    crate::ids::CardId::new(next_card_id + index as u64),
                    run.content_id_after_card_add_relics(content_id)?,
                ))
            })
            .collect::<SimResult<Vec<_>>>()?;
        for card in cards {
            run.remove_deck_card(card.id)
                .expect("transform selected a deck card");
        }
        for card in transformed {
            run.add_deck_card(card)?;
        }
    }
    Ok(())
}

fn astrolabe_obtain_is_pending(run: &RunState) -> bool {
    (run.phase == RunPhase::Event
        && run
            .event
            .as_ref()
            .is_some_and(|screen| screen.event == Event::Neow && screen.stage == 2))
        || (run.phase == RunPhase::Treasure
            && run.current_room_kind() == Some(crate::RoomKind::Boss)
            && run.boss_chest_opened)
}

fn transform_astrolabe_cards(
    run: &mut RunState,
    cards: &[CardInstance],
    defer_obtains: bool,
) -> SimResult<()> {
    let pending_transform = defer_obtains.then(|| PendingAstrolabeTransform {
        sources: cards.to_vec(),
        rng_counter: run.misc_rng_counter,
        omamori_charges_used: run.omamori_charges_used,
    });
    let next_card_id = if defer_obtains {
        None
    } else {
        Some(run.reserve_card_instance_ids(cards.len())?)
    };
    let mut rng = StsRng::with_counter(run.misc_rng_seed as i64, run.misc_rng_counter);
    let transformed = cards
        .iter()
        .enumerate()
        .map(|(index, card)| -> SimResult<CardInstance> {
            // Astrolabe transforms, then upgrades the result. Use upgrade_card_instance
            // so Searing Blow+ carries searing_blow_upgrades (and similar metadata).
            let transformed = transform_card_content_id(card.content_id, &mut rng);
            let card_id = next_card_id
                .map(|first_id| first_id + index as u64)
                .unwrap_or_else(|| card.id.get());
            let base = CardInstance::new(crate::ids::CardId::new(card_id), transformed);
            // Astrolabe upgrades the transformed card when it can. Colorless or
            // curse results such as Doubt stay unupgraded (FIDL01329).
            Ok(upgrade_card_instance(base)?.unwrap_or(base))
        })
        .collect::<SimResult<Vec<_>>>()?;
    run.misc_rng_counter = rng.counter();

    for card in cards {
        run.remove_deck_card(card.id)
            .expect("transform selected a deck card");
    }
    if defer_obtains {
        // Astrolabe's ShowCardAndObtainEffect removes the selected sources while
        // its generated cards remain pending until the owning boundary settles.
        run.pending_astrolabe_transform = pending_transform;
        for card in transformed {
            run.queue_pending_obtain_card(card.content_id);
        }
    } else {
        for card in transformed {
            run.add_deck_card(card)?;
        }
    }
    Ok(())
}

fn transform_event_cards(
    run: &mut RunState,
    cards: &[CardInstance],
    defer_obtains: bool,
) -> SimResult<()> {
    let pending_transform = defer_obtains.then(|| PendingEventTransform {
        sources: cards.to_vec(),
        rng_counter: run.misc_rng_counter,
        omamori_charges_used: run.omamori_charges_used,
    });
    let next_card_id = if defer_obtains {
        None
    } else {
        Some(run.reserve_card_instance_ids(cards.len())?)
    };
    let mut rng = StsRng::with_counter(run.misc_rng_seed as i64, run.misc_rng_counter);
    let transformed = cards
        .iter()
        .enumerate()
        .map(|(index, card)| -> SimResult<CardInstance> {
            let content_id = transform_card_content_id(card.content_id, &mut rng);
            let card_id = next_card_id
                .map(|first_id| first_id + index as u64)
                .unwrap_or_else(|| card.id.get());
            Ok(CardInstance::new(
                crate::ids::CardId::new(card_id),
                run.content_id_after_card_add_relics(content_id)?,
            ))
        })
        .collect::<SimResult<Vec<_>>>()?;
    run.misc_rng_counter = rng.counter();

    for card in cards {
        run.remove_deck_card(card.id)
            .expect("transform selected a deck card");
    }
    if defer_obtains {
        run.pending_event_transform = pending_transform;
        for card in transformed {
            run.queue_pending_obtain_card(card.content_id);
        }
    } else {
        for card in transformed {
            run.add_deck_card(card)?;
        }
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
            upgrade_content_id, ASCENDERS_BANE_ID, BASH_ID, BITE_ID, BITE_PLUS_ID,
            CURSE_OF_THE_BELL_ID, RITUAL_DAGGER_ID, STRIKE_R_ID,
        },
        run::shop,
        RunState,
    };

    #[test]
    fn rest_smith_grid_includes_unupgraded_ritual_dagger() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.current_room_override = Some(crate::RoomKind::Rest);
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
    fn event_grids_require_their_exact_reachable_owner_stage() {
        let mut purifier = RunState::seeded_ironclad(1, 0);
        purifier.phase = RunPhase::Event;
        purifier.event = Some(crate::run::event::event_screen_for_run(
            &purifier,
            Event::Purifier,
        ));
        let opened = crate::run::event::apply_event_action(
            &purifier,
            crate::EventAction::Choose { choice_index: 0 },
        )
        .expect("Purifier opens its remove grid");
        opened
            .validate()
            .expect("Purifier owns its stage-zero remove grid");

        let grid = opened.card_grid.clone().expect("remove grid");
        let selected = select_grid_card(&opened, 0).expect("remove card can be selected");
        let mut stale = confirm_grid(&selected).expect("Purifier remove confirms");
        stale.card_grid = Some(grid);
        assert_eq!(
            stale.validate(),
            Err(SimError::InvalidState(
                "card grid purpose has no authoritative phase owner"
            ))
        );

        let mut generic = opened;
        generic.card_grid.as_mut().expect("remove grid").purpose = GridPurpose::EventUpgrade;
        assert_eq!(
            generic.validate(),
            Err(SimError::InvalidState(
                "card grid purpose has no authoritative phase owner"
            ))
        );

        let mut living_wall = RunState::seeded_ironclad(1, 0);
        living_wall.phase = RunPhase::Event;
        living_wall.event = Some(crate::run::event::event_screen_for_run(
            &living_wall,
            Event::LivingWall,
        ));
        let mut transform = crate::run::event::apply_event_action(
            &living_wall,
            crate::EventAction::Choose { choice_index: 1 },
        )
        .expect("Living Wall opens its transform grid");
        transform
            .validate()
            .expect("Living Wall owns its one-card transform grid");
        transform
            .card_grid
            .as_mut()
            .expect("transform grid")
            .purpose = GridPurpose::EventTransformReturnToEvent {
            event: Event::LivingWall,
            count: 2,
        };
        assert_eq!(
            transform.validate(),
            Err(SimError::InvalidState(
                "card grid purpose has no authoritative phase owner"
            ))
        );
    }

    #[test]
    fn generated_event_obtain_grids_require_authoritative_payloads() {
        let mut library = RunState::seeded_ironclad(1, 0);
        library.current_act = 2;
        library.phase = RunPhase::Event;
        library.event = Some(crate::run::event::event_screen_for_run(
            &library,
            Event::TheLibrary,
        ));
        let library = crate::run::event::apply_event_action(
            &library,
            crate::EventAction::Choose { choice_index: 0 },
        )
        .expect("Library opens its generated card grid");
        library
            .validate()
            .expect("Library generated payload is authoritative");

        let mut fabricated_library = library.clone();
        fabricated_library
            .card_grid
            .as_mut()
            .expect("Library grid")
            .cards[0]
            .content_id = STRIKE_R_ID;
        assert_eq!(
            fabricated_library.validate(),
            Err(SimError::InvalidState(
                "Library grid does not match generated offer authority"
            ))
        );

        let mut selected_library = library;
        selected_library
            .card_grid
            .as_mut()
            .expect("Library grid")
            .selected = Some(0);
        assert_eq!(
            selected_library.validate(),
            Err(SimError::InvalidState(
                "Library grid does not match generated offer authority"
            ))
        );

        let mut duplicator = RunState::seeded_ironclad(1, 0);
        duplicator.current_act = 2;
        duplicator.phase = RunPhase::Event;
        duplicator.event = Some(crate::run::event::event_screen_for_run(
            &duplicator,
            Event::Duplicator,
        ));
        let duplicator = crate::run::event::apply_event_action(
            &duplicator,
            crate::EventAction::Choose { choice_index: 0 },
        )
        .expect("Duplicator opens its copy grid");
        duplicator
            .validate()
            .expect("Duplicator copies exactly the current deck");

        let mut fabricated_copy = duplicator;
        fabricated_copy
            .card_grid
            .as_mut()
            .expect("Duplicator grid")
            .cards[0]
            .content_id = BASH_ID;
        assert_eq!(
            fabricated_copy.validate(),
            Err(SimError::InvalidState(
                "Duplicator grid does not match deck-copy authority"
            ))
        );
    }

    #[test]
    fn deck_derived_grids_require_complete_canonical_payloads() {
        let mut purifier = RunState::seeded_ironclad(1, 0);
        purifier.deck[0].bottled = true;
        let bottled = purifier.deck[0];
        purifier.phase = RunPhase::Event;
        purifier.event = Some(crate::run::event::event_screen_for_run(
            &purifier,
            Event::Purifier,
        ));
        let opened = crate::run::event::apply_event_action(
            &purifier,
            crate::EventAction::Choose { choice_index: 0 },
        )
        .expect("Purifier opens its remove grid");
        opened
            .validate()
            .expect("Purifier grid excludes the bottled card");
        assert!(!opened
            .card_grid
            .as_ref()
            .expect("Purifier grid")
            .cards
            .contains(&bottled));

        let mut forbidden = opened.clone();
        forbidden
            .card_grid
            .as_mut()
            .expect("Purifier grid")
            .cards
            .push(bottled);
        assert_eq!(
            forbidden.validate(),
            Err(SimError::InvalidState(
                "card grid payload does not match its deck-derived authority"
            ))
        );

        let mut incomplete = opened;
        incomplete
            .card_grid
            .as_mut()
            .expect("Purifier grid")
            .cards
            .pop();
        assert_eq!(
            incomplete.validate(),
            Err(SimError::InvalidState(
                "card grid payload does not match its deck-derived authority"
            ))
        );

        let mut shop_run = RunState::map_fixture();
        shop_run
            .gain_deck_card(CURSE_OF_THE_BELL_ID)
            .expect("Curse of the Bell can be added to the deck");
        let curse = *shop_run.deck.last().expect("added curse");
        shop_run.phase = RunPhase::Shop;
        let generated_shop = shop::generate_shop_screen(&mut shop_run).expect("shop generates");
        shop_run.shop = Some(generated_shop);
        shop_run.shop_merchant_open = true;
        open_shop_remove_grid(&mut shop_run);
        shop_run
            .validate()
            .expect("shop remove grid excludes Curse of the Bell");

        shop_run
            .card_grid
            .as_mut()
            .expect("shop remove grid")
            .cards
            .push(curse);
        assert_eq!(
            shop_run.validate(),
            Err(SimError::InvalidState(
                "card grid payload does not match its deck-derived authority"
            ))
        );
    }

    #[test]
    fn event_purge_grid_excludes_target_non_purgeable_cards() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.gain_deck_card(ASCENDERS_BANE_ID)
            .expect("Ascender's Bane can be added to the deck");
        run.gain_deck_card(CURSE_OF_THE_BELL_ID)
            .expect("Curse of the Bell can be added to the deck");
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::event_screen_for_run(
            &run,
            Event::WingStatue,
        ));

        let prayed = crate::run::event::apply_event_action(
            &run,
            crate::EventAction::Choose { choice_index: 0 },
        )
        .expect("Wing Statue prayer advances to the second screen");
        let opened = crate::run::event::apply_event_action(
            &prayed,
            crate::EventAction::Choose { choice_index: 0 },
        )
        .expect("Wing Statue opens its purge grid");
        let grid = opened.card_grid.as_ref().expect("purge grid");

        assert!(grid
            .cards
            .iter()
            .all(|card| !matches!(card.content_id, ASCENDERS_BANE_ID | CURSE_OF_THE_BELL_ID)));
        opened.validate().expect("purge grid is authoritative");
    }

    #[test]
    fn event_transform_grid_excludes_target_non_purgeable_cards() {
        // Permanent-trace cluster: Transmogrifier pray with Calling Bell in
        // deck must not list Curse of the Bell (or Ascender's Bane) as a
        // transform choice — same getPurgeableCards authority as remove.
        let mut run = RunState::seeded_ironclad(1, 0);
        run.gain_deck_card(ASCENDERS_BANE_ID)
            .expect("Ascender's Bane can be added to the deck");
        run.gain_deck_card(CURSE_OF_THE_BELL_ID)
            .expect("Curse of the Bell can be added to the deck");
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::event_screen_for_run(
            &run,
            Event::Transmorgrifier,
        ));

        let opened = crate::run::event::apply_event_action(
            &run,
            crate::EventAction::Choose { choice_index: 0 },
        )
        .expect("Transmogrifier opens its transform grid");
        let grid = opened.card_grid.as_ref().expect("transform grid");

        assert!(matches!(
            grid.purpose,
            GridPurpose::EventTransformReturnToEvent {
                event: Event::Transmorgrifier,
                count: 1
            }
        ));
        assert!(grid
            .cards
            .iter()
            .all(|card| !matches!(card.content_id, ASCENDERS_BANE_ID | CURSE_OF_THE_BELL_ID)));
        assert!(grid.cards.iter().any(|card| card.content_id == STRIKE_R_ID));
        opened.validate().expect("transform grid is authoritative");
    }

    #[test]
    fn event_transform_grid_includes_bottled_purgeable_cards() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.deck[0].bottled = true;
        let bottled = run.deck[0];
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::event_screen_for_run(
            &run,
            Event::DrugDealer,
        ));
        let opened = crate::run::event::apply_event_action(
            &run,
            crate::EventAction::Choose { choice_index: 1 },
        )
        .expect("Drug Dealer opens its transform grid");
        let grid = opened.card_grid.as_ref().expect("transform grid");
        assert!(grid.cards.contains(&bottled));
        opened
            .validate()
            .expect("bottled transform grid is authoritative");
    }

    #[test]
    fn living_wall_transform_grid_excludes_bottled_cards() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.deck[0].bottled = true;
        let bottled = run.deck[0];
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::event_screen_for_run(
            &run,
            Event::LivingWall,
        ));
        let opened = crate::run::event::apply_event_action(
            &run,
            crate::EventAction::Choose { choice_index: 1 },
        )
        .expect("Living Wall change opens its transform grid");
        let grid = opened.card_grid.as_ref().expect("transform grid");
        assert!(!grid.cards.contains(&bottled));
        opened
            .validate()
            .expect("Living Wall transform grid is authoritative");
    }

    #[test]
    fn falling_preselects_cards_before_the_choice() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::event_screen_for_run(
            &run,
            Event::Falling,
        ));
        let intro = crate::run::event::apply_event_action(
            &run,
            crate::EventAction::Choose { choice_index: 0 },
        )
        .expect("Falling intro opens card-type choices");
        assert!(intro.card_grid.is_none());
        assert_eq!(intro.misc_rng_counter, run.misc_rng_counter + 2);
        let completed = crate::run::event::apply_event_action(
            &intro,
            crate::EventAction::Choose { choice_index: 0 },
        )
        .expect("Falling removes its preselected card");
        assert!(completed.card_grid.is_none());
        completed
            .validate()
            .expect("Falling preselection and removal are authoritative");
    }

    #[test]
    fn relic_grids_require_matching_owners_and_generated_payloads() {
        let mut calling_bell = RunState::seeded_ironclad(1, 0);
        calling_bell.phase = RunPhase::Event;
        calling_bell.event = Some(crate::run::event::event_screen_for_run(
            &calling_bell,
            Event::Neow,
        ));
        calling_bell
            .gain_relic(crate::Relic::CallingBell)
            .expect("Calling Bell opens its curse grid");
        calling_bell
            .validate()
            .expect("Calling Bell owns its exact generated curse");

        let mut fabricated_bell = calling_bell.clone();
        fabricated_bell
            .card_grid
            .as_mut()
            .expect("Calling Bell grid")
            .cards[0]
            .content_id = STRIKE_R_ID;
        assert_eq!(
            fabricated_bell.validate(),
            Err(SimError::InvalidState(
                "Calling Bell grid does not match generated curse authority"
            ))
        );

        let mut unowned_bell = calling_bell;
        unowned_bell
            .relics
            .retain(|relic| *relic != crate::Relic::CallingBell);
        assert_eq!(
            unowned_bell.validate(),
            Err(SimError::InvalidState(
                "card grid purpose has no authoritative phase owner"
            ))
        );

        let mut pandora = RunState::seeded_ironclad(1, 0);
        pandora.phase = RunPhase::Event;
        pandora.event = Some(crate::run::event::event_screen_for_run(
            &pandora,
            Event::Neow,
        ));
        pandora
            .gain_relic(crate::Relic::PandorasBox)
            .expect("Pandora's Box opens its generated grid");
        pandora
            .validate()
            .expect("Pandora's Box payload replays from card RNG");

        let mut fabricated_pandora = pandora.clone();
        fabricated_pandora
            .card_grid
            .as_mut()
            .expect("Pandora's Box grid")
            .cards[0]
            .content_id = BASH_ID;
        assert_eq!(
            fabricated_pandora.validate(),
            Err(SimError::InvalidState(
                "Pandora's Box grid does not match generated card authority"
            ))
        );

        let mut missing_draw = pandora;
        missing_draw.card_random_rng_counter = 0;
        assert_eq!(
            missing_draw.validate(),
            Err(SimError::InvalidState(
                "Pandora's Box grid has no preceding card RNG draws"
            ))
        );
    }

    #[test]
    fn mandatory_relic_grids_reject_select_and_cancel() {
        let mut calling_bell = RunState::seeded_ironclad(1, 0);
        calling_bell.phase = RunPhase::Event;
        calling_bell.event = Some(crate::run::event::event_screen_for_run(
            &calling_bell,
            Event::Neow,
        ));
        calling_bell
            .gain_relic(crate::Relic::CallingBell)
            .expect("Calling Bell opens its curse grid");

        assert_eq!(
            select_grid_card(&calling_bell, 0),
            Err(SimError::IllegalAction(
                "confirmation-only grid does not accept card selection"
            ))
        );
        assert_eq!(
            cancel_grid(&calling_bell),
            Err(SimError::IllegalAction("card grid cannot be cancelled"))
        );

        let mut empty_cage = RunState::map_fixture();
        empty_cage.phase = RunPhase::Treasure;
        empty_cage.current_room_override = Some(crate::RoomKind::Boss);
        empty_cage.boss_chest_opened = true;
        empty_cage.relics.push(crate::Relic::EmptyCage);
        open_empty_cage_grid(&mut empty_cage);
        assert_eq!(
            cancel_grid(&empty_cage),
            Err(SimError::IllegalAction("card grid cannot be cancelled"))
        );

        let mut rest = RunState::map_fixture();
        rest.phase = RunPhase::Rest;
        rest.current_room_override = Some(crate::RoomKind::Rest);
        open_rest_smith_grid(&mut rest);
        cancel_grid(&rest).expect("campfire grid preserves the target cancel affordance");
    }

    #[test]
    fn rest_smith_grid_includes_bites_and_upgrades_them() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.current_room_override = Some(crate::RoomKind::Rest);
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
    fn empty_cage_grid_excludes_target_non_purgeable_cards() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Treasure;
        run.current_room_override = Some(crate::RoomKind::Boss);
        run.boss_chest_opened = true;
        run.relics.push(crate::Relic::EmptyCage);
        run.gain_deck_card(ASCENDERS_BANE_ID)
            .expect("Ascender's Bane gain succeeds");
        run.gain_deck_card(CURSE_OF_THE_BELL_ID)
            .expect("Curse of the Bell gain succeeds");

        open_empty_cage_grid(&mut run);

        let grid = run.card_grid.as_ref().expect("Empty Cage grid");
        assert!(grid
            .cards
            .iter()
            .all(|card| !matches!(card.content_id, ASCENDERS_BANE_ID | CURSE_OF_THE_BELL_ID)));
        run.validate().expect("Empty Cage grid is authoritative");
    }

    #[test]
    fn empty_cage_grid_includes_bottled_cards() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Treasure;
        run.current_room_override = Some(crate::RoomKind::Boss);
        run.boss_chest_opened = true;
        run.relics.push(crate::Relic::EmptyCage);
        run.deck[0].bottled = true;
        let bottled = run.deck[0];

        open_empty_cage_grid(&mut run);

        assert!(run
            .card_grid
            .as_ref()
            .expect("Empty Cage grid")
            .cards
            .contains(&bottled));
        run.validate()
            .expect("Empty Cage bottled card remains authoritative");
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
    fn neow_remove_two_auto_confirms_on_the_final_selection() {
        let mut run = RunState::seeded_ironclad(1, 0);
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

        let confirmed =
            select_grid_card(&first_selected, 1).expect("final selection auto-confirms removal");

        assert!(confirmed.card_grid.is_none());
        assert_eq!(confirmed.phase, RunPhase::Event);
        assert_eq!(confirmed.event.as_ref().map(|event| event.stage), Some(2));
        assert_eq!(
            confirmed.event.as_ref().expect("Neow leave screen").choices,
            vec![crate::EventChoice {
                label: "Leave".to_owned(),
            }]
        );
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
    fn neow_remove_one_still_requires_explicit_confirm() {
        let mut run = RunState::seeded_ironclad(1, 0);
        open_neow_remove_grid(&mut run, 1);
        let original_deck = run.deck.clone();

        let selected = select_grid_card(&run, 0).expect("single remove select");
        assert!(selected.card_grid.is_some());
        assert_eq!(selected.deck, original_deck);

        let confirmed = confirm_grid(&selected).expect("single remove confirm");
        assert!(confirmed.card_grid.is_none());
        assert_eq!(confirmed.deck.len(), original_deck.len() - 1);
        assert_eq!(confirmed.phase, RunPhase::Event);
        assert_eq!(confirmed.event.as_ref().map(|event| event.stage), Some(2));
    }

    #[test]
    fn neow_transform_auto_confirms_and_defers_obtains_until_leave() {
        let mut run = RunState::seeded_ironclad(1, 0);
        let original_deck = run.deck.clone();
        let sources = original_deck
            .iter()
            .take(2)
            .map(|card| card.content_id)
            .collect::<Vec<_>>();
        let expected =
            crate::generate_neow_transform_reward(run.reward_rng_seed as i64, &sources).cards;
        open_neow_transform_grid(&mut run, 2);

        let after_first = select_grid_card(&run, 0).expect("first transform source");
        assert_eq!(after_first.deck, original_deck);
        let after_final = select_grid_card(&after_first, 1).expect("second transform source");

        assert!(after_final.card_grid.is_none());
        assert_eq!(after_final.deck.len(), original_deck.len() - 2);
        assert_eq!(after_final.pending_obtain_cards, expected);
        assert!(after_final
            .deck
            .iter()
            .all(|card| !original_deck[..2].contains(card)));
        assert_eq!(after_final.event.as_ref().map(|event| event.stage), Some(2));
        after_final
            .validate()
            .expect("Neow transform pending obtains are authoritative");

        let left =
            crate::apply_event_action(&after_final, crate::EventAction::Choose { choice_index: 0 })
                .expect("Neow Leave flushes transformed obtains");
        assert!(left.pending_obtain_cards.is_empty());
        assert_eq!(left.deck.len(), original_deck.len());
        assert_eq!(
            left.deck[original_deck.len() - 2..]
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            expected
        );
        left.validate().expect("Neow Leave produces a valid run");
    }

    #[test]
    fn neow_single_transform_requires_confirm_and_defers_obtain_until_leave() {
        let mut run = RunState::seeded_ironclad(1, 0);
        let original_deck = run.deck.clone();
        let source = original_deck[0].content_id;
        let expected =
            crate::generate_neow_transform_reward(run.reward_rng_seed as i64, &[source]).cards;
        open_neow_transform_grid(&mut run, 1);

        let selected = select_grid_card(&run, 0).expect("single transform source");
        assert!(selected.card_grid.is_some());
        assert_eq!(selected.deck, original_deck);
        assert!(selected.pending_obtain_cards.is_empty());

        let confirmed = confirm_grid(&selected).expect("single transform confirms explicitly");
        assert!(confirmed.card_grid.is_none());
        assert_eq!(confirmed.deck.len(), original_deck.len() - 1);
        assert_eq!(confirmed.pending_obtain_cards, expected);
        assert_eq!(confirmed.event.as_ref().map(|event| event.stage), Some(2));
        confirmed
            .validate()
            .expect("single Neow transform pending obtain is authoritative");

        let left =
            crate::apply_event_action(&confirmed, crate::EventAction::Choose { choice_index: 0 })
                .expect("Neow Leave flushes the transformed obtain");
        assert!(left.pending_obtain_cards.is_empty());
        assert_eq!(left.deck.len(), original_deck.len());
        assert_eq!(
            left.deck.last().map(|card| card.content_id),
            expected.last().copied()
        );
        left.validate().expect("Neow Leave produces a valid run");
    }

    #[test]
    fn event_transform_return_to_event_defers_obtain_until_leave() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::event_screen_for_run(
            &run,
            Event::LivingWall,
        ));
        let original_deck = run.deck.clone();

        let changed = crate::run::event::apply_event_action(
            &run,
            crate::EventAction::Choose { choice_index: 1 },
        )
        .expect("Living Wall Change opens its transform grid");
        let source = changed
            .card_grid
            .as_ref()
            .expect("Living Wall transform grid")
            .cards[0];
        let mut expected_rng =
            StsRng::with_counter(changed.misc_rng_seed as i64, changed.misc_rng_counter);
        let expected = transform_card_content_id(source.content_id, &mut expected_rng);

        let selected = select_grid_card(&changed, 0).expect("transform source can be selected");
        let before_leave = confirm_grid(&selected).expect("transform confirms");
        assert!(before_leave.card_grid.is_none());
        assert_eq!(before_leave.deck.len(), original_deck.len() - 1);
        assert_eq!(before_leave.pending_obtain_cards, vec![expected]);
        assert_eq!(
            before_leave.event.as_ref().map(|event| event.stage),
            Some(1)
        );
        before_leave
            .validate()
            .expect("pending transform obtain is event-owned");
        let mut wrong_pending = before_leave.clone();
        wrong_pending.pending_obtain_cards = vec![crate::content::cards::JAX_ID];
        wrong_pending.pending_obtain_cards_bypass_omamori = vec![true];
        assert_eq!(
            wrong_pending.validate(),
            Err(SimError::InvalidState(
                "pending obtain cards do not match event authority"
            ))
        );

        let after_leave = crate::run::event::apply_event_action(
            &before_leave,
            crate::EventAction::Choose { choice_index: 0 },
        )
        .expect("Leave settles the transformed card");
        assert!(after_leave.pending_obtain_cards.is_empty());
        assert_eq!(after_leave.deck.len(), original_deck.len());
        assert_eq!(after_leave.deck.last().unwrap().content_id, expected);
        assert_eq!(after_leave.phase, RunPhase::Idle);
        assert!(after_leave.event.is_none());
    }

    #[test]
    fn astrolabe_excludes_target_non_purgeable_cards() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::event_screen(crate::Event::Neow));
        run.relics.push(crate::Relic::Astrolabe);
        run.gain_deck_card(ASCENDERS_BANE_ID)
            .expect("Ascender's Bane can be added to the deck");
        run.gain_deck_card(CURSE_OF_THE_BELL_ID)
            .expect("Curse of the Bell can be added to the deck");
        run.deck[0].bottled = true;
        let bottled = run.deck[0];

        open_astrolabe_grid(&mut run).expect("Astrolabe opens its grid");

        let grid = run.card_grid.as_ref().expect("Astrolabe grid");
        assert!(grid.cards.contains(&bottled));
        assert!(grid
            .cards
            .iter()
            .all(|card| !matches!(card.content_id, ASCENDERS_BANE_ID | CURSE_OF_THE_BELL_ID)));
        run.validate().expect("Astrolabe grid is authoritative");
    }

    #[test]
    fn astrolabe_multi_select_toggles_a_selected_card() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::event_screen(crate::Event::Neow));
        run.relics.push(crate::Relic::Astrolabe);
        let original_deck = run.deck.clone();
        open_astrolabe_grid(&mut run).expect("Astrolabe opens its grid");

        let after_first = select_grid_card(&run, 0).expect("first selection");
        let after_second = select_grid_card(&after_first, 1).expect("second selection");
        let after_toggle = select_grid_card(&after_second, 0).expect("toggle first selection");

        assert_eq!(
            after_toggle
                .card_grid
                .as_ref()
                .expect("Astrolabe grid remains open")
                .selected_indices,
            vec![1]
        );
        assert_eq!(after_toggle.deck, original_deck);
        assert!(confirm_grid(&after_toggle).is_err());
    }

    fn boss_astrolabe_pending_fixture() -> RunState {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Treasure;
        run.current_room_override = Some(crate::RoomKind::Boss);
        run.boss_chest_opened = true;
        run.relics.push(crate::Relic::Astrolabe);
        open_astrolabe_grid(&mut run).expect("Astrolabe opens its grid");
        let after_first = select_grid_card(&run, 0).expect("first selection");
        let after_second = select_grid_card(&after_first, 1).expect("second selection");
        select_grid_card(&after_second, 2).expect("third selection")
    }

    #[test]
    fn astrolabe_boss_grid_defers_obtains_until_chest_proceed() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Treasure;
        run.current_room_override = Some(crate::RoomKind::Boss);
        run.boss_chest_opened = true;
        run.relics.push(crate::Relic::Astrolabe);
        open_astrolabe_grid(&mut run).expect("Astrolabe opens its grid");
        let original_deck = run.deck.clone();

        let after_first = select_grid_card(&run, 0).expect("first selection");
        let after_second = select_grid_card(&after_first, 1).expect("second selection");
        let after_third = select_grid_card(&after_second, 2).expect("third selection");

        assert!(after_third.card_grid.is_none());
        assert_eq!(after_third.deck.len(), original_deck.len() - 3);
        assert_eq!(after_third.pending_obtain_cards.len(), 3);
        assert!(after_third.misc_rng_counter > run.misc_rng_counter);
        after_third
            .validate()
            .expect("boss chest owns pending Astrolabe obtains");

        let pending = after_third.pending_obtain_cards.clone();
        let settled = crate::apply_run_action(&after_third, crate::RunAction::Proceed)
            .expect("chest Proceed settles Astrolabe obtains");
        assert!(settled.pending_obtain_cards.is_empty());
        assert_eq!(settled.deck.len(), original_deck.len());
        assert_eq!(
            settled.deck[settled.deck.len() - 3..]
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            pending
        );
    }

    #[test]
    fn astrolabe_pending_provenance_rejects_malformed_source_id() {
        let mut run = boss_astrolabe_pending_fixture();
        run.pending_astrolabe_transform
            .as_mut()
            .expect("Astrolabe provenance")
            .sources[0]
            .id = crate::ids::CardId::new(0);

        assert_eq!(
            run.validate(),
            Err(crate::SimError::InvalidState(
                "pending obtain cards do not match event authority"
            ))
        );
    }

    #[test]
    fn astrolabe_pending_provenance_rejects_unknown_source_content() {
        let mut run = boss_astrolabe_pending_fixture();
        run.pending_astrolabe_transform
            .as_mut()
            .expect("Astrolabe provenance")
            .sources[0]
            .content_id = crate::ContentId::new(u64::MAX);

        assert_eq!(
            run.validate(),
            Err(crate::SimError::InvalidState(
                "pending obtain cards do not match event authority"
            ))
        );
    }

    #[test]
    fn astrolabe_pending_provenance_rejects_source_still_in_deck() {
        let mut run = boss_astrolabe_pending_fixture();
        let source_still_in_deck = run.deck[0];
        run.pending_astrolabe_transform
            .as_mut()
            .expect("Astrolabe provenance")
            .sources[0] = source_still_in_deck;

        assert_eq!(
            run.validate(),
            Err(crate::SimError::InvalidState(
                "pending obtain cards do not match event authority"
            ))
        );
    }

    #[test]
    fn astrolabe_pending_provenance_rejects_rng_counter_mismatch() {
        let mut run = boss_astrolabe_pending_fixture();
        let provenance = run
            .pending_astrolabe_transform
            .as_mut()
            .expect("Astrolabe provenance");
        provenance.rng_counter = provenance
            .rng_counter
            .checked_add(1)
            .expect("fixture counter has headroom");

        assert_eq!(
            run.validate(),
            Err(crate::SimError::InvalidState(
                "pending obtain cards do not match event authority"
            ))
        );
    }

    #[test]
    fn astrolabe_pending_provenance_rejects_non_boss_owner() {
        let mut run = boss_astrolabe_pending_fixture();
        run.boss_chest_opened = false;

        assert_eq!(
            run.validate(),
            Err(crate::SimError::InvalidState(
                "pending Astrolabe transform has no owning boundary"
            ))
        );
    }

    #[test]
    fn astrolabe_pending_provenance_rejects_omamori_counter_mismatch() {
        let mut run = boss_astrolabe_pending_fixture();
        let provenance = run
            .pending_astrolabe_transform
            .as_mut()
            .expect("Astrolabe provenance");
        provenance.omamori_charges_used = provenance
            .omamori_charges_used
            .checked_add(1)
            .expect("fixture counter has headroom");

        assert_eq!(
            run.validate(),
            Err(crate::SimError::InvalidState(
                "pending obtain cards do not match event authority"
            ))
        );
    }

    #[test]
    fn astrolabe_pending_provenance_rejects_unreachable_pending_count() {
        let mut run = boss_astrolabe_pending_fixture();
        run.pending_obtain_cards.pop();
        run.pending_obtain_cards_bypass_omamori.pop();

        assert_eq!(
            run.validate(),
            Err(crate::SimError::InvalidState(
                "pending obtain cards do not match event authority"
            ))
        );
    }

    #[test]
    fn astrolabe_pending_provenance_rejects_pending_content_mismatch() {
        let mut run = boss_astrolabe_pending_fixture();
        let original = run.pending_obtain_cards[0];
        run.pending_obtain_cards[0] = if original == crate::content::cards::BASH_ID {
            crate::content::cards::STRIKE_R_ID
        } else {
            crate::content::cards::BASH_ID
        };

        assert_eq!(
            run.validate(),
            Err(crate::SimError::InvalidState(
                "pending obtain cards do not match event authority"
            ))
        );
    }

    #[test]
    fn astrolabe_pending_provenance_rejects_non_grid_source_count() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Treasure;
        run.current_room_override = Some(crate::RoomKind::Boss);
        run.boss_chest_opened = true;
        run.relics.push(crate::Relic::Astrolabe);
        let sources = run.deck[..2].to_vec();
        transform_astrolabe_cards(&mut run, &sources, true)
            .expect("fixture can construct a deferred transform");

        assert_eq!(
            run.validate(),
            Err(crate::SimError::InvalidState(
                "pending obtain cards do not match event authority"
            ))
        );
    }

    #[test]
    fn astrolabe_neow_auto_confirms_and_defers_obtains_until_leave() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::neow_screen_for_stage(&run, 2));
        run.relics.push(crate::Relic::Astrolabe);
        let original_deck = run.deck.clone();
        let sources = original_deck[..3].to_vec();
        open_astrolabe_grid(&mut run).expect("Astrolabe opens its grid");

        let after_first = select_grid_card(&run, 0).expect("first Astrolabe source");
        let after_second = select_grid_card(&after_first, 1).expect("second Astrolabe source");
        let after_final = select_grid_card(&after_second, 2).expect("third Astrolabe source");

        assert!(after_final.card_grid.is_none());
        assert_eq!(after_final.deck.len(), original_deck.len() - 3);
        assert_eq!(after_final.pending_obtain_cards.len(), 3);
        assert!(sources
            .iter()
            .all(|source| !after_final.deck.iter().any(|card| card.id == source.id)));
        assert_eq!(after_final.event.as_ref().map(|event| event.stage), Some(2));
        after_final
            .validate()
            .expect("Astrolabe pending obtains are authoritative on Neow Leave");

        let pending = after_final.pending_obtain_cards.clone();
        let left =
            crate::apply_event_action(&after_final, crate::EventAction::Choose { choice_index: 0 })
                .expect("Neow Leave flushes Astrolabe transformed cards");
        assert!(left.pending_obtain_cards.is_empty());
        assert_eq!(left.deck.len(), original_deck.len());
        assert_eq!(
            left.deck[original_deck.len() - 3..]
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            pending
        );
        left.validate().expect("Neow Leave produces a valid run");
    }

    #[test]
    fn empty_cage_removes_two_cards_after_the_second_selection() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Treasure;
        run.current_room_override = Some(crate::RoomKind::Boss);
        run.boss_chest_opened = true;
        run.relics.push(crate::Relic::EmptyCage);
        open_empty_cage_grid(&mut run);
        let original_deck = run.deck.clone();

        let first_selected = select_grid_card(&run, 0).expect("first select");
        assert_eq!(first_selected.deck, original_deck);
        assert_eq!(
            first_selected
                .card_grid
                .as_ref()
                .expect("grid remains open")
                .selected_indices,
            vec![0]
        );
        let after_second_selection =
            select_grid_card(&first_selected, 1).expect("second select resolves Empty Cage");

        assert!(after_second_selection.card_grid.is_none());
        assert_eq!(after_second_selection.deck.len(), original_deck.len() - 2);
        assert_eq!(
            after_second_selection
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

        transform_event_cards(&mut event_run, &sources, false)
            .expect("event transforms allocate cards");
        transform_astrolabe_cards(&mut astrolabe_run, &sources, false)
            .expect("Astrolabe transforms allocate cards");

        let event_results = &event_run.deck[event_run.deck.len() - sources.len()..];
        let astrolabe_results = &astrolabe_run.deck[astrolabe_run.deck.len() - sources.len()..];
        assert_eq!(event_results.len(), astrolabe_results.len());
        for (event_card, astrolabe_card) in event_results.iter().zip(astrolabe_results) {
            if let Some(upgraded) = upgrade_content_id(event_card.content_id) {
                assert_eq!(upgraded, astrolabe_card.content_id);
            } else {
                assert_eq!(event_card.content_id, astrolabe_card.content_id);
                assert_eq!(astrolabe_card.upgrades, 0);
            }
        }
    }

    #[test]
    fn astrolabe_keeps_unupgradable_transform_results() {
        let mut run = RunState::map_fixture();
        let source = run.deck[0];
        let result = upgrade_card_instance(CardInstance::new(
            crate::ids::CardId::new(9002),
            crate::content::cards::DOUBT_ID,
        ))
        .expect("upgrade lookup");
        assert!(result.is_none());

        transform_astrolabe_cards(&mut run, &[source], false)
            .expect("Astrolabe can emit an unupgradable result");
        assert!(run.validate().is_ok());
    }

    #[test]
    fn astrolabe_searing_blow_transform_sets_upgrade_count() {
        use crate::content::cards::{
            validate_searing_blow_metadata, SEARING_BLOW_ID, SEARING_BLOW_PLUS_ID,
        };

        let mut run = RunState::map_fixture();
        let source = run.deck[0];
        // Astrolabe upgrades via upgrade_card_instance so Searing Blow+ keeps count.
        let upgraded = upgrade_card_instance(CardInstance::new(
            crate::ids::CardId::new(9001),
            SEARING_BLOW_ID,
        ))
        .expect("upgrade")
        .expect("Searing Blow upgrades");
        assert_eq!(upgraded.content_id, SEARING_BLOW_PLUS_ID);
        assert_eq!(upgraded.searing_blow_upgrades, 1);
        validate_searing_blow_metadata(&upgraded).expect("metadata");

        let card = upgrade_card_instance(CardInstance::new(source.id, SEARING_BLOW_ID))
            .expect("upgrade")
            .expect("upgradeable");
        run.deck[0] = card;
        run.validate()
            .expect("deck with Astrolabe Searing Blow+ remains valid");
    }

    #[test]
    fn dollys_mirror_shop_select_auto_confirms_and_duplicates_card() {
        use crate::relic::Relic;
        use crate::RunAction;

        let mut run = RunState::seeded_ironclad(1, 0);
        run.phase = RunPhase::Shop;
        run.event = None;
        run.gold = 999;
        let shop = shop::generate_shop_screen(&mut run).expect("shop fixture allocation is valid");
        run.shop = Some(shop);
        run.shop_merchant_open = true;
        run.shop.as_mut().expect("shop").relics[0].relic_key = Relic::DollysMirror;
        run.shop.as_mut().expect("shop").relics[0].price = 100;
        run.shop.as_mut().expect("shop").relics[0].sold = false;

        let original_deck = run.deck.clone();
        let source = original_deck[0];
        let after_buy = shop::apply_shop_action(&run, RunAction::BuyShopRelic { slot: 0 })
            .expect("Dolly's Mirror purchase succeeds");
        assert!(after_buy.relics.contains(&crate::Relic::DollysMirror));
        assert_eq!(
            after_buy
                .card_grid
                .as_ref()
                .expect("Dolly's Mirror opens a deck grid")
                .purpose,
            GridPurpose::DollysMirror
        );
        assert_eq!(
            after_buy.card_grid.as_ref().expect("grid").cards,
            original_deck
        );
        assert!(after_buy.shop_merchant_open);
        assert_eq!(after_buy.phase, RunPhase::Shop);

        let after_select =
            select_grid_card(&after_buy, 0).expect("Dolly's Mirror select auto-confirms");
        assert!(after_select.card_grid.is_none());
        assert_eq!(after_select.deck.len(), original_deck.len() + 1);
        assert_eq!(
            after_select.deck[original_deck.len()].content_id,
            source.content_id
        );
        assert_ne!(after_select.deck[original_deck.len()].id, source.id);
        assert!(!after_select.deck[original_deck.len()].bottled);
        assert!(after_select.shop_merchant_open);
        assert_eq!(after_select.phase, RunPhase::Shop);
        after_select
            .validate()
            .expect("shop after Dolly's Mirror copy remains valid");
    }
}
