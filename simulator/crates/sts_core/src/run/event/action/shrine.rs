use super::super::*;

pub(super) fn apply_shrine_event_action(
    next: &mut RunState,
    screen: &EventScreen,
    choice_index: usize,
) -> SimResult<bool> {
    match screen.event {
        Event::GoldenShrine => match screen.stage {
            0 if choice_index == 0 => {
                next.gain_gold(golden_shrine_gold(next.ascension))?;
                next.event = Some(make_event_screen(
                    Event::GoldenShrine,
                    golden_shrine_choices(1),
                    1,
                ));
            }
            0 if choice_index == 1 => {
                next.gain_gold(GOLDEN_SHRINE_DESECRATE_GOLD)?;
                next.queue_pending_obtain_card(REGRET_ID);
                next.event = Some(make_event_screen(
                    Event::GoldenShrine,
                    golden_shrine_choices(1),
                    1,
                ));
            }
            0 if choice_index == 2 => {
                next.event = Some(make_event_screen(
                    Event::GoldenShrine,
                    golden_shrine_choices(1),
                    1,
                ));
            }
            1 if choice_index == 0 => {
                next.flush_pending_obtain_cards()?;
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Golden Shrine",
                ));
            }
        },
        Event::Purifier if screen.stage == 0 && choice_index == 0 => {
            open_event_remove_return_to_event_grid(next, Event::Purifier);
        }
        Event::Purifier if screen.stage == 0 && choice_index == 1 => {
            next.event = Some(make_event_screen(
                Event::Purifier,
                labeled_choices(&["Leave"]),
                2,
            ));
        }
        Event::Purifier if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::Transmorgrifier if screen.stage == 0 && choice_index == 0 => {
            open_event_transform_return_to_event_grid(next, Event::Transmorgrifier, 1);
        }
        Event::Transmorgrifier if screen.stage == 0 && choice_index == 1 => {
            next.event = Some(make_event_screen(
                Event::Transmorgrifier,
                labeled_choices(&["Leave"]),
                1,
            ));
        }
        Event::Transmorgrifier if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::UpgradeShrine if screen.stage == 0 && choice_index == 0 => {
            open_event_upgrade_return_to_event_grid(next, Event::UpgradeShrine);
        }
        Event::UpgradeShrine if screen.stage == 0 && choice_index == 1 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::UpgradeShrine if screen.stage == 1 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::WheelOfChange => match screen.stage {
            0 if choice_index == 0 => {
                let result = roll_wheel_of_change_result(next);
                next.event = Some(EventScreen {
                    event: Event::WheelOfChange,
                    choices: wheel_of_change_choices(1, result),
                    stage: 1,
                    event_data: result,
                });
            }
            1 if choice_index == 0 => {
                if screen.event_data == 0 {
                    next.gain_gold(wheel_of_change_gold(next.current_act))?;
                }
                next.event = Some(EventScreen {
                    event: Event::WheelOfChange,
                    choices: wheel_of_change_choices(2, screen.event_data),
                    stage: 2,
                    event_data: screen.event_data,
                });
            }
            2 if choice_index == 0 => match screen.event_data {
                0 => {
                    next.event = Some(EventScreen {
                        event: Event::WheelOfChange,
                        choices: wheel_of_change_choices(3, 0),
                        stage: 3,
                        event_data: 0,
                    });
                }
                1 => {
                    let act = next.current_act;
                    let key = roll_event_relic_reward(next, act);
                    next.phase = RunPhase::Reward;
                    next.event = Some(EventScreen {
                        event: Event::WheelOfChange,
                        choices: wheel_of_change_choices(3, 0),
                        stage: 3,
                        event_data: 0,
                    });
                    next.reward = Some(RewardScreen {
                        continuation: crate::RewardContinuation::Event,
                        choices: Vec::new(),
                        queued_card_rewards: Vec::new(),
                        gold_offer: 0,
                        stolen_gold_offer: 0,
                        potion_offer: None,
                        potion_offers: Vec::new(),
                        relic_offer: Some(key),
                        pending_relic_offer: None,
                        queued_relic_offers: Vec::new(),
                        boss_relic_choices: Vec::new(),
                        card_reward_flow: crate::run::CardRewardFlow::None,
                    });
                }
                2 => {
                    next.heal_player(next.player_max_hp)?;
                    next.event = Some(EventScreen {
                        event: Event::WheelOfChange,
                        choices: wheel_of_change_choices(3, screen.event_data),
                        stage: 3,
                        event_data: screen.event_data,
                    });
                }
                3 => {
                    next.gain_deck_card(DECAY_ID)?;
                    next.event = Some(EventScreen {
                        event: Event::WheelOfChange,
                        choices: wheel_of_change_choices(3, screen.event_data),
                        stage: 3,
                        event_data: screen.event_data,
                    });
                }
                4 => {
                    open_event_remove_return_to_event_grid(next, Event::WheelOfChange);
                }
                _ => {
                    let hp_loss = wheel_of_change_hp_loss(next.player_max_hp, next.ascension);
                    lose_event_hp(next, hp_loss);
                    next.event = Some(EventScreen {
                        event: Event::WheelOfChange,
                        choices: wheel_of_change_choices(3, screen.event_data),
                        stage: 3,
                        event_data: screen.event_data,
                    });
                }
            },
            3 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Wheel of Change",
                ));
            }
        },
        Event::MatchAndKeep if screen.stage == 0 && choice_index == 0 => {
            next.event = Some(make_event_screen(
                Event::MatchAndKeep,
                match_and_keep_choices(1, 0),
                1,
            ));
        }
        Event::MatchAndKeep if screen.stage == 1 && choice_index == 0 => {
            next.event = Some(make_event_screen(
                Event::MatchAndKeep,
                match_and_keep_card_choices(next)?,
                2,
            ));
        }
        Event::MatchAndKeep if screen.stage == 2 => {
            let card_index = match_and_keep_card_index_for_choice(next, screen, choice_index)?;
            apply_match_and_keep_card_choice(next, card_index)?;
        }
        Event::MatchAndKeep if screen.stage == 3 && choice_index == 0 => {
            next.flush_pending_obtain_cards()?;
            next.phase = RunPhase::Idle;
            next.event = None;
            next.match_and_keep = None;
        }
        _ => return Ok(false),
    }
    Ok(true)
}
