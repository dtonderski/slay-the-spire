use crate::{
    content::cards::upgrade_content_id,
    rng::{JavaRng, StsRng},
    EventAction, RunPhase, RunState, SimError, SimResult,
};
use serde::{Deserialize, Serialize};

pub const GOLDEN_SHRINE_GOLD: i32 = 100;
pub const SHINING_LIGHT_HP_PERCENT: f32 = 0.20;
pub const SHRINE_CHANCE: f32 = 0.25;

#[must_use]
pub fn shining_light_hp_loss(max_hp: i32) -> i32 {
    (max_hp as f32 * SHINING_LIGHT_HP_PERCENT).round() as i32
}

fn upgrade_random_deck_cards(run: &mut RunState, max_count: usize) {
    let mut upgradeable: Vec<usize> = run
        .deck
        .iter()
        .enumerate()
        .filter_map(|(index, card)| {
            upgrade_content_id(card.content_id)
                .is_some()
                .then_some(index)
        })
        .collect();
    if upgradeable.is_empty() {
        return;
    }

    let mut misc_rng = StsRng::with_counter(run.misc_rng_seed as i64, run.misc_rng_counter);
    let shuffle_seed = misc_rng.random_long();
    run.misc_rng_counter = misc_rng.counter();

    JavaRng::new(shuffle_seed).collections_shuffle(&mut upgradeable);

    for index in upgradeable.into_iter().take(max_count) {
        let upgraded_content_id = upgrade_content_id(run.deck[index].content_id)
            .expect("upgradeable card validated before shuffle");
        run.deck[index].content_id = upgraded_content_id;
    }
}

const ACT1_EVENTS: [Event; 11] = [
    Event::BigFish,
    Event::TheCleric,
    Event::DeadAdventurer,
    Event::GoldenIdol,
    Event::WingStatue,
    Event::WorldOfGoop,
    Event::TheSsssserpent,
    Event::LivingWall,
    Event::HypnotizingColoredMushrooms,
    Event::ScrapOoze,
    Event::ShiningLight,
];

