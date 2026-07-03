use crate::{
    card::{CardInstance, CardType, TargetRequirement},
    combat::damage::deal_unmodified_damage_to_monster,
    combat::transition::{
        apply_play_top_draw_card_action, choose_discard_select, choose_draw_select,
        choose_exhaust_select, choose_hand_select, close_discovery_card_reward_source,
        confirm_discard_select, confirm_draw_select, confirm_exhaust_select, confirm_hand_select,
        discard_select_ui_to_discard_index, draw_select_ui_to_draw_index,
        exhaust_select_ui_to_hand_index, flush_pending_player_spikes_damage_if_ready,
        hand_select_ui_to_hand_index, open_discard_select_with_max_choices, open_exhaust_select,
        open_gambling_chip_select, player_draw_cards, top_draw_card_definition,
    },
    combat::{
        apply_burning_blood, CombatPhase, CombatState, DiscardSelectPurpose, ExhaustSelectPurpose,
        HandSelectPurpose,
    },
    content::cards::{get_card_definition, upgrade_card_instance},
    content::shop_pool::{
        burn_colorless_discovery_card_choice_draws,
        burn_colorless_discovery_card_choice_generations, burn_discovery_card_choice_draws,
        burn_discovery_card_choice_generations, colorless_discovery_card_choices,
        discovery_card_choices,
    },
    ids::{CardId, MonsterId},
    map::RoomKind,
    potion::{
        Potion, ANCIENT_POTION_ARTIFACT, BLOCK_POTION_BLOCK, BLOOD_POTION_HEAL_PERCENT,
        CULTIST_POTION_RITUAL, DEXTERITY_POTION_DEXTERITY, ENERGY_POTION_ENERGY,
        ESSENCE_OF_STEEL_PLATED_ARMOR, EXPLOSIVE_POTION_DAMAGE, FEAR_POTION_VULNERABLE,
        FIRE_POTION_DAMAGE, FLEX_POTION_TEMP_STRENGTH, FRUIT_JUICE_MAX_HP,
        HEART_OF_IRON_METALLICIZE, LIQUID_BRONZE_THORNS, REGEN_POTION_REGEN, SNECKO_OIL_DRAW,
        SPEED_POTION_TEMP_DEXTERITY, STRENGTH_POTION_STRENGTH, SWIFT_POTION_DRAW, WEAK_POTION_WEAK,
    },
    power::{apply_monster_vulnerable, apply_monster_weak},
    rng::StsRng,
    run::reward::{
        apply_dead_branch_for_exhaust_count, target_random_combat_potion, target_random_potion,
    },
    RunAction, RunPhase, RunState, SimError, SimResult,
};

const DISCOVERY_ACTION_HIDDEN_GENERATIONS: usize = 3;
const DISCOVERY_ACTION_SCREEN_SETTLE_DRAWS: usize = 1;

