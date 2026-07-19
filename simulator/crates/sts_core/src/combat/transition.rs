use super::card_effects;
use crate::{
    action::{CardPile, CombatAction, HpLossSource, InternalAction},
    card::{CardType, TargetRequirement},
    combat::{
        apply_burning_blood,
        damage::{
            deal_damage_info_to_monster_with_result, deal_unmodified_damage_to_monster,
            reflect_spikes_to_player, DamageInfo, DamageSource,
        },
        state::BombTimer,
        validate_combat_action, CombatPhase, DiscardSelectPurpose, DrawSelectPurpose,
        HandSelectPurpose,
    },
    content::cards::{
        card_instance_is_upgradeable, get_card_definition, ritual_dagger_card_damage,
        ritual_dagger_card_growth, searing_blow_card_damage, upgrade_card_instance,
        upgrade_content_id, ANGER_ID, ANGER_PLUS_ID, BASH_ID, BATTLE_TRANCE_ID,
        BATTLE_TRANCE_PLUS_ID, BLIND_PLUS_ID, BLOOD_FOR_BLOOD_ID, BLOOD_FOR_BLOOD_PLUS_ID,
        BODY_SLAM_ID, BODY_SLAM_PLUS_ID, BURN_ID, CARNAGE_ID, CARNAGE_PLUS_ID, CHRYSALIS_ID,
        CHRYSALIS_PLUS_ID, CLASH_ID, CLASH_PLUS_ID, CLEAVE_ID, CLEAVE_PLUS_ID, CLOTHESLINE_ID,
        CLOTHESLINE_PLUS_ID, COMBUST_ID, COMBUST_PLUS_ID, DAZED_ID, DEEP_BREATH_ID,
        DEEP_BREATH_PLUS_ID, DEFEND_R_ID, DISARM_ID, DISARM_PLUS_ID, DRAMATIC_ENTRANCE_ID,
        DRAMATIC_ENTRANCE_PLUS_ID, DROPKICK_ID, DROPKICK_PLUS_ID, DUAL_WIELD_PLUS_ID,
        ENLIGHTENMENT_ID, ENLIGHTENMENT_PLUS_ID, ENTRENCH_ID, ENTRENCH_PLUS_ID, EXHUME_ID,
        EXHUME_PLUS_ID, FEED_ID, FEED_PLUS_ID, FINESSE_ID, FLAME_BARRIER_ID, FLAME_BARRIER_PLUS_ID,
        FLASH_OF_STEEL_ID, FLASH_OF_STEEL_PLUS_ID, FLEX_ID, FLEX_PLUS_ID, HEADBUTT_ID,
        HEADBUTT_PLUS_ID, HEAVY_BLADE_ID, HEAVY_BLADE_PLUS_ID, HEMOKINESIS_ID, HEMOKINESIS_PLUS_ID,
        IMMOLATE_ID, IMMOLATE_PLUS_ID, IMPATIENCE_ID, IMPATIENCE_PLUS_ID, INFERNAL_BLADE_ID,
        INFERNAL_BLADE_PLUS_ID, INTIMIDATE_ID, INTIMIDATE_PLUS_ID, IRON_WAVE_ID, IRON_WAVE_PLUS_ID,
        MASTER_OF_STRATEGY_ID, MASTER_OF_STRATEGY_PLUS_ID, MIND_BLAST_ID, MIND_BLAST_PLUS_ID,
        OFFERING_ID, PAIN_ID, PANACEA_ID, PANACEA_PLUS_ID, PANACHE_ID, PANACHE_PLUS_ID,
        PANIC_BUTTON_ID, PANIC_BUTTON_PLUS_ID, PERFECTED_STRIKE_ID, PERFECTED_STRIKE_PLUS_ID,
        POMMEL_STRIKE_ID, POMMEL_STRIKE_PLUS_ID, POWER_THROUGH_ID, POWER_THROUGH_PLUS_ID,
        PUMMEL_ID, PUMMEL_PLUS_ID, PURITY_ID, PURITY_PLUS_ID, RAGE_ID, RAGE_PLUS_ID, RAMPAGE_ID,
        RAMPAGE_PLUS_ID, REAPER_ID, REAPER_PLUS_ID, RECKLESS_CHARGE_ID, RECKLESS_CHARGE_PLUS_ID,
        RITUAL_DAGGER_ID, RUPTURE_ID, RUPTURE_PLUS_ID, SADISTIC_NATURE_ID, SADISTIC_NATURE_PLUS_ID,
        SEARING_BLOW_ID, SEARING_BLOW_PLUS_ID, SECRET_TECHNIQUE_ID, SECRET_TECHNIQUE_PLUS_ID,
        SECRET_WEAPON_ID, SECRET_WEAPON_PLUS_ID, SEEING_RED_ID, SEEING_RED_PLUS_ID, SENTINEL_ID,
        SENTINEL_PLUS_ID, SEVER_SOUL_ID, SEVER_SOUL_PLUS_ID, SHRUG_IT_OFF_ID, SHRUG_IT_OFF_PLUS_ID,
        STRIKE_R_ID, STRIKE_R_PLUS_ID, SWORD_BOOMERANG_ID, SWORD_BOOMERANG_PLUS_ID, THUNDERCLAP_ID,
        THUNDERCLAP_PLUS_ID, TRIP_PLUS_ID, TRUE_GRIT_ID, TRUE_GRIT_PLUS_ID, TWIN_STRIKE_ID,
        TWIN_STRIKE_PLUS_ID, WHIRLWIND_ID, WHIRLWIND_PLUS_ID, WILD_STRIKE_ID, WILD_STRIKE_PLUS_ID,
        WOUND_ID,
    },
    content::monsters::{
        apply_collector_death_escape, apply_gremlin_leader_death_escape, check_slime_boss_split,
        enter_guardian_defensive_mode, get_monster_definition, guardian_accumulate_hp_damage,
        release_stasis_card_on_death, wake_lagavulin_on_damage, DARKLING_ID, GIANT_HEAD_ID,
        GUARDIAN_ID,
    },
    content::shop_pool::{colorless_discovery_pool, ironclad_combat_discovery_pool},
    ids::{CardId, ContentId, MonsterId},
    power::{
        apply_monster_vulnerable, apply_monster_weak, apply_player_vulnerable, calculate_block,
        reduce_monster_strength,
    },
    relic::Relic,
    rng::{JavaRng, SimulatorRng},
    CardInstance, CombatState, MonsterState, SimError, SimResult,
};
use std::collections::VecDeque;

pub use super::card_effects::top_draw_card_definition;

const MAX_HAND_SIZE: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatTransition {
    pub state: CombatState,
    pub event_log: Vec<InternalAction>,
}

pub fn apply_combat_action(state: &CombatState, action: CombatAction) -> SimResult<CombatState> {
    Ok(apply_combat_action_with_events(state, action)?.state)
}

pub fn apply_combat_action_with_events(
    state: &CombatState,
    action: CombatAction,
) -> SimResult<CombatTransition> {
    validate_combat_action(state, action)?;

    match action {
        CombatAction::PlayCard { card_id, target } => apply_play_card(state, card_id, target),
        CombatAction::EndTurn => Ok(apply_end_turn(state)),
    }
}

fn apply_end_turn(state: &CombatState) -> CombatTransition {
    let ethereal_ids = end_turn_ethereal_hand_card_ids(state);
    let next = crate::combat::end_player_turn(state);
    let event_log = ethereal_ids
        .into_iter()
        .filter(|card_id| {
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.id == *card_id)
        })
        .map(|card_id| InternalAction::CardExhausted { card_id })
        .collect();

    CombatTransition {
        state: next,
        event_log,
    }
}

fn end_turn_ethereal_hand_card_ids(state: &CombatState) -> Vec<CardId> {
    state
        .piles
        .hand
        .iter()
        .filter(|card| {
            get_card_definition(card.content_id)
                .is_some_and(|definition| definition.keywords.ethereal)
        })
        .map(|card| card.id)
        .collect()
}

pub fn apply_play_top_draw_card_action(
    state: &CombatState,
    target: Option<MonsterId>,
) -> SimResult<CombatState> {
    Ok(process_internal_queue(
        state,
        VecDeque::from([InternalAction::PlayTopDrawCard {
            target,
            exhaust_played_card: false,
            random_living_target: false,
        }]),
    )?
    .state)
}

pub fn apply_play_top_draw_card_to_state(
    state: &mut CombatState,
    target: Option<MonsterId>,
) -> SimResult<()> {
    let transition = process_internal_queue(
        state,
        VecDeque::from([InternalAction::PlayTopDrawCard {
            target,
            exhaust_played_card: false,
            random_living_target: false,
        }]),
    )?;
    *state = transition.state;
    Ok(())
}

fn apply_play_card(
    state: &CombatState,
    card_id: CardId,
    target: Option<MonsterId>,
) -> SimResult<CombatTransition> {
    let (queued_state, queue) = card_effects::play_card_queue(state, card_id, target)?;
    process_internal_queue(&queued_state, queue)
}

fn process_internal_queue(
    state: &CombatState,
    mut queue: VecDeque<InternalAction>,
) -> SimResult<CombatTransition> {
    let mut next = state.clone();
    let mut event_log = Vec::new();

    while let Some(internal_action) = queue.pop_front() {
        if let InternalAction::SkipCopiedCardEffectsIfTargetDead { target } = internal_action {
            event_log.push(internal_action);
            if !living_monster_alive(&next, target) {
                while let Some(skipped_action) = queue.pop_front() {
                    event_log.push(skipped_action);
                    if matches!(skipped_action, InternalAction::EndCopiedCardEffects) {
                        break;
                    }
                }
            }
            continue;
        }
        if matches!(
            internal_action,
            InternalAction::SkipCopiedCardEffectsIfCombatDone
        ) {
            event_log.push(internal_action);
            if next.monsters.iter().all(|monster| !monster.alive) {
                while let Some(skipped_action) = queue.pop_front() {
                    event_log.push(skipped_action);
                    if matches!(skipped_action, InternalAction::EndCopiedCardEffects) {
                        break;
                    }
                }
            }
            continue;
        }
        if matches!(internal_action, InternalAction::EndCopiedCardEffects) {
            event_log.push(internal_action);
            continue;
        }
        let follow_ups = apply_internal_action(&mut next, internal_action)?;
        event_log.push(internal_action);
        for follow_up in follow_ups {
            push_follow_up(&mut queue, follow_up);
        }
        if next.hand_select.is_some() && !queue.is_empty() {
            next.pending_after_hand_select_actions
                .extend(queue.drain(..));
            break;
        }
    }

    flush_pending_player_spikes_damage_if_ready(&mut next);
    flush_pending_monster_death_relics_if_ready(&mut next);

    // Byrd's Grounded action is queued behind the complete card action. A
    // copied or multi-hit card therefore keeps Flight's reduction for every
    // hit, even when an earlier hit reduced Flight to zero.
    for monster in &mut next.monsters {
        if monster.powers.flight_grounding_pending {
            monster.powers.flight_grounding_pending = false;
            if monster.alive {
                monster.intent = crate::MonsterIntent::Stun;
            }
        }
    }

    // Target queues Guardian's Mode Shift action behind the complete card
    // effect queue. This matters for copied multi-hit attacks: every hit lands
    // before Guardian receives its defensive block.
    for monster in &mut next.monsters {
        if monster.content_id == GUARDIAN_ID
            && !monster.in_defensive_mode
            && monster.mode_shift <= 0
            && monster.alive
        {
            enter_guardian_defensive_mode(monster);
        }
    }

    if next.player.hp <= 0 {
        next.player.hp = 0;
        next.player.block = 0;
        next.phase = CombatPhase::Lost;
    } else if next.monsters.iter().all(|monster| !monster.alive) {
        next.phase = CombatPhase::Won;
        apply_burning_blood(&mut next);
    } else {
        next.phase = CombatPhase::WaitingForPlayer;
    }

    Ok(CombatTransition {
        state: next,
        event_log,
    })
}

