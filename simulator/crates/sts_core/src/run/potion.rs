use crate::{
    card::{CardInstance, CardType},
    combat::damage::deal_unmodified_damage_to_monster,
    combat::transition::{
        choose_hand_select, confirm_hand_select, hand_select_ui_to_hand_index, player_draw_cards,
    },
    combat::CombatPhase,
    content::cards::upgrade_content_id,
    content::shop_pool::{colorless_discovery_card_choices, discovery_card_choices},
    ids::CardId,
    potion::{
        Potion, ANCIENT_POTION_ARTIFACT, BLOCK_POTION_BLOCK, BLOOD_POTION_HEAL_PERCENT,
        CULTIST_POTION_RITUAL, DEXTERITY_POTION_DEXTERITY, ENERGY_POTION_ENERGY,
        ESSENCE_OF_STEEL_PLATED_ARMOR, EXPLOSIVE_POTION_DAMAGE, FEAR_POTION_WEAK,
        FIRE_POTION_DAMAGE, FLEX_POTION_TEMP_STRENGTH, FRUIT_JUICE_MAX_HP, GAMBLE_POTION_LOSS_GOLD,
        GAMBLE_POTION_WIN_GOLD, HEART_OF_IRON_METALLICIZE, LIQUID_BRONZE_THORNS,
        REGEN_POTION_REGEN, SPEED_POTION_TEMP_DEXTERITY, STRENGTH_POTION_STRENGTH,
        SWIFT_POTION_DRAW, WEAK_POTION_WEAK,
    },
    rng::{RngStream, SimulatorRng},
    run::reward::target_random_potion,
    RunAction, RunPhase, RunState, SimError, SimResult,
};

pub fn validate_potion_action(run: &RunState, action: RunAction) -> SimResult<()> {
    match action {
        RunAction::UsePotion { slot, target } => {
            let potion = run
                .potions
                .get(slot)
                .ok_or(SimError::IllegalAction("potion slot is not available"))?;

            if potion.requires_combat() {
                if run.phase != RunPhase::Combat {
                    return Err(SimError::IllegalAction("potion use requires combat phase"));
                }
                let combat = run
                    .combat
                    .as_ref()
                    .ok_or(SimError::InvalidState("combat state is missing"))?;

                if potion.requires_target() {
                    let Some(target) = target else {
                        return Err(SimError::IllegalAction("potion requires a target"));
                    };
                    if !combat
                        .monsters
                        .iter()
                        .any(|monster| monster.id == target && monster.alive)
                    {
                        return Err(SimError::IllegalAction("potion target is not alive"));
                    }
                } else if target.is_some() {
                    return Err(SimError::IllegalAction("potion does not take a target"));
                }
            } else if target.is_some() {
                return Err(SimError::IllegalAction("potion does not take a target"));
            }

            Ok(())
        }
        RunAction::DiscardPotion { slot } => {
            run.potions
                .get(slot)
                .ok_or(SimError::IllegalAction("potion slot is not available"))?;
            Ok(())
        }
        RunAction::ChooseCombatCardReward { index } => {
            validate_combat_card_reward_choice(run, index)
        }
        RunAction::ChooseHandSelect { index } => validate_hand_select_choice(run, index),
        RunAction::ConfirmHandSelect => validate_hand_select_confirm(run),
        _ => Err(SimError::IllegalAction("not a potion action")),
    }
}

pub fn validate_combat_card_reward_choice(run: &RunState, index: usize) -> SimResult<()> {
    let combat = run.combat.as_ref().ok_or(SimError::IllegalAction(
        "combat card reward requires combat",
    ))?;
    let choices = combat
        .potion_card_reward
        .as_ref()
        .ok_or(SimError::IllegalAction("no combat card reward is open"))?;
    if index >= choices.len() {
        return Err(SimError::IllegalAction(
            "combat card reward index out of range",
        ));
    }
    Ok(())
}

pub fn validate_hand_select_choice(run: &RunState, index: usize) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("hand select requires combat"))?;
    hand_select_ui_to_hand_index(combat, index)?;
    Ok(())
}

pub fn validate_hand_select_confirm(run: &RunState) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("hand select requires combat"))?;
    let hand_select = combat
        .hand_select
        .as_ref()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    if hand_select.selected_hand_index.is_none() {
        return Err(SimError::IllegalAction("hand select choice is required"));
    }
    Ok(())
}

