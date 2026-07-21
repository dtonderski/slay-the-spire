use crate::{
    card::{CardInstance, CardType, TargetRequirement},
    combat::damage::deal_unmodified_damage_to_monster,
    combat::transition::{
        apply_monster_death_hooks, apply_play_top_draw_card_action, choose_discard_select,
        choose_draw_select, choose_exhaust_select, choose_hand_select, close_discovery_source_card,
        confirm_discard_select, confirm_draw_select, confirm_exhaust_select, confirm_hand_select,
        discard_select_ui_to_discard_index, draw_select_ui_to_draw_index,
        exhaust_select_ui_to_hand_index, flush_pending_player_spikes_damage_if_ready,
        hand_select_ui_to_hand_index, open_discard_select_with_max_choices, open_exhaust_select,
        open_gambling_chip_select, player_draw_cards, player_shuffle_discard_into_draw,
        top_draw_card_definition,
    },
    combat::{
        apply_burning_blood, CombatDecisionState, CombatPhase, CombatState, DiscardSelectPurpose,
        ExhaustSelectPurpose, HandSelectPurpose, PotionCardRewardKind,
    },
    content::cards::{get_card_definition, upgrade_card_instance},
    content::monsters::wake_lagavulin_on_damage,
    content::shop_pool::{
        burn_all_discovery_card_choice_generations, burn_colorless_discovery_card_choice_draws,
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

// DiscoveryAction generates choices at the top of every update while its
// fast-duration action settles. Real CommunicationMod traces prove that the
// target burn differs when the reward is picked versus skipped, and that the
// colorless generation branch settles two updates later than typed cards in
// this verifier environment.
const DISCOVERY_ACTION_PICKED_HIDDEN_GENERATIONS: usize = 9;
const COLORLESS_DISCOVERY_ACTION_PICKED_HIDDEN_GENERATIONS: usize = 11;
const DISCOVERY_ACTION_PICKED_SCREEN_SETTLE_DRAWS: usize = 1;
const DISCOVERY_ACTION_SKIPPED_HIDDEN_GENERATIONS: usize = 6;
const DISCOVERY_ACTION_SKIPPED_SCREEN_SETTLE_DRAWS: usize = 3;
const PLAYED_DISCOVERY_PICKED_HIDDEN_GENERATIONS: usize = 4;

pub fn validate_potion_action(run: &RunState, action: RunAction) -> SimResult<()> {
    run.validate()?;

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
    if combat.potion_card_reward_choices().is_some() {
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
        .combat_card_reward_choices()
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
    let hand_index = hand_select_ui_to_hand_index(combat, index)?;
    let hand_select = combat
        .hand_select()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    if hand_select.purpose != HandSelectPurpose::ForethoughtPutAnyOnDraw
        && hand_select.selected_hand_index == Some(hand_index)
    {
        return Err(SimError::IllegalAction(
            "hand select choice is already selected",
        ));
    }
    Ok(())
}

pub fn validate_hand_select_confirm(run: &RunState) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("hand select requires combat"))?;
    let hand_select = combat
        .hand_select()
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
    let draw_index = draw_select_ui_to_draw_index(combat, index)?;
    let draw_select = combat
        .draw_select()
        .ok_or(SimError::IllegalAction("no draw select is open"))?;
    if draw_select.selected_draw_index == Some(draw_index) {
        return Err(SimError::IllegalAction(
            "draw select choice is already selected",
        ));
    }
    Ok(())
}

pub fn validate_draw_select_confirm(run: &RunState) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("draw select requires combat"))?;
    let draw_select = combat
        .draw_select()
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
        .discard_select()
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
        .exhaust_select()
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
        .discard_select()
        .map(|select| select.purpose)
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    choose_discard_select(combat, index)?;
    if purpose == DiscardSelectPurpose::HeadbuttPutOnDraw
        || (purpose == DiscardSelectPurpose::LiquidMemoriesReturnToHand
            && combat
                .discard_select()
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
        .and_then(CombatState::exhaust_select)
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
    let Some(select) = before.exhaust_select() else {
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
    let decision = combat
        .decision
        .take()
        .ok_or(SimError::IllegalAction("no combat card reward is open"))?;
    match decision {
        CombatDecisionState::PotionCardReward {
            choices,
            reward_kind,
        } => {
            settle_potion_card_reward_rng(combat, reward_kind, true);
            let choice = choices[index];
            let card_id = CardId::new(combat.next_card_instance_id());
            let mut card = CardInstance::combat_generated(card_id, choice.content_id, 0);
            card.temp_cost_turn_only = true;
            // CommunicationMod exposes potion-generated cards after the cards that
            // were already in hand, unlike Toolbox and Discovery rewards.
            combat.piles.hand.push(card);
            crate::relic::apply_potion_use_relics_to_combat(combat);
            next.player_hp = combat.player.hp;
            next.card_random_rng_counter = combat.rng.card_random_rng.counter();
        }
        CombatDecisionState::DiscoveryCardReward {
            choices,
            source_card,
        } => {
            // After a card-played Discovery is selected, DiscoveryAction keeps
            // regenerating its three choices for four hidden fast-action updates.
            burn_all_discovery_card_choice_generations(
                &mut combat.rng.card_random_rng,
                3,
                PLAYED_DISCOVERY_PICKED_HIDDEN_GENERATIONS,
            );
            let choice = choices[index];
            let card_id = CardId::new(combat.next_card_instance_id());
            let mut card = CardInstance::combat_generated(card_id, choice.content_id, 0);
            card.temp_cost_turn_only = true;
            // DiscoveryAction adds the generated card after the cards already in hand.
            combat.piles.hand.push(card);
            close_discovery_source_card(combat, source_card)?;
            next.card_random_rng_counter = combat.rng.card_random_rng.counter();
        }
        CombatDecisionState::ToolboxCardReward { choices } => {
            let choice = choices[index];
            let card_id = CardId::new(combat.next_card_instance_id());
            combat.piles.hand.insert(
                0,
                CardInstance {
                    combat_only: true,
                    ..CardInstance::new(card_id, choice.content_id)
                },
            );
            next.card_random_rng_counter = combat.rng.card_random_rng.counter();
            crate::relic::settle_pending_start_of_turn_relic_actions(combat);
        }
        other => {
            combat.decision = Some(other);
            return Err(SimError::IllegalAction("no combat card reward is open"));
        }
    }
    combat.activate_next_queued_decision_if_idle();
    Ok(next)
}

pub fn apply_combat_card_reward_skip(run: &RunState) -> SimResult<RunState> {
    validate_combat_card_reward_skip(run)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    let Some(CombatDecisionState::PotionCardReward { reward_kind, .. }) = combat.decision.take()
    else {
        return Err(SimError::IllegalAction(
            "no skippable combat card reward is open",
        ));
    };
    settle_potion_card_reward_rng(combat, reward_kind, false);
    crate::relic::apply_potion_use_relics_to_combat(combat);
    next.player_hp = combat.player.hp;
    next.card_random_rng_counter = combat.rng.card_random_rng.counter();
    combat.activate_next_queued_decision_if_idle();
    Ok(next)
}

fn settle_potion_card_reward_rng(
    combat: &mut CombatState,
    kind: PotionCardRewardKind,
    picked: bool,
) {
    let rng = &mut combat.rng.card_random_rng;
    let (mut hidden_generations, settle_draws) = if picked {
        (
            DISCOVERY_ACTION_PICKED_HIDDEN_GENERATIONS,
            DISCOVERY_ACTION_PICKED_SCREEN_SETTLE_DRAWS,
        )
    } else {
        (
            DISCOVERY_ACTION_SKIPPED_HIDDEN_GENERATIONS,
            DISCOVERY_ACTION_SKIPPED_SCREEN_SETTLE_DRAWS,
        )
    };
    if picked && kind == PotionCardRewardKind::Colorless {
        hidden_generations = COLORLESS_DISCOVERY_ACTION_PICKED_HIDDEN_GENERATIONS;
    }
    match kind {
        PotionCardRewardKind::Attack => {
            burn_discovery_card_choice_generations(rng, CardType::Attack, 3, hidden_generations);
            burn_discovery_card_choice_draws(rng, CardType::Attack, settle_draws);
        }
        PotionCardRewardKind::Skill => {
            burn_discovery_card_choice_generations(rng, CardType::Skill, 3, hidden_generations);
            burn_discovery_card_choice_draws(rng, CardType::Skill, settle_draws);
        }
        PotionCardRewardKind::Power => {
            burn_discovery_card_choice_generations(rng, CardType::Power, 3, hidden_generations);
            burn_discovery_card_choice_draws(rng, CardType::Power, settle_draws);
        }
        PotionCardRewardKind::Colorless => {
            burn_colorless_discovery_card_choice_generations(rng, 3, hidden_generations);
            burn_colorless_discovery_card_choice_draws(rng, settle_draws);
        }
    }
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
        .rng
        .card_random_rng
        .random_int((living.len() - 1) as i32) as usize;
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
            let mut defer_potion_use_relics = false;
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
                        let hp_damage = deal_unmodified_damage_to_monster(
                            monster,
                            FIRE_POTION_DAMAGE * multiplier,
                        );
                        wake_lagavulin_on_damage(monster, hp_damage);
                        !monster.alive
                    };
                    if killed {
                        apply_monster_death_hooks(combat, target);
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
                        next.heal_player(heal);
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
                    combat.player.energy = combat
                        .player
                        .energy
                        .wrapping_add(ENERGY_POTION_ENERGY * multiplier);
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
                            let hp_damage = deal_unmodified_damage_to_monster(
                                monster,
                                EXPLOSIVE_POTION_DAMAGE * multiplier,
                            );
                            wake_lagavulin_on_damage(monster, hp_damage);
                            !monster.alive
                        };
                        if killed {
                            apply_monster_death_hooks(combat, target);
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
                    let mut combat = next.combat.take().expect("validated combat state");
                    combat.phase = CombatPhase::Won;
                    apply_burning_blood(&mut combat);
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
                    // DistilledChaosPotion constructs every PlayTopCardAction
                    // up front and chooses a random living monster for each
                    // action, even when the eventual top card has no target.
                    // Those cardRandomRng draws precede all played-card effects.
                    let queued_targets = (0..3 * multiplier)
                        .map(|_| distilled_chaos_target(&mut combat, TargetRequirement::Enemy))
                        .collect::<SimResult<Vec<_>>>()?;
                    // Each PlayTopCardAction moves its selected card to limbo,
                    // but the queued cards do not resolve until all three
                    // actions have selected. If selection finds an empty draw
                    // pile, it shuffles the discard pile while the earlier
                    // selections remain held out. Session 35 reaches that
                    // branch on the third action of a second Distilled Chaos.
                    let mut queued_cards = Vec::with_capacity((3 * multiplier) as usize);
                    for _ in 0..3 * multiplier {
                        if combat.piles.draw_pile.is_empty()
                            && !combat.piles.discard_pile.is_empty()
                        {
                            player_shuffle_discard_into_draw(&mut combat);
                        }
                        let Some(card) = combat.piles.draw_pile.pop() else {
                            break;
                        };
                        queued_cards.push(card);
                    }
                    for (card, queued_target) in queued_cards.into_iter().zip(queued_targets) {
                        if combat.phase != CombatPhase::WaitingForPlayer {
                            break;
                        }
                        combat.piles.draw_pile.push(card);
                        let top_definition = top_draw_card_definition(&combat)
                            .ok_or(SimError::IllegalAction("draw pile is empty"))?;
                        let target = if top_definition.target == TargetRequirement::Enemy {
                            queued_target
                        } else {
                            None
                        };
                        combat = apply_play_top_draw_card_action(&combat, target)?;
                    }
                    next.card_random_rng_counter = combat.rng.card_random_rng.counter();
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
                    next.player_max_hp = next.player_max_hp.wrapping_add(max_hp);
                    next.player_hp = next.player_hp.wrapping_add(max_hp);
                    if let Some(combat) = next.combat.as_mut() {
                        combat.player.max_hp = combat.player.max_hp.wrapping_add(max_hp);
                        combat.player.hp = combat.player.hp.wrapping_add(max_hp);
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
                    defer_potion_use_relics = true;
                    let mut combat = next.combat.take().expect("validated combat state");
                    let rng = &mut combat.rng.card_random_rng;
                    let (kind, content_ids) = match potion {
                        Potion::Attack => (
                            PotionCardRewardKind::Attack,
                            discovery_card_choices(rng, CardType::Attack, 3),
                        ),
                        Potion::Skill => (
                            PotionCardRewardKind::Skill,
                            discovery_card_choices(rng, CardType::Skill, 3),
                        ),
                        Potion::Colorless => (
                            PotionCardRewardKind::Colorless,
                            colorless_discovery_card_choices(rng, 3),
                        ),
                        Potion::Power => (
                            PotionCardRewardKind::Power,
                            discovery_card_choices(rng, CardType::Power, 3),
                        ),
                        _ => unreachable!("matched discovery potion"),
                    };
                    next.card_random_rng_counter = rng.counter();
                    let next_card_id = combat.next_card_instance_id();
                    let reward_cards = content_ids
                        .into_iter()
                        .enumerate()
                        .map(|(index, content_id)| {
                            CardInstance::new(CardId::new(next_card_id + index as u64), content_id)
                        })
                        .collect();
                    combat.decision = Some(CombatDecisionState::PotionCardReward {
                        choices: reward_cards,
                        reward_kind: kind,
                    });
                    next.combat = Some(combat);
                }
                _ => {
                    return Err(SimError::IllegalAction(
                        "potion mechanics are not implemented",
                    ));
                }
            }
            if defer_potion_use_relics {
                if let Some(combat) = next.combat.as_ref() {
                    next.player_hp = combat.player.hp;
                }
            } else if let Some(combat) = next.combat.as_mut() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distilled_chaos_top_deck_combust_applies_its_end_turn_power() {
        use crate::content::cards::{COMBUST_ID, DEFEND_R_ID, STRIKE_R_ID};

        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::DistilledChaos];
        run.empty_potion_slots = vec![1, 2];
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(101), DEFEND_R_ID),
            CardInstance::new(CardId::new(102), STRIKE_R_ID),
            CardInstance::new(CardId::new(103), COMBUST_ID),
        ];

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("Distilled Chaos resolves top three cards");
        let mut combat = next.combat.expect("combat remains open");

        assert_eq!(combat.player.powers.combust, 1);
        assert_eq!(combat.player.powers.combust_damage, 5);
        let player_hp = combat.player.hp;
        let monster_hp = combat.monsters[0].hp;
        crate::combat::turn_powers::apply_end_of_player_turn_powers(&mut combat)
            .expect("end-turn powers resolve");
        assert_eq!(combat.player.hp, player_hp - 1);
        assert_eq!(combat.monsters[0].hp, monster_hp - 5);
    }

    #[test]
    fn distilled_chaos_reshuffles_to_resolve_its_third_action() {
        use crate::content::cards::DEFEND_R_ID;

        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::DistilledChaos];
        run.empty_potion_slots = vec![1, 2];
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(101), DEFEND_R_ID),
            CardInstance::new(CardId::new(102), DEFEND_R_ID),
        ];
        combat.piles.discard_pile = vec![CardInstance::new(CardId::new(103), DEFEND_R_ID)];

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("Distilled Chaos reshuffles before its third action");
        let combat = next.combat.expect("combat remains open");

        assert_eq!(combat.player.block, 15);
        assert!(combat.piles.draw_pile.is_empty());
        assert_eq!(combat.piles.discard_pile.len(), 3);
    }

    #[test]
    fn smoke_bomb_runs_end_of_combat_healing() {
        let mut run = RunState::combat_fixture();
        run.current_room_override = Some(RoomKind::Elite);
        run.relics = vec![crate::relic::Relic::BurningBlood];
        run.potions = vec![Potion::SmokeBomb];
        run.empty_potion_slots = vec![1, 2];
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.relics = run.relics.clone();
        combat.player.hp = 10;
        combat.player.max_hp = 80;

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("Smoke Bomb escapes elite combat");

        assert_eq!(next.player_hp, 16);
        assert!(next.combat.is_none());
        assert_eq!(next.phase, RunPhase::Idle);
        assert_eq!(next.potion_at_slot(0), None);
    }

    #[test]
    fn explosive_potion_kill_releases_spore_cloud() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::Explosive];
        run.empty_potion_slots = vec![1, 2];
        let combat = run.combat.as_mut().unwrap();
        let mut surviving = combat.monsters[0].clone();
        surviving.id = MonsterId::new(1);
        surviving.hp = 20;
        surviving.max_hp = 20;
        surviving.alive = true;
        surviving.powers.spore_cloud = 0;
        let mut dying = surviving.clone();
        dying.id = MonsterId::new(2);
        dying.hp = EXPLOSIVE_POTION_DAMAGE;
        dying.powers.spore_cloud = 2;
        combat.monsters = vec![surviving, dying];

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(MonsterId::new(1)),
            },
        )
        .unwrap();
        let combat = next.combat.unwrap();

        assert_eq!(combat.player.powers.vulnerable, 2);
        assert_eq!(combat.monsters[0].hp, 10);
        assert!(!combat.monsters[1].alive);
    }

    #[test]
    fn explosive_potion_wakes_sleeping_lagavulin() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::Explosive];
        run.empty_potion_slots = vec![1, 2];
        run.combat = Some(CombatState::lagavulin_fixture());

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Explosive Potion damages Lagavulin");
        let monster = &next.combat.expect("combat remains open").monsters[0];

        assert_eq!(monster.sleep_turns_remaining, 0);
        assert_eq!(monster.intent, crate::MonsterIntent::Stun);
        assert_eq!(
            crate::content::monsters::target_move_byte_for_monster(monster),
            Some(4)
        );
    }

    #[test]
    fn discovery_potion_choice_is_appended_to_hand() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let original_ids = combat
            .piles
            .hand
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>();
        let chosen_id = CardId::new(combat.next_card_instance_id());
        let choice_content = combat.piles.hand[0].content_id;
        combat.decision = Some(CombatDecisionState::PotionCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                choice_content,
            )],
            reward_kind: PotionCardRewardKind::Colorless,
        });
        combat.rng.card_random_rng = StsRng::new(123);
        let rng_counter_before = combat.rng.card_random_rng.counter();

        let next = apply_combat_card_reward_choice(&run, 0).expect("potion card choice");
        let combat = next.combat.expect("combat remains open");
        let hand = &combat.piles.hand;

        assert_eq!(
            hand[..original_ids.len()]
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            original_ids
        );
        assert_eq!(hand.last().map(|card| card.id), Some(chosen_id));
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            rng_counter_before + 35,
            "captured colorless Discovery settlement must advance cardRandomRng"
        );
    }

    #[test]
    fn colorless_potion_pick_matches_session_1203_card_random_rng() {
        use crate::content::cards::{FORETHOUGHT_ID, PANIC_BUTTON_ID, SADISTIC_NATURE_ID};

        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::Colorless];
        run.empty_potion_slots = vec![1, 2];
        run.combat
            .as_mut()
            .expect("combat fixture")
            .rng
            .card_random_rng = StsRng::with_counter(-571_295_464_674_976_220, 4);

        let reward = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("Colorless Potion opens its reward");
        assert_eq!(
            reward
                .combat
                .as_ref()
                .and_then(CombatState::potion_card_reward_choices)
                .expect("potion reward")
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![FORETHOUGHT_ID, SADISTIC_NATURE_ID, PANIC_BUTTON_ID]
        );

        let next = apply_combat_card_reward_choice(&reward, 1).expect("pick Sadistic Nature");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.piles.hand.last().map(|card| card.content_id),
            Some(SADISTIC_NATURE_ID)
        );
        let mut card_random_rng = combat.rng.card_random_rng;
        assert_eq!(card_random_rng.counter(), 41);
        assert_eq!(
            card_random_rng.random_int(1),
            1,
            "session-1203 Havoc must target the second living slime"
        );
    }

    #[test]
    fn discovery_card_choice_is_appended_to_hand() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        let original_ids = combat
            .piles
            .hand
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>();
        let chosen_id = CardId::new(combat.next_card_instance_id());
        let choice_content = combat.piles.hand[0].content_id;
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                choice_content,
            )],
            source_card: None,
        });

        let next = apply_combat_card_reward_choice(&run, 0).expect("Discovery card choice");
        let combat = next.combat.expect("combat remains open");
        let hand = &combat.piles.hand;

        assert_eq!(
            hand[..original_ids.len()]
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            original_ids
        );
        assert_eq!(hand.last().map(|card| card.id), Some(chosen_id));
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            28,
            "four hidden DiscoveryAction generations consume twelve draws"
        );
    }

    #[test]
    fn toolbox_card_choice_is_inserted_at_front_of_hand() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let original_ids = combat
            .piles
            .hand
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>();
        let chosen_id = CardId::new(combat.next_card_instance_id());
        let choice_content = combat.piles.hand[0].content_id;
        combat.decision = Some(CombatDecisionState::ToolboxCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                choice_content,
            )],
        });

        let next = apply_combat_card_reward_choice(&run, 0).expect("Toolbox card choice");
        let hand = &next.combat.expect("combat remains open").piles.hand;

        assert_eq!(hand[0].id, chosen_id);
        assert_eq!(
            hand[1..].iter().map(|card| card.id).collect::<Vec<_>>(),
            original_ids
        );
    }

    #[test]
    fn happy_flower_energy_waits_for_opening_toolbox_choice() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.relics = vec![
            crate::relic::Relic::HappyFlower,
            crate::relic::Relic::Toolbox,
        ];
        run.happy_flower_turns = 2;

        let combat = run
            .init_combat(CombatState::initial_fixture())
            .expect("combat initializes");
        assert!(combat.toolbox_card_reward_choices().is_some());
        assert_eq!(combat.relic_counters.happy_flower_turns, 0);
        assert_eq!(combat.player.energy, 3);
        assert_eq!(combat.pending_start_of_turn_relic_energy, 1);

        let json = serde_json::to_string(&combat).expect("combat serializes");
        let combat: CombatState = serde_json::from_str(&json).expect("combat deserializes");
        run.combat = Some(combat);

        let next = apply_combat_card_reward_choice(&run, 0).expect("Toolbox card choice");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(combat.player.energy, 4);
        assert_eq!(combat.pending_start_of_turn_relic_energy, 0);
    }

    #[test]
    fn toolbox_choice_activates_queued_gambling_chip_selection() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.relics = vec![
            crate::relic::Relic::GamblingChip,
            crate::relic::Relic::Toolbox,
        ];

        let combat = run
            .init_combat(CombatState::initial_fixture())
            .expect("combat initializes");
        assert!(matches!(
            combat.decision.as_ref(),
            Some(CombatDecisionState::ToolboxCardReward { .. })
        ));
        assert!(matches!(
            combat.queued_decisions.front(),
            Some(CombatDecisionState::ExhaustSelect { state })
                if state.purpose == ExhaustSelectPurpose::GamblingChip
        ));
        run.combat = Some(combat);

        let next = apply_combat_card_reward_choice(&run, 0).expect("Toolbox card choice");
        let combat = next.combat.as_ref().expect("combat remains open");
        assert!(matches!(
            combat.decision.as_ref(),
            Some(CombatDecisionState::ExhaustSelect { state })
                if state.purpose == ExhaustSelectPurpose::GamblingChip
        ));
        assert!(combat.queued_decisions.is_empty());

        let next = apply_exhaust_select_confirm(&next).expect("Gambling Chip confirmation");
        assert!(next.combat.expect("combat remains open").decision.is_none());
    }
}
