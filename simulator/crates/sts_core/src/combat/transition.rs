use super::card_effects;
mod card_actions;
mod damage_actions;
mod decision_actions;
mod defense_actions;
mod pile_actions;
mod player_actions;
use crate::{
    action::{CardPile, CombatAction, HpLossSource, InternalAction},
    card::{CardType, TargetRequirement},
    combat::{
        apply_burning_blood,
        cost::{effective_card_cost, printed_card_cost},
        damage::{
            deal_damage_info_to_monster_with_result, deal_unmodified_damage_to_monster,
            reflect_spikes_to_player, DamageInfo, DamageSource,
        },
        validate_combat_action, CombatDecisionState, CombatPhase, DiscardSelectPurpose,
        DrawSelectPurpose, HandSelectPurpose,
    },
    content::cards::{
        card_instance_is_upgradeable, get_card_definition, required_upgrade_content_id,
        upgrade_card_instance, DAZED_ID, DUAL_WIELD_PLUS_ID, EXHUME_ID, EXHUME_PLUS_ID, PAIN_ID,
        PURITY_PLUS_ID, SENTINEL_ID, SENTINEL_PLUS_ID,
    },
    content::monsters::{
        apply_collector_death_escape, apply_gremlin_leader_death_escape, check_slime_boss_split,
        enter_guardian_defensive_mode, get_monster_definition, guardian_accumulate_hp_damage,
        release_stasis_card_on_death, wake_lagavulin_on_damage, GIANT_HEAD_ID, GUARDIAN_ID,
    },
    content::shop_pool::{colorless_discovery_pool, ironclad_combat_discovery_pool},
    ids::{CardId, ContentId, MonsterId},
    power::{
        apply_monster_vulnerable, apply_monster_weak, apply_player_vulnerable, calculate_block,
    },
    relic::Relic,
    rng::JavaRng,
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

    let transition = match action {
        CombatAction::PlayCard { card_id, target } => apply_play_card(state, card_id, target),
        CombatAction::EndTurn => apply_end_turn(state),
    }?;
    transition.state.validate()?;
    Ok(transition)
}

fn apply_end_turn(state: &CombatState) -> SimResult<CombatTransition> {
    let ethereal_ids = end_turn_ethereal_hand_card_ids(state);
    let next = crate::combat::end_player_turn(state)?;
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

    Ok(CombatTransition {
        state: next,
        event_log,
    })
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
        if !queue.is_empty() {
            if let Some(CombatDecisionState::HandSelect {
                pending_actions, ..
            }) = next.decision.as_mut()
            {
                pending_actions.extend(queue.drain(..));
                break;
            }
        }
    }

    flush_pending_player_spikes_damage_if_ready(&mut next)?;
    flush_pending_monster_death_relics_if_ready(&mut next)?;

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
        apply_burning_blood(&mut next)?;
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

pub fn flush_pending_player_spikes_damage_if_ready(state: &mut CombatState) -> SimResult<()> {
    let mut next = state.clone();
    if next.pending_player_spikes_damage <= 0 || next.decision.is_some() {
        return Ok(());
    }
    let damage = std::mem::take(&mut next.pending_player_spikes_damage);
    let hp_loss = reflect_spikes_to_player(&mut next.player, &next.relics, damage);
    crate::combat::hp_loss::apply_player_hp_loss_hooks(&mut next, hp_loss)?;
    *state = next;
    Ok(())
}

pub fn flush_pending_monster_death_relics_if_ready(state: &mut CombatState) -> SimResult<()> {
    let mut next = state.clone();
    if next.pending_monster_death_relic_triggers == 0 || next.decision.is_some() {
        return Ok(());
    }
    let triggers = std::mem::take(&mut next.pending_monster_death_relic_triggers);
    for _ in 0..triggers {
        crate::relic::apply_monster_death_relics(&mut next)?;
    }
    *state = next;
    Ok(())
}

fn checked_combat_sum(value: i32, amount: i32) -> SimResult<i32> {
    value.checked_add(amount).ok_or(SimError::InvalidState(
        "combat integer addition overflows i32",
    ))
}

fn checked_add_combat_value(value: &mut i32, amount: i32) -> SimResult<()> {
    *value = checked_combat_sum(*value, amount)?;
    Ok(())
}

