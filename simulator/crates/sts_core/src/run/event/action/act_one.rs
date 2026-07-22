use super::super::*;

pub(super) fn apply_act_one_event_action(
    next: &mut RunState,
    screen: &EventScreen,
    choice_index: usize,
) -> SimResult<bool> {
    match screen.event {
        Event::GoldenIdol => match screen.stage {
            0 if choice_index == 0 => {
                if has_relic_key(next, RelicKey::GoldenIdol) {
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
                next.event = Some(EventScreen {
                    event: Event::GoldenIdol,
                    choices: golden_idol_choices(3, next.player_max_hp, next.ascension),
                    stage: 3,
                    event_data: 0,
                });
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
                lose_event_hp(next, hp_loss);
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
            2 | 3 if choice_index == 0 => {
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
                lose_event_hp(next, WING_STATUE_PRAY_HP_LOSS);
                next.event = Some(EventScreen {
                    event: Event::WingStatue,
                    choices: wing_statue_choices(1, false),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 && has_wing_statue_attack_card(next) => {
                let gold = roll_wing_statue_gold(next);
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
                open_event_remove_return_to_event_grid(next, Event::WingStatue);
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
                lose_event_hp(next, WORLD_OF_GOOP_DAMAGE);
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
        Event::DeadAdventurer => match screen.stage {
            0 if choice_index == 0 => {
                let attempts = dead_adventurer_attempts(screen.event_data);
                let encounter_chance = dead_adventurer_encounter_chance(next, attempts);
                let mut misc_rng = next.rng_for_stream(RunRngStream::Misc);
                let encounter = misc_rng.random_int(99) < encounter_chance;
                next.store_rng_counter(RunRngStream::Misc, &misc_rng);
                let event_data = dead_adventurer_event_data(
                    dead_adventurer_order(screen.event_data)?,
                    dead_adventurer_enemy(screen.event_data),
                    attempts + 1,
                );
                if encounter {
                    next.event = Some(dead_adventurer_screen(next, 3, event_data));
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
                            let relic = roll_event_relic_reward(next, act);
                            next.gain_relic_key(relic)?;
                        }
                        _ => {}
                    }
                    let stage = if attempts + 1 >= 3 { 1 } else { 0 };
                    next.event = Some(dead_adventurer_screen(next, stage, event_data));
                }
            }
            0 if choice_index == 1 => {
                next.event = Some(dead_adventurer_screen(next, 1, screen.event_data));
            }
            1 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            2 if choice_index == 0 => {
                if dead_adventurer_pending_encounter(screen.event_data) {
                    next.event = Some(dead_adventurer_screen(next, 3, screen.event_data));
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
                            let relic = roll_event_relic_reward(next, act);
                            next.gain_relic_key(relic)?;
                        }
                        _ => {}
                    }
                    let stage = if attempts >= 3 { 1 } else { 0 };
                    next.event = Some(dead_adventurer_screen(next, stage, screen.event_data));
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
                next.pending_event_combat_relic_offer = Some(roll_relic_reward(next, relic_tier));
                match dead_adventurer_enemy(screen.event_data) {
                    0 => enter_event_combat(next, &[&SENTRY_A0, &SENTRY_A0, &SENTRY_A0])?,
                    1 => enter_event_combat(next, &[&GREMLIN_NOB_A0])?,
                    _ => enter_event_combat(next, &[&LAGAVULIN_A0])?,
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
                    Some(if has_relic_key(next, RelicKey::OddMushroom) {
                        RelicKey::Circlet
                    } else {
                        RelicKey::OddMushroom
                    });
                // Target MonsterHelper encounter "The Mushroom Lair" creates
                // three FungiBeast instances (desktop-1.0.jar case 18).
                enter_event_combat(next, &[&FUNGI_BEAST_A0, &FUNGI_BEAST_A0, &FUNGI_BEAST_A0])?;
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
        Event::TheCleric if screen.stage > 0 && choice_index == 0 => {
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
            if purgeable_event_card_count(next) == 0 {
                next.event = Some(make_event_screen(
                    Event::TheCleric,
                    vec![EventChoice {
                        label: "Leave".to_owned(),
                    }],
                    1,
                ));
                return Ok(true);
            }
            let cost = cleric_purify_cost(next);
            if next.gold < cost {
                return Err(SimError::IllegalAction("not enough gold"));
            }
            next.gold -= cost;
            open_event_remove_return_to_event_grid(next, Event::TheCleric);
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
            upgrade_random_deck_cards(next, 2)?;
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
        Event::ScrapOoze => match screen.stage {
            0 if choice_index == 0 => {
                let hp_loss = scrap_ooze_hp_loss(next.ascension, screen.event_data)?;
                next.player_hp = (next.player_hp - hp_loss).max(0);
                if roll_scrap_ooze_relic(next, screen.event_data)? {
                    scrap_ooze_success(next)?;
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
                if roll_scrap_ooze_relic(next, screen.event_data)? {
                    scrap_ooze_success(next)?;
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
                let key = super::super::super::reward::roll_event_relic_reward(next, act);
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
        Event::LivingWall if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::LivingWall if screen.stage == 0 && choice_index == 0 => {
            open_event_remove_return_to_event_grid(next, Event::LivingWall);
            if next.card_grid.is_none() {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
        }
        Event::LivingWall if screen.stage == 0 && choice_index == 1 => {
            open_event_transform_return_to_event_grid(next, Event::LivingWall, 1);
            if next.card_grid.is_none() {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
        }
        Event::LivingWall if screen.stage == 0 && choice_index == 2 => {
            open_event_upgrade_return_to_event_grid(next, Event::LivingWall);
            if next.card_grid.is_none() {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}
