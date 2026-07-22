use super::*;

mod act_one;
mod act_three;
mod act_two;
mod lifecycle;
mod shrine;
mod special;
use act_one::apply_act_one_event_action;
use act_three::apply_act_three_event_action;
use act_two::apply_act_two_event_action;
use lifecycle::apply_lifecycle_event_action;
use shrine::apply_shrine_event_action;
use special::apply_special_event_action;

pub fn apply_event_action(run: &RunState, action: EventAction) -> SimResult<RunState> {
    validate_event_action(run, action)?;

    let mut next = run.clone();
    let EventAction::Choose { choice_index } = action;
    let screen = next.event.as_ref().expect("validated event screen").clone();

    if apply_act_one_event_action(&mut next, &screen, choice_index)? {
        return Ok(next);
    }

    if apply_act_two_event_action(&mut next, &screen, choice_index)? {
        return Ok(next);
    }

    if apply_act_three_event_action(&mut next, &screen, choice_index)? {
        return Ok(next);
    }

    if apply_shrine_event_action(&mut next, &screen, choice_index)? {
        return Ok(next);
    }

    if apply_special_event_action(&mut next, &screen, choice_index)? {
        return Ok(next);
    }

    if apply_lifecycle_event_action(&mut next, &screen, choice_index)? {
        return Ok(next);
    }

    match screen.event {
        _ if choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
            next.match_and_keep = None;
        }
        _ => {
            return Err(SimError::IllegalAction(
                "event choice is not implemented for this event",
            ));
        }
    }

    Ok(next)
}
