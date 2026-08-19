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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventActionFamily {
    ActOne,
    ActTwo,
    ActThree,
    Shrine,
    Special,
    Lifecycle,
}

const fn event_action_family(event: Event) -> EventActionFamily {
    match event {
        Event::BigFish
        | Event::TheCleric
        | Event::DeadAdventurer
        | Event::GoldenIdol
        | Event::WingStatue
        | Event::WorldOfGoop
        | Event::TheSsssserpent
        | Event::LivingWall
        | Event::HypnotizingColoredMushrooms
        | Event::ScrapOoze
        | Event::ShiningLight => EventActionFamily::ActOne,
        Event::Addict
        | Event::BackToBasics
        | Event::Beggar
        | Event::Colosseum
        | Event::CursedTome
        | Event::DrugDealer
        | Event::ForgottenAltar
        | Event::Ghosts
        | Event::MaskedBandits
        | Event::Nest
        | Event::TheLibrary
        | Event::TheMausoleum
        | Event::Vampires => EventActionFamily::ActTwo,
        Event::Falling
        | Event::MindBloom
        | Event::MoaiHead
        | Event::MysteriousSphere
        | Event::SensoryStone
        | Event::TombOfLordRedMask
        | Event::WindingHalls => EventActionFamily::ActThree,
        Event::GoldenShrine
        | Event::Transmorgrifier
        | Event::Purifier
        | Event::UpgradeShrine
        | Event::WheelOfChange
        | Event::MatchAndKeep => EventActionFamily::Shrine,
        Event::AccursedBlacksmith
        | Event::BonfireElementals
        | Event::Designer
        | Event::Duplicator
        | Event::FountainOfCleansing
        | Event::FaceTrader
        | Event::Nloth
        | Event::NoteForYourself
        | Event::SecretPortal
        | Event::TheJoust
        | Event::WeMeetAgain
        | Event::TheWomanInBlue
        | Event::KnowingSkull
        | Event::Lab => EventActionFamily::Special,
        Event::Neow | Event::SpireHeart => EventActionFamily::Lifecycle,
    }
}

fn require_event_handler(handled: bool) -> SimResult<()> {
    if handled {
        Ok(())
    } else {
        Err(SimError::InvalidState(
            "validated event action reached no event handler",
        ))
    }
}

pub fn apply_event_action(run: &RunState, action: EventAction) -> SimResult<RunState> {
    validate_event_action(run, action)?;

    let mut next = run.clone();
    let EventAction::Choose { choice_index } = action;
    let screen = next.event.as_ref().expect("validated event screen").clone();

    let handled = match event_action_family(screen.event) {
        EventActionFamily::ActOne => apply_act_one_event_action(&mut next, &screen, choice_index)?,
        EventActionFamily::ActTwo => apply_act_two_event_action(&mut next, &screen, choice_index)?,
        EventActionFamily::ActThree => {
            apply_act_three_event_action(&mut next, &screen, choice_index)?
        }
        EventActionFamily::Shrine => apply_shrine_event_action(&mut next, &screen, choice_index)?,
        EventActionFamily::Special => apply_special_event_action(&mut next, &screen, choice_index)?,
        EventActionFamily::Lifecycle => {
            apply_lifecycle_event_action(&mut next, &screen, choice_index)?
        }
    };

    require_event_handler(handled)?;

    // An event obtain reaches the master deck when the event is exited, not
    // when the choice resolves. The leave screen still shows the pre-obtain
    // deck; the card appears on the map transition (FIDL01244 transform: deck
    // 10 -> 7 at the leave screen, 10 again at MAP; FIDL01246 curse; FIDL01248).
    if run.phase == RunPhase::Event && next.phase != RunPhase::Event {
        next.flush_pending_obtain_cards()?;
    }

    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_event_handler_fails_closed() {
        assert_eq!(
            require_event_handler(false),
            Err(SimError::InvalidState(
                "validated event action reached no event handler"
            ))
        );
    }
}
