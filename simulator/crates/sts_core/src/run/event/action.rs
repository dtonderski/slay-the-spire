use super::*;

pub fn apply_event_action(run: &RunState, action: EventAction) -> SimResult<RunState> {
    validate_event_action(run, action)?;

    let mut next = run.clone();
    let EventAction::Choose { choice_index } = action;
    let screen = next.event.as_ref().expect("validated event screen").clone();

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
        Event::GoldenIdol => match screen.stage {
            0 if choice_index == 0 => {
                if has_relic_key(&next, RelicKey::GoldenIdol) {
                    next.gain_relic_key(RelicKey::Circlet)?;
                } else {
                    next.gain_relic_key(RelicKey::GoldenIdol)?;
                }
                next.event = Some(EventScreen {
                    event: Event::GoldenIdol,
                    choices: golden_idol_choices(1, next.player_max_hp, next.ascension),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            1 if choice_index == 0 => {
                // Target source uses ShowCardAndObtainEffect for the curse; the
                // card reaches the deck when that visual effect resolves.
                next.queue_pending_obtain_card(INJURY_ID);
                next.event = Some(EventScreen {
                    event: Event::GoldenIdol,
                    choices: golden_idol_choices(2, next.player_max_hp, next.ascension),
                    stage: 2,
                    event_data: 0,
                });
            }
            1 if choice_index == 1 => {
                let hp_loss = golden_idol_hp_loss(next.player_max_hp, next.ascension);
                lose_event_hp(&mut next, hp_loss);
                next.event = Some(EventScreen {
                    event: Event::GoldenIdol,
                    choices: golden_idol_choices(2, next.player_max_hp, next.ascension),
                    stage: 2,
                    event_data: hp_loss as u32,
                });
            }
            1 if choice_index == 2 => {
                let max_hp_loss = golden_idol_max_hp_loss(next.player_max_hp, next.ascension);
                next.player_max_hp = (next.player_max_hp - max_hp_loss).max(1);
                next.player_hp = next.player_hp.min(next.player_max_hp);
                next.event = Some(EventScreen {
                    event: Event::GoldenIdol,
                    choices: golden_idol_choices(2, next.player_max_hp, next.ascension),
                    stage: 2,
                    event_data: max_hp_loss as u32,
                });
            }
            2 if choice_index == 0 => {
                next.flush_pending_obtain_cards()?;
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Golden Idol",
                ));
            }
        },
        Event::WingStatue => match screen.stage {
            0 if choice_index == 0 => {
                lose_event_hp(&mut next, WING_STATUE_PRAY_HP_LOSS);
                next.event = Some(EventScreen {
                    event: Event::WingStatue,
                    choices: wing_statue_choices(1, false),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 && has_wing_statue_attack_card(&next) => {
                let gold = roll_wing_statue_gold(&mut next);
                next.gain_gold(gold)?;
                next.event = Some(EventScreen {
                    event: Event::WingStatue,
                    choices: wing_statue_choices(2, true),
                    stage: 2,
                    event_data: gold as u32,
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
                next.event = Some(EventScreen {
                    event: Event::WingStatue,
                    choices: wing_statue_choices(2, false),
                    stage: 2,
                    event_data: 0,
                });
                open_event_remove_return_to_event_grid(&mut next, Event::WingStatue);
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Wing Statue",
                ));
            }
        },
        Event::WorldOfGoop => match screen.stage {
            0 if choice_index == 0 => {
                lose_event_hp(&mut next, WORLD_OF_GOOP_DAMAGE);
                next.gain_gold(WORLD_OF_GOOP_GOLD)?;
                next.event = Some(EventScreen {
                    event: Event::WorldOfGoop,
                    choices: world_of_goop_choices(1, screen.event_data as i32),
                    stage: 1,
                    event_data: screen.event_data,
                });
            }
            0 if choice_index == 1 => {
                let gold_loss = screen.event_data as i32;
                next.gold = (next.gold - gold_loss).max(0);
                next.event = Some(EventScreen {
                    event: Event::WorldOfGoop,
                    choices: world_of_goop_choices(1, gold_loss),
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
                    "event choice is not implemented for World of Goop",
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
        Event::DeadAdventurer => match screen.stage {
            0 if choice_index == 0 => {
                let attempts = dead_adventurer_attempts(screen.event_data);
                let encounter_chance = dead_adventurer_encounter_chance(&next, attempts);
                let mut misc_rng = next.rng_for_stream(RunRngStream::Misc);
                let encounter = misc_rng.random_int(99) < encounter_chance;
                next.store_rng_counter(RunRngStream::Misc, &misc_rng);
                let event_data = dead_adventurer_event_data(
                    dead_adventurer_order(screen.event_data)?,
                    dead_adventurer_enemy(screen.event_data),
                    attempts + 1,
                );
                if encounter {
                    next.event = Some(dead_adventurer_screen(&next, 3, event_data));
                } else {
                    let reward = *dead_adventurer_order(screen.event_data)?
                        .get(attempts as usize)
                        .ok_or(SimError::InvalidState(
                            "Dead Adventurer search attempts exceed reward count",
                        ))?;
                    match reward {
                        0 => next.gain_gold(30)?,
                        2 => {
                            let act = next.current_act;
                            let relic = roll_event_relic_reward(&mut next, act);
                            next.gain_relic_key(relic)?;
                        }
                        _ => {}
                    }
                    let stage = if attempts + 1 >= 3 { 1 } else { 0 };
                    next.event = Some(dead_adventurer_screen(&next, stage, event_data));
                }
            }
            0 if choice_index == 1 => {
                next.event = Some(dead_adventurer_screen(&next, 1, screen.event_data));
            }
            1 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            2 if choice_index == 0 => {
                if dead_adventurer_pending_encounter(screen.event_data) {
                    next.event = Some(dead_adventurer_screen(&next, 3, screen.event_data));
                } else {
                    let attempts = dead_adventurer_attempts(screen.event_data);
                    let reward = *dead_adventurer_order(screen.event_data)?
                        .get(attempts.saturating_sub(1) as usize)
                        .ok_or(SimError::InvalidState(
                            "Dead Adventurer continuation attempts exceed reward count",
                        ))?;
                    match reward {
                        0 => next.gain_gold(30)?,
                        2 => {
                            let act = next.current_act;
                            let relic = roll_event_relic_reward(&mut next, act);
                            next.gain_relic_key(relic)?;
                        }
                        _ => {}
                    }
                    let stage = if attempts >= 3 { 1 } else { 0 };
                    next.event = Some(dead_adventurer_screen(&next, stage, screen.event_data));
                }
            }
            2 if choice_index == 1 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            3 if choice_index == 0 => {
                // DeadAdventurer adds its 25-35 combat gold with miscRng when
                // the search fails. A previously found GOLD reward contributes
                // the fixed extra 30 when the player then enters combat.
                let mut misc_rng = next.rng_for_stream(RunRngStream::Misc);
                next.pending_event_combat_gold_offer = 30 + misc_rng.random_int_range(25, 35);
                next.store_rng_counter(RunRngStream::Misc, &misc_rng);
                // Dead Adventurer marks the encounter as an elite fight, so
                // the post-combat screen always contains the normal elite
                // relic reward. The shuffled search reward is unrelated.
                let mut relic_rng = next.rng_for_stream(RunRngStream::Relic);
                let relic_tier = target_elite_relic_tier(&mut relic_rng);
                next.store_rng_counter(RunRngStream::Relic, &relic_rng);
                next.pending_event_combat_relic_offer =
                    Some(roll_relic_reward(&mut next, relic_tier));
                match dead_adventurer_enemy(screen.event_data) {
                    0 => enter_event_combat(&mut next, &[&SENTRY_A0, &SENTRY_A0, &SENTRY_A0])?,
                    1 => enter_event_combat(&mut next, &[&GREMLIN_NOB_A0])?,
                    _ => enter_event_combat(&mut next, &[&LAGAVULIN_A0])?,
                }
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Dead Adventurer",
                ));
            }
        },
        Event::HypnotizingColoredMushrooms => match screen.stage {
            0 if choice_index == 0 => {
                let mut misc_rng = next.rng_for_stream(RunRngStream::Misc);
                next.pending_event_combat_gold_offer = misc_rng.random_int_range(20, 30);
                next.store_rng_counter(RunRngStream::Misc, &misc_rng);
                next.pending_event_combat_relic_offer =
                    Some(if has_relic_key(&next, RelicKey::OddMushroom) {
                        RelicKey::Circlet
                    } else {
                        RelicKey::OddMushroom
                    });
                // Target MonsterHelper encounter "The Mushroom Lair" creates
                // three FungiBeast instances (desktop-1.0.jar case 18).
                enter_event_combat(
                    &mut next,
                    &[&FUNGI_BEAST_A0, &FUNGI_BEAST_A0, &FUNGI_BEAST_A0],
                )?;
            }
            0 if choice_index == 1 => {
                let heal = next.player_max_hp * 25 / 100;
                next.heal_player(heal)?;
                next.queue_pending_obtain_card(PARASITE_ID);
                next.event = Some(EventScreen {
                    event: Event::HypnotizingColoredMushrooms,
                    choices: hypnotizing_colored_mushrooms_choices(1),
                    stage: 1,
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
                    "event choice is not implemented for Hypnotizing Colored Mushrooms",
                ));
            }
        },
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
        Event::TheCleric if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
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
        Event::TheCleric if screen.stage == 0 && choice_index == 0 => {
            if next.gold < 35 {
                return Err(SimError::IllegalAction("not enough gold"));
            }
            next.gold -= 35;
            let heal = next.player_max_hp * 25 / 100;
            next.heal_player(heal)?;
            next.event = Some(make_event_screen(
                Event::TheCleric,
                vec![EventChoice {
                    label: "Leave".to_owned(),
                }],
                1,
            ));
        }
        Event::TheCleric if screen.stage == 0 && choice_index == 1 => {
            if purgeable_event_card_count(&next) == 0 {
                next.event = Some(make_event_screen(
                    Event::TheCleric,
                    vec![EventChoice {
                        label: "Leave".to_owned(),
                    }],
                    1,
                ));
                return Ok(next);
            }
            let cost = cleric_purify_cost(&next);
            if next.gold < cost {
                return Err(SimError::IllegalAction("not enough gold"));
            }
            next.gold -= cost;
            open_event_remove_return_to_event_grid(&mut next, Event::TheCleric);
        }
        Event::TheCleric if screen.stage == 0 && choice_index == 2 => {
            next.event = Some(make_event_screen(
                Event::TheCleric,
                vec![EventChoice {
                    label: "Leave".to_owned(),
                }],
                1,
            ));
        }
        Event::ShiningLight if screen.stage == 0 && choice_index == 0 => {
            let loss = shining_light_hp_loss(next.player_max_hp);
            next.player_hp = (next.player_hp - loss).max(0);
            upgrade_random_deck_cards(&mut next, 2)?;
            next.event = Some(make_event_screen(
                Event::ShiningLight,
                labeled_choices(&["Leave"]),
                1,
            ));
        }
        Event::ShiningLight if screen.stage == 0 && choice_index == 1 => {
            next.event = Some(make_event_screen(
                Event::ShiningLight,
                labeled_choices(&["Leave"]),
                1,
            ));
        }
        Event::ShiningLight if screen.stage == 1 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::Purifier if screen.stage == 0 && choice_index == 0 => {
            open_event_remove_return_to_event_grid(&mut next, Event::Purifier);
        }
        Event::Purifier if screen.stage == 0 && choice_index == 1 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::Purifier if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::Transmorgrifier if screen.stage == 0 && choice_index == 0 => {
            open_event_transform_return_to_event_grid(&mut next, Event::Transmorgrifier, 1);
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
            open_event_upgrade_return_to_event_grid(&mut next, Event::UpgradeShrine);
        }
        Event::UpgradeShrine if screen.stage == 0 && choice_index == 1 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::ScrapOoze => match screen.stage {
            0 if choice_index == 0 => {
                let hp_loss = scrap_ooze_hp_loss(next.ascension, screen.event_data)?;
                next.player_hp = (next.player_hp - hp_loss).max(0);
                if roll_scrap_ooze_relic(&mut next, screen.event_data)? {
                    scrap_ooze_success(&mut next)?;
                } else {
                    next.event = Some(EventScreen {
                        event: Event::ScrapOoze,
                        choices: scrap_ooze_choices(1),
                        stage: 1,
                        event_data: next_scrap_ooze_failed_reaches(screen.event_data)?,
                    });
                }
            }
            0 if choice_index == 1 => {
                next.event = Some(EventScreen {
                    event: Event::ScrapOoze,
                    choices: scrap_ooze_choices(2),
                    stage: 2,
                    event_data: screen.event_data,
                });
            }
            1 if choice_index == 0 => {
                let hp_loss = scrap_ooze_hp_loss(next.ascension, screen.event_data)?;
                next.player_hp = (next.player_hp - hp_loss).max(0);
                if roll_scrap_ooze_relic(&mut next, screen.event_data)? {
                    scrap_ooze_success(&mut next)?;
                } else {
                    next.event = Some(EventScreen {
                        event: Event::ScrapOoze,
                        choices: scrap_ooze_choices(1),
                        stage: 1,
                        event_data: next_scrap_ooze_failed_reaches(screen.event_data)?,
                    });
                }
            }
            1 if choice_index == 1 => {
                next.event = Some(EventScreen {
                    event: Event::ScrapOoze,
                    choices: scrap_ooze_choices(2),
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
                    "event choice is not implemented for Scrap Ooze",
                ));
            }
        },
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
        Event::Falling => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::Falling,
                    choices: falling_choices(&next, 1),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 => {
                let card_types = falling_card_types(&next);
                if card_types.is_empty() {
                    next.event = Some(make_event_screen(
                        Event::Falling,
                        labeled_choices(&["Leave"]),
                        2,
                    ));
                } else if let Some(card_type) = card_types.get(choice_index).copied() {
                    open_falling_card_grid(&mut next, card_type);
                    if next.card_grid.is_none() {
                        next.event = Some(make_event_screen(
                            Event::Falling,
                            labeled_choices(&["Leave"]),
                            2,
                        ));
                    }
                } else {
                    return Err(SimError::IllegalAction(
                        "event choice is not implemented for Falling",
                    ));
                }
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Falling",
                ));
            }
        },
        Event::MoaiHead => match screen.stage {
            0 if choice_index == 0 => {
                let loss = rounded_event_percent(
                    next.player_max_hp,
                    if next.ascension >= 15 { 0.18 } else { 0.125 },
                );
                next.player_max_hp = (next.player_max_hp - loss).max(1);
                next.player_hp = next.player_hp.min(next.player_max_hp);
                next.heal_player(next.player_max_hp)?;
                next.event = Some(make_event_screen(
                    Event::MoaiHead,
                    moai_choices(&next, 1),
                    1,
                ));
            }
            0 if has_relic_key(&next, RelicKey::GoldenIdol) && choice_index == 1 => {
                remove_relic_key(&mut next, RelicKey::GoldenIdol);
                next.gain_gold(333)?;
                next.event = Some(make_event_screen(
                    Event::MoaiHead,
                    moai_choices(&next, 1),
                    1,
                ));
            }
            0 if choice_index == screen.choices.len().saturating_sub(1) => {
                next.event = Some(make_event_screen(
                    Event::MoaiHead,
                    moai_choices(&next, 1),
                    1,
                ));
            }
            1 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for The Moai Head",
                ));
            }
        },
        Event::MysteriousSphere => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(make_event_screen(
                    Event::MysteriousSphere,
                    mysterious_sphere_choices(1),
                    1,
                ));
            }
            0 if choice_index == 1 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            1 if choice_index == 0 => {
                let mut misc_rng = next.rng_for_stream(RunRngStream::Misc);
                next.pending_event_combat_gold_offer = misc_rng.random_int_range(45, 55);
                next.store_rng_counter(RunRngStream::Misc, &misc_rng);
                next.pending_event_combat_relic_offer = Some(
                    super::super::reward::roll_relic_reward(&mut next, RelicTier::Rare),
                );
                enter_event_combat(&mut next, &[&ORB_WALKER_A0, &ORB_WALKER_A0])?;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Mysterious Sphere",
                ));
            }
        },
        Event::SensoryStone if screen.stage == 0 && choice_index == 0 => {
            let memory = roll_sensory_memory(&mut next);
            next.event = Some(EventScreen {
                event: Event::SensoryStone,
                choices: sensory_stone_choices(1),
                stage: 1,
                event_data: memory,
            });
        }
        Event::SensoryStone if screen.stage == 1 && choice_index < 3 => {
            let hp_loss = match choice_index {
                1 => 5,
                2 => 10,
                _ => 0,
            };
            if hp_loss > 0 {
                lose_event_hp(&mut next, hp_loss);
            }
            let reward_count = u8::try_from(choice_index + 1)
                .expect("Sensory Stone offers at most three card rewards");
            let card_choice_count = reward_card_choice_count(&next);
            let total_card_choices = usize::from(reward_count)
                .checked_mul(card_choice_count)
                .ok_or(SimError::InvalidState(
                    "Sensory Stone card choice count overflows usize",
                ))?;
            let mut next_card_id = next.reserve_card_instance_ids(total_card_choices)?;
            let mut card_rng = next.rng_for_stream(RunRngStream::CardReward);
            let mut rarity_factor = next.card_rarity_factor;
            let mut queued_card_rewards = Vec::with_capacity(usize::from(reward_count));
            for _ in 0..reward_count {
                let cards = target_colorless_card_reward_choices_with_count(
                    &mut card_rng,
                    &mut rarity_factor,
                    next_card_id,
                    card_choice_count,
                );
                next_card_id += cards.len() as u64;
                queued_card_rewards.push(cards);
            }
            next.card_rarity_factor = rarity_factor;
            next.store_rng_counter(RunRngStream::CardReward, &card_rng);
            next.phase = RunPhase::Reward;
            next.event = Some(make_event_screen(
                Event::SensoryStone,
                labeled_choices(&["Leave"]),
                2,
            ));
            next.reward = Some(RewardScreen {
                continuation: crate::RewardContinuation::Event,
                choices: Vec::new(),
                queued_card_rewards,
                gold_offer: 0,
                stolen_gold_offer: 0,
                potion_offer: None,
                potion_offers: Vec::new(),
                relic_offer: None,
                pending_relic_offer: None,
                queued_relic_offers: Vec::new(),
                boss_relic_choices: Vec::new(),
                card_reward_flow: crate::run::CardRewardFlow::pending(reward_count),
            });
        }
        Event::SensoryStone if screen.stage == 2 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::WindingHalls => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::WindingHalls,
                    choices: winding_halls_choices(&next, 1),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                let loss = rounded_event_percent(
                    next.player_max_hp,
                    if next.ascension >= 15 { 0.18 } else { 0.125 },
                );
                lose_event_hp(&mut next, loss);
                next.queue_pending_obtain_card(MADNESS_ID);
                next.queue_pending_obtain_card(MADNESS_ID);
                next.event = Some(make_event_screen(
                    Event::WindingHalls,
                    winding_halls_choices(&next, 2),
                    2,
                ));
            }
            1 if choice_index == 1 => {
                let heal = rounded_event_percent(
                    next.player_max_hp,
                    if next.ascension >= 15 { 0.20 } else { 0.25 },
                );
                next.heal_player(heal)?;
                next.queue_pending_obtain_card(WRITHE_ID);
                next.event = Some(make_event_screen(
                    Event::WindingHalls,
                    winding_halls_choices(&next, 2),
                    2,
                ));
            }
            1 if choice_index == 2 => {
                let loss = rounded_event_percent(next.player_max_hp, 0.05);
                next.player_max_hp = (next.player_max_hp - loss).max(1);
                next.player_hp = next.player_hp.min(next.player_max_hp);
                next.event = Some(make_event_screen(
                    Event::WindingHalls,
                    winding_halls_choices(&next, 2),
                    2,
                ));
            }
            2 if choice_index == 0 => {
                next.flush_pending_obtain_cards()?;
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Winding Halls",
                ));
            }
        },
        Event::BigFish => match screen.stage {
            0 if choice_index == 0 => {
                let heal = next.player_max_hp / 3;
                next.heal_player(heal)?;
                next.event = Some(EventScreen {
                    event: Event::BigFish,
                    choices: big_fish_choices(1),
                    stage: 1,
                    event_data: heal as u32,
                });
            }
            0 if choice_index == 1 => {
                next.gain_max_hp(BIG_FISH_MAX_HP_GAIN)?;
                next.event = Some(EventScreen {
                    event: Event::BigFish,
                    choices: big_fish_choices(1),
                    stage: 1,
                    event_data: BIG_FISH_MAX_HP_GAIN as u32,
                });
            }
            0 if choice_index == 2 => {
                let act = next.current_act;
                let key = super::super::reward::roll_event_relic_reward(&mut next, act);
                next.gain_relic_key(key)?;
                // Target source uses ShowCardAndObtainEffect for the curse; the
                // relic is obtained immediately, but the card reaches the deck
                // when that visual effect resolves.
                next.queue_pending_obtain_card(REGRET_ID);
                next.event = Some(EventScreen {
                    event: Event::BigFish,
                    choices: big_fish_choices(1),
                    stage: 1,
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
                    "event choice is not implemented for Big Fish",
                ));
            }
        },
        Event::TheSsssserpent => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::TheSsssserpent,
                    choices: sssssserpent_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                next.event = Some(EventScreen {
                    event: Event::TheSsssserpent,
                    choices: sssssserpent_choices(3),
                    stage: 3,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                next.gain_gold(SSSSSERPENT_GOLD)?;
                next.event = Some(EventScreen {
                    event: Event::TheSsssserpent,
                    choices: sssssserpent_choices(2),
                    stage: 2,
                    event_data: 0,
                });
            }
            2 if choice_index == 0 => {
                next.gain_deck_card(DOUBT_ID)?;
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            3 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for The Ssssserpent",
                ));
            }
        },
        Event::BackToBasics if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::BackToBasics if choice_index == 1 => {
            upgrade_starter_strikes_and_defends(&mut next)?;
            next.event = Some(EventScreen {
                event: Event::BackToBasics,
                choices: labeled_choices(&["Leave"]),
                stage: 1,
                event_data: 0,
            });
        }
        Event::BackToBasics if choice_index == 0 => {
            open_event_remove_grid(&mut next);
            if next.card_grid.is_none() {
                next.event = Some(EventScreen {
                    event: Event::BackToBasics,
                    choices: labeled_choices(&["Leave"]),
                    stage: 1,
                    event_data: 0,
                });
            }
        }
        Event::LivingWall if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::LivingWall if screen.stage == 0 && choice_index == 0 => {
            open_event_remove_return_to_event_grid(&mut next, Event::LivingWall);
            if next.card_grid.is_none() {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
        }
        Event::LivingWall if screen.stage == 0 && choice_index == 1 => {
            open_event_transform_return_to_event_grid(&mut next, Event::LivingWall, 1);
            if next.card_grid.is_none() {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
        }
        Event::LivingWall if screen.stage == 0 && choice_index == 2 => {
            open_event_upgrade_return_to_event_grid(&mut next, Event::LivingWall);
            if next.card_grid.is_none() {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
        }
        Event::TheLibrary if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::TheLibrary if screen.stage == 0 && choice_index == 1 => {
            let heal = the_library_heal_for_ascension(next.player_max_hp, next.ascension);
            next.heal_player(heal)?;
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::TheLibrary if screen.stage == 0 && choice_index == 0 => {
            open_the_library_read_grid(&mut next)?;
        }
        Event::TheMausoleum | Event::Vampires
            if choice_index == screen.choices.len().saturating_sub(1) =>
        {
            if screen.event == Event::TheMausoleum {
                next.flush_pending_obtain_cards()?;
            }
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::TheMausoleum if screen.stage == 0 && choice_index == 0 => {
            if roll_mausoleum_curses_player(&mut next) {
                next.queue_pending_obtain_card(WRITHE_ID);
            }
            let act = next.current_act;
            let key = super::super::reward::roll_event_relic_reward(&mut next, act);
            next.gain_relic_key(key)?;
            next.event = Some(EventScreen {
                event: Event::TheMausoleum,
                choices: labeled_choices(&["Leave"]),
                stage: 1,
                event_data: 0,
            });
        }
        Event::Vampires if choice_index == 0 => {
            let loss = vampires_max_hp_loss(next.player_max_hp);
            next.player_max_hp = (next.player_max_hp - loss).max(1);
            next.player_hp = next.player_hp.min(next.player_max_hp);
            replace_starter_strikes_with_bites(&mut next)?;
            next.event = Some(EventScreen {
                event: Event::Vampires,
                choices: labeled_choices(&["Leave"]),
                stage: 1,
                event_data: 0,
            });
        }
        Event::Vampires if choice_index == 1 && screen.choices.len() == 3 => {
            if !next.relics.contains(&Relic::BloodVial) {
                return Err(SimError::IllegalAction(
                    "Blood Vial choice requires Blood Vial",
                ));
            }
            next.relics.retain(|relic| *relic != Relic::BloodVial);
            replace_starter_strikes_with_bites(&mut next)?;
            next.event = Some(EventScreen {
                event: Event::Vampires,
                choices: labeled_choices(&["Leave"]),
                stage: 1,
                event_data: 0,
            });
        }
        Event::CursedTome => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::CursedTome,
                    choices: cursed_tome_choices(1, next.ascension),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                next.event = Some(EventScreen {
                    event: Event::CursedTome,
                    choices: cursed_tome_choices(5, next.ascension),
                    stage: 5,
                    event_data: screen.event_data,
                });
            }
            1 if choice_index == 0 => {
                lose_event_hp(&mut next, CURSED_TOME_PAGE_1_HP_LOSS);
                next.event = Some(EventScreen {
                    event: Event::CursedTome,
                    choices: cursed_tome_choices(2, next.ascension),
                    stage: 2,
                    event_data: cursed_tome_add_hp_loss(
                        screen.event_data,
                        CURSED_TOME_PAGE_1_HP_LOSS,
                    )?,
                });
            }
            2 if choice_index == 0 => {
                lose_event_hp(&mut next, CURSED_TOME_PAGE_2_HP_LOSS);
                next.event = Some(EventScreen {
                    event: Event::CursedTome,
                    choices: cursed_tome_choices(3, next.ascension),
                    stage: 3,
                    event_data: cursed_tome_add_hp_loss(
                        screen.event_data,
                        CURSED_TOME_PAGE_2_HP_LOSS,
                    )?,
                });
            }
            3 if choice_index == 0 => {
                lose_event_hp(&mut next, CURSED_TOME_PAGE_3_HP_LOSS);
                next.event = Some(EventScreen {
                    event: Event::CursedTome,
                    choices: cursed_tome_choices(4, next.ascension),
                    stage: 4,
                    event_data: cursed_tome_add_hp_loss(
                        screen.event_data,
                        CURSED_TOME_PAGE_3_HP_LOSS,
                    )?,
                });
            }
            4 if choice_index == 0 => {
                let hp_loss = cursed_tome_final_hp_loss(next.ascension);
                lose_event_hp(&mut next, hp_loss);
                let key = choose_cursed_tome_book(&mut next);
                open_cursed_tome_book_reward(&mut next, key);
            }
            4 if choice_index == 1 => {
                lose_event_hp(&mut next, CURSED_TOME_STOP_HP_LOSS);
                next.event = Some(EventScreen {
                    event: Event::CursedTome,
                    choices: cursed_tome_choices(5, next.ascension),
                    stage: 5,
                    event_data: cursed_tome_add_hp_loss(
                        screen.event_data,
                        CURSED_TOME_STOP_HP_LOSS,
                    )?,
                });
            }
            5 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Cursed Tome",
                ));
            }
        },
        Event::Nest => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::Nest,
                    choices: nest_choices(1, next.ascension),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                next.gain_gold(nest_gold_gain(next.ascension))?;
                next.event = Some(EventScreen {
                    event: Event::Nest,
                    choices: nest_choices(2, next.ascension),
                    stage: 2,
                    event_data: 0,
                });
            }
            1 if choice_index == 1 => {
                lose_event_hp(&mut next, NEST_HP_LOSS);
                next.queue_pending_obtain_card(RITUAL_DAGGER_ID);
                next.event = Some(EventScreen {
                    event: Event::Nest,
                    choices: nest_choices(2, next.ascension),
                    stage: 2,
                    event_data: 0,
                });
            }
            2 if choice_index == 0 => {
                next.flush_pending_obtain_cards()?;
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Nest",
                ));
            }
        },
        Event::Beggar => match screen.stage {
            0 if choice_index == 0 => {
                if next.gold < BEGGAR_GOLD_COST {
                    return Err(SimError::IllegalAction("not enough gold"));
                }
                next.gold -= BEGGAR_GOLD_COST;
                next.event = Some(EventScreen {
                    event: Event::Beggar,
                    choices: beggar_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            1 if choice_index == 0 => {
                open_event_remove_grid(&mut next);
                next.event = Some(EventScreen {
                    event: Event::Beggar,
                    choices: beggar_choices(2),
                    stage: 2,
                    event_data: 0,
                });
                if next.card_grid.is_none() {
                    next.phase = RunPhase::Idle;
                    next.event = None;
                }
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Beggar",
                ));
            }
        },
        Event::Addict => match screen.stage {
            0 if choice_index == 0 => {
                if next.gold < ADDICT_GOLD_COST {
                    return Err(SimError::IllegalAction("not enough gold"));
                }
                next.gold -= ADDICT_GOLD_COST;
                let act = next.current_act;
                let key = super::super::reward::roll_event_relic_reward(&mut next, act);
                next.gain_relic_key(key)?;
                next.event = Some(EventScreen {
                    event: Event::Addict,
                    choices: addict_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                next.gain_deck_card(SHAME_ID)?;
                let act = next.current_act;
                let key = super::super::reward::roll_event_relic_reward(&mut next, act);
                next.gain_relic_key(key)?;
                next.event = Some(EventScreen {
                    event: Event::Addict,
                    choices: addict_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 2 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            1 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Addict",
                ));
            }
        },
        Event::ForgottenAltar => match screen.stage {
            0 if next.relics.contains(&Relic::GoldenIdol) && choice_index == 0 => {
                give_forgotten_altar_idol(&mut next)?;
                next.event = Some(EventScreen {
                    event: Event::ForgottenAltar,
                    choices: forgotten_altar_choices(1, false),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == usize::from(next.relics.contains(&Relic::GoldenIdol)) => {
                let hp_loss = forgotten_altar_hp_loss(next.player_max_hp, next.ascension);
                next.gain_max_hp(FORGOTTEN_ALTAR_MAX_HP_GAIN)?;
                lose_event_hp(&mut next, hp_loss);
                next.event = Some(EventScreen {
                    event: Event::ForgottenAltar,
                    choices: forgotten_altar_choices(1, false),
                    stage: 1,
                    event_data: hp_loss as u32,
                });
            }
            0 if choice_index == usize::from(next.relics.contains(&Relic::GoldenIdol)) + 1 => {
                next.gain_deck_card(DECAY_ID)?;
                next.event = Some(EventScreen {
                    event: Event::ForgottenAltar,
                    choices: forgotten_altar_choices(1, false),
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
                    "event choice is not implemented for Forgotten Altar",
                ));
            }
        },
        Event::Ghosts => match screen.stage {
            0 if choice_index == 0 => {
                let loss = ghosts_max_hp_loss(next.player_max_hp);
                next.player_max_hp = (next.player_max_hp - loss).max(1);
                next.player_hp = next.player_hp.min(next.player_max_hp);
                for _ in 0..ghosts_apparition_count(next.ascension) {
                    next.queue_pending_obtain_card(APPARITION_ID);
                }
                next.event = Some(EventScreen {
                    event: Event::Ghosts,
                    choices: ghosts_choices(1, next.player_max_hp),
                    stage: 1,
                    event_data: loss as u32,
                });
            }
            0 if choice_index == 1 => {
                next.event = Some(EventScreen {
                    event: Event::Ghosts,
                    choices: ghosts_choices(1, next.player_max_hp),
                    stage: 2,
                    event_data: 0,
                });
            }
            1 | 2 if choice_index == 0 => {
                next.flush_pending_obtain_cards()?;
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Ghosts",
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
        Event::MaskedBandits => match screen.stage {
            0 if choice_index == 0 => {
                let stolen_gold = next.gold.max(0) as u32;
                next.gold = 0;
                next.event = Some(EventScreen {
                    event: Event::MaskedBandits,
                    choices: masked_bandits_choices(1),
                    stage: 1,
                    event_data: stolen_gold,
                });
            }
            0 if choice_index == 1 => {
                let mut misc_rng = next.rng_for_stream(RunRngStream::Misc);
                next.pending_event_combat_gold_offer = misc_rng.random_int_range(25, 35);
                next.store_rng_counter(RunRngStream::Misc, &misc_rng);
                next.pending_event_combat_relic_offer =
                    Some(if has_relic_key(&next, RelicKey::RedMask) {
                        RelicKey::Circlet
                    } else {
                        RelicKey::RedMask
                    });
                enter_event_combat(
                    &mut next,
                    &[&BANDIT_POINTY_A0, &BANDIT_LEADER_A0, &BANDIT_BEAR_A0],
                )?;
            }
            1 | 2 if choice_index == 0 => {
                let stage = screen.stage + 1;
                next.event = Some(EventScreen {
                    event: Event::MaskedBandits,
                    choices: masked_bandits_choices(stage as u8),
                    stage,
                    event_data: screen.event_data,
                });
            }
            3 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Masked Bandits",
                ));
            }
        },
        Event::Colosseum => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::Colosseum,
                    choices: colosseum_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                enter_event_combat(
                    &mut next,
                    &[&SLAVER_BLUE_A0, &TASKMASTER_A0, &SLAVER_RED_A0],
                )?;
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            2 if choice_index == 1 => {
                enter_event_combat(&mut next, &[&GREMLIN_NOB_A0])?;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Colosseum",
                ));
            }
        },
        Event::DrugDealer => match screen.stage {
            0 if choice_index == 0 => {
                next.gain_deck_card(JAX_ID)?;
                next.event = Some(EventScreen {
                    event: Event::DrugDealer,
                    choices: drug_dealer_choices(1, true),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                if purgeable_event_card_count(&next) < usize::from(DRUG_DEALER_TRANSFORM_COUNT) {
                    return Err(SimError::IllegalAction("not enough transformable cards"));
                }
                open_event_transform_return_to_event_grid(
                    &mut next,
                    Event::DrugDealer,
                    DRUG_DEALER_TRANSFORM_COUNT,
                );
                next.event = Some(EventScreen {
                    event: Event::DrugDealer,
                    choices: drug_dealer_choices(1, true),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 2 => {
                if has_relic_key(&next, RelicKey::MutagenicStrength) {
                    next.gain_relic_key(RelicKey::Circlet)?;
                } else {
                    next.gain_relic_key(RelicKey::MutagenicStrength)?;
                }
                next.event = Some(EventScreen {
                    event: Event::DrugDealer,
                    choices: drug_dealer_choices(1, true),
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
                    "event choice is not implemented for Drug Dealer",
                ));
            }
        },
        Event::WheelOfChange => match screen.stage {
            0 if choice_index == 0 => {
                let result = roll_wheel_of_change_result(&mut next);
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
                        choices: wheel_of_change_choices(3, screen.event_data),
                        stage: 3,
                        event_data: screen.event_data,
                    });
                }
                1 => {
                    let act = next.current_act;
                    let key = roll_event_relic_reward(&mut next, act);
                    next.phase = RunPhase::Reward;
                    next.event = None;
                    next.reward = Some(RewardScreen {
                        continuation: crate::RewardContinuation::None,
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
                    open_event_remove_return_to_event_grid(&mut next, Event::WheelOfChange);
                }
                _ => {
                    let hp_loss = wheel_of_change_hp_loss(next.player_max_hp, next.ascension);
                    lose_event_hp(&mut next, hp_loss);
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
        Event::TombOfLordRedMask if screen.stage == 0 && choice_index == 0 => {
            if has_relic_key(&next, RelicKey::RedMask) {
                next.gain_gold(222)?;
            } else {
                next.gold = 0;
                next.gain_relic_key(RelicKey::RedMask)?;
            }
            next.event = Some(make_event_screen(
                Event::TombOfLordRedMask,
                tomb_of_lord_red_mask_choices(&next, 1),
                1,
            ));
        }
        Event::TombOfLordRedMask
            if (screen.stage == 0 && choice_index == 1)
                || (screen.stage == 1 && choice_index == 0) =>
        {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::MindBloom if screen.stage == 0 && choice_index == 0 => {
            let boss = roll_mind_bloom_boss(&mut next);
            next.pending_event_combat_gold_offer = if next.ascension >= 13 { 25 } else { 50 };
            next.pending_event_combat_relic_offer = Some(super::super::reward::roll_relic_reward(
                &mut next,
                RelicTier::Rare,
            ));
            let event_room_override = next.current_room_override;
            next.current_room_override = Some(crate::map::RoomKind::Boss);
            match boss {
                0 => enter_event_combat(&mut next, &[&GUARDIAN_A0])?,
                1 => enter_event_combat(&mut next, &[&HEXAGHOST_A0])?,
                _ => enter_event_combat(&mut next, &[&SLIME_BOSS_A0])?,
            }
            next.current_room_override = event_room_override;
        }
        Event::MindBloom if screen.stage == 0 && choice_index == 1 => {
            for card in &mut next.deck {
                if let Some(upgraded) = upgrade_card_instance(*card)? {
                    *card = upgraded;
                }
            }
            next.gain_relic_key(RelicKey::MarkOfBloom)?;
            next.event = Some(make_event_screen(
                Event::MindBloom,
                labeled_choices(&["Leave"]),
                1,
            ));
        }
        Event::MindBloom if screen.stage == 0 && choice_index == 2 => {
            if next.current_floor % 50 <= 40 {
                next.gain_gold(999)?;
                next.gain_deck_card(NORMALITY_ID)?;
                next.gain_deck_card(NORMALITY_ID)?;
            } else {
                next.heal_player(next.player_max_hp)?;
                next.gain_deck_card(DOUBT_ID)?;
            }
            next.event = Some(make_event_screen(
                Event::MindBloom,
                labeled_choices(&["Leave"]),
                1,
            ));
        }
        Event::MindBloom if screen.stage == 1 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
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
                match_and_keep_card_choices(&next)?,
                2,
            ));
        }
        Event::MatchAndKeep if screen.stage == 2 => {
            let card_index = match_and_keep_card_index_for_choice(&next, &screen, choice_index)?;
            apply_match_and_keep_card_choice(&mut next, card_index)?;
        }
        Event::MatchAndKeep if screen.stage == 3 && choice_index == 0 => {
            next.flush_pending_obtain_cards()?;
            next.phase = RunPhase::Idle;
            next.event = None;
            next.match_and_keep = None;
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
