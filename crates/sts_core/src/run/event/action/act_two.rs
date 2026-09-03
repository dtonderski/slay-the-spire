use super::super::*;

pub(super) fn apply_act_two_event_action(
    next: &mut RunState,
    screen: &EventScreen,
    choice_index: usize,
) -> SimResult<bool> {
    match screen.event {
        Event::BackToBasics if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::BackToBasics if choice_index == 1 => {
            upgrade_starter_strikes_and_defends(next)?;
            next.event = Some(EventScreen {
                event: Event::BackToBasics,
                choices: labeled_choices(&["Leave"]),
                stage: 1,
                event_data: 0,
            });
        }
        Event::BackToBasics if choice_index == 0 => {
            // Elegance: one-card purgeable remove that returns to Leave.
            // CommunicationMod publishes for_purge=false / no confirm button and
            // resolves on the CHOOSE click (same family as Note For Yourself).
            open_event_remove_return_to_event_grid(next, Event::BackToBasics);
            if next.card_grid.is_none() {
                next.event = Some(EventScreen {
                    event: Event::BackToBasics,
                    choices: labeled_choices(&["Leave"]),
                    stage: 1,
                    event_data: 0,
                });
            }
        }
        Event::TheLibrary if screen.stage > 0 && choice_index == 0 => {
            // Flush deferred Read-path card obtain (Ceramic Fish) on Leave.
            next.flush_pending_obtain_cards()?;
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::TheLibrary if screen.stage == 0 && choice_index == 1 => {
            let heal = the_library_heal_for_ascension(next.player_max_hp, next.ascension);
            next.heal_player(heal)?;
            next.event = Some(EventScreen {
                event: Event::TheLibrary,
                choices: labeled_choices(&["Leave"]),
                stage: 1,
                event_data: 0,
            });
        }
        Event::TheLibrary if screen.stage == 0 && choice_index == 0 => {
            open_the_library_read_grid(next)?;
        }
        Event::TheMausoleum if screen.stage == 1 && choice_index == 0 => {
            next.flush_pending_obtain_cards()?;
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::TheMausoleum
            if screen.stage == 0 && choice_index == screen.choices.len().saturating_sub(1) =>
        {
            next.event = Some(EventScreen {
                event: Event::TheMausoleum,
                choices: labeled_choices(&["Leave"]),
                stage: 1,
                event_data: 0,
            });
        }
        Event::TheMausoleum if screen.stage == 0 && choice_index == 0 => {
            if roll_mausoleum_curses_player(next) {
                next.queue_pending_obtain_card(WRITHE_ID);
            }
            let act = next.current_act;
            let key = super::super::super::reward::roll_event_relic_reward(next, act);
            next.gain_relic_key(key)?;
            next.event = Some(EventScreen {
                event: Event::TheMausoleum,
                choices: labeled_choices(&["Leave"]),
                stage: 1,
                event_data: 0,
            });
        }
        Event::Vampires if screen.stage == 0 && choice_index == 0 => {
            let loss = vampires_max_hp_loss(next.player_max_hp);
            next.player_max_hp = (next.player_max_hp - loss).max(1);
            next.player_hp = next.player_hp.min(next.player_max_hp);
            replace_starter_strikes_with_bites(next)?;
            next.event = Some(EventScreen {
                event: Event::Vampires,
                choices: labeled_choices(&["Leave"]),
                stage: 1,
                event_data: 0,
            });
        }
        Event::Vampires if screen.stage == 0 && choice_index == 1 && screen.choices.len() == 3 => {
            if !next.relics.contains(&Relic::BloodVial) {
                return Err(SimError::IllegalAction(
                    "Blood Vial choice requires Blood Vial",
                ));
            }
            next.relics.retain(|relic| *relic != Relic::BloodVial);
            replace_starter_strikes_with_bites(next)?;
            next.event = Some(EventScreen {
                event: Event::Vampires,
                choices: labeled_choices(&["Leave"]),
                stage: 1,
                event_data: 0,
            });
        }
        Event::Vampires if screen.stage == 1 && choice_index == 0 => {
            next.flush_pending_obtain_cards()?;
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::Vampires
            if screen.stage == 0 && choice_index == screen.choices.len().saturating_sub(1) =>
        {
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
                lose_event_hp(next, CURSED_TOME_PAGE_1_HP_LOSS);
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
                lose_event_hp(next, CURSED_TOME_PAGE_2_HP_LOSS);
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
                lose_event_hp(next, CURSED_TOME_PAGE_3_HP_LOSS);
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
                lose_event_hp(next, hp_loss);
                let key = choose_cursed_tome_book(next);
                open_cursed_tome_book_reward(next, key);
            }
            4 if choice_index == 1 => {
                lose_event_hp(next, CURSED_TOME_STOP_HP_LOSS);
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
                lose_event_hp(next, NEST_HP_LOSS);
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
                next.event = Some(EventScreen {
                    event: Event::Beggar,
                    choices: beggar_choices(2),
                    stage: 2,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                open_event_remove_grid(next);
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
            0 if choice_index == 0 && next.gold >= ADDICT_GOLD_COST => {
                if next.gold < ADDICT_GOLD_COST {
                    return Err(SimError::IllegalAction("not enough gold"));
                }
                next.gold -= ADDICT_GOLD_COST;
                let act = next.current_act;
                let key = super::super::super::reward::roll_event_relic_reward(next, act);
                next.gain_relic_key(key)?;
                next.event = Some(EventScreen {
                    event: Event::Addict,
                    choices: addict_choices(1, next.gold),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if (choice_index == 1 && next.gold >= ADDICT_GOLD_COST)
                || (choice_index == 0 && next.gold < ADDICT_GOLD_COST) =>
            {
                // The real event uses ShowCardAndObtainEffect: the Leave screen
                // is visible before Shame is committed to masterDeck.
                next.queue_pending_obtain_card(SHAME_ID);
                let act = next.current_act;
                let key = super::super::super::reward::roll_event_relic_reward(next, act);
                next.gain_relic_key(key)?;
                next.event = Some(EventScreen {
                    event: Event::Addict,
                    choices: addict_choices(1, next.gold),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if (choice_index == 2 && next.gold >= ADDICT_GOLD_COST)
                || (choice_index == 1 && next.gold < ADDICT_GOLD_COST) =>
            {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            1 if choice_index == 0 => {
                next.flush_pending_obtain_cards()?;
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
                give_forgotten_altar_idol(next)?;
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
                lose_event_hp(next, hp_loss);
                next.event = Some(EventScreen {
                    event: Event::ForgottenAltar,
                    choices: forgotten_altar_choices(1, false),
                    stage: 1,
                    event_data: hp_loss as u32,
                });
            }
            0 if choice_index == usize::from(next.relics.contains(&Relic::GoldenIdol)) + 1 => {
                next.queue_pending_obtain_card(DECAY_ID);
                next.event = Some(EventScreen {
                    event: Event::ForgottenAltar,
                    choices: forgotten_altar_choices(1, false),
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
                    Some(if has_relic_key(next, Relic::RedMask) {
                        Relic::Circlet
                    } else {
                        Relic::RedMask
                    });
                enter_event_combat(
                    next,
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
                enter_event_combat(next, &[&SLAVER_BLUE_A0, &SLAVER_RED_A0])?;
            }
            2 if choice_index == 0 => {
                // COWARDICE: leave after the first fight. The first-fight end
                // path parks combat RNG for a possible second Colosseum bout
                // (`pending_event_combat_rng`). Discard it so a later event
                // combat (e.g. Mind Bloom) does not inherit that shuffle stream
                // (FIDL00438).
                next.pending_event_combat_rng = None;
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            2 if choice_index == 1 => {
                let event_room_override = next.current_room_override;
                next.current_room_override = Some(crate::map::RoomKind::Elite);
                enter_event_combat(next, &[&TASKMASTER_A0, &GREMLIN_NOB_A0])?;
                next.current_room_override = event_room_override;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Colosseum",
                ));
            }
        },
        Event::DrugDealer => match screen.stage {
            0 if choice_index == 0 => {
                // Drug Dealer uses ShowCardAndObtainEffect. The event advances
                // to its Leave screen before the visual effect commits J.A.X.
                // to masterDeck.
                next.queue_pending_obtain_card(JAX_ID);
                next.event = Some(EventScreen {
                    event: Event::DrugDealer,
                    choices: drug_dealer_choices(1, true),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                if transformable_event_card_count(next) < usize::from(DRUG_DEALER_TRANSFORM_COUNT) {
                    return Err(SimError::IllegalAction("not enough transformable cards"));
                }
                open_event_transform_return_to_event_grid(
                    next,
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
                if has_relic_key(next, Relic::MutagenicStrength) {
                    next.gain_relic_key(Relic::Circlet)?;
                } else {
                    next.gain_relic_key(Relic::MutagenicStrength)?;
                }
                next.event = Some(EventScreen {
                    event: Event::DrugDealer,
                    choices: drug_dealer_choices(1, true),
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
                    "event choice is not implemented for Drug Dealer",
                ));
            }
        },
        _ => return Ok(false),
    }
    Ok(true)
}
