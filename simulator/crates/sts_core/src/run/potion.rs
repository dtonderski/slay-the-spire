use crate::{
    card::{CardInstance, CardType, TargetRequirement},
    combat::damage::deal_unmodified_damage_to_monster,
    combat::transition::{
        apply_play_top_draw_card_action, choose_discard_select, choose_draw_select,
        choose_exhaust_select, choose_hand_select, confirm_discard_select, confirm_draw_select,
        confirm_exhaust_select, confirm_hand_select, discard_select_ui_to_discard_index,
        draw_select_ui_to_draw_index, exhaust_select_ui_to_hand_index,
        hand_select_ui_to_hand_index, open_discard_select, open_exhaust_select, player_draw_cards,
        top_draw_card_definition,
    },
    combat::{CombatPhase, CombatState, ExhaustSelectPurpose},
    content::cards::{get_card_definition, upgrade_content_id},
    content::shop_pool::{colorless_discovery_card_choices, discovery_card_choices},
    ids::{CardId, MonsterId},
    map::RoomKind,
    potion::{
        Potion, ANCIENT_POTION_ARTIFACT, BLOCK_POTION_BLOCK, BLOOD_POTION_HEAL_PERCENT,
        CULTIST_POTION_RITUAL, DEXTERITY_POTION_DEXTERITY, ENERGY_POTION_ENERGY,
        ESSENCE_OF_STEEL_PLATED_ARMOR, EXPLOSIVE_POTION_DAMAGE, FEAR_POTION_WEAK,
        FIRE_POTION_DAMAGE, FLEX_POTION_TEMP_STRENGTH, FRUIT_JUICE_MAX_HP, GAMBLE_POTION_LOSS_GOLD,
        GAMBLE_POTION_WIN_GOLD, HEART_OF_IRON_METALLICIZE, LIQUID_BRONZE_THORNS,
        REGEN_POTION_REGEN, SNECKO_OIL_DRAW, SPEED_POTION_TEMP_DEXTERITY, STRENGTH_POTION_STRENGTH,
        SWIFT_POTION_DRAW, WEAK_POTION_WEAK,
    },
    power::apply_monster_weak,
    rng::{RngStream, SimulatorRng, StsRng},
    run::reward::{apply_dead_branch_for_exhaust_count, target_random_potion},
    RunAction, RunPhase, RunState, SimError, SimResult,
};

pub fn validate_potion_action(run: &RunState, action: RunAction) -> SimResult<()> {
    match action {
        RunAction::UsePotion { slot, target } => {
            let potion = run
                .potions
                .get(slot)
                .ok_or(SimError::IllegalAction("potion slot is not available"))?;

            if *potion == Potion::Fairy {
                return Err(SimError::IllegalAction("Fairy is passive"));
            }

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
                if *potion == Potion::SmokeBomb && current_room_kind(run) == Some(RoomKind::Boss) {
                    return Err(SimError::IllegalAction(
                        "Smoke Bomb cannot be used in boss combat",
                    ));
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
        RunAction::ChooseDrawSelect { index } => validate_draw_select_choice(run, index),
        RunAction::ConfirmDrawSelect => validate_draw_select_confirm(run),
        RunAction::ChooseDiscardSelect { index } => validate_discard_select_choice(run, index),
        RunAction::ConfirmDiscardSelect => validate_discard_select_confirm(run),
        RunAction::ChooseExhaustSelect { index } => validate_exhaust_select_choice(run, index),
        RunAction::ConfirmExhaustSelect => validate_exhaust_select_confirm(run),
        _ => Err(SimError::IllegalAction("not a potion action")),
    }
}

fn current_room_kind(run: &RunState) -> Option<RoomKind> {
    run.map.as_ref().and_then(|map_state| {
        map_state
            .map
            .node(map_state.current_node)
            .map(|node| node.room_kind)
    })
}

pub fn validate_combat_card_reward_choice(run: &RunState, index: usize) -> SimResult<()> {
    let combat = run.combat.as_ref().ok_or(SimError::IllegalAction(
        "combat card reward requires combat",
    ))?;
    let choices = combat
        .potion_card_reward
        .as_ref()
        .or(combat.toolbox_card_reward.as_ref())
        .or(combat.discovery_card_reward.as_ref())
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

pub fn validate_draw_select_choice(run: &RunState, index: usize) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("draw select requires combat"))?;
    draw_select_ui_to_draw_index(combat, index)?;
    Ok(())
}

pub fn validate_draw_select_confirm(run: &RunState) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("draw select requires combat"))?;
    let draw_select = combat
        .draw_select
        .as_ref()
        .ok_or(SimError::IllegalAction("no draw select is open"))?;
    if draw_select.selected_draw_index.is_none() {
        return Err(SimError::IllegalAction("draw select choice is required"));
    }
    Ok(())
}

