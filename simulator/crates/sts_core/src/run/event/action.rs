use super::*;

mod act_one;
mod act_three;
mod act_two;
mod shrine;
use act_one::apply_act_one_event_action;
use act_three::apply_act_three_event_action;
use act_two::apply_act_two_event_action;
use shrine::apply_shrine_event_action;

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
        Event::BonfireElementals => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(make_event_screen(
                    Event::BonfireElementals,
                    bonfire_elementals_choices(1),
                    1,
                ));
            }
            1 if choice_index == 0 => {
                open_bonfire_elementals_grid(&mut next);
                if next.card_grid.is_none() {
                    next.event = Some(make_event_screen(
                        Event::BonfireElementals,
                        bonfire_elementals_choices(2),
                        2,
                    ));
                }
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Bonfire Elementals",
                ));
            }
        },
        Event::Designer => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(designer_screen(&next, 1, screen.event_data));
            }
            1 if choice_index == 0 => {
                let (cost, _, _, _) = designer_costs(&next);
                if next.gold < cost {
                    return Err(SimError::IllegalAction(
                        "not enough gold for Designer adjustments",
                    ));
                }
                if !designer_has_upgradable_card(&next) {
                    return Err(SimError::IllegalAction(
                        "Designer adjustments require an upgradable card",
                    ));
                }
                next.gold -= cost;
                if designer_event_data_adjustment_upgrades_one(screen.event_data) {
                    open_event_upgrade_return_to_event_grid(&mut next, Event::Designer);
                } else {
                    upgrade_random_deck_cards(&mut next, 2)?;
                    designer_done_screen(&mut next);
                }
            }
            1 if choice_index == 1 => {
                let (_, cost, _, _) = designer_costs(&next);
                if next.gold < cost {
                    return Err(SimError::IllegalAction(
                        "not enough gold for Designer cleanup",
                    ));
                }
                let card_count = designer_purgeable_card_count(&next);
                if designer_event_data_cleanup_removes_cards(screen.event_data) {
                    if card_count == 0 {
                        return Err(SimError::IllegalAction(
                            "Designer cleanup requires a purgeable card",
                        ));
                    }
                    next.gold -= cost;
                    open_event_remove_return_to_event_grid(&mut next, Event::Designer);
                } else {
                    if card_count < 2 {
                        return Err(SimError::IllegalAction(
                            "Designer cleanup requires two purgeable cards",
                        ));
                    }
                    next.gold -= cost;
                    open_event_transform_return_to_event_grid(&mut next, Event::Designer, 2);
                }
            }
            1 if choice_index == 2 => {
                let (_, _, cost, _) = designer_costs(&next);
                if next.gold < cost {
                    return Err(SimError::IllegalAction(
                        "not enough gold for Designer full service",
                    ));
                }
                if !designer_has_purgeable_card(&next) {
                    return Err(SimError::IllegalAction(
                        "Designer full service requires a purgeable card",
                    ));
                }
                next.gold -= cost;
                open_designer_remove_and_upgrade_grid(&mut next);
            }
            1 if choice_index == 3 => {
                let (_, _, _, hp_loss) = designer_costs(&next);
                lose_event_hp(&mut next, hp_loss);
                designer_done_screen(&mut next);
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Designer",
                ));
            }
        },
        Event::Duplicator => match screen.stage {
            0 if choice_index == 0 => {
                open_duplicator_card_grid(&mut next)?;
                if next.card_grid.is_none() {
                    next.event = Some(make_event_screen(
                        Event::Duplicator,
                        labeled_choices(&["Leave"]),
                        2,
                    ));
                }
            }
            0 if choice_index == 1 => {
                next.event = Some(make_event_screen(
                    Event::Duplicator,
                    labeled_choices(&["Leave"]),
                    2,
                ));
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Duplicator",
                ));
            }
        },
        Event::FountainOfCleansing => match screen.stage {
            0 if choice_index == 0 => {
                next.deck
                    .retain(|card| !fountain_removes_curse(card.content_id));
                next.event = Some(EventScreen {
                    event: Event::FountainOfCleansing,
                    choices: fountain_of_cleansing_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            1 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Fountain of Cleansing",
                ));
            }
        },
        Event::AccursedBlacksmith => match screen.stage {
            0 if choice_index == 0 => {
                open_event_upgrade_return_to_event_grid(&mut next, Event::AccursedBlacksmith);
            }
            0 if choice_index == 1 => {
                next.gain_relic_key(RelicKey::WarpedTongs)?;
                next.pending_obtain_cards.push(PAIN_ID);
                next.event = Some(EventScreen {
                    event: Event::AccursedBlacksmith,
                    choices: labeled_choices(&["Leave"]),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 2 => {
                next.event = Some(EventScreen {
                    event: Event::WingStatue,
                    choices: wing_statue_choices(2, false),
                    stage: 2,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                next.flush_pending_obtain_cards()?;
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Accursed Blacksmith",
                ));
            }
        },
        Event::WeMeetAgain => {
            match screen.stage {
                0 => {
                    let options = we_meet_again_options_from_event_data(screen.event_data);
                    let choice = we_meet_again_available_choices(options)
                        .get(choice_index)
                        .copied()
                        .ok_or(SimError::IllegalAction(
                            "We Meet Again choice is unavailable",
                        ))?;
                    match choice {
                        WeMeetAgainChoice::GivePotion => {
                            let Some(slot) = options.potion_slot else {
                                return Err(SimError::InvalidState(
                                    "We Meet Again potion choice has no encoded slot",
                                ));
                            };
                            next.take_potion_slot(slot)?;
                            let act = next.current_act;
                            let key = roll_event_relic_reward(&mut next, act);
                            next.gain_relic_key(key)?;
                        }
                        WeMeetAgainChoice::GiveGold => {
                            if options.gold_amount <= 0 || next.gold < options.gold_amount {
                                return Err(SimError::InvalidState(
                                    "We Meet Again gold choice has no payable encoded amount",
                                ));
                            }
                            next.gold -= options.gold_amount;
                            let act = next.current_act;
                            let key = roll_event_relic_reward(&mut next, act);
                            next.gain_relic_key(key)?;
                        }
                        WeMeetAgainChoice::GiveCard => {
                            let Some(card_index) = options.card_index else {
                                return Err(SimError::InvalidState(
                                    "We Meet Again card choice has no encoded index",
                                ));
                            };
                            let card = next.deck.get(card_index).copied().ok_or(
                                SimError::InvalidState("We Meet Again card option is missing"),
                            )?;
                            next.remove_deck_card(card.id)
                                .expect("We Meet Again selected a deck card");
                            let act = next.current_act;
                            let key = roll_event_relic_reward(&mut next, act);
                            next.gain_relic_key(key)?;
                        }
                        WeMeetAgainChoice::Attack => {}
                    }
                    next.event = Some(EventScreen {
                        event: Event::WeMeetAgain,
                        choices: we_meet_again_choices(1, options),
                        stage: 1,
                        event_data: screen.event_data,
                    });
                }
                1 if choice_index == 0 => {
                    next.phase = RunPhase::Idle;
                    next.event = None;
                }
                _ => {
                    return Err(SimError::IllegalAction(
                        "event choice is not implemented for We Meet Again",
                    ));
                }
            }
        }
        Event::Nloth => match screen.stage {
            0 if choice_index <= 1 => {
                let owned = nloth_owned_relic_keys(&next);
                let offered_index = nloth_choice_index(screen.event_data, choice_index);
                let offered = *owned
                    .get(offered_index)
                    .ok_or(SimError::InvalidState("N'loth offered relic is missing"))?;
                if has_relic_key(&next, RelicKey::NlothsGift) {
                    next.gain_relic_key(RelicKey::Circlet)?;
                } else {
                    if !remove_relic_key(&mut next, offered) {
                        return Err(SimError::InvalidState(
                            "N'loth offered relic is no longer owned",
                        ));
                    }
                    next.gain_relic_key(RelicKey::NlothsGift)?;
                }
                next.event = Some(EventScreen {
                    event: Event::Nloth,
                    choices: labeled_choices(&["Leave"]),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 2 => {
                next.event = Some(EventScreen {
                    event: Event::Nloth,
                    choices: labeled_choices(&["Leave"]),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for N'loth",
                ));
            }
        },
        Event::TheJoust => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(make_event_screen(Event::TheJoust, joust_choices(1), 1));
            }
            1 if choice_index <= 1 => {
                if next.gold < 50 {
                    return Err(SimError::IllegalAction("not enough gold"));
                }
                next.gold -= 50;
                next.event = Some(EventScreen {
                    event: Event::TheJoust,
                    choices: joust_choices(2),
                    stage: 2,
                    event_data: joust_event_data(choice_index == 1, false),
                });
            }
            2 if choice_index == 0 => {
                let mut misc_rng = next.rng_for_stream(RunRngStream::Misc);
                let owner_wins = misc_rng.random_float() < 0.3;
                next.store_rng_counter(RunRngStream::Misc, &misc_rng);
                next.event = Some(EventScreen {
                    event: Event::TheJoust,
                    choices: joust_choices(3),
                    stage: 3,
                    event_data: joust_event_data(screen.event_data & 1 != 0, owner_wins),
                });
            }
            3 if choice_index == 0 => {
                let bet_for = screen.event_data & 1 != 0;
                let owner_wins = screen.event_data & 2 != 0;
                if owner_wins && bet_for {
                    next.gain_gold(250)?;
                } else if !owner_wins && !bet_for {
                    next.gain_gold(100)?;
                }
                next.event = Some(make_event_screen(Event::TheJoust, joust_choices(4), 4));
            }
            4 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for The Joust",
                ));
            }
        },
        Event::TheWomanInBlue if screen.stage == 0 && choice_index < 3 => {
            let costs = [20, 30, 40];
            let cost = costs[choice_index];
            if next.gold < cost {
                return Err(SimError::IllegalAction("not enough gold"));
            }
            next.gold -= cost;
            let mut potion_rng = next.rng_for_stream(RunRngStream::Potion);
            let potion_offers = (0..=choice_index)
                .map(|_| target_uniform_random_potion(&mut potion_rng))
                .collect();
            next.store_rng_counter(RunRngStream::Potion, &potion_rng);
            next.phase = RunPhase::Reward;
            next.event = None;
            next.reward = Some(RewardScreen {
                continuation: crate::RewardContinuation::None,
                choices: Vec::new(),
                queued_card_rewards: Vec::new(),
                gold_offer: 0,
                stolen_gold_offer: 0,
                potion_offer: None,
                potion_offers,
                relic_offer: None,
                pending_relic_offer: None,
                queued_relic_offers: Vec::new(),
                boss_relic_choices: Vec::new(),
                card_reward_flow: crate::run::CardRewardFlow::None,
            });
        }
        Event::TheWomanInBlue if screen.stage == 0 && choice_index == 3 => {
            if next.ascension >= 15 {
                let hp_loss = woman_in_blue_punch_hp_loss(next.player_max_hp);
                next.player_hp = (next.player_hp - hp_loss).max(0);
            }
            next.event = Some(make_event_screen(
                Event::TheWomanInBlue,
                labeled_choices(&["Leave"]),
                1,
            ));
        }
        Event::TheWomanInBlue if screen.stage == 1 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::FaceTrader => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::FaceTrader,
                    choices: face_trader_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                let damage = face_trader_damage(next.player_max_hp);
                next.gain_gold(face_trader_gold(next.ascension))?;
                lose_event_hp(&mut next, damage);
                next.event = Some(EventScreen {
                    event: Event::FaceTrader,
                    choices: face_trader_choices(2),
                    stage: 2,
                    event_data: 0,
                });
            }
            1 if choice_index == 1 => {
                let key = roll_face_trader_relic(&mut next);
                next.gain_relic_key(key)?;
                next.event = Some(EventScreen {
                    event: Event::FaceTrader,
                    choices: face_trader_choices(2),
                    stage: 2,
                    event_data: 0,
                });
            }
            1 if choice_index == 2 => {
                next.event = Some(EventScreen {
                    event: Event::FaceTrader,
                    choices: face_trader_choices(2),
                    stage: 2,
                    event_data: 0,
                });
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Face Trader",
                ));
            }
        },
        Event::NoteForYourself => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::NoteForYourself,
                    choices: note_for_yourself_choices_for_run(&next, 1)?,
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                let note = note_card_for_run(&next)?;
                next.add_deck_card(note)?;
                open_event_remove_return_to_event_grid(&mut next, Event::NoteForYourself);
            }
            1 if choice_index == 1 => {
                next.event = Some(EventScreen {
                    event: Event::NoteForYourself,
                    choices: note_for_yourself_choices(2),
                    stage: 2,
                    event_data: 0,
                });
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Note For Yourself",
                ));
            }
        },
        Event::SecretPortal => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(make_event_screen(
                    Event::SecretPortal,
                    labeled_choices(&["Continue"]),
                    1,
                ));
            }
            0 if choice_index == 1 => {
                next.event = Some(make_event_screen(
                    Event::SecretPortal,
                    labeled_choices(&["Leave"]),
                    2,
                ));
            }
            1 if choice_index == 0 => {
                next.current_room_override = Some(crate::map::RoomKind::Boss);
                enter_secret_portal_boss_combat(&mut next)?;
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Secret Portal",
                ));
            }
        },
        Event::KnowingSkull => match screen.stage {
            0 if choice_index == 0 => {
                let event_data = knowing_skull_event_data(knowing_skull_costs(0))?;
                next.event = Some(EventScreen {
                    event: Event::KnowingSkull,
                    choices: knowing_skull_choices(1, event_data),
                    stage: 1,
                    event_data,
                });
            }
            1 if choice_index == 0 => {
                let mut costs = knowing_skull_costs(screen.event_data);
                lose_event_hp(&mut next, costs.potion);
                costs.potion += 1;
                let event_data = knowing_skull_event_data(costs)?;
                knowing_skull_gain_random_potion(&mut next);
                next.event = Some(EventScreen {
                    event: Event::KnowingSkull,
                    choices: knowing_skull_choices(1, event_data),
                    stage: 1,
                    event_data,
                });
            }
            1 if choice_index == 1 => {
                let mut costs = knowing_skull_costs(screen.event_data);
                lose_event_hp(&mut next, costs.gold);
                costs.gold += 1;
                let event_data = knowing_skull_event_data(costs)?;
                next.gain_gold(KNOWING_SKULL_GOLD_REWARD)?;
                next.event = Some(EventScreen {
                    event: Event::KnowingSkull,
                    choices: knowing_skull_choices(1, event_data),
                    stage: 1,
                    event_data,
                });
            }
            1 if choice_index == 2 => {
                let mut costs = knowing_skull_costs(screen.event_data);
                lose_event_hp(&mut next, costs.card);
                costs.card += 1;
                let event_data = knowing_skull_event_data(costs)?;
                knowing_skull_gain_random_colorless(&mut next)?;
                next.event = Some(EventScreen {
                    event: Event::KnowingSkull,
                    choices: knowing_skull_choices(1, event_data),
                    stage: 1,
                    event_data,
                });
            }
            1 if choice_index == 3 => {
                let costs = knowing_skull_costs(screen.event_data);
                lose_event_hp(&mut next, costs.leave);
                next.event = Some(EventScreen {
                    event: Event::KnowingSkull,
                    choices: knowing_skull_choices(2, screen.event_data),
                    stage: 2,
                    event_data: screen.event_data,
                });
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Knowing Skull",
                ));
            }
        },
        Event::Lab if choice_index == 0 => {
            let mut potion_rng = next.rng_for_stream(RunRngStream::Potion);
            let potion_count = if next.ascension < 15 { 3 } else { 2 };
            let potion_offers = (0..potion_count)
                .map(|_| target_uniform_random_potion(&mut potion_rng))
                .collect();
            next.store_rng_counter(RunRngStream::Potion, &potion_rng);
            next.phase = RunPhase::Reward;
            next.event = None;
            next.reward = Some(RewardScreen {
                continuation: crate::RewardContinuation::None,
                choices: Vec::new(),
                queued_card_rewards: Vec::new(),
                gold_offer: 0,
                stolen_gold_offer: 0,
                potion_offer: None,
                potion_offers,
                relic_offer: None,
                pending_relic_offer: None,
                queued_relic_offers: Vec::new(),
                boss_relic_choices: Vec::new(),
                card_reward_flow: crate::run::CardRewardFlow::None,
            });
        }
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
