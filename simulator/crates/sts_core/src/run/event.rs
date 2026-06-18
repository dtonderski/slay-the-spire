use crate::{EventAction, RunPhase, RunState, SimError, SimResult};
use serde::{Deserialize, Serialize};

pub const GOLDEN_SHRINE_GOLD: i32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    GoldenShrine,
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
        _ => unreachable!("validated fixed event action"),
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