pub fn validate_discard_select_choice(run: &RunState, index: usize) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("discard select requires combat"))?;
    discard_select_ui_to_discard_index(combat, index)?;
    Ok(())
}

pub fn validate_discard_select_confirm(run: &RunState) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("discard select requires combat"))?;
    let discard_select = combat
        .discard_select
        .as_ref()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    if discard_select.selected_discard_index.is_none() {
        return Err(SimError::IllegalAction("discard select choice is required"));
    }
    Ok(())
}

pub fn validate_exhaust_select_choice(run: &RunState, index: usize) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("exhaust select requires combat"))?;
    exhaust_select_ui_to_hand_index(combat, index)?;
    Ok(())
}

pub fn validate_exhaust_select_confirm(run: &RunState) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("exhaust select requires combat"))?;
    let exhaust_select = combat
        .exhaust_select
        .as_ref()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    if exhaust_select.purpose == ExhaustSelectPurpose::ExhumeReturnToHand
        && exhaust_select.selected_hand_indices.is_empty()
    {
        return Err(SimError::IllegalAction("exhaust select choice is required"));
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
    let mut combat = next.combat.take().expect("validated combat");
    let exhaust_before = combat.piles.exhaust_pile.len();
    confirm_hand_select(&mut combat)?;
    let exhaust_count = combat
        .piles
        .exhaust_pile
        .len()
        .saturating_sub(exhaust_before);
    apply_dead_branch_for_exhaust_count(&mut next, &mut combat, exhaust_count);
    next.combat = Some(combat);
    Ok(next)
}

pub fn apply_draw_select_choice(run: &RunState, index: usize) -> SimResult<RunState> {
    validate_draw_select_choice(run, index)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    choose_draw_select(combat, index)?;
    Ok(next)
}

pub fn apply_draw_select_confirm(run: &RunState) -> SimResult<RunState> {
    validate_draw_select_confirm(run)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    let exhaust_before = combat.piles.exhaust_pile.len();
    confirm_draw_select(&mut combat)?;
    let exhaust_count = combat
        .piles
        .exhaust_pile
        .len()
        .saturating_sub(exhaust_before);
    apply_dead_branch_for_exhaust_count(&mut next, &mut combat, exhaust_count);
    next.combat = Some(combat);
    Ok(next)
}

pub fn apply_discard_select_choice(run: &RunState, index: usize) -> SimResult<RunState> {
    validate_discard_select_choice(run, index)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    choose_discard_select(combat, index)?;
    Ok(next)
}

pub fn apply_discard_select_confirm(run: &RunState) -> SimResult<RunState> {
    validate_discard_select_confirm(run)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    confirm_discard_select(combat)?;
    Ok(next)
}

pub fn apply_exhaust_select_choice(run: &RunState, index: usize) -> SimResult<RunState> {
    validate_exhaust_select_choice(run, index)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    choose_exhaust_select(combat, index)?;
    Ok(next)
}

pub fn apply_exhaust_select_confirm(run: &RunState) -> SimResult<RunState> {
    validate_exhaust_select_confirm(run)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    let before = combat.clone();
    let exhaust_before = combat.piles.exhaust_pile.len();
    confirm_exhaust_select(&mut combat)?;
    let exhaust_count = exhaust_count_for_confirmed_select(&before, &combat, exhaust_before);
    apply_dead_branch_for_exhaust_count(&mut next, &mut combat, exhaust_count);
    next.combat = Some(combat);
    Ok(next)
}

fn exhaust_count_for_confirmed_select(
    before: &CombatState,
    after: &CombatState,
    exhaust_before: usize,
) -> usize {
    let Some(select) = before.exhaust_select.as_ref() else {
        return after
            .piles
            .exhaust_pile
            .len()
            .saturating_sub(exhaust_before);
    };
    if select.purpose != ExhaustSelectPurpose::ExhumeReturnToHand {
        return after
            .piles
            .exhaust_pile
            .len()
            .saturating_sub(exhaust_before);
    }
    let Some(source_card_id) = select.source_card_id else {
        return 0;
    };
    let source_started_in_hand = before
        .piles
        .hand
        .iter()
        .any(|card| card.id == source_card_id);
    let source_ended_in_exhaust = after
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.id == source_card_id);
    usize::from(source_started_in_hand && source_ended_in_exhaust)
}