fn apply_internal_action(
    state: &mut CombatState,
    action: InternalAction,
) -> SimResult<Vec<InternalAction>> {
    match action {
        InternalAction::ConsumeDuplicationPotion => card_actions::consume_duplication_potion(state),
        InternalAction::ConsumeDoubleTap => card_actions::consume_double_tap(state),
        InternalAction::ConsumeNecronomicon => card_actions::consume_necronomicon(state),
        InternalAction::PlayCard { card_id } => card_actions::play_card(state, card_id),
        InternalAction::PlayCardCopy { card_id } => card_actions::play_card_copy(state, card_id),
        InternalAction::SkipCopiedCardEffectsIfTargetDead { .. }
        | InternalAction::SkipCopiedCardEffectsIfCombatDone
        | InternalAction::EndCopiedCardEffects => Ok(Vec::new()),
        InternalAction::SpendEnergy { amount } => card_actions::spend_energy(state, amount),
        InternalAction::SpendCardEnergy { card_id } => {
            card_actions::spend_card_energy(state, card_id)
        }
        InternalAction::SetHandCardCostForTurn { card_id, cost } => {
            card_actions::set_hand_card_cost_for_turn(state, card_id, cost)
        }
        InternalAction::SetHandCardCostForCombat { card_id, cost } => {
            card_actions::set_hand_card_cost_for_combat(state, card_id, cost)
        }
        InternalAction::DealDamage { info } => damage_actions::deal_damage(state, info),
        InternalAction::DealHandOfGreedDamage { info, gold } => {
            damage_actions::deal_hand_of_greed_damage(state, info, gold)
        }
        InternalAction::DealDamageRandomEnemy { source, amount } => {
            damage_actions::deal_damage_random_enemy(state, source, amount)
        }
        InternalAction::DealDamageAndHealUnblocked { info } => {
            damage_actions::deal_damage_and_heal_unblocked(state, info)
        }
        InternalAction::DealFeedDamage { info, max_hp_gain } => {
            damage_actions::deal_feed_damage(state, info, max_hp_gain)
        }
        InternalAction::DealRitualDaggerDamage { info, growth } => {
            damage_actions::deal_ritual_dagger_damage(state, info, growth)
        }
        InternalAction::DealDamageAll { source, amount } => {
            damage_actions::deal_damage_all(state, source, amount)
        }
        InternalAction::DealDamageAllRepeated {
            source,
            amount,
            times,
        } => damage_actions::deal_damage_all_repeated(state, source, amount, times),
        InternalAction::DealDamageAllAndHealUnblocked { source, amount } => {
            damage_actions::deal_damage_all_and_heal_unblocked(state, source, amount)
        }
        InternalAction::HealPlayer { amount } => defense_actions::heal_player(state, amount),
        InternalAction::GainBlock { amount } => defense_actions::gain_player_block(state, amount),
        InternalAction::GainMonsterBlock { target, amount } => {
            defense_actions::gain_monster_block(state, target, amount)
        }
        InternalAction::PreventBlockGain { turns } => {
            defense_actions::prevent_block_gain(state, turns)
        }
        InternalAction::GainTemporaryThorns { amount } => {
            defense_actions::gain_temporary_thorns(state, amount)
        }
        InternalAction::DoublePlayerBlock => defense_actions::double_player_block(state),
        InternalAction::ApplyVulnerable { target, amount } => {
            defense_actions::apply_monster_vulnerable(state, target, amount)
        }
        InternalAction::ApplyPlayerVulnerable { amount } => {
            defense_actions::apply_player_vulnerable(state, amount)
        }
        InternalAction::ApplyWeak { target, amount } => {
            defense_actions::apply_weak(state, target, amount)
        }
        InternalAction::ReduceMonsterStrength { target, amount } => {
            defense_actions::reduce_strength(state, target, amount)
        }
        InternalAction::ReduceMonsterStrengthThisTurn { target, amount } => {
            defense_actions::reduce_strength_this_turn(state, target, amount)
        }
        InternalAction::MoveCard { card_id, from, to } => {
            pile_actions::move_card_between_piles(state, card_id, from, to)
        }
        InternalAction::ReturnExhaustCardToHand { card_id } => {
            pile_actions::return_exhaust_card_to_hand(state, card_id)
        }
        InternalAction::ForethoughtAutoMove {
            source_card_id,
            card_id,
        } => pile_actions::forethought_auto_move(state, source_card_id, card_id),
        InternalAction::ExhaustRandomHandCardExcept { excluded_card_id } => {
            pile_actions::exhaust_random_hand_card_except(state, excluded_card_id)
        }
        InternalAction::RemoveCard { card_id, from } => {
            pile_actions::remove_card(state, card_id, from)
        }
        InternalAction::AddCardToPile { content_id, to } => {
            pile_actions::add_card(state, content_id, to)
        }
        InternalAction::AddGeneratedCardToPile {
            content_id,
            to,
            temp_cost,
            temp_cost_turn_only,
        } => {
            pile_actions::add_generated_card(state, content_id, to, temp_cost, temp_cost_turn_only)
        }
        InternalAction::AddGeneratedHandCardBeforePendingDraw {
            content_id,
            temp_cost,
            temp_cost_turn_only,
        } => pile_actions::add_generated_hand_card_before_pending_draw(
            state,
            content_id,
            temp_cost,
            temp_cost_turn_only,
        ),
        InternalAction::AddStatEquivalentCopyToPile { card, to } => {
            pile_actions::add_stat_equivalent_copy(state, card, to)
        }
        InternalAction::AddGeneratedCardToDrawPileRandomSpot { content_id } => {
            pile_actions::add_generated_card_to_random_draw_spot(state, content_id, None, false)
        }
        InternalAction::AddGeneratedCardToDrawPileRandomSpotWithCost {
            content_id,
            temp_cost,
            temp_cost_turn_only,
        } => pile_actions::add_generated_card_to_random_draw_spot(
            state,
            content_id,
            temp_cost,
            temp_cost_turn_only,
        ),
        InternalAction::AddRandomColorlessCardToHand { temp_cost, upgrade } => {
            pile_actions::add_random_colorless_card_to_hand(state, temp_cost, upgrade)
        }
        InternalAction::DrawCards { count } => pile_actions::draw_cards(state, count),
        InternalAction::DrawCardsWhilePlayedCardIsInLimbo { card_id, count } => {
            pile_actions::draw_cards_while_played_card_is_in_limbo(state, card_id, count)
        }
        InternalAction::DrawCardsFromInkBottle { count } => pile_actions::draw_cards(state, count),
        InternalAction::ShuffleDiscardIntoDraw => pile_actions::shuffle_discard_into_draw(state),
        InternalAction::DeepBreathShuffleDiscardIntoDraw => {
            pile_actions::deep_breath_shuffle_discard_into_draw(state)
        }
        InternalAction::DrawCardsIfNoAttacksInHand { count } => {
            pile_actions::draw_cards_if_no_attacks_in_hand(state, count)
        }
        InternalAction::DrawRandomAttacksFromDrawPile { count } => {
            pile_actions::draw_random_attacks(state, count)
        }
        InternalAction::GainEnergy { amount } => player_actions::gain_energy(state, amount),
        InternalAction::LoseHp { amount, source } => player_actions::lose_hp(state, amount, source),
        InternalAction::SetCannotDraw => player_actions::set_cannot_draw(state),
        InternalAction::GainRage { amount } => player_actions::gain_rage(state, amount),
        InternalAction::SetRandomHandCardCostForCombat { amount } => {
            player_actions::set_random_hand_card_cost(state, amount)
        }
        InternalAction::UpgradeHandCardsExcept { card_id } => {
            player_actions::upgrade_hand_cards_other_than(state, card_id)
        }
        InternalAction::UpgradeHandCard { card_id } => {
            player_actions::upgrade_one_hand_card(state, card_id)
        }
        InternalAction::IncreaseRampageDamage { card_id, amount } => {
            player_actions::increase_rampage_damage(state, card_id, amount)
        }
        InternalAction::GainFeelNoPain { amount } => {
            player_actions::gain_feel_no_pain(state, amount)
        }
        InternalAction::GainDarkEmbrace { amount } => {
            player_actions::gain_dark_embrace(state, amount)
        }
        InternalAction::GainBarricade { amount } => player_actions::gain_barricade(state, amount),
        InternalAction::GainEvolve { amount } => player_actions::gain_evolve(state, amount),
        InternalAction::GainBerserk { amount } => player_actions::gain_berserk(state, amount),
        InternalAction::GainRupture { amount } => player_actions::gain_rupture(state, amount),
        InternalAction::GainJuggernaut { amount } => player_actions::gain_juggernaut(state, amount),
        InternalAction::GainBrutality { amount } => player_actions::gain_brutality(state, amount),
        InternalAction::GainMayhem { amount } => player_actions::gain_mayhem(state, amount),
        InternalAction::GainPanache { amount } => player_actions::gain_panache(state, amount),
        InternalAction::GainCombust { amount } => player_actions::gain_combust(state, amount),
        InternalAction::GainDoubleTap { amount } => player_actions::gain_double_tap(state, amount),
        InternalAction::GainFireBreathing { amount } => {
            player_actions::gain_fire_breathing(state, amount)
        }
        InternalAction::GainCorruption { amount } => player_actions::gain_corruption(state, amount),
        InternalAction::GainSadisticNature { amount } => {
            player_actions::gain_sadistic_nature(state, amount)
        }
        InternalAction::GainMagnetism { amount } => player_actions::gain_magnetism(state, amount),
        InternalAction::ArmTheBomb { turns, damage } => {
            player_actions::arm_the_bomb(state, turns, damage)
        }
        InternalAction::DealUnmodifiedDamage { target, amount } => {
            deal_unmodified_damage_to_living_monster(state, target, amount)?;
            Ok(Vec::new())
        }
        InternalAction::GainMetallicize { amount } => {
            player_actions::gain_metallicize(state, amount)
        }
        InternalAction::GainStrength { amount } => player_actions::gain_strength(state, amount),
        InternalAction::GainDexterity { amount } => player_actions::gain_dexterity(state, amount),
        InternalAction::GainTempStrength { amount } => {
            player_actions::gain_temp_strength(state, amount)
        }
        InternalAction::GainIntangible { amount } => player_actions::gain_intangible(state, amount),
        InternalAction::GainRitual { amount } => player_actions::gain_ritual(state, amount),
        InternalAction::GainArtifact { amount } => player_actions::gain_artifact(state, amount),
        InternalAction::UpgradeCombatCards => player_actions::upgrade_all_combat_cards(state),
        InternalAction::CardExhausted { card_id } => {
            apply_on_exhaust_effects(state, card_id)?;
            Ok(dead_branch_follow_up(state).into_iter().collect())
        }
        InternalAction::HandCardExhausted { card_id } => {
            apply_on_exhaust_effects(state, card_id)?;
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
            let next_id = CardId::new(state.next_card_instance_id()?);
            state
                .piles
                .hand
                .push(CardInstance::new(next_id, card.content_id));
            Ok(Vec::new())
        }
        InternalAction::AwaitHandSelect {
            source_card_id,
            purpose,
        } => decision_actions::await_hand_select(state, source_card_id, purpose),
        InternalAction::AwaitDrawSelect {
            source_card_id,
            purpose,
        } => decision_actions::await_draw_select(state, source_card_id, purpose),
        InternalAction::AwaitDiscardSelect {
            source_card_id,
            purpose,
        } => decision_actions::await_discard_select(state, source_card_id, purpose),
        InternalAction::AwaitExhaustSelect {
            source_card_id,
            purpose,
        } => decision_actions::await_exhaust_select(state, source_card_id, purpose),
        InternalAction::OpenDiscoveryCardReward { source_card_id } => {
            decision_actions::open_discovery_card_reward(state, source_card_id)
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
        .rng
        .card_random_rng
        .random_int_range(0, (candidates.len() - 1) as i32) as usize;
    let card = &mut state.piles.hand[candidates[pick]];
    card.temp_cost = Some(0);
    card.temp_cost_turn_only = true;
}

fn apply_on_card_play_powers(
    state: &mut CombatState,
    card_type: CardType,
) -> SimResult<Vec<InternalAction>> {
    let mut follow_ups = Vec::new();

    for monster in state.monsters.iter_mut().filter(|monster| monster.alive) {
        if monster.content_id == GIANT_HEAD_ID || monster.powers.slow > 0 {
            checked_add_combat_value(&mut monster.powers.slow, 1)?;
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
            .try_fold(0, checked_combat_sum)?;
        checked_add_combat_value(&mut state.pending_player_spikes_damage, sharp_hide_damage)?;
    }

    if state.player.powers.hex > 0 && card_type != CardType::Attack {
        for _ in 0..state.player.powers.hex {
            follow_ups.push(InternalAction::AddGeneratedCardToDrawPileRandomSpot {
                content_id: DAZED_ID,
            });
        }
    }

    if state.player.powers.panache <= 0 {
        return Ok(follow_ups);
    }
    checked_add_combat_value(&mut state.player.powers.panache_cards_played, 1)?;
    if state.player.powers.panache_cards_played < 5 {
        return Ok(follow_ups);
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
    Ok(follow_ups)
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
            apply_monster_death_hooks(state, target)?;
        }
        apply_or_queue_spikes_to_player(state, monster_content_id, spikes)?;
    }

    Ok((total_hp_damage, follow_ups))
}

fn apply_or_queue_spikes_to_player(
    state: &mut CombatState,
    monster_content_id: ContentId,
    spikes: i32,
) -> SimResult<()> {
    if spikes <= 0 {
        return Ok(());
    }
    if monster_content_id == GUARDIAN_ID {
        return Ok(());
    }
    let hp_loss = reflect_spikes_to_player(&mut state.player, &state.relics, spikes);
    crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss)
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
        apply_monster_death_hooks(state, target)?;
    }
    Ok(())
}