fn push_follow_up(queue: &mut VecDeque<InternalAction>, follow_up: InternalAction) {
    if let InternalAction::GainMonsterBlock { target, .. } = &follow_up {
        // Multi-hit cards enqueue their remaining hit actions before Malleable's
        // block actions resolve. Keep the block behind contiguous hits from the
        // same card; copied attacks are separated by PlayCardCopy and therefore
        // still resolve Malleable before the copy.
        let pending_hits = queue
            .iter()
            .take_while(|action| {
                matches!(
                    action,
                    InternalAction::DealDamage { info } if info.target == *target
                )
            })
            .count();
        if pending_hits > 0 {
            queue.insert(pending_hits, follow_up);
            return;
        }
        queue.push_front(follow_up);
        return;
    }

    if matches!(follow_up, InternalAction::HandCardExhausted { .. }) {
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::DrawCardsFromInkBottle { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
    }

    if matches!(
        follow_up,
        InternalAction::AddGeneratedHandCardBeforePendingDraw { .. }
    ) {
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::DrawCardsFromInkBottle { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
    }

    if matches!(follow_up, InternalAction::GainStrength { .. }) {
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::PlayCardCopy { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
    }

    queue.push_back(follow_up);
}

pub fn flush_pending_player_spikes_damage_if_ready(state: &mut CombatState) {
    if state.pending_player_spikes_damage <= 0
        || state.discard_select.is_some()
        || state.draw_select.is_some()
        || state.exhaust_select.is_some()
        || state.hand_select.is_some()
        || state.discovery_card_reward.is_some()
        || state.potion_card_reward.is_some()
        || state.toolbox_card_reward.is_some()
    {
        return;
    }
    let damage = std::mem::take(&mut state.pending_player_spikes_damage);
    let hp_loss = reflect_spikes_to_player(&mut state.player, &state.relics, damage);
    crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss);
}

pub fn flush_pending_monster_death_relics_if_ready(state: &mut CombatState) {
    if state.pending_monster_death_relic_triggers == 0
        || state.discard_select.is_some()
        || state.draw_select.is_some()
        || state.exhaust_select.is_some()
        || state.hand_select.is_some()
        || state.discovery_card_reward.is_some()
        || state.potion_card_reward.is_some()
        || state.toolbox_card_reward.is_some()
    {
        return;
    }
    let triggers = std::mem::take(&mut state.pending_monster_death_relic_triggers);
    for _ in 0..triggers {
        crate::relic::apply_monster_death_relics(state);
    }
}

fn apply_internal_action(
    state: &mut CombatState,
    action: InternalAction,
) -> SimResult<Vec<InternalAction>> {
    match action {
        InternalAction::ConsumeDuplicationPotion => {
            if state.duplication_potion_stacks > 0 {
                state.duplication_potion_stacks -= 1;
                if state.duplication_potion_stacks == 0 {
                    state.duplication_potion_pending = false;
                }
            } else if state.duplication_potion_pending {
                state.duplication_potion_pending = false;
            }
            Ok(Vec::new())
        }
        InternalAction::ConsumeDoubleTap => {
            state.double_tap_pending = state.double_tap_pending.saturating_sub(1);
            Ok(Vec::new())
        }
        InternalAction::ConsumeNecronomicon => {
            state.relic_counters.necronomicon_used_this_turn = true;
            Ok(Vec::new())
        }
        InternalAction::PlayCard { card_id } => {
            let card = find_hand_card(state, card_id)?;
            let definition = get_card_definition(card.content_id)
                .ok_or(SimError::UnknownContent(card.content_id))?;
            apply_enrage_on_card_type(state, definition.card_type);
            apply_rage_on_card_type(state, definition.card_type);
            let mut follow_ups =
                crate::relic::apply_on_card_play_relics(state, definition.card_type);
            apply_mummified_hand_on_power_play(state, card_id, definition.card_type);
            follow_ups.extend(apply_on_card_play_powers(state, definition.card_type));
            follow_ups.extend(apply_hand_card_play_triggers(state, card_id));
            Ok(follow_ups)
        }
        InternalAction::PlayCardCopy { card_id } => {
            let definition = card_content_definition(state, card_id)?;
            apply_enrage_on_card_type(state, definition.card_type);
            apply_rage_on_card_type(state, definition.card_type);
            let mut follow_ups =
                crate::relic::apply_on_card_play_relics(state, definition.card_type);
            follow_ups.extend(apply_on_card_play_powers(state, definition.card_type));
            follow_ups.extend(apply_copied_card_play_triggers(state));
            Ok(follow_ups)
        }
        InternalAction::SkipCopiedCardEffectsIfTargetDead { .. }
        | InternalAction::SkipCopiedCardEffectsIfCombatDone
        | InternalAction::EndCopiedCardEffects => Ok(Vec::new()),
        InternalAction::SpendEnergy { amount } => {
            state.player.energy -= amount;
            Ok(Vec::new())
        }
        InternalAction::SpendCardEnergy { card_id } => {
            let cost = effective_hand_card_cost(state, card_id);
            state.player.energy -= cost;
            Ok(Vec::new())
        }
        InternalAction::SetHandCardCostForTurn { card_id, cost } => {
            let card = find_hand_card_mut(state, card_id)?;
            card.temp_cost = Some(cost);
            card.temp_cost_turn_only = true;
            Ok(Vec::new())
        }
        InternalAction::SetHandCardCostForCombat { card_id, cost } => {
            let card = find_hand_card_mut(state, card_id)?;
            card.temp_cost = Some(cost);
            card.temp_cost_turn_only = false;
            Ok(Vec::new())
        }
        InternalAction::DealDamage { info } => {
            if living_monster_mut_opt(state, info.target).is_none() {
                return Ok(Vec::new());
            }
            let player_powers = state.player.powers;
            let temp_strength = state.player.temp_strength;
            let relics = state.relics.clone();
            let (spikes, monster_content_id, still_alive, hand_drill_applies, malleable_block) = {
                let monster = living_monster_mut(state, info.target)?;
                let spikes = monster.powers.spikes;
                let monster_content_id = monster.content_id;
                let damage = deal_damage_info_to_monster_with_result(
                    monster,
                    info,
                    player_powers,
                    temp_strength,
                    &relics,
                );
                wake_lagavulin_on_damage(monster, damage.hp_damage);
                guardian_accumulate_hp_damage(monster, damage.hp_damage);
                (
                    spikes,
                    monster_content_id,
                    monster.alive,
                    relics.contains(&crate::Relic::HandDrill) && damage.broke_block,
                    damage.malleable_block,
                )
            };
            let mut follow_ups = Vec::new();
            push_malleable_block_follow_up(
                state,
                &mut follow_ups,
                info.target,
                monster_content_id,
                still_alive,
                malleable_block,
            );
            if still_alive && hand_drill_applies {
                apply_player_vulnerable_debuff(
                    state,
                    info.target,
                    crate::relic::HAND_DRILL_VULNERABLE,
                )?;
            }
            check_slime_boss_split(state, info.target);
            if !still_alive {
                queue_monster_death_hooks(state, info.target);
            }
            apply_or_queue_spikes_to_player(state, monster_content_id, spikes);
            Ok(follow_ups)
        }
        InternalAction::DealHandOfGreedDamage { info, gold } => {
            if living_monster_mut_opt(state, info.target).is_none() {
                return Ok(Vec::new());
            }
            let player_powers = state.player.powers;
            let temp_strength = state.player.temp_strength;
            let relics = state.relics.clone();
            let (
                spikes,
                monster_content_id,
                still_alive,
                minion,
                hand_drill_applies,
                malleable_block,
            ) = {
                let monster = living_monster_mut(state, info.target)?;
                let spikes = monster.powers.spikes;
                let monster_content_id = monster.content_id;
                let damage = deal_damage_info_to_monster_with_result(
                    monster,
                    info,
                    player_powers,
                    temp_strength,
                    &relics,
                );
                wake_lagavulin_on_damage(monster, damage.hp_damage);
                guardian_accumulate_hp_damage(monster, damage.hp_damage);
                (
                    spikes,
                    monster_content_id,
                    monster.alive,
                    monster.powers.minion > 0,
                    relics.contains(&crate::Relic::HandDrill) && damage.broke_block,
                    damage.malleable_block,
                )
            };
            let mut follow_ups = Vec::new();
            push_malleable_block_follow_up(
                state,
                &mut follow_ups,
                info.target,
                monster_content_id,
                still_alive,
                malleable_block,
            );
            if still_alive && hand_drill_applies {
                apply_player_vulnerable_debuff(
                    state,
                    info.target,
                    crate::relic::HAND_DRILL_VULNERABLE,
                )?;
            }
            check_slime_boss_split(state, info.target);
            if !still_alive {
                if !minion {
                    state.combat_gold_gained += gold.max(0);
                }
                apply_monster_death_hooks(state, info.target);
            }
            apply_or_queue_spikes_to_player(state, monster_content_id, spikes);
            Ok(follow_ups)
        }
        InternalAction::DealDamageRandomEnemy { source, amount } => {
            if let Some(target) = random_living_monster_id(state) {
                let player_powers = state.player.powers;
                let temp_strength = state.player.temp_strength;
                let relics = state.relics.clone();
                let (spikes, monster_content_id, still_alive, hand_drill_applies, malleable_block) = {
                    let monster = living_monster_mut(state, target)?;
                    let spikes = monster.powers.spikes;
                    let monster_content_id = monster.content_id;
                    let damage = deal_damage_info_to_monster_with_result(
                        monster,
                        DamageInfo {
                            source: DamageSource::Card(source),
                            target,
                            amount,
                        },
                        player_powers,
                        temp_strength,
                        &relics,
                    );
                    wake_lagavulin_on_damage(monster, damage.hp_damage);
                    guardian_accumulate_hp_damage(monster, damage.hp_damage);
                    (
                        spikes,
                        monster_content_id,
                        monster.alive,
                        relics.contains(&crate::Relic::HandDrill) && damage.broke_block,
                        damage.malleable_block,
                    )
                };
                let mut follow_ups = Vec::new();
                push_malleable_block_follow_up(
                    state,
                    &mut follow_ups,
                    target,
                    monster_content_id,
                    still_alive,
                    malleable_block,
                );
                if still_alive && hand_drill_applies {
                    apply_player_vulnerable_debuff(
                        state,
                        target,
                        crate::relic::HAND_DRILL_VULNERABLE,
                    )?;
                }
                check_slime_boss_split(state, target);
                if !still_alive {
                    apply_monster_death_hooks(state, target);
                }
                apply_or_queue_spikes_to_player(state, monster_content_id, spikes);
                return Ok(follow_ups);
            }
            Ok(Vec::new())
        }
        InternalAction::DealDamageAndHealUnblocked { info } => {
            if living_monster_mut_opt(state, info.target).is_none() {
                return Ok(Vec::new());
            }
            let player_powers = state.player.powers;
            let temp_strength = state.player.temp_strength;
            let relics = state.relics.clone();
            let (
                hp_damage,
                spikes,
                monster_content_id,
                still_alive,
                hand_drill_applies,
                malleable_block,
            ) = {
                let monster = living_monster_mut(state, info.target)?;
                let spikes = monster.powers.spikes;
                let monster_content_id = monster.content_id;
                let damage = deal_damage_info_to_monster_with_result(
                    monster,
                    info,
                    player_powers,
                    temp_strength,
                    &relics,
                );
                wake_lagavulin_on_damage(monster, damage.hp_damage);
                guardian_accumulate_hp_damage(monster, damage.hp_damage);
                (
                    damage.hp_damage,
                    spikes,
                    monster_content_id,
                    monster.alive,
                    relics.contains(&crate::Relic::HandDrill) && damage.broke_block,
                    damage.malleable_block,
                )
            };
            let mut follow_ups = Vec::new();
            push_malleable_block_follow_up(
                state,
                &mut follow_ups,
                info.target,
                monster_content_id,
                still_alive,
                malleable_block,
            );
            crate::relic::heal_combat_player_with_relics(state, hp_damage);
            if still_alive && hand_drill_applies {
                apply_player_vulnerable_debuff(
                    state,
                    info.target,
                    crate::relic::HAND_DRILL_VULNERABLE,
                )?;
            }
            check_slime_boss_split(state, info.target);
            if !still_alive {
                apply_monster_death_hooks(state, info.target);
            }
            apply_or_queue_spikes_to_player(state, monster_content_id, spikes);
            Ok(follow_ups)
        }
        InternalAction::DealFeedDamage { info, max_hp_gain } => {
            if living_monster_mut_opt(state, info.target).is_none() {
                return Ok(Vec::new());
            }
            let player_powers = state.player.powers;
            let temp_strength = state.player.temp_strength;
            let relics = state.relics.clone();
            let (
                spikes,
                monster_content_id,
                still_alive,
                minion,
                hand_drill_applies,
                malleable_block,
            ) = {
                let monster = living_monster_mut(state, info.target)?;
                let spikes = monster.powers.spikes;
                let monster_content_id = monster.content_id;
                let damage = deal_damage_info_to_monster_with_result(
                    monster,
                    info,
                    player_powers,
                    temp_strength,
                    &relics,
                );
                wake_lagavulin_on_damage(monster, damage.hp_damage);
                guardian_accumulate_hp_damage(monster, damage.hp_damage);
                (
                    spikes,
                    monster_content_id,
                    monster.alive,
                    monster.powers.minion > 0,
                    relics.contains(&crate::Relic::HandDrill) && damage.broke_block,
                    damage.malleable_block,
                )
            };
            let mut follow_ups = Vec::new();
            push_malleable_block_follow_up(
                state,
                &mut follow_ups,
                info.target,
                monster_content_id,
                still_alive,
                malleable_block,
            );
            if still_alive && hand_drill_applies {
                apply_player_vulnerable_debuff(
                    state,
                    info.target,
                    crate::relic::HAND_DRILL_VULNERABLE,
                )?;
            }
            check_slime_boss_split(state, info.target);
            if !still_alive {
                if !minion && monster_content_id != DARKLING_ID {
                    let hp_gain =
                        crate::relic::combat_healing_amount_with_relics(max_hp_gain, &state.relics);
                    state.player.max_hp += max_hp_gain;
                    state.player.hp = (state.player.hp + hp_gain).min(state.player.max_hp);
                    crate::relic::sync_red_skull_strength(state);
                }
                apply_monster_death_hooks(state, info.target);
            }
            apply_or_queue_spikes_to_player(state, monster_content_id, spikes);
            Ok(follow_ups)
        }
        InternalAction::DealRitualDaggerDamage { info, growth } => {
            if living_monster_mut_opt(state, info.target).is_none() {
                return Ok(Vec::new());
            }
            let player_powers = state.player.powers;
            let temp_strength = state.player.temp_strength;
            let relics = state.relics.clone();
            let (
                spikes,
                monster_content_id,
                still_alive,
                minion,
                hand_drill_applies,
                malleable_block,
            ) = {
                let monster = living_monster_mut(state, info.target)?;
                let spikes = monster.powers.spikes;
                let monster_content_id = monster.content_id;
                let damage = deal_damage_info_to_monster_with_result(
                    monster,
                    info,
                    player_powers,
                    temp_strength,
                    &relics,
                );
                wake_lagavulin_on_damage(monster, damage.hp_damage);
                guardian_accumulate_hp_damage(monster, damage.hp_damage);
                (
                    spikes,
                    monster_content_id,
                    monster.alive,
                    monster.powers.minion > 0,
                    relics.contains(&crate::Relic::HandDrill) && damage.broke_block,
                    damage.malleable_block,
                )
            };
            let mut follow_ups = Vec::new();
            push_malleable_block_follow_up(
                state,
                &mut follow_ups,
                info.target,
                monster_content_id,
                still_alive,
                malleable_block,
            );
            if still_alive && hand_drill_applies {
                apply_player_vulnerable_debuff(
                    state,
                    info.target,
                    crate::relic::HAND_DRILL_VULNERABLE,
                )?;
            }
            check_slime_boss_split(state, info.target);
            if !still_alive {
                if !minion {
                    let DamageSource::Card(source_card_id) = info.source;
                    add_ritual_dagger_damage_bonus(state, source_card_id, growth);
                }
                apply_monster_death_hooks(state, info.target);
            }
            apply_or_queue_spikes_to_player(state, monster_content_id, spikes);
            Ok(follow_ups)
        }
        InternalAction::DealDamageAll { source, amount } => {
            let (_, follow_ups) = deal_attack_damage_to_all_living(state, source, amount)?;
            Ok(follow_ups)
        }
        InternalAction::DealDamageAllRepeated {
            source,
            amount,
            times,
        } => {
            let initial_malleable = state
                .monsters
                .iter()
                .map(|monster| (monster.id, monster.powers.malleable))
                .collect::<Vec<_>>();
            let mut follow_ups = Vec::new();
            for _ in 0..times {
                let (_, hit_follow_ups) = deal_attack_damage_to_all_living(state, source, amount)?;
                follow_ups.extend(hit_follow_ups.into_iter().filter(|follow_up| {
                    !matches!(follow_up, InternalAction::GainMonsterBlock { .. })
                }));
            }
            for (target, malleable) in initial_malleable {
                if malleable <= 0 {
                    continue;
                }
                if let Some(monster) = living_monster_mut_opt(state, target) {
                    if monster.powers.malleable > malleable {
                        monster.powers.malleable = malleable + times;
                        let block = (0..times).map(|offset| malleable + offset).sum();
                        follow_ups.push(InternalAction::GainMonsterBlock {
                            target,
                            amount: block,
                        });
                    }
                }
            }
            Ok(follow_ups)
        }
        InternalAction::DealDamageAllAndHealUnblocked { source, amount } => {
            let (hp_damage, follow_ups) = deal_attack_damage_to_all_living(state, source, amount)?;
            crate::relic::heal_combat_player_with_relics(state, hp_damage);
            Ok(follow_ups)
        }
        InternalAction::HealPlayer { amount } => {
            crate::relic::heal_combat_player_with_relics(state, amount);
            Ok(Vec::new())
        }
        InternalAction::GainBlock { amount } => Ok(apply_player_card_block_gain(state, amount)),
        InternalAction::GainMonsterBlock { target, amount } => {
            if let Some(monster) = living_monster_mut_opt(state, target) {
                monster.block += amount;
            }
            Ok(Vec::new())
        }
        InternalAction::PreventBlockGain { turns } => {
            state.player.no_block_turns = state.player.no_block_turns.max(turns);
            Ok(Vec::new())
        }
        InternalAction::GainTemporaryThorns { amount } => {
            state.player.temp_thorns += amount;
            Ok(Vec::new())
        }
        InternalAction::DoublePlayerBlock => {
            state.player.block *= 2;
            Ok(Vec::new())
        }
        InternalAction::ApplyVulnerable { target, amount } => {
            apply_player_vulnerable_debuff(state, target, amount)?;
            Ok(Vec::new())
        }
        InternalAction::ApplyPlayerVulnerable { amount } => {
            crate::power::apply_player_vulnerable(&mut state.player.powers, amount);
            Ok(Vec::new())
        }
        InternalAction::ApplyWeak { target, amount } => {
            let mut applied = false;
            if let Some(monster) = living_monster_mut_opt(state, target) {
                applied = apply_monster_weak(&mut monster.powers, amount);
            }
            apply_sadistic_nature_after_monster_debuff(state, target, applied)?;
            Ok(Vec::new())
        }
        InternalAction::ReduceMonsterStrength { target, amount } => {
            let mut applied = false;
            if let Some(monster) = living_monster_mut_opt(state, target) {
                applied = reduce_monster_strength(&mut monster.powers, amount);
            }
            apply_sadistic_nature_after_monster_debuff(state, target, applied)?;
            Ok(Vec::new())
        }
        InternalAction::ReduceMonsterStrengthThisTurn { target, amount } => {
            let mut applied = false;
            if let Some(monster) = living_monster_mut_opt(state, target) {
                applied = reduce_monster_strength(&mut monster.powers, amount);
                if applied {
                    monster.temp_strength_down += amount;
                }
            }
            apply_sadistic_nature_after_monster_debuff(state, target, applied)?;
            Ok(Vec::new())
        }
        InternalAction::MoveCard { card_id, from, to } => {
            let hand_exhaust_is_attack = if from == CardPile::Hand && to == CardPile::ExhaustPile {
                find_hand_card(state, card_id)
                    .ok()
                    .and_then(|card| get_card_definition(card.content_id))
                    .is_some_and(|definition| definition.card_type == CardType::Attack)
            } else {
                false
            };
            move_card(state, card_id, from, to)?;
            let mut follow_ups = Vec::new();
            if from == CardPile::Hand && state.piles.hand.is_empty() {
                apply_unceasing_top_after_hand_emptied(state);
            }
            if to == CardPile::ExhaustPile {
                if hand_exhaust_is_attack {
                    follow_ups.push(InternalAction::HandCardExhausted { card_id });
                } else {
                    follow_ups.push(InternalAction::CardExhausted { card_id });
                }
            }
            Ok(follow_ups)
        }
        InternalAction::ReturnExhaustCardToHand { card_id } => {
            let card = remove_card_from_pile(state, card_id, CardPile::ExhaustPile)?;
            state.piles.hand.push(card);
            Ok(Vec::new())
        }
        InternalAction::ForethoughtAutoMove {
            source_card_id,
            card_id,
        } => {
            move_forethought_card_to_draw_bottom(state, source_card_id, card_id)?;
            Ok(Vec::new())
        }
        InternalAction::ExhaustRandomHandCardExcept { excluded_card_id } => {
            let Some(card_id) = random_hand_card_id_except(state, excluded_card_id) else {
                return Ok(Vec::new());
            };
            move_card(state, card_id, CardPile::Hand, CardPile::ExhaustPile)?;
            if state.piles.hand.is_empty() {
                apply_unceasing_top_after_hand_emptied(state);
            }
            Ok(vec![InternalAction::CardExhausted { card_id }])
        }
        InternalAction::RemoveCard { card_id, from } => {
            remove_card_from_pile(state, card_id, from)?;
            if from == CardPile::Hand && state.piles.hand.is_empty() {
                apply_unceasing_top_after_hand_emptied(state);
            }
            Ok(Vec::new())
        }
        InternalAction::AddCardToPile { content_id, to } => {
            add_card_to_pile(state, content_id, to);
            Ok(Vec::new())
        }
        InternalAction::AddGeneratedCardToPile {
            content_id,
            to,
            temp_cost,
            temp_cost_turn_only,
        } => {
            add_generated_card_to_pile(state, content_id, to, temp_cost, temp_cost_turn_only);
            Ok(Vec::new())
        }
        InternalAction::AddGeneratedHandCardBeforePendingDraw {
            content_id,
            temp_cost,
            temp_cost_turn_only,
        } => {
            add_generated_card_to_pile(
                state,
                content_id,
                CardPile::Hand,
                temp_cost,
                temp_cost_turn_only,
            );
            Ok(Vec::new())
        }
        InternalAction::AddStatEquivalentCopyToPile { card, to } => {
            add_stat_equivalent_copy_to_pile(state, card, to);
            Ok(Vec::new())
        }
        InternalAction::AddGeneratedCardToDrawPileRandomSpot { content_id } => {
            add_generated_card_to_draw_pile_random_spot(state, content_id, None, false);
            Ok(Vec::new())
        }
        InternalAction::AddGeneratedCardToDrawPileRandomSpotWithCost {
            content_id,
            temp_cost,
            temp_cost_turn_only,
        } => {
            add_generated_card_to_draw_pile_random_spot(
                state,
                content_id,
                temp_cost,
                temp_cost_turn_only,
            );
            Ok(Vec::new())
        }
        InternalAction::AddRandomColorlessCardToHand { temp_cost, upgrade } => {
            let mut content_id = random_colorless_card(state);
            if upgrade {
                content_id = upgrade_content_id(content_id).unwrap_or(content_id);
            }
            add_generated_card_to_pile(state, content_id, CardPile::Hand, temp_cost, false);
            Ok(Vec::new())
        }
        InternalAction::DrawCards { count } => {
            player_draw_cards(state, count);
            Ok(Vec::new())
        }
        InternalAction::DrawCardsWhilePlayedCardIsInLimbo { card_id, count } => {
            let hand_index = state
                .piles
                .hand
                .iter()
                .position(|card| card.id == card_id)
                .ok_or(SimError::IllegalAction("played card is not in hand"))?;
            let played_card = state.piles.hand.remove(hand_index);
            player_draw_cards(state, count);
            state.piles.discard_pile.push(played_card);
            Ok(Vec::new())
        }
        InternalAction::DrawCardsFromInkBottle { count } => {
            player_draw_cards(state, count);
            Ok(Vec::new())
        }
        InternalAction::ShuffleDiscardIntoDraw => {
            player_shuffle_discard_into_draw(state);
            Ok(Vec::new())
        }
        InternalAction::DeepBreathShuffleDiscardIntoDraw => {
            player_deep_breath_shuffle_discard_into_draw(state);
            Ok(Vec::new())
        }
        InternalAction::DrawCardsIfNoAttacksInHand { count } => {
            if !hand_contains_attack(state) {
                player_draw_cards(state, count);
            }
            Ok(Vec::new())
        }
        InternalAction::DrawRandomAttacksFromDrawPile { count } => {
            draw_random_attacks_from_draw_pile(state, count);
            Ok(Vec::new())
        }
        InternalAction::GainEnergy { amount } => {
            state.player.energy += amount;
            Ok(Vec::new())
        }
        InternalAction::LoseHp { amount, source } => {
            let hp_loss = crate::combat::hp_loss::lose_player_hp(state, amount);
            if matches!(source, HpLossSource::Card(_)) {
                crate::combat::hp_loss::apply_player_card_hp_loss_hooks(state, hp_loss);
            } else {
                crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss);
            }
            Ok(Vec::new())
        }
        InternalAction::SetCannotDraw => {
            state.player.cannot_draw = true;
            Ok(Vec::new())
        }
        InternalAction::GainRage { amount } => {
            state.player.temp_rage_block += amount;
            Ok(Vec::new())
        }
        InternalAction::SetRandomHandCardCostForCombat { amount } => {
            set_random_hand_card_cost_for_combat(state, amount);
            Ok(Vec::new())
        }
        InternalAction::UpgradeHandCardsExcept { card_id } => {
            upgrade_hand_cards_except(state, card_id);
            Ok(Vec::new())
        }
        InternalAction::UpgradeHandCard { card_id } => {
            upgrade_hand_card(state, card_id)?;
            Ok(Vec::new())
        }
        InternalAction::IncreaseRampageDamage { card_id, amount } => {
            find_hand_card_mut(state, card_id)?.rampage_damage_bonus += amount;
            Ok(Vec::new())
        }
        InternalAction::GainFeelNoPain { amount } => {
            state.player.powers.feel_no_pain += amount;
            Ok(Vec::new())
        }
        InternalAction::GainDarkEmbrace { amount } => {
            state.player.powers.dark_embrace += amount;
            Ok(Vec::new())
        }
        InternalAction::GainBarricade { amount } => {
            state.player.powers.barricade = state.player.powers.barricade.max(amount);
            Ok(Vec::new())
        }
        InternalAction::GainEvolve { amount } => {
            state.player.powers.evolve += amount;
            Ok(Vec::new())
        }
        InternalAction::GainBerserk { amount } => {
            state.player.powers.berserk += amount;
            Ok(Vec::new())
        }
        InternalAction::GainRupture { amount } => {
            state.player.powers.rupture += amount;
            Ok(Vec::new())
        }
        InternalAction::GainJuggernaut { amount } => {
            state.player.powers.juggernaut += amount;
            Ok(Vec::new())
        }
        InternalAction::GainBrutality { amount } => {
            state.player.powers.brutality += amount;
            Ok(Vec::new())
        }
        InternalAction::GainMayhem { amount } => {
            state.player.powers.mayhem += amount;
            Ok(Vec::new())
        }
        InternalAction::GainPanache { amount } => {
            state.player.powers.panache += amount;
            Ok(Vec::new())
        }
        InternalAction::GainCombust { amount } => {
            state.player.powers.combust += 1;
            state.player.powers.combust_damage += amount;
            Ok(Vec::new())
        }
        InternalAction::GainDoubleTap { amount } => {
            state.double_tap_pending += amount;
            Ok(Vec::new())
        }
        InternalAction::GainFireBreathing { amount } => {
            state.player.powers.fire_breathing += amount;
            Ok(Vec::new())
        }
        InternalAction::GainCorruption { amount } => {
            state.player.powers.corruption = state.player.powers.corruption.max(amount);
            Ok(Vec::new())
        }
        InternalAction::GainSadisticNature { amount } => {
            state.player.powers.sadistic_nature += amount;
            Ok(Vec::new())
        }
        InternalAction::GainMagnetism { amount } => {
            state.player.powers.magnetism += amount;
            Ok(Vec::new())
        }
        InternalAction::ArmTheBomb { turns, damage } => {
            state.bomb_timers.push(BombTimer {
                turns_remaining: turns,
                damage,
            });
            Ok(Vec::new())
        }
        InternalAction::DealUnmodifiedDamage { target, amount } => {
            deal_unmodified_damage_to_living_monster(state, target, amount)?;
            Ok(Vec::new())
        }
        InternalAction::GainMetallicize { amount } => {
            state.player.powers.metallicize += amount;
            Ok(Vec::new())
        }
        InternalAction::GainStrength { amount } => {
            state.player.powers.strength += amount;
            Ok(Vec::new())
        }
        InternalAction::GainDexterity { amount } => {
            state.player.powers.dexterity += amount;
            Ok(Vec::new())
        }
        InternalAction::GainTempStrength { amount } => {
            // Flex applies Strength and a debuff that removes it at end of
            // turn. Artifact blocks that debuff when it is created, consuming
            // one Artifact and leaving the gained Strength permanent.
            if state.player.powers.artifact > 0 {
                state.player.powers.artifact -= 1;
                state.player.powers.strength += amount;
            } else {
                state.player.temp_strength += amount;
            }
            Ok(Vec::new())
        }
        InternalAction::GainIntangible { amount } => {
            state.player.powers.intangible += amount;
            Ok(Vec::new())
        }
        InternalAction::GainRitual { amount } => {
            state.player.powers.ritual += amount;
            Ok(Vec::new())
        }
        InternalAction::GainArtifact { amount } => {
            state.player.powers.artifact += amount;
            Ok(Vec::new())
        }
        InternalAction::UpgradeCombatCards => {
            upgrade_combat_cards(state);
            Ok(Vec::new())
        }
        InternalAction::CardExhausted { card_id } => {
            apply_on_exhaust_effects(state, card_id);
            Ok(dead_branch_follow_up(state).into_iter().collect())
        }
        InternalAction::HandCardExhausted { card_id } => {
            apply_on_exhaust_effects(state, card_id);
            Ok(dead_branch_follow_up_before_pending_draw(state)
                .into_iter()
                .collect())
        }
        InternalAction::PlayTopDrawCard {
            target,
            exhaust_played_card,
            random_living_target,
        } => apply_play_top_draw_card(state, target, exhaust_played_card, random_living_target),
        InternalAction::PutHandCardOnTopOfDraw { card_id } => {
            let card = remove_card_from_pile(state, card_id, CardPile::Hand)?;
            state.piles.draw_pile.insert(0, card);
            Ok(Vec::new())
        }
        InternalAction::CopyHandCardToHand { card_id } => {
            let card = find_hand_card(state, card_id)?;
            let next_id = CardId::new(state.piles.max_card_instance_id() + 1);
            state
                .piles
                .hand
                .push(CardInstance::new(next_id, card.content_id));
            Ok(Vec::new())
        }
        InternalAction::AwaitHandSelect {
            source_card_id,
            purpose,
        } => {
            if purpose == HandSelectPurpose::WarcryPutOnDraw
                && !state
                    .piles
                    .hand
                    .iter()
                    .any(|card| card.id != source_card_id)
            {
                finish_warcry_source(state, source_card_id)?;
                return Ok(Vec::new());
            }
            state.hand_select = Some(crate::combat::HandSelectState {
                purpose,
                source_card_id,
                selected_hand_index: None,
                selected_hand_indices: Vec::new(),
            });
            Ok(Vec::new())
        }
        InternalAction::AwaitDrawSelect {
            source_card_id,
            purpose,
        } => {
            state.draw_select = Some(crate::combat::DrawSelectState {
                purpose,
                source_card_id,
                selected_draw_index: None,
            });
            Ok(Vec::new())
        }
        InternalAction::AwaitDiscardSelect {
            source_card_id,
            purpose,
        } => {
            if purpose == DiscardSelectPurpose::HeadbuttPutOnDraw {
                let source_card = if let Some(index) = state
                    .piles
                    .hand
                    .iter()
                    .position(|card| card.id == source_card_id)
                {
                    Some(state.piles.hand.remove(index))
                } else if let Some(index) = state
                    .piles
                    .discard_pile
                    .iter()
                    .position(|card| card.id == source_card_id)
                {
                    Some(state.piles.discard_pile.remove(index))
                } else if state
                    .piles
                    .exhaust_pile
                    .iter()
                    .any(|card| card.id == source_card_id)
                {
                    None
                } else {
                    return Err(SimError::IllegalAction(
                        "Headbutt source card is not in a playable destination",
                    ));
                };

                if state.monsters.iter().all(|monster| !monster.alive) {
                    if let Some(source_card) = source_card {
                        state.piles.discard_pile.push(source_card);
                    }
                    return Ok(Vec::new());
                }
                if state.piles.discard_pile.is_empty() {
                    if let Some(source_card) = source_card {
                        state.piles.discard_pile.push(source_card);
                    }
                    return Ok(Vec::new());
                }
                if state.piles.discard_pile.len() == 1 {
                    let selected = state.piles.discard_pile.remove(0);
                    state.piles.draw_pile.push(selected);
                    if let Some(source_card) = source_card {
                        state.piles.discard_pile.push(source_card);
                    }
                    return Ok(Vec::new());
                }
                state.discard_select = Some(crate::combat::DiscardSelectState {
                    purpose,
                    source_card_id: source_card.map(|_| source_card_id),
                    source_card,
                    selected_discard_indices: Vec::new(),
                    max_choices: 1,
                    selected_discard_index: None,
                });
                return Ok(Vec::new());
            }
            state.discard_select = Some(crate::combat::DiscardSelectState {
                purpose,
                source_card_id: Some(source_card_id),
                source_card: None,
                selected_discard_indices: Vec::new(),
                max_choices: 1,
                selected_discard_index: None,
            });
            Ok(Vec::new())
        }
        InternalAction::AwaitExhaustSelect {
            source_card_id,
            purpose,
        } => {
            let source_card = if matches!(
                purpose,
                crate::combat::ExhaustSelectPurpose::BurningPactDraw2
                    | crate::combat::ExhaustSelectPurpose::BurningPactDraw3
                    | crate::combat::ExhaustSelectPurpose::ExhumeReturnToHand
            ) {
                Some(remove_card_from_pile(
                    state,
                    source_card_id,
                    CardPile::Hand,
                )?)
            } else if purpose == crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3 {
                state
                    .piles
                    .hand
                    .iter()
                    .position(|card| card.id == source_card_id)
                    .map(|index| state.piles.hand.remove(index))
            } else {
                None
            };
            state.exhaust_select = Some(crate::combat::ExhaustSelectState {
                purpose,
                source_card_id: Some(source_card_id),
                source_card,
                selected_hand_indices: Vec::new(),
            });
            Ok(Vec::new())
        }
        InternalAction::OpenDiscoveryCardReward { source_card_id } => {
            state.discovery_source_card = state
                .piles
                .hand
                .iter()
                .position(|card| card.id == source_card_id)
                .map(|index| state.piles.hand.remove(index));
            Ok(Vec::new())
        }
    }
}

fn apply_mummified_hand_on_power_play(
    state: &mut CombatState,
    played_card_id: CardId,
    card_type: CardType,
) {
    if card_type != CardType::Power || !state.relics.contains(&Relic::MummifiedHand) {
        return;
    }

    let candidates = state
        .piles
        .hand
        .iter()
        .enumerate()
        .filter_map(|(index, card)| {
            if card.id == played_card_id {
                return None;
            }
            let definition = get_card_definition(card.content_id)?;
            let cost_for_turn = card.temp_cost.map_or(definition.cost, |cost| cost as i8);
            (definition.cost > 0 && cost_for_turn > 0).then_some(index)
        })
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        return;
    }

    let pick = state
        .card_random_rng
        .as_mut()
        .map(|rng| rng.random_int_range(0, (candidates.len() - 1) as i32) as usize)
        .unwrap_or(0);
    let card = &mut state.piles.hand[candidates[pick]];
    card.temp_cost = Some(0);
    card.temp_cost_turn_only = true;
}