pub fn apply_hand_select_choice(run: &RunState, index: usize) -> SimResult<RunState> {
    validate_hand_select_choice(run, index)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    choose_hand_select(combat, index)?;
    Ok(next)
}

pub fn apply_hand_select_confirm(run: &RunState) -> SimResult<RunState> {
    validate_hand_select_confirm(run)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    confirm_hand_select(combat)?;
    Ok(next)
}

pub fn apply_combat_card_reward_choice(run: &RunState, index: usize) -> SimResult<RunState> {
    validate_combat_card_reward_choice(run, index)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    let choices = combat.potion_card_reward.take().expect("validated reward");
    let choice = choices[index];
    let card_id = CardId::new(combat.piles.max_card_instance_id() + 1);
    combat.piles.hand.push(CardInstance::combat_generated(
        card_id,
        choice.content_id,
        0,
    ));
    Ok(next)
}

pub fn apply_potion_action(run: &RunState, action: RunAction) -> SimResult<RunState> {
    validate_potion_action(run, action)?;

    let mut next = run.clone();
    match action {
        RunAction::UsePotion { slot, target } => {
            let potion = next.potions.remove(slot);
            match potion {
                Potion::Fire => {
                    let target = target.expect("validated fire potion target");
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let monster = combat
                        .monsters
                        .iter_mut()
                        .find(|monster| monster.id == target)
                        .expect("validated potion target");
                    deal_unmodified_damage_to_monster(monster, FIRE_POTION_DAMAGE);
                    if combat.monsters.iter().all(|monster| !monster.alive) {
                        combat.phase = CombatPhase::Won;
                    }
                }
                Potion::Block => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.block += BLOCK_POTION_BLOCK;
                }
                Potion::Fear => {
                    let target = target.expect("validated fear potion target");
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let monster = combat
                        .monsters
                        .iter_mut()
                        .find(|monster| monster.id == target)
                        .expect("validated potion target");
                    monster.powers.weak += FEAR_POTION_WEAK;
                }
                Potion::Blood => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let heal = combat.player.max_hp * BLOOD_POTION_HEAL_PERCENT / 100;
                    combat.player.hp = (combat.player.hp + heal).min(combat.player.max_hp);
                }
                Potion::Ancient => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.artifact += ANCIENT_POTION_ARTIFACT;
                }
                Potion::HeartOfIron => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.metallicize += HEART_OF_IRON_METALLICIZE;
                }
                Potion::Cultist => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.ritual += CULTIST_POTION_RITUAL;
                }
                Potion::Dexterity => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.dexterity += DEXTERITY_POTION_DEXTERITY;
                }
                Potion::Energy => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.energy += ENERGY_POTION_ENERGY;
                }
                Potion::EssenceOfSteel => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.plated_armor += ESSENCE_OF_STEEL_PLATED_ARMOR;
                }
                Potion::Explosive => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    for monster in combat.monsters.iter_mut().filter(|monster| monster.alive) {
                        deal_unmodified_damage_to_monster(monster, EXPLOSIVE_POTION_DAMAGE);
                    }
                    if combat.monsters.iter().all(|monster| !monster.alive) {
                        combat.phase = CombatPhase::Won;
                    }
                }
                Potion::LiquidBronze => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.thorns += LIQUID_BRONZE_THORNS;
                }
                Potion::Regen => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.regen += REGEN_POTION_REGEN;
                }
                Potion::Strength => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.strength += STRENGTH_POTION_STRENGTH;
                }
                Potion::Flex => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.temp_strength += FLEX_POTION_TEMP_STRENGTH;
                }
                Potion::Speed => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.dexterity += SPEED_POTION_TEMP_DEXTERITY;
                    combat.player.temp_dexterity += SPEED_POTION_TEMP_DEXTERITY;
                }
                Potion::Swift => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    player_draw_cards(combat, SWIFT_POTION_DRAW);
                }
                Potion::BlessingOfTheForge => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    for card in &mut combat.piles.hand {
                        if let Some(upgraded) = upgrade_content_id(card.content_id) {
                            card.content_id = upgraded;
                        }
                    }
                }
                Potion::Weak => {
                    let target = target.expect("validated weak potion target");
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let monster = combat
                        .monsters
                        .iter_mut()
                        .find(|monster| monster.id == target)
                        .expect("validated potion target");
                    monster.powers.weak += WEAK_POTION_WEAK;
                }
                Potion::FruitJuice => {
                    next.player_max_hp += FRUIT_JUICE_MAX_HP;
                    next.player_hp += FRUIT_JUICE_MAX_HP;
                    if let Some(combat) = next.combat.as_mut() {
                        combat.player.max_hp += FRUIT_JUICE_MAX_HP;
                        combat.player.hp += FRUIT_JUICE_MAX_HP;
                    }
                }
                Potion::Gamble => {
                    let mut rng = SimulatorRng::new(next.potion_rng_seed);
                    let win = rng.next_bool(RngStream::Potion, "gamble_potion");
                    next.potion_rng_seed = rng.seed_state();
                    if win {
                        next.gold += GAMBLE_POTION_WIN_GOLD;
                    } else {
                        next.gold = (next.gold - GAMBLE_POTION_LOSS_GOLD).max(0);
                    }
                }
                Potion::EntropicBrew => {
                    let mut rng = crate::rng::StsRng::with_counter(
                        next.potion_rng_seed as i64,
                        next.potion_rng_counter,
                    );
                    while next.potions.len() < crate::potion::MAX_POTIONS {
                        next.potions.push(target_random_potion(&mut rng));
                    }
                    next.potion_rng_counter = rng.counter();
                }
                Potion::Attack | Potion::Skill | Potion::Colorless | Potion::Power => {
                    let mut rng = next.card_random_rng();
                    let content_ids = match potion {
                        Potion::Attack => discovery_card_choices(&mut rng, CardType::Attack, 3),
                        Potion::Skill => discovery_card_choices(&mut rng, CardType::Skill, 3),
                        Potion::Colorless => colorless_discovery_card_choices(&mut rng, 3),
                        Potion::Power => discovery_card_choices(&mut rng, CardType::Power, 3),
                        _ => unreachable!("matched discovery potion"),
                    };
                    next.card_random_rng_counter = rng.counter();
                    let next_card_id = next.next_card_instance_id();
                    let reward_cards = content_ids
                        .into_iter()
                        .enumerate()
                        .map(|(index, content_id)| {
                            CardInstance::new(CardId::new(next_card_id + index as u64), content_id)
                        })
                        .collect();
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.potion_card_reward = Some(reward_cards);
                }
                _ => {
                    return Err(SimError::IllegalAction(
                        "potion mechanics are not implemented",
                    ));
                }
            }
            let won = next
                .combat
                .as_ref()
                .map(|combat| combat.phase == CombatPhase::Won)
                .unwrap_or(false);
            if won {
                super::reward::enter_reward_screen(&mut next);
            }
        }
        RunAction::DiscardPotion { slot } => {
            next.potions.remove(slot);
        }
        _ => unreachable!("validated potion action"),
    }

    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MonsterId;

    #[test]
    fn fire_potion_deals_twenty_damage_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Fire);
        let monster_id = run.combat.as_ref().expect("combat").monsters[0].id;
        let hp_before = run.combat.as_ref().expect("combat").monsters[0].hp;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(monster_id),
            },
        )
        .expect("use fire potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.monsters[0].hp, hp_before - FIRE_POTION_DAMAGE);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn block_potion_grants_twelve_block_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Block);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use block potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.player.block, BLOCK_POTION_BLOCK);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn gamble_potion_wins_gold_deterministically_for_seed() {
        let mut run = RunState::map_fixture();
        run.potion_rng_seed = 42;
        run.potions.push(Potion::Gamble);
        let gold_before = run.gold;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use gamble potion");

        assert_ne!(after.potion_rng_seed, run.potion_rng_seed);
        assert!(
            after.gold == gold_before + GAMBLE_POTION_WIN_GOLD
                || after.gold == (gold_before - GAMBLE_POTION_LOSS_GOLD).max(0)
        );
        assert!(after.potions.is_empty());
    }

    #[test]
    fn gamble_potion_round_trips_rng_seed_through_run_json() {
        let mut run = RunState::map_fixture();
        run.potion_rng_seed = 99;
        run.potions.push(Potion::Gamble);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use gamble potion");

        let json = serde_json::to_string(&after).expect("run serializes");
        let restored: RunState = serde_json::from_str(&json).expect("run deserializes");
        assert_eq!(restored.potion_rng_seed, after.potion_rng_seed);
    }

    #[test]
    fn entropic_brew_fills_empty_potion_slots_and_advances_rng_counter() {
        let mut run = RunState::map_fixture();
        run.potion_rng_seed = 22_079_335_079;
        run.potion_rng_counter = 0;
        run.potions.push(Potion::EntropicBrew);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use entropic brew");

        assert_eq!(after.potions.len(), crate::potion::MAX_POTIONS);
        assert!(after.potion_rng_counter > run.potion_rng_counter);
    }

    #[test]
    fn entropic_brew_preserves_existing_potions_while_refilling_open_slots() {
        let mut run = RunState::map_fixture();
        run.potion_rng_seed = 22_079_335_079;
        run.potions = vec![Potion::Fire, Potion::EntropicBrew, Potion::Block];

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 1,
                target: None,
            },
        )
        .expect("use entropic brew from full belt");

        assert_eq!(after.potions.len(), crate::potion::MAX_POTIONS);
        assert_eq!(after.potions[0], Potion::Fire);
        assert_eq!(after.potions[1], Potion::Block);
    }

    #[test]
    fn gamble_potion_works_outside_combat() {
        let mut run = RunState::map_fixture();
        run.potions.push(Potion::Gamble);

        apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("gamble outside combat");
    }

    #[test]
    fn fear_potion_applies_weak_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Fear);
        let monster_id = run.combat.as_ref().expect("combat").monsters[0].id;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(monster_id),
            },
        )
        .expect("use fear potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.monsters[0].powers.weak, FEAR_POTION_WEAK);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn blood_potion_heals_twenty_percent_of_max_hp_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Blood);
        let combat = run.combat.as_mut().expect("combat");
        combat.player.hp = 50;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use blood potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.player.hp, 66);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn ancient_potion_grants_artifact_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Ancient);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use ancient potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.player.powers.artifact, ANCIENT_POTION_ARTIFACT);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn heart_of_iron_grants_metallicize_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::HeartOfIron);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use heart of iron");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.player.powers.metallicize, HEART_OF_IRON_METALLICIZE);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn cultist_potion_grants_ritual_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Cultist);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use cultist potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.player.powers.ritual, CULTIST_POTION_RITUAL);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn cultist_potion_ritual_grants_strength_at_end_of_turn() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Cultist);
        run.combat.as_mut().expect("combat").piles.draw_pile.clear();

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use cultist potion");
        let combat = crate::combat::turn::end_player_turn(after.combat.as_ref().expect("combat"));

        assert_eq!(combat.player.powers.strength, CULTIST_POTION_RITUAL);
    }

    #[test]
    fn dexterity_potion_grants_dexterity_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Dexterity);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use dexterity potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.player.powers.dexterity, DEXTERITY_POTION_DEXTERITY);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn energy_potion_grants_two_energy_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Energy);
        run.combat.as_mut().expect("combat").player.energy = 1;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use energy potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.player.energy, 1 + ENERGY_POTION_ENERGY);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn explosive_potion_hits_all_living_monsters_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.combat = Some(crate::combat::CombatState::sentry_fixture());
        run.potions.push(Potion::Explosive);
        let hp_before: Vec<_> = run
            .combat
            .as_ref()
            .expect("combat")
            .monsters
            .iter()
            .map(|monster| monster.hp)
            .collect();

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use explosive potion");

        let combat = after.combat.expect("combat continues");
        for (monster, before) in combat.monsters.iter().zip(hp_before) {
            assert_eq!(monster.hp, before - EXPLOSIVE_POTION_DAMAGE);
        }
        assert!(after.potions.is_empty());
    }

    #[test]
    fn essence_of_steel_grants_plated_armor_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::EssenceOfSteel);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use essence of steel");

        let combat = after.combat.expect("combat continues");
        assert_eq!(
            combat.player.powers.plated_armor,
            ESSENCE_OF_STEEL_PLATED_ARMOR
        );
        assert!(after.potions.is_empty());
    }

    #[test]
    fn regen_potion_grants_regen_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Regen);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use regen potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.player.powers.regen, REGEN_POTION_REGEN);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn regen_potion_heals_at_end_of_player_turn() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Regen);
        let combat = run.combat.as_mut().expect("combat");
        combat.player.hp = 70;
        combat.piles.draw_pile.clear();

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use regen potion");
        let combat = crate::combat::turn::end_player_turn(after.combat.as_ref().expect("combat"));

        assert_eq!(combat.player.hp, 69);
        assert_eq!(combat.player.powers.regen, REGEN_POTION_REGEN - 1);
    }

    #[test]
    fn liquid_bronze_grants_thorns_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::LiquidBronze);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use liquid bronze");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.player.powers.thorns, LIQUID_BRONZE_THORNS);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn strength_potion_grants_strength_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Strength);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use strength potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.player.powers.strength, STRENGTH_POTION_STRENGTH);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn flex_potion_grants_temp_strength_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Flex);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use flex potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.player.temp_strength, FLEX_POTION_TEMP_STRENGTH);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn speed_potion_grants_temp_dexterity_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Speed);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use speed potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.player.powers.dexterity, SPEED_POTION_TEMP_DEXTERITY);
        assert_eq!(combat.player.temp_dexterity, SPEED_POTION_TEMP_DEXTERITY);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn speed_potion_dexterity_clears_on_next_player_turn() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Speed);
        run.combat.as_mut().expect("combat").piles.draw_pile.clear();

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use speed potion");
        let combat = crate::combat::turn::end_player_turn(after.combat.as_ref().expect("combat"));

        assert_eq!(combat.player.powers.dexterity, 0);
        assert_eq!(combat.player.temp_dexterity, 0);
    }

    #[test]
    fn swift_potion_draws_three_cards_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Swift);
        let combat = run.combat.as_mut().expect("combat");
        combat.piles.hand.clear();
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), crate::content::cards::STRIKE_R_ID),
            CardInstance::new(CardId::new(11), crate::content::cards::DEFEND_R_ID),
            CardInstance::new(CardId::new(12), crate::content::cards::BASH_ID),
        ];

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use swift potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.piles.hand.len(), SWIFT_POTION_DRAW);
        assert!(combat.piles.draw_pile.is_empty());
        assert!(after.potions.is_empty());
    }

    #[test]
    fn blessing_of_the_forge_upgrades_hand_and_is_consumed() {
        use crate::content::cards::{ANGER_ID, ANGER_PLUS_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID};

        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::BlessingOfTheForge);
        let combat = run.combat.as_mut().expect("combat");
        combat.piles.hand = vec![
            CardInstance::new(CardId::new(10), STRIKE_R_ID),
            CardInstance::new(CardId::new(11), ANGER_ID),
            CardInstance::new(CardId::new(12), STRIKE_R_PLUS_ID),
        ];

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use blessing of the forge");

        let combat = after.combat.expect("combat continues");
        let hand: Vec<_> = combat
            .piles
            .hand
            .iter()
            .map(|card| card.content_id)
            .collect();
        assert_eq!(
            hand,
            vec![STRIKE_R_PLUS_ID, ANGER_PLUS_ID, STRIKE_R_PLUS_ID]
        );
        assert!(after.potions.is_empty());
    }

    #[test]
    fn weak_potion_applies_weak_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Weak);
        let monster_id = run.combat.as_ref().expect("combat").monsters[0].id;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(monster_id),
            },
        )
        .expect("use weak potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.monsters[0].powers.weak, WEAK_POTION_WEAK);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn fruit_juice_increases_max_hp_outside_combat_and_is_consumed() {
        let mut run = RunState::map_fixture();
        run.potions.push(Potion::FruitJuice);
        let max_hp_before = run.player_max_hp;
        let current_hp_before = run.player_hp;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use fruit juice");

        assert_eq!(after.player_max_hp, max_hp_before + FRUIT_JUICE_MAX_HP);
        assert_eq!(after.player_hp, current_hp_before + FRUIT_JUICE_MAX_HP);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn block_potion_rejects_target() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Block);
        let monster_id = run.combat.as_ref().expect("combat").monsters[0].id;

        let err = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(monster_id),
            },
        )
        .expect_err("block potion rejects target");

        assert_eq!(
            err,
            SimError::IllegalAction("potion does not take a target")
        );
    }

    #[test]
    fn fire_potion_can_win_combat_and_enter_rewards() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Fire);
        let combat = run.combat.as_mut().expect("combat");
        combat.monsters[0].hp = FIRE_POTION_DAMAGE;
        let monster_id = combat.monsters[0].id;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(monster_id),
            },
        )
        .expect("use lethal fire potion");

        assert_eq!(after.phase, RunPhase::Reward);
        assert!(after.combat.is_none());
        assert!(after.reward.is_some());
        assert!(after.potions.is_empty());
    }

    #[test]
    fn discard_potion_removes_selected_slot() {
        let mut run = RunState::map_fixture();
        run.potions = vec![Potion::Fire, Potion::Fire];

        let after = apply_potion_action(&run, RunAction::DiscardPotion { slot: 0 })
            .expect("discard potion");

        assert_eq!(after.potions, vec![Potion::Fire]);
    }

    #[test]
    fn use_potion_rejects_missing_slot() {
        let run = RunState::combat_fixture();
        let monster_id = run.combat.as_ref().expect("combat").monsters[0].id;

        let err = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(monster_id),
            },
        )
        .expect_err("no potion");

        assert_eq!(err, SimError::IllegalAction("potion slot is not available"));
    }

    #[test]
    fn use_potion_rejects_dead_target() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Fire);

        let err = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(MonsterId::new(999)),
            },
        )
        .expect_err("bad target");

        assert_eq!(err, SimError::IllegalAction("potion target is not alive"));
    }

    #[test]
    fn fire_potion_rejects_missing_target() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Fire);

        let err = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect_err("fire potion needs target");

        assert_eq!(err, SimError::IllegalAction("potion requires a target"));
    }

    #[test]
    fn attack_skill_and_colorless_potions_open_unique_card_choices() {
        for potion in [Potion::Attack, Potion::Skill, Potion::Colorless] {
            let mut run = RunState::combat_fixture();
            run.potions.push(potion);

            let after = apply_potion_action(
                &run,
                RunAction::UsePotion {
                    slot: 0,
                    target: None,
                },
            )
            .expect("use discovery potion");

            let choices = after
                .combat
                .as_ref()
                .expect("combat")
                .potion_card_reward
                .as_ref()
                .expect("card choices open");
            assert_eq!(choices.len(), 3);
            assert_ne!(choices[0].content_id, choices[1].content_id);
            assert_ne!(choices[0].content_id, choices[2].content_id);
            assert_ne!(choices[1].content_id, choices[2].content_id);
            assert!(after.potions.is_empty());
        }
    }

    #[test]
    fn attack_potion_choice_adds_generated_card_to_hand() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Attack);
        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use attack potion");
        let expected = after
            .combat
            .as_ref()
            .expect("combat")
            .potion_card_reward
            .as_ref()
            .expect("card choices")[0]
            .content_id;

        let after = apply_combat_card_reward_choice(&after, 0).expect("choose generated card");

        let hand = &after.combat.as_ref().expect("combat").piles.hand;
        assert!(hand.iter().any(|card| card.content_id == expected));
        assert!(after
            .combat
            .as_ref()
            .expect("combat")
            .potion_card_reward
            .is_none());
    }

    #[test]
    fn power_potion_discovery_matches_test_trace_floor_six() {
        use crate::content::cards::{
            get_card_definition, BERSERK_ID, DEMON_FORM_ID, FEEL_NO_PAIN_ID,
        };
        use crate::content::shop_pool::discovery_card_choices;
        use crate::rng::StsRng;

        let mut rng = StsRng::new(1_218_623 + 6);
        let choices = discovery_card_choices(&mut rng, CardType::Power, 3);
        assert_eq!(
            choices,
            vec![DEMON_FORM_ID, BERSERK_ID, FEEL_NO_PAIN_ID],
            "unexpected discovery choices for TEST Lagavulin Power Potion"
        );
        assert!(
            get_card_definition(DEMON_FORM_ID).is_some(),
            "chosen Demon Form should be playable"
        );
    }
}
