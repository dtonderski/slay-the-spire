use super::card_effects;
use crate::{
    action::{CardPile, CombatAction, HpLossSource, InternalAction},
    card::{CardRarity, CardType},
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
        get_card_definition, searing_blow_card_damage, upgrade_card_instance, upgrade_content_id,
        ANGER_ID, ANGER_PLUS_ID, BASH_ID, BLIND_PLUS_ID, BLOOD_FOR_BLOOD_ID,
        BLOOD_FOR_BLOOD_PLUS_ID, BODY_SLAM_ID, BODY_SLAM_PLUS_ID, CARNAGE_ID, CARNAGE_PLUS_ID,
        CHRYSALIS_ID, CLASH_ID, CLASH_PLUS_ID, CLEAVE_ID, CLEAVE_PLUS_ID, CLOTHESLINE_ID,
        CLOTHESLINE_PLUS_ID, DAZED_ID, DEEP_BREATH_ID, DEEP_BREATH_PLUS_ID, DEFEND_R_ID, DISARM_ID,
        DISARM_PLUS_ID, DRAMATIC_ENTRANCE_ID, DROPKICK_ID, DROPKICK_PLUS_ID, ENLIGHTENMENT_ID,
        ENLIGHTENMENT_PLUS_ID, EXHUME_ID, EXHUME_PLUS_ID, FEED_ID, FINESSE_ID, FLASH_OF_STEEL_ID,
        FLASH_OF_STEEL_PLUS_ID, HEAVY_BLADE_ID, HEAVY_BLADE_PLUS_ID, HEMOKINESIS_ID,
        HEMOKINESIS_PLUS_ID, IMPATIENCE_ID, IMPATIENCE_PLUS_ID, INTIMIDATE_ID, INTIMIDATE_PLUS_ID,
        IRON_WAVE_ID, IRON_WAVE_PLUS_ID, MASTER_OF_STRATEGY_ID, MIND_BLAST_ID, OFFERING_ID,
        PAIN_ID, PANACEA_ID, PANIC_BUTTON_ID, PERFECTED_STRIKE_ID, PERFECTED_STRIKE_PLUS_ID,
        POMMEL_STRIKE_ID, POMMEL_STRIKE_PLUS_ID, POWER_THROUGH_ID, POWER_THROUGH_PLUS_ID,
        PUMMEL_ID, PUMMEL_PLUS_ID, PURITY_PLUS_ID, RAGE_ID, RAGE_PLUS_ID, REAPER_ID,
        REAPER_PLUS_ID, RECKLESS_CHARGE_ID, RECKLESS_CHARGE_PLUS_ID, SEARING_BLOW_ID,
        SEARING_BLOW_PLUS_ID, SENTINEL_ID, SENTINEL_PLUS_ID, SEVER_SOUL_ID, SEVER_SOUL_PLUS_ID,
        SHRUG_IT_OFF_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID, SWORD_BOOMERANG_ID,
        SWORD_BOOMERANG_PLUS_ID, THUNDERCLAP_ID, THUNDERCLAP_PLUS_ID, TRIP_PLUS_ID, TWIN_STRIKE_ID,
        TWIN_STRIKE_PLUS_ID, WILD_STRIKE_ID, WILD_STRIKE_PLUS_ID, WOUND_ID,
    },
    content::monsters::{
        apply_collector_death_escape, apply_gremlin_leader_death_escape, check_slime_boss_split,
        get_monster_definition, guardian_on_hp_damage, release_stasis_card_on_death,
        wake_lagavulin_on_damage, GUARDIAN_ID,
    },
    content::reward_pool::IRONCLAD_REWARD_ENTRIES,
    content::shop_pool::colorless_discovery_pool,
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
        let follow_ups = apply_internal_action(&mut next, internal_action)?;
        event_log.push(internal_action);
        for follow_up in follow_ups {
            if matches!(follow_up, InternalAction::CardExhausted { .. }) {
                queue.push_front(follow_up);
            } else {
                queue.push_back(follow_up);
            }
        }
    }

    flush_pending_player_spikes_damage_if_ready(&mut next);

    if next.player.hp <= 0 {
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
            Ok(follow_ups)
        }
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
                guardian_on_hp_damage(monster, damage.hp_damage);
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
                &mut follow_ups,
                info.target,
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
                    guardian_on_hp_damage(monster, damage.hp_damage);
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
                    &mut follow_ups,
                    target,
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
                guardian_on_hp_damage(monster, damage.hp_damage);
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
                &mut follow_ups,
                info.target,
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
                guardian_on_hp_damage(monster, damage.hp_damage);
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
                &mut follow_ups,
                info.target,
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
                    state.player.max_hp += max_hp_gain;
                    state.player.hp += max_hp_gain;
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
            move_card(state, card_id, from, to)?;
            let mut follow_ups = Vec::new();
            if from == CardPile::Hand && state.piles.hand.is_empty() {
                apply_unceasing_top_after_hand_emptied(state);
            }
            if to == CardPile::ExhaustPile {
                follow_ups.push(InternalAction::CardExhausted { card_id });
            }
            Ok(follow_ups)
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
        InternalAction::AddGeneratedCardToDrawPileRandomSpot { content_id } => {
            add_generated_card_to_draw_pile_random_spot(state, content_id);
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
        InternalAction::ShuffleDiscardIntoDraw => {
            player_shuffle_discard_into_draw(state);
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
            state.player.powers.barricade += amount;
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
            state.player.powers.corruption += amount;
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
            state.player.temp_strength += amount;
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
        InternalAction::PlayTopDrawCard {
            target,
            exhaust_played_card,
        } => apply_play_top_draw_card(state, target, exhaust_played_card),
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
            state.hand_select = Some(crate::combat::HandSelectState {
                purpose,
                source_card_id,
                selected_hand_index: None,
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
                if state.monsters.iter().all(|monster| !monster.alive) {
                    move_card(state, source_card_id, CardPile::Hand, CardPile::DiscardPile)?;
                    return Ok(Vec::new());
                }
                if state.piles.discard_pile.len() == 1 {
                    let source_card = remove_card_from_pile(state, source_card_id, CardPile::Hand)?;
                    let selected = state.piles.discard_pile.remove(0);
                    state.piles.draw_pile.push(selected);
                    state.piles.discard_pile.push(source_card);
                    return Ok(Vec::new());
                }
            }
            let source_card = if purpose == DiscardSelectPurpose::HeadbuttPutOnDraw {
                Some(remove_card_from_pile(
                    state,
                    source_card_id,
                    CardPile::Hand,
                )?)
            } else {
                None
            };
            state.discard_select = Some(crate::combat::DiscardSelectState {
                purpose,
                source_card_id: Some(source_card_id),
                source_card,
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
            let cost_for_turn = card.temp_cost.unwrap_or(definition.cost);
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
            guardian_on_hp_damage(monster, damage.hp_damage);
            (
                damage.hp_damage,
                monster.alive,
                relics.contains(&crate::Relic::HandDrill) && damage.broke_block,
                damage.malleable_block,
            )
        };
        push_malleable_block_follow_up(&mut follow_ups, target, still_alive, malleable_block);
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
    follow_ups: &mut Vec<InternalAction>,
    target: MonsterId,
    still_alive: bool,
    malleable_block: Option<i32>,
) {
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
        guardian_on_hp_damage(monster, hp_damage);
        monster.alive
    };
    check_slime_boss_split(state, target);
    if !still_alive {
        apply_monster_death_hooks(state, target);
    }
    Ok(())
}

pub(crate) fn apply_monster_death_hooks(state: &mut CombatState, monster_id: MonsterId) {
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
    crate::relic::apply_monster_death_relics(state);
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
    state: &CombatState,
    gained: i32,
) -> Vec<InternalAction> {
    if gained <= 0 || state.player.powers.juggernaut <= 0 {
        return Vec::new();
    }
    first_living_monster_id(state)
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

fn first_living_monster_id(state: &CombatState) -> Option<MonsterId> {
    state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .min_by_key(|monster| monster.id.get())
        .map(|monster| monster.id)
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

fn dead_branch_card_pool() -> Vec<ContentId> {
    [CardRarity::Common, CardRarity::Uncommon, CardRarity::Rare]
        .into_iter()
        .flat_map(|rarity| {
            IRONCLAD_REWARD_ENTRIES
                .iter()
                .filter(move |entry| entry.rarity == rarity)
                .rev()
                .map(|entry| entry.content_id)
        })
        .filter(|content_id| *content_id != FEED_ID && *content_id != REAPER_ID)
        .collect()
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
                guardian_on_hp_damage(monster, hp_damage);
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
                    rng.random_int(attack_ids.len() as i32) as usize
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

fn add_generated_card_to_draw_pile_random_spot(state: &mut CombatState, content_id: ContentId) {
    let next_id = CardId::new(state.piles.max_card_instance_id() + 1);
    let card = CardInstance {
        combat_only: true,
        ..CardInstance::new(next_id, content_id)
    };
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
        let gained = calculate_block(state.player.temp_rage_block, state.player.powers);
        apply_player_direct_block_gain(state, gained);
    }
}

fn set_random_hand_card_cost_for_combat(state: &mut CombatState, amount: u8) {
    if state.piles.hand.is_empty() {
        return;
    }

    let index = if let Some(rng) = state.card_random_rng.as_mut() {
        rng.random_int((state.piles.hand.len() - 1) as i32) as usize
    } else {
        0
    };

    let card = &mut state.piles.hand[index];
    card.temp_cost = Some(amount);
    card.temp_cost_turn_only = false;
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

fn apply_play_top_draw_card(
    state: &mut CombatState,
    target: Option<MonsterId>,
    exhaust_played_card: bool,
) -> SimResult<Vec<InternalAction>> {
    if state.piles.draw_pile.is_empty() {
        return Err(SimError::IllegalAction("draw pile is empty"));
    }

    let card = state
        .piles
        .draw_pile
        .pop()
        .ok_or(SimError::IllegalAction("draw pile is empty"))?;
    let card_id = card.id;
    let definition =
        get_card_definition(card.content_id).ok_or(SimError::UnknownContent(card.content_id))?;

    card_effects::validate_havoc_target(definition, target)?;
    apply_enrage_on_card_type(state, definition.card_type);
    apply_rage_on_card_type(state, definition.card_type);

    let mut follow_ups = crate::relic::apply_on_card_play_relics(state, definition.card_type);
    follow_ups.extend(apply_on_card_play_powers(state, definition.card_type));
    let current_pile_count_with_top_card = card_effects::current_combat_pile_card_count(state) + 1;

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
            if definition.id == HEMOKINESIS_ID || definition.id == HEMOKINESIS_PLUS_ID {
                follow_ups.push(InternalAction::LoseHp {
                    amount: 2,
                    source: HpLossSource::Card(card_id),
                });
            }
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: searing_blow_card_damage(&card)
                        .unwrap_or_else(|| definition.values.damage.unwrap_or(0)),
                },
            });
            if definition.id == RECKLESS_CHARGE_ID || definition.id == RECKLESS_CHARGE_PLUS_ID {
                follow_ups.push(InternalAction::AddCardToPile {
                    content_id: DAZED_ID,
                    to: CardPile::DrawPile,
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
        }
        IRON_WAVE_ID | IRON_WAVE_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: definition.values.damage.unwrap_or(0),
                },
            });
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
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
        MIND_BLAST_ID => {
            let target = target.expect("validated havoc attack target");
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: current_pile_count_with_top_card,
                },
            });
        }
        MASTER_OF_STRATEGY_ID => {
            follow_ups.push(InternalAction::DrawCards { count: 3 });
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
        CLEAVE_ID | CLEAVE_PLUS_ID | DRAMATIC_ENTRANCE_ID => {
            follow_ups.push(InternalAction::DealDamageAll {
                source: card_id,
                amount: definition.values.damage.unwrap_or(0),
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
        DEFEND_R_ID => {
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
        }
        SHRUG_IT_OFF_ID => {
            follow_ups.push(InternalAction::GainBlock { amount: 8 });
            follow_ups.push(InternalAction::DrawCards { count: 1 });
        }
        IMPATIENCE_ID => {
            follow_ups.push(InternalAction::DrawCardsIfNoAttacksInHand { count: 2 });
        }
        IMPATIENCE_PLUS_ID => {
            follow_ups.push(InternalAction::DrawCardsIfNoAttacksInHand { count: 3 });
        }
        CHRYSALIS_ID => {
            for content_id in card_effects::chrysalis_generated_skills(state, 3) {
                follow_ups.push(InternalAction::AddGeneratedCardToPile {
                    content_id,
                    to: CardPile::DrawPile,
                    temp_cost: Some(0),
                    temp_cost_turn_only: false,
                });
            }
        }
        PANIC_BUTTON_ID => {
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
            follow_ups.push(InternalAction::PreventBlockGain { turns: 2 });
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
            follow_ups.push(InternalAction::ShuffleDiscardIntoDraw);
            follow_ups.push(InternalAction::DrawCards { count });
        }
        ENLIGHTENMENT_ID | ENLIGHTENMENT_PLUS_ID => {
            follow_ups.extend(card_effects::enlightenment_cost_actions(
                state,
                card_id,
                definition.id == ENLIGHTENMENT_PLUS_ID,
            ));
        }
        OFFERING_ID => {
            follow_ups.push(InternalAction::LoseHp {
                amount: 6,
                source: HpLossSource::Card(card_id),
            });
            follow_ups.push(InternalAction::GainEnergy { amount: 2 });
            follow_ups.push(InternalAction::DrawCards { count: 3 });
        }
        PANACEA_ID => {
            follow_ups.push(InternalAction::GainArtifact { amount: 1 });
        }
        POWER_THROUGH_ID | POWER_THROUGH_PLUS_ID => {
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
            follow_ups.push(InternalAction::AddCardToPile {
                content_id: WOUND_ID,
                to: CardPile::Hand,
            });
            follow_ups.push(InternalAction::AddCardToPile {
                content_id: WOUND_ID,
                to: CardPile::Hand,
            });
        }
        _ if definition.values.block.is_some() => {
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
        }
        _ => {}
    }

    if exhaust_played_card || definition.keywords.exhaust {
        state.piles.exhaust_pile.push(card);
        follow_ups.push(InternalAction::CardExhausted { card_id });
    } else if !card.combat_only {
        state.piles.discard_pile.push(card);
    }

    Ok(follow_ups)
}

pub fn choose_hand_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    let hand_index = hand_select_ui_to_hand_index(state, ui_index)?;
    let hand_select = state
        .hand_select
        .as_mut()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    hand_select.selected_hand_index = Some(hand_index);
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
        HandSelectPurpose::ArmamentsUpgrade => upgrade_content_id(card.content_id).is_some(),
        HandSelectPurpose::ForethoughtPutOnDraw => true,
        HandSelectPurpose::DualWieldCopy => dual_wield_select_allows_card(card),
    }
}