fn apply_on_card_play_powers(state: &mut CombatState, card_type: CardType) -> Vec<InternalAction> {
    let mut follow_ups = Vec::new();

    for monster in state.monsters.iter_mut().filter(|monster| monster.alive) {
        if monster.content_id == GIANT_HEAD_ID || monster.powers.slow > 0 {
            monster.powers.slow += 1;
        }
    }

    if card_type == CardType::Attack {
        let sharp_hide_damage: i32 = state
            .monsters
            .iter()
            .filter(|monster| {
                monster.alive && monster.content_id == GUARDIAN_ID && monster.powers.spikes > 0
            })
            .map(|monster| monster.powers.spikes)
            .sum();
        state.pending_player_spikes_damage += sharp_hide_damage;
    }

    if state.player.powers.hex > 0 && card_type != CardType::Attack {
        for _ in 0..state.player.powers.hex {
            follow_ups.push(InternalAction::AddGeneratedCardToDrawPileRandomSpot {
                content_id: DAZED_ID,
            });
        }
    }

    if state.player.powers.panache <= 0 {
        return follow_ups;
    }
    state.player.powers.panache_cards_played += 1;
    if state.player.powers.panache_cards_played < 5 {
        return follow_ups;
    }

    state.player.powers.panache_cards_played = 0;
    let amount = state.player.powers.panache;
    follow_ups.extend(
        state
            .monsters
            .iter()
            .filter(|monster| monster.alive)
            .map(|monster| InternalAction::DealUnmodifiedDamage {
                target: monster.id,
                amount,
            }),
    );
    follow_ups
}

fn apply_hand_card_play_triggers(
    state: &CombatState,
    played_card_id: CardId,
) -> Vec<InternalAction> {
    state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != played_card_id && card.content_id == PAIN_ID)
        .map(|card| InternalAction::LoseHp {
            amount: 1,
            source: HpLossSource::Card(card.id),
        })
        .collect()
}

fn apply_copied_card_play_triggers(state: &CombatState) -> Vec<InternalAction> {
    state
        .piles
        .hand
        .iter()
        .filter(|card| card.content_id == PAIN_ID)
        .map(|card| InternalAction::LoseHp {
            amount: 1,
            source: HpLossSource::Card(card.id),
        })
        .collect()
}

fn deal_attack_damage_to_all_living(
    state: &mut CombatState,
    source: CardId,
    amount: i32,
) -> SimResult<(i32, Vec<InternalAction>)> {
    let player_powers = state.player.powers;
    let temp_strength = state.player.temp_strength;
    let relics = state.relics.clone();
    let targets: Vec<(MonsterId, ContentId, i32)> = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| (monster.id, monster.content_id, monster.powers.spikes))
        .collect();
    let mut total_hp_damage = 0;
    let mut follow_ups = Vec::new();

    for (target, monster_content_id, spikes) in targets {
        let (hp_damage, still_alive, hand_drill_applies, malleable_block) = {
            let monster = living_monster_mut(state, target)?;
            let damage = deal_damage_info_to_monster_with_result(
                monster,
                DamageInfo {
                    source: DamageSource::Card(source),
                    target,
                    amount,
                },
                player_powers,
                temp_strength,
                &relics,
            );
            wake_lagavulin_on_damage(monster, damage.hp_damage);
            guardian_accumulate_hp_damage(monster, damage.hp_damage);
            (
                damage.hp_damage,
                monster.alive,
                relics.contains(&crate::Relic::HandDrill) && damage.broke_block,
                damage.malleable_block,
            )
        };
        push_malleable_block_follow_up(
            state,
            &mut follow_ups,
            target,
            monster_content_id,
            still_alive,
            malleable_block,
        );
        if still_alive && hand_drill_applies {
            apply_player_vulnerable_debuff(state, target, crate::relic::HAND_DRILL_VULNERABLE)?;
        }
        total_hp_damage += hp_damage;
        check_slime_boss_split(state, target);
        if !still_alive {
            apply_monster_death_hooks(state, target);
        }
        apply_or_queue_spikes_to_player(state, monster_content_id, spikes);
    }

    Ok((total_hp_damage, follow_ups))
}

fn apply_or_queue_spikes_to_player(
    state: &mut CombatState,
    monster_content_id: ContentId,
    spikes: i32,
) {
    if spikes <= 0 {
        return;
    }
    if monster_content_id == GUARDIAN_ID {
        return;
    }
    let hp_loss = reflect_spikes_to_player(&mut state.player, &state.relics, spikes);
    crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss);
}

fn push_malleable_block_follow_up(
    state: &mut CombatState,
    follow_ups: &mut Vec<InternalAction>,
    target: MonsterId,
    monster_content_id: ContentId,
    still_alive: bool,
    malleable_block: Option<i32>,
) {
    if still_alive
        && malleable_block.is_some()
        && monster_content_id == crate::content::monsters::WRITHING_MASS_ID
    {
        crate::combat::turn::reroll_writhing_mass_after_attack(state, target);
    }
    if still_alive {
        if let Some(amount) = malleable_block {
            follow_ups.push(InternalAction::GainMonsterBlock { target, amount });
        }
    }
}

fn deal_unmodified_damage_to_living_monster(
    state: &mut CombatState,
    target: MonsterId,
    amount: i32,
) -> SimResult<()> {
    let still_alive = {
        let monster = living_monster_mut(state, target)?;
        let hp_damage = deal_unmodified_damage_to_monster(monster, amount);
        wake_lagavulin_on_damage(monster, hp_damage);
        guardian_accumulate_hp_damage(monster, hp_damage);
        monster.alive
    };
    check_slime_boss_split(state, target);
    if !still_alive {
        apply_monster_death_hooks(state, target);
    }
    Ok(())
}

pub(crate) fn apply_monster_death_hooks(state: &mut CombatState, monster_id: MonsterId) {
    apply_monster_death_non_relic_hooks(state, monster_id);
    if state.monsters.iter().any(|monster| monster.alive) {
        crate::relic::apply_monster_death_relics(state);
    }
}

fn queue_monster_death_hooks(state: &mut CombatState, monster_id: MonsterId) {
    apply_monster_death_non_relic_hooks(state, monster_id);
    if state.monsters.iter().any(|monster| monster.alive)
        && state.relics.contains(&Relic::GremlinHorn)
    {
        state.pending_monster_death_relic_triggers += 1;
    }
}

fn apply_monster_death_non_relic_hooks(state: &mut CombatState, monster_id: MonsterId) {
    if let Some(monster) = state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == monster_id)
    {
        release_stasis_card_on_death(monster, &mut state.piles);
    }
    apply_gremlin_leader_death_escape(&mut state.monsters, monster_id);
    apply_collector_death_escape(&mut state.monsters, monster_id);
    apply_spore_cloud_on_monster_death(state, monster_id);
}

fn apply_spore_cloud_on_monster_death(state: &mut CombatState, monster_id: MonsterId) {
    let amount = state
        .monsters
        .iter()
        .find(|monster| monster.id == monster_id)
        .map_or(0, |monster| monster.powers.spore_cloud);
    if amount <= 0 || !state.monsters.iter().any(|monster| monster.alive) {
        return;
    }

    apply_player_vulnerable(&mut state.player.powers, amount);
}

fn apply_sadistic_nature_after_monster_debuff(
    state: &mut CombatState,
    target: MonsterId,
    applied: bool,
) -> SimResult<()> {
    if !applied || state.player.powers.sadistic_nature <= 0 {
        return Ok(());
    }

    deal_unmodified_damage_to_living_monster(state, target, state.player.powers.sadistic_nature)
}

