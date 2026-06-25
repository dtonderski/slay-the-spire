use super::card_effects;
use crate::{
    action::{CardPile, CombatAction, HpLossSource, InternalAction},
    card::CardType,
    combat::{
        apply_burning_blood,
        damage::{
            deal_damage_info_to_monster_with_result, deal_unmodified_damage_to_monster,
            reflect_spikes_to_player, DamageInfo, DamageSource,
        },
        validate_combat_action, CombatPhase, DiscardSelectPurpose, HandSelectPurpose,
    },
    content::cards::{
        get_card_definition, upgrade_content_id, ANGER_ID, ANGER_PLUS_ID, BASH_ID,
        BLOOD_FOR_BLOOD_ID, CLEAVE_ID, CLEAVE_PLUS_ID, DAZED_ID, DEFEND_R_ID, DRAMATIC_ENTRANCE_ID,
        EXHUME_ID, FINESSE_ID, FLASH_OF_STEEL_ID, OFFERING_ID, POMMEL_STRIKE_ID,
        POMMEL_STRIKE_PLUS_ID, POWER_THROUGH_ID, PUMMEL_ID, RECKLESS_CHARGE_ID, SEARING_BLOW_ID,
        SEARING_BLOW_PLUS_ID, SENTINEL_ID, SHRUG_IT_OFF_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID,
        TWIN_STRIKE_ID, TWIN_STRIKE_PLUS_ID, WOUND_ID,
    },
    content::monsters::{
        check_slime_boss_split, get_monster_definition, guardian_on_hp_damage,
        wake_lagavulin_on_damage,
    },
    ids::{CardId, ContentId, MonsterId},
    power::calculate_block,
    relic::Relic,
    rng::SimulatorRng,
    CardInstance, CombatState, MonsterState, SimError, SimResult,
};
use std::collections::VecDeque;

pub use super::card_effects::top_draw_card_definition;

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
        CombatAction::EndTurn => Ok(CombatTransition {
            state: crate::combat::end_player_turn(state),
            event_log: Vec::new(),
        }),
    }
}

pub fn apply_play_top_draw_card_action(
    state: &CombatState,
    target: Option<MonsterId>,
) -> SimResult<CombatState> {
    Ok(process_internal_queue(
        state,
        VecDeque::from([InternalAction::PlayTopDrawCard { target }]),
    )?
    .state)
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
            queue.push_back(follow_up);
        }
    }

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

fn apply_internal_action(
    state: &mut CombatState,
    action: InternalAction,
) -> SimResult<Vec<InternalAction>> {
    match action {
        InternalAction::ConsumeDuplicationPotion => {
            state.duplication_potion_pending = false;
            Ok(Vec::new())
        }
        InternalAction::ConsumeDoubleTap => {
            state.double_tap_pending = 0;
            Ok(Vec::new())
        }
        InternalAction::PlayCard { card_id } => {
            let card = find_hand_card(state, card_id)?;
            let definition = get_card_definition(card.content_id)
                .ok_or(SimError::UnknownContent(card.content_id))?;
            apply_enrage_on_card_type(state, definition.card_type);
            apply_rage_on_card_type(state, definition.card_type);
            Ok(crate::relic::apply_on_card_play_relics(
                state,
                definition.card_type,
            ))
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
        InternalAction::DealDamage { info } => {
            let player_powers = state.player.powers;
            let temp_strength = state.player.temp_strength;
            let relics = state.relics.clone();
            let (spikes, still_alive) = {
                let monster = living_monster_mut(state, info.target)?;
                let spikes = monster.powers.spikes;
                let damage = deal_damage_info_to_monster_with_result(
                    monster,
                    info,
                    player_powers,
                    temp_strength,
                    &relics,
                );
                if relics.contains(&crate::Relic::HandDrill) && damage.broke_block {
                    crate::relic::apply_monster_vulnerable_with_relics(
                        &mut monster.powers,
                        &relics,
                        crate::relic::HAND_DRILL_VULNERABLE,
                    );
                }
                wake_lagavulin_on_damage(monster, damage.hp_damage);
                guardian_on_hp_damage(monster, damage.hp_damage);
                (spikes, monster.alive)
            };
            check_slime_boss_split(state, info.target);
            if !still_alive {
                crate::relic::apply_monster_death_relics(state);
            }
            if still_alive && spikes > 0 {
                let hp_before = state.player.hp;
                reflect_spikes_to_player(&mut state.player, &state.relics, spikes);
                crate::combat::hp_loss::apply_player_hp_loss_hooks(
                    state,
                    hp_before - state.player.hp,
                );
            }
            Ok(Vec::new())
        }
        InternalAction::DealFeedDamage { info, max_hp_gain } => {
            let player_powers = state.player.powers;
            let temp_strength = state.player.temp_strength;
            let relics = state.relics.clone();
            let (spikes, still_alive) = {
                let monster = living_monster_mut(state, info.target)?;
                let spikes = monster.powers.spikes;
                let damage = deal_damage_info_to_monster_with_result(
                    monster,
                    info,
                    player_powers,
                    temp_strength,
                    &relics,
                );
                if relics.contains(&crate::Relic::HandDrill) && damage.broke_block {
                    crate::relic::apply_monster_vulnerable_with_relics(
                        &mut monster.powers,
                        &relics,
                        crate::relic::HAND_DRILL_VULNERABLE,
                    );
                }
                wake_lagavulin_on_damage(monster, damage.hp_damage);
                guardian_on_hp_damage(monster, damage.hp_damage);
                (spikes, monster.alive)
            };
            check_slime_boss_split(state, info.target);
            if !still_alive {
                state.player.max_hp += max_hp_gain;
                state.player.hp += max_hp_gain;
                crate::relic::apply_monster_death_relics(state);
            }
            if still_alive && spikes > 0 {
                let hp_before = state.player.hp;
                reflect_spikes_to_player(&mut state.player, &state.relics, spikes);
                crate::relic::apply_player_hp_loss_relics(state, hp_before - state.player.hp);
            }
            Ok(Vec::new())
        }
        InternalAction::DealDamageAll { source, amount } => {
            deal_attack_damage_to_all_living(state, source, amount)?;
            Ok(Vec::new())
        }
        InternalAction::DealDamageAllAndHealUnblocked { source, amount } => {
            let hp_damage = deal_attack_damage_to_all_living(state, source, amount)?;
            crate::relic::heal_player_in_combat_with_relics(
                &mut state.player.hp,
                state.player.max_hp,
                hp_damage,
                &state.relics,
            );
            Ok(Vec::new())
        }
        InternalAction::HealPlayer { amount } => {
            crate::relic::heal_player_in_combat_with_relics(
                &mut state.player.hp,
                state.player.max_hp,
                amount,
                &state.relics,
            );
            Ok(Vec::new())
        }
        InternalAction::GainBlock { amount } => {
            let gained = calculate_block(amount, state.player.powers);
            state.player.block += gained;
            Ok(juggernaut_follow_up_for_positive_block_gain(state, gained))
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
            let relics = state.relics.clone();
            if let Some(monster) = living_monster_mut_opt(state, target) {
                crate::relic::apply_monster_vulnerable_with_relics(
                    &mut monster.powers,
                    &relics,
                    amount,
                );
            }
            Ok(Vec::new())
        }
        InternalAction::ApplyPlayerVulnerable { amount } => {
            crate::power::apply_player_vulnerable(&mut state.player.powers, amount);
            Ok(Vec::new())
        }
        InternalAction::ApplyWeak { target, amount } => {
            if let Some(monster) = living_monster_mut_opt(state, target) {
                monster.powers.weak += amount;
            }
            Ok(Vec::new())
        }
        InternalAction::ReduceMonsterStrength { target, amount } => {
            if let Some(monster) = living_monster_mut_opt(state, target) {
                monster.powers.strength -= amount;
            }
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
        } => {
            add_generated_card_to_pile(state, content_id, to, temp_cost);
            Ok(Vec::new())
        }
        InternalAction::DrawCards { count } => {
            player_draw_cards(state, count);
            Ok(Vec::new())
        }
        InternalAction::GainEnergy { amount } => {
            state.player.energy += amount;
            Ok(Vec::new())
        }
        InternalAction::LoseHp { amount, source } => {
            let mitigated = crate::relic::mitigate_hp_loss(&state.relics, amount);
            let hp_loss =
                crate::relic::apply_buffer_to_hp_loss(&mut state.player.powers, mitigated);
            state.player.hp -= hp_loss;
            crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss);
            apply_rupture_after_hp_loss(state, source, hp_loss);
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
        InternalAction::GainCombust { amount } => {
            state.player.powers.combust += amount;
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
        InternalAction::GainTempStrength { amount } => {
            state.player.temp_strength += amount;
            Ok(Vec::new())
        }
        InternalAction::GainRitual { amount } => {
            state.player.powers.ritual += amount;
            Ok(Vec::new())
        }
        InternalAction::CardExhausted { card_id } => {
            apply_on_exhaust_effects(state, card_id);
            Ok(Vec::new())
        }
        InternalAction::PlayTopDrawCard { target } => apply_play_top_draw_card(state, target),
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
        InternalAction::AwaitDiscardSelect {
            source_card_id,
            purpose,
        } => {
            if purpose == DiscardSelectPurpose::HeadbuttPutOnDraw
                && state.monsters.iter().all(|monster| !monster.alive)
            {
                move_card(state, source_card_id, CardPile::Hand, CardPile::DiscardPile)?;
                return Ok(Vec::new());
            }
            state.discard_select = Some(crate::combat::DiscardSelectState {
                purpose,
                source_card_id: Some(source_card_id),
                selected_discard_index: None,
            });
            Ok(Vec::new())
        }
        InternalAction::AwaitExhaustSelect {
            source_card_id,
            purpose,
        } => {
            state.exhaust_select = Some(crate::combat::ExhaustSelectState {
                purpose,
                source_card_id: Some(source_card_id),
                selected_hand_indices: Vec::new(),
            });
            Ok(Vec::new())
        }
    }
}

fn apply_rupture_after_hp_loss(state: &mut CombatState, source: HpLossSource, actual_hp_loss: i32) {
    if actual_hp_loss <= 0 || !matches!(source, HpLossSource::Card(_)) {
        return;
    }

    state.player.powers.strength += state.player.powers.rupture;
}

fn deal_attack_damage_to_all_living(
    state: &mut CombatState,
    source: CardId,
    amount: i32,
) -> SimResult<i32> {
    let player_powers = state.player.powers;
    let temp_strength = state.player.temp_strength;
    let relics = state.relics.clone();
    let targets: Vec<(MonsterId, i32)> = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| (monster.id, monster.powers.spikes))
        .collect();
    let mut total_hp_damage = 0;

    for (target, spikes) in targets {
        let (hp_damage, still_alive) = {
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
            if relics.contains(&crate::Relic::HandDrill) && damage.broke_block {
                crate::relic::apply_monster_vulnerable_with_relics(
                    &mut monster.powers,
                    &relics,
                    crate::relic::HAND_DRILL_VULNERABLE,
                );
            }
            wake_lagavulin_on_damage(monster, damage.hp_damage);
            guardian_on_hp_damage(monster, damage.hp_damage);
            (damage.hp_damage, monster.alive)
        };
        total_hp_damage += hp_damage;
        check_slime_boss_split(state, target);
        if !still_alive {
            crate::relic::apply_monster_death_relics(state);
        }
        if still_alive && spikes > 0 {
            let hp_before = state.player.hp;
            reflect_spikes_to_player(&mut state.player, &state.relics, spikes);
            crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_before - state.player.hp);
        }
    }

    Ok(total_hp_damage)
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
        crate::relic::apply_monster_death_relics(state);
    }
    Ok(())
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

fn first_living_monster_id(state: &CombatState) -> Option<MonsterId> {
    state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .min_by_key(|monster| monster.id.get())
        .map(|monster| monster.id)
}

pub(crate) fn apply_on_exhaust_effects(state: &mut CombatState, card_id: CardId) {
    if exhausted_card_content_id(state, card_id) == Some(SENTINEL_ID) {
        state.player.energy += 2;
    }
    if state.player.powers.feel_no_pain > 0 {
        let gained = 3 * state.player.powers.feel_no_pain;
        state.player.block += gained;
        apply_juggernaut_after_direct_block_gain(state, gained);
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
                crate::relic::apply_monster_death_relics(state);
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
) {
    let next_id = CardId::new(state.piles.max_card_instance_id() + 1);
    let mut card = CardInstance {
        combat_only: true,
        ..CardInstance::new(next_id, content_id)
    };
    card.temp_cost = temp_cost;
    push_card_to_pile(state, card, to);
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
        if let Some(monster_def) = get_monster_definition(monster.content_id) {
            if monster_def.enrage_weak_on_skill > 0 {
                monster.powers.anger += monster_def.enrage_weak_on_skill;
            }
        }
    }
}

fn apply_rage_on_card_type(state: &mut CombatState, card_type: CardType) {
    if card_type == CardType::Attack && state.player.temp_rage_block > 0 {
        let gained = calculate_block(state.player.temp_rage_block, state.player.powers);
        state.player.block += gained;
        apply_juggernaut_after_direct_block_gain(state, gained);
    }
}

fn apply_play_top_draw_card(
    state: &mut CombatState,
    target: Option<MonsterId>,
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

    let mut follow_ups = Vec::new();

    match definition.id {
        STRIKE_R_ID
        | STRIKE_R_PLUS_ID
        | ANGER_ID
        | ANGER_PLUS_ID
        | POMMEL_STRIKE_ID
        | POMMEL_STRIKE_PLUS_ID
        | FLASH_OF_STEEL_ID
        | SEARING_BLOW_ID
        | SEARING_BLOW_PLUS_ID
        | BASH_ID
        | RECKLESS_CHARGE_ID => {
            let target = target.expect("validated havoc attack target");
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: definition.values.damage.unwrap_or(0),
                },
            });
            if definition.id == RECKLESS_CHARGE_ID {
                follow_ups.push(InternalAction::AddCardToPile {
                    content_id: DAZED_ID,
                    to: CardPile::DrawPile,
                });
            }
            if definition.id == FLASH_OF_STEEL_ID {
                follow_ups.push(InternalAction::DrawCards { count: 1 });
            }
        }
        PUMMEL_ID => {
            let target = target.expect("validated havoc attack target");
            let damage = definition.values.damage.unwrap_or(0);
            for _ in 0..4 {
                follow_ups.push(InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(card_id),
                        target,
                        amount: damage,
                    },
                });
            }
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
        DEFEND_R_ID => {
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
        }
        SHRUG_IT_OFF_ID => {
            follow_ups.push(InternalAction::GainBlock { amount: 8 });
            follow_ups.push(InternalAction::DrawCards { count: 1 });
        }
        FINESSE_ID => {
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
            follow_ups.push(InternalAction::DrawCards { count: 1 });
        }
        OFFERING_ID => {
            follow_ups.push(InternalAction::LoseHp {
                amount: 6,
                source: HpLossSource::Card(card_id),
            });
            follow_ups.push(InternalAction::GainEnergy { amount: 2 });
            follow_ups.push(InternalAction::DrawCards { count: 3 });
        }
        POWER_THROUGH_ID => {
            follow_ups.push(InternalAction::GainBlock { amount: 15 });
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

    state.piles.exhaust_pile.push(card);
    follow_ups.push(InternalAction::CardExhausted { card_id });

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
        HandSelectPurpose::WarcryPutOnDraw => true,
        HandSelectPurpose::ArmamentsUpgrade => upgrade_content_id(card.content_id).is_some(),
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
        HandSelectPurpose::ArmamentsUpgrade => {
            confirm_armaments_select(state, hand_select.source_card_id, index)
        }
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
    let warcry = remove_card_from_pile(state, source_card_id, CardPile::Hand)?;
    state.piles.exhaust_pile.push(warcry);
    apply_on_exhaust_effects(state, source_card_id);
    Ok(())
}

fn confirm_armaments_select(
    state: &mut CombatState,
    source_card_id: CardId,
    index: usize,
) -> SimResult<()> {
    let card = state
        .piles
        .hand
        .get_mut(index)
        .ok_or(SimError::IllegalAction("hand select index out of range"))?;
    if card.id == source_card_id {
        return Err(SimError::IllegalAction("cannot upgrade Armaments"));
    }
    let upgraded = upgrade_content_id(card.content_id)
        .ok_or(SimError::IllegalAction("selected card cannot be upgraded"))?;
    card.content_id = upgraded;
    move_card(state, source_card_id, CardPile::Hand, CardPile::DiscardPile)
}