pub fn confirm_hand_select(state: &mut CombatState) -> SimResult<()> {
    let hand_select = state
        .hand_select
        .take()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    let index = hand_select
        .selected_hand_index
        .ok_or(SimError::IllegalAction("hand select choice is required"))?;
    match hand_select.purpose {
        HandSelectPurpose::WarcryPutOnDraw => {
            confirm_warcry_select(state, hand_select.source_card_id, index)
        }
        HandSelectPurpose::ThinkingAheadPutOnDraw => {
            confirm_thinking_ahead_select(state, hand_select.source_card_id, index)
        }
        HandSelectPurpose::ArmamentsUpgrade => {
            confirm_armaments_select(state, hand_select.source_card_id, index)
        }
        HandSelectPurpose::ForethoughtPutOnDraw => {
            confirm_forethought_select(state, hand_select.source_card_id, index)
        }
        HandSelectPurpose::DualWieldCopy => {
            confirm_dual_wield_select(state, hand_select.source_card_id, index)
        }
    }
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
        .find(|card| card.id == source_card_id)
        .and_then(|card| get_card_definition(card.content_id))
        .ok_or(SimError::IllegalAction("draw select source card missing"))
}

fn move_draw_select_source_card(
    state: &mut CombatState,
    source_card_id: CardId,
    _source_definition: &'static crate::card::CardDefinition,
) -> SimResult<()> {
    move_delayed_played_source_with_strange_spoon(state, source_card_id)
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
        .filter(|card| card.id != source_card_id && upgrade_content_id(card.content_id).is_some())
        .count();
    let cannot_upgrade = if upgradeable_count > 1 {
        let card_ids: Vec<CardId> = state
            .piles
            .hand
            .iter()
            .filter(|card| {
                card.id != source_card_id && upgrade_content_id(card.content_id).is_none()
            })
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
    let source_definition = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == source_card_id)
        .and_then(|card| get_card_definition(card.content_id))
        .ok_or(SimError::IllegalAction("Forethought source card missing"))?;
    let mut card = remove_card_from_pile(state, card_id, CardPile::Hand)?;
    card.temp_cost = Some(0);
    state.piles.draw_pile.push(card);
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
    let next_id = CardId::new(state.piles.max_card_instance_id() + 1);
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
    let content_id = selected.content_id;
    state.piles.hand = unselected_selectable;
    state.piles.hand.extend(nonselectable);
    state.piles.hand.push(selected);
    state
        .piles
        .hand
        .push(CardInstance::new(next_id, content_id));
    state.piles.hand.push(source_card);
    move_card(state, source_card_id, CardPile::Hand, CardPile::ExhaustPile)
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
    let source_card_id = discard_select
        .source_card_id
        .ok_or(SimError::IllegalAction("discard select source is required"))?;
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
        Ok(())
    } else {
        move_card(state, source_card_id, CardPile::Hand, CardPile::DiscardPile)
    }
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
        return state
            .piles
            .hand
            .iter()
            .enumerate()
            .filter(|(_, card)| card.id != source_card_id)
            .map(|(index, _)| index)
            .nth(ui_index)
            .ok_or(SimError::IllegalAction("exhaust select index out of range"));
    }
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::TrueGritExhaustOne {
        let source_card_id = exhaust_select
            .source_card_id
            .ok_or(SimError::IllegalAction("exhaust select source is required"))?;
        return state
            .piles
            .hand
            .iter()
            .enumerate()
            .filter(|(_, card)| card.id != source_card_id)
            .map(|(index, _)| index)
            .nth(ui_index)
            .ok_or(SimError::IllegalAction("exhaust select index out of range"));
    }
    if ui_index >= state.piles.hand.len() {
        return Err(SimError::IllegalAction("exhaust select index out of range"));
    }
    Ok(ui_index)
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
    let source_card_id = exhaust_select
        .source_card_id
        .ok_or(SimError::IllegalAction("exhaust select source is required"))?;
    let selected = unique_selected_indices_in_choice_order(exhaust_select.selected_hand_indices);
    let target_index = selected.first().copied().ok_or(SimError::IllegalAction(
        "True Grit requires a selected card",
    ))?;
    if target_index >= state.piles.hand.len() {
        return Err(SimError::IllegalAction("exhaust select index out of range"));
    }
    let target_card_id = state.piles.hand[target_index].id;
    if target_card_id == source_card_id {
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

    let source_position = state
        .piles
        .hand
        .iter()
        .position(|card| card.id == source_card_id)
        .ok_or(SimError::UnknownCard(source_card_id))?;
    let source_card = state.piles.hand.remove(source_position);
    state.piles.discard_pile.push(source_card);
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
    let cap = purity_select_cap(state, source_card_id)?;
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
    let source_destination = purity_source_destination(state);
    move_card(state, source_card_id, CardPile::Hand, source_destination)?;
    if source_destination == CardPile::ExhaustPile {
        apply_on_exhaust_effects(state, source_card_id);
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
        .find(|card| card.id == source_card_id)
        .ok_or(SimError::IllegalAction("Purity source card is not in hand"))?;
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
    for index in selected.into_iter().rev() {
        if index >= state.piles.hand.len() {
            return Err(SimError::IllegalAction("exhaust select index out of range"));
        }
        let card = state.piles.hand.remove(index);
        state.piles.discard_pile.push(card);
    }
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
    use crate::content::monsters::{monster_state, FUNGI_BEAST_A0, JAW_WORM_A0};

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
}