fn apply_player_vulnerable_debuff(
    state: &mut CombatState,
    target: MonsterId,
    amount: i32,
) -> SimResult<()> {
    let applies_champion_belt = state.relics.contains(&crate::Relic::ChampionBelt);
    let mut vulnerable_applied = false;
    let mut champion_belt_weak_applied = false;
    if let Some(monster) = living_monster_mut_opt(state, target) {
        vulnerable_applied = apply_monster_vulnerable(&mut monster.powers, amount);
        if vulnerable_applied && applies_champion_belt {
            champion_belt_weak_applied =
                apply_monster_weak(&mut monster.powers, crate::relic::CHAMPION_BELT_WEAK);
        }
    }

    apply_sadistic_nature_after_monster_debuff(state, target, vulnerable_applied)?;
    apply_sadistic_nature_after_monster_debuff(state, target, champion_belt_weak_applied)
}

fn juggernaut_follow_up_for_positive_block_gain(
    state: &mut CombatState,
    gained: i32,
) -> Vec<InternalAction> {
    if gained <= 0 || state.player.powers.juggernaut <= 0 {
        return Vec::new();
    }
    random_living_monster_id(state)
        .map(|target| {
            vec![InternalAction::DealUnmodifiedDamage {
                target,
                amount: state.player.powers.juggernaut,
            }]
        })
        .unwrap_or_default()
}

pub(crate) fn apply_juggernaut_after_direct_block_gain(state: &mut CombatState, gained: i32) {
    if let Some(InternalAction::DealUnmodifiedDamage { target, amount }) =
        juggernaut_follow_up_for_positive_block_gain(state, gained)
            .into_iter()
            .next()
    {
        let _ = deal_unmodified_damage_to_living_monster(state, target, amount);
    }
}

fn apply_player_card_block_gain(state: &mut CombatState, amount: i32) -> Vec<InternalAction> {
    if state.player.no_block_turns > 0 {
        return Vec::new();
    }
    let gained = calculate_block(amount, state.player.powers);
    state.player.block += gained;
    juggernaut_follow_up_for_positive_block_gain(state, gained)
}

pub(crate) fn apply_player_direct_block_gain(state: &mut CombatState, amount: i32) {
    if state.player.no_block_turns > 0 {
        return;
    }
    state.player.block += amount;
    apply_juggernaut_after_direct_block_gain(state, amount);
}

fn random_living_monster_id(state: &mut CombatState) -> Option<MonsterId> {
    let living: Vec<_> = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster.id)
        .collect();
    if living.is_empty() {
        return None;
    }
    let Some(rng) = state.card_random_rng.as_mut() else {
        return living.first().copied();
    };
    let index = rng.random_int((living.len() - 1) as i32) as usize;
    living.get(index).copied()
}

fn random_hand_card_id_except(state: &mut CombatState, excluded_card_id: CardId) -> Option<CardId> {
    let candidates = state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != excluded_card_id)
        .map(|card| card.id)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }
    let index = if let Some(rng) = state.card_random_rng.as_mut() {
        rng.random_int((candidates.len() - 1) as i32) as usize
    } else {
        0
    };
    candidates.get(index).copied()
}

fn dead_branch_follow_up(state: &mut CombatState) -> Option<InternalAction> {
    if !state.relics.contains(&Relic::DeadBranch)
        || !state.monsters.iter().any(|monster| monster.alive)
    {
        return None;
    }

    let pool = dead_branch_card_pool();
    let index = state
        .card_random_rng
        .as_mut()
        .map(|rng| rng.random_int((pool.len() - 1) as i32) as usize)
        .unwrap_or(0);
    Some(InternalAction::AddGeneratedCardToPile {
        content_id: pool[index],
        to: CardPile::Hand,
        temp_cost: None,
        temp_cost_turn_only: false,
    })
}

fn dead_branch_follow_up_before_pending_draw(state: &mut CombatState) -> Option<InternalAction> {
    match dead_branch_follow_up(state) {
        Some(InternalAction::AddGeneratedCardToPile {
            content_id,
            to: CardPile::Hand,
            temp_cost,
            temp_cost_turn_only,
        }) => Some(InternalAction::AddGeneratedHandCardBeforePendingDraw {
            content_id,
            temp_cost,
            temp_cost_turn_only,
        }),
        other => other,
    }
}

fn dead_branch_card_pool() -> Vec<ContentId> {
    ironclad_combat_discovery_pool().to_vec()
}

pub(crate) fn apply_on_exhaust_effects(state: &mut CombatState, card_id: CardId) {
    match exhausted_card_content_id(state, card_id) {
        Some(SENTINEL_PLUS_ID) => state.player.energy += 3,
        Some(SENTINEL_ID) => state.player.energy += 2,
        _ => {}
    }
    if state.player.powers.feel_no_pain > 0 {
        let gained = state.player.powers.feel_no_pain;
        apply_player_direct_block_gain(state, gained);
    }
    if state.player.powers.dark_embrace > 0 {
        player_draw_cards(state, state.player.powers.dark_embrace as usize);
    }
    if state.relics.contains(&Relic::CharonsAshes) {
        let targets = state
            .monsters
            .iter()
            .filter(|monster| monster.alive)
            .map(|monster| monster.id)
            .collect::<Vec<_>>();
        for target in targets {
            let still_alive = {
                let monster = living_monster_mut(state, target)
                    .expect("target was collected from living monsters");
                let hp_damage =
                    deal_unmodified_damage_to_monster(monster, crate::relic::CHARONS_ASHES_DAMAGE);
                wake_lagavulin_on_damage(monster, hp_damage);
                guardian_accumulate_hp_damage(monster, hp_damage);
                monster.alive
            };
            check_slime_boss_split(state, target);
            if !still_alive {
                apply_monster_death_hooks(state, target);
            }
        }
    }
}

fn exhausted_card_content_id(state: &CombatState, card_id: CardId) -> Option<ContentId> {
    state
        .piles
        .exhaust_pile
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.content_id)
}

pub(crate) fn player_draw_cards(state: &mut CombatState, count: usize) {
    if state.player.cannot_draw {
        return;
    }
    if let Some(mut rng) = state.shuffle_rng.take() {
        crate::combat::draw::draw_cards_with_sts_rng(state, count, &mut rng);
        state.shuffle_rng = Some(rng);
    } else {
        let mut rng = SimulatorRng::new(0);
        crate::combat::draw::draw_cards(state, count, &mut rng);
    }
}

pub(crate) fn player_shuffle_discard_into_draw(state: &mut CombatState) {
    if let Some(mut rng) = state.shuffle_rng.take() {
        crate::combat::draw::shuffle_discard_into_draw_sts(state, &mut rng);
        state.shuffle_rng = Some(rng);
    } else {
        let mut rng = SimulatorRng::new(0);
        crate::combat::draw::shuffle_discard_into_draw(state, &mut rng);
    }
}

pub(crate) fn player_deep_breath_shuffle_discard_into_draw(state: &mut CombatState) {
    if let Some(mut rng) = state.shuffle_rng.take() {
        crate::combat::draw::deep_breath_shuffle_discard_into_draw_sts(state, &mut rng);
        state.shuffle_rng = Some(rng);
    } else {
        let mut rng = SimulatorRng::new(0);
        crate::combat::draw::deep_breath_shuffle_discard_into_draw(state, &mut rng);
    }
}

fn hand_contains_attack(state: &CombatState) -> bool {
    state.piles.hand.iter().any(|card| {
        get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == CardType::Attack)
    })
}

fn draw_random_attacks_from_draw_pile(state: &mut CombatState, count: usize) {
    let mut attack_ids = Vec::new();
    for card in &state.piles.draw_pile {
        let is_attack = get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == CardType::Attack);
        if !is_attack {
            continue;
        }

        if attack_ids.is_empty() {
            attack_ids.push(card.id);
        } else {
            let index = state
                .card_random_rng
                .as_mut()
                .map_or(attack_ids.len(), |rng| {
                    // CardGroup::addToRandomSpot asks cardRandomRng for an
                    // inclusive index in 0..=(size - 1). It never appends to
                    // the end once the temporary group is non-empty.
                    rng.random_int((attack_ids.len() - 1) as i32) as usize
                });
            attack_ids.insert(index, card.id);
        }
    }

    for _ in 0..count {
        if attack_ids.is_empty() {
            return;
        }

        if let Some(rng) = state.shuffle_rng.as_mut() {
            let shuffle_seed = rng.random_long();
            JavaRng::new(shuffle_seed).collections_shuffle(&mut attack_ids);
        }

        let selected_id = attack_ids.remove(0);
        let Some(draw_index) = state
            .piles
            .draw_pile
            .iter()
            .position(|card| card.id == selected_id)
        else {
            continue;
        };
        let card = state.piles.draw_pile.remove(draw_index);
        if state.piles.hand.len() >= 10 {
            state.piles.discard_pile.push(card);
        } else {
            state.piles.hand.push(card);
        }
    }
}

fn apply_unceasing_top_after_hand_emptied(state: &mut CombatState) {
    if state.relics.contains(&Relic::UnceasingTop) {
        player_draw_cards(state, crate::relic::UNCEASING_TOP_DRAW);
    }
}

fn add_card_to_pile(state: &mut CombatState, content_id: ContentId, to: CardPile) {
    let next_id = CardId::new(state.piles.max_card_instance_id() + 1);
    let card = CardInstance::new(next_id, content_id);
    push_card_to_pile(state, card, to);
}

fn add_generated_card_to_pile(
    state: &mut CombatState,
    content_id: ContentId,
    to: CardPile,
    temp_cost: Option<u8>,
    temp_cost_turn_only: bool,
) {
    let next_id = CardId::new(state.piles.max_card_instance_id() + 1);
    let mut card = CardInstance {
        combat_only: true,
        ..CardInstance::new(next_id, content_id)
    };
    card.temp_cost = temp_cost;
    card.temp_cost_turn_only = temp_cost_turn_only;
    let destination = if to == CardPile::Hand && state.piles.hand.len() >= MAX_HAND_SIZE {
        CardPile::DiscardPile
    } else {
        to
    };
    push_card_to_pile(state, card, destination);
}

fn add_stat_equivalent_copy_to_pile(state: &mut CombatState, source: CardInstance, to: CardPile) {
    let next_id = CardId::new(state.piles.max_card_instance_id() + 1);
    let mut copy = source;
    copy.id = next_id;
    copy.bottled = false;
    // AbstractCard.makeStatEquivalentCopy() starts from makeCopy() and does
    // not copy purgeOnUse. Anger copies therefore keep cycling through combat
    // piles after play even when their source was temporary.
    copy.combat_only = false;
    let destination = if to == CardPile::Hand && state.piles.hand.len() >= MAX_HAND_SIZE {
        CardPile::DiscardPile
    } else {
        to
    };
    push_card_to_pile(state, copy, destination);
}

fn add_generated_card_to_draw_pile_random_spot(
    state: &mut CombatState,
    content_id: ContentId,
    temp_cost: Option<u8>,
    temp_cost_turn_only: bool,
) {
    let next_id = CardId::new(state.piles.max_card_instance_id() + 1);
    let mut card = CardInstance {
        combat_only: true,
        ..CardInstance::new(next_id, content_id)
    };
    card.temp_cost = temp_cost;
    card.temp_cost_turn_only = temp_cost_turn_only;
    if state.piles.draw_pile.is_empty() {
        state.piles.draw_pile.push(card);
        return;
    }
    let index = state
        .card_random_rng
        .as_mut()
        .map(|rng| rng.random_int((state.piles.draw_pile.len() - 1) as i32) as usize)
        .unwrap_or(0);
    state.piles.draw_pile.insert(index, card);
}

fn generated_card_zero_cost_if_positive(content_id: ContentId) -> Option<u8> {
    get_card_definition(content_id).and_then(|definition| (definition.cost > 0).then_some(0))
}

fn random_colorless_card(state: &mut CombatState) -> ContentId {
    let pool = colorless_discovery_pool();
    if let Some(rng) = state.card_random_rng.as_mut() {
        let idx = rng.random_int((pool.len() - 1) as i32) as usize;
        return pool[idx];
    }
    pool[0]
}

fn push_card_to_pile(state: &mut CombatState, card: CardInstance, to: CardPile) {
    match to {
        CardPile::DiscardPile => state.piles.discard_pile.push(card),
        CardPile::DrawPile => state.piles.draw_pile.push(card),
        CardPile::Hand => state.piles.hand.push(card),
        CardPile::ExhaustPile => state.piles.exhaust_pile.push(card),
    }
}

fn living_monster_mut(state: &mut CombatState, target: MonsterId) -> SimResult<&mut MonsterState> {
    living_monster_mut_opt(state, target)
        .ok_or(SimError::IllegalAction("target is not a living monster"))
}

fn living_monster_alive(state: &CombatState, target: MonsterId) -> bool {
    state
        .monsters
        .iter()
        .any(|monster| monster.id == target && monster.alive)
}

fn living_monster_mut_opt(state: &mut CombatState, target: MonsterId) -> Option<&mut MonsterState> {
    state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == target && monster.alive)
}

fn apply_enrage_on_card_type(state: &mut CombatState, card_type: CardType) {
    if card_type != CardType::Skill {
        return;
    }

    for monster in &mut state.monsters {
        if !monster.alive {
            continue;
        }
        if get_monster_definition(monster.content_id).is_some_and(|definition| {
            definition.enrage_weak_on_skill > 0 && monster.powers.anger > 0
        }) {
            monster.powers.strength += monster.powers.anger;
        }
    }
}

fn apply_rage_on_card_type(state: &mut CombatState, card_type: CardType) {
    if card_type == CardType::Attack && state.player.temp_rage_block > 0 {
        apply_player_direct_block_gain(state, state.player.temp_rage_block);
    }
}

fn set_random_hand_card_cost_for_combat(state: &mut CombatState, amount: u8) {
    if state.piles.hand.is_empty() {
        return;
    }

    let better_possible = state
        .piles
        .hand
        .iter()
        .any(|card| card_cost_for_turn(card) > 0);
    let possible = state
        .piles
        .hand
        .iter()
        .any(|card| card_printed_cost(card) > 0);
    if !better_possible && !possible {
        return;
    }

    let Some(index) = random_madness_candidate_index(state, better_possible) else {
        return;
    };

    let card = &mut state.piles.hand[index];
    card.temp_cost = Some(amount);
    card.temp_cost_turn_only = false;
}

fn random_madness_candidate_index(state: &mut CombatState, better_possible: bool) -> Option<usize> {
    if state.card_random_rng.is_none() {
        return state
            .piles
            .hand
            .iter()
            .position(|card| madness_card_matches(card, better_possible));
    }

    loop {
        let index = if let Some(rng) = state.card_random_rng.as_mut() {
            rng.random_int((state.piles.hand.len() - 1) as i32) as usize
        } else {
            unreachable!("handled missing Madness card_random_rng");
        };
        if madness_card_matches(&state.piles.hand[index], better_possible) {
            return Some(index);
        }
    }
}

fn madness_card_matches(card: &CardInstance, better_possible: bool) -> bool {
    if better_possible {
        card_cost_for_turn(card) > 0
    } else {
        card_printed_cost(card) > 0
    }
}

fn card_cost_for_turn(card: &CardInstance) -> i8 {
    card.temp_cost
        .map(|cost| cost as i8)
        .or_else(|| get_card_definition(card.content_id).map(|definition| definition.cost))
        .unwrap_or(0)
}

fn card_printed_cost(card: &CardInstance) -> i8 {
    get_card_definition(card.content_id)
        .map(|definition| definition.cost)
        .unwrap_or(0)
}

fn draw_pile_has_card_type(state: &CombatState, card_type: CardType) -> bool {
    state.piles.draw_pile.iter().any(|card| {
        get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == card_type)
    })
}

fn upgrade_hand_cards_except(state: &mut CombatState, excluded_card_id: CardId) {
    for card in &mut state.piles.hand {
        if card.id == excluded_card_id {
            continue;
        }
        if let Some(upgraded) = upgrade_card_instance(*card) {
            *card = upgraded;
        }
    }
}

fn upgrade_hand_card(state: &mut CombatState, card_id: CardId) -> SimResult<()> {
    let card = find_hand_card_mut(state, card_id)?;
    *card = upgrade_card_instance(*card).ok_or(SimError::IllegalAction("card cannot upgrade"))?;
    Ok(())
}

