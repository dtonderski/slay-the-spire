use super::super::*;

pub(super) fn apply_act_three_event_action(
    next: &mut RunState,
    screen: &EventScreen,
    choice_index: usize,
) -> SimResult<bool> {
    match screen.event {
        Event::Falling => match screen.stage {
            0 if choice_index == 0 => {
                roll_falling_card_choices(next)?;
                next.event = Some(EventScreen {
                    event: Event::Falling,
                    choices: falling_choices(next, 1),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 => {
                let card_types = falling_card_types(next);
                if card_types.is_empty() {
                    next.event = Some(make_event_screen(
                        Event::Falling,
                        labeled_choices(&["Leave"]),
                        2,
                    ));
                } else if let Some(card_type) = card_types.get(choice_index).copied() {
                    let selected = falling_selected_card(next, card_type)?;
                    let Some(index) = next.deck.iter().position(|card| card.id == selected.id)
                    else {
                        return Err(SimError::InvalidState(
                            "Falling selected card is missing from the deck",
                        ));
                    };
                    next.deck.remove(index);
                    next.event = Some(make_event_screen(
                        Event::Falling,
                        labeled_choices(&["Leave"]),
                        2,
                    ));
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
                    next.max_hp,
                    if next.ascension >= 15 { 0.18 } else { 0.125 },
                );
                next.max_hp = (next.max_hp - loss).max(1);
                next.hp = next.hp.min(next.max_hp);
                next.heal_player(next.max_hp)?;
                next.event = Some(make_event_screen(Event::MoaiHead, moai_choices(next, 1), 1));
            }
            0 if has_relic_key(next, Relic::GoldenIdol) && choice_index == 1 => {
                remove_relic_key(next, Relic::GoldenIdol)?;
                next.gain_gold(333)?;
                next.event = Some(make_event_screen(Event::MoaiHead, moai_choices(next, 1), 1));
            }
            0 if choice_index == screen.choices.len().saturating_sub(1) => {
                next.event = Some(make_event_screen(Event::MoaiHead, moai_choices(next, 1), 1));
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
                // CommMod shows an intermediate Leave page before returning to map.
                next.event = Some(make_event_screen(
                    Event::MysteriousSphere,
                    mysterious_sphere_choices(2),
                    2,
                ));
            }
            1 if choice_index == 0 => {
                let mut misc_rng = next.rng_for_stream(RunRngStream::Misc);
                next.pending_event_combat_gold_offer = misc_rng.random_int_range(45, 55);
                next.store_rng_counter(RunRngStream::Misc, &misc_rng);
                next.pending_event_combat_relic_offer = Some(
                    super::super::super::reward::roll_relic_reward(next, RelicTier::Rare),
                );
                // Elite-like rewards, but room stays EventRoom — Slaver's Collar
                // does not fire (FIDL00228: energy is base+Lantern only).
                enter_event_combat(next, &[&ORB_WALKER_A0, &ORB_WALKER_A0])?;
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Mysterious Sphere",
                ));
            }
        },
        Event::SensoryStone if screen.stage == 0 && choice_index == 0 => {
            let memory = roll_sensory_memory(next);
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
                lose_event_hp(next, hp_loss);
            }
            let reward_count = u8::try_from(choice_index + 1)
                .expect("Sensory Stone offers at most three card rewards");
            let card_choice_count = reward_card_choice_count(next);
            let total_card_choices = usize::from(reward_count)
                .checked_mul(card_choice_count)
                .ok_or(SimError::InvalidState(
                    "Sensory Stone card choice count overflows usize",
                ))?;
            let mut next_card_id = next.reserve_card_instance_ids(total_card_choices)?;
            let mut card_rng = next.rng_for_stream(RunRngStream::CardReward);
            let mut queued_card_rewards = Vec::with_capacity(usize::from(reward_count));
            for _ in 0..reward_count {
                let cards = target_colorless_event_card_reward_choices_with_count(
                    &mut card_rng,
                    &mut next.card_rarity_factor,
                    next_card_id,
                    card_choice_count,
                )
                .into_iter()
                .map(|mut card| {
                    // Sensory Stone creates RewardItems through the same
                    // preview-obtain path as other visible card rewards.
                    // Egg relics therefore upgrade the displayed choice
                    // before the reward screen is opened.
                    card.content_id = next.content_id_after_card_add_relics(card.content_id)?;
                    Ok(card)
                })
                .collect::<SimResult<Vec<_>>>()?;
                next_card_id += cards.len() as u64;
                queued_card_rewards.push(cards);
            }
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
                    choices: winding_halls_choices(next, 1),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                let loss = rounded_event_percent(
                    next.max_hp,
                    if next.ascension >= 15 { 0.18 } else { 0.125 },
                );
                lose_event_hp(next, loss);
                next.queue_pending_obtain_card(MADNESS_ID);
                next.queue_pending_obtain_card(MADNESS_ID);
                next.event = Some(make_event_screen(
                    Event::WindingHalls,
                    winding_halls_choices(next, 2),
                    2,
                ));
            }
            1 if choice_index == 1 => {
                let heal = rounded_event_percent(
                    next.max_hp,
                    if next.ascension >= 15 { 0.20 } else { 0.25 },
                );
                next.heal_player(heal)?;
                next.queue_pending_obtain_card(WRITHE_ID);
                next.event = Some(make_event_screen(
                    Event::WindingHalls,
                    winding_halls_choices(next, 2),
                    2,
                ));
            }
            1 if choice_index == 2 => {
                let loss = rounded_event_percent(next.max_hp, 0.05);
                next.max_hp = (next.max_hp - loss).max(1);
                next.hp = next.hp.min(next.max_hp);
                next.event = Some(make_event_screen(
                    Event::WindingHalls,
                    winding_halls_choices(next, 2),
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
        Event::TombOfLordRedMask if screen.stage == 0 && choice_index == 0 => {
            if has_relic_key(next, Relic::RedMask) {
                next.gain_gold(222)?;
            } else {
                next.gold = 0;
                next.gain_relic_key(Relic::RedMask)?;
            }
            next.event = Some(make_event_screen(
                Event::TombOfLordRedMask,
                tomb_of_lord_red_mask_choices(next, 1),
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
            let boss = roll_mind_bloom_boss(next);
            next.pending_event_combat_gold_offer = if next.ascension >= 13 { 25 } else { 50 };
            next.pending_event_combat_relic_offer = Some(
                super::super::super::reward::roll_relic_reward(next, RelicTier::Rare),
            );
            let event_room_override = next.current_room_override;
            next.current_room_override = Some(crate::map::RoomKind::Boss);
            match boss {
                0 => enter_event_combat(next, &[&GUARDIAN_A0])?,
                1 => enter_event_combat(next, &[&HEXAGHOST_A0])?,
                _ => enter_event_combat(next, &[&SLIME_BOSS_A0])?,
            }
            next.current_room_override = event_room_override;
        }
        Event::MindBloom if screen.stage == 0 && choice_index == 1 => {
            for card in &mut next.deck {
                if let Some(upgraded) = upgrade_card_instance(*card)? {
                    *card = upgraded;
                }
            }
            next.gain_relic_key(Relic::MarkOfBloom)?;
            next.event = Some(make_event_screen(
                Event::MindBloom,
                labeled_choices(&["Leave"]),
                1,
            ));
        }
        Event::MindBloom if screen.stage == 0 && choice_index == 2 => {
            if next.current_floor % 50 <= 40 {
                next.gain_gold(999)?;
                next.queue_pending_obtain_card(NORMALITY_ID);
                next.queue_pending_obtain_card(NORMALITY_ID);
            } else {
                next.heal_player(next.max_hp)?;
                // Mind Bloom uses ShowCardAndObtainEffect. The Leave screen is
                // observable before the curse reaches the master deck.
                next.queue_pending_obtain_card(DOUBT_ID);
            }
            next.event = Some(make_event_screen(
                Event::MindBloom,
                labeled_choices(&["Leave"]),
                1,
            ));
        }
        Event::MindBloom if screen.stage == 1 && choice_index == 0 => {
            next.flush_pending_obtain_cards()?;
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        _ => return Ok(false),
    }
    Ok(true)
}
