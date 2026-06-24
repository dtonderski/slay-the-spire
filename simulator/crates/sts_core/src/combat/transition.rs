use super::card_effects;
use crate::{
    action::{CardPile, CombatAction, InternalAction},
    card::CardType,
    combat::{
        apply_burning_blood,
        damage::{
            deal_damage_info_to_monster_with_result, deal_unmodified_damage_to_monster,
            reflect_spikes_to_player, DamageInfo, DamageSource,
        },
        validate_combat_action, CombatPhase,
    },
    content::cards::{
        get_card_definition, ANGER_ID, ANGER_PLUS_ID, BASH_ID, CLEAVE_ID, CLEAVE_PLUS_ID,
        DEFEND_R_ID, DRAMATIC_ENTRANCE_ID, POMMEL_STRIKE_ID, POMMEL_STRIKE_PLUS_ID,
        SEARING_BLOW_ID, SEARING_BLOW_PLUS_ID, SHRUG_IT_OFF_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID,
        TWIN_STRIKE_ID, TWIN_STRIKE_PLUS_ID,
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
        InternalAction::PlayCard { card_id } => {
            let card = find_hand_card(state, card_id)?;
            let definition = get_card_definition(card.content_id)
                .ok_or(SimError::UnknownContent(card.content_id))?;
            apply_enrage_on_card_type(state, definition.card_type);
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
                crate::relic::apply_player_hp_loss_relics(state, hp_before - state.player.hp);
            }
            Ok(Vec::new())
        }
        InternalAction::DealDamageAll { source, amount } => {
            let player_powers = state.player.powers;
            let temp_strength = state.player.temp_strength;
            let relics = state.relics.clone();
            let targets: Vec<(MonsterId, i32)> = state
                .monsters
                .iter()
                .filter(|monster| monster.alive)
                .map(|monster| (monster.id, monster.powers.spikes))
                .collect();
            for (target, spikes) in targets {
                let still_alive = {
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
                    monster.alive
                };
                check_slime_boss_split(state, target);
                if !still_alive {
                    crate::relic::apply_monster_death_relics(state);
                }
                if still_alive && spikes > 0 {
                    let hp_before = state.player.hp;
                    reflect_spikes_to_player(&mut state.player, &state.relics, spikes);
                    crate::relic::apply_player_hp_loss_relics(state, hp_before - state.player.hp);
                }
            }
            Ok(Vec::new())
        }
        InternalAction::GainBlock { amount } => {
            let gained = calculate_block(amount, state.player.powers);
            state.player.block += gained;
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
        InternalAction::ApplyWeak { target, amount } => {
            if let Some(monster) = living_monster_mut_opt(state, target) {
                monster.powers.weak += amount;
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
        InternalAction::DrawCards { count } => {
            player_draw_cards(state, count);
            Ok(Vec::new())
        }
        InternalAction::GainEnergy { amount } => {
            state.player.energy += amount;
            Ok(Vec::new())
        }
        InternalAction::LoseHp { amount } => {
            let mitigated = crate::relic::mitigate_hp_loss(&state.relics, amount);
            let hp_loss =
                crate::relic::apply_buffer_to_hp_loss(&mut state.player.powers, mitigated);
            state.player.hp -= hp_loss;
            crate::relic::apply_player_hp_loss_relics(state, hp_loss);
            Ok(Vec::new())
        }
        InternalAction::SetCannotDraw => {
            state.player.cannot_draw = true;
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
        InternalAction::CardExhausted { .. } => {
            apply_on_exhaust_effects(state);
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
        InternalAction::AwaitHandSelect { source_card_id } => {
            state.hand_select = Some(crate::combat::HandSelectState {
                source_card_id,
                selected_hand_index: None,
            });
            Ok(Vec::new())
        }
    }
}

pub(crate) fn apply_on_exhaust_effects(state: &mut CombatState) {
    if state.player.powers.feel_no_pain > 0 {
        state.player.block += 3 * state.player.powers.feel_no_pain;
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

    let mut follow_ups = Vec::new();

    match definition.id {
        STRIKE_R_ID
        | STRIKE_R_PLUS_ID
        | ANGER_ID
        | ANGER_PLUS_ID
        | POMMEL_STRIKE_ID
        | POMMEL_STRIKE_PLUS_ID
        | SEARING_BLOW_ID
        | SEARING_BLOW_PLUS_ID
        | BASH_ID => {
            let target = target.expect("validated havoc attack target");
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: definition.values.damage.unwrap_or(0),
                },
            });
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
    let source_card_id = state
        .hand_select
        .as_ref()
        .ok_or(SimError::IllegalAction("no hand select is open"))?
        .source_card_id;
    let selectable: Vec<usize> = state
        .piles
        .hand
        .iter()
        .enumerate()
        .filter(|(_, card)| card.id != source_card_id)
        .map(|(index, _)| index)
        .collect();
    selectable
        .get(ui_index)
        .copied()
        .ok_or(SimError::IllegalAction("hand select index out of range"))
}

pub fn confirm_hand_select(state: &mut CombatState) -> SimResult<()> {
    let hand_select = state
        .hand_select
        .take()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    let index = hand_select
        .selected_hand_index
        .ok_or(SimError::IllegalAction("hand select choice is required"))?;
    let put_back = state.piles.hand[index].id;
    if put_back == hand_select.source_card_id {
        return Err(SimError::IllegalAction(
            "cannot put Warcry on top of the draw pile",
        ));
    }
    let card = remove_card_from_pile(state, put_back, CardPile::Hand)?;
    state.piles.draw_pile.push(card);
    let warcry = remove_card_from_pile(state, hand_select.source_card_id, CardPile::Hand)?;
    state.piles.exhaust_pile.push(warcry);
    apply_on_exhaust_effects(state);
    Ok(())
}

pub fn open_discard_select(state: &mut CombatState) -> SimResult<()> {
    if state.piles.discard_pile.is_empty() {
        return Err(SimError::IllegalAction("discard pile is empty"));
    }
    state.discard_select = Some(crate::combat::DiscardSelectState {
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

pub fn open_exhaust_select(state: &mut CombatState) -> SimResult<()> {
    state.exhaust_select = Some(crate::combat::ExhaustSelectState {
        purpose: crate::combat::ExhaustSelectPurpose::Exhaust,
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
        selected_hand_indices: Vec::new(),
    });
    Ok(())
}

pub fn choose_exhaust_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    exhaust_select_ui_to_hand_index(state, ui_index)?;
    let exhaust_select = state
        .exhaust_select
        .as_mut()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    if let Some(position) = exhaust_select
        .selected_hand_indices
        .iter()
        .position(|index| *index == ui_index)
    {
        exhaust_select.selected_hand_indices.remove(position);
    } else {
        exhaust_select.selected_hand_indices.push(ui_index);
        exhaust_select.selected_hand_indices.sort_unstable();
    }
    Ok(())
}

pub fn exhaust_select_ui_to_hand_index(state: &CombatState, ui_index: usize) -> SimResult<usize> {
    state
        .exhaust_select
        .as_ref()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
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
    let mut selected = exhaust_select.selected_hand_indices;
    selected.sort_unstable();
    selected.dedup();
    for index in selected.into_iter().rev() {
        if index >= state.piles.hand.len() {
            return Err(SimError::IllegalAction("exhaust select index out of range"));
        }
        let card = state.piles.hand.remove(index);
        state.piles.exhaust_pile.push(card);
        apply_on_exhaust_effects(state);
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
    if let Some(cost) = card.temp_cost {
        return i32::from(cost);
    }
    get_card_definition(card.content_id)
        .map(|definition| i32::from(definition.cost))
        .unwrap_or(0)
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
    use super::*;
    use crate::content::cards::{
        ANGER_ID, ANGER_PLUS_ID, BASH_ID, BATTLE_TRANCE_ID, BATTLE_TRANCE_PLUS_ID, BODY_SLAM_ID,
        BURNING_PACT_ID, CLASH_ID, CLEAVE_ID, CLEAVE_PLUS_ID, CLOTHESLINE_ID, DARK_EMBRACE_ID,
        DEFEND_R_ID, DUAL_WIELD_ID, FEEL_NO_PAIN_ID, FLEX_ID, FLEX_PLUS_ID, HAVOC_ID,
        HEAVY_BLADE_ID, INFLAME_ID, INFLAME_PLUS_ID, INTIMIDATE_ID, IRON_WAVE_ID, METALLICIZE_ID,
        POMMEL_STRIKE_ID, POMMEL_STRIKE_PLUS_ID, REGRET_ID, SEARING_BLOW_ID, SEEING_RED_ID,
        SEEING_RED_PLUS_ID, SEVER_SOUL_ID, SHRUG_IT_OFF_ID, SLIMED_ID, SPOT_WEAKNESS_ID,
        SPOT_WEAKNESS_PLUS_ID, STRIKE_R_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, TWIN_STRIKE_PLUS_ID,
        WARCRY_ID, WARCRY_PLUS_ID, WHIRLWIND_ID, WHIRLWIND_PLUS_ID, WOUND_ID,
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
    fn bash_is_illegal_with_less_than_two_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;

        assert_eq!(
            apply_combat_action(&state, bash_action(&state)),
            Err(SimError::IllegalAction("card is unaffordable"))
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

    fn clash_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, CLASH_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn heavy_blade_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, HEAVY_BLADE_ID),
            target: Some(MonsterId::new(1)),
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