fn apply_play_top_draw_card(
    state: &mut CombatState,
    target: Option<MonsterId>,
    exhaust_played_card: bool,
    random_living_target: bool,
) -> SimResult<Vec<InternalAction>> {
    if state.piles.draw_pile.is_empty() {
        if state.piles.discard_pile.is_empty() {
            return Ok(Vec::new());
        }
        player_shuffle_discard_into_draw(state);
    }

    let mut card = state
        .piles
        .draw_pile
        .pop()
        .ok_or(SimError::IllegalAction("draw pile is empty"))?;
    let card_id = card.id;
    let definition =
        get_card_definition(card.content_id).ok_or(SimError::UnknownContent(card.content_id))?;

    let random_target = random_living_target.then(|| random_living_monster_id(state));
    let target = target.or_else(|| {
        if definition.target == TargetRequirement::Enemy {
            random_target.flatten()
        } else {
            None
        }
    });

    if !crate::relic::can_play_card_with_relics(state) {
        let mut follow_ups = Vec::new();
        if exhaust_played_card || definition.keywords.exhaust {
            state.piles.exhaust_pile.push(card);
            follow_ups.push(InternalAction::CardExhausted { card_id });
        } else if !card.combat_only {
            state.piles.discard_pile.push(card);
        }
        return Ok(follow_ups);
    }

    card_effects::validate_havoc_target(definition, target, false)?;
    apply_enrage_on_card_type(state, definition.card_type);
    apply_rage_on_card_type(state, definition.card_type);

    let mut follow_ups = Vec::new();
    match definition.id {
        STRIKE_R_ID
        | STRIKE_R_PLUS_ID
        | ANGER_ID
        | ANGER_PLUS_ID
        | POMMEL_STRIKE_ID
        | POMMEL_STRIKE_PLUS_ID
        | FLASH_OF_STEEL_ID
        | FLASH_OF_STEEL_PLUS_ID
        | SEARING_BLOW_ID
        | SEARING_BLOW_PLUS_ID
        | BASH_ID
        | CLASH_ID
        | CLASH_PLUS_ID
        | CARNAGE_ID
        | CARNAGE_PLUS_ID
        | DROPKICK_ID
        | DROPKICK_PLUS_ID
        | HEMOKINESIS_ID
        | HEMOKINESIS_PLUS_ID
        | RECKLESS_CHARGE_ID
        | RECKLESS_CHARGE_PLUS_ID
        | WILD_STRIKE_ID
        | WILD_STRIKE_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: searing_blow_card_damage(&card)
                        .unwrap_or_else(|| definition.values.damage.unwrap_or(0)),
                },
            });
            if definition.id == HEMOKINESIS_ID || definition.id == HEMOKINESIS_PLUS_ID {
                follow_ups.push(InternalAction::LoseHp {
                    amount: 2,
                    source: HpLossSource::Card(card_id),
                });
            }
            if definition.id == RECKLESS_CHARGE_ID || definition.id == RECKLESS_CHARGE_PLUS_ID {
                follow_ups.push(InternalAction::AddGeneratedCardToDrawPileRandomSpot {
                    content_id: DAZED_ID,
                });
            }
            if definition.id == ANGER_ID || definition.id == ANGER_PLUS_ID {
                follow_ups.push(InternalAction::AddStatEquivalentCopyToPile {
                    card,
                    to: CardPile::DiscardPile,
                });
            }
            if definition.id == WILD_STRIKE_ID || definition.id == WILD_STRIKE_PLUS_ID {
                follow_ups.push(InternalAction::AddGeneratedCardToDrawPileRandomSpot {
                    content_id: WOUND_ID,
                });
            }
            if definition.id == FLASH_OF_STEEL_ID || definition.id == FLASH_OF_STEEL_PLUS_ID {
                follow_ups.push(InternalAction::DrawCards { count: 1 });
            }
            if definition.id == POMMEL_STRIKE_ID || definition.id == POMMEL_STRIKE_PLUS_ID {
                follow_ups.push(InternalAction::DrawCards {
                    count: if definition.id == POMMEL_STRIKE_PLUS_ID {
                        2
                    } else {
                        1
                    },
                });
            }
            if definition.id == DROPKICK_ID || definition.id == DROPKICK_PLUS_ID {
                let target_has_vulnerable = state
                    .monsters
                    .iter()
                    .find(|monster| monster.id == target)
                    .map(|monster| monster.powers.vulnerable > 0)
                    .unwrap_or(false);
                if target_has_vulnerable {
                    follow_ups.push(InternalAction::GainEnergy { amount: 1 });
                    follow_ups.push(InternalAction::DrawCards { count: 1 });
                }
            }
            if definition.id == BASH_ID {
                follow_ups.push(InternalAction::ApplyVulnerable {
                    target,
                    amount: definition.values.vulnerable.unwrap_or(0),
                });
            }
        }
        FEED_ID | FEED_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            follow_ups.push(InternalAction::DealFeedDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: definition.values.damage.unwrap_or(0),
                },
                max_hp_gain: if definition.id == FEED_PLUS_ID { 4 } else { 3 },
            });
        }
        HEADBUTT_ID | HEADBUTT_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: definition.values.damage.unwrap_or(0),
                },
            });
            if !state.piles.discard_pile.is_empty() {
                follow_ups.push(InternalAction::AwaitDiscardSelect {
                    source_card_id: card_id,
                    purpose: DiscardSelectPurpose::HeadbuttPutOnDraw,
                });
            }
        }
        RAMPAGE_ID | RAMPAGE_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: definition.values.damage.unwrap_or(0) + card.rampage_damage_bonus,
                },
            });
            // Rampage.upgradeDamage mutates the played card before
            // PlayTopCardAction moves it to exhaust.
            card.rampage_damage_bonus += if definition.id == RAMPAGE_PLUS_ID {
                8
            } else {
                5
            };
        }
        RITUAL_DAGGER_ID => {
            let target = target.expect("validated havoc attack target");
            follow_ups.push(InternalAction::DealRitualDaggerDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: ritual_dagger_card_damage(&card)
                        .unwrap_or_else(|| definition.values.damage.unwrap_or(0)),
                },
                growth: ritual_dagger_card_growth(&card).unwrap_or(3),
            });
        }
        IRON_WAVE_ID | IRON_WAVE_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: definition.values.damage.unwrap_or(0),
                },
            });
        }
        BODY_SLAM_ID | BODY_SLAM_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: state.player.block,
                },
            });
        }
        HEAVY_BLADE_ID | HEAVY_BLADE_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            let multiplier = if definition.id == HEAVY_BLADE_PLUS_ID {
                5
            } else {
                3
            };
            let extra_strength =
                (multiplier - 1) * (state.player.powers.strength + state.player.temp_strength);
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: (definition.values.damage.unwrap_or(0) + extra_strength).max(0),
                },
            });
        }
        PERFECTED_STRIKE_ID | PERFECTED_STRIKE_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            let strike_count =
                card_effects::combat_strike_named_card_count_with_extra(state, Some(definition))
                    as i32;
            let strike_bonus = if definition.id == PERFECTED_STRIKE_PLUS_ID {
                3
            } else {
                2
            };
            let damage = definition.values.damage.unwrap_or(0) + (strike_bonus * strike_count);
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: crate::relic::strike_damage_with_relics(&state.relics, damage),
                },
            });
        }
        PUMMEL_ID | PUMMEL_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            let damage = definition.values.damage.unwrap_or(0);
            let hits = if definition.id == PUMMEL_PLUS_ID {
                5
            } else {
                4
            };
            for _ in 0..hits {
                follow_ups.push(InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(card_id),
                        target,
                        amount: damage,
                    },
                });
            }
        }
        SWORD_BOOMERANG_ID | SWORD_BOOMERANG_PLUS_ID => {
            let hits = if definition.id == SWORD_BOOMERANG_PLUS_ID {
                4
            } else {
                3
            };
            for _ in 0..hits {
                follow_ups.push(InternalAction::DealDamageRandomEnemy {
                    source: card_id,
                    amount: definition.values.damage.unwrap_or(0),
                });
            }
        }
        MIND_BLAST_ID | MIND_BLAST_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: i32::try_from(state.piles.draw_pile.len())
                        .expect("draw pile count fits in i32"),
                },
            });
        }
        MASTER_OF_STRATEGY_ID | MASTER_OF_STRATEGY_PLUS_ID => {
            let count = if definition.id == MASTER_OF_STRATEGY_PLUS_ID {
                4
            } else {
                3
            };
            follow_ups.push(InternalAction::DrawCards { count });
        }
        TWIN_STRIKE_ID | TWIN_STRIKE_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            let damage = definition.values.damage.unwrap_or(0);
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: damage,
                },
            });
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: damage,
                },
            });
        }
        CLEAVE_ID | CLEAVE_PLUS_ID | DRAMATIC_ENTRANCE_ID | DRAMATIC_ENTRANCE_PLUS_ID => {
            follow_ups.push(InternalAction::DealDamageAll {
                source: card_id,
                amount: definition.values.damage.unwrap_or(0),
            });
        }
        IMMOLATE_ID | IMMOLATE_PLUS_ID => {
            follow_ups.push(InternalAction::DealDamageAll {
                source: card_id,
                amount: definition.values.damage.unwrap_or(0),
            });
            follow_ups.push(InternalAction::AddGeneratedCardToPile {
                content_id: BURN_ID,
                to: CardPile::DiscardPile,
                temp_cost: None,
                temp_cost_turn_only: false,
            });
        }
        THUNDERCLAP_ID | THUNDERCLAP_PLUS_ID => {
            follow_ups.push(InternalAction::DealDamageAll {
                source: card_id,
                amount: definition.values.damage.unwrap_or(0),
            });
            for monster in state.monsters.iter().filter(|monster| monster.alive) {
                follow_ups.push(InternalAction::ApplyVulnerable {
                    target: monster.id,
                    amount: definition.values.vulnerable.unwrap_or(0),
                });
            }
        }
        REAPER_ID | REAPER_PLUS_ID => {
            follow_ups.push(InternalAction::DealDamageAllAndHealUnblocked {
                source: card_id,
                amount: definition.values.damage.unwrap_or(0),
            });
        }
        WHIRLWIND_ID | WHIRLWIND_PLUS_ID => {
            let chemical_x_bonus = if state.relics.contains(&Relic::ChemicalX) {
                crate::relic::CHEMICAL_X_BONUS_X
            } else {
                0
            };
            follow_ups.push(InternalAction::DealDamageAllRepeated {
                source: card_id,
                amount: definition.values.damage.unwrap_or(0),
                times: state.player.energy + chemical_x_bonus,
            });
        }
        DEFEND_R_ID => {
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
        }
        SHRUG_IT_OFF_ID | SHRUG_IT_OFF_PLUS_ID => {
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
            follow_ups.push(InternalAction::DrawCards { count: 1 });
        }
        IMPATIENCE_ID => {
            follow_ups.push(InternalAction::DrawCardsIfNoAttacksInHand { count: 2 });
        }
        IMPATIENCE_PLUS_ID => {
            follow_ups.push(InternalAction::DrawCardsIfNoAttacksInHand { count: 3 });
        }
        BATTLE_TRANCE_ID | BATTLE_TRANCE_PLUS_ID => {
            let count = if definition.id == BATTLE_TRANCE_PLUS_ID {
                4
            } else {
                3
            };
            follow_ups.push(InternalAction::DrawCards { count });
            follow_ups.push(InternalAction::SetCannotDraw);
        }
        CHRYSALIS_ID | CHRYSALIS_PLUS_ID => {
            let count = if definition.id == CHRYSALIS_PLUS_ID {
                5
            } else {
                3
            };
            for content_id in card_effects::chrysalis_generated_skills(state, count) {
                follow_ups.push(
                    InternalAction::AddGeneratedCardToDrawPileRandomSpotWithCost {
                        content_id,
                        temp_cost: generated_card_zero_cost_if_positive(content_id),
                        temp_cost_turn_only: false,
                    },
                );
            }
        }
        PANIC_BUTTON_ID | PANIC_BUTTON_PLUS_ID => {
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
            follow_ups.push(InternalAction::PreventBlockGain { turns: 2 });
        }
        ENTRENCH_ID | ENTRENCH_PLUS_ID => {
            follow_ups.push(InternalAction::DoublePlayerBlock);
        }
        FLAME_BARRIER_ID | FLAME_BARRIER_PLUS_ID => {
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
            follow_ups.push(InternalAction::GainTemporaryThorns {
                amount: card_effects::flame_barrier_thorns_amount(definition),
            });
        }
        FLEX_ID | FLEX_PLUS_ID => {
            follow_ups.push(InternalAction::GainTempStrength {
                amount: card_effects::flex_temp_strength_amount(definition),
            });
        }
        PURITY_ID | PURITY_PLUS_ID => {
            if !state.piles.hand.is_empty() {
                follow_ups.push(InternalAction::AwaitExhaustSelect {
                    source_card_id: card_id,
                    purpose: crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3,
                });
            }
        }
        CLOTHESLINE_ID | CLOTHESLINE_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: definition.values.damage.unwrap_or(0),
                },
            });
            follow_ups.push(InternalAction::ApplyWeak {
                target,
                amount: if definition.id == CLOTHESLINE_PLUS_ID {
                    3
                } else {
                    2
                },
            });
        }
        INTIMIDATE_ID | INTIMIDATE_PLUS_ID => {
            for monster in state.monsters.iter().filter(|monster| monster.alive) {
                follow_ups.push(InternalAction::ApplyWeak {
                    target: monster.id,
                    amount: if definition.id == INTIMIDATE_PLUS_ID {
                        2
                    } else {
                        1
                    },
                });
            }
        }
        DISARM_ID | DISARM_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            follow_ups.push(InternalAction::ReduceMonsterStrength {
                target,
                amount: if definition.id == DISARM_PLUS_ID {
                    3
                } else {
                    2
                },
            });
        }
        RAGE_ID | RAGE_PLUS_ID => {
            follow_ups.push(InternalAction::GainRage {
                amount: if definition.id == RAGE_PLUS_ID { 5 } else { 3 },
            });
        }
        SEVER_SOUL_ID | SEVER_SOUL_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            let exhausted = state
                .piles
                .hand
                .iter()
                .filter_map(|card| {
                    get_card_definition(card.content_id)
                        .filter(|definition| definition.card_type != CardType::Attack)
                        .map(|_| card.id)
                })
                .collect::<Vec<_>>();
            for card_id in exhausted {
                follow_ups.push(InternalAction::MoveCard {
                    card_id,
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                });
            }
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: definition.values.damage.unwrap_or(0),
                },
            });
        }
        FINESSE_ID => {
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
            follow_ups.push(InternalAction::DrawCards { count: 1 });
        }
        BLIND_PLUS_ID => {
            for monster in state.monsters.iter().filter(|monster| monster.alive) {
                follow_ups.push(InternalAction::ApplyWeak {
                    target: monster.id,
                    amount: 2,
                });
            }
        }
        TRIP_PLUS_ID => {
            for monster in state.monsters.iter().filter(|monster| monster.alive) {
                follow_ups.push(InternalAction::ApplyVulnerable {
                    target: monster.id,
                    amount: definition.values.vulnerable.unwrap_or(0),
                });
            }
        }
        DEEP_BREATH_ID | DEEP_BREATH_PLUS_ID => {
            let count = if definition.id == DEEP_BREATH_PLUS_ID {
                2
            } else {
                1
            };
            follow_ups.push(InternalAction::DeepBreathShuffleDiscardIntoDraw);
            follow_ups.push(InternalAction::DrawCards { count });
        }
        ENLIGHTENMENT_ID | ENLIGHTENMENT_PLUS_ID => {
            follow_ups.extend(card_effects::enlightenment_cost_actions(
                state,
                card_id,
                definition.id == ENLIGHTENMENT_PLUS_ID,
            ));
        }
        SECRET_TECHNIQUE_ID | SECRET_TECHNIQUE_PLUS_ID => {
            if draw_pile_has_card_type(state, CardType::Skill) {
                follow_ups.push(InternalAction::AwaitDrawSelect {
                    source_card_id: card_id,
                    purpose: crate::combat::DrawSelectPurpose::SecretTechniqueSkillToHand,
                });
            }
        }
        SECRET_WEAPON_ID | SECRET_WEAPON_PLUS_ID => {
            if draw_pile_has_card_type(state, CardType::Attack) {
                follow_ups.push(InternalAction::AwaitDrawSelect {
                    source_card_id: card_id,
                    purpose: crate::combat::DrawSelectPurpose::SecretWeaponAttackToHand,
                });
            }
        }
        OFFERING_ID => {
            follow_ups.push(InternalAction::LoseHp {
                amount: 6,
                source: HpLossSource::Card(card_id),
            });
            follow_ups.push(InternalAction::GainEnergy { amount: 2 });
            follow_ups.push(InternalAction::DrawCards { count: 3 });
        }
        INFERNAL_BLADE_ID | INFERNAL_BLADE_PLUS_ID => {
            let generated = card_effects::infernal_blade_generated_attack(state);
            follow_ups.push(InternalAction::AddGeneratedCardToPile {
                content_id: generated,
                to: CardPile::Hand,
                temp_cost: Some(0),
                temp_cost_turn_only: true,
            });
        }
        SEEING_RED_ID | SEEING_RED_PLUS_ID => {
            follow_ups.push(InternalAction::GainEnergy { amount: 2 });
        }
        RUPTURE_ID | RUPTURE_PLUS_ID => {
            follow_ups.push(InternalAction::GainRupture {
                amount: if definition.id == RUPTURE_PLUS_ID {
                    2
                } else {
                    1
                },
            });
        }
        COMBUST_ID | COMBUST_PLUS_ID => {
            follow_ups.push(InternalAction::GainCombust {
                amount: definition.values.damage.unwrap_or(0),
            });
        }
        PANACEA_ID | PANACEA_PLUS_ID => {
            let amount = if definition.id == PANACEA_PLUS_ID {
                2
            } else {
                1
            };
            follow_ups.push(InternalAction::GainArtifact { amount });
        }
        PANACHE_ID | PANACHE_PLUS_ID => {
            follow_ups.push(InternalAction::GainPanache {
                amount: definition.values.damage.unwrap_or(0),
            });
        }
        SADISTIC_NATURE_ID | SADISTIC_NATURE_PLUS_ID => {
            follow_ups.push(InternalAction::GainSadisticNature {
                amount: definition.values.damage.unwrap_or(0),
            });
        }
        POWER_THROUGH_ID | POWER_THROUGH_PLUS_ID => {
            follow_ups.push(InternalAction::AddGeneratedCardToPile {
                content_id: WOUND_ID,
                to: CardPile::Hand,
                temp_cost: None,
                temp_cost_turn_only: false,
            });
            follow_ups.push(InternalAction::AddGeneratedCardToPile {
                content_id: WOUND_ID,
                to: CardPile::Hand,
                temp_cost: None,
                temp_cost_turn_only: false,
            });
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
        }
        TRUE_GRIT_ID | TRUE_GRIT_PLUS_ID => {
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
            if !state.piles.hand.is_empty() {
                if definition.id == TRUE_GRIT_PLUS_ID {
                    follow_ups.push(InternalAction::AwaitExhaustSelect {
                        source_card_id: card_id,
                        purpose: crate::combat::ExhaustSelectPurpose::TrueGritExhaustOne,
                    });
                } else {
                    follow_ups.push(InternalAction::ExhaustRandomHandCardExcept {
                        excluded_card_id: card_id,
                    });
                }
            }
        }
        _ if definition.values.block.is_some() => {
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
        }
        _ => {}
    }

    if (exhaust_played_card || definition.keywords.exhaust)
        && definition.card_type != CardType::Power
    {
        state.piles.exhaust_pile.push(card);
        follow_ups.push(InternalAction::CardExhausted { card_id });
    } else if !card.combat_only && definition.card_type != CardType::Power {
        state.piles.discard_pile.push(card);
    }

    follow_ups.extend(crate::relic::apply_on_card_play_relics(
        state,
        definition.card_type,
    ));
    follow_ups.extend(apply_on_card_play_powers(state, definition.card_type));

    Ok(follow_ups)
}

pub fn choose_hand_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    let hand_index = hand_select_ui_to_hand_index(state, ui_index)?;
    let hand_select = state
        .hand_select
        .as_mut()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    if hand_select.purpose == HandSelectPurpose::ForethoughtPutAnyOnDraw {
        if let Some(position) = hand_select
            .selected_hand_indices
            .iter()
            .position(|index| *index == hand_index)
        {
            hand_select.selected_hand_indices.remove(position);
        } else {
            hand_select.selected_hand_indices.push(hand_index);
        }
    } else {
        hand_select.selected_hand_index = Some(hand_index);
    }
    Ok(())
}

pub fn hand_select_ui_to_hand_index(state: &CombatState, ui_index: usize) -> SimResult<usize> {
    let hand_select = state
        .hand_select
        .as_ref()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    let selectable: Vec<usize> = state
        .piles
        .hand
        .iter()
        .enumerate()
        .filter(|(_, card)| hand_select_allows_card(hand_select, card))
        .map(|(index, _)| index)
        .collect();
    selectable
        .get(ui_index)
        .copied()
        .ok_or(SimError::IllegalAction("hand select index out of range"))
}