pub fn apply_combat_card_reward_choice(run: &RunState, index: usize) -> SimResult<RunState> {
    validate_combat_card_reward_choice(run, index)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    let card_id = CardId::new(combat.piles.max_card_instance_id() + 1);
    if let Some(choices) = combat.potion_card_reward.take() {
        let choice = choices[index];
        combat.piles.hand.push(CardInstance::combat_generated(
            card_id,
            choice.content_id,
            0,
        ));
    } else if let Some(choices) = combat.discovery_card_reward.take() {
        let choice = choices[index];
        let mut card = CardInstance::combat_generated(card_id, choice.content_id, 0);
        card.temp_cost_turn_only = true;
        combat.piles.hand.push(card);
    } else {
        let choices = combat.toolbox_card_reward.take().expect("validated reward");
        let choice = choices[index];
        combat.piles.hand.push(CardInstance {
            combat_only: true,
            ..CardInstance::new(card_id, choice.content_id)
        });
        if let Some(rng) = combat.card_random_rng.as_ref() {
            next.card_random_rng_counter = rng.counter();
        }
    }
    Ok(next)
}

fn distilled_chaos_target(
    combat: &CombatState,
    target: TargetRequirement,
    rng: &mut StsRng,
) -> SimResult<Option<MonsterId>> {
    if target != TargetRequirement::Enemy {
        return Ok(None);
    }

    let living = combat
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster.id)
        .collect::<Vec<_>>();
    if living.is_empty() {
        return Err(SimError::IllegalAction("no living monsters to target"));
    }

    let index = rng.random_int((living.len() - 1) as i32) as usize;
    Ok(Some(living[index]))
}

fn randomize_playable_hand_costs_for_snecko_oil(combat: &mut CombatState, rng: &mut StsRng) {
    for card in &mut combat.piles.hand {
        let Some(definition) = get_card_definition(card.content_id) else {
            continue;
        };
        if definition.keywords.unplayable {
            continue;
        }
        card.temp_cost = Some(rng.random_int(3) as u8);
    }
}

fn potion_multiplier(run: &RunState) -> i32 {
    if run.relics.contains(&crate::Relic::SacredBark) {
        2
    } else {
        1
    }
}

