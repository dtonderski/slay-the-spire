use super::*;

mod act_one;
mod act_three;
mod act_two;
mod shrine;
mod special;
use act_one::apply_act_one_event_action;
use act_three::apply_act_three_event_action;
use act_two::apply_act_two_event_action;
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

    match screen.event {
        Event::Neow => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(neow_screen_for_stage(&next, 1));
            }
            1 => {
                let options = generate_neow_options(next.event_rng_seed as i64, next.player_max_hp);
                let option = options
                    .into_iter()
                    .find(|option| option.slot == choice_index)
                    .ok_or(SimError::IllegalAction("Neow option is not available"))?;
                apply_neow_immediate_option(&mut next, option)?;
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Neow",
                ));
            }
        },
        Event::SpireHeart => match screen.stage {
            0..=2 if choice_index == 0 => {
                let stage = screen.stage + 1;
                next.event = Some(make_event_screen(
                    Event::SpireHeart,
                    spire_heart_choices(stage),
                    stage,
                ));
            }
            3 if choice_index == 0 => {
                next.phase = RunPhase::Complete;
                next.event = Some(make_event_screen(Event::SpireHeart, Vec::new(), 4));
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Spire Heart",
                ));
            }
        },
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
