use super::super::*;

pub(super) fn apply_lifecycle_event_action(
    next: &mut RunState,
    screen: &EventScreen,
    choice_index: usize,
) -> SimResult<bool> {
    match screen.event {
        Event::Neow => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(neow_screen_for_stage(next, 1));
            }
            1 => {
                let options = generate_neow_options(next.event_rng_seed as i64, next.player_max_hp);
                let option = options
                    .into_iter()
                    .find(|option| option.slot == choice_index)
                    .ok_or(SimError::IllegalAction("Neow option is not available"))?;
                apply_neow_immediate_option(next, option)?;
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
        _ => return Ok(false),
    }
    Ok(true)
}