pub fn apply_potion_action(run: &RunState, action: RunAction) -> SimResult<RunState> {
    validate_potion_action(run, action)?;

    let mut next = run.clone();
    match action {
        RunAction::UsePotion { slot, target } => {
            let potion = next.potions.remove(slot);
            let multiplier = potion_multiplier(&next);
            match potion {
                Potion::Fire => {
                    let target = target.expect("validated fire potion target");
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let killed = {
                        let monster = combat
                            .monsters
                            .iter_mut()
                            .find(|monster| monster.id == target)
                            .expect("validated potion target");
                        deal_unmodified_damage_to_monster(monster, FIRE_POTION_DAMAGE * multiplier);
                        !monster.alive
                    };
                    if killed {
                        crate::relic::apply_monster_death_relics(combat);
                    }
                    if combat.monsters.iter().all(|monster| !monster.alive) {
                        combat.phase = CombatPhase::Won;
                    }
                }
                Potion::Block => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.block += BLOCK_POTION_BLOCK * multiplier;
                }
                Potion::Fear => {
                    let target = target.expect("validated fear potion target");
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let monster = combat
                        .monsters
                        .iter_mut()
                        .find(|monster| monster.id == target)
                        .expect("validated potion target");
                    apply_monster_weak(&mut monster.powers, FEAR_POTION_WEAK * multiplier);
                }
                Potion::Blood => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let heal = combat.player.max_hp * BLOOD_POTION_HEAL_PERCENT * multiplier / 100;
                    crate::relic::heal_player_in_combat_with_relics(
                        &mut combat.player.hp,
                        combat.player.max_hp,
                        heal,
                        &combat.relics,
                    );
                }
                Potion::Ancient => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.artifact += ANCIENT_POTION_ARTIFACT * multiplier;
                }
                Potion::HeartOfIron => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.metallicize += HEART_OF_IRON_METALLICIZE * multiplier;
                }
                Potion::Cultist => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.ritual += CULTIST_POTION_RITUAL * multiplier;
                }
                Potion::Dexterity => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.dexterity += DEXTERITY_POTION_DEXTERITY * multiplier;
                }
                Potion::Energy => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.energy += ENERGY_POTION_ENERGY * multiplier;
                }
                Potion::EssenceOfSteel => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.plated_armor += ESSENCE_OF_STEEL_PLATED_ARMOR * multiplier;
                }
                Potion::Explosive => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let targets = combat
                        .monsters
                        .iter()
                        .filter(|monster| monster.alive)
                        .map(|monster| monster.id)
                        .collect::<Vec<_>>();
                    for target in targets {
                        let killed = {
                            let monster = combat
                                .monsters
                                .iter_mut()
                                .find(|monster| monster.id == target)
                                .expect("target was collected from combat");
                            deal_unmodified_damage_to_monster(
                                monster,
                                EXPLOSIVE_POTION_DAMAGE * multiplier,
                            );
                            !monster.alive
                        };
                        if killed {
                            crate::relic::apply_monster_death_relics(combat);
                        }
                    }
                    if combat.monsters.iter().all(|monster| !monster.alive) {
                        combat.phase = CombatPhase::Won;
                    }
                }
                Potion::LiquidBronze => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.thorns += LIQUID_BRONZE_THORNS * multiplier;
                }
                Potion::Regen => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.regen += REGEN_POTION_REGEN * multiplier;
                }
                Potion::Strength => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.strength += STRENGTH_POTION_STRENGTH * multiplier;
                }
                Potion::Flex => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.temp_strength += FLEX_POTION_TEMP_STRENGTH * multiplier;
                }
                Potion::Speed => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.dexterity += SPEED_POTION_TEMP_DEXTERITY * multiplier;
                    combat.player.temp_dexterity += SPEED_POTION_TEMP_DEXTERITY * multiplier;
                }
                Potion::Swift => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    player_draw_cards(combat, SWIFT_POTION_DRAW * multiplier as usize);
                }
                Potion::SneckoOil => {
                    let mut rng = next.card_random_rng();
                    let combat = next.combat.as_mut().expect("validated combat state");
                    player_draw_cards(combat, SNECKO_OIL_DRAW * multiplier as usize);
                    randomize_playable_hand_costs_for_snecko_oil(combat, &mut rng);
                    next.card_random_rng_counter = rng.counter();
                }
                Potion::SmokeBomb => {
                    let combat = next.combat.take().expect("validated combat state");
                    next.player_hp = combat.player.hp;
                    next.player_max_hp = combat.player.max_hp;
                    next.reward = None;
                    next.phase = RunPhase::Idle;
                }
                Potion::Elixir => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    open_exhaust_select(combat)?;
                }
                Potion::BlessingOfTheForge => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    for card in &mut combat.piles.hand {
                        if let Some(upgraded) = upgrade_content_id(card.content_id) {
                            card.content_id = upgraded;
                        }
                    }
                }
                Potion::Duplication => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.duplication_potion_pending = true;
                }
                Potion::DistilledChaos => {
                    let mut rng = next.card_random_rng();
                    let mut combat = next.combat.take().expect("validated combat state");
                    for _ in 0..3 * multiplier {
                        if combat.phase != CombatPhase::WaitingForPlayer
                            || combat.piles.draw_pile.is_empty()
                        {
                            break;
                        }
                        let top_definition = top_draw_card_definition(&combat)
                            .ok_or(SimError::IllegalAction("draw pile is empty"))?;
                        let target =
                            distilled_chaos_target(&combat, top_definition.target, &mut rng)?;
                        combat = apply_play_top_draw_card_action(&combat, target)?;
                    }
                    next.card_random_rng_counter = rng.counter();
                    next.combat = Some(combat);
                }
                Potion::LiquidMemories => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    open_discard_select(combat)?;
                }
                Potion::Weak => {
                    let target = target.expect("validated weak potion target");
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let monster = combat
                        .monsters
                        .iter_mut()
                        .find(|monster| monster.id == target)
                        .expect("validated potion target");
                    apply_monster_weak(&mut monster.powers, WEAK_POTION_WEAK * multiplier);
                }
                Potion::FruitJuice => {
                    let max_hp = FRUIT_JUICE_MAX_HP * multiplier;
                    next.player_max_hp += max_hp;
                    next.player_hp += max_hp;
                    if let Some(combat) = next.combat.as_mut() {
                        combat.player.max_hp += max_hp;
                        combat.player.hp += max_hp;
                    }
                }
                Potion::Gamble => {
                    let mut rng = SimulatorRng::new(next.potion_rng_seed);
                    let win = rng.next_bool(RngStream::Potion, "gamble_potion");
                    next.potion_rng_seed = rng.seed_state();
                    if win {
                        next.gain_gold(GAMBLE_POTION_WIN_GOLD);
                    } else {
                        next.gold = (next.gold - GAMBLE_POTION_LOSS_GOLD).max(0);
                    }
                }
                Potion::EntropicBrew => {
                    let mut rng = crate::rng::StsRng::with_counter(
                        next.potion_rng_seed as i64,
                        next.potion_rng_counter,
                    );
                    if next.can_gain_potions() {
                        while next.potions.len() < next.potion_capacity() {
                            next.potions.push(target_random_potion(&mut rng));
                        }
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
            if let Some(combat) = next.combat.as_mut() {
                crate::relic::apply_potion_use_relics_to_combat(combat);
                next.player_hp = combat.player.hp;
            } else {
                crate::relic::apply_potion_use_relics_to_run_hp(
                    &mut next.player_hp,
                    next.player_max_hp,
                    &next.relics,
                );
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
    use crate::{
        action::CombatAction,
        content::cards::{
            DEFEND_R_ID, DISCOVERY_ID, SECRET_TECHNIQUE_ID, SHRUG_IT_OFF_ID, STRIKE_R_ID, WOUND_ID,
        },
        MapNodeId, MonsterId, Relic,
    };

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
    fn gremlin_horn_triggers_when_potion_kills_monster() {
        use crate::{
            content::monsters::{monster_state, FIXED_SIMPLE_MONSTER},
            CardId, CardInstance,
        };

        let mut run = RunState::combat_fixture();
        run.relics.push(Relic::GremlinHorn);
        run.potions.push(Potion::Fire);
        let combat = run.combat.as_mut().expect("combat");
        combat.relics = run.relics.clone();
        combat.player.energy = 0;
        combat.monsters[0].hp = FIRE_POTION_DAMAGE;
        combat
            .monsters
            .push(monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(2)));
        combat.piles.draw_pile = vec![CardInstance::new(CardId::new(30), DEFEND_R_ID)];
        let target = combat.monsters[0].id;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(target),
            },
        )
        .expect("use fire potion");

        let combat = after.combat.expect("combat continues");
        assert!(!combat.monsters[0].alive);
        assert!(combat.monsters[1].alive);
        assert_eq!(combat.player.energy, crate::relic::GREMLIN_HORN_ENERGY);
        assert!(combat
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == DEFEND_R_ID));
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
    fn sacred_bark_doubles_block_potion_block() {
        let mut run = RunState::combat_fixture_with_relics(vec![Relic::SacredBark]);
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
        assert_eq!(combat.player.block, BLOCK_POTION_BLOCK * 2);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn toy_ornithopter_heals_when_potion_is_used_in_combat() {
        let mut run = RunState::combat_fixture();
        run.relics.push(Relic::ToyOrnithopter);
        run.potions.push(Potion::Block);
        run.combat.as_mut().expect("combat").relics = run.relics.clone();
        run.combat.as_mut().expect("combat").player.hp = 70;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("block potion applies");

        let combat = after.combat.expect("combat remains active");
        assert_eq!(combat.player.hp, 70 + crate::relic::TOY_ORNITHOPTER_HEAL);
        assert_eq!(after.player_hp, combat.player.hp);
    }

    #[test]
    fn toy_ornithopter_does_not_heal_when_potion_is_discarded() {
        let mut run = RunState::combat_fixture();
        run.relics.push(Relic::ToyOrnithopter);
        run.potions.push(Potion::Block);
        run.combat.as_mut().expect("combat").relics = run.relics.clone();
        run.combat.as_mut().expect("combat").player.hp = 70;

        let after = apply_potion_action(&run, RunAction::DiscardPotion { slot: 0 })
            .expect("potion discards");

        assert_eq!(after.combat.expect("combat").player.hp, 70);
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
    fn entropic_brew_refills_extra_potion_belt_slots() {
        let mut run = RunState::map_fixture();
        run.relics.push(crate::Relic::PotionBelt);
        run.potion_rng_seed = 22_079_335_079;
        run.potions = vec![Potion::EntropicBrew];

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use entropic brew");

        assert_eq!(after.potions.len(), run.potion_capacity());
    }

    #[test]
    fn sozu_makes_entropic_brew_consume_without_filling_slots() {
        let mut run = RunState::map_fixture();
        run.relics.push(crate::Relic::Sozu);
        run.potion_rng_seed = 22_079_335_079;
        run.potions = vec![Potion::Fire, Potion::EntropicBrew];

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 1,
                target: None,
            },
        )
        .expect("use entropic brew");

        assert_eq!(after.potions, vec![Potion::Fire]);
        assert_eq!(after.potion_rng_counter, run.potion_rng_counter);
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
    fn fairy_cannot_be_used_directly() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Fairy);

        let err = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect_err("fairy is passive");

        assert_eq!(err, SimError::IllegalAction("Fairy is passive"));
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
    fn fear_potion_consumes_monster_artifact_instead_of_applying_weak() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Fear);
        let combat = run.combat.as_mut().expect("combat");
        combat.monsters[0].powers.artifact = 1;
        let monster_id = combat.monsters[0].id;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(monster_id),
            },
        )
        .expect("use fear potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.monsters[0].powers.artifact, 0);
        assert_eq!(combat.monsters[0].powers.weak, 0);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn sacred_bark_doubles_targeted_potion_values() {
        let mut run = RunState::combat_fixture_with_relics(vec![Relic::SacredBark]);
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
        assert_eq!(combat.monsters[0].powers.weak, FEAR_POTION_WEAK * 2);
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
    fn magic_flower_increases_blood_potion_combat_healing() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Blood);
        let combat = run.combat.as_mut().expect("combat");
        combat.relics = vec![Relic::MagicFlower];
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
        assert_eq!(combat.player.hp, 74);
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
    fn duplication_potion_sets_next_card_duplicate_flag_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Duplication);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use duplication potion");

        let combat = after.combat.expect("combat continues");
        assert!(combat.duplication_potion_pending);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn distilled_chaos_plays_up_to_three_top_draw_cards_and_consumes_potion() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat");
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), DEFEND_R_ID),
            CardInstance::new(CardId::new(11), DEFEND_R_ID),
            CardInstance::new(CardId::new(12), DEFEND_R_ID),
        ];
        run.potions.push(Potion::DistilledChaos);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use distilled chaos");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.player.block, 15);
        assert!(combat.piles.draw_pile.is_empty());
        assert_eq!(combat.piles.exhaust_pile.len(), 3);
        assert_eq!(after.card_random_rng_counter, run.card_random_rng_counter);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn distilled_chaos_attack_targets_use_card_random_rng() {
        let mut run = RunState::combat_fixture();
        run.combat = Some(crate::combat::CombatState::sentry_fixture());
        let combat = run.combat.as_mut().expect("combat");
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), STRIKE_R_ID),
            CardInstance::new(CardId::new(11), STRIKE_R_ID),
            CardInstance::new(CardId::new(12), STRIKE_R_ID),
        ];
        let hp_before: i32 = combat.monsters.iter().map(|monster| monster.hp).sum();
        run.potions.push(Potion::DistilledChaos);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use distilled chaos");

        let combat = after.combat.expect("combat continues");
        let hp_after: i32 = combat.monsters.iter().map(|monster| monster.hp).sum();
        assert_eq!(hp_after, hp_before - 18);
        assert_eq!(
            after.card_random_rng_counter,
            run.card_random_rng_counter + 3
        );
        assert!(after.potions.is_empty());
    }

    #[test]
    fn distilled_chaos_targets_actual_top_draw_card_and_skips_rng_for_unplayable() {
        let mut run = RunState::combat_fixture();
        run.combat = Some(crate::combat::CombatState::sentry_fixture());
        let combat = run.combat.as_mut().expect("combat");
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), DEFEND_R_ID),
            CardInstance::new(CardId::new(11), WOUND_ID),
            CardInstance::new(CardId::new(12), STRIKE_R_ID),
        ];
        let hp_before: i32 = combat.monsters.iter().map(|monster| monster.hp).sum();
        run.potions.push(Potion::DistilledChaos);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use distilled chaos");

        let combat = after.combat.expect("combat continues");
        let hp_after: i32 = combat.monsters.iter().map(|monster| monster.hp).sum();
        assert_eq!(hp_after, hp_before - 6);
        assert_eq!(combat.player.block, 5);
        assert!(combat.piles.draw_pile.is_empty());
        assert_eq!(
            combat
                .piles
                .exhaust_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![STRIKE_R_ID, WOUND_ID, DEFEND_R_ID]
        );
        assert_eq!(
            after.card_random_rng_counter,
            run.card_random_rng_counter + 1
        );
        assert!(after.potions.is_empty());
    }

    #[test]
    fn liquid_memories_returns_selected_discard_card_to_hand_at_zero_cost() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat");
        combat.piles.discard_pile = vec![
            CardInstance::new(CardId::new(20), STRIKE_R_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
        ];
        run.potions.push(Potion::LiquidMemories);

        let opened = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use liquid memories");
        assert!(opened
            .combat
            .as_ref()
            .expect("combat")
            .discard_select
            .is_some());
        assert!(opened.potions.is_empty());

        let chosen = apply_discard_select_choice(&opened, 1).expect("choose defend");
        let after = apply_discard_select_confirm(&chosen).expect("confirm liquid memories");
        let combat = after.combat.expect("combat continues");
        assert!(combat.discard_select.is_none());
        assert_eq!(combat.piles.discard_pile.len(), 1);
        let returned = combat
            .piles
            .hand
            .iter()
            .find(|card| card.id == CardId::new(21))
            .expect("returned card");
        assert_eq!(returned.content_id, DEFEND_R_ID);
        assert_eq!(returned.temp_cost, Some(0));
    }

    #[test]
    fn draw_select_run_actions_confirm_secret_technique_choice() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat");
        combat.piles.hand = vec![CardInstance::new(CardId::new(20), SECRET_TECHNIQUE_ID)];
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), SHRUG_IT_OFF_ID),
        ];

        let after_play = crate::run::apply_combat_action_on_run(
            &run,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("play Secret Technique");
        let chosen =
            crate::run::apply_run_action(&after_play, RunAction::ChooseDrawSelect { index: 0 })
                .expect("choose draw-pile skill");
        let after = crate::run::apply_run_action(&chosen, RunAction::ConfirmDrawSelect)
            .expect("confirm draw select");
        let combat = after.combat.expect("combat continues");

        assert!(combat.draw_select.is_none());
        assert_eq!(combat.piles.hand[0].content_id, SHRUG_IT_OFF_ID);
        assert_eq!(combat.piles.exhaust_pile[0].content_id, SECRET_TECHNIQUE_ID);
    }

    #[test]
    fn liquid_memories_requires_discard_choice_before_confirm() {
        let mut run = RunState::combat_fixture();
        run.combat
            .as_mut()
            .expect("combat")
            .piles
            .discard_pile
            .push(CardInstance::new(CardId::new(20), STRIKE_R_ID));
        run.potions.push(Potion::LiquidMemories);

        let opened = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use liquid memories");

        assert_eq!(
            apply_discard_select_confirm(&opened),
            Err(SimError::IllegalAction("discard select choice is required"))
        );
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
    fn snecko_oil_draws_five_and_randomizes_playable_hand_costs() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::SneckoOil);
        let combat = run.combat.as_mut().expect("combat");
        combat.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), WOUND_ID),
        ];
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), STRIKE_R_ID),
            CardInstance::new(CardId::new(11), DEFEND_R_ID),
            CardInstance::new(CardId::new(12), STRIKE_R_ID),
            CardInstance::new(CardId::new(13), DEFEND_R_ID),
            CardInstance::new(CardId::new(14), STRIKE_R_ID),
        ];

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use snecko oil");

        let combat = after.combat.expect("combat continues");
        let mut expected_rng = run.card_random_rng();
        let expected_randomized_card_ids = [1, 14, 13, 12, 11, 10];
        let expected_costs = expected_randomized_card_ids
            .map(|card_id| (CardId::new(card_id), expected_rng.random_int(3) as u8));

        assert_eq!(combat.piles.hand.len(), 7);
        assert_eq!(
            combat
                .piles
                .hand
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            [1, 2, 14, 13, 12, 11, 10].map(CardId::new).to_vec()
        );
        assert!(combat.piles.draw_pile.is_empty());
        assert_eq!(
            combat
                .piles
                .hand
                .iter()
                .filter(|card| card.temp_cost.is_some())
                .count(),
            6
        );
        assert_eq!(
            combat
                .piles
                .hand
                .iter()
                .find(|card| card.content_id == WOUND_ID)
                .expect("wound")
                .temp_cost,
            None
        );
        for (card_id, expected_cost) in expected_costs {
            assert_eq!(
                combat
                    .piles
                    .hand
                    .iter()
                    .find(|card| card.id == card_id)
                    .expect("playable hand card")
                    .temp_cost,
                Some(expected_cost)
            );
        }
        assert_eq!(
            after.card_random_rng_counter,
            run.card_random_rng_counter + 6
        );
        assert!(after.potions.is_empty());
    }

    #[test]
    fn smoke_bomb_escapes_combat_without_reward() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::SmokeBomb);
        run.combat.as_mut().expect("combat").player.hp = 42;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use smoke bomb");

        assert_eq!(after.phase, RunPhase::Idle);
        assert_eq!(after.player_hp, 42);
        assert!(after.combat.is_none());
        assert!(after.reward.is_none());
        assert!(after.potions.is_empty());
    }

    #[test]
    fn smoke_bomb_is_illegal_in_boss_room() {
        let mut run = RunState::combat_fixture();
        run.map = Some(crate::map::milestone8_fixture());
        run.map.as_mut().expect("map").current_node = MapNodeId::new(6);
        run.potions.push(Potion::SmokeBomb);

        assert_eq!(
            apply_potion_action(
                &run,
                RunAction::UsePotion {
                    slot: 0,
                    target: None,
                },
            ),
            Err(SimError::IllegalAction(
                "Smoke Bomb cannot be used in boss combat"
            ))
        );
    }

    #[test]
    fn elixir_exhausts_selected_hand_cards_and_consumes_potion() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Elixir);
        let combat = run.combat.as_mut().expect("combat");
        combat.piles.hand = vec![
            CardInstance::new(CardId::new(10), STRIKE_R_ID),
            CardInstance::new(CardId::new(11), DEFEND_R_ID),
            CardInstance::new(CardId::new(12), STRIKE_R_ID),
        ];

        let opened = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use elixir");
        assert!(opened
            .combat
            .as_ref()
            .expect("combat")
            .exhaust_select
            .is_some());
        assert!(opened.potions.is_empty());

        let selected = apply_exhaust_select_choice(&opened, 0).expect("choose first");
        let selected = apply_exhaust_select_choice(&selected, 2).expect("choose third");
        let after = apply_exhaust_select_confirm(&selected).expect("confirm elixir");
        let combat = after.combat.expect("combat continues");

        assert!(combat.exhaust_select.is_none());
        assert_eq!(
            combat
                .piles
                .hand
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(11)]
        );
        assert_eq!(
            combat
                .piles
                .exhaust_pile
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(12), CardId::new(10)]
        );
    }

    #[test]
    fn elixir_selection_can_toggle_cards_before_confirm() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Elixir);

        let opened = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use elixir");
        let selected = apply_exhaust_select_choice(&opened, 0).expect("select");
        let toggled = apply_exhaust_select_choice(&selected, 0).expect("toggle");
        let after = apply_exhaust_select_confirm(&toggled).expect("confirm elixir");

        assert_eq!(after.combat.expect("combat").piles.exhaust_pile.len(), 0);
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
    fn weak_potion_consumes_monster_artifact_instead_of_applying_weak() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Weak);
        let combat = run.combat.as_mut().expect("combat");
        combat.monsters[0].powers.artifact = 1;
        let monster_id = combat.monsters[0].id;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(monster_id),
            },
        )
        .expect("use weak potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.monsters[0].powers.artifact, 0);
        assert_eq!(combat.monsters[0].powers.weak, 0);
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
    fn toy_ornithopter_heals_when_potion_is_used_outside_combat() {
        let mut run = RunState::map_fixture();
        run.relics.push(Relic::ToyOrnithopter);
        run.player_hp = 60;
        run.potions.push(Potion::FruitJuice);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("fruit juice applies");

        assert_eq!(
            after.player_hp,
            60 + FRUIT_JUICE_MAX_HP + crate::relic::TOY_ORNITHOPTER_HEAL
        );
    }

    #[test]
    fn sacred_bark_doubles_fruit_juice_without_doubling_toy_ornithopter() {
        let mut run = RunState::map_fixture();
        run.relics.push(Relic::SacredBark);
        run.relics.push(Relic::ToyOrnithopter);
        run.player_hp = 60;
        run.potions.push(Potion::FruitJuice);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("fruit juice applies");

        assert_eq!(
            after.player_max_hp,
            run.player_max_hp + FRUIT_JUICE_MAX_HP * 2
        );
        assert_eq!(
            after.player_hp,
            60 + FRUIT_JUICE_MAX_HP * 2 + crate::relic::TOY_ORNITHOPTER_HEAL
        );
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
    fn discovery_choice_adds_zero_cost_turn_only_combat_card_to_hand() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat");
        combat.piles.hand = vec![CardInstance::new(CardId::new(20), DISCOVERY_ID)];

        let after_play = crate::combat::apply_combat_action(
            combat,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Discovery applies");
        let expected = after_play.discovery_card_reward.as_ref().expect("choices")[0].content_id;
        assert!(crate::combat::legal_combat_actions(&after_play).is_empty());
        run.combat = Some(after_play);

        let after_choice = apply_combat_card_reward_choice(&run, 0).expect("choose Discovery card");

        let combat = after_choice.combat.as_ref().expect("combat");
        assert!(combat.discovery_card_reward.is_none());
        let generated = combat
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == expected)
            .expect("generated Discovery card");
        assert!(generated.combat_only);
        assert_eq!(generated.temp_cost, Some(0));
        assert!(generated.temp_cost_turn_only);
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