fn hand_select_allows_card(
    hand_select: &crate::combat::HandSelectState,
    card: &CardInstance,
) -> bool {
    if card.id == hand_select.source_card_id {
        return false;
    }

    match hand_select.purpose {
        HandSelectPurpose::WarcryPutOnDraw | HandSelectPurpose::ThinkingAheadPutOnDraw => true,
        HandSelectPurpose::ArmamentsUpgrade => upgrade_card_instance(*card).is_some(),
        HandSelectPurpose::ForethoughtPutOnDraw | HandSelectPurpose::ForethoughtPutAnyOnDraw => {
            true
        }
        HandSelectPurpose::DualWieldCopy => dual_wield_select_allows_card(card),
    }
}

pub fn confirm_hand_select(state: &mut CombatState) -> SimResult<()> {
    let hand_select = state
        .hand_select
        .take()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    match hand_select.purpose {
        HandSelectPurpose::WarcryPutOnDraw => confirm_warcry_select(
            state,
            hand_select.source_card_id,
            required_hand_select_index(&hand_select)?,
        ),
        HandSelectPurpose::ThinkingAheadPutOnDraw => confirm_thinking_ahead_select(
            state,
            hand_select.source_card_id,
            required_hand_select_index(&hand_select)?,
        ),
        HandSelectPurpose::ArmamentsUpgrade => confirm_armaments_select(
            state,
            hand_select.source_card_id,
            required_hand_select_index(&hand_select)?,
        ),
        HandSelectPurpose::ForethoughtPutOnDraw => confirm_forethought_select(
            state,
            hand_select.source_card_id,
            required_hand_select_index(&hand_select)?,
        ),
        HandSelectPurpose::ForethoughtPutAnyOnDraw => confirm_forethought_multi_select(
            state,
            hand_select.source_card_id,
            hand_select.selected_hand_indices,
        ),
        HandSelectPurpose::DualWieldCopy => confirm_dual_wield_select(
            state,
            hand_select.source_card_id,
            required_hand_select_index(&hand_select)?,
        ),
    }?;
    resume_actions_after_hand_select(state)
}

fn resume_actions_after_hand_select(state: &mut CombatState) -> SimResult<()> {
    if state.pending_after_hand_select_actions.is_empty() {
        return Ok(());
    }
    let queue = std::mem::take(&mut state.pending_after_hand_select_actions);
    let transition = process_internal_queue(state, queue)?;
    *state = transition.state;
    Ok(())
}

fn required_hand_select_index(hand_select: &crate::combat::HandSelectState) -> SimResult<usize> {
    hand_select
        .selected_hand_index
        .ok_or(SimError::IllegalAction("hand select choice is required"))
}

pub fn choose_draw_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    let draw_index = draw_select_ui_to_draw_index(state, ui_index)?;
    let draw_select = state
        .draw_select
        .as_mut()
        .ok_or(SimError::IllegalAction("no draw select is open"))?;
    draw_select.selected_draw_index = Some(draw_index);
    Ok(())
}

pub fn draw_select_ui_to_draw_index(state: &CombatState, ui_index: usize) -> SimResult<usize> {
    let draw_select = state
        .draw_select
        .as_ref()
        .ok_or(SimError::IllegalAction("no draw select is open"))?;
    let selectable: Vec<usize> = state
        .piles
        .draw_pile
        .iter()
        .enumerate()
        .filter(|(_, card)| draw_select_allows_card(draw_select, card))
        .map(|(index, _)| index)
        .collect();
    selectable
        .get(ui_index)
        .copied()
        .ok_or(SimError::IllegalAction("draw select index out of range"))
}

fn draw_select_allows_card(
    draw_select: &crate::combat::DrawSelectState,
    card: &CardInstance,
) -> bool {
    match draw_select.purpose {
        DrawSelectPurpose::SecretTechniqueSkillToHand => get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == CardType::Skill),
        DrawSelectPurpose::SecretWeaponAttackToHand => get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == CardType::Attack),
    }
}

pub fn confirm_draw_select(state: &mut CombatState) -> SimResult<()> {
    let draw_select = state
        .draw_select
        .take()
        .ok_or(SimError::IllegalAction("no draw select is open"))?;
    let index = draw_select
        .selected_draw_index
        .ok_or(SimError::IllegalAction("draw select choice is required"))?;
    match draw_select.purpose {
        DrawSelectPurpose::SecretTechniqueSkillToHand => {
            confirm_secret_technique_select(state, draw_select.source_card_id, index)
        }
        DrawSelectPurpose::SecretWeaponAttackToHand => {
            confirm_secret_weapon_select(state, draw_select.source_card_id, index)
        }
    }
}

fn confirm_secret_technique_select(
    state: &mut CombatState,
    source_card_id: CardId,
    index: usize,
) -> SimResult<()> {
    let source_definition = draw_select_source_definition(state, source_card_id)?;
    let card = state
        .piles
        .draw_pile
        .get(index)
        .copied()
        .ok_or(SimError::IllegalAction("draw select index out of range"))?;
    if !get_card_definition(card.content_id)
        .is_some_and(|definition| definition.card_type == CardType::Skill)
    {
        return Err(SimError::IllegalAction("Secret Technique requires a Skill"));
    }
    move_selected_draw_card_to_hand_or_discard(state, index);
    move_draw_select_source_card(state, source_card_id, source_definition)?;
    Ok(())
}

fn confirm_secret_weapon_select(
    state: &mut CombatState,
    source_card_id: CardId,
    index: usize,
) -> SimResult<()> {
    let source_definition = draw_select_source_definition(state, source_card_id)?;
    let card = state
        .piles
        .draw_pile
        .get(index)
        .copied()
        .ok_or(SimError::IllegalAction("draw select index out of range"))?;
    if !get_card_definition(card.content_id)
        .is_some_and(|definition| definition.card_type == CardType::Attack)
    {
        return Err(SimError::IllegalAction("Secret Weapon requires an Attack"));
    }
    move_selected_draw_card_to_hand_or_discard(state, index);
    move_draw_select_source_card(state, source_card_id, source_definition)?;
    Ok(())
}

fn draw_select_source_definition(
    state: &CombatState,
    source_card_id: CardId,
) -> SimResult<&'static crate::card::CardDefinition> {
    state
        .piles
        .hand
        .iter()
        .chain(state.piles.exhaust_pile.iter())
        .chain(state.piles.discard_pile.iter())
        .find(|card| card.id == source_card_id)
        .and_then(|card| get_card_definition(card.content_id))
        .ok_or(SimError::IllegalAction("draw select source card missing"))
}

fn move_draw_select_source_card(
    state: &mut CombatState,
    source_card_id: CardId,
    _source_definition: &'static crate::card::CardDefinition,
) -> SimResult<()> {
    if state
        .piles
        .hand
        .iter()
        .any(|card| card.id == source_card_id)
    {
        move_delayed_played_source_with_strange_spoon(state, source_card_id)?;
    }
    Ok(())
}

fn move_selected_draw_card_to_hand_or_discard(state: &mut CombatState, index: usize) {
    let card = state.piles.draw_pile.remove(index);
    if state.piles.hand.len() >= 10 {
        state.piles.discard_pile.push(card);
    } else {
        state.piles.hand.push(card);
    }
}

fn confirm_warcry_select(
    state: &mut CombatState,
    source_card_id: CardId,
    index: usize,
) -> SimResult<()> {
    let put_back = state.piles.hand[index].id;
    let card = remove_card_from_pile(state, put_back, CardPile::Hand)?;
    state.piles.draw_pile.push(card);
    finish_warcry_source(state, source_card_id)
}

fn finish_warcry_source(state: &mut CombatState, source_card_id: CardId) -> SimResult<()> {
    let source = remove_card_from_pile(state, source_card_id, CardPile::Hand)?;
    let definition = get_card_definition(source.content_id)
        .ok_or(SimError::UnknownContent(source.content_id))?;
    if definition.keywords.exhaust {
        state.piles.exhaust_pile.push(source);
        apply_on_exhaust_effects(state, source_card_id);
    } else {
        state.piles.discard_pile.push(source);
    }
    Ok(())
}

fn confirm_thinking_ahead_select(
    state: &mut CombatState,
    source_card_id: CardId,
    index: usize,
) -> SimResult<()> {
    let put_back = state.piles.hand[index].id;
    let card = remove_card_from_pile(state, put_back, CardPile::Hand)?;
    state.piles.draw_pile.push(card);
    move_delayed_played_source_with_strange_spoon(state, source_card_id)
}

fn move_delayed_played_source_with_strange_spoon(
    state: &mut CombatState,
    source_card_id: CardId,
) -> SimResult<()> {
    let source = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == source_card_id)
        .copied()
        .ok_or(SimError::IllegalAction(
            "delayed source card is not in hand",
        ))?;
    let definition = get_card_definition(source.content_id)
        .ok_or(SimError::UnknownContent(source.content_id))?;
    let destination = delayed_source_card_destination(state, definition);
    move_card(state, source_card_id, CardPile::Hand, destination)?;
    if destination == CardPile::ExhaustPile {
        apply_on_exhaust_effects(state, source_card_id);
    }
    Ok(())
}

pub fn close_discovery_card_reward_source(state: &mut CombatState) -> SimResult<()> {
    let Some(source) = state.discovery_source_card.take() else {
        return Ok(());
    };
    let source_card_id = source.id;
    let definition = get_card_definition(source.content_id)
        .ok_or(SimError::UnknownContent(source.content_id))?;
    let destination = delayed_source_card_destination(state, definition);
    match destination {
        CardPile::ExhaustPile => {
            state.piles.exhaust_pile.push(source);
            apply_on_exhaust_effects(state, source_card_id);
        }
        CardPile::DiscardPile => state.piles.discard_pile.push(source),
        CardPile::Hand => state.piles.hand.push(source),
        CardPile::DrawPile => state.piles.draw_pile.push(source),
    }
    Ok(())
}

fn delayed_source_card_destination(
    state: &mut CombatState,
    definition: &crate::card::CardDefinition,
) -> CardPile {
    if definition.keywords.exhaust
        || (definition.card_type == CardType::Skill && state.player.powers.corruption > 0)
    {
        delayed_source_exhaust_destination(state)
    } else {
        CardPile::DiscardPile
    }
}

fn delayed_source_exhaust_destination(state: &mut CombatState) -> CardPile {
    if !state.relics.contains(&Relic::StrangeSpoon) {
        return CardPile::ExhaustPile;
    }
    let Some(rng) = state.card_random_rng.as_mut() else {
        return CardPile::ExhaustPile;
    };
    if rng.random_bool() {
        CardPile::DiscardPile
    } else {
        CardPile::ExhaustPile
    }
}

fn confirm_armaments_select(
    state: &mut CombatState,
    source_card_id: CardId,
    index: usize,
) -> SimResult<()> {
    let selected = *state
        .piles
        .hand
        .get(index)
        .ok_or(SimError::IllegalAction("hand select index out of range"))?;
    if selected.id == source_card_id {
        return Err(SimError::IllegalAction("cannot upgrade Armaments"));
    }
    let source_definition = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == source_card_id)
        .and_then(|card| get_card_definition(card.content_id))
        .ok_or(SimError::IllegalAction("Armaments source card missing"))?;
    let upgraded = upgrade_card_instance(selected)
        .ok_or(SimError::IllegalAction("selected card cannot be upgraded"))?;
    let upgradeable_count = state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != source_card_id && card_instance_is_upgradeable(card))
        .count();
    let cannot_upgrade = if upgradeable_count > 1 {
        let card_ids: Vec<CardId> = state
            .piles
            .hand
            .iter()
            .filter(|card| card.id != source_card_id && !card_instance_is_upgradeable(card))
            .map(|card| card.id)
            .collect();
        card_ids
            .into_iter()
            .map(|card_id| remove_card_from_pile(state, card_id, CardPile::Hand))
            .collect::<SimResult<Vec<_>>>()?
    } else {
        Vec::new()
    };
    let selected_card_id = selected.id;
    let _removed = remove_card_from_pile(state, selected_card_id, CardPile::Hand)?;
    let card = upgraded;
    let source_destination = delayed_source_card_destination(state, source_definition);
    move_card(state, source_card_id, CardPile::Hand, source_destination)?;
    if source_destination == CardPile::ExhaustPile {
        apply_on_exhaust_effects(state, source_card_id);
    }
    state.piles.hand.push(card);
    state.piles.hand.extend(cannot_upgrade);
    Ok(())
}

fn confirm_forethought_select(
    state: &mut CombatState,
    source_card_id: CardId,
    index: usize,
) -> SimResult<()> {
    let card_id = state
        .piles
        .hand
        .get(index)
        .ok_or(SimError::IllegalAction("hand select index out of range"))?
        .id;
    if card_id == source_card_id {
        return Err(SimError::IllegalAction("cannot choose Forethought"));
    }
    move_forethought_card_to_draw_bottom(state, source_card_id, card_id)
}

fn confirm_forethought_multi_select(
    state: &mut CombatState,
    source_card_id: CardId,
    indices: Vec<usize>,
) -> SimResult<()> {
    let mut card_ids = Vec::with_capacity(indices.len());
    for index in indices {
        let card_id = state
            .piles
            .hand
            .get(index)
            .ok_or(SimError::IllegalAction("hand select index out of range"))?
            .id;
        if card_id == source_card_id {
            return Err(SimError::IllegalAction("cannot choose Forethought"));
        }
        card_ids.push(card_id);
    }

    let source_definition = forethought_source_definition(state, source_card_id)?;
    for card_id in card_ids {
        move_forethought_selected_card_to_draw_bottom(state, card_id)?;
    }
    move_forethought_source_card(state, source_card_id, source_definition)
}

fn move_forethought_card_to_draw_bottom(
    state: &mut CombatState,
    source_card_id: CardId,
    card_id: CardId,
) -> SimResult<()> {
    let source_definition = forethought_source_definition(state, source_card_id)?;
    move_forethought_selected_card_to_draw_bottom(state, card_id)?;
    move_forethought_source_card(state, source_card_id, source_definition)
}

fn forethought_source_definition(
    state: &CombatState,
    source_card_id: CardId,
) -> SimResult<&'static crate::card::CardDefinition> {
    state
        .piles
        .hand
        .iter()
        .find(|card| card.id == source_card_id)
        .and_then(|card| get_card_definition(card.content_id))
        .ok_or(SimError::IllegalAction("Forethought source card missing"))
}

fn move_forethought_selected_card_to_draw_bottom(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<()> {
    let mut card = remove_card_from_pile(state, card_id, CardPile::Hand)?;
    card.temp_cost = Some(0);
    state.piles.draw_pile.insert(0, card);
    Ok(())
}

fn move_forethought_source_card(
    state: &mut CombatState,
    source_card_id: CardId,
    source_definition: &'static crate::card::CardDefinition,
) -> SimResult<()> {
    let source_destination = delayed_source_card_destination(state, source_definition);
    move_card(state, source_card_id, CardPile::Hand, source_destination)?;
    if source_destination == CardPile::ExhaustPile {
        apply_on_exhaust_effects(state, source_card_id);
    }
    Ok(())
}

fn confirm_dual_wield_select(
    state: &mut CombatState,
    source_card_id: CardId,
    index: usize,
) -> SimResult<()> {
    if index >= state.piles.hand.len() {
        return Err(SimError::IllegalAction("hand select index out of range"));
    }
    let source_card = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == source_card_id)
        .cloned()
        .ok_or(SimError::IllegalAction(
            "Dual Wield source card is not in hand",
        ))?;
    let source_definition = get_card_definition(source_card.content_id)
        .ok_or(SimError::UnknownContent(source_card.content_id))?;
    let copy_count = if source_definition.id == DUAL_WIELD_PLUS_ID {
        2
    } else {
        1
    };
    let mut next_id = state.piles.max_card_instance_id() + 1;
    let mut selected = None;
    let mut unselected_selectable = Vec::new();
    let mut nonselectable = Vec::new();
    for (hand_index, card) in std::mem::take(&mut state.piles.hand)
        .into_iter()
        .enumerate()
    {
        if card.id == source_card_id {
            continue;
        }
        if hand_index == index {
            selected = Some(card);
        } else if dual_wield_select_allows_card(&card) {
            unselected_selectable.push(card);
        } else {
            nonselectable.push(card);
        }
    }
    let selected = selected.ok_or(SimError::IllegalAction("hand select index out of range"))?;
    state.piles.hand = unselected_selectable;
    state.piles.hand.extend(nonselectable);
    state.piles.hand.push(selected);
    for _ in 0..copy_count {
        let mut copy = selected;
        copy.id = CardId::new(next_id);
        copy.combat_only = true;
        state.piles.hand.push(copy);
        next_id += 1;
    }
    state.piles.hand.push(source_card);
    let source_destination = delayed_source_card_destination(state, source_definition);
    move_card(state, source_card_id, CardPile::Hand, source_destination)?;
    if source_destination == CardPile::ExhaustPile {
        apply_on_exhaust_effects(state, source_card_id);
    }
    Ok(())
}

fn dual_wield_select_allows_card(card: &CardInstance) -> bool {
    get_card_definition(card.content_id).is_some_and(|definition| {
        matches!(definition.card_type, CardType::Attack | CardType::Power)
    })
}

pub fn open_discard_select(state: &mut CombatState) -> SimResult<()> {
    open_discard_select_with_max_choices(state, 1)
}

pub fn open_discard_select_with_max_choices(
    state: &mut CombatState,
    max_choices: usize,
) -> SimResult<()> {
    if state.piles.discard_pile.is_empty() {
        return Err(SimError::IllegalAction("discard pile is empty"));
    }
    state.discard_select = Some(crate::combat::DiscardSelectState {
        purpose: DiscardSelectPurpose::LiquidMemoriesReturnToHand,
        source_card_id: None,
        source_card: None,
        selected_discard_indices: Vec::new(),
        max_choices,
        selected_discard_index: None,
    });
    Ok(())
}

pub fn choose_discard_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    discard_select_ui_to_discard_index(state, ui_index)?;
    let discard_select = state
        .discard_select
        .as_mut()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    if discard_select.purpose == DiscardSelectPurpose::LiquidMemoriesReturnToHand {
        if let Some(position) = discard_select
            .selected_discard_indices
            .iter()
            .position(|index| *index == ui_index)
        {
            discard_select.selected_discard_indices.remove(position);
        } else {
            if discard_select.selected_discard_indices.len() >= discard_select.max_choices {
                return Err(SimError::IllegalAction("too many discard select choices"));
            }
            discard_select.selected_discard_indices.push(ui_index);
        }
        discard_select.selected_discard_index =
            discard_select.selected_discard_indices.first().copied();
    } else {
        discard_select.selected_discard_index = Some(ui_index);
    }
    Ok(())
}