pub(crate) fn apply_monster_death_hooks(
    state: &mut CombatState,
    monster_id: MonsterId,
) -> SimResult<()> {
    let mut next = state.clone();
    apply_monster_death_non_relic_hooks(&mut next, monster_id)?;
    if next.monsters.iter().any(|monster| monster.alive) {
        crate::relic::apply_monster_death_relics(&mut next)?;
    }
    *state = next;
    Ok(())
}

fn queue_monster_death_hooks(state: &mut CombatState, monster_id: MonsterId) -> SimResult<()> {
    apply_monster_death_non_relic_hooks(state, monster_id)?;
    if state.monsters.iter().any(|monster| monster.alive)
        && state.relics.contains(&Relic::GremlinHorn)
    {
        state.pending_monster_death_relic_triggers = state
            .pending_monster_death_relic_triggers
            .checked_add(1)
            .ok_or(SimError::InvalidState(
                "combat death trigger counter overflows u32",
            ))?;
    }
    Ok(())
}

fn apply_monster_death_non_relic_hooks(
    state: &mut CombatState,
    monster_id: MonsterId,
) -> SimResult<()> {
    if let Some(monster) = state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == monster_id)
    {
        release_stasis_card_on_death(monster, &mut state.piles);
    }
    apply_gremlin_leader_death_escape(&mut state.monsters, monster_id);
    apply_collector_death_escape(&mut state.monsters, monster_id);
    apply_spore_cloud_on_monster_death(state, monster_id)
}

fn apply_spore_cloud_on_monster_death(
    state: &mut CombatState,
    monster_id: MonsterId,
) -> SimResult<()> {
    let amount = state
        .monsters
        .iter()
        .find(|monster| monster.id == monster_id)
        .map_or(0, |monster| monster.powers.spore_cloud);
    if amount <= 0 || !state.monsters.iter().any(|monster| monster.alive) {
        return Ok(());
    }

    apply_player_vulnerable(&mut state.player.powers, amount)?;
    Ok(())
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
        let mut next_powers = monster.powers;
        vulnerable_applied = apply_monster_vulnerable(&mut next_powers, amount)?;
        if vulnerable_applied && applies_champion_belt {
            champion_belt_weak_applied =
                apply_monster_weak(&mut next_powers, crate::relic::CHAMPION_BELT_WEAK)?;
        }
        monster.powers = next_powers;
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

pub(crate) fn apply_juggernaut_after_direct_block_gain(
    state: &mut CombatState,
    gained: i32,
) -> SimResult<()> {
    if let Some(InternalAction::DealUnmodifiedDamage { target, amount }) =
        juggernaut_follow_up_for_positive_block_gain(state, gained)
            .into_iter()
            .next()
    {
        deal_unmodified_damage_to_living_monster(state, target, amount)?;
    }
    Ok(())
}

fn apply_player_card_block_gain(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    if state.player.no_block_turns > 0 {
        return Ok(Vec::new());
    }
    let gained = calculate_block(amount, state.player.powers);
    checked_add_combat_value(&mut state.player.block, gained)?;
    Ok(juggernaut_follow_up_for_positive_block_gain(state, gained))
}

pub(crate) fn apply_player_direct_block_gain(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<()> {
    if state.player.no_block_turns > 0 {
        return Ok(());
    }
    // The target runtime uses signed 32-bit arithmetic. Authoritative combat
    // transitions validate that block remains nonnegative before returning.
    state.player.block = state.player.block.wrapping_add(amount);
    apply_juggernaut_after_direct_block_gain(state, amount)
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
    let index = state
        .rng
        .card_random_rng
        .random_int((living.len() - 1) as i32) as usize;
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
    let index = state
        .rng
        .card_random_rng
        .random_int((candidates.len() - 1) as i32) as usize;
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
        .rng
        .card_random_rng
        .random_int((pool.len() - 1) as i32) as usize;
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

pub(crate) fn apply_on_exhaust_effects(state: &mut CombatState, card_id: CardId) -> SimResult<()> {
    match exhausted_card_content_id(state, card_id) {
        // Energy is nonnegative in every valid combat state, so signed target
        // overflow is rejected by the authoritative transition validation.
        Some(SENTINEL_PLUS_ID) => state.player.energy = state.player.energy.wrapping_add(3),
        Some(SENTINEL_ID) => state.player.energy = state.player.energy.wrapping_add(2),
        _ => {}
    }
    if state.player.powers.feel_no_pain > 0 {
        let gained = state.player.powers.feel_no_pain;
        apply_player_direct_block_gain(state, gained)?;
    }
    if state.player.powers.dark_embrace > 0 {
        player_draw_cards(state, state.player.powers.dark_embrace as usize)?;
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
                apply_monster_death_hooks(state, target)?;
            }
        }
    }
    Ok(())
}

fn exhausted_card_content_id(state: &CombatState, card_id: CardId) -> Option<ContentId> {
    state
        .piles
        .exhaust_pile
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.content_id)
}