pub fn validate_potion_action(run: &RunState, action: RunAction) -> SimResult<()> {
    match action {
        RunAction::UsePotion { slot, target } => {
            let potion = run
                .potion_at_slot(slot)
                .ok_or(SimError::IllegalAction("potion slot is not available"))?;

            if potion == Potion::Fairy {
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
                if potion == Potion::SmokeBomb && current_room_kind(run) == Some(RoomKind::Boss) {
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
            run.potion_at_slot(slot)
                .ok_or(SimError::IllegalAction("potion slot is not available"))?;
            Ok(())
        }
        RunAction::ChooseCombatCardReward { index } => {
            validate_combat_card_reward_choice(run, index)
        }
        RunAction::SkipCombatCardReward => validate_combat_card_reward_skip(run),
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

pub fn validate_combat_card_reward_skip(run: &RunState) -> SimResult<()> {
    let combat = run.combat.as_ref().ok_or(SimError::IllegalAction(
        "combat card reward requires combat",
    ))?;
    if combat.potion_card_reward.is_some() {
        Ok(())
    } else {
        Err(SimError::IllegalAction(
            "no skippable combat card reward is open",
        ))
    }
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
    if hand_select.purpose != HandSelectPurpose::ForethoughtPutAnyOnDraw
        && hand_select.selected_hand_index.is_none()
    {
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
    let purpose = combat
        .discard_select
        .as_ref()
        .map(|select| select.purpose)
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    choose_discard_select(combat, index)?;
    if purpose == DiscardSelectPurpose::HeadbuttPutOnDraw
        || (purpose == DiscardSelectPurpose::LiquidMemoriesReturnToHand
            && combat
                .discard_select
                .as_ref()
                .is_some_and(|select| select.max_choices == 1))
    {
        confirm_discard_select(combat)?;
        flush_pending_player_spikes_damage_if_ready(combat);
    }
    Ok(next)
}

pub fn apply_discard_select_confirm(run: &RunState) -> SimResult<RunState> {
    validate_discard_select_confirm(run)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    confirm_discard_select(combat)?;
    flush_pending_player_spikes_damage_if_ready(combat);
    Ok(next)
}

pub fn apply_exhaust_select_choice(run: &RunState, index: usize) -> SimResult<RunState> {
    validate_exhaust_select_choice(run, index)?;
    let mut next = run.clone();
    let purpose = next
        .combat
        .as_ref()
        .and_then(|combat| combat.exhaust_select.as_ref())
        .map(|select| select.purpose)
        .expect("validated exhaust select");
    let combat = next.combat.as_mut().expect("validated combat");
    choose_exhaust_select(combat, index)?;
    if purpose == ExhaustSelectPurpose::ExhumeReturnToHand {
        let mut combat = next.combat.take().expect("validated combat");
        let before = combat.clone();
        let exhaust_before = combat.piles.exhaust_pile.len();
        confirm_exhaust_select(&mut combat)?;
        let exhaust_count = exhaust_count_for_confirmed_select(&before, &combat, exhaust_before);
        apply_dead_branch_for_exhaust_count(&mut next, &mut combat, exhaust_count);
        next.combat = Some(combat);
    }
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
    let source_started_in_select = select
        .source_card
        .is_some_and(|card| card.id == source_card_id);
    let source_ended_in_exhaust = after
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.id == source_card_id);
    usize::from((source_started_in_hand || source_started_in_select) && source_ended_in_exhaust)
}

pub fn apply_combat_card_reward_choice(run: &RunState, index: usize) -> SimResult<RunState> {
    validate_combat_card_reward_choice(run, index)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    let card_id = CardId::new(combat.piles.max_card_instance_id() + 1);
    if let Some(choices) = combat.potion_card_reward.take() {
        let choice = choices[index];
        let mut card = CardInstance::combat_generated(card_id, choice.content_id, 0);
        card.temp_cost_turn_only = true;
        combat.piles.hand.push(card);
    } else if let Some(choices) = combat.discovery_card_reward.take() {
        let choice = choices[index];
        let mut card = CardInstance::combat_generated(card_id, choice.content_id, 0);
        card.temp_cost_turn_only = true;
        combat.piles.hand.push(card);
        close_discovery_card_reward_source(combat)?;
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

pub fn apply_combat_card_reward_skip(run: &RunState) -> SimResult<RunState> {
    validate_combat_card_reward_skip(run)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    if combat.discovery_card_reward.take().is_some() {
        close_discovery_card_reward_source(combat)?;
    } else {
        combat.potion_card_reward = None;
    }
    Ok(next)
}

fn distilled_chaos_target(
    combat: &mut CombatState,
    target: TargetRequirement,
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

    let index = combat
        .card_random_rng
        .as_mut()
        .map(|rng| rng.random_int((living.len() - 1) as i32) as usize)
        .unwrap_or(0);
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
            let potion = next.take_potion_slot(slot)?;
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
                    apply_monster_vulnerable(
                        &mut monster.powers,
                        FEAR_POTION_VULNERABLE * multiplier,
                    );
                }
                Potion::Blood => {
                    if let Some(combat) = next.combat.as_mut() {
                        let heal =
                            combat.player.max_hp * BLOOD_POTION_HEAL_PERCENT * multiplier / 100;
                        crate::relic::heal_combat_player_with_relics(combat, heal);
                    } else {
                        let heal =
                            next.player_max_hp * BLOOD_POTION_HEAL_PERCENT * multiplier / 100;
                        crate::relic::heal_player_in_combat_with_relics(
                            &mut next.player_hp,
                            next.player_max_hp,
                            heal,
                            &next.relics,
                        );
                    }
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
                        if let Some(upgraded) = upgrade_card_instance(*card) {
                            *card = upgraded;
                        }
                    }
                }
                Potion::Duplication => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.duplication_potion_stacks += multiplier;
                    combat.duplication_potion_pending = false;
                }
                Potion::DistilledChaos => {
                    let mut combat = next.combat.take().expect("validated combat state");
                    if combat.card_random_rng.is_none() {
                        combat.card_random_rng = Some(next.card_random_rng());
                    }
                    for _ in 0..3 * multiplier {
                        if combat.phase != CombatPhase::WaitingForPlayer
                            || combat.piles.draw_pile.is_empty()
                        {
                            break;
                        }
                        let top_definition = top_draw_card_definition(&combat)
                            .ok_or(SimError::IllegalAction("draw pile is empty"))?;
                        let target = distilled_chaos_target(&mut combat, top_definition.target)?;
                        combat = apply_play_top_draw_card_action(&combat, target)?;
                    }
                    if let Some(rng) = combat.card_random_rng.as_ref() {
                        next.card_random_rng_counter = rng.counter();
                    }
                    next.combat = Some(combat);
                }
                Potion::LiquidMemories => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    open_discard_select_with_max_choices(combat, multiplier as usize)?;
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
                Potion::GamblersBrew => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    open_gambling_chip_select(combat)?;
                }
                Potion::EntropicBrew => {
                    let mut rng = crate::rng::StsRng::with_counter(
                        next.potion_rng_seed as i64,
                        next.potion_rng_counter,
                    );
                    if next.can_gain_potions() {
                        let capacity = next.potion_capacity();
                        let combat_fill = next.phase == RunPhase::Combat;
                        for _ in 0..capacity {
                            let potion = if combat_fill {
                                target_random_combat_potion(&mut rng)
                            } else {
                                target_random_potion(&mut rng)
                            };
                            if next.open_potion_slots() > 0 {
                                next.gain_potion(potion)
                                    .expect("open potion slot validated");
                            }
                        }
                    }
                    next.potion_rng_counter = rng.counter();
                }
                Potion::Attack | Potion::Skill | Potion::Colorless | Potion::Power => {
                    let mut combat = next.combat.take().expect("validated combat state");
                    let mut rng = combat
                        .card_random_rng
                        .take()
                        .unwrap_or_else(|| next.card_random_rng());
                    let content_ids = match potion {
                        Potion::Attack => discovery_card_choices(&mut rng, CardType::Attack, 3),
                        Potion::Skill => discovery_card_choices(&mut rng, CardType::Skill, 3),
                        Potion::Colorless => colorless_discovery_card_choices(&mut rng, 3),
                        Potion::Power => discovery_card_choices(&mut rng, CardType::Power, 3),
                        _ => unreachable!("matched discovery potion"),
                    };
                    // Target DiscoveryAction.generate*Choices is called at the top of update(),
                    // before the action checks whether the reward screen is already open. At
                    // fast duration this burns three extra generations after the visible choices,
                    // then one live verifier screen-settle draw before the next combat action.
                    match potion {
                        Potion::Attack => {
                            burn_discovery_card_choice_generations(
                                &mut rng,
                                CardType::Attack,
                                3,
                                DISCOVERY_ACTION_HIDDEN_GENERATIONS,
                            );
                            burn_discovery_card_choice_draws(
                                &mut rng,
                                CardType::Attack,
                                DISCOVERY_ACTION_SCREEN_SETTLE_DRAWS,
                            );
                        }
                        Potion::Skill => {
                            burn_discovery_card_choice_generations(
                                &mut rng,
                                CardType::Skill,
                                3,
                                DISCOVERY_ACTION_HIDDEN_GENERATIONS,
                            );
                            burn_discovery_card_choice_draws(
                                &mut rng,
                                CardType::Skill,
                                DISCOVERY_ACTION_SCREEN_SETTLE_DRAWS,
                            );
                        }
                        Potion::Colorless => {
                            burn_colorless_discovery_card_choice_generations(
                                &mut rng,
                                3,
                                DISCOVERY_ACTION_HIDDEN_GENERATIONS,
                            );
                            burn_colorless_discovery_card_choice_draws(
                                &mut rng,
                                DISCOVERY_ACTION_SCREEN_SETTLE_DRAWS,
                            );
                        }
                        Potion::Power => {
                            burn_discovery_card_choice_generations(
                                &mut rng,
                                CardType::Power,
                                3,
                                DISCOVERY_ACTION_HIDDEN_GENERATIONS,
                            );
                            burn_discovery_card_choice_draws(
                                &mut rng,
                                CardType::Power,
                                DISCOVERY_ACTION_SCREEN_SETTLE_DRAWS,
                            );
                        }
                        _ => unreachable!("matched discovery potion"),
                    }
                    next.card_random_rng_counter = rng.counter();
                    let next_card_id = next.next_card_instance_id();
                    let reward_cards = content_ids
                        .into_iter()
                        .enumerate()
                        .map(|(index, content_id)| {
                            CardInstance::new(CardId::new(next_card_id + index as u64), content_id)
                        })
                        .collect();
                    combat.card_random_rng = Some(rng);
                    combat.potion_card_reward = Some(reward_cards);
                    next.combat = Some(combat);
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
                if let Some(combat) = next.combat.as_mut() {
                    apply_burning_blood(combat);
                    next.player_hp = combat.player.hp;
                    next.player_max_hp = combat.player.max_hp;
                }
                enter_combat_reward_for_current_room(&mut next);
            }
        }
        RunAction::DiscardPotion { slot } => {
            next.take_potion_slot(slot)?;
        }
        _ => unreachable!("validated potion action"),
    }

    Ok(next)
}

fn enter_combat_reward_for_current_room(run: &mut RunState) {
    match current_room_kind(run) {
        Some(RoomKind::Boss) => super::reward::enter_boss_combat_reward_screen(run),
        Some(RoomKind::Elite) => super::reward::enter_elite_combat_reward_screen(run),
        _ => super::reward::enter_reward_screen(run),
    }
}