pub fn discard_select_ui_to_discard_index(
    state: &CombatState,
    ui_index: usize,
) -> SimResult<usize> {
    state
        .discard_select
        .as_ref()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    if ui_index >= state.piles.discard_pile.len() {
        return Err(SimError::IllegalAction("discard select index out of range"));
    }
    Ok(ui_index)
}

pub fn confirm_liquid_memories_select(state: &mut CombatState) -> SimResult<()> {
    let discard_select = state
        .discard_select
        .take()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    if discard_select.purpose != DiscardSelectPurpose::LiquidMemoriesReturnToHand {
        return Err(SimError::IllegalAction("discard select purpose mismatch"));
    }
    let mut selected = discard_select.selected_discard_indices;
    if selected.is_empty() {
        selected.extend(discard_select.selected_discard_index);
    }
    if selected.is_empty() {
        return Err(SimError::IllegalAction("discard select choice is required"));
    }
    if selected.len() > discard_select.max_choices {
        return Err(SimError::IllegalAction("too many discard select choices"));
    }
    selected.sort_unstable();
    selected.dedup();
    for index in &selected {
        if *index >= state.piles.discard_pile.len() {
            return Err(SimError::IllegalAction("discard select index out of range"));
        }
    }
    let mut cards = Vec::new();
    for index in selected.into_iter().rev() {
        let mut card = state.piles.discard_pile.remove(index);
        card.temp_cost = Some(0);
        card.temp_cost_turn_only = true;
        cards.push(card);
    }
    cards.reverse();
    state.piles.hand.extend(cards);
    Ok(())
}

pub fn confirm_discard_select(state: &mut CombatState) -> SimResult<()> {
    let purpose = state
        .discard_select
        .as_ref()
        .ok_or(SimError::IllegalAction("no discard select is open"))?
        .purpose;
    match purpose {
        DiscardSelectPurpose::LiquidMemoriesReturnToHand => confirm_liquid_memories_select(state),
        DiscardSelectPurpose::HeadbuttPutOnDraw => confirm_headbutt_select(state),
    }
}

pub fn confirm_headbutt_select(state: &mut CombatState) -> SimResult<()> {
    let discard_select = state
        .discard_select
        .take()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    if discard_select.purpose != DiscardSelectPurpose::HeadbuttPutOnDraw {
        return Err(SimError::IllegalAction("discard select purpose mismatch"));
    }
    let index = discard_select
        .selected_discard_index
        .ok_or(SimError::IllegalAction("discard select choice is required"))?;
    let card = state
        .piles
        .discard_pile
        .get(index)
        .copied()
        .ok_or(SimError::IllegalAction("discard select index out of range"))?;
    state.piles.discard_pile.remove(index);
    state.piles.draw_pile.push(card);
    if let Some(source_card) = discard_select.source_card {
        state.piles.discard_pile.push(source_card);
    } else if let Some(source_card_id) = discard_select.source_card_id {
        move_card(state, source_card_id, CardPile::Hand, CardPile::DiscardPile)?;
    }
    flush_pending_monster_death_relics_if_ready(state);
    Ok(())
}

pub fn open_exhaust_select(state: &mut CombatState) -> SimResult<()> {
    state.exhaust_select = Some(crate::combat::ExhaustSelectState {
        purpose: crate::combat::ExhaustSelectPurpose::Exhaust,
        source_card_id: None,
        source_card: None,
        selected_hand_indices: Vec::new(),
    });
    Ok(())
}

pub fn open_gambling_chip_select(state: &mut CombatState) -> SimResult<()> {
    if state.piles.hand.is_empty() {
        return Ok(());
    }
    state.exhaust_select = Some(crate::combat::ExhaustSelectState {
        purpose: crate::combat::ExhaustSelectPurpose::GamblingChip,
        source_card_id: None,
        source_card: None,
        selected_hand_indices: Vec::new(),
    });
    Ok(())
}

pub fn choose_exhaust_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    let pile_index = exhaust_select_ui_to_hand_index(state, ui_index)?;
    let purity_cap = state
        .exhaust_select
        .as_ref()
        .filter(|select| select.purpose == crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3)
        .map(|select| {
            let source_card_id = select
                .source_card_id
                .ok_or(SimError::IllegalAction("exhaust select source is required"))?;
            purity_select_cap(state, source_card_id)
        })
        .transpose()?;
    let exhaust_select = state
        .exhaust_select
        .as_mut()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::ExhumeReturnToHand {
        exhaust_select.selected_hand_indices.clear();
        exhaust_select.selected_hand_indices.push(pile_index);
        return Ok(());
    }
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::TrueGritExhaustOne {
        exhaust_select.selected_hand_indices.clear();
        exhaust_select.selected_hand_indices.push(pile_index);
        return Ok(());
    }
    if let Some(position) = exhaust_select
        .selected_hand_indices
        .iter()
        .position(|index| *index == pile_index)
    {
        exhaust_select.selected_hand_indices.remove(position);
    } else {
        if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3
            && exhaust_select.selected_hand_indices.len() >= purity_cap.unwrap_or(3)
        {
            return Err(purity_too_many_cards_error(purity_cap.unwrap_or(3)));
        }
        exhaust_select.selected_hand_indices.push(pile_index);
    }
    Ok(())
}

pub fn exhaust_select_ui_to_hand_index(state: &CombatState, ui_index: usize) -> SimResult<usize> {
    let exhaust_select = state
        .exhaust_select
        .as_ref()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::ExhumeReturnToHand {
        return exhumable_ui_to_exhaust_index(state, ui_index);
    }
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3 {
        let source_card_id = exhaust_select
            .source_card_id
            .ok_or(SimError::IllegalAction("exhaust select source is required"))?;
        return exhaust_select_visible_hand_indices(state)
            .into_iter()
            .filter(|index| state.piles.hand[*index].id != source_card_id)
            .nth(ui_index)
            .ok_or(SimError::IllegalAction("exhaust select index out of range"));
    }
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::TrueGritExhaustOne {
        let source_card_id = exhaust_select.source_card_id;
        return state
            .piles
            .hand
            .iter()
            .enumerate()
            .filter(|(_, card)| Some(card.id) != source_card_id)
            .map(|(index, _)| index)
            .nth(ui_index)
            .ok_or(SimError::IllegalAction("exhaust select index out of range"));
    }
    exhaust_select_visible_hand_indices(state)
        .into_iter()
        .nth(ui_index)
        .ok_or(SimError::IllegalAction("exhaust select index out of range"))
}

fn exhaust_select_visible_hand_indices(state: &CombatState) -> Vec<usize> {
    let Some(exhaust_select) = state.exhaust_select.as_ref() else {
        return Vec::new();
    };
    state
        .piles
        .hand
        .iter()
        .enumerate()
        .filter(|(index, _)| !exhaust_select.selected_hand_indices.contains(index))
        .map(|(index, _)| index)
        .collect()
}

pub fn confirm_exhaust_select(state: &mut CombatState) -> SimResult<()> {
    let exhaust_select = state
        .exhaust_select
        .take()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::GamblingChip {
        return confirm_gambling_chip_select(state, exhaust_select.selected_hand_indices);
    }
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::ExhumeReturnToHand {
        return confirm_exhume_select(state, exhaust_select);
    }
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3 {
        return confirm_purity_select(state, exhaust_select);
    }
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::BurningPactDraw2 {
        return confirm_burning_pact_select(state, exhaust_select, 2);
    }
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::BurningPactDraw3 {
        return confirm_burning_pact_select(state, exhaust_select, 3);
    }
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::TrueGritExhaustOne {
        return confirm_true_grit_select(state, exhaust_select);
    }
    let selected = unique_selected_indices_in_choice_order(exhaust_select.selected_hand_indices);
    let mut removal_order = selected.clone();
    removal_order.sort_unstable();
    for index in &removal_order {
        if *index >= state.piles.hand.len() {
            return Err(SimError::IllegalAction("exhaust select index out of range"));
        }
    }
    let exhausted = selected
        .iter()
        .map(|index| state.piles.hand[*index])
        .collect::<Vec<_>>();
    for index in removal_order.into_iter().rev() {
        state.piles.hand.remove(index);
    }
    for card in exhausted {
        let card_id = card.id;
        state.piles.exhaust_pile.push(card);
        apply_on_exhaust_effects(state, card_id);
    }
    Ok(())
}

fn confirm_true_grit_select(
    state: &mut CombatState,
    exhaust_select: crate::combat::ExhaustSelectState,
) -> SimResult<()> {
    let source_card_id = exhaust_select.source_card_id;
    let selected = unique_selected_indices_in_choice_order(exhaust_select.selected_hand_indices);
    let target_index = selected.first().copied().ok_or(SimError::IllegalAction(
        "True Grit requires a selected card",
    ))?;
    if target_index >= state.piles.hand.len() {
        return Err(SimError::IllegalAction("exhaust select index out of range"));
    }
    let target_card_id = state.piles.hand[target_index].id;
    if Some(target_card_id) == source_card_id {
        return Err(SimError::IllegalAction("True Grit cannot exhaust itself"));
    }

    let target_position = state
        .piles
        .hand
        .iter()
        .position(|card| card.id == target_card_id)
        .ok_or(SimError::UnknownCard(target_card_id))?;
    let target_card = state.piles.hand.remove(target_position);
    state.piles.exhaust_pile.push(target_card);
    apply_on_exhaust_effects(state, target_card_id);

    if let Some(source_card_id) = source_card_id {
        if let Some(source_position) = state
            .piles
            .hand
            .iter()
            .position(|card| card.id == source_card_id)
        {
            let source_card = state.piles.hand.remove(source_position);
            state.piles.discard_pile.push(source_card);
        } else if !state
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == source_card_id)
        {
            return Err(SimError::UnknownCard(source_card_id));
        }
    }
    Ok(())
}

fn confirm_burning_pact_select(
    state: &mut CombatState,
    exhaust_select: crate::combat::ExhaustSelectState,
    draw_count: usize,
) -> SimResult<()> {
    let source_card_id = exhaust_select
        .source_card_id
        .ok_or(SimError::IllegalAction("exhaust select source is required"))?;
    let mut selected = exhaust_select.selected_hand_indices;
    selected.sort_unstable();
    selected.dedup();
    if selected.len() != 1 {
        return Err(SimError::IllegalAction(
            "Burning Pact requires exactly one card",
        ));
    }
    let index = selected[0];
    let card = state
        .piles
        .hand
        .get(index)
        .copied()
        .ok_or(SimError::IllegalAction("exhaust select index out of range"))?;
    if card.id == source_card_id {
        return Err(SimError::IllegalAction("Burning Pact cannot select itself"));
    }
    state.piles.hand.remove(index);
    state.piles.exhaust_pile.push(card);
    apply_on_exhaust_effects(state, card.id);
    player_draw_cards(state, draw_count);
    if let Some(source_card) = exhaust_select.source_card {
        state.piles.discard_pile.push(source_card);
    } else {
        move_card(state, source_card_id, CardPile::Hand, CardPile::DiscardPile)?;
    }
    Ok(())
}

fn confirm_purity_select(
    state: &mut CombatState,
    exhaust_select: crate::combat::ExhaustSelectState,
) -> SimResult<()> {
    let source_card_id = exhaust_select
        .source_card_id
        .ok_or(SimError::IllegalAction("exhaust select source is required"))?;
    let cap = if let Some(source) = exhaust_select.source_card.as_ref() {
        if source.content_id == PURITY_PLUS_ID {
            5
        } else {
            3
        }
    } else {
        purity_select_cap(state, source_card_id)?
    };
    let selected = unique_selected_indices_in_choice_order(exhaust_select.selected_hand_indices);
    if selected.len() > cap {
        return Err(purity_too_many_cards_error(cap));
    }
    let mut removal_order = selected.clone();
    removal_order.sort_unstable();
    for index in &selected {
        let card = state
            .piles
            .hand
            .get(*index)
            .copied()
            .ok_or(SimError::IllegalAction("exhaust select index out of range"))?;
        if card.id == source_card_id {
            return Err(SimError::IllegalAction("Purity cannot select itself"));
        }
    }
    let exhausted = selected
        .iter()
        .map(|index| state.piles.hand[*index])
        .collect::<Vec<_>>();
    for index in removal_order.into_iter().rev() {
        state.piles.hand.remove(index);
    }
    for card in exhausted {
        state.piles.exhaust_pile.push(card);
        apply_on_exhaust_effects(state, card.id);
    }
    if let Some(source_card) = exhaust_select.source_card {
        let source_destination = purity_source_destination(state);
        let source_card_id = source_card.id;
        push_card_to_pile(state, source_card, source_destination);
        if source_destination == CardPile::ExhaustPile {
            apply_on_exhaust_effects(state, source_card_id);
        }
    } else if state
        .piles
        .hand
        .iter()
        .any(|card| card.id == source_card_id)
    {
        let source_destination = purity_source_destination(state);
        move_card(state, source_card_id, CardPile::Hand, source_destination)?;
        if source_destination == CardPile::ExhaustPile {
            apply_on_exhaust_effects(state, source_card_id);
        }
    }
    Ok(())
}

fn unique_selected_indices_in_choice_order(selected: Vec<usize>) -> Vec<usize> {
    let mut unique = Vec::new();
    for index in selected {
        if !unique.contains(&index) {
            unique.push(index);
        }
    }
    unique
}

fn purity_source_destination(state: &mut CombatState) -> CardPile {
    if !state.relics.contains(&Relic::StrangeSpoon) {
        return CardPile::ExhaustPile;
    }
    let Some(rng) = state.card_random_rng.as_mut() else {
        return CardPile::ExhaustPile;
    };
    if rng.random_bool() {
        CardPile::DiscardPile
    } else {
        CardPile::ExhaustPile
    }
}

fn purity_select_cap(state: &CombatState, source_card_id: CardId) -> SimResult<usize> {
    let source = state
        .piles
        .hand
        .iter()
        .chain(state.piles.exhaust_pile.iter())
        .chain(state.piles.discard_pile.iter())
        .chain(
            state
                .exhaust_select
                .as_ref()
                .and_then(|select| select.source_card.as_ref()),
        )
        .find(|card| card.id == source_card_id)
        .ok_or(SimError::IllegalAction(
            "Purity source card is not available",
        ))?;
    Ok(if source.content_id == PURITY_PLUS_ID {
        5
    } else {
        3
    })
}

fn purity_too_many_cards_error(cap: usize) -> SimError {
    if cap == 5 {
        SimError::IllegalAction("Purity can select at most 5 cards")
    } else {
        SimError::IllegalAction("Purity can select at most 3 cards")
    }
}

fn exhumable_ui_to_exhaust_index(state: &CombatState, ui_index: usize) -> SimResult<usize> {
    state
        .piles
        .exhaust_pile
        .iter()
        .enumerate()
        .filter(|(_, card)| card.content_id != EXHUME_ID && card.content_id != EXHUME_PLUS_ID)
        .map(|(index, _)| index)
        .nth(ui_index)
        .ok_or(SimError::IllegalAction("exhaust select index out of range"))
}

fn confirm_exhume_select(
    state: &mut CombatState,
    exhaust_select: crate::combat::ExhaustSelectState,
) -> SimResult<()> {
    let source_card_id = exhaust_select
        .source_card_id
        .ok_or(SimError::IllegalAction("exhaust select source is required"))?;
    let index = exhaust_select
        .selected_hand_indices
        .first()
        .copied()
        .ok_or(SimError::IllegalAction("exhaust select choice is required"))?;
    let card = state
        .piles
        .exhaust_pile
        .get(index)
        .copied()
        .ok_or(SimError::IllegalAction("exhaust select index out of range"))?;
    if card.content_id == EXHUME_ID || card.content_id == EXHUME_PLUS_ID {
        return Err(SimError::IllegalAction("Exhume cannot return Exhume"));
    }
    state.piles.exhaust_pile.remove(index);
    state.piles.hand.push(card);
    if let Some(source_card) = exhaust_select.source_card {
        state.piles.exhaust_pile.push(source_card);
    } else {
        move_card(state, source_card_id, CardPile::Hand, CardPile::ExhaustPile)?;
    }
    apply_on_exhaust_effects(state, source_card_id);
    Ok(())
}

fn confirm_gambling_chip_select(
    state: &mut CombatState,
    mut selected: Vec<usize>,
) -> SimResult<()> {
    selected.sort_unstable();
    selected.dedup();
    let count = selected.len();
    for index in &selected {
        if *index >= state.piles.hand.len() {
            return Err(SimError::IllegalAction("exhaust select index out of range"));
        }
    }
    let discarded = selected
        .iter()
        .map(|index| state.piles.hand[*index])
        .collect::<Vec<_>>();
    for index in selected.into_iter().rev() {
        state.piles.hand.remove(index);
    }
    state.piles.discard_pile.extend(discarded);
    player_draw_cards(state, count);
    Ok(())
}

fn remove_card_from_pile(
    state: &mut CombatState,
    card_id: CardId,
    pile: CardPile,
) -> SimResult<CardInstance> {
    let cards = match pile {
        CardPile::Hand => &mut state.piles.hand,
        CardPile::DrawPile => &mut state.piles.draw_pile,
        CardPile::DiscardPile => &mut state.piles.discard_pile,
        CardPile::ExhaustPile => &mut state.piles.exhaust_pile,
    };
    let index = cards
        .iter()
        .position(|card| card.id == card_id)
        .ok_or(SimError::UnknownCard(card_id))?;
    Ok(cards.remove(index))
}

fn find_hand_card(state: &CombatState, card_id: CardId) -> SimResult<CardInstance> {
    state
        .piles
        .hand
        .iter()
        .copied()
        .find(|card| card.id == card_id)
        .ok_or(SimError::UnknownCard(card_id))
}

fn card_content_definition(
    state: &CombatState,
    card_id: CardId,
) -> SimResult<&'static crate::card::CardDefinition> {
    state
        .piles
        .hand
        .iter()
        .chain(state.piles.discard_pile.iter())
        .chain(state.piles.draw_pile.iter())
        .chain(state.piles.exhaust_pile.iter())
        .find(|card| card.id == card_id)
        .and_then(|card| get_card_definition(card.content_id))
        .ok_or(SimError::UnknownCard(card_id))
}

fn find_hand_card_mut(state: &mut CombatState, card_id: CardId) -> SimResult<&mut CardInstance> {
    state
        .piles
        .hand
        .iter_mut()
        .find(|card| card.id == card_id)
        .ok_or(SimError::UnknownCard(card_id))
}

fn find_combat_card_mut(state: &mut CombatState, card_id: CardId) -> Option<&mut CardInstance> {
    state
        .piles
        .hand
        .iter_mut()
        .chain(state.piles.discard_pile.iter_mut())
        .chain(state.piles.draw_pile.iter_mut())
        .chain(state.piles.exhaust_pile.iter_mut())
        .find(|card| card.id == card_id)
}