pub(crate) fn player_draw_cards(state: &mut CombatState, count: usize) -> SimResult<()> {
    if state.player.cannot_draw {
        return Ok(());
    }
    crate::combat::draw::draw_cards_with_combat_rng(state, count)
}

pub(crate) fn player_shuffle_discard_into_draw(state: &mut CombatState) -> SimResult<()> {
    crate::combat::draw::shuffle_discard_into_draw_with_combat_rng(state)
}

pub(crate) fn player_deep_breath_shuffle_discard_into_draw(
    state: &mut CombatState,
) -> SimResult<()> {
    crate::combat::draw::deep_breath_shuffle_discard_into_draw_with_combat_rng(state)
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
            // CardGroup::addToRandomSpot asks cardRandomRng for an inclusive
            // index in 0..=(size - 1). It never appends to the end once the
            // temporary group is non-empty.
            let index = state
                .rng
                .card_random_rng
                .random_int((attack_ids.len() - 1) as i32) as usize;
            attack_ids.insert(index, card.id);
        }
    }

    for _ in 0..count {
        if attack_ids.is_empty() {
            return;
        }

        let shuffle_seed = state.rng.shuffle_rng.random_long();
        JavaRng::new(shuffle_seed).collections_shuffle(&mut attack_ids);

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

fn apply_unceasing_top_after_hand_emptied(state: &mut CombatState) -> SimResult<()> {
    if state.relics.contains(&Relic::UnceasingTop) {
        player_draw_cards(state, crate::relic::UNCEASING_TOP_DRAW)?;
    }
    Ok(())
}

fn add_card_to_pile(state: &mut CombatState, content_id: ContentId, to: CardPile) -> SimResult<()> {
    let next_id = CardId::new(state.next_card_instance_id()?);
    let card = CardInstance::new(next_id, content_id);
    push_card_to_pile(state, card, to);
    Ok(())
}

fn add_generated_card_to_pile(
    state: &mut CombatState,
    content_id: ContentId,
    to: CardPile,
    temp_cost: Option<u8>,
    temp_cost_turn_only: bool,
) -> SimResult<()> {
    let next_id = CardId::new(state.next_card_instance_id()?);
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
    Ok(())
}

fn add_stat_equivalent_copy_to_pile(
    state: &mut CombatState,
    source: CardInstance,
    to: CardPile,
) -> SimResult<()> {
    let next_id = CardId::new(state.next_card_instance_id()?);
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
    Ok(())
}

fn add_generated_card_to_draw_pile_random_spot(
    state: &mut CombatState,
    content_id: ContentId,
    temp_cost: Option<u8>,
    temp_cost_turn_only: bool,
) -> SimResult<()> {
    let next_id = CardId::new(state.next_card_instance_id()?);
    let mut card = CardInstance {
        combat_only: true,
        ..CardInstance::new(next_id, content_id)
    };
    card.temp_cost = temp_cost;
    card.temp_cost_turn_only = temp_cost_turn_only;
    if state.piles.draw_pile.is_empty() {
        state.piles.draw_pile.push(card);
        return Ok(());
    }
    let bound = (state.piles.draw_pile.len() - 1) as i32;
    let index = state.rng.card_random_rng.random_int(bound) as usize;
    state.piles.draw_pile.insert(index, card);
    Ok(())
}

fn random_colorless_card(state: &mut CombatState, upgrade: bool) -> SimResult<ContentId> {
    let pool = colorless_discovery_pool()
        .iter()
        .map(|content_id| {
            if upgrade {
                required_upgrade_content_id(*content_id)
            } else {
                Ok(*content_id)
            }
        })
        .collect::<SimResult<Vec<_>>>()?;
    let max_index = pool
        .len()
        .checked_sub(1)
        .and_then(|index| i32::try_from(index).ok())
        .ok_or(SimError::InvalidState(
            "colorless discovery pool has no representable random bound",
        ))?;
    let idx = state.rng.card_random_rng.random_int(max_index) as usize;
    Ok(pool[idx])
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

fn apply_enrage_on_card_type(state: &mut CombatState, card_type: CardType) -> SimResult<()> {
    if card_type != CardType::Skill {
        return Ok(());
    }

    for monster in &mut state.monsters {
        if !monster.alive {
            continue;
        }
        if get_monster_definition(monster.content_id).is_some_and(|definition| {
            definition.enrage_weak_on_skill > 0 && monster.powers.anger > 0
        }) {
            checked_add_combat_value(&mut monster.powers.strength, monster.powers.anger)?;
        }
    }
    Ok(())
}

fn apply_rage_on_card_type(state: &mut CombatState, card_type: CardType) -> SimResult<()> {
    if card_type == CardType::Attack && state.player.temp_rage_block > 0 {
        apply_player_direct_block_gain(state, state.player.temp_rage_block)?;
    }
    Ok(())
}

fn set_random_hand_card_cost_for_combat(state: &mut CombatState, amount: u8) -> SimResult<()> {
    if state.piles.hand.is_empty() {
        return Ok(());
    }

    let better_possible = state.piles.hand.iter().try_fold(false, |found, card| {
        Ok(found || effective_card_cost(card)? > 0)
    })?;
    let possible = state.piles.hand.iter().try_fold(false, |found, card| {
        Ok(found || printed_card_cost(card)? > 0)
    })?;
    if !better_possible && !possible {
        return Ok(());
    }

    let index = random_madness_candidate_index(state, better_possible)?;

    let card = &mut state.piles.hand[index];
    card.temp_cost = Some(amount);
    card.temp_cost_turn_only = false;
    Ok(())
}

fn random_madness_candidate_index(
    state: &mut CombatState,
    better_possible: bool,
) -> SimResult<usize> {
    loop {
        let bound = (state.piles.hand.len() - 1) as i32;
        let index = state.rng.card_random_rng.random_int(bound) as usize;
        if madness_card_matches(&state.piles.hand[index], better_possible)? {
            return Ok(index);
        }
    }
}

fn madness_card_matches(card: &CardInstance, better_possible: bool) -> SimResult<bool> {
    if better_possible {
        Ok(effective_card_cost(card)? > 0)
    } else {
        Ok(printed_card_cost(card)? > 0)
    }
}

fn upgrade_hand_cards_except(state: &mut CombatState, excluded_card_id: CardId) -> SimResult<()> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != excluded_card_id)
        .map(|card| Ok((card.id, upgrade_card_instance(*card)?)))
        .collect::<SimResult<Vec<_>>>()?;
    for (card_id, upgraded) in upgrades {
        if let Some(upgraded) = upgraded {
            *find_hand_card_mut(state, card_id)? = upgraded;
        }
    }
    Ok(())
}