pub fn open_discard_select(state: &mut CombatState) -> SimResult<()> {
    if state.piles.discard_pile.is_empty() {
        return Err(SimError::IllegalAction("discard pile is empty"));
    }
    state.discard_select = Some(crate::combat::DiscardSelectState {
        purpose: DiscardSelectPurpose::LiquidMemoriesReturnToHand,
        source_card_id: None,
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
    discard_select.selected_discard_index = Some(ui_index);
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
    let index = discard_select
        .selected_discard_index
        .ok_or(SimError::IllegalAction("discard select choice is required"))?;
    let mut card = state
        .piles
        .discard_pile
        .get(index)
        .copied()
        .ok_or(SimError::IllegalAction("discard select index out of range"))?;
    state.piles.discard_pile.remove(index);
    card.temp_cost = Some(0);
    state.piles.hand.push(card);
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
    move_card(state, source_card_id, CardPile::Hand, CardPile::DiscardPile)
}

pub fn open_exhaust_select(state: &mut CombatState) -> SimResult<()> {
    state.exhaust_select = Some(crate::combat::ExhaustSelectState {
        purpose: crate::combat::ExhaustSelectPurpose::Exhaust,
        source_card_id: None,
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
        selected_hand_indices: Vec::new(),
    });
    Ok(())
}

pub fn choose_exhaust_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    let pile_index = exhaust_select_ui_to_hand_index(state, ui_index)?;
    let exhaust_select = state
        .exhaust_select
        .as_mut()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::ExhumeReturnToHand {
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
        exhaust_select.selected_hand_indices.push(pile_index);
        exhaust_select.selected_hand_indices.sort_unstable();
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
    let mut selected = exhaust_select.selected_hand_indices;
    selected.sort_unstable();
    selected.dedup();
    for index in selected.into_iter().rev() {
        if index >= state.piles.hand.len() {
            return Err(SimError::IllegalAction("exhaust select index out of range"));
        }
        let card = state.piles.hand.remove(index);
        let card_id = card.id;
        state.piles.exhaust_pile.push(card);
        apply_on_exhaust_effects(state, card_id);
    }
    Ok(())
}

fn exhumable_ui_to_exhaust_index(state: &CombatState, ui_index: usize) -> SimResult<usize> {
    state
        .piles
        .exhaust_pile
        .iter()
        .enumerate()
        .filter(|(_, card)| card.content_id != EXHUME_ID)
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
    if card.content_id == EXHUME_ID {
        return Err(SimError::IllegalAction("Exhume cannot return Exhume"));
    }
    state.piles.exhaust_pile.remove(index);
    state.piles.hand.push(card);
    move_card(state, source_card_id, CardPile::Hand, CardPile::ExhaustPile)?;
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
    if card.content_id == BLOOD_FOR_BLOOD_ID {
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
            if !card.combat_only {
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

#[cfg(test)]
mod tests {
    use super::card_effects::infernal_blade_modeled_attack_pool;
    use super::*;
    use crate::content::cards::ARMAMENTS_ID;
    use crate::content::cards::{
        ANGER_ID, ANGER_PLUS_ID, BANDAGE_UP_ID, BARRICADE_ID, BASH_ID, BATTLE_TRANCE_ID,
        BATTLE_TRANCE_PLUS_ID, BERSERK_ID, BLOODLETTING_ID, BLOOD_FOR_BLOOD_ID, BLUDGEON_ID,
        BODY_SLAM_ID, BRUTALITY_ID, BURNING_PACT_ID, CARNAGE_ID, CLASH_ID, CLEAVE_ID,
        CLEAVE_PLUS_ID, CLOTHESLINE_ID, COMBUST_ID, CORRUPTION_ID, DARK_EMBRACE_ID, DEFEND_R_ID,
        DEMON_FORM_ID, DISARM_ID, DOUBLE_TAP_ID, DROPKICK_ID, DUAL_WIELD_ID, ENTRENCH_ID,
        EVOLVE_ID, EXHUME_ID, FEED_ID, FEEL_NO_PAIN_ID, FIEND_FIRE_ID, FINESSE_ID,
        FIRE_BREATHING_ID, FLAME_BARRIER_ID, FLASH_OF_STEEL_ID, FLEX_ID, FLEX_PLUS_ID,
        GHOSTLY_ARMOR_ID, GOOD_INSTINCTS_ID, HAVOC_ID, HEADBUTT_ID, HEAVY_BLADE_ID, HEMOKINESIS_ID,
        IMPERVIOUS_ID, INFERNAL_BLADE_ID, INFLAME_ID, INFLAME_PLUS_ID, INTIMIDATE_ID, IRON_WAVE_ID,
        JUGGERNAUT_ID, LIMIT_BREAK_ID, METALLICIZE_ID, OFFERING_ID, PERFECTED_STRIKE_ID,
        POMMEL_STRIKE_ID, POMMEL_STRIKE_PLUS_ID, POWER_THROUGH_ID, PUMMEL_ID, RAGE_ID, RAMPAGE_ID,
        REAPER_ID, RECKLESS_CHARGE_ID, REGRET_ID, RUPTURE_ID, SEARING_BLOW_ID, SECOND_WIND_ID,
        SEEING_RED_ID, SEEING_RED_PLUS_ID, SENTINEL_ID, SEVER_SOUL_ID, SHOCKWAVE_ID,
        SHRUG_IT_OFF_ID, SLIMED_ID, SPOT_WEAKNESS_ID, SPOT_WEAKNESS_PLUS_ID, STRIKE_R_ID,
        STRIKE_R_PLUS_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, TWIN_STRIKE_PLUS_ID, WARCRY_ID,
        WARCRY_PLUS_ID, WHIRLWIND_ID, WHIRLWIND_PLUS_ID, WILD_STRIKE_ID, WOUND_ID,
    };

    #[test]
    fn strike_decreases_monster_hp_by_six() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 6);
    }

    #[test]
    fn strike_decreases_energy_by_one() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
    }

    #[test]
    fn duplication_potion_pending_doubles_next_attack_without_extra_energy() {
        let mut state = CombatState::initial_fixture();
        state.duplication_potion_pending = true;

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 12);
        assert_eq!(next.player.energy, state.player.energy - 1);
        assert!(!next.duplication_potion_pending);
    }

    #[test]
    fn duplication_potion_pending_doubles_next_skill_block_once() {
        let mut state = CombatState::initial_fixture();
        state.duplication_potion_pending = true;

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(next.player.block, state.player.block + 10);
        assert_eq!(next.player.energy, state.player.energy - 1);
        assert!(!next.duplication_potion_pending);
    }

    #[test]
    fn strike_moves_card_from_hand_to_discard() {
        let state = CombatState::initial_fixture();
        let strike_id = hand_strike_id(&state);

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert!(!next.piles.hand.iter().any(|card| card.id == strike_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == strike_id));
    }

    #[test]
    fn strike_can_win_combat() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].hp = 6;

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.phase, CombatPhase::Won);
        assert!(!next.monsters[0].alive);
    }

    #[test]
    fn invalid_target_returns_error_and_preserves_state() {
        let state = CombatState::initial_fixture();
        let before = state.snapshot().hash().expect("state hashes before");

        let result = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: hand_strike_id(&state),
                target: Some(MonsterId::new(99)),
            },
        );

        assert_eq!(
            result,
            Err(SimError::IllegalAction("target is not a living monster"))
        );
        assert_eq!(state.snapshot().hash().expect("state hashes after"), before);
    }

    #[test]
    fn defend_increases_player_block_by_five() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(next.player.block, state.player.block + 5);
    }

    #[test]
    fn defend_decreases_energy_by_one() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
    }

    #[test]
    fn defend_moves_card_from_hand_to_discard() {
        let state = CombatState::initial_fixture();
        let defend_id = hand_card_id(&state, DEFEND_R_ID);

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert!(!next.piles.hand.iter().any(|card| card.id == defend_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == defend_id));
    }

    #[test]
    fn bash_decreases_monster_hp_by_eight() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 8);
    }

    #[test]
    fn bash_adds_two_vulnerable_to_monster() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");

        assert_eq!(next.monsters[0].powers.vulnerable, 2);
    }

    #[test]
    fn champion_belt_adds_weak_when_bash_applies_vulnerable() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![crate::Relic::ChampionBelt];

        let next = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");

        assert_eq!(next.monsters[0].powers.vulnerable, 2);
        assert_eq!(next.monsters[0].powers.weak, 1);
    }

    #[test]
    fn lethal_bash_does_not_fail_when_vulnerable_target_dies() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].hp = 2;

        let next = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");

        assert!(next.monsters[0].hp <= 0);
        assert!(!next.monsters[0].alive);
        assert_eq!(next.phase, CombatPhase::Won);
    }

    #[test]
    fn bash_decreases_energy_by_two_and_moves_to_discard() {
        let state = CombatState::initial_fixture();
        let bash_id = hand_card_id(&state, BASH_ID);

        let next = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");

        assert_eq!(next.player.energy, state.player.energy - 2);
        assert!(!next.piles.hand.iter().any(|card| card.id == bash_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == bash_id));
    }

    #[test]
    fn iron_wave_deals_five_damage_and_gains_five_block() {
        let state = hand_only(IRON_WAVE_ID);

        let next =
            apply_combat_action(&state, iron_wave_action(&state)).expect("Iron Wave applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 5);
        assert_eq!(next.player.block, state.player.block + 5);
        assert_eq!(next.player.energy, state.player.energy - 1);
    }

    #[test]
    fn iron_wave_moves_to_discard_after_play() {
        let state = hand_only(IRON_WAVE_ID);
        let iron_wave_id = hand_card_id(&state, IRON_WAVE_ID);

        let next =
            apply_combat_action(&state, iron_wave_action(&state)).expect("Iron Wave applies");

        assert!(!next.piles.hand.iter().any(|card| card.id == iron_wave_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == iron_wave_id));
    }

    #[test]
    fn iron_wave_event_log_records_damage_then_block() {
        let state = hand_only(IRON_WAVE_ID);
        let iron_wave_id = hand_card_id(&state, IRON_WAVE_ID);

        let transition = apply_combat_action_with_events(&state, iron_wave_action(&state))
            .expect("Iron Wave applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: iron_wave_id
                },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(iron_wave_id),
                        target: MonsterId::new(1),
                        amount: 5,
                    },
                },
                InternalAction::GainBlock { amount: 5 },
                InternalAction::MoveCard {
                    card_id: iron_wave_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn body_slam_deals_current_block_damage_without_consuming_block() {
        let mut state = hand_only(BODY_SLAM_ID);
        state.player.block = 11;

        let next =
            apply_combat_action(&state, body_slam_action(&state)).expect("Body Slam applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 11);
        assert_eq!(next.player.block, state.player.block);
        assert_eq!(next.player.energy, state.player.energy - 1);
    }

    #[test]
    fn body_slam_at_zero_block_deals_zero_damage() {
        let mut state = hand_only(BODY_SLAM_ID);
        state.player.block = 0;

        let next =
            apply_combat_action(&state, body_slam_action(&state)).expect("Body Slam applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp);
        assert_eq!(next.player.block, 0);
    }

    #[test]
    fn body_slam_moves_to_discard_after_play() {
        let state = hand_only(BODY_SLAM_ID);
        let body_slam_id = hand_card_id(&state, BODY_SLAM_ID);

        let next =
            apply_combat_action(&state, body_slam_action(&state)).expect("Body Slam applies");

        assert!(!next.piles.hand.iter().any(|card| card.id == body_slam_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == body_slam_id));
    }

    #[test]
    fn body_slam_event_log_records_current_block_damage() {
        let mut state = hand_only(BODY_SLAM_ID);
        state.player.block = 9;
        let body_slam_id = hand_card_id(&state, BODY_SLAM_ID);

        let transition = apply_combat_action_with_events(&state, body_slam_action(&state))
            .expect("Body Slam applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: body_slam_id
                },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(body_slam_id),
                        target: MonsterId::new(1),
                        amount: 9,
                    },
                },
                InternalAction::MoveCard {
                    card_id: body_slam_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn akabeko_adds_eight_damage_to_body_slam() {
        let mut state = hand_only(BODY_SLAM_ID);
        state.player.block = 4;
        state.relics.push(Relic::Akabeko);

        let next =
            apply_combat_action(&state, body_slam_action(&state)).expect("Body Slam applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 12);
    }

    #[test]
    fn flame_barrier_grants_block_and_temporary_thorns_then_discards() {
        let state = hand_only(FLAME_BARRIER_ID);
        let flame_barrier_id = hand_card_id(&state, FLAME_BARRIER_ID);

        let next = apply_combat_action(&state, flame_barrier_action(&state))
            .expect("Flame Barrier applies");

        assert_eq!(next.player.block, state.player.block + 12);
        assert_eq!(next.player.powers.thorns, state.player.powers.thorns);
        assert_eq!(next.player.temp_thorns, state.player.temp_thorns + 4);
        assert_eq!(next.player.energy, state.player.energy - 2);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == flame_barrier_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == flame_barrier_id));
    }

    #[test]
    fn flame_barrier_uses_existing_block_calculation() {
        let mut state = hand_only(FLAME_BARRIER_ID);
        state.player.powers.dexterity = 2;
        state.player.powers.frail = 1;

        let next = apply_combat_action(&state, flame_barrier_action(&state))
            .expect("Flame Barrier applies");

        assert_eq!(next.player.block, 10);
    }

    #[test]
    fn flame_barrier_event_log_records_block_then_temporary_thorns_then_discard() {
        let state = hand_only(FLAME_BARRIER_ID);
        let flame_barrier_id = hand_card_id(&state, FLAME_BARRIER_ID);

        let transition = apply_combat_action_with_events(&state, flame_barrier_action(&state))
            .expect("Flame Barrier applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: flame_barrier_id
                },
                InternalAction::SpendEnergy { amount: 2 },
                InternalAction::GainBlock { amount: 12 },
                InternalAction::GainTemporaryThorns { amount: 4 },
                InternalAction::MoveCard {
                    card_id: flame_barrier_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn flame_barrier_thorns_damage_attacking_monster_on_monster_turn() {
        let state = hand_only(FLAME_BARRIER_ID);
        let after_play = apply_combat_action(&state, flame_barrier_action(&state))
            .expect("Flame Barrier applies");

        let after_turn =
            apply_combat_action(&after_play, CombatAction::EndTurn).expect("monster turn applies");

        assert_eq!(after_turn.monsters[0].hp, after_play.monsters[0].hp - 4);
    }

    #[test]
    fn flame_barrier_temporary_thorns_clear_after_monster_turn() {
        let state = hand_only(FLAME_BARRIER_ID);
        let after_play = apply_combat_action(&state, flame_barrier_action(&state))
            .expect("Flame Barrier applies");

        let after_turn =
            apply_combat_action(&after_play, CombatAction::EndTurn).expect("monster turn applies");

        assert_eq!(after_play.player.temp_thorns, 4);
        assert_eq!(after_turn.player.temp_thorns, 0);
        assert_eq!(after_turn.player.powers.thorns, 0);
    }

    #[test]
    fn flame_barrier_temporary_thorns_stack_with_persistent_thorns_for_reflection() {
        let mut state = hand_only(FLAME_BARRIER_ID);
        state.player.powers.thorns = 3;

        let after_play = apply_combat_action(&state, flame_barrier_action(&state))
            .expect("Flame Barrier applies");
        let after_turn =
            apply_combat_action(&after_play, CombatAction::EndTurn).expect("monster turn applies");

        assert_eq!(after_turn.monsters[0].hp, after_play.monsters[0].hp - 7);
        assert_eq!(after_turn.player.powers.thorns, 3);
        assert_eq!(after_turn.player.temp_thorns, 0);
    }

    #[test]
    fn clash_deals_fourteen_damage_without_spending_energy() {
        let state = hand_only(CLASH_ID);

        let next = apply_combat_action(&state, clash_action(&state)).expect("Clash applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 14);
        assert_eq!(next.player.energy, state.player.energy);
    }

    #[test]
    fn clash_moves_to_discard_after_play() {
        let state = hand_only(CLASH_ID);
        let clash_id = hand_card_id(&state, CLASH_ID);

        let next = apply_combat_action(&state, clash_action(&state)).expect("Clash applies");

        assert!(!next.piles.hand.iter().any(|card| card.id == clash_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == clash_id));
    }

    #[test]
    fn clash_event_log_records_zero_energy_attack() {
        let state = hand_only(CLASH_ID);
        let clash_id = hand_card_id(&state, CLASH_ID);

        let transition =
            apply_combat_action_with_events(&state, clash_action(&state)).expect("Clash applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard { card_id: clash_id },
                InternalAction::SpendEnergy { amount: 0 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(clash_id),
                        target: MonsterId::new(1),
                        amount: 14,
                    },
                },
                InternalAction::MoveCard {
                    card_id: clash_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn akabeko_adds_eight_damage_to_clash() {
        let mut state = hand_only(CLASH_ID);
        state.relics.push(Relic::Akabeko);

        let next = apply_combat_action(&state, clash_action(&state)).expect("Clash applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 22);
    }

    #[test]
    fn wild_strike_deals_twelve_damage_spends_one_and_adds_wound_to_draw() {
        let mut state = hand_only(WILD_STRIKE_ID);
        state.piles.draw_pile.clear();

        let next =
            apply_combat_action(&state, wild_strike_action(&state)).expect("Wild Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 12);
        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.piles.draw_pile.len(), 1);
        assert_eq!(next.piles.draw_pile[0].content_id, WOUND_ID);
    }

    #[test]
    fn wild_strike_moves_to_discard_after_play() {
        let state = hand_only(WILD_STRIKE_ID);
        let wild_strike_id = hand_card_id(&state, WILD_STRIKE_ID);

        let next =
            apply_combat_action(&state, wild_strike_action(&state)).expect("Wild Strike applies");

        assert!(!next.piles.hand.iter().any(|card| card.id == wild_strike_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == wild_strike_id));
    }

    #[test]
    fn wild_strike_appends_wound_to_draw_pile_locally() {
        let mut state = hand_only(WILD_STRIKE_ID);
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next =
            apply_combat_action(&state, wild_strike_action(&state)).expect("Wild Strike applies");

        assert_eq!(next.piles.draw_pile.len(), 2);
        assert_eq!(next.piles.draw_pile[0].content_id, STRIKE_R_ID);
        assert_eq!(next.piles.draw_pile[1].content_id, WOUND_ID);
    }

    #[test]
    fn wild_strike_applies_strength_normally() {
        let mut state = hand_only(WILD_STRIKE_ID);
        state.player.powers.strength = 2;

        let next =
            apply_combat_action(&state, wild_strike_action(&state)).expect("Wild Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 14);
    }

    #[test]
    fn akabeko_adds_eight_damage_to_wild_strike() {
        let mut state = hand_only(WILD_STRIKE_ID);
        state.relics.push(Relic::Akabeko);

        let next =
            apply_combat_action(&state, wild_strike_action(&state)).expect("Wild Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 20);
    }

    #[test]
    fn wild_strike_event_log_records_damage_wound_and_pile_move() {
        let state = hand_only(WILD_STRIKE_ID);
        let wild_strike_id = hand_card_id(&state, WILD_STRIKE_ID);

        let transition = apply_combat_action_with_events(&state, wild_strike_action(&state))
            .expect("Wild Strike applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: wild_strike_id
                },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(wild_strike_id),
                        target: MonsterId::new(1),
                        amount: 12,
                    },
                },
                InternalAction::AddCardToPile {
                    content_id: WOUND_ID,
                    to: CardPile::DrawPile,
                },
                InternalAction::MoveCard {
                    card_id: wild_strike_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn heavy_blade_deals_fourteen_damage_and_spends_two_energy() {
        let state = hand_only(HEAVY_BLADE_ID);

        let next =
            apply_combat_action(&state, heavy_blade_action(&state)).expect("Heavy Blade applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 14);
        assert_eq!(next.player.energy, state.player.energy - 2);
    }

    #[test]
    fn heavy_blade_applies_strength_three_times() {
        let mut state = hand_only(HEAVY_BLADE_ID);
        state.player.powers.strength = 2;

        let next =
            apply_combat_action(&state, heavy_blade_action(&state)).expect("Heavy Blade applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 20);
    }

    #[test]
    fn heavy_blade_applies_temp_strength_three_times() {
        let mut state = hand_only(HEAVY_BLADE_ID);
        state.player.temp_strength = 3;

        let next =
            apply_combat_action(&state, heavy_blade_action(&state)).expect("Heavy Blade applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 23);
    }

    #[test]
    fn akabeko_adds_eight_damage_to_heavy_blade_after_extra_strength() {
        let mut state = hand_only(HEAVY_BLADE_ID);
        state.player.powers.strength = 2;
        state.relics.push(Relic::Akabeko);

        let next =
            apply_combat_action(&state, heavy_blade_action(&state)).expect("Heavy Blade applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 28);
    }

    #[test]
    fn heavy_blade_moves_to_discard_after_play() {
        let state = hand_only(HEAVY_BLADE_ID);
        let heavy_blade_id = hand_card_id(&state, HEAVY_BLADE_ID);

        let next =
            apply_combat_action(&state, heavy_blade_action(&state)).expect("Heavy Blade applies");

        assert!(!next.piles.hand.iter().any(|card| card.id == heavy_blade_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == heavy_blade_id));
    }

    #[test]
    fn heavy_blade_event_log_records_extra_strength_queue_amount() {
        let mut state = hand_only(HEAVY_BLADE_ID);
        state.player.powers.strength = 2;
        let heavy_blade_id = hand_card_id(&state, HEAVY_BLADE_ID);

        let transition = apply_combat_action_with_events(&state, heavy_blade_action(&state))
            .expect("Heavy Blade applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: heavy_blade_id
                },
                InternalAction::SpendEnergy { amount: 2 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(heavy_blade_id),
                        target: MonsterId::new(1),
                        amount: 18,
                    },
                },
                InternalAction::MoveCard {
                    card_id: heavy_blade_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn perfected_strike_counts_current_combat_pile_strike_named_cards() {
        let mut state = hand_only(PERFECTED_STRIKE_ID);
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(21), STRIKE_R_ID));
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), STRIKE_R_ID),
        ];
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(40), STRIKE_R_ID)];
        state.piles.exhaust_pile = vec![CardInstance::new(CardId::new(50), STRIKE_R_ID)];

        let next = apply_combat_action(&state, perfected_strike_action(&state))
            .expect("Perfected Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 18);
    }

    #[test]
    fn perfected_strike_counts_upgraded_strike_names() {
        let mut state = hand_only(PERFECTED_STRIKE_ID);
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_PLUS_ID)];

        let next = apply_combat_action(&state, perfected_strike_action(&state))
            .expect("Perfected Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 10);
    }

    #[test]
    fn perfected_strike_ignores_non_strike_named_cards() {
        let mut state = hand_only(PERFECTED_STRIKE_ID);
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), DEFEND_R_ID),
            CardInstance::new(CardId::new(31), BASH_ID),
        ];
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(40), SHRUG_IT_OFF_ID)];

        let next = apply_combat_action(&state, perfected_strike_action(&state))
            .expect("Perfected Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 8);
    }

    #[test]
    fn perfected_strike_applies_strength_normally() {
        let mut state = hand_only(PERFECTED_STRIKE_ID);
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];
        state.player.powers.strength = 2;

        let next = apply_combat_action(&state, perfected_strike_action(&state))
            .expect("Perfected Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 12);
    }

    #[test]
    fn akabeko_adds_eight_damage_to_perfected_strike() {
        let mut state = hand_only(PERFECTED_STRIKE_ID);
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];
        state.relics.push(crate::Relic::Akabeko);

        let next = apply_combat_action(&state, perfected_strike_action(&state))
            .expect("Perfected Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 18);
    }

    #[test]
    fn perfected_strike_moves_to_discard_after_play() {
        let state = hand_only(PERFECTED_STRIKE_ID);
        let perfected_strike_id = hand_card_id(&state, PERFECTED_STRIKE_ID);

        let next = apply_combat_action(&state, perfected_strike_action(&state))
            .expect("Perfected Strike applies");

        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == perfected_strike_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == perfected_strike_id));
    }

    #[test]
    fn perfected_strike_event_log_records_combat_pile_count_damage() {
        let mut state = hand_only(PERFECTED_STRIKE_ID);
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];
        let perfected_strike_id = hand_card_id(&state, PERFECTED_STRIKE_ID);

        let transition = apply_combat_action_with_events(&state, perfected_strike_action(&state))
            .expect("Perfected Strike applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: perfected_strike_id
                },
                InternalAction::SpendEnergy { amount: 2 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(perfected_strike_id),
                        target: MonsterId::new(1),
                        amount: 10,
                    },
                },
                InternalAction::MoveCard {
                    card_id: perfected_strike_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn rampage_first_play_deals_eight_gains_bonus_and_moves_to_discard() {
        let state = hand_only(RAMPAGE_ID);
        let rampage_id = hand_card_id(&state, RAMPAGE_ID);

        let next = apply_combat_action(&state, rampage_action(&state)).expect("Rampage applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 8);
        assert_eq!(next.player.energy, state.player.energy - 1);
        assert!(!next.piles.hand.iter().any(|card| card.id == rampage_id));
        let discarded = next
            .piles
            .discard_pile
            .iter()
            .find(|card| card.id == rampage_id)
            .expect("Rampage moved to discard");
        assert_eq!(discarded.rampage_damage_bonus, 5);
    }

    #[test]
    fn rampage_later_play_uses_accumulated_card_instance_bonus() {
        let state = hand_only(RAMPAGE_ID);
        let after_first =
            apply_combat_action(&state, rampage_action(&state)).expect("Rampage applies");
        let mut replay = after_first.clone();
        replay.piles.hand = vec![replay.piles.discard_pile.remove(0)];
        replay.player.energy = 3;

        let after_second =
            apply_combat_action(&replay, rampage_action(&replay)).expect("Rampage applies again");

        assert_eq!(after_second.monsters[0].hp, after_first.monsters[0].hp - 13);
        assert_eq!(after_second.piles.discard_pile[0].rampage_damage_bonus, 10);
    }

    #[test]
    fn rampage_bonus_is_scoped_to_the_played_card_instance() {
        let mut state = hand_only(RAMPAGE_ID);
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(21), RAMPAGE_ID));

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Rampage applies");

        assert_eq!(next.piles.discard_pile[0].id, CardId::new(20));
        assert_eq!(next.piles.discard_pile[0].rampage_damage_bonus, 5);
        assert_eq!(next.piles.hand[0].id, CardId::new(21));
        assert_eq!(next.piles.hand[0].rampage_damage_bonus, 0);
    }

    #[test]
    fn rampage_card_instance_bonus_round_trips_through_combat_state_json() {
        let state = hand_only(RAMPAGE_ID);
        let next = apply_combat_action(&state, rampage_action(&state)).expect("Rampage applies");

        let restored: CombatState =
            serde_json::from_str(&serde_json::to_string(&next).expect("serialize combat"))
                .expect("deserialize combat");

        assert_eq!(restored.piles.discard_pile[0].rampage_damage_bonus, 5);
    }

    #[test]
    fn rampage_event_log_records_damage_bonus_before_discard() {
        let state = hand_only(RAMPAGE_ID);
        let rampage_id = hand_card_id(&state, RAMPAGE_ID);

        let transition = apply_combat_action_with_events(&state, rampage_action(&state))
            .expect("Rampage applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: rampage_id
                },
                InternalAction::SpendCardEnergy {
                    card_id: rampage_id,
                },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(rampage_id),
                        target: MonsterId::new(1),
                        amount: 8,
                    },
                },
                InternalAction::IncreaseRampageDamage {
                    card_id: rampage_id,
                    amount: 5,
                },
                InternalAction::MoveCard {
                    card_id: rampage_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn rampage_uses_generic_strength_and_vulnerable_damage_resolution() {
        let mut state = hand_only(RAMPAGE_ID);
        state.player.powers.strength = 2;
        state.monsters[0].powers.vulnerable = 1;

        let next = apply_combat_action(&state, rampage_action(&state)).expect("Rampage applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 15);
    }

    #[test]
    fn rampage_uses_effective_card_cost() {
        let mut state = hand_only(RAMPAGE_ID);
        state.player.energy = 0;
        state.piles.hand[0].temp_cost = Some(0);

        let next = apply_combat_action(&state, rampage_action(&state))
            .expect("Rampage applies with temp cost");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.piles.discard_pile[0].rampage_damage_bonus, 5);
    }

    #[test]
    fn rampage_queue_accepts_akabeko_and_pen_nib_damage_modifiers() {
        let mut state = hand_only(RAMPAGE_ID);
        state.relics = vec![Relic::Akabeko, Relic::PenNib];
        state.relic_counters.pen_nib_attacks_played = 9;

        let transition = apply_combat_action_with_events(&state, rampage_action(&state))
            .expect("Rampage applies");

        assert!(transition.event_log.contains(&InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(CardId::new(20)),
                target: MonsterId::new(1),
                amount: 32,
            },
        }));
    }

    #[test]
    fn power_through_gains_fifteen_block_spends_one_and_moves_to_discard() {
        let state = hand_only(POWER_THROUGH_ID);
        let power_through_id = hand_card_id(&state, POWER_THROUGH_ID);

        let next = apply_combat_action(&state, power_through_action(&state))
            .expect("Power Through applies");

        assert_eq!(next.player.block, state.player.block + 15);
        assert_eq!(next.player.energy, state.player.energy - 1);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == power_through_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == power_through_id));
    }

    #[test]
    fn power_through_with_dexterity_gains_extra_block() {
        let mut state = hand_only(POWER_THROUGH_ID);
        state.player.powers.dexterity = 2;

        let next = apply_combat_action(&state, power_through_action(&state))
            .expect("Power Through applies");

        assert_eq!(next.player.block, state.player.block + 17);
    }

    #[test]
    fn power_through_with_frail_gains_reduced_block() {
        let mut state = hand_only(POWER_THROUGH_ID);
        state.player.powers.frail = 1;

        let next = apply_combat_action(&state, power_through_action(&state))
            .expect("Power Through applies");

        assert_eq!(next.player.block, state.player.block + 11);
    }

    #[test]
    fn power_through_adds_two_wounds_to_hand_with_deterministic_ids() {
        let state = hand_only(POWER_THROUGH_ID);
        let first_generated_id = CardId::new(state.piles.max_card_instance_id() + 1);

        let next = apply_combat_action(&state, power_through_action(&state))
            .expect("Power Through applies");

        let wounds = next
            .piles
            .hand
            .iter()
            .filter(|card| card.content_id == WOUND_ID)
            .map(|card| (card.id, card.content_id))
            .collect::<Vec<_>>();

        assert_eq!(
            wounds,
            vec![
                (first_generated_id, WOUND_ID),
                (CardId::new(first_generated_id.get() + 1), WOUND_ID),
            ]
        );
    }

    #[test]
    fn duplication_potion_duplicates_power_through_block_and_wounds() {
        let mut state = hand_only(POWER_THROUGH_ID);
        state.duplication_potion_pending = true;

        let next = apply_combat_action(&state, power_through_action(&state))
            .expect("Power Through applies");

        assert_eq!(next.player.block, state.player.block + 30);
        assert_eq!(
            next.piles
                .hand
                .iter()
                .filter(|card| card.content_id == WOUND_ID)
                .count(),
            4
        );
        assert!(!next.duplication_potion_pending);
    }

    #[test]
    fn power_through_event_log_records_block_wounds_and_pile_move() {
        let state = hand_only(POWER_THROUGH_ID);
        let power_through_id = hand_card_id(&state, POWER_THROUGH_ID);

        let transition = apply_combat_action_with_events(&state, power_through_action(&state))
            .expect("Power Through applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: power_through_id
                },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::GainBlock { amount: 15 },
                InternalAction::AddCardToPile {
                    content_id: WOUND_ID,
                    to: CardPile::Hand,
                },
                InternalAction::AddCardToPile {
                    content_id: WOUND_ID,
                    to: CardPile::Hand,
                },
                InternalAction::MoveCard {
                    card_id: power_through_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn infernal_blade_adds_zero_cost_combat_only_attack_to_hand_and_discards_source() {
        let mut state = hand_only(INFERNAL_BLADE_ID);
        state.card_random_rng = Some(crate::rng::StsRng::new(123));
        let mut expected_rng = crate::rng::StsRng::new(123);
        let expected_pool = infernal_blade_modeled_attack_pool();
        let expected =
            expected_pool[expected_rng.random_int((expected_pool.len() - 1) as i32) as usize];

        let next = apply_combat_action(&state, infernal_blade_action(&state))
            .expect("Infernal Blade applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(
            next.card_random_rng.as_ref().expect("card rng").counter(),
            expected_rng.counter()
        );
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == INFERNAL_BLADE_ID));
        let generated = next
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == expected)
            .expect("generated attack");
        assert!(generated.combat_only);
        assert_eq!(generated.temp_cost, Some(0));
        assert_eq!(
            get_card_definition(generated.content_id).map(|definition| definition.card_type),
            Some(CardType::Attack)
        );
    }

    #[test]
    fn infernal_blade_without_card_random_rng_uses_deterministic_modeled_fallback() {
        let state = hand_only(INFERNAL_BLADE_ID);
        let expected = infernal_blade_modeled_attack_pool()[0];

        let next = apply_combat_action(&state, infernal_blade_action(&state))
            .expect("Infernal Blade applies");

        assert!(next.card_random_rng.is_none());
        let generated = next
            .piles
            .hand
            .iter()
            .find(|card| card.combat_only)
            .expect("generated attack");
        assert_eq!(generated.content_id, expected);
        assert_eq!(generated.temp_cost, Some(0));
    }

    #[test]
    fn infernal_blade_event_log_records_generation_before_source_discard() {
        let state = hand_only(INFERNAL_BLADE_ID);
        let card_id = hand_card_id(&state, INFERNAL_BLADE_ID);
        let expected = infernal_blade_modeled_attack_pool()[0];

        let transition = apply_combat_action_with_events(&state, infernal_blade_action(&state))
            .expect("Infernal Blade applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard { card_id },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::AddGeneratedCardToPile {
                    content_id: expected,
                    to: CardPile::Hand,
                    temp_cost: Some(0),
                },
                InternalAction::MoveCard {
                    card_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn entrench_doubles_nonzero_block_spends_two_and_moves_to_discard() {
        let mut state = hand_only(ENTRENCH_ID);
        state.player.block = 7;
        let entrench_id = hand_card_id(&state, ENTRENCH_ID);

        let next = apply_combat_action(&state, entrench_action(&state)).expect("Entrench applies");

        assert_eq!(next.player.block, 14);
        assert_eq!(next.player.energy, state.player.energy - 2);
        assert!(!next.piles.hand.iter().any(|card| card.id == entrench_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == entrench_id));
    }

    #[test]
    fn entrench_with_zero_block_stays_zero() {
        let state = hand_only(ENTRENCH_ID);

        let next = apply_combat_action(&state, entrench_action(&state)).expect("Entrench applies");

        assert_eq!(next.player.block, 0);
    }

    #[test]
    fn entrench_ignores_dexterity_and_frail_when_doubling() {
        let mut state = hand_only(ENTRENCH_ID);
        state.player.block = 8;
        state.player.powers.dexterity = 3;
        state.player.powers.frail = 1;

        let next = apply_combat_action(&state, entrench_action(&state)).expect("Entrench applies");

        assert_eq!(next.player.block, 16);
    }

    #[test]
    fn entrench_uses_effective_temp_cost() {
        let mut state = hand_only(ENTRENCH_ID);
        state.player.block = 4;
        state.piles.hand[0].temp_cost = Some(1);

        let next = apply_combat_action(&state, entrench_action(&state)).expect("Entrench applies");

        assert_eq!(next.player.block, 8);
        assert_eq!(next.player.energy, state.player.energy - 1);
    }

    #[test]
    fn entrench_event_log_records_dynamic_block_double_and_pile_move() {
        let mut state = hand_only(ENTRENCH_ID);
        state.player.block = 6;
        let entrench_id = hand_card_id(&state, ENTRENCH_ID);

        let transition = apply_combat_action_with_events(&state, entrench_action(&state))
            .expect("Entrench applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: entrench_id
                },
                InternalAction::SpendCardEnergy {
                    card_id: entrench_id
                },
                InternalAction::DoublePlayerBlock,
                InternalAction::MoveCard {
                    card_id: entrench_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn duplication_potion_duplicates_entrench_block_doubling_sequentially() {
        let mut state = hand_only(ENTRENCH_ID);
        state.player.block = 7;
        state.duplication_potion_pending = true;

        let next = apply_combat_action(&state, entrench_action(&state)).expect("Entrench applies");

        assert_eq!(next.player.block, 28);
        assert_eq!(next.player.energy, state.player.energy - 2);
        assert!(!next.duplication_potion_pending);
    }

    #[test]
    fn ghostly_armor_gains_ten_block_spends_one_and_moves_to_discard() {
        let state = hand_only(GHOSTLY_ARMOR_ID);
        let ghostly_armor_id = hand_card_id(&state, GHOSTLY_ARMOR_ID);

        let next = apply_combat_action(&state, ghostly_armor_action(&state))
            .expect("Ghostly Armor applies");

        assert_eq!(next.player.block, state.player.block + 10);
        assert_eq!(next.player.energy, state.player.energy - 1);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == ghostly_armor_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == ghostly_armor_id));
    }

    #[test]
    fn ghostly_armor_with_dexterity_gains_extra_block() {
        let mut state = hand_only(GHOSTLY_ARMOR_ID);
        state.player.powers.dexterity = 2;

        let next = apply_combat_action(&state, ghostly_armor_action(&state))
            .expect("Ghostly Armor applies");

        assert_eq!(next.player.block, state.player.block + 12);
    }

    #[test]
    fn ghostly_armor_with_frail_gains_reduced_block() {
        let mut state = hand_only(GHOSTLY_ARMOR_ID);
        state.player.powers.frail = 1;

        let next = apply_combat_action(&state, ghostly_armor_action(&state))
            .expect("Ghostly Armor applies");

        assert_eq!(next.player.block, state.player.block + 7);
    }

    #[test]
    fn played_ghostly_armor_moves_to_discard_not_exhaust() {
        let state = hand_only(GHOSTLY_ARMOR_ID);
        let ghostly_armor_id = hand_card_id(&state, GHOSTLY_ARMOR_ID);

        let next = apply_combat_action(&state, ghostly_armor_action(&state))
            .expect("Ghostly Armor applies");

        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == ghostly_armor_id));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == ghostly_armor_id));
    }

    #[test]
    fn unplayed_ghostly_armor_exhausts_at_end_of_turn() {
        let mut state = hand_only(GHOSTLY_ARMOR_ID);
        state.piles.draw_pile.clear();

        let next = apply_combat_action(&state, CombatAction::EndTurn).expect("end turn applies");

        assert!(next.piles.hand.is_empty());
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == GHOSTLY_ARMOR_ID));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == GHOSTLY_ARMOR_ID));
    }

    #[test]
    fn ghostly_armor_event_log_records_generic_block_skill_queue() {
        let state = hand_only(GHOSTLY_ARMOR_ID);
        let ghostly_armor_id = hand_card_id(&state, GHOSTLY_ARMOR_ID);

        let transition = apply_combat_action_with_events(&state, ghostly_armor_action(&state))
            .expect("Ghostly Armor applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: ghostly_armor_id
                },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::GainBlock { amount: 10 },
                InternalAction::MoveCard {
                    card_id: ghostly_armor_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn reckless_charge_deals_seven_damage_spends_zero_and_adds_dazed_to_draw() {
        let mut state = hand_only(RECKLESS_CHARGE_ID);
        state.player.energy = 0;
        state.piles.draw_pile.clear();

        let next = apply_combat_action(&state, reckless_charge_action(&state))
            .expect("Reckless Charge applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 7);
        assert_eq!(next.player.energy, 0);
        assert_eq!(next.piles.draw_pile.len(), 1);
        assert_eq!(next.piles.draw_pile[0].content_id, DAZED_ID);
    }

    #[test]
    fn reckless_charge_moves_to_discard_after_play() {
        let state = hand_only(RECKLESS_CHARGE_ID);
        let reckless_charge_id = hand_card_id(&state, RECKLESS_CHARGE_ID);

        let next = apply_combat_action(&state, reckless_charge_action(&state))
            .expect("Reckless Charge applies");

        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == reckless_charge_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == reckless_charge_id));
    }

    #[test]
    fn reckless_charge_appends_dazed_to_draw_pile_with_deterministic_id() {
        let mut state = hand_only(RECKLESS_CHARGE_ID);
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];
        let first_generated_id = CardId::new(state.piles.max_card_instance_id() + 1);

        let next = apply_combat_action(&state, reckless_charge_action(&state))
            .expect("Reckless Charge applies");

        assert_eq!(next.piles.draw_pile.len(), 2);
        assert_eq!(next.piles.draw_pile[0].content_id, STRIKE_R_ID);
        assert_eq!(
            (
                next.piles.draw_pile[1].id,
                next.piles.draw_pile[1].content_id
            ),
            (first_generated_id, DAZED_ID)
        );
    }

    #[test]
    fn reckless_charge_applies_strength_normally() {
        let mut state = hand_only(RECKLESS_CHARGE_ID);
        state.player.powers.strength = 2;

        let next = apply_combat_action(&state, reckless_charge_action(&state))
            .expect("Reckless Charge applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 9);
    }

    #[test]
    fn akabeko_adds_eight_damage_to_reckless_charge() {
        let mut state = hand_only(RECKLESS_CHARGE_ID);
        state.relics.push(Relic::Akabeko);

        let next = apply_combat_action(&state, reckless_charge_action(&state))
            .expect("Reckless Charge applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 15);
    }

    #[test]
    fn reckless_charge_event_log_records_damage_dazed_and_pile_move() {
        let state = hand_only(RECKLESS_CHARGE_ID);
        let reckless_charge_id = hand_card_id(&state, RECKLESS_CHARGE_ID);

        let transition = apply_combat_action_with_events(&state, reckless_charge_action(&state))
            .expect("Reckless Charge applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: reckless_charge_id
                },
                InternalAction::SpendEnergy { amount: 0 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(reckless_charge_id),
                        target: MonsterId::new(1),
                        amount: 7,
                    },
                },
                InternalAction::AddCardToPile {
                    content_id: DAZED_ID,
                    to: CardPile::DrawPile,
                },
                InternalAction::MoveCard {
                    card_id: reckless_charge_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn pummel_deals_four_hits_spends_one_and_exhausts() {
        let state = hand_only(PUMMEL_ID);

        let next = apply_combat_action(&state, pummel_action(&state)).expect("Pummel applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 8);
        assert_eq!(next.player.energy, state.player.energy - 1);
        assert!(next.piles.hand.is_empty());
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, PUMMEL_ID);
    }

    #[test]
    fn pummel_event_log_records_four_damage_hits_in_order() {
        let state = hand_only(PUMMEL_ID);
        let pummel_id = hand_card_id(&state, PUMMEL_ID);

        let transition =
            apply_combat_action_with_events(&state, pummel_action(&state)).expect("Pummel applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard { card_id: pummel_id },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(pummel_id),
                        target: MonsterId::new(1),
                        amount: 2,
                    },
                },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(pummel_id),
                        target: MonsterId::new(1),
                        amount: 2,
                    },
                },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(pummel_id),
                        target: MonsterId::new(1),
                        amount: 2,
                    },
                },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(pummel_id),
                        target: MonsterId::new(1),
                        amount: 2,
                    },
                },
                InternalAction::MoveCard {
                    card_id: pummel_id,
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::CardExhausted { card_id: pummel_id },
            ]
        );
    }

    #[test]
    fn pummel_applies_strength_to_each_hit() {
        let mut state = hand_only(PUMMEL_ID);
        state.player.powers.strength = 2;

        let next = apply_combat_action(&state, pummel_action(&state)).expect("Pummel applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 16);
    }

    #[test]
    fn pummel_applies_vulnerable_to_each_hit() {
        let mut state = hand_only(PUMMEL_ID);
        state.monsters[0].powers.vulnerable = 1;

        let next = apply_combat_action(&state, pummel_action(&state)).expect("Pummel applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 12);
    }

    #[test]
    fn akabeko_bonus_applies_to_each_hit_of_pummel() {
        let mut state = hand_only(PUMMEL_ID);
        state.relics.push(Relic::Akabeko);

        let next = apply_combat_action(&state, pummel_action(&state)).expect("Pummel applies");

        assert_eq!(next.monsters[0].hp, 0);
        assert_eq!(next.relic_counters.attacks_played_this_combat, 1);
    }

    #[test]
    fn pen_nib_bonus_applies_to_each_hit_of_pummel() {
        let mut state = hand_only(PUMMEL_ID);
        state.relics.push(Relic::PenNib);
        state.relic_counters.pen_nib_attacks_played = 9;

        let next = apply_combat_action(&state, pummel_action(&state)).expect("Pummel applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 16);
        assert_eq!(next.relic_counters.pen_nib_attacks_played, 0);
    }

    #[test]
    fn strange_spoon_can_move_played_pummel_to_discard() {
        let mut state = hand_only(PUMMEL_ID);
        state.relics = vec![Relic::StrangeSpoon];
        state.card_random_rng = Some(crate::rng::StsRng::new(123));
        let mut expected_rng = crate::rng::StsRng::new(123);
        let spoon_proc = expected_rng.random_bool();

        let next = apply_combat_action(&state, pummel_action(&state)).expect("Pummel applies");

        assert_eq!(
            next.card_random_rng.as_ref().expect("card rng").counter(),
            expected_rng.counter()
        );
        if spoon_proc {
            assert!(next.piles.exhaust_pile.is_empty());
            assert_eq!(next.piles.discard_pile[0].content_id, PUMMEL_ID);
        } else {
            assert!(next.piles.discard_pile.is_empty());
            assert_eq!(next.piles.exhaust_pile[0].content_id, PUMMEL_ID);
        }
    }

    #[test]
    fn havoc_plays_top_pummel_for_four_hits_and_exhausts_it() {
        let mut state = hand_only(HAVOC_ID);
        state.piles.hand = vec![CardInstance::new(CardId::new(20), HAVOC_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), PUMMEL_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Havoc applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 8);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == PUMMEL_ID));
    }

    #[test]
    fn bludgeon_deals_thirty_two_damage_and_spends_three_energy() {
        let state = hand_only(BLUDGEON_ID);

        let next = apply_combat_action(&state, bludgeon_action(&state)).expect("Bludgeon applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 32);
        assert_eq!(next.player.energy, state.player.energy - 3);
    }

    #[test]
    fn bludgeon_applies_strength_normally() {
        let mut state = hand_only(BLUDGEON_ID);
        state.player.powers.strength = 2;

        let next = apply_combat_action(&state, bludgeon_action(&state)).expect("Bludgeon applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 34);
    }

    #[test]
    fn akabeko_adds_eight_damage_to_bludgeon() {
        let mut state = hand_only(BLUDGEON_ID);
        state.relics.push(Relic::Akabeko);

        let next = apply_combat_action(&state, bludgeon_action(&state)).expect("Bludgeon applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 40);
    }

    #[test]
    fn bludgeon_moves_to_discard_after_play() {
        let state = hand_only(BLUDGEON_ID);
        let bludgeon_id = hand_card_id(&state, BLUDGEON_ID);

        let next = apply_combat_action(&state, bludgeon_action(&state)).expect("Bludgeon applies");

        assert!(!next.piles.hand.iter().any(|card| card.id == bludgeon_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == bludgeon_id));
    }

    #[test]
    fn bludgeon_event_log_records_generic_attack_queue() {
        let state = hand_only(BLUDGEON_ID);
        let bludgeon_id = hand_card_id(&state, BLUDGEON_ID);

        let transition = apply_combat_action_with_events(&state, bludgeon_action(&state))
            .expect("Bludgeon applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: bludgeon_id
                },
                InternalAction::SpendEnergy { amount: 3 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(bludgeon_id),
                        target: MonsterId::new(1),
                        amount: 32,
                    },
                },
                InternalAction::MoveCard {
                    card_id: bludgeon_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn feed_deals_ten_spends_one_and_exhausts_without_max_hp_on_nonlethal_hit() {
        let mut state = hand_only(FEED_ID);
        state.player.hp = 40;
        state.player.max_hp = 70;
        state.monsters[0].hp = 20;
        let feed_id = hand_card_id(&state, FEED_ID);

        let next = apply_combat_action(&state, feed_action(&state)).expect("Feed applies");

        assert_eq!(next.monsters[0].hp, 10);
        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.player.hp, 40);
        assert_eq!(next.player.max_hp, 70);
        assert!(!next.piles.hand.iter().any(|card| card.id == feed_id));
        assert_eq!(next.piles.exhaust_pile[0].id, feed_id);
    }

    #[test]
    fn feed_fatal_damage_increases_current_and_max_hp_by_three() {
        let mut state = two_monster_hand(FEED_ID);
        state.player.hp = 40;
        state.player.max_hp = 70;
        state.monsters[0].hp = 10;

        let next = apply_combat_action(&state, feed_action(&state)).expect("Feed applies");

        assert_eq!(next.player.max_hp, 73);
        assert_eq!(next.player.hp, 43);
        assert!(!next.monsters[0].alive);
        assert_eq!(next.phase, CombatPhase::WaitingForPlayer);
    }

    #[test]
    fn feed_does_not_gain_max_hp_when_block_prevents_fatal_hp_damage() {
        let mut state = hand_only(FEED_ID);
        state.player.hp = 40;
        state.player.max_hp = 70;
        state.monsters[0].hp = 10;
        state.monsters[0].block = 10;

        let next = apply_combat_action(&state, feed_action(&state)).expect("Feed applies");

        assert_eq!(next.monsters[0].hp, 10);
        assert!(next.monsters[0].alive);
        assert_eq!(next.player.max_hp, 70);
        assert_eq!(next.player.hp, 40);
    }

    #[test]
    fn strength_can_make_feed_fatal_for_max_hp_gain() {
        let mut state = two_monster_hand(FEED_ID);
        state.player.hp = 40;
        state.player.max_hp = 70;
        state.player.powers.strength = 5;
        state.monsters[0].hp = 15;

        let next = apply_combat_action(&state, feed_action(&state)).expect("Feed applies");

        assert_eq!(next.player.max_hp, 73);
        assert_eq!(next.player.hp, 43);
    }

    #[test]
    fn akabeko_and_pen_nib_modify_feed_damage_before_fatal_check() {
        let mut state = two_monster_hand(FEED_ID);
        state.player.hp = 40;
        state.player.max_hp = 70;
        state.monsters[0].hp = 36;
        state.relics = vec![Relic::Akabeko, Relic::PenNib];
        state.relic_counters.pen_nib_attacks_played = 9;

        let next = apply_combat_action(&state, feed_action(&state)).expect("Feed applies");

        assert_eq!(next.player.max_hp, 73);
        assert_eq!(next.player.hp, 43);
        assert!(!next.monsters[0].alive);
    }

    #[test]
    fn feed_fatal_damage_still_triggers_monster_death_relics() {
        let mut state = two_monster_hand(FEED_ID);
        state.player.hp = 40;
        state.player.max_hp = 70;
        state.monsters[0].hp = 10;
        state.relics = vec![Relic::GremlinHorn];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), DEFEND_R_ID)];

        let next = apply_combat_action(&state, feed_action(&state)).expect("Feed applies");

        assert_eq!(next.player.max_hp, 73);
        assert_eq!(next.player.hp, 43);
        assert_eq!(
            next.player.energy,
            state.player.energy - 1 + crate::relic::GREMLIN_HORN_ENERGY
        );
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == DEFEND_R_ID));
    }

    #[test]
    fn feed_event_log_records_damage_bonus_action_then_exhaust() {
        let state = hand_only(FEED_ID);
        let feed_id = hand_card_id(&state, FEED_ID);

        let transition =
            apply_combat_action_with_events(&state, feed_action(&state)).expect("Feed applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard { card_id: feed_id },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::DealFeedDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(feed_id),
                        target: MonsterId::new(1),
                        amount: 10,
                    },
                    max_hp_gain: 3,
                },
                InternalAction::MoveCard {
                    card_id: feed_id,
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::CardExhausted { card_id: feed_id },
            ]
        );
    }

    #[test]
    fn carnage_deals_twenty_spends_two_and_moves_to_discard() {
        let state = hand_only(CARNAGE_ID);
        let carnage_id = hand_card_id(&state, CARNAGE_ID);

        let next = apply_combat_action(&state, carnage_action(&state)).expect("Carnage applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 20);
        assert_eq!(next.player.energy, state.player.energy - 2);
        assert!(!next.piles.hand.iter().any(|card| card.id == carnage_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == carnage_id));
    }

    #[test]
    fn carnage_applies_strength_normally() {
        let mut state = hand_only(CARNAGE_ID);
        state.player.powers.strength = 2;

        let next = apply_combat_action(&state, carnage_action(&state)).expect("Carnage applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 22);
    }

    #[test]
    fn akabeko_adds_eight_damage_to_carnage() {
        let mut state = hand_only(CARNAGE_ID);
        state.relics.push(Relic::Akabeko);

        let next = apply_combat_action(&state, carnage_action(&state)).expect("Carnage applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 28);
    }

    #[test]
    fn pen_nib_doubles_carnage_damage() {
        let mut state = hand_only(CARNAGE_ID);
        state.relics.push(Relic::PenNib);
        state.relic_counters.pen_nib_attacks_played = 9;

        let next = apply_combat_action(&state, carnage_action(&state)).expect("Carnage applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 40);
        assert_eq!(next.relic_counters.pen_nib_attacks_played, 0);
    }

    #[test]
    fn played_carnage_moves_to_discard_not_exhaust() {
        let state = hand_only(CARNAGE_ID);
        let carnage_id = hand_card_id(&state, CARNAGE_ID);

        let next = apply_combat_action(&state, carnage_action(&state)).expect("Carnage applies");

        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == carnage_id));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == carnage_id));
    }

    #[test]
    fn unplayed_carnage_exhausts_at_end_of_turn() {
        let mut state = hand_only(CARNAGE_ID);
        state.piles.draw_pile.clear();

        let next = apply_combat_action(&state, CombatAction::EndTurn).expect("end turn applies");

        assert!(next.piles.hand.is_empty());
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == CARNAGE_ID));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == CARNAGE_ID));
    }

    #[test]
    fn runic_pyramid_still_exhausts_unplayed_carnage() {
        let mut state = hand_only(CARNAGE_ID);
        state.relics.push(Relic::RunicPyramid);
        state.piles.draw_pile.clear();

        let next = apply_combat_action(&state, CombatAction::EndTurn).expect("end turn applies");

        assert!(next.piles.hand.is_empty());
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == CARNAGE_ID));
    }

    #[test]
    fn carnage_event_log_records_generic_attack_queue() {
        let state = hand_only(CARNAGE_ID);
        let carnage_id = hand_card_id(&state, CARNAGE_ID);

        let transition = apply_combat_action_with_events(&state, carnage_action(&state))
            .expect("Carnage applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: carnage_id
                },
                InternalAction::SpendEnergy { amount: 2 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(carnage_id),
                        target: MonsterId::new(1),
                        amount: 20,
                    },
                },
                InternalAction::MoveCard {
                    card_id: carnage_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn hemokinesis_loses_two_hp_deals_fifteen_spends_one_and_moves_to_discard() {
        let state = hand_only(HEMOKINESIS_ID);
        let hemokinesis_id = hand_card_id(&state, HEMOKINESIS_ID);

        let next =
            apply_combat_action(&state, hemokinesis_action(&state)).expect("Hemokinesis applies");

        assert_eq!(next.player.hp, state.player.hp - 2);
        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 15);
        assert_eq!(next.player.energy, state.player.energy - 1);
        assert!(!next.piles.hand.iter().any(|card| card.id == hemokinesis_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == hemokinesis_id));
    }

    #[test]
    fn hemokinesis_applies_strength_normally() {
        let mut state = hand_only(HEMOKINESIS_ID);
        state.player.powers.strength = 2;

        let next =
            apply_combat_action(&state, hemokinesis_action(&state)).expect("Hemokinesis applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 17);
        assert_eq!(next.player.hp, state.player.hp - 2);
    }

    #[test]
    fn akabeko_adds_eight_damage_to_hemokinesis() {
        let mut state = hand_only(HEMOKINESIS_ID);
        state.relics.push(Relic::Akabeko);

        let next =
            apply_combat_action(&state, hemokinesis_action(&state)).expect("Hemokinesis applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 23);
        assert_eq!(next.player.hp, state.player.hp - 2);
    }

    #[test]
    fn pen_nib_doubles_hemokinesis_damage_not_hp_loss() {
        let mut state = hand_only(HEMOKINESIS_ID);
        state.relics.push(Relic::PenNib);
        state.relic_counters.pen_nib_attacks_played = 9;

        let next =
            apply_combat_action(&state, hemokinesis_action(&state)).expect("Hemokinesis applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 30);
        assert_eq!(next.player.hp, state.player.hp - 2);
        assert_eq!(next.relic_counters.pen_nib_attacks_played, 0);
    }

    #[test]
    fn duplication_potion_duplicates_hemokinesis_damage_and_hp_loss() {
        let mut state = hand_only(HEMOKINESIS_ID);
        state.duplication_potion_pending = true;

        let next =
            apply_combat_action(&state, hemokinesis_action(&state)).expect("Hemokinesis applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 30);
        assert_eq!(next.player.hp, state.player.hp - 4);
        assert_eq!(next.player.energy, state.player.energy - 1);
        assert!(!next.duplication_potion_pending);
    }

    #[test]
    fn hemokinesis_hp_loss_is_reduced_by_tungsten_rod() {
        let mut state = hand_only(HEMOKINESIS_ID);
        state.relics.push(Relic::TungstenRod);

        let next =
            apply_combat_action(&state, hemokinesis_action(&state)).expect("Hemokinesis applies");

        assert_eq!(next.player.hp, state.player.hp - 1);
        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 15);
    }

    #[test]
    fn hemokinesis_hp_loss_consumes_buffer_without_losing_hp() {
        let mut state = hand_only(HEMOKINESIS_ID);
        state.player.powers.buffer = 1;

        let next =
            apply_combat_action(&state, hemokinesis_action(&state)).expect("Hemokinesis applies");

        assert_eq!(next.player.hp, state.player.hp);
        assert_eq!(next.player.powers.buffer, 0);
        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 15);
    }

    #[test]
    fn hemokinesis_event_log_records_hp_loss_before_damage() {
        let state = hand_only(HEMOKINESIS_ID);
        let hemokinesis_id = hand_card_id(&state, HEMOKINESIS_ID);

        let transition = apply_combat_action_with_events(&state, hemokinesis_action(&state))
            .expect("Hemokinesis applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: hemokinesis_id
                },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::LoseHp {
                    amount: 2,
                    source: HpLossSource::Card(hemokinesis_id),
                },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(hemokinesis_id),
                        target: MonsterId::new(1),
                        amount: 15,
                    },
                },
                InternalAction::MoveCard {
                    card_id: hemokinesis_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn blood_for_blood_deals_eighteen_spends_dynamic_cost_and_moves_to_discard() {
        let mut state = hand_only(BLOOD_FOR_BLOOD_ID);
        state.player.energy = 3;
        state.piles.hand[0].blood_for_blood_cost_reduction = 1;
        let blood_for_blood_id = hand_card_id(&state, BLOOD_FOR_BLOOD_ID);

        let next = apply_combat_action(&state, blood_for_blood_action(&state))
            .expect("Blood for Blood applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 18);
        assert_eq!(next.player.energy, 0);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == blood_for_blood_id));
        assert_eq!(next.piles.discard_pile[0].id, blood_for_blood_id);
        assert_eq!(next.piles.discard_pile[0].blood_for_blood_cost_reduction, 1);
    }

    #[test]
    fn player_hp_loss_reduces_blood_for_blood_cost_in_all_combat_piles_once() {
        let mut state = hand_only(HEMOKINESIS_ID);
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(21), BLOOD_FOR_BLOOD_ID));
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), BLOOD_FOR_BLOOD_ID)];
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(31), BLOOD_FOR_BLOOD_ID)];
        state.piles.exhaust_pile = vec![CardInstance::new(CardId::new(32), BLOOD_FOR_BLOOD_ID)];

        let next =
            apply_combat_action(&state, hemokinesis_action(&state)).expect("Hemokinesis applies");

        assert_eq!(next.piles.hand[0].blood_for_blood_cost_reduction, 1);
        assert_eq!(next.piles.draw_pile[0].blood_for_blood_cost_reduction, 1);
        assert_eq!(next.piles.discard_pile[0].blood_for_blood_cost_reduction, 1);
        assert_eq!(next.piles.exhaust_pile[0].blood_for_blood_cost_reduction, 1);
    }

    #[test]
    fn prevented_hp_loss_does_not_reduce_blood_for_blood_cost() {
        let mut state = hand_only(HEMOKINESIS_ID);
        state.player.powers.buffer = 1;
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(21), BLOOD_FOR_BLOOD_ID));

        let next =
            apply_combat_action(&state, hemokinesis_action(&state)).expect("Hemokinesis applies");

        assert_eq!(next.player.hp, state.player.hp);
        assert_eq!(next.piles.hand[0].blood_for_blood_cost_reduction, 0);
    }

    #[test]
    fn blood_for_blood_cost_reduction_round_trips_through_combat_state_json() {
        let mut state = hand_only(HEMOKINESIS_ID);
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(21), BLOOD_FOR_BLOOD_ID));
        let next =
            apply_combat_action(&state, hemokinesis_action(&state)).expect("Hemokinesis applies");

        let restored: CombatState =
            serde_json::from_str(&serde_json::to_string(&next).expect("serialize combat"))
                .expect("deserialize combat");

        assert_eq!(restored.piles.hand[0].blood_for_blood_cost_reduction, 1);
    }

    #[test]
    fn blood_for_blood_event_log_records_dynamic_spend_before_damage() {
        let mut state = hand_only(BLOOD_FOR_BLOOD_ID);
        state.player.energy = 3;
        state.piles.hand[0].blood_for_blood_cost_reduction = 1;
        let blood_for_blood_id = hand_card_id(&state, BLOOD_FOR_BLOOD_ID);

        let transition = apply_combat_action_with_events(&state, blood_for_blood_action(&state))
            .expect("Blood for Blood applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: blood_for_blood_id
                },
                InternalAction::SpendCardEnergy {
                    card_id: blood_for_blood_id
                },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(blood_for_blood_id),
                        target: MonsterId::new(1),
                        amount: 18,
                    },
                },
                InternalAction::MoveCard {
                    card_id: blood_for_blood_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn dropkick_without_vulnerable_deals_five_spends_one_and_moves_to_discard() {
        let mut state = hand_only(DROPKICK_ID);
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];
        let dropkick_id = hand_card_id(&state, DROPKICK_ID);

        let next = apply_combat_action(&state, dropkick_action(&state)).expect("Dropkick applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 5);
        assert_eq!(next.player.energy, state.player.energy - 1);
        assert!(next
            .piles
            .draw_pile
            .iter()
            .any(|card| card.id == CardId::new(30)));
        assert!(!next.piles.hand.iter().any(|card| card.id == dropkick_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == dropkick_id));
        assert!(next.piles.exhaust_pile.is_empty());
    }

    #[test]
    fn dropkick_against_vulnerable_gains_one_energy_and_draws_one() {
        let mut state = hand_only(DROPKICK_ID);
        state.monsters[0].powers.vulnerable = 1;
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next = apply_combat_action(&state, dropkick_action(&state)).expect("Dropkick applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 7);
        assert_eq!(next.player.energy, state.player.energy);
        assert!(next.piles.draw_pile.is_empty());
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(30)));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == DROPKICK_ID));
    }

    #[test]
    fn dropkick_vulnerable_bonus_uses_pre_damage_target_state_even_when_lethal() {
        let mut state = hand_only(DROPKICK_ID);
        state.monsters[0].hp = 7;
        state.monsters[0].powers.vulnerable = 1;
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), DEFEND_R_ID)];

        let next = apply_combat_action(&state, dropkick_action(&state)).expect("Dropkick applies");

        assert_eq!(next.phase, CombatPhase::Won);
        assert_eq!(next.player.energy, state.player.energy);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(30)));
    }

    #[test]
    fn dropkick_with_strength_and_vulnerable_applies_existing_attack_damage_order() {
        let mut state = hand_only(DROPKICK_ID);
        state.player.powers.strength = 2;
        state.monsters[0].powers.vulnerable = 1;
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next = apply_combat_action(&state, dropkick_action(&state)).expect("Dropkick applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 10);
        assert_eq!(next.player.energy, state.player.energy);
    }

    #[test]
    fn dropkick_event_log_records_damage_bonus_then_discard_when_target_was_vulnerable() {
        let mut state = hand_only(DROPKICK_ID);
        state.monsters[0].powers.vulnerable = 1;
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];
        let dropkick_id = hand_card_id(&state, DROPKICK_ID);

        let transition = apply_combat_action_with_events(&state, dropkick_action(&state))
            .expect("Dropkick applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: dropkick_id
                },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(dropkick_id),
                        target: MonsterId::new(1),
                        amount: 5,
                    },
                },
                InternalAction::GainEnergy { amount: 1 },
                InternalAction::DrawCards { count: 1 },
                InternalAction::MoveCard {
                    card_id: dropkick_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn impervious_gains_thirty_block_spends_two_and_exhausts() {
        let state = hand_only(IMPERVIOUS_ID);

        let next =
            apply_combat_action(&state, impervious_action(&state)).expect("Impervious applies");

        assert_eq!(next.player.block, state.player.block + 30);
        assert_eq!(next.player.energy, state.player.energy - 2);
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, IMPERVIOUS_ID);
    }

    #[test]
    fn impervious_with_dexterity_gains_extra_block() {
        let mut state = hand_only(IMPERVIOUS_ID);
        state.player.powers.dexterity = 2;

        let next =
            apply_combat_action(&state, impervious_action(&state)).expect("Impervious applies");

        assert_eq!(next.player.block, state.player.block + 32);
    }

    #[test]
    fn impervious_with_frail_gains_reduced_block() {
        let mut state = hand_only(IMPERVIOUS_ID);
        state.player.powers.frail = 1;

        let next =
            apply_combat_action(&state, impervious_action(&state)).expect("Impervious applies");

        assert_eq!(next.player.block, state.player.block + 22);
    }

    #[test]
    fn impervious_moves_to_exhaust_after_play() {
        let state = hand_only(IMPERVIOUS_ID);
        let impervious_id = hand_card_id(&state, IMPERVIOUS_ID);

        let next =
            apply_combat_action(&state, impervious_action(&state)).expect("Impervious applies");

        assert!(!next.piles.hand.iter().any(|card| card.id == impervious_id));
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == impervious_id));
    }

    #[test]
    fn impervious_event_log_records_generic_exhaust_skill_queue() {
        let state = hand_only(IMPERVIOUS_ID);
        let impervious_id = hand_card_id(&state, IMPERVIOUS_ID);

        let transition = apply_combat_action_with_events(&state, impervious_action(&state))
            .expect("Impervious applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: impervious_id
                },
                InternalAction::SpendEnergy { amount: 2 },
                InternalAction::GainBlock { amount: 30 },
                InternalAction::MoveCard {
                    card_id: impervious_id,
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::CardExhausted {
                    card_id: impervious_id,
                },
            ]
        );
    }

    #[test]
    fn clothesline_deals_twelve_damage_and_applies_two_weak() {
        let state = hand_only(CLOTHESLINE_ID);

        let next =
            apply_combat_action(&state, clothesline_action(&state)).expect("Clothesline applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 12);
        assert_eq!(next.monsters[0].powers.weak, 2);
        assert_eq!(next.player.energy, state.player.energy - 2);
    }

    #[test]
    fn clothesline_moves_to_discard_after_play() {
        let state = hand_only(CLOTHESLINE_ID);
        let clothesline_id = hand_card_id(&state, CLOTHESLINE_ID);

        let next =
            apply_combat_action(&state, clothesline_action(&state)).expect("Clothesline applies");

        assert!(!next.piles.hand.iter().any(|card| card.id == clothesline_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == clothesline_id));
    }

    #[test]
    fn clothesline_event_log_records_damage_then_weak() {
        let state = hand_only(CLOTHESLINE_ID);
        let clothesline_id = hand_card_id(&state, CLOTHESLINE_ID);

        let transition = apply_combat_action_with_events(&state, clothesline_action(&state))
            .expect("Clothesline applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: clothesline_id
                },
                InternalAction::SpendEnergy { amount: 2 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(clothesline_id),
                        target: MonsterId::new(1),
                        amount: 12,
                    },
                },
                InternalAction::ApplyWeak {
                    target: MonsterId::new(1),
                    amount: 2,
                },
                InternalAction::MoveCard {
                    card_id: clothesline_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn intimidate_applies_one_weak_to_each_living_enemy_and_exhausts() {
        let state = two_monster_hand(INTIMIDATE_ID);

        let next =
            apply_combat_action(&state, intimidate_action(&state)).expect("Intimidate applies");

        assert_eq!(next.player.energy, state.player.energy);
        assert_eq!(next.monsters[0].powers.weak, 1);
        assert_eq!(next.monsters[1].powers.weak, 1);
        assert!(next.piles.discard_pile.is_empty());
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, INTIMIDATE_ID);
    }

    #[test]
    fn intimidate_skips_dead_enemies() {
        let mut state = two_monster_hand(INTIMIDATE_ID);
        state.monsters[1].alive = false;

        let next =
            apply_combat_action(&state, intimidate_action(&state)).expect("Intimidate applies");

        assert_eq!(next.monsters[0].powers.weak, 1);
        assert_eq!(next.monsters[1].powers.weak, 0);
    }

    #[test]
    fn intimidate_event_log_records_weak_applications_then_exhaust() {
        let state = two_monster_hand(INTIMIDATE_ID);
        let intimidate_id = hand_card_id(&state, INTIMIDATE_ID);

        let transition = apply_combat_action_with_events(&state, intimidate_action(&state))
            .expect("Intimidate applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: intimidate_id
                },
                InternalAction::SpendEnergy { amount: 0 },
                InternalAction::ApplyWeak {
                    target: MonsterId::new(1),
                    amount: 1,
                },
                InternalAction::ApplyWeak {
                    target: MonsterId::new(2),
                    amount: 1,
                },
                InternalAction::MoveCard {
                    card_id: intimidate_id,
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::CardExhausted {
                    card_id: intimidate_id,
                },
            ]
        );
    }

    #[test]
    fn shockwave_applies_three_debuffs_to_each_living_enemy_and_exhausts() {
        let state = two_monster_hand(SHOCKWAVE_ID);

        let next =
            apply_combat_action(&state, shockwave_action(&state)).expect("Shockwave applies");

        assert_eq!(next.player.energy, state.player.energy - 2);
        assert_eq!(next.monsters[0].powers.weak, 3);
        assert_eq!(next.monsters[0].powers.vulnerable, 3);
        assert_eq!(next.monsters[0].powers.strength, -3);
        assert_eq!(next.monsters[1].powers.weak, 3);
        assert_eq!(next.monsters[1].powers.vulnerable, 3);
        assert_eq!(next.monsters[1].powers.strength, -3);
        assert!(next.piles.discard_pile.is_empty());
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, SHOCKWAVE_ID);
    }

    #[test]
    fn shockwave_skips_dead_enemies() {
        let mut state = two_monster_hand(SHOCKWAVE_ID);
        state.monsters[1].alive = false;

        let next =
            apply_combat_action(&state, shockwave_action(&state)).expect("Shockwave applies");

        assert_eq!(next.monsters[0].powers.weak, 3);
        assert_eq!(next.monsters[0].powers.vulnerable, 3);
        assert_eq!(next.monsters[0].powers.strength, -3);
        assert_eq!(next.monsters[1].powers.weak, 0);
        assert_eq!(next.monsters[1].powers.vulnerable, 0);
        assert_eq!(next.monsters[1].powers.strength, 0);
    }

    #[test]
    fn shockwave_champion_belt_adds_weak_from_vulnerable_applications() {
        let mut state = two_monster_hand(SHOCKWAVE_ID);
        state.relics.push(Relic::ChampionBelt);

        let next =
            apply_combat_action(&state, shockwave_action(&state)).expect("Shockwave applies");

        assert_eq!(
            next.monsters[0].powers.weak,
            3 + crate::relic::CHAMPION_BELT_WEAK
        );
        assert_eq!(
            next.monsters[1].powers.weak,
            3 + crate::relic::CHAMPION_BELT_WEAK
        );
        assert_eq!(next.monsters[0].powers.vulnerable, 3);
        assert_eq!(next.monsters[1].powers.vulnerable, 3);
    }

    #[test]
    fn shockwave_reduces_monster_outgoing_attack_damage() {
        let mut state = hand_only(SHOCKWAVE_ID);
        state.monsters[0].powers.strength = 4;

        let next =
            apply_combat_action(&state, shockwave_action(&state)).expect("Shockwave applies");

        assert_eq!(next.monsters[0].powers.strength, 1);
        assert_eq!(next.monsters[0].powers.weak, 3);
        assert_eq!(
            crate::combat::turn_powers::monster_attack_damage(&next.monsters[0], 6),
            5
        );
    }

    #[test]
    fn shockwave_event_log_records_all_debuffs_then_exhaust() {
        let state = two_monster_hand(SHOCKWAVE_ID);
        let shockwave_id = hand_card_id(&state, SHOCKWAVE_ID);

        let transition = apply_combat_action_with_events(&state, shockwave_action(&state))
            .expect("Shockwave applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: shockwave_id
                },
                InternalAction::SpendEnergy { amount: 2 },
                InternalAction::ApplyWeak {
                    target: MonsterId::new(1),
                    amount: 3,
                },
                InternalAction::ApplyVulnerable {
                    target: MonsterId::new(1),
                    amount: 3,
                },
                InternalAction::ReduceMonsterStrength {
                    target: MonsterId::new(1),
                    amount: 3,
                },
                InternalAction::ApplyWeak {
                    target: MonsterId::new(2),
                    amount: 3,
                },
                InternalAction::ApplyVulnerable {
                    target: MonsterId::new(2),
                    amount: 3,
                },
                InternalAction::ReduceMonsterStrength {
                    target: MonsterId::new(2),
                    amount: 3,
                },
                InternalAction::MoveCard {
                    card_id: shockwave_id,
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::CardExhausted {
                    card_id: shockwave_id,
                },
            ]
        );
    }

    #[test]
    fn bash_is_illegal_with_less_than_two_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;

        assert_eq!(
            apply_combat_action(&state, bash_action(&state)),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn disarm_applies_minus_two_strength_to_target_and_exhausts() {
        let state = two_monster_hand(DISARM_ID);

        let next = apply_combat_action(&state, disarm_action(&state)).expect("Disarm applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.monsters[0].powers.strength, -2);
        assert_eq!(next.monsters[1].powers.strength, 0);
        assert!(next.piles.discard_pile.is_empty());
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, DISARM_ID);
    }

    #[test]
    fn disarm_stacks_with_existing_monster_strength() {
        let mut state = hand_only(DISARM_ID);
        state.monsters[0].powers.strength = 3;

        let next = apply_combat_action(&state, disarm_action(&state)).expect("Disarm applies");

        assert_eq!(next.monsters[0].powers.strength, 1);
    }

    #[test]
    fn disarm_reduces_monster_outgoing_attack_damage() {
        let mut state = hand_only(DISARM_ID);
        state.monsters[0].powers.strength = 3;

        let next = apply_combat_action(&state, disarm_action(&state)).expect("Disarm applies");

        assert_eq!(
            crate::combat::turn_powers::monster_attack_damage(&next.monsters[0], 6),
            7
        );
    }

    #[test]
    fn disarm_event_log_records_strength_reduction_then_exhaust() {
        let state = hand_only(DISARM_ID);
        let disarm_id = hand_card_id(&state, DISARM_ID);

        let transition =
            apply_combat_action_with_events(&state, disarm_action(&state)).expect("Disarm applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard { card_id: disarm_id },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::ReduceMonsterStrength {
                    target: MonsterId::new(1),
                    amount: 2,
                },
                InternalAction::MoveCard {
                    card_id: disarm_id,
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::CardExhausted { card_id: disarm_id },
            ]
        );
    }

    #[test]
    fn bash_then_strike_deals_expected_damage_with_vulnerable() {
        let state = CombatState::initial_fixture();
        let after_bash = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");
        let after_strike =
            apply_combat_action(&after_bash, strike_action(&after_bash)).expect("Strike applies");

        assert_eq!(after_bash.monsters[0].hp, state.monsters[0].hp - 8);
        assert_eq!(after_bash.monsters[0].powers.vulnerable, 2);
        assert_eq!(after_strike.monsters[0].hp, after_bash.monsters[0].hp - 9);
    }

    #[test]
    fn defend_with_dexterity_gains_extra_block() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.dexterity = 2;

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(next.player.block, 7);
    }

    #[test]
    fn defend_with_frail_gains_reduced_block() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.frail = 1;

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(next.player.block, 3);
    }

    #[test]
    fn good_instincts_gains_six_block_at_zero_cost_and_discards() {
        let mut state = hand_only(GOOD_INSTINCTS_ID);
        state.player.energy = 0;
        let good_instincts_id = hand_card_id(&state, GOOD_INSTINCTS_ID);

        let next = apply_combat_action(&state, good_instincts_action(&state))
            .expect("Good Instincts applies");

        assert_eq!(next.player.block, 6);
        assert_eq!(next.player.energy, 0);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == good_instincts_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == good_instincts_id));
    }

    #[test]
    fn good_instincts_rejects_target() {
        let state = hand_only(GOOD_INSTINCTS_ID);

        assert_eq!(
            validate_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: hand_card_id(&state, GOOD_INSTINCTS_ID),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn good_instincts_uses_existing_block_calculation() {
        let mut state = hand_only(GOOD_INSTINCTS_ID);
        state.player.powers.dexterity = 2;
        state.player.powers.frail = 1;

        let next = apply_combat_action(&state, good_instincts_action(&state))
            .expect("Good Instincts applies");

        assert_eq!(next.player.block, calculate_block(6, state.player.powers));
    }

    #[test]
    fn good_instincts_event_log_records_generic_skill_queue() {
        let state = hand_only(GOOD_INSTINCTS_ID);
        let good_instincts_id = hand_card_id(&state, GOOD_INSTINCTS_ID);

        let transition = apply_combat_action_with_events(&state, good_instincts_action(&state))
            .expect("Good Instincts applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: good_instincts_id
                },
                InternalAction::SpendEnergy { amount: 0 },
                InternalAction::GainBlock { amount: 6 },
                InternalAction::MoveCard {
                    card_id: good_instincts_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn bandage_up_heals_four_at_zero_cost_and_exhausts() {
        let mut state = hand_only(BANDAGE_UP_ID);
        state.player.energy = 0;
        state.player.hp = 40;
        state.player.max_hp = 80;
        let bandage_up_id = hand_card_id(&state, BANDAGE_UP_ID);

        let next =
            apply_combat_action(&state, bandage_up_action(&state)).expect("Bandage Up applies");

        assert_eq!(next.player.hp, 44);
        assert_eq!(next.player.energy, 0);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == bandage_up_id));
    }

    #[test]
    fn bandage_up_healing_is_capped_at_max_hp() {
        let mut state = hand_only(BANDAGE_UP_ID);
        state.player.hp = 78;
        state.player.max_hp = 80;

        let next =
            apply_combat_action(&state, bandage_up_action(&state)).expect("Bandage Up applies");

        assert_eq!(next.player.hp, 80);
    }

    #[test]
    fn magic_flower_increases_bandage_up_healing() {
        let mut state = hand_only(BANDAGE_UP_ID);
        state.player.hp = 40;
        state.player.max_hp = 80;
        state.relics = vec![Relic::MagicFlower];

        let next =
            apply_combat_action(&state, bandage_up_action(&state)).expect("Bandage Up applies");

        assert_eq!(next.player.hp, 46);
    }

    #[test]
    fn bandage_up_event_log_records_heal_before_exhaust() {
        let mut state = hand_only(BANDAGE_UP_ID);
        state.player.hp = 40;
        let bandage_up_id = hand_card_id(&state, BANDAGE_UP_ID);

        let transition = apply_combat_action_with_events(&state, bandage_up_action(&state))
            .expect("Bandage Up applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: bandage_up_id
                },
                InternalAction::SpendEnergy { amount: 0 },
                InternalAction::HealPlayer { amount: 4 },
                InternalAction::MoveCard {
                    card_id: bandage_up_id,
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::CardExhausted {
                    card_id: bandage_up_id
                },
            ]
        );
    }

    #[test]
    fn finesse_gains_two_block_draws_one_at_zero_cost_and_discards() {
        let mut state = hand_only(FINESSE_ID);
        state.player.energy = 0;
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];
        let finesse_id = hand_card_id(&state, FINESSE_ID);

        let next = apply_combat_action(&state, finesse_action(&state)).expect("Finesse applies");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.block, 2);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(30)));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == finesse_id));
    }

    #[test]
    fn finesse_event_log_records_block_draw_then_discard() {
        let mut state = hand_only(FINESSE_ID);
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];
        let finesse_id = hand_card_id(&state, FINESSE_ID);

        let transition = apply_combat_action_with_events(&state, finesse_action(&state))
            .expect("Finesse applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: finesse_id
                },
                InternalAction::SpendEnergy { amount: 0 },
                InternalAction::GainBlock { amount: 2 },
                InternalAction::DrawCards { count: 1 },
                InternalAction::MoveCard {
                    card_id: finesse_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn flash_of_steel_deals_three_draws_one_and_discards_at_zero_cost() {
        let mut state = hand_only(FLASH_OF_STEEL_ID);
        state.player.energy = 0;
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next = apply_combat_action(&state, flash_of_steel_action(&state))
            .expect("Flash of Steel applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 3);
        assert_eq!(next.player.energy, 0);
        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert!(next.piles.draw_pile.is_empty());
        assert_eq!(next.piles.discard_pile[0].content_id, FLASH_OF_STEEL_ID);
        assert!(next.piles.exhaust_pile.is_empty());
    }

    #[test]
    fn flash_of_steel_event_log_records_damage_draw_then_discard() {
        let mut state = hand_only(FLASH_OF_STEEL_ID);
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), DEFEND_R_ID)];
        let flash_of_steel_id = hand_card_id(&state, FLASH_OF_STEEL_ID);

        let transition = apply_combat_action_with_events(&state, flash_of_steel_action(&state))
            .expect("Flash of Steel applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: flash_of_steel_id
                },
                InternalAction::SpendEnergy { amount: 0 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(flash_of_steel_id),
                        target: MonsterId::new(1),
                        amount: 3,
                    }
                },
                InternalAction::DrawCards { count: 1 },
                InternalAction::MoveCard {
                    card_id: flash_of_steel_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn havoc_top_draw_flash_of_steel_deals_damage_draws_and_exhausts_it() {
        let mut state = hand_only(HAVOC_ID);
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(31), FLASH_OF_STEEL_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: hand_card_id(&state, HAVOC_ID),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Havoc plays Flash of Steel");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 3);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == HAVOC_ID));
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == FLASH_OF_STEEL_ID));
    }

    #[test]
    fn slimed_exhausts_and_adds_slimed_to_discard() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), SLIMED_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Slimed applies");

        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == SLIMED_ID));
    }

    #[test]
    fn blue_candle_exhausts_curse_and_loses_one_hp() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::BlueCandle];
        state.player.hp = 50;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), REGRET_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Regret applies with Blue Candle");

        assert_eq!(next.player.hp, 50 - crate::relic::BLUE_CANDLE_HP_LOSS);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn buffer_prevents_blue_candle_hp_loss() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::BlueCandle];
        state.player.hp = 50;
        state.player.powers.buffer = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), REGRET_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Regret applies with Blue Candle");

        assert_eq!(next.player.hp, 50);
        assert_eq!(next.player.powers.buffer, 0);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn medical_kit_exhausts_status_without_hp_loss() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::MedicalKit];
        state.player.hp = 50;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), WOUND_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Wound applies with Medical Kit");

        assert_eq!(next.player.hp, 50);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn strike_event_log_records_ordered_internal_actions() {
        let state = CombatState::initial_fixture();
        let strike_id = hand_strike_id(&state);

        let transition =
            apply_combat_action_with_events(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard { card_id: strike_id },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(strike_id),
                        target: MonsterId::new(1),
                        amount: 6,
                    },
                },
                InternalAction::MoveCard {
                    card_id: strike_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn strike_dummy_adds_three_damage_to_strike_cards() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(crate::Relic::StrikeDummy);

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 9);
    }

    #[test]
    fn strike_dummy_adds_three_damage_to_upgraded_strike_cards() {
        let mut state = hand_only(STRIKE_R_PLUS_ID);
        state.relics.push(crate::Relic::StrikeDummy);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Strike+ applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 12);
    }

    #[test]
    fn akabeko_adds_eight_damage_to_first_attack_card() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(crate::Relic::Akabeko);

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 14);
        assert_eq!(next.relic_counters.attacks_played_this_combat, 1);
    }

    #[test]
    fn akabeko_does_not_apply_after_first_attack_card() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(crate::Relic::Akabeko);
        state.relic_counters.attacks_played_this_combat = 1;

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 6);
        assert_eq!(next.relic_counters.attacks_played_this_combat, 2);
    }

    #[test]
    fn akabeko_bonus_applies_to_each_hit_of_first_multi_hit_attack() {
        let mut state = hand_only(TWIN_STRIKE_ID);
        state.relics.push(crate::Relic::Akabeko);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Twin Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 26);
        assert_eq!(next.relic_counters.attacks_played_this_combat, 1);
    }

    #[test]
    fn pen_nib_doubles_tenth_attack_card_damage_and_resets_counter() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(crate::Relic::PenNib);
        state.relic_counters.pen_nib_attacks_played = 9;

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 12);
        assert_eq!(next.relic_counters.pen_nib_attacks_played, 0);
    }

    #[test]
    fn pen_nib_does_not_double_before_tenth_attack_card() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(crate::Relic::PenNib);
        state.relic_counters.pen_nib_attacks_played = 8;

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 6);
        assert_eq!(next.relic_counters.pen_nib_attacks_played, 9);
    }

    #[test]
    fn pen_nib_bonus_applies_to_each_hit_of_multi_hit_attack() {
        let mut state = hand_only(TWIN_STRIKE_ID);
        state.relics.push(crate::Relic::PenNib);
        state.relic_counters.pen_nib_attacks_played = 9;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Twin Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 20);
        assert_eq!(next.relic_counters.pen_nib_attacks_played, 0);
    }

    #[test]
    fn centennial_puzzle_draws_after_spikes_hp_loss() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(crate::Relic::CentennialPuzzle);
        state.player.hp = 20;
        state.monsters[0].powers.spikes = 1;
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), DEFEND_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
            CardInstance::new(CardId::new(32), DEFEND_R_ID),
        ];

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.player.hp, 19);
        assert_eq!(next.relic_counters.centennial_puzzle_triggers, 1);
        assert_eq!(next.piles.hand.len(), 5);
        assert!(next.piles.draw_pile.is_empty());
    }

    #[test]
    fn hand_drill_applies_vulnerable_when_attack_breaks_block() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(crate::Relic::HandDrill);
        state.monsters[0].block = 5;

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(
            next.monsters[0].powers.vulnerable,
            crate::relic::HAND_DRILL_VULNERABLE
        );
        assert_eq!(next.monsters[0].block, 0);
    }

    #[test]
    fn hand_drill_does_not_apply_without_relic() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].block = 5;

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.monsters[0].powers.vulnerable, 0);
        assert_eq!(next.monsters[0].block, 0);
    }

    #[test]
    fn hand_drill_does_not_apply_when_attack_does_not_break_block() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(crate::Relic::HandDrill);
        state.monsters[0].block = 10;

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.monsters[0].powers.vulnerable, 0);
        assert_eq!(next.monsters[0].block, 4);
    }

    #[test]
    fn defend_event_log_records_gain_block() {
        let state = CombatState::initial_fixture();
        let defend_id = hand_card_id(&state, DEFEND_R_ID);

        let transition =
            apply_combat_action_with_events(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard { card_id: defend_id },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::GainBlock { amount: 5 },
                InternalAction::MoveCard {
                    card_id: defend_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn bash_damage_event_log_includes_source_target_and_amount() {
        let state = CombatState::initial_fixture();
        let bash_id = hand_card_id(&state, BASH_ID);

        let transition =
            apply_combat_action_with_events(&state, bash_action(&state)).expect("Bash applies");

        assert!(transition.event_log.contains(&InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(bash_id),
                target: MonsterId::new(1),
                amount: 8,
            },
        }));
    }

    #[test]
    fn anger_deals_six_damage_without_spending_energy() {
        let state = hand_only(ANGER_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Anger applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 6);
        assert_eq!(next.player.energy, state.player.energy);
    }

    #[test]
    fn anger_adds_copy_to_discard() {
        let state = hand_only(ANGER_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Anger applies");

        assert_eq!(next.piles.discard_pile.len(), 2);
        assert_eq!(
            next.piles
                .discard_pile
                .iter()
                .filter(|card| card.content_id == ANGER_ID)
                .count(),
            2
        );
    }

    #[test]
    fn anger_plus_deals_seven_damage_and_copies_upgraded_card() {
        let state = hand_only(ANGER_PLUS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Anger+ applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 7);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == ANGER_PLUS_ID));
    }

    #[test]
    fn anger_played_twice_grows_discard_copies() {
        let state = hand_only(ANGER_ID);

        let after_first = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("first Anger applies");
        let anger_copy = after_first
            .piles
            .discard_pile
            .iter()
            .find(|card| card.content_id == ANGER_ID && card.id != CardId::new(20))
            .expect("Anger copy is in discard")
            .id;
        let mut second_hand = after_first.clone();
        second_hand
            .piles
            .discard_pile
            .retain(|card| card.id != anger_copy);
        second_hand.piles.hand = vec![CardInstance::new(anger_copy, ANGER_ID)];

        let after_second = apply_combat_action(
            &second_hand,
            CombatAction::PlayCard {
                card_id: anger_copy,
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("second Anger applies");

        assert_eq!(
            after_second
                .piles
                .discard_pile
                .iter()
                .filter(|card| card.content_id == ANGER_ID)
                .count(),
            3
        );
    }

    #[test]
    fn cleave_deals_eight_to_all_enemies() {
        let state = two_monster_hand(CLEAVE_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Cleave applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 8);
        assert_eq!(next.monsters[1].hp, state.monsters[1].hp - 8);
        assert_eq!(next.player.energy, state.player.energy - 1);
    }

    #[test]
    fn cleave_plus_deals_nine_to_all_enemies() {
        let state = two_monster_hand(CLEAVE_PLUS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Cleave+ applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 9);
        assert_eq!(next.monsters[1].hp, state.monsters[1].hp - 9);
    }

    #[test]
    fn gremlin_horn_gains_energy_and_draws_when_monster_dies() {
        let mut state = two_monster_hand(CLEAVE_ID);
        state.relics = vec![Relic::GremlinHorn];
        state.player.energy = 2;
        state.monsters[0].hp = 8;
        state.monsters[1].hp = 20;
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), DEFEND_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Cleave applies");

        assert!(!next.monsters[0].alive);
        assert!(next.monsters[1].alive);
        assert_eq!(next.player.energy, 2);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == DEFEND_R_ID));
    }

    #[test]
    fn reaper_damages_all_living_enemies_heals_unblocked_damage_and_exhausts() {
        let mut state = two_monster_hand(REAPER_ID);
        state.player.hp = 50;
        state.monsters[0].block = 2;
        let reaper_id = hand_card_id(&state, REAPER_ID);

        let next = apply_combat_action(&state, reaper_action(&state)).expect("Reaper applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 2);
        assert_eq!(next.monsters[1].hp, state.monsters[1].hp - 4);
        assert_eq!(next.player.hp, 56);
        assert_eq!(next.player.energy, state.player.energy - 2);
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].id, reaper_id);
    }

    #[test]
    fn reaper_ignores_dead_enemies_for_damage_and_healing() {
        let mut state = two_monster_hand(REAPER_ID);
        state.player.hp = 50;
        state.monsters[1].alive = false;
        state.monsters[1].hp = 0;

        let next = apply_combat_action(&state, reaper_action(&state)).expect("Reaper applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 4);
        assert_eq!(next.monsters[1].hp, 0);
        assert_eq!(next.player.hp, 54);
    }

    #[test]
    fn magic_flower_increases_reaper_healing() {
        let mut state = two_monster_hand(REAPER_ID);
        state.player.hp = 40;
        state.relics = vec![Relic::MagicFlower];

        let next = apply_combat_action(&state, reaper_action(&state)).expect("Reaper applies");

        assert_eq!(next.player.hp, 52);
    }

    #[test]
    fn akabeko_and_pen_nib_modify_reaper_damage_and_healing() {
        let mut state = two_monster_hand(REAPER_ID);
        state.player.hp = 20;
        state.relics = vec![Relic::Akabeko, Relic::PenNib];
        state.relic_counters.pen_nib_attacks_played = 9;

        let next = apply_combat_action(&state, reaper_action(&state)).expect("Reaper applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 24);
        assert_eq!(next.monsters[1].hp, state.monsters[1].hp - 24);
        assert_eq!(next.player.hp, 68);
    }

    #[test]
    fn reaper_event_log_records_damage_heal_action_then_exhaust() {
        let state = two_monster_hand(REAPER_ID);
        let reaper_id = hand_card_id(&state, REAPER_ID);

        let transition =
            apply_combat_action_with_events(&state, reaper_action(&state)).expect("Reaper applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard { card_id: reaper_id },
                InternalAction::SpendEnergy { amount: 2 },
                InternalAction::DealDamageAllAndHealUnblocked {
                    source: reaper_id,
                    amount: 4,
                },
                InternalAction::MoveCard {
                    card_id: reaper_id,
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::CardExhausted { card_id: reaper_id },
            ]
        );
    }

    #[test]
    fn dramatic_entrance_deals_eight_to_all_enemies_and_exhausts() {
        use crate::content::cards::DRAMATIC_ENTRANCE_ID;

        let state = two_monster_hand(DRAMATIC_ENTRANCE_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Dramatic Entrance applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 8);
        assert_eq!(next.monsters[1].hp, state.monsters[1].hp - 8);
        assert_eq!(next.player.energy, state.player.energy);
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, DRAMATIC_ENTRANCE_ID);
        assert!(next.piles.hand.is_empty());
    }

    #[test]
    fn strange_spoon_roll_controls_played_card_exhaust_destination() {
        use crate::content::cards::DRAMATIC_ENTRANCE_ID;

        let mut state = two_monster_hand(DRAMATIC_ENTRANCE_ID);
        state.relics = vec![Relic::StrangeSpoon];
        state.card_random_rng = Some(crate::rng::StsRng::new(123));
        let mut expected_rng = crate::rng::StsRng::new(123);
        let spoon_proc = expected_rng.random_bool();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Dramatic Entrance applies");

        assert_eq!(
            next.card_random_rng.as_ref().expect("card rng").counter(),
            expected_rng.counter()
        );
        if spoon_proc {
            assert!(next.piles.exhaust_pile.is_empty());
            assert_eq!(next.piles.discard_pile[0].content_id, DRAMATIC_ENTRANCE_ID);
        } else {
            assert!(next.piles.discard_pile.is_empty());
            assert_eq!(next.piles.exhaust_pile[0].content_id, DRAMATIC_ENTRANCE_ID);
        }
    }

    #[test]
    fn whirlwind_at_three_energy_hits_all_enemies_three_times() {
        let state = two_monster_hand(WHIRLWIND_ID);
        assert_eq!(state.player.energy, 3);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Whirlwind applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 15);
        assert_eq!(next.monsters[1].hp, state.monsters[1].hp - 15);
        assert_eq!(next.player.energy, 0);
    }

    #[test]
    fn whirlwind_with_chemical_x_adds_two_hits() {
        let mut state = two_monster_hand(WHIRLWIND_ID);
        state.relics.push(crate::Relic::ChemicalX);
        assert_eq!(state.player.energy, 3);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Whirlwind applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 25);
        assert_eq!(next.monsters[1].hp, state.monsters[1].hp - 25);
        assert_eq!(next.player.energy, 0);
    }

    #[test]
    fn zero_energy_whirlwind_with_chemical_x_hits_twice() {
        let mut state = two_monster_hand(WHIRLWIND_ID);
        state.player.energy = 0;
        state.relics.push(crate::Relic::ChemicalX);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Whirlwind applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 10);
        assert_eq!(next.monsters[1].hp, state.monsters[1].hp - 10);
        assert_eq!(next.player.energy, 0);
    }

    #[test]
    fn whirlwind_plus_at_three_energy_hits_all_enemies_three_times() {
        let state = two_monster_hand(WHIRLWIND_PLUS_ID);
        assert_eq!(state.player.energy, 3);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Whirlwind+ applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 24);
        assert_eq!(next.monsters[1].hp, state.monsters[1].hp - 24);
        assert_eq!(next.player.energy, 0);
    }

    #[test]
    fn twin_strike_deals_five_damage_twice() {
        let state = hand_only(TWIN_STRIKE_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Twin Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 10);
        assert_eq!(next.player.energy, state.player.energy - 1);
    }

    #[test]
    fn twin_strike_plus_deals_six_damage_twice() {
        let state = hand_only(TWIN_STRIKE_PLUS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Twin Strike+ applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 12);
    }

    #[test]
    fn twin_strike_with_vulnerable_deals_bonus_on_both_hits() {
        let mut state = hand_only(TWIN_STRIKE_ID);
        state.monsters[0].powers.vulnerable = 1;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Twin Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 14);
    }

    #[test]
    fn shrug_it_off_gains_eight_block_draws_one_and_moves_to_discard() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), SHRUG_IT_OFF_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Shrug It Off applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.player.block, 8);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(30)));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn shrug_it_off_event_log_records_draw_cards() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), SHRUG_IT_OFF_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let transition = apply_combat_action_with_events(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Shrug It Off applies");

        assert!(transition
            .event_log
            .contains(&InternalAction::DrawCards { count: 1 }));
    }

    #[test]
    fn true_grit_gains_seven_block_and_exhausts_lowest_other_hand_card() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(25), TRUE_GRIT_ID),
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("True Grit applies");

        assert_eq!(next.player.block, 7);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(25)));
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(30)));
    }

    #[test]
    fn strange_spoon_does_not_roll_for_true_grit_target_exhaust() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::StrangeSpoon];
        state.card_random_rng = Some(crate::rng::StsRng::new(123));
        state.piles.hand = vec![
            CardInstance::new(CardId::new(25), TRUE_GRIT_ID),
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("True Grit applies");

        assert_eq!(
            next.card_random_rng.as_ref().expect("card rng").counter(),
            0
        );
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(25)));
    }

    #[test]
    fn true_grit_with_only_itself_in_hand_does_not_exhaust() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(25), TRUE_GRIT_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("True Grit applies");

        assert_eq!(next.player.block, 7);
        assert!(next.piles.exhaust_pile.is_empty());
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(25)));
    }

    #[test]
    fn charons_ashes_deals_three_to_all_enemies_when_card_exhausts() {
        let mut state = two_monster_hand(TRUE_GRIT_ID);
        state.relics = vec![Relic::CharonsAshes];
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(10), DEFEND_R_ID));
        let first_hp = state.monsters[0].hp;
        let second_hp = state.monsters[1].hp;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("True Grit applies");

        assert_eq!(
            next.monsters[0].hp,
            first_hp - crate::relic::CHARONS_ASHES_DAMAGE
        );
        assert_eq!(
            next.monsters[1].hp,
            second_hp - crate::relic::CHARONS_ASHES_DAMAGE
        );
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(10)));
    }

    #[test]
    fn burning_pact_exhausts_lowest_other_hand_card_and_draws_two() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(25), BURNING_PACT_ID),
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(40), STRIKE_R_ID),
            CardInstance::new(CardId::new(41), DEFEND_R_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Burning Pact applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(25)));
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(40)));
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(41)));
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(30)));
    }

    #[test]
    fn burning_pact_with_only_itself_in_hand_draws_two_without_exhaust() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(25), BURNING_PACT_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(40), STRIKE_R_ID),
            CardInstance::new(CardId::new(41), DEFEND_R_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Burning Pact applies");

        assert!(next.piles.exhaust_pile.is_empty());
        assert_eq!(next.piles.hand.len(), 2);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(25)));
    }

    #[test]
    fn sentinel_gains_five_block_spends_one_and_moves_to_discard_without_exhaust_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(25), SENTINEL_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Sentinel applies");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.block, 5);
        assert!(next.piles.exhaust_pile.is_empty());
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(25)));
    }

    #[test]
    fn sentinel_event_log_records_generic_block_skill_queue() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(25), SENTINEL_ID)];

        let transition = apply_combat_action_with_events(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Sentinel applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(25)
                },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::GainBlock { amount: 5 },
                InternalAction::MoveCard {
                    card_id: CardId::new(25),
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn sentinel_block_uses_dexterity_and_frail() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.dexterity = 2;
        state.player.powers.frail = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(25), SENTINEL_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Sentinel applies");

        assert_eq!(next.player.block, 5);
    }

    #[test]
    fn burning_pact_exhausting_sentinel_grants_two_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(25), BURNING_PACT_ID),
            CardInstance::new(CardId::new(20), SENTINEL_ID),
        ];
        state.piles.draw_pile.clear();

        let transition = apply_combat_action_with_events(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Burning Pact applies");
        let next = transition.state;

        assert_eq!(next.player.energy, 2);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(
            transition.event_log.iter().position(|action| {
                *action
                    == InternalAction::MoveCard {
                        card_id: CardId::new(20),
                        from: CardPile::Hand,
                        to: CardPile::ExhaustPile,
                    }
            }) < transition.event_log.iter().position(|action| {
                *action
                    == InternalAction::CardExhausted {
                        card_id: CardId::new(20),
                    }
            })
        );
    }

    #[test]
    fn true_grit_exhausting_sentinel_grants_two_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(25), TRUE_GRIT_ID),
            CardInstance::new(CardId::new(20), SENTINEL_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("True Grit applies");

        assert_eq!(next.player.energy, 2);
        assert_eq!(next.player.block, 7);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn burning_pact_exhausting_non_sentinel_does_not_grant_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(25), BURNING_PACT_ID),
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
        ];
        state.piles.draw_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Burning Pact applies");

        assert_eq!(next.player.energy, 0);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn sentinel_exhaust_still_triggers_feel_no_pain() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.player.powers.feel_no_pain = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(25), BURNING_PACT_ID),
            CardInstance::new(CardId::new(20), SENTINEL_ID),
        ];
        state.piles.draw_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Burning Pact applies");

        assert_eq!(next.player.energy, 2);
        assert_eq!(next.player.block, 3);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn exhaust_select_exhausting_sentinel_grants_two_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 0;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), SENTINEL_ID)];

        open_exhaust_select(&mut state).expect("opens exhaust select");
        choose_exhaust_select(&mut state, 0).expect("chooses Sentinel");
        confirm_exhaust_select(&mut state).expect("confirms exhaust select");

        assert_eq!(state.player.energy, 2);
        assert!(state
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(state.piles.hand.is_empty());
    }

    #[test]
    fn feel_no_pain_grants_power_and_moves_to_discard() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), FEEL_NO_PAIN_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Feel No Pain applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.player.powers.feel_no_pain, 1);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn feel_no_pain_grants_block_when_card_exhausted() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.feel_no_pain = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(25), BURNING_PACT_ID),
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
        ];
        state.piles.draw_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Burning Pact applies");

        assert_eq!(next.player.block, 3);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn dark_embrace_grants_power_and_moves_to_discard() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DARK_EMBRACE_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Dark Embrace applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.player.powers.dark_embrace, 1);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn dark_embrace_draws_when_card_exhausted() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.dark_embrace = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(25), BURNING_PACT_ID),
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(40), STRIKE_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Burning Pact applies");

        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(40)));
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn demon_form_grants_ritual_spends_three_and_is_removed_from_hand() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DEMON_FORM_ID)];

        let next =
            apply_combat_action(&state, demon_form_action(&state)).expect("Demon Form applies");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.ritual, 2);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn demon_form_uses_effective_card_cost() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 2;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DEMON_FORM_ID)];
        state.piles.hand[0].temp_cost = Some(1);

        let next = apply_combat_action(&state, demon_form_action(&state))
            .expect("Demon Form applies with temp cost");

        assert_eq!(next.player.energy, 1);
        assert_eq!(next.player.powers.ritual, 2);
    }

    #[test]
    fn demon_form_event_log_records_power_gain_and_removal() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DEMON_FORM_ID)];

        let transition = apply_combat_action_with_events(&state, demon_form_action(&state))
            .expect("Demon Form applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(20),
                },
                InternalAction::SpendCardEnergy {
                    card_id: CardId::new(20),
                },
                InternalAction::GainRitual { amount: 2 },
                InternalAction::RemoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                },
            ]
        );
    }

    #[test]
    fn demon_form_ritual_grants_strength_at_end_of_player_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DEMON_FORM_ID)];
        state.piles.draw_pile.clear();

        let after_demon =
            apply_combat_action(&state, demon_form_action(&state)).expect("Demon Form applies");
        let next_turn = crate::combat::end_player_turn(&after_demon);

        assert_eq!(next_turn.player.powers.ritual, 2);
        assert_eq!(next_turn.player.powers.strength, 2);
    }

    #[test]
    fn demon_form_ritual_strength_stacks_across_turns() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DEMON_FORM_ID)];
        state.piles.draw_pile.clear();

        let after_demon =
            apply_combat_action(&state, demon_form_action(&state)).expect("Demon Form applies");
        let next_turn = crate::combat::end_player_turn(&after_demon);
        let following_turn = crate::combat::end_player_turn(&next_turn);

        assert_eq!(following_turn.player.powers.ritual, 2);
        assert_eq!(following_turn.player.powers.strength, 4);
    }

    #[test]
    fn demon_form_ritual_round_trips_through_combat_state_json() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DEMON_FORM_ID)];

        let next =
            apply_combat_action(&state, demon_form_action(&state)).expect("Demon Form applies");
        let json = serde_json::to_string(&next).expect("combat state serializes");
        let restored: CombatState = serde_json::from_str(&json).expect("combat state restores");

        assert_eq!(restored.player.powers.ritual, 2);
        assert_eq!(restored, next);
    }

    #[test]
    fn demon_form_triggers_bird_faced_urn_power_heal() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 60;
        state.player.max_hp = 70;
        state.player.energy = 3;
        state.relics = vec![Relic::BirdFacedUrn];
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DEMON_FORM_ID)];

        let next =
            apply_combat_action(&state, demon_form_action(&state)).expect("Demon Form applies");

        assert_eq!(next.player.hp, 60 + crate::relic::BIRD_FACED_URN_HEAL);
        assert_eq!(next.player.powers.ritual, 2);
    }

    #[test]
    fn demon_form_removal_can_trigger_unceasing_top() {
        let mut state = hand_only(DEMON_FORM_ID);
        state.player.energy = 3;
        state.relics = vec![Relic::UnceasingTop];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next =
            apply_combat_action(&state, demon_form_action(&state)).expect("Demon Form applies");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert_eq!(next.player.powers.ritual, 2);
    }

    #[test]
    fn barricade_grants_power_spends_three_and_is_removed_from_hand() {
        let mut state = hand_only(BARRICADE_ID);
        state.player.energy = 3;

        let next =
            apply_combat_action(&state, barricade_action(&state)).expect("Barricade applies");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.barricade, 1);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn barricade_rejects_target() {
        let mut state = hand_only(BARRICADE_ID);
        state.player.energy = 3;

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn barricade_uses_effective_card_cost() {
        let mut state = hand_only(BARRICADE_ID);
        state.player.energy = 2;
        state.piles.hand[0].temp_cost = Some(1);

        let next = apply_combat_action(&state, barricade_action(&state))
            .expect("Barricade applies with temp cost");

        assert_eq!(next.player.energy, 1);
        assert_eq!(next.player.powers.barricade, 1);
    }

    #[test]
    fn barricade_retains_block_across_turn_transition() {
        let mut state = hand_only(BARRICADE_ID);
        state.player.energy = 3;
        state.player.block = 18;
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 0 };
        state.piles.draw_pile.clear();

        let after_barricade =
            apply_combat_action(&state, barricade_action(&state)).expect("Barricade applies");
        let next_turn = crate::combat::end_player_turn(&after_barricade);

        assert_eq!(next_turn.player.powers.barricade, 1);
        assert_eq!(next_turn.player.block, 18);
    }

    #[test]
    fn barricade_round_trips_through_combat_state_json() {
        let mut state = hand_only(BARRICADE_ID);
        state.player.energy = 3;

        let next =
            apply_combat_action(&state, barricade_action(&state)).expect("Barricade applies");
        let json = serde_json::to_string(&next).expect("combat state serializes");
        let restored: CombatState = serde_json::from_str(&json).expect("combat state restores");

        assert_eq!(restored.player.powers.barricade, 1);
        assert_eq!(restored, next);
    }

    #[test]
    fn barricade_event_log_records_power_gain_and_removal() {
        let mut state = hand_only(BARRICADE_ID);
        state.player.energy = 3;

        let transition = apply_combat_action_with_events(&state, barricade_action(&state))
            .expect("Barricade applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(20),
                },
                InternalAction::SpendCardEnergy {
                    card_id: CardId::new(20),
                },
                InternalAction::GainBarricade { amount: 1 },
                InternalAction::RemoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                },
            ]
        );
    }

    #[test]
    fn barricade_triggers_bird_faced_urn_power_heal() {
        let mut state = hand_only(BARRICADE_ID);
        state.player.hp = 60;
        state.player.max_hp = 70;
        state.player.energy = 3;
        state.relics = vec![Relic::BirdFacedUrn];

        let next =
            apply_combat_action(&state, barricade_action(&state)).expect("Barricade applies");

        assert_eq!(next.player.hp, 60 + crate::relic::BIRD_FACED_URN_HEAL);
        assert_eq!(next.player.powers.barricade, 1);
    }

    #[test]
    fn barricade_removal_can_trigger_unceasing_top() {
        let mut state = hand_only(BARRICADE_ID);
        state.player.energy = 3;
        state.relics = vec![Relic::UnceasingTop];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next =
            apply_combat_action(&state, barricade_action(&state)).expect("Barricade applies");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert_eq!(next.player.powers.barricade, 1);
    }

    #[test]
    fn evolve_grants_power_and_is_removed_from_hand() {
        let mut state = hand_only(EVOLVE_ID);
        state.player.energy = 1;

        let next = apply_combat_action(&state, evolve_action(&state)).expect("Evolve applies");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.evolve, 1);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn evolve_rejects_target() {
        let state = hand_only(EVOLVE_ID);

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn evolve_uses_effective_card_cost() {
        let mut state = hand_only(EVOLVE_ID);
        state.player.energy = 2;
        state.piles.hand[0].temp_cost = Some(2);

        let next =
            apply_combat_action(&state, evolve_action(&state)).expect("Evolve applies with cost");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.evolve, 1);
    }

    #[test]
    fn evolve_round_trips_through_combat_state_json() {
        let mut state = hand_only(EVOLVE_ID);
        state.player.energy = 1;

        let next = apply_combat_action(&state, evolve_action(&state)).expect("Evolve applies");
        let json = serde_json::to_string(&next).expect("combat state serializes");
        let restored: CombatState = serde_json::from_str(&json).expect("combat state restores");

        assert_eq!(restored.player.powers.evolve, 1);
        assert_eq!(restored, next);
    }

    #[test]
    fn evolve_event_log_records_power_gain_and_removal() {
        let mut state = hand_only(EVOLVE_ID);
        state.player.energy = 1;

        let transition =
            apply_combat_action_with_events(&state, evolve_action(&state)).expect("Evolve applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(20),
                },
                InternalAction::SpendCardEnergy {
                    card_id: CardId::new(20),
                },
                InternalAction::GainEvolve { amount: 1 },
                InternalAction::RemoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                },
            ]
        );
    }

    #[test]
    fn evolve_draws_one_extra_card_when_status_is_drawn() {
        let mut state = hand_only(POMMEL_STRIKE_ID);
        state.player.powers.evolve = 1;
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), DEFEND_R_ID),
            CardInstance::new(CardId::new(31), STRIKE_R_ID),
            CardInstance::new(CardId::new(32), WOUND_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Pommel Strike draws through Evolve");

        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(32), CardId::new(31)]
        );
        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(30)]
        );
    }

    #[test]
    fn evolve_stacks_extra_status_draws() {
        let mut state = hand_only(POMMEL_STRIKE_ID);
        state.player.powers.evolve = 2;
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), BASH_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
            CardInstance::new(CardId::new(32), STRIKE_R_ID),
            CardInstance::new(CardId::new(33), WOUND_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Pommel Strike draws through stacked Evolve");

        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(33), CardId::new(32), CardId::new(31)]
        );
        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(30)]
        );
    }

    #[test]
    fn evolve_extra_draw_can_chain_from_another_status_card() {
        let mut state = hand_only(POMMEL_STRIKE_ID);
        state.player.powers.evolve = 1;
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DAZED_ID),
            CardInstance::new(CardId::new(32), WOUND_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Pommel Strike draws chained statuses");

        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(32), CardId::new(31), CardId::new(30)]
        );
        assert!(next.piles.draw_pile.is_empty());
    }

    #[test]
    fn evolve_triggers_when_status_is_drawn_during_normal_turn_refill() {
        let mut state = hand_only(EVOLVE_ID);
        state.player.energy = 1;
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 0 };
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), WOUND_ID),
            CardInstance::new(CardId::new(32), DEFEND_R_ID),
            CardInstance::new(CardId::new(33), STRIKE_R_ID),
            CardInstance::new(CardId::new(34), DEFEND_R_ID),
            CardInstance::new(CardId::new(35), STRIKE_R_ID),
        ];

        let after_evolve =
            apply_combat_action(&state, evolve_action(&state)).expect("Evolve applies");
        let next_turn = crate::combat::end_player_turn(&after_evolve);

        assert_eq!(next_turn.player.powers.evolve, 1);
        assert_eq!(next_turn.piles.hand.len(), 6);
        assert_eq!(
            next_turn
                .piles
                .hand
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![
                CardId::new(35),
                CardId::new(34),
                CardId::new(33),
                CardId::new(32),
                CardId::new(31),
                CardId::new(30),
            ]
        );
        assert!(next_turn.piles.draw_pile.is_empty());
    }

    #[test]
    fn evolve_triggers_bird_faced_urn_power_heal() {
        let mut state = hand_only(EVOLVE_ID);
        state.player.hp = 60;
        state.player.max_hp = 70;
        state.player.energy = 1;
        state.relics = vec![Relic::BirdFacedUrn];

        let next = apply_combat_action(&state, evolve_action(&state)).expect("Evolve applies");

        assert_eq!(next.player.hp, 60 + crate::relic::BIRD_FACED_URN_HEAL);
        assert_eq!(next.player.powers.evolve, 1);
    }

    #[test]
    fn evolve_removal_can_trigger_unceasing_top_and_status_extra_draw() {
        let mut state = hand_only(EVOLVE_ID);
        state.player.energy = 1;
        state.relics = vec![Relic::UnceasingTop];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), WOUND_ID),
        ];

        let next = apply_combat_action(&state, evolve_action(&state)).expect("Evolve applies");

        assert_eq!(next.player.powers.evolve, 1);
        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(31), CardId::new(30)]
        );
        assert!(next.piles.draw_pile.is_empty());
    }

    #[test]
    fn combust_grants_power_spends_one_and_is_removed_from_hand() {
        let mut state = hand_only(COMBUST_ID);
        state.player.energy = 1;

        let next = apply_combat_action(&state, combust_action(&state)).expect("Combust applies");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.combust, 1);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn combust_rejects_target() {
        let mut state = hand_only(COMBUST_ID);
        state.player.energy = 1;

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn combust_uses_effective_card_cost() {
        let mut state = hand_only(COMBUST_ID);
        state.player.energy = 2;
        state.piles.hand[0].temp_cost = Some(2);

        let next = apply_combat_action(&state, combust_action(&state))
            .expect("Combust applies with temp cost");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.combust, 1);
    }

    #[test]
    fn combust_round_trips_through_combat_state_json() {
        let mut state = hand_only(COMBUST_ID);
        state.player.energy = 1;

        let next = apply_combat_action(&state, combust_action(&state)).expect("Combust applies");
        let json = serde_json::to_string(&next).expect("combat state serializes");
        let restored: CombatState = serde_json::from_str(&json).expect("combat state restores");

        assert_eq!(restored.player.powers.combust, 1);
        assert_eq!(restored, next);
    }

    #[test]
    fn combust_event_log_records_power_gain_and_removal() {
        let mut state = hand_only(COMBUST_ID);
        state.player.energy = 1;

        let transition = apply_combat_action_with_events(&state, combust_action(&state))
            .expect("Combust applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(20),
                },
                InternalAction::SpendCardEnergy {
                    card_id: CardId::new(20),
                },
                InternalAction::GainCombust { amount: 1 },
                InternalAction::RemoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                },
            ]
        );
    }

    #[test]
    fn combust_loses_hp_and_damages_all_living_enemies_at_end_of_turn() {
        let mut state = two_monster_hand(COMBUST_ID);
        state.player.energy = 1;
        state.player.hp = 40;
        state.monsters[0].hp = 20;
        state.monsters[1].hp = 20;
        state.monsters[1].block = 2;
        for monster in &mut state.monsters {
            monster.intent = crate::MonsterIntent::Block { block: 0 };
        }
        state.piles.draw_pile.clear();

        let after_combust =
            apply_combat_action(&state, combust_action(&state)).expect("Combust applies");
        let next_turn = crate::combat::end_player_turn(&after_combust);

        assert_eq!(next_turn.player.hp, 39);
        assert_eq!(next_turn.player.powers.combust, 1);
        assert_eq!(next_turn.monsters[0].hp, 15);
        assert_eq!(next_turn.monsters[1].hp, 17);
        assert_eq!(next_turn.monsters[1].block, 0);
    }

    #[test]
    fn combust_stacks_end_turn_hp_loss_and_damage() {
        let mut state = two_monster_hand(COMBUST_ID);
        state.player.energy = 1;
        state.player.hp = 40;
        state.player.powers.combust = 1;
        state.monsters[0].hp = 30;
        state.monsters[1].hp = 30;
        for monster in &mut state.monsters {
            monster.intent = crate::MonsterIntent::Block { block: 0 };
        }
        state.piles.draw_pile.clear();

        let after_combust =
            apply_combat_action(&state, combust_action(&state)).expect("Combust applies");
        let next_turn = crate::combat::end_player_turn(&after_combust);

        assert_eq!(next_turn.player.hp, 38);
        assert_eq!(next_turn.player.powers.combust, 2);
        assert_eq!(next_turn.monsters[0].hp, 20);
        assert_eq!(next_turn.monsters[1].hp, 20);
    }

    #[test]
    fn combust_hp_loss_uses_mitigation_but_damage_still_applies() {
        let mut state = hand_only(COMBUST_ID);
        state.player.energy = 1;
        state.player.hp = 40;
        state.relics = vec![Relic::TungstenRod];
        state.monsters[0].hp = 20;
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 0 };
        state.piles.draw_pile.clear();

        let after_combust =
            apply_combat_action(&state, combust_action(&state)).expect("Combust applies");
        let next_turn = crate::combat::end_player_turn(&after_combust);

        assert_eq!(next_turn.player.hp, 40);
        assert_eq!(next_turn.monsters[0].hp, 15);
    }

    #[test]
    fn combust_lethal_end_turn_damage_wins_before_monster_turn() {
        let mut state = hand_only(COMBUST_ID);
        state.player.energy = 1;
        state.player.hp = 40;
        state.monsters[0].hp = 5;
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 99 };
        state.piles.draw_pile.clear();

        let after_combust =
            apply_combat_action(&state, combust_action(&state)).expect("Combust applies");
        let after_end = crate::combat::end_player_turn(&after_combust);

        assert_eq!(after_end.phase, CombatPhase::Won);
        assert_eq!(after_end.player.hp, 45);
        assert!(!after_end.monsters[0].alive);
    }

    #[test]
    fn combust_triggers_bird_faced_urn_power_heal() {
        let mut state = hand_only(COMBUST_ID);
        state.player.hp = 60;
        state.player.max_hp = 70;
        state.player.energy = 1;
        state.relics = vec![Relic::BirdFacedUrn];

        let next = apply_combat_action(&state, combust_action(&state)).expect("Combust applies");

        assert_eq!(next.player.hp, 60 + crate::relic::BIRD_FACED_URN_HEAL);
        assert_eq!(next.player.powers.combust, 1);
    }

    #[test]
    fn combust_removal_can_trigger_unceasing_top() {
        let mut state = hand_only(COMBUST_ID);
        state.player.energy = 1;
        state.relics = vec![Relic::UnceasingTop];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next = apply_combat_action(&state, combust_action(&state)).expect("Combust applies");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert_eq!(next.player.powers.combust, 1);
    }

    #[test]
    fn corruption_grants_power_spends_three_and_is_removed_from_hand() {
        let mut state = hand_only(CORRUPTION_ID);
        state.player.energy = 3;

        let next =
            apply_combat_action(&state, corruption_action(&state)).expect("Corruption applies");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.corruption, 1);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn corruption_rejects_target() {
        let mut state = hand_only(CORRUPTION_ID);
        state.player.energy = 3;

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn corruption_uses_effective_card_cost() {
        let mut state = hand_only(CORRUPTION_ID);
        state.player.energy = 2;
        state.piles.hand[0].temp_cost = Some(1);

        let next = apply_combat_action(&state, corruption_action(&state))
            .expect("Corruption applies with temp cost");

        assert_eq!(next.player.energy, 1);
        assert_eq!(next.player.powers.corruption, 1);
    }

    #[test]
    fn corruption_round_trips_through_combat_state_json() {
        let mut state = hand_only(CORRUPTION_ID);
        state.player.energy = 3;

        let next =
            apply_combat_action(&state, corruption_action(&state)).expect("Corruption applies");
        let json = serde_json::to_string(&next).expect("combat state serializes");
        let restored: CombatState = serde_json::from_str(&json).expect("combat state restores");

        assert_eq!(restored.player.powers.corruption, 1);
        assert_eq!(restored, next);
    }

    #[test]
    fn corruption_event_log_records_power_gain_and_removal() {
        let mut state = hand_only(CORRUPTION_ID);
        state.player.energy = 3;

        let transition = apply_combat_action_with_events(&state, corruption_action(&state))
            .expect("Corruption applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(20),
                },
                InternalAction::SpendCardEnergy {
                    card_id: CardId::new(20),
                },
                InternalAction::GainCorruption { amount: 1 },
                InternalAction::RemoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                },
            ]
        );
    }

    #[test]
    fn corruption_makes_played_skills_cost_zero_and_exhaust() {
        let mut state = hand_only(DEFEND_R_ID);
        state.player.energy = 0;
        state.player.powers.corruption = 1;
        state.player.powers.feel_no_pain = 1;

        let transition = apply_combat_action_with_events(&state, defend_action(&state))
            .expect("Defend applies under Corruption");
        let next = transition.state;

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.block, 8);
        assert!(next.piles.discard_pile.is_empty());
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].id, CardId::new(20));
        assert!(transition
            .event_log
            .contains(&InternalAction::SpendEnergy { amount: 0 }));
        assert!(transition.event_log.contains(&InternalAction::MoveCard {
            card_id: CardId::new(20),
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        }));
        assert!(transition
            .event_log
            .contains(&InternalAction::CardExhausted {
                card_id: CardId::new(20),
            }));
    }

    #[test]
    fn corruption_triggers_bird_faced_urn_power_heal() {
        let mut state = hand_only(CORRUPTION_ID);
        state.player.hp = 60;
        state.player.max_hp = 70;
        state.player.energy = 3;
        state.relics = vec![Relic::BirdFacedUrn];

        let next =
            apply_combat_action(&state, corruption_action(&state)).expect("Corruption applies");

        assert_eq!(next.player.hp, 60 + crate::relic::BIRD_FACED_URN_HEAL);
        assert_eq!(next.player.powers.corruption, 1);
    }

    #[test]
    fn corruption_removal_can_trigger_unceasing_top() {
        let mut state = hand_only(CORRUPTION_ID);
        state.player.energy = 3;
        state.relics = vec![Relic::UnceasingTop];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next =
            apply_combat_action(&state, corruption_action(&state)).expect("Corruption applies");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert_eq!(next.player.powers.corruption, 1);
    }

    #[test]
    fn berserk_grants_power_applies_vulnerable_and_is_removed_from_hand() {
        let mut state = hand_only(BERSERK_ID);
        state.player.energy = 0;

        let next = apply_combat_action(&state, berserk_action(&state)).expect("Berserk applies");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.berserk, 1);
        assert_eq!(next.player.powers.vulnerable, 2);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn berserk_rejects_target() {
        let mut state = hand_only(BERSERK_ID);
        state.player.energy = 0;

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn berserk_uses_effective_card_cost() {
        let mut state = hand_only(BERSERK_ID);
        state.player.energy = 1;
        state.piles.hand[0].temp_cost = Some(1);

        let next = apply_combat_action(&state, berserk_action(&state))
            .expect("Berserk applies with temp cost");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.berserk, 1);
    }

    #[test]
    fn berserk_artifact_blocks_self_vulnerable_but_not_power_gain() {
        let mut state = hand_only(BERSERK_ID);
        state.player.powers.artifact = 1;

        let next = apply_combat_action(&state, berserk_action(&state)).expect("Berserk applies");

        assert_eq!(next.player.powers.artifact, 0);
        assert_eq!(next.player.powers.vulnerable, 0);
        assert_eq!(next.player.powers.berserk, 1);
    }

    #[test]
    fn berserk_round_trips_through_combat_state_json() {
        let state = hand_only(BERSERK_ID);

        let next = apply_combat_action(&state, berserk_action(&state)).expect("Berserk applies");
        let json = serde_json::to_string(&next).expect("combat state serializes");
        let restored: CombatState = serde_json::from_str(&json).expect("combat state restores");

        assert_eq!(restored.player.powers.berserk, 1);
        assert_eq!(restored, next);
    }

    #[test]
    fn berserk_event_log_records_self_vulnerable_power_gain_and_removal() {
        let state = hand_only(BERSERK_ID);

        let transition = apply_combat_action_with_events(&state, berserk_action(&state))
            .expect("Berserk applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(20),
                },
                InternalAction::SpendCardEnergy {
                    card_id: CardId::new(20),
                },
                InternalAction::ApplyPlayerVulnerable { amount: 2 },
                InternalAction::GainBerserk { amount: 1 },
                InternalAction::RemoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                },
            ]
        );
    }

    #[test]
    fn berserk_grants_energy_at_start_of_later_player_turn() {
        let mut state = hand_only(BERSERK_ID);
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 0 };
        state.piles.draw_pile.clear();

        let after_berserk =
            apply_combat_action(&state, berserk_action(&state)).expect("Berserk applies");
        let next_turn = crate::combat::end_player_turn(&after_berserk);

        assert_eq!(after_berserk.player.energy, state.player.energy);
        assert_eq!(next_turn.player.energy, next_turn.player.max_energy + 1);
        assert_eq!(next_turn.player.powers.berserk, 1);
    }

    #[test]
    fn berserk_energy_stacks() {
        let mut state = hand_only(BERSERK_ID);
        state.player.powers.berserk = 1;
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 0 };
        state.piles.draw_pile.clear();

        let after_berserk =
            apply_combat_action(&state, berserk_action(&state)).expect("Berserk applies");
        let next_turn = crate::combat::end_player_turn(&after_berserk);

        assert_eq!(next_turn.player.energy, next_turn.player.max_energy + 2);
        assert_eq!(next_turn.player.powers.berserk, 2);
    }

    #[test]
    fn berserk_adds_energy_after_ice_cream_preserves_energy() {
        let mut state = hand_only(BERSERK_ID);
        state.player.energy = 2;
        state.player.powers.berserk = 1;
        state.relics.push(Relic::IceCream);
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 0 };
        state.piles.draw_pile.clear();

        let after_berserk =
            apply_combat_action(&state, berserk_action(&state)).expect("Berserk applies");
        let next_turn = crate::combat::end_player_turn(&after_berserk);

        assert_eq!(after_berserk.player.energy, 2);
        assert_eq!(next_turn.player.energy, 4);
        assert_eq!(next_turn.player.powers.berserk, 2);
    }

    #[test]
    fn berserk_triggers_bird_faced_urn_power_heal() {
        let mut state = hand_only(BERSERK_ID);
        state.player.hp = 60;
        state.player.max_hp = 70;
        state.relics = vec![Relic::BirdFacedUrn];

        let next = apply_combat_action(&state, berserk_action(&state)).expect("Berserk applies");

        assert_eq!(next.player.hp, 60 + crate::relic::BIRD_FACED_URN_HEAL);
        assert_eq!(next.player.powers.berserk, 1);
    }

    #[test]
    fn berserk_removal_can_trigger_unceasing_top() {
        let mut state = hand_only(BERSERK_ID);
        state.relics = vec![Relic::UnceasingTop];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next = apply_combat_action(&state, berserk_action(&state)).expect("Berserk applies");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert_eq!(next.player.powers.berserk, 1);
    }

    #[test]
    fn rupture_grants_power_spends_energy_and_is_removed_from_hand() {
        let state = hand_only(RUPTURE_ID);

        let next = apply_combat_action(&state, rupture_action(&state)).expect("Rupture applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.player.powers.rupture, 1);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn rupture_uses_effective_card_cost() {
        let mut state = hand_only(RUPTURE_ID);
        state.player.energy = 0;
        state.piles.hand[0].temp_cost = Some(0);

        let next = apply_combat_action(&state, rupture_action(&state)).expect("Rupture applies");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.rupture, 1);
    }

    #[test]
    fn rupture_round_trips_through_combat_state_json() {
        let state = hand_only(RUPTURE_ID);

        let next = apply_combat_action(&state, rupture_action(&state)).expect("Rupture applies");
        let json = serde_json::to_string(&next).expect("combat state serializes");
        let restored: CombatState = serde_json::from_str(&json).expect("combat state restores");

        assert_eq!(restored.player.powers.rupture, 1);
        assert_eq!(restored, next);
    }

    #[test]
    fn rupture_event_log_records_power_gain_and_removal() {
        let state = hand_only(RUPTURE_ID);

        let transition = apply_combat_action_with_events(&state, rupture_action(&state))
            .expect("Rupture applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(20),
                },
                InternalAction::SpendCardEnergy {
                    card_id: CardId::new(20),
                },
                InternalAction::GainRupture { amount: 1 },
                InternalAction::RemoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                },
            ]
        );
    }

    #[test]
    fn rupture_card_hp_loss_gains_strength_before_later_damage() {
        let mut state = hand_only(RUPTURE_ID);
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(21), HEMOKINESIS_ID));

        let after_rupture =
            apply_combat_action(&state, rupture_action(&state)).expect("Rupture applies");
        let after_hemo = apply_combat_action(&after_rupture, hemokinesis_action(&after_rupture))
            .expect("Hemokinesis applies");

        assert_eq!(after_hemo.player.hp, after_rupture.player.hp - 2);
        assert_eq!(after_hemo.player.powers.strength, 1);
        assert_eq!(after_hemo.monsters[0].hp, after_rupture.monsters[0].hp - 16);
    }

    #[test]
    fn rupture_stacks_for_card_hp_loss() {
        let mut state = hand_only(BLOODLETTING_ID);
        state.player.powers.rupture = 2;

        let next =
            apply_combat_action(&state, bloodletting_action(&state)).expect("Bloodletting applies");

        assert_eq!(next.player.hp, state.player.hp - 3);
        assert_eq!(next.player.powers.strength, 2);
        assert_eq!(next.player.powers.rupture, 2);
    }

    #[test]
    fn rupture_does_not_trigger_when_buffer_prevents_card_hp_loss() {
        let mut state = hand_only(OFFERING_ID);
        state.player.powers.rupture = 1;
        state.player.powers.buffer = 1;

        let next = apply_combat_action(&state, offering_action(&state)).expect("Offering applies");

        assert_eq!(next.player.hp, state.player.hp);
        assert_eq!(next.player.powers.buffer, 0);
        assert_eq!(next.player.powers.strength, 0);
    }

    #[test]
    fn rupture_does_not_trigger_on_blue_candle_hp_loss() {
        let mut state = hand_only(REGRET_ID);
        state.relics = vec![Relic::BlueCandle];
        state.player.powers.rupture = 1;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Regret applies with Blue Candle");

        assert_eq!(
            next.player.hp,
            state.player.hp - crate::relic::BLUE_CANDLE_HP_LOSS
        );
        assert_eq!(next.player.powers.strength, 0);
    }

    #[test]
    fn rupture_triggers_on_havoc_played_offering_hp_loss() {
        let mut state = hand_only(HAVOC_ID);
        state.player.powers.rupture = 1;
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(33), OFFERING_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Havoc applies");

        assert_eq!(next.player.hp, state.player.hp - 6);
        assert_eq!(next.player.powers.strength, 1);
    }

    #[test]
    fn juggernaut_grants_power_spends_two_and_is_removed_from_hand() {
        let mut state = hand_only(JUGGERNAUT_ID);
        state.player.energy = 2;

        let next =
            apply_combat_action(&state, juggernaut_action(&state)).expect("Juggernaut applies");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.juggernaut, 5);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn juggernaut_deals_damage_after_block_gain_and_stacks() {
        let mut state = hand_only(JUGGERNAUT_ID);
        state.player.energy = 2;
        state.player.powers.juggernaut = 5;

        let after_power =
            apply_combat_action(&state, juggernaut_action(&state)).expect("Juggernaut applies");
        let mut block_state = after_power.clone();
        block_state.piles.hand = vec![CardInstance::new(CardId::new(30), DEFEND_R_ID)];
        block_state.player.energy = 1;
        let monster_hp = block_state.monsters[0].hp;

        let transition = apply_combat_action_with_events(
            &block_state,
            CombatAction::PlayCard {
                card_id: CardId::new(30),
                target: None,
            },
        )
        .expect("Defend applies");

        assert_eq!(after_power.player.powers.juggernaut, 10);
        assert_eq!(transition.state.player.block, 5);
        assert_eq!(transition.state.monsters[0].hp, monster_hp - 10);
        assert!(transition
            .event_log
            .contains(&InternalAction::DealUnmodifiedDamage {
                target: MonsterId::new(1),
                amount: 10,
            }));
    }

    #[test]
    fn juggernaut_does_not_trigger_when_block_gain_is_zero() {
        let mut state = hand_only(DEFEND_R_ID);
        state.player.powers.juggernaut = 5;
        state.player.powers.frail = 1;
        state.player.powers.dexterity = -5;
        let monster_hp = state.monsters[0].hp;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Defend applies");

        assert_eq!(next.player.block, 0);
        assert_eq!(next.monsters[0].hp, monster_hp);
    }

    #[test]
    fn juggernaut_triggers_from_end_turn_metallicize_block() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.juggernaut = 5;
        state.player.powers.metallicize = 4;
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 0 };
        state.piles.draw_pile.clear();
        let monster_hp = state.monsters[0].hp;

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.monsters[0].hp, monster_hp - 5);
    }

    #[test]
    fn juggernaut_event_log_records_power_gain_and_removal() {
        let mut state = hand_only(JUGGERNAUT_ID);
        state.player.energy = 2;

        let transition = apply_combat_action_with_events(&state, juggernaut_action(&state))
            .expect("Juggernaut applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(20),
                },
                InternalAction::SpendCardEnergy {
                    card_id: CardId::new(20),
                },
                InternalAction::GainJuggernaut { amount: 5 },
                InternalAction::RemoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                },
            ]
        );
    }

    #[test]
    fn juggernaut_triggers_bird_faced_urn_power_heal() {
        let mut state = hand_only(JUGGERNAUT_ID);
        state.player.hp = 60;
        state.player.max_hp = 70;
        state.player.energy = 2;
        state.relics = vec![Relic::BirdFacedUrn];

        let next =
            apply_combat_action(&state, juggernaut_action(&state)).expect("Juggernaut applies");

        assert_eq!(next.player.hp, 60 + crate::relic::BIRD_FACED_URN_HEAL);
        assert_eq!(next.player.powers.juggernaut, 5);
    }

    #[test]
    fn juggernaut_removal_can_trigger_unceasing_top() {
        let mut state = hand_only(JUGGERNAUT_ID);
        state.player.energy = 2;
        state.relics = vec![Relic::UnceasingTop];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next =
            apply_combat_action(&state, juggernaut_action(&state)).expect("Juggernaut applies");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert_eq!(next.player.powers.juggernaut, 5);
    }

    #[test]
    fn brutality_grants_power_and_is_removed_from_hand() {
        let mut state = hand_only(BRUTALITY_ID);
        state.player.energy = 0;

        let next =
            apply_combat_action(&state, brutality_action(&state)).expect("Brutality applies");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.brutality, 1);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn brutality_rejects_target() {
        let state = hand_only(BRUTALITY_ID);

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn brutality_uses_effective_card_cost() {
        let mut state = hand_only(BRUTALITY_ID);
        state.player.energy = 1;
        state.piles.hand[0].temp_cost = Some(1);

        let next = apply_combat_action(&state, brutality_action(&state))
            .expect("Brutality applies with temp cost");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.brutality, 1);
    }

    #[test]
    fn brutality_round_trips_through_combat_state_json() {
        let state = hand_only(BRUTALITY_ID);

        let next =
            apply_combat_action(&state, brutality_action(&state)).expect("Brutality applies");
        let json = serde_json::to_string(&next).expect("combat state serializes");
        let restored: CombatState = serde_json::from_str(&json).expect("combat state restores");

        assert_eq!(restored.player.powers.brutality, 1);
        assert_eq!(restored, next);
    }

    #[test]
    fn brutality_event_log_records_power_gain_and_removal() {
        let state = hand_only(BRUTALITY_ID);

        let transition = apply_combat_action_with_events(&state, brutality_action(&state))
            .expect("Brutality applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(20),
                },
                InternalAction::SpendCardEnergy {
                    card_id: CardId::new(20),
                },
                InternalAction::GainBrutality { amount: 1 },
                InternalAction::RemoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                },
            ]
        );
    }

    #[test]
    fn brutality_loses_hp_and_draws_before_normal_refill_at_start_of_later_player_turn() {
        let mut state = hand_only(BRUTALITY_ID);
        state.player.hp = 40;
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 0 };
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
            CardInstance::new(CardId::new(32), STRIKE_R_ID),
            CardInstance::new(CardId::new(33), DEFEND_R_ID),
            CardInstance::new(CardId::new(34), STRIKE_R_ID),
            CardInstance::new(CardId::new(35), DEFEND_R_ID),
        ];

        let after_brutality =
            apply_combat_action(&state, brutality_action(&state)).expect("Brutality applies");
        let next_turn = crate::combat::end_player_turn(&after_brutality);

        assert_eq!(next_turn.player.hp, 39);
        assert_eq!(next_turn.player.powers.brutality, 1);
        assert_eq!(next_turn.piles.hand.len(), 5);
        assert_eq!(
            next_turn
                .piles
                .hand
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![
                CardId::new(35),
                CardId::new(34),
                CardId::new(33),
                CardId::new(32),
                CardId::new(31),
            ]
        );
        assert_eq!(
            next_turn
                .piles
                .draw_pile
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(30)]
        );
    }

    #[test]
    fn brutality_stacks_start_turn_hp_loss_and_draw() {
        let mut state = hand_only(BRUTALITY_ID);
        state.player.hp = 40;
        state.player.powers.brutality = 1;
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 0 };
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
            CardInstance::new(CardId::new(32), STRIKE_R_ID),
            CardInstance::new(CardId::new(33), DEFEND_R_ID),
            CardInstance::new(CardId::new(34), STRIKE_R_ID),
            CardInstance::new(CardId::new(35), DEFEND_R_ID),
            CardInstance::new(CardId::new(36), STRIKE_R_ID),
        ];

        let after_brutality =
            apply_combat_action(&state, brutality_action(&state)).expect("Brutality applies");
        let next_turn = crate::combat::end_player_turn(&after_brutality);

        assert_eq!(next_turn.player.hp, 38);
        assert_eq!(next_turn.player.powers.brutality, 2);
        assert_eq!(next_turn.piles.hand.len(), 5);
        assert_eq!(
            next_turn
                .piles
                .draw_pile
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(30), CardId::new(31)]
        );
    }

    #[test]
    fn brutality_triggers_bird_faced_urn_power_heal() {
        let mut state = hand_only(BRUTALITY_ID);
        state.player.hp = 60;
        state.player.max_hp = 70;
        state.relics = vec![Relic::BirdFacedUrn];

        let next =
            apply_combat_action(&state, brutality_action(&state)).expect("Brutality applies");

        assert_eq!(next.player.hp, 60 + crate::relic::BIRD_FACED_URN_HEAL);
        assert_eq!(next.player.powers.brutality, 1);
    }

    #[test]
    fn brutality_removal_can_trigger_unceasing_top() {
        let mut state = hand_only(BRUTALITY_ID);
        state.relics = vec![Relic::UnceasingTop];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next =
            apply_combat_action(&state, brutality_action(&state)).expect("Brutality applies");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert_eq!(next.player.powers.brutality, 1);
    }

    #[test]
    fn double_tap_has_expected_base_definition_and_rarity() {
        let definition = get_card_definition(DOUBLE_TAP_ID).expect("Double Tap definition exists");

        assert_eq!(definition.cost, 1);
        assert_eq!(definition.card_type, CardType::Skill);
        assert_eq!(definition.target, crate::TargetRequirement::None);
        assert_eq!(
            crate::content::cards::card_type_and_rarity(DOUBLE_TAP_ID),
            Some((CardType::Skill, crate::card::CardRarity::Rare))
        );
    }

    #[test]
    fn double_tap_gains_pending_next_attack_replay_and_discards() {
        let state = hand_only(DOUBLE_TAP_ID);

        let next =
            apply_combat_action(&state, double_tap_action(&state)).expect("Double Tap applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.double_tap_pending, 1);
        assert!(next.piles.hand.is_empty());
        assert_eq!(next.piles.discard_pile.len(), 1);
        assert_eq!(next.piles.discard_pile[0].content_id, DOUBLE_TAP_ID);
    }

    #[test]
    fn double_tap_rejects_target() {
        let state = hand_only(DOUBLE_TAP_ID);

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn double_tap_is_listed_as_no_target_legal_action() {
        let state = hand_only(DOUBLE_TAP_ID);

        assert!(
            crate::combat::legal_combat_actions(&state).contains(&CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            })
        );
    }

    #[test]
    fn double_tap_uses_effective_card_cost() {
        let mut state = hand_only(DOUBLE_TAP_ID);
        state.player.energy = 2;
        state.piles.hand[0].temp_cost = Some(2);

        let next = apply_combat_action(&state, double_tap_action(&state))
            .expect("Double Tap applies with temp cost");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.double_tap_pending, 1);
    }

    #[test]
    fn double_tap_pending_round_trips_through_combat_state_json() {
        let state = hand_only(DOUBLE_TAP_ID);

        let next =
            apply_combat_action(&state, double_tap_action(&state)).expect("Double Tap applies");
        let json = serde_json::to_string(&next).expect("combat state serializes");
        let restored: CombatState = serde_json::from_str(&json).expect("combat state restores");

        assert_eq!(restored.double_tap_pending, 1);
        assert_eq!(restored, next);
    }

    #[test]
    fn double_tap_event_log_records_pending_gain_and_discard() {
        let state = hand_only(DOUBLE_TAP_ID);

        let transition = apply_combat_action_with_events(&state, double_tap_action(&state))
            .expect("Double Tap applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(20),
                },
                InternalAction::SpendCardEnergy {
                    card_id: CardId::new(20),
                },
                InternalAction::GainDoubleTap { amount: 1 },
                InternalAction::MoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn double_tap_triggers_gremlin_nob_skill_enrage() {
        let mut state = CombatState::gremlin_nob_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DOUBLE_TAP_ID)];

        let next =
            apply_combat_action(&state, double_tap_action(&state)).expect("Double Tap applies");

        assert_eq!(next.monsters[0].powers.anger, 2);
        assert_eq!(next.double_tap_pending, 1);
    }

    #[test]
    fn double_tap_pending_duplicates_next_attack_without_extra_energy() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), DOUBLE_TAP_ID),
            CardInstance::new(CardId::new(21), STRIKE_R_ID),
        ];

        let after_double_tap =
            apply_combat_action(&state, double_tap_action(&state)).expect("Double Tap applies");
        let after_strike = apply_combat_action(&after_double_tap, strike_action(&after_double_tap))
            .expect("Strike applies");

        assert_eq!(after_strike.monsters[0].hp, state.monsters[0].hp - 12);
        assert_eq!(after_strike.player.energy, state.player.energy - 2);
        assert_eq!(after_strike.double_tap_pending, 0);
        assert_eq!(after_strike.piles.discard_pile.len(), 2);
    }

    #[test]
    fn double_tap_pending_does_not_consume_on_non_attack() {
        let mut state = hand_only(DEFEND_R_ID);
        state.double_tap_pending = 1;

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(next.player.block, 5);
        assert_eq!(next.double_tap_pending, 1);
    }

    #[test]
    fn stacked_double_tap_pending_replays_next_attack_once_per_stack() {
        let mut state = hand_only(STRIKE_R_ID);
        state.double_tap_pending = 2;

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 18);
        assert_eq!(next.double_tap_pending, 0);
    }

    #[test]
    fn double_tap_attack_event_log_consumes_pending_before_replayed_effect() {
        let mut state = hand_only(STRIKE_R_ID);
        state.double_tap_pending = 1;

        let transition =
            apply_combat_action_with_events(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::ConsumeDoubleTap,
                InternalAction::PlayCard {
                    card_id: CardId::new(20),
                },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(CardId::new(20)),
                        target: MonsterId::new(1),
                        amount: 6,
                    },
                },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(CardId::new(20)),
                        target: MonsterId::new(1),
                        amount: 6,
                    },
                },
                InternalAction::MoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn fire_breathing_grants_power_spends_one_and_is_removed_from_hand() {
        let state = hand_only(FIRE_BREATHING_ID);

        let next = apply_combat_action(&state, fire_breathing_action(&state))
            .expect("Fire Breathing applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.player.powers.fire_breathing, 6);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn fire_breathing_rejects_target() {
        let state = hand_only(FIRE_BREATHING_ID);

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn fire_breathing_uses_effective_card_cost() {
        let mut state = hand_only(FIRE_BREATHING_ID);
        state.player.energy = 2;
        state.piles.hand[0].temp_cost = Some(2);

        let next = apply_combat_action(&state, fire_breathing_action(&state))
            .expect("Fire Breathing applies with temp cost");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.fire_breathing, 6);
    }

    #[test]
    fn fire_breathing_round_trips_through_combat_state_json() {
        let state = hand_only(FIRE_BREATHING_ID);

        let next = apply_combat_action(&state, fire_breathing_action(&state))
            .expect("Fire Breathing applies");
        let json = serde_json::to_string(&next).expect("combat state serializes");
        let restored: CombatState = serde_json::from_str(&json).expect("combat state restores");

        assert_eq!(restored.player.powers.fire_breathing, 6);
        assert_eq!(restored, next);
    }

    #[test]
    fn fire_breathing_event_log_records_power_gain_and_removal() {
        let state = hand_only(FIRE_BREATHING_ID);

        let transition = apply_combat_action_with_events(&state, fire_breathing_action(&state))
            .expect("Fire Breathing applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(20),
                },
                InternalAction::SpendCardEnergy {
                    card_id: CardId::new(20),
                },
                InternalAction::GainFireBreathing { amount: 6 },
                InternalAction::RemoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                },
            ]
        );
    }

    #[test]
    fn fire_breathing_drawn_status_deals_six_to_all_living_monsters() {
        let mut state = two_monster_hand(FIRE_BREATHING_ID);
        state.piles.hand.clear();
        state.player.powers.fire_breathing = 6;
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), DAZED_ID)];
        let before = state
            .monsters
            .iter()
            .map(|monster| monster.hp)
            .collect::<Vec<_>>();

        player_draw_cards(&mut state, 1);

        assert_eq!(state.piles.hand[0].content_id, DAZED_ID);
        assert_eq!(state.monsters[0].hp, before[0] - 6);
        assert_eq!(state.monsters[1].hp, before[1] - 6);
    }

    #[test]
    fn fire_breathing_drawn_curse_uses_stacked_damage() {
        let mut state = hand_only(FIRE_BREATHING_ID);
        state.piles.hand.clear();
        state.player.powers.fire_breathing = 12;
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), REGRET_ID)];
        let before = state.monsters[0].hp;

        player_draw_cards(&mut state, 1);

        assert_eq!(state.piles.hand[0].content_id, REGRET_ID);
        assert_eq!(state.monsters[0].hp, before - 12);
    }

    #[test]
    fn fire_breathing_does_not_trigger_on_drawn_attack() {
        let mut state = hand_only(FIRE_BREATHING_ID);
        state.piles.hand.clear();
        state.player.powers.fire_breathing = 6;
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];
        let before = state.monsters[0].hp;

        player_draw_cards(&mut state, 1);

        assert_eq!(state.piles.hand[0].content_id, STRIKE_R_ID);
        assert_eq!(state.monsters[0].hp, before);
    }

    #[test]
    fn fire_breathing_triggers_bird_faced_urn_power_heal() {
        let mut state = hand_only(FIRE_BREATHING_ID);
        state.player.hp = 60;
        state.player.max_hp = 70;
        state.relics = vec![Relic::BirdFacedUrn];

        let next = apply_combat_action(&state, fire_breathing_action(&state))
            .expect("Fire Breathing applies");

        assert_eq!(next.player.hp, 60 + crate::relic::BIRD_FACED_URN_HEAL);
        assert_eq!(next.player.powers.fire_breathing, 6);
    }

    #[test]
    fn fire_breathing_removal_can_trigger_unceasing_top() {
        let mut state = hand_only(FIRE_BREATHING_ID);
        state.relics = vec![Relic::UnceasingTop];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next = apply_combat_action(&state, fire_breathing_action(&state))
            .expect("Fire Breathing applies");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert_eq!(next.player.powers.fire_breathing, 6);
    }

    #[test]
    fn exhume_is_legal_only_with_non_exhume_exhausted_card() {
        let mut state = hand_only(EXHUME_ID);

        assert!(
            !crate::combat::legal_combat_actions(&state).contains(&exhume_action(&state)),
            "empty exhaust pile should not offer Exhume"
        );

        state
            .piles
            .exhaust_pile
            .push(CardInstance::new(CardId::new(30), EXHUME_ID));
        assert!(
            !crate::combat::legal_combat_actions(&state).contains(&exhume_action(&state)),
            "only Exhume in exhaust should not offer Exhume"
        );

        state
            .piles
            .exhaust_pile
            .push(CardInstance::new(CardId::new(31), STRIKE_R_ID));
        assert!(crate::combat::legal_combat_actions(&state).contains(&exhume_action(&state)));
    }

    #[test]
    fn exhume_spends_effective_cost_and_opens_exhaust_pile_select() {
        let mut state = hand_only(EXHUME_ID);
        state.player.energy = 2;
        state.piles.hand[0].temp_cost = Some(2);
        state
            .piles
            .exhaust_pile
            .push(CardInstance::new(CardId::new(30), STRIKE_R_ID));

        let next = apply_combat_action(&state, exhume_action(&state)).expect("Exhume opens select");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.piles.hand[0].content_id, EXHUME_ID);
        assert_eq!(
            next.exhaust_select.as_ref().map(|select| select.purpose),
            Some(crate::combat::ExhaustSelectPurpose::ExhumeReturnToHand)
        );
        assert_eq!(
            next.exhaust_select
                .as_ref()
                .and_then(|select| select.source_card_id),
            Some(CardId::new(20))
        );
    }

    #[test]
    fn exhume_select_purpose_round_trips_through_json() {
        let mut state = hand_only(EXHUME_ID);
        state
            .piles
            .exhaust_pile
            .push(CardInstance::new(CardId::new(30), STRIKE_R_ID));

        let next = apply_combat_action(&state, exhume_action(&state)).expect("Exhume opens select");
        let json = serde_json::to_string(&next).expect("combat state serializes");
        let restored: CombatState = serde_json::from_str(&json).expect("combat state restores");

        assert_eq!(restored.exhaust_select, next.exhaust_select);
        assert_eq!(restored, next);
    }

    #[test]
    fn exhume_confirm_returns_selected_exhausted_card_and_exhausts_source() {
        let mut state = hand_only(EXHUME_ID);
        state.piles.exhaust_pile = vec![
            CardInstance::new(CardId::new(30), EXHUME_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
            CardInstance::new(CardId::new(32), STRIKE_R_ID),
        ];

        let mut next =
            apply_combat_action(&state, exhume_action(&state)).expect("Exhume opens select");
        assert_eq!(exhaust_select_ui_to_hand_index(&next, 0), Ok(1));
        choose_exhaust_select(&mut next, 0).expect("choose Defend");
        confirm_exhaust_select(&mut next).expect("confirm Exhume select");

        assert!(next.exhaust_select.is_none());
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(31) && card.content_id == DEFEND_R_ID));
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20) && card.content_id == EXHUME_ID));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(31)));
    }

    #[test]
    fn exhume_source_exhaust_uses_existing_on_exhaust_hooks() {
        let mut state = hand_only(EXHUME_ID);
        state.player.powers.feel_no_pain = 1;
        state
            .piles
            .exhaust_pile
            .push(CardInstance::new(CardId::new(30), STRIKE_R_ID));

        let mut next =
            apply_combat_action(&state, exhume_action(&state)).expect("Exhume opens select");
        choose_exhaust_select(&mut next, 0).expect("choose Strike");
        confirm_exhaust_select(&mut next).expect("confirm Exhume select");

        assert_eq!(next.player.block, 3);
    }

    #[test]
    fn card_exhausted_event_log_records_on_exhaust_hook() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(25), TRUE_GRIT_ID),
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
        ];

        let transition = apply_combat_action_with_events(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("True Grit applies");

        assert!(transition
            .event_log
            .contains(&InternalAction::CardExhausted {
                card_id: CardId::new(20),
            }));
    }

    #[test]
    fn pommel_strike_deals_nine_damage_draws_one_and_moves_to_discard() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), POMMEL_STRIKE_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Pommel Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 9);
        assert_eq!(next.player.energy, state.player.energy - 1);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(30)));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn pommel_strike_plus_deals_twelve_damage() {
        let state = hand_only(POMMEL_STRIKE_PLUS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Pommel Strike+ applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 12);
    }

    #[test]
    fn battle_trance_draws_two_sets_cannot_draw_and_moves_to_discard() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), BATTLE_TRANCE_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Battle Trance applies");

        assert_eq!(next.player.energy, state.player.energy);
        assert!(next.player.cannot_draw);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(30)));
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(31)));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn battle_trance_plus_draws_three_cards() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), BATTLE_TRANCE_PLUS_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
            CardInstance::new(CardId::new(32), BASH_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Battle Trance+ applies");

        assert_eq!(next.piles.hand.len(), 3);
        assert!(next.player.cannot_draw);
    }

    #[test]
    fn seeing_red_spends_one_and_gains_two_energy() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), SEEING_RED_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Seeing Red applies");

        assert_eq!(next.player.energy, state.player.energy + 1);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn seeing_red_plus_gains_two_energy_without_spending() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), SEEING_RED_PLUS_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Seeing Red+ applies");

        assert_eq!(next.player.energy, 3);
    }

    #[test]
    fn bloodletting_loses_three_hp_and_gains_two_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 0;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), BLOODLETTING_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Bloodletting applies");

        assert_eq!(next.player.hp, state.player.hp - 3);
        assert_eq!(next.player.energy, 2);
    }

    #[test]
    fn bloodletting_moves_to_discard_without_exhausting() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), BLOODLETTING_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Bloodletting applies");

        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(next.piles.exhaust_pile.is_empty());
    }

    #[test]
    fn bloodletting_event_log_records_hp_loss_before_energy_gain() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), BLOODLETTING_ID)];

        let transition = apply_combat_action_with_events(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Bloodletting applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(20),
                },
                InternalAction::SpendEnergy { amount: 0 },
                InternalAction::LoseHp {
                    amount: 3,
                    source: HpLossSource::Card(CardId::new(20)),
                },
                InternalAction::GainEnergy { amount: 2 },
                InternalAction::MoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn bloodletting_hp_loss_is_reduced_by_tungsten_rod() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::TungstenRod];
        state.piles.hand = vec![CardInstance::new(CardId::new(20), BLOODLETTING_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Bloodletting applies");

        assert_eq!(next.player.hp, state.player.hp - 2);
        assert_eq!(next.player.energy, state.player.energy + 2);
    }

    #[test]
    fn bloodletting_hp_loss_consumes_buffer_without_losing_hp() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.buffer = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), BLOODLETTING_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Bloodletting applies");

        assert_eq!(next.player.hp, state.player.hp);
        assert_eq!(next.player.powers.buffer, 0);
        assert_eq!(next.player.energy, state.player.energy + 2);
    }

    #[test]
    fn battle_trance_blocks_later_draw_from_shrug_it_off() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), BATTLE_TRANCE_ID),
            CardInstance::new(CardId::new(25), SHRUG_IT_OFF_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(32), BASH_ID),
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
        ];

        let after_trance = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Battle Trance applies");
        let after_shrug = apply_combat_action(
            &after_trance,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Shrug It Off applies");

        assert_eq!(after_shrug.player.block, 8);
        assert_eq!(after_shrug.piles.hand.len(), 2);
        assert!(!after_shrug
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(32)));
    }

    #[test]
    fn inflame_grants_two_strength_and_moves_to_discard() {
        let state = hand_only(INFLAME_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Inflame applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.player.powers.strength, 2);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn inflame_plus_grants_three_strength() {
        let state = hand_only(INFLAME_PLUS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Inflame+ applies");

        assert_eq!(next.player.powers.strength, 3);
    }

    #[test]
    fn flex_grants_two_temp_strength_at_zero_cost() {
        let state = hand_only(FLEX_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Flex applies");

        assert_eq!(next.player.energy, state.player.energy);
        assert_eq!(next.player.temp_strength, 2);
        assert_eq!(next.player.powers.strength, 0);
    }

    #[test]
    fn flex_plus_grants_four_temp_strength() {
        let state = hand_only(FLEX_PLUS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Flex+ applies");

        assert_eq!(next.player.temp_strength, 4);
    }

    #[test]
    fn flex_temp_strength_boosts_strike_damage() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), FLEX_ID),
            CardInstance::new(CardId::new(21), STRIKE_R_ID),
        ];

        let after_flex = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Flex applies");

        let after_strike = apply_combat_action(
            &after_flex,
            CombatAction::PlayCard {
                card_id: CardId::new(21),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Strike applies");

        assert_eq!(after_strike.monsters[0].hp, state.monsters[0].hp - 8);
    }

    #[test]
    fn flex_temp_strength_clears_after_end_turn() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), FLEX_ID)];
        state.piles.draw_pile.clear();

        let after_flex = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Flex applies");
        assert_eq!(after_flex.player.temp_strength, 2);

        let next_turn = crate::combat::end_player_turn(&after_flex);

        assert_eq!(next_turn.player.temp_strength, 0);
    }

    #[test]
    fn limit_break_doubles_positive_strength_spends_one_and_exhausts() {
        let mut state = hand_only(LIMIT_BREAK_ID);
        state.player.powers.strength = 3;
        let limit_break_id = hand_card_id(&state, LIMIT_BREAK_ID);

        let next =
            apply_combat_action(&state, limit_break_action(&state)).expect("Limit Break applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.player.powers.strength, 6);
        assert!(!next.piles.hand.iter().any(|card| card.id == limit_break_id));
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == limit_break_id));
    }

    #[test]
    fn limit_break_with_zero_strength_keeps_zero() {
        let state = hand_only(LIMIT_BREAK_ID);

        let next =
            apply_combat_action(&state, limit_break_action(&state)).expect("Limit Break applies");

        assert_eq!(next.player.powers.strength, 0);
    }

    #[test]
    fn limit_break_uses_existing_signed_strength_semantics() {
        let mut state = hand_only(LIMIT_BREAK_ID);
        state.player.powers.strength = -2;

        let next =
            apply_combat_action(&state, limit_break_action(&state)).expect("Limit Break applies");

        assert_eq!(next.player.powers.strength, -4);
    }

    #[test]
    fn limit_break_ignores_temporary_strength() {
        let mut state = hand_only(LIMIT_BREAK_ID);
        state.player.powers.strength = 2;
        state.player.temp_strength = 3;

        let next =
            apply_combat_action(&state, limit_break_action(&state)).expect("Limit Break applies");

        assert_eq!(next.player.powers.strength, 4);
        assert_eq!(next.player.temp_strength, 3);
    }

    #[test]
    fn limit_break_uses_effective_temp_cost() {
        let mut state = hand_only(LIMIT_BREAK_ID);
        state.player.energy = 0;
        state.player.powers.strength = 2;
        state.piles.hand[0].temp_cost = Some(0);

        let next =
            apply_combat_action(&state, limit_break_action(&state)).expect("Limit Break applies");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.strength, 4);
    }

    #[test]
    fn limit_break_event_log_records_strength_gain_then_exhaust() {
        let mut state = hand_only(LIMIT_BREAK_ID);
        state.player.powers.strength = 5;
        let limit_break_id = hand_card_id(&state, LIMIT_BREAK_ID);

        let transition = apply_combat_action_with_events(&state, limit_break_action(&state))
            .expect("Limit Break applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: limit_break_id
                },
                InternalAction::SpendCardEnergy {
                    card_id: limit_break_id
                },
                InternalAction::GainStrength { amount: 5 },
                InternalAction::MoveCard {
                    card_id: limit_break_id,
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::CardExhausted {
                    card_id: limit_break_id,
                },
            ]
        );
    }

    #[test]
    fn offering_loses_six_hp_gains_two_energy_draws_three_and_exhausts() {
        let mut state = hand_only(OFFERING_ID);
        state.player.energy = 0;
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
            CardInstance::new(CardId::new(32), BASH_ID),
        ];
        let offering_id = hand_card_id(&state, OFFERING_ID);

        let next = apply_combat_action(&state, offering_action(&state)).expect("Offering applies");

        assert_eq!(next.player.hp, state.player.hp - 6);
        assert_eq!(next.player.energy, 2);
        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![BASH_ID, DEFEND_R_ID, STRIKE_R_ID]
        );
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == offering_id));
    }

    #[test]
    fn offering_event_log_records_hp_loss_energy_draw_then_exhaust() {
        let mut state = hand_only(OFFERING_ID);
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
            CardInstance::new(CardId::new(32), BASH_ID),
        ];
        let offering_id = hand_card_id(&state, OFFERING_ID);

        let transition = apply_combat_action_with_events(&state, offering_action(&state))
            .expect("Offering applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: offering_id
                },
                InternalAction::SpendEnergy { amount: 0 },
                InternalAction::LoseHp {
                    amount: 6,
                    source: HpLossSource::Card(offering_id),
                },
                InternalAction::GainEnergy { amount: 2 },
                InternalAction::DrawCards { count: 3 },
                InternalAction::MoveCard {
                    card_id: offering_id,
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::CardExhausted {
                    card_id: offering_id,
                },
            ]
        );
    }

    #[test]
    fn offering_hp_loss_is_reduced_by_tungsten_rod() {
        let mut state = hand_only(OFFERING_ID);
        state.relics = vec![Relic::TungstenRod];

        let next = apply_combat_action(&state, offering_action(&state)).expect("Offering applies");

        assert_eq!(next.player.hp, state.player.hp - 5);
        assert_eq!(next.player.energy, state.player.energy + 2);
    }

    #[test]
    fn offering_hp_loss_consumes_buffer_without_losing_hp() {
        let mut state = hand_only(OFFERING_ID);
        state.player.powers.buffer = 1;

        let next = apply_combat_action(&state, offering_action(&state)).expect("Offering applies");

        assert_eq!(next.player.hp, state.player.hp);
        assert_eq!(next.player.powers.buffer, 0);
        assert_eq!(next.player.energy, state.player.energy + 2);
    }

    #[test]
    fn offering_hp_loss_triggers_centennial_puzzle_once() {
        let mut state = hand_only(OFFERING_ID);
        state.relics = vec![Relic::CentennialPuzzle];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
            CardInstance::new(CardId::new(32), BASH_ID),
            CardInstance::new(CardId::new(33), CLEAVE_ID),
            CardInstance::new(CardId::new(34), ANGER_ID),
            CardInstance::new(CardId::new(35), SHRUG_IT_OFF_ID),
        ];

        let next = apply_combat_action(&state, offering_action(&state)).expect("Offering applies");

        assert_eq!(next.player.hp, state.player.hp - 6);
        assert_eq!(next.relic_counters.centennial_puzzle_triggers, 1);
        assert_eq!(next.piles.hand.len(), 6);
        assert!(next.piles.draw_pile.is_empty());
    }

    #[test]
    fn offering_hp_loss_triggers_runic_cube() {
        let mut state = hand_only(OFFERING_ID);
        state.relics = vec![Relic::RunicCube];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
            CardInstance::new(CardId::new(32), BASH_ID),
            CardInstance::new(CardId::new(33), CLEAVE_ID),
        ];

        let next = apply_combat_action(&state, offering_action(&state)).expect("Offering applies");

        assert_eq!(next.player.hp, state.player.hp - 6);
        assert_eq!(next.piles.hand.len(), 4);
        assert!(next.piles.draw_pile.is_empty());
    }

    #[test]
    fn offering_buffer_prevents_hp_loss_relic_draws() {
        let mut state = hand_only(OFFERING_ID);
        state.relics = vec![Relic::CentennialPuzzle, Relic::RunicCube];
        state.player.powers.buffer = 1;
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
            CardInstance::new(CardId::new(32), BASH_ID),
            CardInstance::new(CardId::new(33), CLEAVE_ID),
            CardInstance::new(CardId::new(34), ANGER_ID),
            CardInstance::new(CardId::new(35), SHRUG_IT_OFF_ID),
        ];

        let next = apply_combat_action(&state, offering_action(&state)).expect("Offering applies");

        assert_eq!(next.player.hp, state.player.hp);
        assert_eq!(next.player.powers.buffer, 0);
        assert_eq!(next.relic_counters.centennial_puzzle_triggers, 0);
        assert_eq!(next.piles.hand.len(), 3);
        assert_eq!(next.piles.draw_pile.len(), 3);
    }

    #[test]
    fn havoc_plays_top_offering_for_hp_loss_energy_draw_and_exhaust() {
        let mut state = hand_only(HAVOC_ID);
        state.piles.hand = vec![CardInstance::new(CardId::new(20), HAVOC_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(33), OFFERING_ID)];
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
            CardInstance::new(CardId::new(32), BASH_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Havoc applies");

        assert_eq!(next.player.hp, state.player.hp - 6);
        assert_eq!(next.player.energy, state.player.energy + 1);
        assert_eq!(next.piles.hand.len(), 3);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == OFFERING_ID));
    }

    #[test]
    fn spot_weakness_grants_three_strength_when_enemy_intends_attack() {
        let state = hand_only(SPOT_WEAKNESS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Spot Weakness applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.player.powers.strength, 3);
        assert_eq!(next.monsters[0].powers.weak, 0);
    }

    #[test]
    fn spot_weakness_does_nothing_on_ritual_intent() {
        let mut state = CombatState::cultist_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), SPOT_WEAKNESS_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Spot Weakness applies");

        assert_eq!(next.player.powers.strength, 0);
    }

    #[test]
    fn spot_weakness_plus_grants_four_strength() {
        let state = hand_only(SPOT_WEAKNESS_PLUS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Spot Weakness+ applies");

        assert_eq!(next.player.powers.strength, 4);
    }

    #[test]
    fn spot_weakness_boosts_follow_up_attack_damage() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), SPOT_WEAKNESS_ID),
            CardInstance::new(CardId::new(21), STRIKE_R_ID),
        ];
        state.piles.draw_pile.clear();

        let after_spot = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Spot Weakness applies");
        assert_eq!(after_spot.player.powers.strength, 3);

        let after_strike = apply_combat_action(
            &after_spot,
            CombatAction::PlayCard {
                card_id: CardId::new(21),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Strike applies");

        assert_eq!(after_strike.monsters[0].hp, state.monsters[0].hp - 9);
    }

    #[test]
    fn gremlin_nob_enrage_applies_two_anger_on_skill_play() {
        let state = CombatState::gremlin_nob_fixture();

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(next.monsters[0].powers.anger, 2);
    }

    #[test]
    fn gremlin_nob_enrage_applies_to_power_through() {
        let mut state = CombatState::gremlin_nob_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), POWER_THROUGH_ID)];

        let next = apply_combat_action(&state, power_through_action(&state))
            .expect("Power Through applies");

        assert_eq!(next.monsters[0].powers.anger, 2);
    }

    #[test]
    fn gremlin_nob_enrage_does_not_trigger_on_strike() {
        let state = CombatState::gremlin_nob_fixture();

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.monsters[0].powers.anger, 0);
    }

    #[test]
    fn gremlin_nob_enrage_stacks_on_multiple_skills() {
        let mut state = CombatState::gremlin_nob_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
        ];

        let after_first = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("first Defend applies");
        let after_second = apply_combat_action(
            &after_first,
            CombatAction::PlayCard {
                card_id: CardId::new(21),
                target: None,
            },
        )
        .expect("second Defend applies");

        assert_eq!(after_second.monsters[0].powers.anger, 4);
    }

    #[test]
    fn havoc_plays_top_strike_and_exhausts_it() {
        let mut state = hand_only(HAVOC_ID);
        state.piles.hand.clear();
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(20), HAVOC_ID));
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Havoc applies");

        assert_eq!(next.monsters[0].hp, 34);
        assert!(next.piles.draw_pile.is_empty());
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, STRIKE_R_ID);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == HAVOC_ID));
    }

    #[test]
    fn warcry_draws_puts_card_on_draw_pile_and_exhausts() {
        let mut state = hand_only(WARCRY_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), WARCRY_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let mut after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Warcry opens hand select");

        assert!(after_play.hand_select.is_some());
        choose_hand_select(&mut after_play, 0).expect("choose defend");
        confirm_hand_select(&mut after_play).expect("confirm hand select");

        assert_eq!(after_play.piles.draw_pile[0].content_id, DEFEND_R_ID);
        assert_eq!(after_play.piles.draw_pile.len(), 1);
        assert_eq!(after_play.piles.hand.len(), 1);
        assert_eq!(after_play.piles.hand[0].content_id, STRIKE_R_ID);
        assert_eq!(after_play.piles.exhaust_pile[0].content_id, WARCRY_ID);
    }

    #[test]
    fn warcry_hand_select_puts_regret_on_draw_pile_top() {
        use crate::content::cards::REGRET_ID;

        let mut state = hand_only(WARCRY_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
            CardInstance::new(CardId::new(4), DEFEND_R_ID),
            CardInstance::new(CardId::new(5), WARCRY_ID),
            CardInstance::new(CardId::new(6), REGRET_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let mut after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(5),
                target: None,
            },
        )
        .expect("Warcry opens hand select");

        choose_hand_select(&mut after_play, 4).expect("choose regret");
        confirm_hand_select(&mut after_play).expect("confirm hand select");

        assert_eq!(
            after_play.piles.draw_pile.last().unwrap().content_id,
            REGRET_ID
        );
        assert_eq!(after_play.piles.exhaust_pile[0].content_id, WARCRY_ID);
        assert!(after_play.hand_select.is_none());
    }

    #[test]
    fn warcry_plus_draws_two_cards() {
        let mut state = hand_only(WARCRY_PLUS_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), WARCRY_PLUS_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), STRIKE_R_ID),
        ];

        let mut after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Warcry+ opens hand select");

        choose_hand_select(&mut after_play, 0).expect("choose defend");
        confirm_hand_select(&mut after_play).expect("confirm hand select");

        assert_eq!(after_play.piles.hand.len(), 2);
        assert_eq!(after_play.piles.draw_pile[0].content_id, DEFEND_R_ID);
    }

    #[test]
    fn warcry_hand_select_still_allows_unupgradeable_cards() {
        let mut state = hand_only(WARCRY_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), WARCRY_ID),
            CardInstance::new(CardId::new(21), WOUND_ID),
        ];

        let after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Warcry opens hand select");

        assert_eq!(
            after_play.hand_select.as_ref().map(|select| select.purpose),
            Some(HandSelectPurpose::WarcryPutOnDraw)
        );
        assert_eq!(hand_select_ui_to_hand_index(&after_play, 0), Ok(1));
    }

    #[test]
    fn armaments_gains_block_and_opens_upgradeable_hand_select() {
        let mut state = hand_only(ARMAMENTS_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), ARMAMENTS_ID),
            CardInstance::new(CardId::new(21), STRIKE_R_ID),
            CardInstance::new(CardId::new(22), STRIKE_R_PLUS_ID),
        ];

        let after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Armaments opens hand select");

        assert_eq!(after_play.player.block, 5);
        assert_eq!(after_play.player.energy, 2);
        assert!(after_play.hand_select.is_some());
        assert_eq!(
            after_play.hand_select.as_ref().map(|select| select.purpose),
            Some(HandSelectPurpose::ArmamentsUpgrade)
        );
        assert_eq!(hand_select_ui_to_hand_index(&after_play, 0), Ok(1));
        assert_eq!(
            hand_select_ui_to_hand_index(&after_play, 1),
            Err(SimError::IllegalAction("hand select index out of range"))
        );
    }

    #[test]
    fn armaments_hand_select_skips_unupgradeable_cards_before_upgradeable_cards() {
        let mut state = hand_only(ARMAMENTS_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), ARMAMENTS_ID),
            CardInstance::new(CardId::new(21), WOUND_ID),
            CardInstance::new(CardId::new(22), STRIKE_R_ID),
        ];

        let after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Armaments opens hand select");

        assert_eq!(hand_select_ui_to_hand_index(&after_play, 0), Ok(2));
        assert_eq!(
            hand_select_ui_to_hand_index(&after_play, 1),
            Err(SimError::IllegalAction("hand select index out of range"))
        );
    }

    #[test]
    fn armaments_hand_select_purpose_round_trips_through_json() {
        let mut state = hand_only(ARMAMENTS_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), ARMAMENTS_ID),
            CardInstance::new(CardId::new(21), STRIKE_R_ID),
        ];

        let after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Armaments opens hand select");
        let json = serde_json::to_string(&after_play).expect("combat state serializes");
        let restored: CombatState = serde_json::from_str(&json).expect("combat state restores");

        assert_eq!(restored.hand_select, after_play.hand_select);
        assert_eq!(
            restored.hand_select.as_ref().map(|select| select.purpose),
            Some(HandSelectPurpose::ArmamentsUpgrade)
        );
    }

    #[test]
    fn armaments_confirm_upgrades_selected_card_and_discards_armaments() {
        let mut state = hand_only(ARMAMENTS_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), ARMAMENTS_ID),
            CardInstance::new(CardId::new(21), STRIKE_R_ID),
        ];

        let mut after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Armaments opens hand select");

        choose_hand_select(&mut after_play, 0).expect("choose Strike");
        confirm_hand_select(&mut after_play).expect("confirm Armaments select");

        assert!(after_play.hand_select.is_none());
        assert_eq!(after_play.piles.hand[0].id, CardId::new(21));
        assert_eq!(after_play.piles.hand[0].content_id, STRIKE_R_PLUS_ID);
        assert_eq!(after_play.piles.discard_pile[0].content_id, ARMAMENTS_ID);
    }

    #[test]
    fn armaments_confirm_rejects_stale_unupgradeable_selection() {
        let mut state = hand_only(ARMAMENTS_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), ARMAMENTS_ID),
            CardInstance::new(CardId::new(21), STRIKE_R_ID),
        ];

        let mut after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Armaments opens hand select");

        choose_hand_select(&mut after_play, 0).expect("choose Strike");
        after_play.piles.hand[1].content_id = STRIKE_R_PLUS_ID;

        assert_eq!(
            confirm_hand_select(&mut after_play),
            Err(SimError::IllegalAction("selected card cannot be upgraded"))
        );
    }

    #[test]
    fn armaments_without_upgradeable_cards_gains_block_and_discards() {
        let mut state = hand_only(ARMAMENTS_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), ARMAMENTS_ID),
            CardInstance::new(CardId::new(21), STRIKE_R_PLUS_ID),
        ];

        let after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Armaments resolves without hand select");

        assert_eq!(after_play.player.block, 5);
        assert!(after_play.hand_select.is_none());
        assert_eq!(
            after_play
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![ARMAMENTS_ID]
        );
    }

    #[test]
    fn headbutt_with_empty_discard_deals_damage_and_discards() {
        let state = hand_only(HEADBUTT_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Headbutt applies");

        assert_eq!(next.monsters[0].hp, 31);
        assert_eq!(next.player.energy, 2);
        assert!(next.discard_select.is_none());
        assert_eq!(next.piles.discard_pile[0].content_id, HEADBUTT_ID);
    }

    #[test]
    fn headbutt_opens_discard_select_when_discard_has_cards() {
        let mut state = hand_only(HEADBUTT_ID);
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
        ];

        let after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Headbutt opens discard select");

        assert_eq!(after_play.monsters[0].hp, 31);
        assert_eq!(after_play.player.energy, 2);
        assert_eq!(
            after_play
                .discard_select
                .as_ref()
                .map(|select| select.purpose),
            Some(DiscardSelectPurpose::HeadbuttPutOnDraw)
        );
        assert_eq!(discard_select_ui_to_discard_index(&after_play, 1), Ok(1));
        assert_eq!(after_play.piles.hand[0].content_id, HEADBUTT_ID);
    }

    #[test]
    fn headbutt_confirm_puts_selected_discard_card_on_draw_top_and_discards_headbutt() {
        let mut state = hand_only(HEADBUTT_ID);
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(40), BASH_ID)];
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
        ];

        let mut after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Headbutt opens discard select");

        choose_discard_select(&mut after_play, 1).expect("choose Defend");
        confirm_discard_select(&mut after_play).expect("confirm Headbutt select");

        assert!(after_play.discard_select.is_none());
        assert_eq!(
            after_play.piles.draw_pile.last().unwrap().content_id,
            DEFEND_R_ID
        );
        assert_eq!(
            after_play
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![STRIKE_R_ID, HEADBUTT_ID]
        );
        assert!(after_play.piles.hand.is_empty());
    }

    #[test]
    fn lethal_headbutt_with_discard_cards_does_not_open_discard_select() {
        let mut state = hand_only(HEADBUTT_ID);
        state.monsters[0].hp = 9;
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Headbutt applies");

        assert_eq!(next.phase, CombatPhase::Won);
        assert!(next.discard_select.is_none());
        assert_eq!(
            next.piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![STRIKE_R_ID, HEADBUTT_ID]
        );
    }

    #[test]
    fn headbutt_discard_select_purpose_round_trips_through_json() {
        let mut state = hand_only(HEADBUTT_ID);
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Headbutt opens discard select");
        let json = serde_json::to_string(&after_play).expect("combat state serializes");
        let restored: CombatState = serde_json::from_str(&json).expect("combat state restores");

        assert_eq!(restored.discard_select, after_play.discard_select);
        assert_eq!(
            restored
                .discard_select
                .as_ref()
                .map(|select| select.purpose),
            Some(DiscardSelectPurpose::HeadbuttPutOnDraw)
        );
    }

    #[test]
    fn dual_wield_copies_attack_to_hand_and_exhausts() {
        let mut state = hand_only(DUAL_WIELD_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), DUAL_WIELD_ID),
            CardInstance::new(CardId::new(21), ANGER_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Dual Wield applies");

        assert_eq!(
            next.piles
                .hand
                .iter()
                .filter(|card| card.content_id == ANGER_ID)
                .count(),
            2
        );
        assert_eq!(next.piles.exhaust_pile[0].content_id, DUAL_WIELD_ID);
    }

    #[test]
    fn searing_blow_deals_twelve_damage() {
        let state = hand_only(SEARING_BLOW_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Searing Blow applies");

        assert_eq!(next.monsters[0].hp, 28);
    }

    #[test]
    fn sever_soul_deals_sixteen_damage() {
        let state = hand_only(SEVER_SOUL_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Sever Soul applies");

        assert_eq!(next.monsters[0].hp, 24);
        assert_eq!(next.player.energy, 1);
        assert_eq!(next.piles.discard_pile[0].content_id, SEVER_SOUL_ID);
    }

    #[test]
    fn sever_soul_exhausts_non_attack_cards_in_hand() {
        let mut state = hand_only(SEVER_SOUL_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), SEVER_SOUL_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
            CardInstance::new(CardId::new(22), ANGER_ID),
            CardInstance::new(CardId::new(23), BATTLE_TRANCE_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Sever Soul applies");

        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![DEFEND_R_ID, BATTLE_TRANCE_ID]
        );
        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![ANGER_ID]
        );
    }

    #[test]
    fn second_wind_exhausts_non_attack_cards_gains_block_and_discards_source() {
        let mut state = hand_only(SECOND_WIND_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), SECOND_WIND_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
            CardInstance::new(CardId::new(22), ANGER_ID),
            CardInstance::new(CardId::new(23), BATTLE_TRANCE_ID),
        ];

        let next =
            apply_combat_action(&state, second_wind_action(&state)).expect("Second Wind applies");

        assert_eq!(next.player.energy, 2);
        assert_eq!(next.player.block, 10);
        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![DEFEND_R_ID, BATTLE_TRANCE_ID]
        );
        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![ANGER_ID]
        );
        assert_eq!(next.piles.discard_pile[0].content_id, SECOND_WIND_ID);
    }

    #[test]
    fn second_wind_with_no_other_non_attacks_discards_source_without_block() {
        let mut state = hand_only(SECOND_WIND_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), SECOND_WIND_ID),
            CardInstance::new(CardId::new(21), STRIKE_R_ID),
            CardInstance::new(CardId::new(22), ANGER_ID),
        ];

        let next =
            apply_combat_action(&state, second_wind_action(&state)).expect("Second Wind applies");

        assert_eq!(next.player.block, 0);
        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            Vec::<crate::ContentId>::new()
        );
        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![STRIKE_R_ID, ANGER_ID]
        );
    }

    #[test]
    fn second_wind_exhausting_sentinel_uses_existing_exhaust_hooks() {
        let mut state = hand_only(SECOND_WIND_ID);
        state.player.energy = 1;
        state.player.powers.feel_no_pain = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), SECOND_WIND_ID),
            CardInstance::new(CardId::new(21), SENTINEL_ID),
        ];

        let next =
            apply_combat_action(&state, second_wind_action(&state)).expect("Second Wind applies");

        assert_eq!(next.player.energy, 2);
        assert_eq!(next.player.block, 8);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(21)));
    }

    #[test]
    fn second_wind_dark_embrace_draw_does_not_add_new_exhaust_targets() {
        let mut state = hand_only(SECOND_WIND_ID);
        state.player.powers.dark_embrace = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), SECOND_WIND_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), BATTLE_TRANCE_ID)];

        let next =
            apply_combat_action(&state, second_wind_action(&state)).expect("Second Wind applies");

        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![BATTLE_TRANCE_ID]
        );
        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![DEFEND_R_ID]
        );
    }

    #[test]
    fn second_wind_event_log_records_exhausts_then_aggregate_block_before_source_discard() {
        let mut state = hand_only(SECOND_WIND_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), SECOND_WIND_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
            CardInstance::new(CardId::new(22), ANGER_ID),
            CardInstance::new(CardId::new(23), BATTLE_TRANCE_ID),
        ];

        let transition = apply_combat_action_with_events(&state, second_wind_action(&state))
            .expect("Second Wind applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(20)
                },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::MoveCard {
                    card_id: CardId::new(21),
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::MoveCard {
                    card_id: CardId::new(23),
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::GainBlock { amount: 10 },
                InternalAction::MoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
                InternalAction::CardExhausted {
                    card_id: CardId::new(21)
                },
                InternalAction::CardExhausted {
                    card_id: CardId::new(23)
                },
            ]
        );
    }

    #[test]
    fn fiend_fire_deals_once_per_other_card_then_exhausts_others_and_source() {
        let mut state = hand_only(FIEND_FIRE_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), FIEND_FIRE_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
            CardInstance::new(CardId::new(22), ANGER_ID),
            CardInstance::new(CardId::new(23), BATTLE_TRANCE_ID),
        ];

        let next =
            apply_combat_action(&state, fiend_fire_action(&state)).expect("Fiend Fire applies");

        assert_eq!(next.player.energy, 1);
        assert_eq!(next.monsters[0].hp, 19);
        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![DEFEND_R_ID, ANGER_ID, BATTLE_TRANCE_ID, FIEND_FIRE_ID]
        );
        assert!(next.piles.hand.is_empty());
    }

    #[test]
    fn fiend_fire_with_no_other_cards_exhausts_source_without_damage() {
        let state = hand_only(FIEND_FIRE_ID);

        let next =
            apply_combat_action(&state, fiend_fire_action(&state)).expect("Fiend Fire applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp);
        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![FIEND_FIRE_ID]
        );
    }

    #[test]
    fn fiend_fire_exhausting_sentinel_uses_existing_exhaust_hooks() {
        let mut state = hand_only(FIEND_FIRE_ID);
        state.player.energy = 2;
        state.player.powers.feel_no_pain = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), FIEND_FIRE_ID),
            CardInstance::new(CardId::new(21), SENTINEL_ID),
        ];

        let next =
            apply_combat_action(&state, fiend_fire_action(&state)).expect("Fiend Fire applies");

        assert_eq!(next.player.energy, 2);
        assert_eq!(next.player.block, 6);
        assert_eq!(next.monsters[0].hp, 33);
    }

    #[test]
    fn fiend_fire_dark_embrace_draw_does_not_add_new_exhaust_or_damage_targets() {
        let mut state = hand_only(FIEND_FIRE_ID);
        state.player.powers.dark_embrace = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), FIEND_FIRE_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), BATTLE_TRANCE_ID)];

        let next =
            apply_combat_action(&state, fiend_fire_action(&state)).expect("Fiend Fire applies");

        assert_eq!(next.monsters[0].hp, 33);
        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![BATTLE_TRANCE_ID]
        );
        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![DEFEND_R_ID, FIEND_FIRE_ID]
        );
    }

    #[test]
    fn strange_spoon_only_rolls_for_fiend_fire_source_exhaust() {
        let mut state = hand_only(FIEND_FIRE_ID);
        state.relics = vec![Relic::StrangeSpoon];
        state.card_random_rng = Some(crate::rng::StsRng::new(123));
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), FIEND_FIRE_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
        ];
        let mut expected_rng = crate::rng::StsRng::new(123);
        let spoon_proc = expected_rng.random_bool();

        let next =
            apply_combat_action(&state, fiend_fire_action(&state)).expect("Fiend Fire applies");

        assert_eq!(
            next.card_random_rng.as_ref().expect("card rng").counter(),
            expected_rng.counter()
        );
        assert_eq!(next.piles.exhaust_pile[0].content_id, DEFEND_R_ID);
        if spoon_proc {
            assert_eq!(next.piles.discard_pile[0].content_id, FIEND_FIRE_ID);
            assert_eq!(next.piles.exhaust_pile.len(), 1);
        } else {
            assert!(next.piles.discard_pile.is_empty());
            assert_eq!(next.piles.exhaust_pile[1].content_id, FIEND_FIRE_ID);
        }
    }

    #[test]
    fn fiend_fire_event_log_records_local_exhaust_then_damage_order() {
        let mut state = hand_only(FIEND_FIRE_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), FIEND_FIRE_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
            CardInstance::new(CardId::new(22), ANGER_ID),
        ];

        let transition = apply_combat_action_with_events(&state, fiend_fire_action(&state))
            .expect("Fiend Fire applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(20)
                },
                InternalAction::SpendEnergy { amount: 2 },
                InternalAction::MoveCard {
                    card_id: CardId::new(21),
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::MoveCard {
                    card_id: CardId::new(22),
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(CardId::new(20)),
                        target: MonsterId::new(1),
                        amount: 7,
                    }
                },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(CardId::new(20)),
                        target: MonsterId::new(1),
                        amount: 7,
                    }
                },
                InternalAction::MoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::CardExhausted {
                    card_id: CardId::new(21)
                },
                InternalAction::CardExhausted {
                    card_id: CardId::new(22)
                },
                InternalAction::CardExhausted {
                    card_id: CardId::new(20)
                },
            ]
        );
    }

    #[test]
    fn rage_gains_turn_scoped_attack_block_and_discards() {
        let state = hand_only(RAGE_ID);

        let next = apply_combat_action(&state, rage_action(&state)).expect("Rage applies");

        assert_eq!(next.player.energy, 3);
        assert_eq!(next.player.temp_rage_block, 3);
        assert_eq!(next.piles.discard_pile[0].content_id, RAGE_ID);
    }

    #[test]
    fn rage_rejects_target() {
        let state = hand_only(RAGE_ID);

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(20),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::IllegalAction(
                "non-targeted card cannot have a target"
            ))
        );
    }

    #[test]
    fn attack_after_rage_gains_block_before_damage_resolution() {
        let mut state = hand_only(RAGE_ID);
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(21), STRIKE_R_ID));

        let after_rage = apply_combat_action(&state, rage_action(&state)).expect("Rage applies");
        let after_strike =
            apply_combat_action(&after_rage, strike_action(&after_rage)).expect("Strike applies");

        assert_eq!(after_strike.player.block, 3);
        assert_eq!(after_strike.monsters[0].hp, state.monsters[0].hp - 6);
    }

    #[test]
    fn rage_stacks_for_later_attacks() {
        let mut state = hand_only(RAGE_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), RAGE_ID),
            CardInstance::new(CardId::new(21), RAGE_ID),
            CardInstance::new(CardId::new(22), STRIKE_R_ID),
        ];

        let after_first = apply_combat_action(&state, rage_action(&state)).expect("Rage applies");
        let after_second =
            apply_combat_action(&after_first, rage_action(&after_first)).expect("Rage applies");
        let after_strike = apply_combat_action(&after_second, strike_action(&after_second))
            .expect("Strike applies");

        assert_eq!(after_strike.player.temp_rage_block, 6);
        assert_eq!(after_strike.player.block, 6);
    }

    #[test]
    fn rage_does_not_trigger_on_later_skill() {
        let mut state = hand_only(RAGE_ID);
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(21), DEFEND_R_ID));

        let after_rage = apply_combat_action(&state, rage_action(&state)).expect("Rage applies");
        let after_defend =
            apply_combat_action(&after_rage, defend_action(&after_rage)).expect("Defend applies");

        assert_eq!(after_defend.player.block, 5);
    }

    #[test]
    fn rage_expires_on_next_player_turn() {
        let mut state = hand_only(RAGE_ID);
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 0 };

        let after_rage = apply_combat_action(&state, rage_action(&state)).expect("Rage applies");
        let next_turn = apply_combat_action(&after_rage, CombatAction::EndTurn).expect("turn ends");

        assert_eq!(next_turn.player.temp_rage_block, 0);
    }

    #[test]
    fn rage_applies_to_attack_played_from_top_draw() {
        let mut state = hand_only(HAVOC_ID);
        state.player.temp_rage_block = 3;
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Havoc applies");

        assert_eq!(next.player.block, 3);
        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 6);
    }

    #[test]
    fn rage_event_log_records_gain_before_discard() {
        let state = hand_only(RAGE_ID);

        let transition =
            apply_combat_action_with_events(&state, rage_action(&state)).expect("Rage applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard {
                    card_id: CardId::new(20)
                },
                InternalAction::SpendEnergy { amount: 0 },
                InternalAction::GainRage { amount: 3 },
                InternalAction::MoveCard {
                    card_id: CardId::new(20),
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn unceasing_top_draws_when_played_card_empties_hand() {
        let mut state = hand_only(STRIKE_R_ID);
        state.relics = vec![Relic::UnceasingTop];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), DEFEND_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Strike applies");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, DEFEND_R_ID);
        assert!(next.piles.draw_pile.is_empty());
        assert_eq!(next.piles.discard_pile[0].content_id, STRIKE_R_ID);
    }

    #[test]
    fn unceasing_top_does_not_draw_when_other_cards_remain_in_hand() {
        let mut state = hand_only(STRIKE_R_ID);
        state.relics = vec![Relic::UnceasingTop];
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(21), DEFEND_R_ID));
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), BASH_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Strike applies");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, DEFEND_R_ID);
        assert_eq!(next.piles.draw_pile.len(), 1);
        assert_eq!(next.piles.draw_pile[0].content_id, BASH_ID);
    }

    #[test]
    fn unceasing_top_respects_cannot_draw() {
        let mut state = hand_only(STRIKE_R_ID);
        state.relics = vec![Relic::UnceasingTop];
        state.player.cannot_draw = true;
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), DEFEND_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Strike applies");

        assert!(next.piles.hand.is_empty());
        assert_eq!(next.piles.draw_pile.len(), 1);
        assert_eq!(next.piles.draw_pile[0].content_id, DEFEND_R_ID);
    }

    #[test]
    fn unceasing_top_draws_after_power_card_is_removed_from_hand() {
        let mut state = hand_only(METALLICIZE_ID);
        state.relics = vec![Relic::UnceasingTop];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Metallicize applies");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert!(next.piles.draw_pile.is_empty());
        assert!(next
            .piles
            .discard_pile
            .iter()
            .all(|card| card.content_id != METALLICIZE_ID));
    }

    fn hand_only(content_id: crate::ContentId) -> CombatState {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), content_id)];
        state
    }

    fn two_monster_hand(content_id: crate::ContentId) -> CombatState {
        use crate::content::monsters::{monster_state, FIXED_SIMPLE_MONSTER};

        let mut state = hand_only(content_id);
        state
            .monsters
            .push(monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(2)));
        state
    }

    fn strike_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_strike_id(state),
            target: Some(MonsterId::new(1)),
        }
    }

    fn hand_strike_id(state: &CombatState) -> CardId {
        hand_card_id(state, STRIKE_R_ID)
    }

    fn defend_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, DEFEND_R_ID),
            target: None,
        }
    }

    fn bash_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, BASH_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn iron_wave_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, IRON_WAVE_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn body_slam_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, BODY_SLAM_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn flame_barrier_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, FLAME_BARRIER_ID),
            target: None,
        }
    }

    fn good_instincts_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, GOOD_INSTINCTS_ID),
            target: None,
        }
    }

    fn bandage_up_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, BANDAGE_UP_ID),
            target: None,
        }
    }

    fn finesse_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, FINESSE_ID),
            target: None,
        }
    }

    fn flash_of_steel_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, FLASH_OF_STEEL_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn clash_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, CLASH_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn wild_strike_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, WILD_STRIKE_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn heavy_blade_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, HEAVY_BLADE_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn perfected_strike_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, PERFECTED_STRIKE_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn rampage_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, RAMPAGE_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn power_through_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, POWER_THROUGH_ID),
            target: None,
        }
    }

    fn infernal_blade_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, INFERNAL_BLADE_ID),
            target: None,
        }
    }

    fn entrench_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, ENTRENCH_ID),
            target: None,
        }
    }

    fn ghostly_armor_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, GHOSTLY_ARMOR_ID),
            target: None,
        }
    }

    fn reckless_charge_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, RECKLESS_CHARGE_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn pummel_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, PUMMEL_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn bludgeon_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, BLUDGEON_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn feed_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, FEED_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn carnage_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, CARNAGE_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn hemokinesis_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, HEMOKINESIS_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn blood_for_blood_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, BLOOD_FOR_BLOOD_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn dropkick_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, DROPKICK_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn impervious_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, IMPERVIOUS_ID),
            target: None,
        }
    }

    fn demon_form_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, DEMON_FORM_ID),
            target: None,
        }
    }

    fn barricade_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, BARRICADE_ID),
            target: None,
        }
    }

    fn corruption_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, CORRUPTION_ID),
            target: None,
        }
    }

    fn berserk_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, BERSERK_ID),
            target: None,
        }
    }

    fn juggernaut_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, JUGGERNAUT_ID),
            target: None,
        }
    }

    fn exhume_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, EXHUME_ID),
            target: None,
        }
    }

    fn combust_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, COMBUST_ID),
            target: None,
        }
    }

    fn rupture_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, RUPTURE_ID),
            target: None,
        }
    }

    fn brutality_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, BRUTALITY_ID),
            target: None,
        }
    }

    fn double_tap_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, DOUBLE_TAP_ID),
            target: None,
        }
    }

    fn fire_breathing_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, FIRE_BREATHING_ID),
            target: None,
        }
    }

    fn evolve_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, EVOLVE_ID),
            target: None,
        }
    }

    fn limit_break_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, LIMIT_BREAK_ID),
            target: None,
        }
    }

    fn offering_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, OFFERING_ID),
            target: None,
        }
    }

    fn bloodletting_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, BLOODLETTING_ID),
            target: None,
        }
    }

    fn clothesline_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, CLOTHESLINE_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn intimidate_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, INTIMIDATE_ID),
            target: None,
        }
    }

    fn shockwave_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, SHOCKWAVE_ID),
            target: None,
        }
    }

    fn disarm_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, DISARM_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn rage_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, RAGE_ID),
            target: None,
        }
    }

    fn reaper_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, REAPER_ID),
            target: None,
        }
    }

    fn second_wind_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, SECOND_WIND_ID),
            target: None,
        }
    }

    fn fiend_fire_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, FIEND_FIRE_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn hand_card_id(state: &CombatState, content_id: crate::ContentId) -> CardId {
        state
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == content_id)
            .expect("card is in hand")
            .id
    }
}