const ACT1_SHRINES: [Event; 6] = [
    Event::GoldenShrine,
    Event::Transmorgrifier,
    Event::Purifier,
    Event::UpgradeShrine,
    Event::WheelOfChange,
    Event::MatchAndKeep,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    GoldenShrine,
    BigFish,
    TheCleric,
    DeadAdventurer,
    GoldenIdol,
    WingStatue,
    WorldOfGoop,
    TheSsssserpent,
    LivingWall,
    HypnotizingColoredMushrooms,
    ScrapOoze,
    ShiningLight,
    Transmorgrifier,
    Purifier,
    UpgradeShrine,
    WheelOfChange,
    MatchAndKeep,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventChoice {
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventScreen {
    pub event: Event,
    pub choices: Vec<EventChoice>,
}

fn ensure_act1_event_lists(run: &mut RunState) {
    if run.act1_event_list.is_empty() {
        run.act1_event_list = ACT1_EVENTS.to_vec();
    }
    if run.act1_shrine_list.is_empty() {
        run.act1_shrine_list = ACT1_SHRINES.to_vec();
    }
}

fn pick_from_list(rng: &mut StsRng, list: &mut Vec<Event>) -> Event {
    let idx = rng.random_int((list.len() - 1) as i32) as usize;
    list.remove(idx)
}

fn get_shrine(run: &mut RunState, rng: &mut StsRng) -> Event {
    let mut candidates = run.act1_shrine_list.clone();
    if candidates.is_empty() {
        return pick_from_list(rng, &mut run.act1_event_list);
    }
    let event = pick_from_list(rng, &mut candidates);
    run.act1_shrine_list = candidates;
    event
}

fn get_event(run: &mut RunState, rng: &mut StsRng) -> Event {
    if run.act1_event_list.is_empty() {
        return get_shrine(run, rng);
    }
    pick_from_list(rng, &mut run.act1_event_list)
}

fn generate_event(run: &mut RunState, rng: &mut StsRng) -> Event {
    if rng.random_float_range(0.0, 1.0) < SHRINE_CHANCE && !run.act1_shrine_list.is_empty() {
        get_shrine(run, rng)
    } else {
        get_event(run, rng)
    }
}

#[must_use]
pub fn fixed_event_screen() -> EventScreen {
    EventScreen {
        event: Event::GoldenShrine,
        choices: vec![EventChoice {
            label: "Pray".to_owned(),
        }],
    }
}

pub fn enter_fixed_event_screen(run: &mut RunState) {
    run.phase = RunPhase::Event;
    run.event = Some(fixed_event_screen());
}

pub fn enter_event_screen(run: &mut RunState) {
    ensure_act1_event_lists(run);
    let mut rng = StsRng::with_counter(run.event_rng_seed as i64, run.event_rng_counter);
    let event = generate_event(run, &mut rng);
    run.event_rng_counter = rng.counter();
    run.phase = RunPhase::Event;
    run.event = Some(event_screen(event));
}

#[must_use]
pub fn event_screen(event: Event) -> EventScreen {
    match event {
        Event::GoldenShrine => fixed_event_screen(),
        Event::Purifier => EventScreen {
            event,
            choices: vec![EventChoice {
                label: "Purify".to_owned(),
            }],
        },
        Event::UpgradeShrine => EventScreen {
            event,
            choices: vec![EventChoice {
                label: "Upgrade".to_owned(),
            }],
        },
        Event::TheCleric => EventScreen {
            event,
            choices: vec![
                EventChoice {
                    label: "Heal".to_owned(),
                },
                EventChoice {
                    label: "Remove Curse".to_owned(),
                },
            ],
        },
        Event::ShiningLight => EventScreen {
            event,
            choices: vec![
                EventChoice {
                    label: "Enter the light".to_owned(),
                },
                EventChoice {
                    label: "Leave".to_owned(),
                },
            ],
        },
        _ => EventScreen {
            event,
            choices: vec![EventChoice {
                label: "Continue".to_owned(),
            }],
        },
    }
}

#[must_use]
pub fn legal_event_actions(run: &RunState) -> Vec<EventAction> {
    if run.phase != RunPhase::Event {
        return Vec::new();
    }

    run.event
        .as_ref()
        .map(|event| {
            event
                .choices
                .iter()
                .enumerate()
                .map(|(choice_index, _)| EventAction::Choose { choice_index })
                .collect()
        })
        .unwrap_or_default()
}

pub fn validate_event_action(run: &RunState, action: EventAction) -> SimResult<()> {
    if run.phase != RunPhase::Event {
        return Err(SimError::IllegalAction("event actions require event phase"));
    }

    let event = run
        .event
        .as_ref()
        .ok_or(SimError::InvalidState("event screen is missing"))?;

    match action {
        EventAction::Choose { choice_index } => {
            if event.choices.get(choice_index).is_some() {
                Ok(())
            } else {
                Err(SimError::IllegalAction("event choice is not available"))
            }
        }
    }
}

pub fn apply_event_action(run: &RunState, action: EventAction) -> SimResult<RunState> {
    validate_event_action(run, action)?;

    let mut next = run.clone();
    let event = next.event.as_ref().expect("validated event screen").event;
    match (event, action) {
        (Event::GoldenShrine, EventAction::Choose { choice_index: 0 }) => {
            next.gold += GOLDEN_SHRINE_GOLD;
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        (Event::TheCleric, EventAction::Choose { choice_index: 0 }) => {
            let heal = next.player_max_hp * 25 / 100;
            next.player_hp = (next.player_hp + heal).min(next.player_max_hp);
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        (Event::ShiningLight, EventAction::Choose { choice_index: 0 }) => {
            let loss = shining_light_hp_loss(next.player_max_hp);
            next.player_hp = (next.player_hp - loss).max(0);
            upgrade_random_deck_cards(&mut next, 2);
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        (Event::ShiningLight, EventAction::Choose { choice_index: 1 }) => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        (Event::Purifier | Event::UpgradeShrine, EventAction::Choose { choice_index: 0 }) => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        (_, EventAction::Choose { choice_index: 0 }) => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        _ => {
            return Err(SimError::IllegalAction(
                "event choice is not implemented for this event",
            ));
        }
    }

    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_event_screen_exposes_golden_shrine_choice() {
        let event = fixed_event_screen();

        assert_eq!(event.event, Event::GoldenShrine);
        assert_eq!(event.choices.len(), 1);
        assert_eq!(event.choices[0].label, "Pray");
    }

    #[test]
    fn event_screen_selection_is_deterministic_for_seed() {
        let mut first = RunState::map_fixture();
        let mut second = RunState::map_fixture();
        first.event_rng_seed = 7;
        second.event_rng_seed = 7;

        enter_event_screen(&mut first);
        enter_event_screen(&mut second);

        assert_eq!(first.event, second.event);
        assert_eq!(first.event_rng_counter, second.event_rng_counter);
        assert_eq!(first.act1_event_list, second.act1_event_list);
    }

    #[test]
    fn event_screen_selection_advances_event_rng_counter() {
        let mut run = RunState::map_fixture();
        run.event_rng_seed = 7;

        enter_event_screen(&mut run);

        assert!(run.event_rng_counter >= 1);
        assert!(
            run.act1_event_list.len() < ACT1_EVENTS.len()
                || run.act1_shrine_list.len() < ACT1_SHRINES.len()
        );
    }

    #[test]
    fn act1_event_pool_removes_selected_events() {
        let mut run = RunState::map_fixture();
        run.event_rng_seed = 22_079_335_079;

        enter_event_screen(&mut run);

        assert!(
            run.act1_event_list.len() + run.act1_shrine_list.len()
                < ACT1_EVENTS.len() + ACT1_SHRINES.len()
        );
    }

    #[test]
    fn golden_shrine_choice_grants_gold_and_exits_event() {
        let mut run = RunState::map_fixture();
        enter_fixed_event_screen(&mut run);
        let gold_before = run.gold;

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("pray");

        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
        assert_eq!(after.gold, gold_before + GOLDEN_SHRINE_GOLD);
    }

    #[test]
    fn legal_event_actions_are_available_only_during_event_phase() {
        let mut run = RunState::map_fixture();

        assert!(legal_event_actions(&run).is_empty());

        enter_fixed_event_screen(&mut run);

        assert_eq!(
            legal_event_actions(&run),
            vec![EventAction::Choose { choice_index: 0 }]
        );
    }

    #[test]
    fn event_action_rejects_missing_choice() {
        let mut run = RunState::map_fixture();
        enter_fixed_event_screen(&mut run);

        let err = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect_err("missing choice");

        assert_eq!(
            err,
            SimError::IllegalAction("event choice is not available")
        );
    }

    #[test]
    fn shining_light_hp_loss_rounds_twenty_percent_of_max_hp() {
        assert_eq!(shining_light_hp_loss(80), 16);
        assert_eq!(shining_light_hp_loss(79), 16);
    }

    #[test]
    fn shining_light_enter_costs_hp_and_upgrades_two_cards() {
        use crate::content::cards::{ANGER_ID, ANGER_PLUS_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID};
        use crate::CardId;
        use crate::CardInstance;

        let mut run = RunState::map_fixture();
        run.misc_rng_seed = 7;
        run.deck = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), ANGER_ID),
        ];
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::ShiningLight));

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("enter");

        assert_eq!(
            after.player_hp,
            run.player_hp - shining_light_hp_loss(run.player_max_hp)
        );
        assert_eq!(after.deck[0].content_id, STRIKE_R_PLUS_ID);
        assert_eq!(after.deck[1].content_id, ANGER_PLUS_ID);
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
        assert!(after.misc_rng_counter > run.misc_rng_counter);
    }

    #[test]
    fn shining_light_leave_exits_without_changes() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::ShiningLight));
        let hp_before = run.player_hp;

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 1 }).expect("leave");

        assert_eq!(after.player_hp, hp_before);
        assert_eq!(after.deck, run.deck);
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
    }

    #[test]
    fn event_action_is_illegal_outside_event_phase() {
        let run = RunState::map_fixture();

        let err = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect_err("not in event");

        assert_eq!(
            err,
            SimError::IllegalAction("event actions require event phase")
        );
    }
}