fn upgrade_hand_card(state: &mut CombatState, card_id: CardId) -> SimResult<()> {
    let card = find_hand_card_mut(state, card_id)?;
    *card = upgrade_card_instance(*card)?.ok_or(SimError::IllegalAction("card cannot upgrade"))?;
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
        player_shuffle_discard_into_draw(state)?;
    }

    let card = state
        .piles
        .draw_pile
        .pop()
        .ok_or(SimError::IllegalAction("draw pile is empty"))?;
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

    if definition.keywords.unplayable || !crate::relic::can_play_card_with_relics(state) {
        let mut follow_ups = Vec::new();
        if exhaust_played_card || definition.keywords.exhaust {
            state.piles.exhaust_pile.push(card);
            follow_ups.push(InternalAction::CardExhausted { card_id: card.id });
        } else if !card.combat_only {
            state.piles.discard_pile.push(card);
        }
        return Ok(follow_ups);
    }

    let (queued_state, queue) =
        card_effects::play_top_draw_card_queue(state, card, target, exhaust_played_card)?;
    *state = queued_state;
    Ok(queue.into())
}

pub fn choose_hand_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    let hand_index = hand_select_ui_to_hand_index(state, ui_index)?;
    let hand_select = state
        .hand_select_mut()
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
        .hand_select()
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
        HandSelectPurpose::ArmamentsUpgrade => card_instance_is_upgradeable(card),
        HandSelectPurpose::ForethoughtPutOnDraw | HandSelectPurpose::ForethoughtPutAnyOnDraw => {
            true
        }
        HandSelectPurpose::DualWieldCopy => dual_wield_select_allows_card(card),
    }
}

pub fn confirm_hand_select(state: &mut CombatState) -> SimResult<()> {
    let (hand_select, pending_actions) = state
        .take_hand_select()
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
    resume_actions_after_hand_select(state, pending_actions)?;
    state.activate_next_queued_decision_if_idle();
    Ok(())
}

fn resume_actions_after_hand_select(
    state: &mut CombatState,
    pending_actions: VecDeque<InternalAction>,
) -> SimResult<()> {
    if pending_actions.is_empty() {
        return Ok(());
    }
    let transition = process_internal_queue(state, pending_actions)?;
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
        .draw_select_mut()
        .ok_or(SimError::IllegalAction("no draw select is open"))?;
    draw_select.selected_draw_index = Some(draw_index);
    Ok(())
}

pub fn draw_select_ui_to_draw_index(state: &CombatState, ui_index: usize) -> SimResult<usize> {
    let draw_select = state
        .draw_select()
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
        .take_draw_select()
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
    }?;
    state.activate_next_queued_decision_if_idle();
    Ok(())
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
    move_delayed_played_source_with_strange_spoon(state, source_card_id)
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
    let Some(source) = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == source_card_id)
        .copied()
    else {
        if state
            .piles
            .discard_pile
            .iter()
            .chain(state.piles.exhaust_pile.iter())
            .any(|card| card.id == source_card_id)
        {
            return Ok(());
        }
        return Err(SimError::IllegalAction(
            "delayed source card is not in a resolved destination",
        ));
    };
    let definition = get_card_definition(source.content_id)
        .ok_or(SimError::UnknownContent(source.content_id))?;
    let destination = delayed_source_card_destination(state, definition);
    move_card(state, source_card_id, CardPile::Hand, destination)?;
    if destination == CardPile::ExhaustPile {
        apply_on_exhaust_effects(state, source_card_id)?;
    }
    Ok(())
}

pub fn close_discovery_card_reward_source(state: &mut CombatState) -> SimResult<()> {
    let source = {
        let Some(CombatDecisionState::DiscoveryCardReward { source_card, .. }) =
            state.decision.as_mut()
        else {
            return Ok(());
        };
        source_card.take()
    };
    close_discovery_source_card(state, source)
}