fn add_ritual_dagger_damage_bonus(state: &mut CombatState, card_id: CardId, amount: i32) {
    if let Some(card) = find_combat_card_mut(state, card_id) {
        card.ritual_dagger_damage_bonus += amount.max(0);
    }
}

fn remove_card_from_hand(state: &mut CombatState, card_id: CardId) -> SimResult<CardInstance> {
    let index = state
        .piles
        .hand
        .iter()
        .position(|card| card.id == card_id)
        .ok_or(SimError::UnknownCard(card_id))?;

    Ok(state.piles.hand.remove(index))
}

fn effective_hand_card_cost(state: &CombatState, card_id: CardId) -> i32 {
    let card = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .expect("hand card");
    let base_cost = if let Some(cost) = card.temp_cost {
        i32::from(cost)
    } else {
        get_card_definition(card.content_id)
            .map(|definition| i32::from(definition.cost))
            .unwrap_or(0)
    };
    if get_card_definition(card.content_id).is_some_and(|definition| {
        state.player.powers.corruption > 0 && definition.card_type == CardType::Skill
    }) {
        return 0;
    }
    if card.content_id == BLOOD_FOR_BLOOD_ID || card.content_id == BLOOD_FOR_BLOOD_PLUS_ID {
        return (base_cost - card.blood_for_blood_cost_reduction).max(0);
    }
    base_cost
}

fn move_card(
    state: &mut CombatState,
    card_id: CardId,
    from: CardPile,
    to: CardPile,
) -> SimResult<()> {
    let card = match from {
        CardPile::Hand => remove_card_from_hand(state, card_id)?,
        CardPile::DrawPile | CardPile::DiscardPile | CardPile::ExhaustPile => {
            return Err(SimError::IllegalAction(
                "card move source is not implemented",
            ));
        }
    };

    match to {
        CardPile::DiscardPile => {
            let is_played_power = get_card_definition(card.content_id)
                .is_some_and(|definition| definition.card_type == CardType::Power);
            if !is_played_power {
                state.piles.discard_pile.push(card);
            }
            Ok(())
        }
        CardPile::ExhaustPile => {
            state.piles.exhaust_pile.push(card);
            Ok(())
        }
        CardPile::Hand | CardPile::DrawPile => Err(SimError::IllegalAction(
            "card move destination is not implemented",
        )),
    }
}

fn upgrade_combat_cards(state: &mut CombatState) {
    for card in state
        .piles
        .hand
        .iter_mut()
        .chain(state.piles.draw_pile.iter_mut())
        .chain(state.piles.discard_pile.iter_mut())
        .chain(state.piles.exhaust_pile.iter_mut())
    {
        if let Some(upgraded) = upgrade_card_instance(*card) {
            *card = upgraded;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{
        ANGER_ID, ARMAMENTS_ID, BASH_ID, BLUDGEON_ID, DUAL_WIELD_ID, FEED_ID, FIEND_FIRE_ID,
        FIEND_FIRE_PLUS_ID, HAVOC_ID, HAVOC_PLUS_ID, HEADBUTT_ID, INFERNAL_BLADE_ID, RAMPAGE_ID,
        SHRUG_IT_OFF_ID, SHRUG_IT_OFF_PLUS_ID,
    };
    use crate::content::monsters::{
        monster_state, DARKLING_A0, FUNGI_BEAST_A0, GUARDIAN_A0, JAW_WORM_A0, SNAKE_PLANT_A0,
    };
    use crate::rng::StsRng;

    #[test]
    fn hex_dazed_waits_for_armaments_hand_select_to_close() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.hex = 1;
        state.card_random_rng = Some(StsRng::new(7_141_693_325_691_831_207));
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), ARMAMENTS_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(4), SHRUG_IT_OFF_ID),
            CardInstance::new(CardId::new(5), BASH_ID),
            CardInstance::new(CardId::new(6), RAMPAGE_ID),
        ];

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Armaments should open its hand-select screen");

        assert!(next.hand_select.is_some());
        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .filter(|card| card.content_id == DAZED_ID)
                .count(),
            0
        );
        assert_eq!(next.pending_after_hand_select_actions.len(), 1);

        choose_hand_select(&mut next, 0).expect("Strike is selectable");
        confirm_hand_select(&mut next).expect("Armaments selection should resolve");

        assert!(next.pending_after_hand_select_actions.is_empty());
        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .filter(|card| card.content_id == DAZED_ID)
                .count(),
            1
        );
    }

    #[test]
    fn violence_uses_card_group_add_to_random_spot_bounds() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), BASH_ID),
            CardInstance::new(CardId::new(4), HEADBUTT_ID),
            CardInstance::new(CardId::new(5), RAMPAGE_ID),
            CardInstance::new(CardId::new(6), ANGER_ID),
        ];
        state.card_random_rng = Some(StsRng::new(1_234));
        state.shuffle_rng = Some(StsRng::new(5_678));

        draw_random_attacks_from_draw_pile(&mut state, 3);

        assert_eq!(
            state
                .piles
                .hand
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(5), CardId::new(4), CardId::new(6)]
        );
        assert_eq!(
            state
                .piles
                .draw_pile
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(1), CardId::new(2), CardId::new(3)]
        );
    }

    #[test]
    fn feed_kill_applies_magic_flower_to_the_hp_gain() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 1;
        state.monsters[0].max_hp = 1;
        state.player.hp = 44;
        state.player.max_hp = 109;
        state.player.energy = 3;
        state.relics.push(Relic::MagicFlower);
        state.piles.hand = vec![CardInstance::new(CardId::new(1), FEED_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Feed should kill the target");

        assert_eq!(next.player.max_hp, 112);
        assert_eq!(next.player.hp, 49);
    }

    #[test]
    fn feed_does_not_gain_max_hp_from_a_half_dead_darkling() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&DARKLING_A0, target)];
        state.monsters[0].hp = 1;
        state.monsters[0].max_hp = 1;
        state.player.hp = 81;
        state.player.max_hp = 114;
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), FEED_PLUS_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Feed should put the Darkling into its half-dead state");

        assert_eq!(next.player.max_hp, 114);
        assert_eq!(next.player.hp, 81);
        assert!(!next.monsters[0].alive);
        assert!(next.monsters[0].escaped);
    }

    #[test]
    fn rage_triggered_block_ignores_dexterity_before_guardian_sharp_hide() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&GUARDIAN_A0, target)];
        state.monsters[0].powers.spikes = 3;
        state.monsters[0].powers.strength = -2;
        state.player.block = 7;
        state.player.powers.dexterity = 2;
        state.player.temp_rage_block = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), ANGER_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Anger should resolve through Rage and Sharp Hide");

        assert_eq!(next.player.block, 7);
    }

    #[test]
    fn spore_cloud_releases_only_when_battle_is_not_ending() {
        let fungi_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![
            monster_state(&FUNGI_BEAST_A0, fungi_id),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].alive = false;

        apply_monster_death_hooks(&mut state, fungi_id);

        assert_eq!(state.player.powers.vulnerable, 2);

        let last_fungi_id = MonsterId::new(3);
        let mut ending_state = CombatState::initial_fixture();
        ending_state.monsters = vec![monster_state(&FUNGI_BEAST_A0, last_fungi_id)];
        ending_state.monsters[0].alive = false;

        apply_monster_death_hooks(&mut ending_state, last_fungi_id);

        assert_eq!(ending_state.player.powers.vulnerable, 0);
    }

    #[test]
    fn copied_single_target_card_fizzles_when_original_kills_target() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 1;
        state.monsters[0].max_hp = 1;
        state.player.energy = 3;
        state.double_tap_pending = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), POMMEL_STRIKE_PLUS_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
            CardInstance::new(CardId::new(4), DEFEND_R_ID),
            CardInstance::new(CardId::new(5), STRIKE_R_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Pommel Strike+ should play");

        assert_eq!(next.piles.hand.len(), 2);
        assert_eq!(next.piles.draw_pile.len(), 2);
    }

    #[test]
    fn shrug_it_off_draws_after_freeing_its_slot_from_a_full_hand() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = (1..=9)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .chain(std::iter::once(CardInstance::new(
                CardId::new(10),
                SHRUG_IT_OFF_ID,
            )))
            .collect();
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(11), DEFEND_R_ID)];
        state.piles.discard_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(10),
                target: None,
            },
        )
        .expect("Shrug It Off should play from a full hand");

        assert_eq!(next.piles.hand.len(), 10);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == DEFEND_R_ID));
        assert!(next.piles.draw_pile.is_empty());
    }

    #[test]
    fn shrug_it_off_is_not_shuffled_into_its_own_draw() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), SHRUG_IT_OFF_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(2), DEFEND_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Shrug It Off should draw through a reshuffle");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, DEFEND_R_ID);
        assert_eq!(next.piles.discard_pile.len(), 1);
        assert_eq!(next.piles.discard_pile[0].content_id, SHRUG_IT_OFF_ID);
    }

    #[test]
    fn battle_trance_draws_after_freeing_its_full_hand_slot() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = (1..=9)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .chain(std::iter::once(CardInstance::new(
                CardId::new(10),
                BATTLE_TRANCE_PLUS_ID,
            )))
            .collect();
        state.piles.draw_pile = (11..=14)
            .map(|id| CardInstance::new(CardId::new(id), DEFEND_R_ID))
            .collect();
        state.piles.discard_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(10),
                target: None,
            },
        )
        .expect("Battle Trance+ should play from a full hand");

        assert_eq!(next.piles.hand.len(), 10);
        assert_eq!(next.piles.draw_pile.len(), 3);
        assert_eq!(next.piles.discard_pile.len(), 1);
        assert_eq!(next.piles.discard_pile[0].content_id, BATTLE_TRANCE_PLUS_ID);
    }

    #[test]
    fn copied_attack_resolves_malleable_block_before_second_hit() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&SNAKE_PLANT_A0, target)];
        state.monsters[0].hp = 71;
        state.monsters[0].block = 3;
        state.monsters[0].powers.malleable = 4;
        state.monsters[0].powers.malleable_base = 3;
        state.player.energy = 2;
        state.double_tap_pending = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HEAVY_BLADE_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Heavy Blade should play");

        assert_eq!(next.monsters[0].hp, 50);
        assert_eq!(next.monsters[0].block, 5);
        assert_eq!(next.monsters[0].powers.malleable, 6);
    }

    #[test]
    fn fiend_fire_resolves_all_hits_before_malleable_block() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&SNAKE_PLANT_A0, target)];
        state.monsters[0].hp = 100;
        state.monsters[0].max_hp = 100;
        state.monsters[0].block = 0;
        state.monsters[0].powers.malleable = 3;
        state.monsters[0].powers.malleable_base = 3;
        state.player.energy = 3;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), FIEND_FIRE_PLUS_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
            CardInstance::new(CardId::new(4), BASH_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Fiend Fire+ should play");

        assert_eq!(next.monsters[0].hp, 70);
        assert_eq!(next.monsters[0].block, 12);
        assert_eq!(next.monsters[0].powers.malleable, 6);
    }

    #[test]
    fn top_draw_bash_applies_vulnerable_before_followup_hand_strike() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 40;
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(4), STRIKE_R_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), BASH_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        apply_play_top_draw_card_to_state(&mut state, Some(target)).expect("top Strike");
        apply_play_top_draw_card_to_state(&mut state, Some(target)).expect("top Strike");
        apply_play_top_draw_card_to_state(&mut state, Some(target)).expect("top Bash");

        assert_eq!(state.monsters[0].hp, 20);
        assert_eq!(state.monsters[0].powers.vulnerable, 2);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(4),
                target: Some(target),
            },
        )
        .expect("hand Strike should play into Vulnerable");

        assert_eq!(next.monsters[0].hp, 11);
    }

    #[test]
    fn top_draw_anger_copy_is_not_purged_when_mayhem_plays_it_again() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 100;
        state.piles.hand.clear();
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(1), ANGER_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        apply_play_top_draw_card_to_state(&mut state, Some(target)).expect("play original Anger");
        assert_eq!(state.piles.discard_pile.len(), 2);
        assert!(state
            .piles
            .discard_pile
            .iter()
            .all(|card| !card.combat_only));

        let generated_copy = state
            .piles
            .discard_pile
            .pop()
            .expect("generated Anger copy");
        state.piles.discard_pile.clear();
        state.piles.draw_pile.push(generated_copy);

        apply_play_top_draw_card_to_state(&mut state, Some(target))
            .expect("Mayhem-style play of generated Anger copy");

        assert_eq!(
            state
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![ANGER_ID, ANGER_ID]
        );
    }

    #[test]
    fn havoc_played_feed_deals_damage_before_exhausting_it() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 20;
        state.monsters[0].max_hp = 20;
        state.monsters[0].block = 40;
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), FEED_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ should play top-deck Feed");

        assert_eq!(next.monsters[0].hp, 20);
        assert_eq!(next.monsters[0].block, 30);
        assert_eq!(next.player.energy, 3);
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, FEED_ID);
        assert_eq!(next.piles.discard_pile.len(), 1);
        assert_eq!(next.piles.discard_pile[0].content_id, HAVOC_PLUS_ID);
    }

    #[test]
    fn havoc_played_rampage_deals_damage_and_scales_before_exhausting_it() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), RAMPAGE_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.card_random_rng = None;
        let starting_hp = state.monsters[0].hp;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ should play top-deck Rampage");

        assert_eq!(next.monsters[0].hp, starting_hp - 8);
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, RAMPAGE_ID);
        assert_eq!(next.piles.exhaust_pile[0].rampage_damage_bonus, 5);
    }

    #[test]
    fn havoc_played_shrug_it_off_plus_draws_after_gaining_block() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), SHRUG_IT_OFF_PLUS_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ should play top-deck Shrug It Off+");

        assert_eq!(next.player.block, 11);
        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert!(next.piles.draw_pile.is_empty());
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, SHRUG_IT_OFF_PLUS_ID);
    }

    #[test]
    fn havoc_played_headbutt_opens_discard_choice_without_moving_it_from_exhaust() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 40;
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), HEADBUTT_ID)];
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(3), POWER_THROUGH_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
        ];
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc should play top-deck Headbutt");

        assert_eq!(next.monsters[0].hp, 31);
        assert_eq!(next.player.energy, 2);
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, HEADBUTT_ID);
        assert_eq!(next.discard_select.as_ref().unwrap().source_card_id, None);
        assert_eq!(next.discard_select.as_ref().unwrap().source_card, None);

        choose_discard_select(&mut next, 0).expect("select Power Through");
        confirm_headbutt_select(&mut next).expect("confirm forced Headbutt selection");

        assert_eq!(next.piles.draw_pile.len(), 1);
        assert_eq!(next.piles.draw_pile[0].content_id, POWER_THROUGH_ID);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == HEADBUTT_ID));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == HEADBUTT_ID));
    }

    #[test]
    fn havoc_played_true_grit_exhausts_a_random_hand_card() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_PLUS_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(4), TRUE_GRIT_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.card_random_rng = None;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ should play top-deck True Grit");

        assert_eq!(next.player.block, 7);
        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == TRUE_GRIT_ID));
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == DEFEND_R_ID));
    }

    #[test]
    fn havoc_played_true_grit_plus_selects_without_moving_it_from_exhaust() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_PLUS_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(4), TRUE_GRIT_PLUS_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ should play top-deck True Grit+");

        assert_eq!(next.player.block, 9);
        assert!(next.exhaust_select.is_some());
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == TRUE_GRIT_PLUS_ID));

        choose_exhaust_select(&mut next, 0).expect("select Defend");
        confirm_exhaust_select(&mut next).expect("confirm forced True Grit+ selection");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == DEFEND_R_ID));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == TRUE_GRIT_PLUS_ID));
    }

    #[test]
    fn havoc_played_whirlwind_uses_current_energy_for_x_without_spending_it() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&SNAKE_PLANT_A0, target)];
        state.monsters[0].hp = 50;
        state.monsters[0].block = 0;
        state.monsters[0].powers.malleable = 3;
        state.monsters[0].powers.malleable_base = 3;
        state.player.energy = 4;
        state.player.powers.weak = 2;
        state.relics = vec![Relic::DeadBranch];
        state.card_random_rng = Some(StsRng::with_counter(22_079_335_132, 1));
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), WHIRLWIND_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ should play top-deck Whirlwind");

        assert_eq!(next.player.energy, 4);
        assert_eq!(next.monsters[0].hp, 38);
        assert_eq!(next.monsters[0].block, 18);
        assert_eq!(next.monsters[0].powers.malleable, 7);
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, WHIRLWIND_ID);
        assert_eq!(next.piles.discard_pile.len(), 1);
        assert_eq!(next.piles.discard_pile[0].content_id, HAVOC_PLUS_ID);
        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, DUAL_WIELD_ID);
    }

    #[test]
    fn infernal_blade_exhausts_source_after_adding_generated_attack() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), INFERNAL_BLADE_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.card_random_rng = None;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Infernal Blade should play");

        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, INFERNAL_BLADE_ID);
        assert!(next.piles.discard_pile.is_empty());
        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.player.energy, 2);
    }

    #[test]
    fn infernal_blade_uses_source_groups_in_rarity_order() {
        let pool = card_effects::infernal_blade_modeled_attack_pool();

        assert_eq!(pool[17], SEARING_BLOW_ID);
        assert_eq!(pool[21], PUMMEL_ID);
        assert_eq!(pool[25], BLUDGEON_ID);
        assert_eq!(pool[26], FIEND_FIRE_ID);
        assert_eq!(pool[27], IMMOLATE_ID);
    }

    #[test]
    fn exhaust_multi_select_indexes_skip_hidden_selected_cards() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), WOUND_ID),
            CardInstance::new(CardId::new(3), BLOOD_FOR_BLOOD_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
            CardInstance::new(CardId::new(5), DAZED_ID),
            CardInstance::new(CardId::new(6), DAZED_ID),
            CardInstance::new(CardId::new(7), ANGER_PLUS_ID),
            CardInstance::new(CardId::new(8), WOUND_ID),
        ];

        open_exhaust_select(&mut state).expect("open exhaust select");
        choose_exhaust_select(&mut state, 0).expect("select first visible card");
        choose_exhaust_select(&mut state, 4).expect("select second visible Dazed");
        choose_exhaust_select(&mut state, 3).expect("select remaining visible Dazed");

        assert_eq!(
            state
                .exhaust_select
                .as_ref()
                .expect("exhaust select")
                .selected_hand_indices,
            vec![0, 5, 4]
        );

        confirm_exhaust_select(&mut state).expect("confirm exhaust select");

        let hand_ids = state
            .piles
            .hand
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>();
        assert_eq!(
            hand_ids,
            vec![
                WOUND_ID,
                BLOOD_FOR_BLOOD_ID,
                STRIKE_R_ID,
                ANGER_PLUS_ID,
                WOUND_ID
            ]
        );
    }
}