pub fn close_discovery_source_card(
    state: &mut CombatState,
    source: Option<CardInstance>,
) -> SimResult<()> {
    let Some(source) = source else {
        return Ok(());
    };
    let source_card_id = source.id;
    let definition = get_card_definition(source.content_id)
        .ok_or(SimError::UnknownContent(source.content_id))?;
    let destination = delayed_source_card_destination(state, definition);
    match destination {
        CardPile::ExhaustPile => {
            state.piles.exhaust_pile.push(source);
            apply_on_exhaust_effects(state, source_card_id)?;
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
    if state.rng.card_random_rng.random_bool() {
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
    card_content_definition(state, source_card_id)?;
    let upgraded = upgrade_card_instance(selected)?
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
    move_delayed_played_source_with_strange_spoon(state, source_card_id)?;
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
    card_content_definition(state, source_card_id)
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
    _source_definition: &'static crate::card::CardDefinition,
) -> SimResult<()> {
    move_delayed_played_source_with_strange_spoon(state, source_card_id)
}

fn confirm_dual_wield_select(
    state: &mut CombatState,
    source_card_id: CardId,
    index: usize,
) -> SimResult<()> {
    if index >= state.piles.hand.len() {
        return Err(SimError::IllegalAction("hand select index out of range"));
    }
    let source_card_in_hand = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == source_card_id)
        .copied();
    let source_definition = card_content_definition(state, source_card_id)?;
    let copy_count = if source_definition.id == DUAL_WIELD_PLUS_ID {
        2
    } else {
        1
    };
    let mut next_id = state.reserve_card_instance_ids(copy_count)?;
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
    if let Some(source_card) = source_card_in_hand {
        state.piles.hand.push(source_card);
    }
    move_delayed_played_source_with_strange_spoon(state, source_card_id)?;
    Ok(())
}

fn dual_wield_select_allows_card(card: &CardInstance) -> bool {
    get_card_definition(card.content_id).is_some_and(|definition| {
        matches!(definition.card_type, CardType::Attack | CardType::Power)
    })
}

pub fn open_discard_select_with_max_choices(
    state: &mut CombatState,
    max_choices: usize,
) -> SimResult<()> {
    if state.piles.discard_pile.is_empty() {
        return Err(SimError::IllegalAction("discard pile is empty"));
    }
    state.decision = Some(CombatDecisionState::DiscardSelect {
        state: crate::combat::DiscardSelectState {
            purpose: DiscardSelectPurpose::LiquidMemoriesReturnToHand,
            source_card_id: None,
            source_card: None,
            selected_discard_indices: Vec::new(),
            max_choices,
            selected_discard_index: None,
        },
    });
    Ok(())
}

pub fn choose_discard_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    discard_select_ui_to_discard_index(state, ui_index)?;
    let discard_select = state
        .discard_select_mut()
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
        .discard_select()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    if ui_index >= state.piles.discard_pile.len() {
        return Err(SimError::IllegalAction("discard select index out of range"));
    }
    Ok(ui_index)
}

pub fn confirm_liquid_memories_select(state: &mut CombatState) -> SimResult<()> {
    let discard_select = state
        .take_discard_select()
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
    state.activate_next_queued_decision_if_idle();
    Ok(())
}

pub fn confirm_discard_select(state: &mut CombatState) -> SimResult<()> {
    let purpose = state
        .discard_select()
        .ok_or(SimError::IllegalAction("no discard select is open"))?
        .purpose;
    match purpose {
        DiscardSelectPurpose::LiquidMemoriesReturnToHand => confirm_liquid_memories_select(state),
        DiscardSelectPurpose::HeadbuttPutOnDraw => confirm_headbutt_select(state),
    }
}

pub fn confirm_headbutt_select(state: &mut CombatState) -> SimResult<()> {
    let discard_select = state
        .take_discard_select()
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
    flush_pending_monster_death_relics_if_ready(state)?;
    state.activate_next_queued_decision_if_idle();
    Ok(())
}

pub fn open_exhaust_select(state: &mut CombatState) -> SimResult<()> {
    state.decision = Some(CombatDecisionState::ExhaustSelect {
        state: crate::combat::ExhaustSelectState {
            purpose: crate::combat::ExhaustSelectPurpose::Exhaust,
            source_card_id: None,
            source_card: None,
            selected_hand_indices: Vec::new(),
        },
    });
    Ok(())
}

pub fn open_gambling_chip_select(state: &mut CombatState) -> SimResult<()> {
    if state.piles.hand.is_empty() {
        return Ok(());
    }
    state.decision = Some(CombatDecisionState::ExhaustSelect {
        state: crate::combat::ExhaustSelectState {
            purpose: crate::combat::ExhaustSelectPurpose::GamblingChip,
            source_card_id: None,
            source_card: None,
            selected_hand_indices: Vec::new(),
        },
    });
    Ok(())
}

pub fn choose_exhaust_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    let pile_index = exhaust_select_ui_to_hand_index(state, ui_index)?;
    let purity_cap = state
        .exhaust_select()
        .filter(|select| select.purpose == crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3)
        .map(|select| {
            let source_card_id = select
                .source_card_id
                .ok_or(SimError::IllegalAction("exhaust select source is required"))?;
            purity_select_cap(state, source_card_id)
        })
        .transpose()?;
    let exhaust_select = state
        .exhaust_select_mut()
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
        .exhaust_select()
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
    let Some(exhaust_select) = state.exhaust_select() else {
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
        .take_exhaust_select()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    match exhaust_select.purpose {
        crate::combat::ExhaustSelectPurpose::GamblingChip => {
            confirm_gambling_chip_select(state, exhaust_select.selected_hand_indices)?;
        }
        crate::combat::ExhaustSelectPurpose::ExhumeReturnToHand => {
            confirm_exhume_select(state, exhaust_select)?;
        }
        crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3 => {
            confirm_purity_select(state, exhaust_select)?;
        }
        crate::combat::ExhaustSelectPurpose::BurningPactDraw2 => {
            confirm_burning_pact_select(state, exhaust_select, 2)?;
        }
        crate::combat::ExhaustSelectPurpose::BurningPactDraw3 => {
            confirm_burning_pact_select(state, exhaust_select, 3)?;
        }
        crate::combat::ExhaustSelectPurpose::TrueGritExhaustOne => {
            confirm_true_grit_select(state, exhaust_select)?;
        }
        crate::combat::ExhaustSelectPurpose::Exhaust => {
            let selected =
                unique_selected_indices_in_choice_order(exhaust_select.selected_hand_indices);
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
                apply_on_exhaust_effects(state, card_id)?;
            }
        }
    }
    state.activate_next_queued_decision_if_idle();
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
    apply_on_exhaust_effects(state, target_card_id)?;

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
    apply_on_exhaust_effects(state, card.id)?;
    player_draw_cards(state, draw_count)?;
    if let Some(source_card) = exhaust_select.source_card {
        state.piles.discard_pile.push(source_card);
    } else {
        move_delayed_played_source_with_strange_spoon(state, source_card_id)?;
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
        apply_on_exhaust_effects(state, card.id)?;
    }
    if let Some(source_card) = exhaust_select.source_card {
        let source_destination = purity_source_destination(state);
        let source_card_id = source_card.id;
        push_card_to_pile(state, source_card, source_destination);
        if source_destination == CardPile::ExhaustPile {
            apply_on_exhaust_effects(state, source_card_id)?;
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
            apply_on_exhaust_effects(state, source_card_id)?;
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
    if state.rng.card_random_rng.random_bool() {
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
                .exhaust_select()
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
        apply_on_exhaust_effects(state, source_card_id)?;
    } else {
        let source_is_already_exhausted = state
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == source_card_id);
        if !source_is_already_exhausted {
            move_card(state, source_card_id, CardPile::Hand, CardPile::ExhaustPile)?;
            apply_on_exhaust_effects(state, source_card_id)?;
        }
    }
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
    player_draw_cards(state, count)?;
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

fn add_rampage_damage_bonus(
    state: &mut CombatState,
    card_id: CardId,
    amount: i32,
) -> SimResult<()> {
    let card = find_combat_card_mut(state, card_id).ok_or(SimError::UnknownCard(card_id))?;
    if card.content_id != crate::content::cards::RAMPAGE_ID
        && card.content_id != crate::content::cards::RAMPAGE_PLUS_ID
    {
        return Err(SimError::InvalidState(
            "Rampage growth source is not Rampage",
        ));
    }
    card.rampage_damage_bonus = card
        .rampage_damage_bonus
        .checked_add(amount)
        .ok_or(SimError::InvalidState("Rampage damage bonus overflows i32"))?;
    Ok(())
}

fn add_ritual_dagger_damage_bonus(
    state: &mut CombatState,
    card_id: CardId,
    amount: i32,
) -> SimResult<()> {
    let card = find_combat_card_mut(state, card_id).ok_or(SimError::UnknownCard(card_id))?;
    card.ritual_dagger_damage_bonus = card
        .ritual_dagger_damage_bonus
        .checked_add(amount.max(0))
        .ok_or(SimError::InvalidState(
            "Ritual Dagger damage bonus overflows i32",
        ))?;
    Ok(())
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

fn upgrade_combat_cards(state: &mut CombatState) -> SimResult<()> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .chain(state.piles.draw_pile.iter())
        .chain(state.piles.discard_pile.iter())
        .chain(state.piles.exhaust_pile.iter())
        .map(|card| Ok((card.id, upgrade_card_instance(*card)?)))
        .collect::<SimResult<Vec<_>>>()?;
    for (card_id, upgraded) in upgrades {
        if let Some(upgraded) = upgraded {
            *find_combat_card_mut(state, card_id).ok_or(SimError::UnknownCard(card_id))? = upgraded;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::*;
    use crate::content::monsters::{
        monster_state, DARKLING_A0, FUNGI_BEAST_A0, GUARDIAN_A0, JAW_WORM_A0, SNAKE_PLANT_A0,
    };
    use crate::rng::StsRng;

    #[test]
    fn rampage_growth_overflow_and_wrong_source_fail_closed() {
        let mut state = CombatState::initial_fixture();
        let mut rampage = CardInstance::new(CardId::new(1), RAMPAGE_ID);
        rampage.rampage_damage_bonus = i32::MAX;
        state.piles.hand = vec![rampage];
        let before = state.clone();

        assert_eq!(
            add_rampage_damage_bonus(&mut state, CardId::new(1), 1),
            Err(SimError::InvalidState("Rampage damage bonus overflows i32"))
        );
        assert_eq!(state, before);

        state.piles.hand[0] = CardInstance::new(CardId::new(1), STRIKE_R_ID);
        let wrong_source = state.clone();
        assert_eq!(
            add_rampage_damage_bonus(&mut state, CardId::new(1), 5),
            Err(SimError::InvalidState(
                "Rampage growth source is not Rampage"
            ))
        );
        assert_eq!(state, wrong_source);
    }

    #[test]
    fn ritual_dagger_growth_overflow_and_missing_source_fail_closed() {
        let mut state = CombatState::initial_fixture();
        let mut ritual_dagger = CardInstance::new(CardId::new(1), RITUAL_DAGGER_ID);
        ritual_dagger.ritual_dagger_damage_bonus = i32::MAX;
        state.piles.hand = vec![ritual_dagger];
        let before = state.clone();

        assert_eq!(
            add_ritual_dagger_damage_bonus(&mut state, CardId::new(1), 1),
            Err(SimError::InvalidState(
                "Ritual Dagger damage bonus overflows i32"
            ))
        );
        assert_eq!(state, before);
        assert_eq!(
            add_ritual_dagger_damage_bonus(&mut state, CardId::new(99), 1),
            Err(SimError::UnknownCard(CardId::new(99)))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn power_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.strength = i32::MAX;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), INFLAME_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
    }

    #[test]
    fn red_skull_healing_failure_reaches_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::RedSkull);
        state.player.hp = state.player.max_hp / 2;
        state.player.powers.strength = i32::MIN;
        state.relic_counters.red_skull_active = true;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), BANDAGE_UP_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "Red Skull Strength removal underflows i32"
            ))
        );
    }

    #[test]
    fn monster_vulnerable_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].powers.vulnerable = i32::MAX;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), BASH_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: Some(state.monsters[0].id),
                },
            ),
            Err(SimError::InvalidState(
                "monster Vulnerable application overflows i32"
            ))
        );
    }

    #[test]
    fn champion_belt_weak_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::ChampionBelt);
        state.monsters[0].powers.weak = i32::MAX;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), BASH_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: Some(state.monsters[0].id),
                },
            ),
            Err(SimError::InvalidState(
                "monster Weak application overflows i32"
            ))
        );
        assert_eq!(state.monsters[0].powers.vulnerable, 0);
        assert_eq!(state.monsters[0].powers.weak, i32::MAX);
    }

    #[test]
    fn card_play_trigger_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].powers.slow = i32::MAX;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), INFLAME_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
    }

    #[test]
    fn copied_hand_card_rejects_id_exhaustion_without_mutating_combat() {
        let mut state = CombatState::initial_fixture();
        let card_id = CardId::new(crate::ids::MAX_SUPPORTED_CARD_INSTANCE_ID);
        state.piles.hand = vec![CardInstance::new(card_id, STRIKE_R_ID)];
        let before = state.clone();

        assert_eq!(
            apply_internal_action(&mut state, InternalAction::CopyHandCardToHand { card_id },),
            Err(SimError::InvalidState(
                "card instance ID allocation exceeds the supported domain"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn rage_block_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.player.block = i32::MAX;
        state.player.temp_rage_block = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: Some(state.monsters[0].id),
                },
            ),
            Err(SimError::InvalidState(
                "combat player block or energy is negative"
            ))
        );
    }

    #[test]
    fn sentinel_energy_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = i32::MAX;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), TRUE_GRIT_ID),
            CardInstance::new(CardId::new(2), SENTINEL_ID),
        ];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "combat player block or energy is negative"
            ))
        );
    }

    #[test]
    fn relic_counter_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.relic_counters.cards_played_this_turn = i32::MAX as u32;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), INFLAME_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "combat relic counter exceeds the target signed range"
            ))
        );
    }

    #[test]
    fn nunchaku_energy_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = i32::MAX;
        state.relics.push(Relic::Nunchaku);
        state.relic_counters.nunchaku_attacks_played = crate::relic::NUNCHAKU_THRESHOLD - 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), ANGER_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: Some(state.monsters[0].id),
                },
            ),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
    }

    fn shuffle_trigger_state(relic: Relic) -> CombatState {
        let mut state = CombatState::initial_fixture();
        state.relics.push(relic);
        state.piles.hand = vec![CardInstance::new(CardId::new(1), SHRUG_IT_OFF_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(2), STRIKE_R_ID)];
        state
    }

    #[test]
    fn abacus_block_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = shuffle_trigger_state(Relic::TheAbacus);
        state.player.block = i32::MAX;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), POMMEL_STRIKE_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: Some(state.monsters[0].id),
                },
            ),
            Err(SimError::InvalidState(
                "combat player block or energy is negative"
            ))
        );
    }

    #[test]
    fn sundial_counter_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = shuffle_trigger_state(Relic::Sundial);
        state.relic_counters.sundial_shuffles = i32::MAX as u32;

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "combat relic counter is outside its stable range"
            ))
        );
    }

    #[test]
    fn sundial_energy_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = shuffle_trigger_state(Relic::Sundial);
        state.player.energy = i32::MAX;
        state.relic_counters.sundial_shuffles = crate::relic::SUNDIAL_THRESHOLD - 1;

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "combat player block or energy is negative"
            ))
        );
    }

    #[test]
    fn gremlin_horn_energy_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = i32::MAX;
        state.relics.push(Relic::GremlinHorn);
        state.monsters[0].hp = 1;
        let mut survivor = state.monsters[0].clone();
        survivor.id = MonsterId::new(2);
        survivor.hp = survivor.max_hp;
        state.monsters.push(survivor);
        state.piles.hand = vec![CardInstance::new(CardId::new(1), ANGER_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: Some(state.monsters[0].id),
                },
            ),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
    }

    #[test]
    fn juggernaut_death_hook_failure_propagates_to_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = i32::MAX;
        state.player.temp_rage_block = 1;
        state.player.powers.juggernaut = 1;
        state.relics.push(Relic::GremlinHorn);
        state.monsters = vec![
            monster_state(&FUNGI_BEAST_A0, MonsterId::new(1)),
            monster_state(&FUNGI_BEAST_A0, MonsterId::new(2)),
        ];
        for monster in &mut state.monsters {
            monster.hp = 1;
        }
        state.piles.hand = vec![CardInstance::new(CardId::new(1), ANGER_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
    }

    #[test]
    fn immediate_death_hook_failure_rolls_back_spore_cloud_and_relics() {
        let fungi_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.player.energy = i32::MAX;
        state.relics.push(Relic::GremlinHorn);
        state.monsters = vec![
            monster_state(&FUNGI_BEAST_A0, fungi_id),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].alive = false;
        state.monsters[0].hp = 0;
        let before = state.clone();

        assert_eq!(
            apply_monster_death_hooks(&mut state, fungi_id),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn spore_cloud_vulnerable_overflow_rolls_back_death_hooks() {
        let fungi_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.player.powers.vulnerable = i32::MAX;
        state.monsters = vec![
            monster_state(&FUNGI_BEAST_A0, fungi_id),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].alive = false;
        state.monsters[0].hp = 0;
        let before = state.clone();

        assert_eq!(
            apply_monster_death_hooks(&mut state, fungi_id),
            Err(SimError::InvalidState(
                "player Vulnerable application overflows i32"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn draw_failure_rolls_back_card_rng_fire_breathing_and_death_hooks() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = i32::MAX;
        state.player.powers.fire_breathing = 1;
        state.relics.push(Relic::GremlinHorn);
        state.monsters = vec![
            monster_state(&FUNGI_BEAST_A0, MonsterId::new(1)),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].hp = 1;
        state.piles.hand.clear();
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(1), BURN_ID)];
        let before = state.clone();

        assert_eq!(
            player_draw_cards(&mut state, 1),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn queued_monster_death_counter_overflow_fails_without_wrapping() {
        let mut state = CombatState::initial_fixture();
        let dead_id = state.monsters[0].id;
        let mut survivor = state.monsters[0].clone();
        survivor.id = MonsterId::new(2);
        state.monsters[0].alive = false;
        state.monsters.push(survivor);
        state.relics.push(Relic::GremlinHorn);
        state.pending_monster_death_relic_triggers = u32::MAX;

        assert_eq!(
            queue_monster_death_hooks(&mut state, dead_id),
            Err(SimError::InvalidState(
                "combat death trigger counter overflows u32"
            ))
        );
        assert_eq!(state.pending_monster_death_relic_triggers, u32::MAX);
    }

    #[test]
    fn hex_dazed_waits_for_armaments_hand_select_to_close() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.hex = 1;
        state.rng.card_random_rng = StsRng::new(7_141_693_325_691_831_207);
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

        assert!(next.hand_select().is_some());
        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .filter(|card| card.content_id == DAZED_ID)
                .count(),
            0
        );
        assert_eq!(next.pending_hand_select_action_count(), 1);

        choose_hand_select(&mut next, 0).expect("Strike is selectable");
        confirm_hand_select(&mut next).expect("Armaments selection should resolve");

        assert_eq!(next.pending_hand_select_action_count(), 0);
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
        state.rng.card_random_rng = StsRng::new(1_234);
        state.rng.shuffle_rng = StsRng::new(5_678);

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
        state.monsters[0].rolled_attack_damage = Some(8);
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 8 };
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

        apply_monster_death_hooks(&mut state, fungi_id).expect("death hooks resolve");

        assert_eq!(state.player.powers.vulnerable, 2);

        let last_fungi_id = MonsterId::new(3);
        let mut ending_state = CombatState::initial_fixture();
        ending_state.monsters = vec![monster_state(&FUNGI_BEAST_A0, last_fungi_id)];
        ending_state.monsters[0].alive = false;

        apply_monster_death_hooks(&mut ending_state, last_fungi_id)
            .expect("ending death hooks resolve");

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
        state.piles.draw_pile.clear();

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
    fn top_draw_double_tap_plus_grants_two_pending_attack_replays() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(1), DOUBLE_TAP_PLUS_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        apply_play_top_draw_card_to_state(&mut state, None).expect("top Double Tap+ plays");

        assert_eq!(state.double_tap_pending, 2);
        assert!(state.piles.draw_pile.is_empty());
        assert_eq!(state.piles.discard_pile.len(), 1);
        assert_eq!(state.piles.discard_pile[0].content_id, DOUBLE_TAP_PLUS_ID);
        assert!(state.piles.exhaust_pile.is_empty());
    }

    #[test]
    fn havoc_played_double_tap_plus_replays_the_following_attack() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 100;
        state.monsters[0].max_hp = 100;
        state.player.energy = 3;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_PLUS_ID),
            CardInstance::new(CardId::new(2), BASH_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(3), DOUBLE_TAP_PLUS_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let after_havoc = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ plays Double Tap+");
        assert_eq!(after_havoc.double_tap_pending, 2);

        let after_bash = apply_combat_action(
            &after_havoc,
            CombatAction::PlayCard {
                card_id: CardId::new(2),
                target: Some(target),
            },
        )
        .expect("Double Tap+ replays Bash");

        assert_eq!(after_bash.double_tap_pending, 1);
        assert_eq!(after_bash.monsters[0].hp, 80);
        assert_eq!(after_bash.monsters[0].powers.vulnerable, 4);
    }

    #[test]
    fn havoc_exhausts_unplayable_top_card_without_fabricating_an_effect() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), DOUBT_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ resolves an unplayable top card");

        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, DOUBT_ID);
        assert!(next.piles.draw_pile.is_empty());
    }

    #[test]
    fn havoc_exhausts_slimed_after_its_explicit_no_effect_play() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), SLIMED_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ resolves Slimed's explicit no-effect play");

        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, SLIMED_ID);
        assert!(next.piles.draw_pile.is_empty());
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
    fn havoc_played_burning_pact_resolves_selection_after_source_exhaust() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(1), DEFEND_R_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), BASH_ID),
            CardInstance::new(CardId::new(4), BURNING_PACT_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut staged = state;
        let queue = apply_play_top_draw_card(&mut staged, None, true, false)
            .expect("top-draw Burning Pact queue");
        let mut next = process_internal_queue(&staged, queue.into())
            .expect("top-draw Burning Pact opens exhaust selection")
            .state;
        assert!(next.exhaust_select().is_some());
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, BURNING_PACT_ID);

        choose_exhaust_select(&mut next, 0).expect("select Defend");
        confirm_exhaust_select(&mut next).expect("resolve Burning Pact selection");

        assert!(next.exhaust_select().is_none());
        assert_eq!(next.piles.hand.len(), 2);
        assert_eq!(next.piles.exhaust_pile.len(), 2);
        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .filter(|card| card.content_id == BURNING_PACT_ID)
                .count(),
            1
        );
    }

    #[test]
    fn havoc_played_dual_wield_resolves_without_duplicating_source() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), DUAL_WIELD_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut staged = state;
        let queue = apply_play_top_draw_card(&mut staged, None, true, false)
            .expect("top-draw Dual Wield queue");
        let mut next = process_internal_queue(&staged, queue.into())
            .expect("top-draw Dual Wield opens hand selection")
            .state;
        choose_hand_select(&mut next, 0).expect("select Strike");
        confirm_hand_select(&mut next).expect("resolve Dual Wield selection");

        assert_eq!(next.piles.hand.len(), 2);
        assert!(next
            .piles
            .hand
            .iter()
            .all(|card| card.content_id == STRIKE_R_ID));
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, DUAL_WIELD_ID);
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
        assert_eq!(next.discard_select().unwrap().source_card_id, None);
        assert_eq!(next.discard_select().unwrap().source_card, None);

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
        assert!(next.exhaust_select().is_some());
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
        state.rng.card_random_rng = StsRng::with_counter(22_079_335_132, 1);
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
                .exhaust_select()
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
