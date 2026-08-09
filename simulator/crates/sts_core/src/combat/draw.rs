use crate::{
    action::InternalAction,
    card::CardType,
    combat::{damage::deal_unmodified_damage_to_monster, CombatState},
    content::{
        cards::{get_card_definition, is_curse_content_id, VOID_ID},
        monsters::{check_slime_boss_split, wake_lagavulin_on_damage},
    },
    ids::{ContentId, MonsterId},
    rng::{JavaRng, StsRng},
    CardInstance, Relic, SimResult,
};

/// CommunicationMod lists draw piles bottom-first; the game draws from the top (last entry).
fn draw_card_from_pile_top(state: &mut CombatState) -> Option<CardInstance> {
    state.piles.draw_pile.pop()
}

pub(crate) const MAX_HAND_SIZE: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DrawFollowUp {
    DrawCards { count: usize },
    FireBreathingDamage { amount: i32 },
}

impl DrawFollowUp {
    fn into_internal_action(self) -> InternalAction {
        match self {
            Self::DrawCards { count } => InternalAction::DrawCards { count },
            Self::FireBreathingDamage { amount } => InternalAction::FireBreathingDamage { amount },
        }
    }
}

pub fn draw_cards_with_sts_rng(
    state: &mut CombatState,
    count: usize,
    rng: &mut StsRng,
) -> SimResult<()> {
    let mut next = state.clone();
    let mut next_rng = rng.clone();
    let mut pending = std::collections::VecDeque::from(draw_cards_with_sts_rng_batch_deferred(
        &mut next,
        count,
        &mut next_rng,
    )?);
    while let Some(follow_up) = pending.pop_front() {
        match follow_up {
            DrawFollowUp::DrawCards { count } => pending.extend(
                draw_cards_with_sts_rng_batch_deferred(&mut next, count, &mut next_rng)?,
            ),
            DrawFollowUp::FireBreathingDamage { amount } => {
                apply_fire_breathing_damage(&mut next, amount)?;
            }
        }
    }
    *state = next;
    *rng = next_rng;
    Ok(())
}

fn draw_cards_with_sts_rng_batch_deferred(
    state: &mut CombatState,
    count: usize,
    rng: &mut StsRng,
) -> SimResult<Vec<DrawFollowUp>> {
    let mut deferred_follow_ups = Vec::new();
    for _ in 0..count {
        if state.piles.hand.len() >= MAX_HAND_SIZE {
            break;
        }
        if state.piles.draw_pile.is_empty() {
            // EmptyDeckShuffleAction no-ops when the battle is already ending
            // (last enemy dead mid-card). DrawCardAction still runs for any
            // cards already on the draw pile.
            if !combat_has_living_monster(state) {
                break;
            }
            shuffle_discard_into_draw_sts(state, rng)?;
        }

        if state.piles.draw_pile.is_empty() {
            break;
        }

        if let Some(mut card) = draw_card_from_pile_top(state) {
            let content_id = card.content_id;
            let extra_draws = evolve_extra_draw_count(state, content_id);
            apply_confusion_cost_randomization(state, &mut card);
            state.piles.hand.push(card);
            if content_id == VOID_ID {
                state.player.energy = state.player.energy.saturating_sub(1);
            }
            deferred_follow_ups.extend(draw_follow_ups_for_card(state, content_id, extra_draws)?);
        }
    }
    Ok(deferred_follow_ups)
}

/// Draw `count` cards, matching one target-game `DrawCardAction`.
///
/// `EvolvePower.onCardDraw` uses `addToBot(new DrawCardAction(...))`, so extra
/// draws from statuses are queued after the current draw action finishes—not
/// interleaved between the remaining cards of this batch. Nested status draws
/// from those follow-up actions are processed FIFO in the same way.
///
/// `FireBreathingPower.onCardDraw` similarly `addToBot`s damage, so status/curse
/// draws must not kill enemies (and release Stasis into hand) mid-batch.
pub fn draw_cards_with_combat_rng(state: &mut CombatState, count: usize) -> SimResult<()> {
    let mut next = state.clone();
    let mut pending = std::collections::VecDeque::from(draw_cards_batch_deferred_evolve_in_place(
        &mut next, count,
    )?);
    while let Some(follow_up) = pending.pop_front() {
        match follow_up {
            DrawFollowUp::DrawCards { count } => {
                pending.extend(draw_cards_batch_deferred_evolve_in_place(&mut next, count)?)
            }
            DrawFollowUp::FireBreathingDamage { amount } => {
                apply_fire_breathing_damage(&mut next, amount)?;
            }
        }
    }
    *state = next;
    Ok(())
}

/// Draw `count` cards and return source action-manager follow-ups in FIFO
/// order. The caller appends these behind actions already queued by the
/// enclosing DrawCardAction; nested draws therefore retain target queue order.
pub(crate) fn draw_cards_with_combat_rng_deferred_evolve(
    state: &mut CombatState,
    count: usize,
) -> SimResult<Vec<InternalAction>> {
    let mut next = state.clone();
    let follow_ups = draw_cards_batch_deferred_evolve_in_place(&mut next, count)?;
    *state = next;
    Ok(follow_ups
        .into_iter()
        .map(DrawFollowUp::into_internal_action)
        .collect())
}

pub(crate) fn draw_cards_with_combat_rng_without_evolve(
    state: &mut CombatState,
    count: usize,
) -> SimResult<()> {
    let mut next = state.clone();
    let mut pending =
        std::collections::VecDeque::from(draw_cards_batch_in_place(&mut next, count, false)?);
    while let Some(follow_up) = pending.pop_front() {
        match follow_up {
            // Evolve is disabled for this draw action, so this branch is a
            // fail-closed guard if a future callback source violates that
            // contract rather than silently dropping a draw.
            DrawFollowUp::DrawCards { .. } => {
                return Err(crate::SimError::InvalidState(
                    "Evolve follow-up emitted by draw-without-evolve action",
                ));
            }
            DrawFollowUp::FireBreathingDamage { amount } => {
                apply_fire_breathing_damage(&mut next, amount)?;
            }
        }
    }
    *state = next;
    Ok(())
}

fn draw_cards_batch_deferred_evolve_in_place(
    state: &mut CombatState,
    count: usize,
) -> SimResult<Vec<DrawFollowUp>> {
    draw_cards_batch_in_place(state, count, true)
}

fn combat_has_living_monster(state: &CombatState) -> bool {
    state.monsters.iter().any(|monster| monster.alive)
}

fn draw_cards_batch_in_place(
    state: &mut CombatState,
    count: usize,
    trigger_evolve: bool,
) -> SimResult<Vec<DrawFollowUp>> {
    let mut deferred_follow_ups = Vec::new();
    let had_cards_at_start =
        !state.piles.draw_pile.is_empty() || !state.piles.discard_pile.is_empty();
    for _ in 0..count {
        if state.piles.hand.len() >= MAX_HAND_SIZE {
            break;
        }
        if state.piles.draw_pile.is_empty() {
            // Target EmptyDeckShuffleAction / Sundial do not run once the battle
            // is ending (last enemy already dead mid-card, e.g. lethal Pommel
            // draw with an empty draw pile). Remaining draw attempts simply stop.
            if !combat_has_living_monster(state) {
                break;
            }
            if state.piles.discard_pile.is_empty() {
                if had_cards_at_start {
                    consume_empty_deck_shuffle_with_combat_rng(state)?;
                }
            } else {
                shuffle_discard_into_draw_with_combat_rng(state)?;
            }
        }

        if state.piles.draw_pile.is_empty() {
            break;
        }

        if let Some(mut card) = draw_card_from_pile_top(state) {
            let content_id = card.content_id;
            let extra_draws = if trigger_evolve {
                evolve_extra_draw_count(state, content_id)
            } else {
                0
            };
            apply_confusion_cost_randomization(state, &mut card);
            state.piles.hand.push(card);
            if content_id == VOID_ID {
                state.player.energy = state.player.energy.saturating_sub(1);
            }
            deferred_follow_ups.extend(draw_follow_ups_for_card(state, content_id, extra_draws)?);
        }
    }
    Ok(deferred_follow_ups)
}

pub(crate) fn consume_empty_deck_shuffle_with_combat_rng(state: &mut CombatState) -> SimResult<()> {
    let _ = state.rng.shuffle_rng.random_long();
    Ok(())
}

fn fire_breathing_triggers_on_draw(state: &CombatState, content_id: ContentId) -> bool {
    state.player.powers.fire_breathing > 0 && is_status_or_curse(content_id)
}

fn draw_follow_ups_for_card(
    state: &CombatState,
    content_id: ContentId,
    evolve_draw_count: usize,
) -> SimResult<Vec<DrawFollowUp>> {
    if !is_status_or_curse(content_id)
        || (evolve_draw_count == 0 && !fire_breathing_triggers_on_draw(state, content_id))
    {
        return Ok(Vec::new());
    }

    let mut follow_ups = Vec::new();
    for power in state.active_draw_trigger_powers()? {
        match power {
            crate::power::DrawTriggerPower::Evolve if evolve_draw_count > 0 => {
                follow_ups.push(DrawFollowUp::DrawCards {
                    count: evolve_draw_count,
                });
            }
            crate::power::DrawTriggerPower::FireBreathing
                if fire_breathing_triggers_on_draw(state, content_id) =>
            {
                // The target constructs DamageAllEnemiesAction with the power
                // amount at callback time; do not reread a mutable power when
                // this queued action later resolves.
                follow_ups.push(DrawFollowUp::FireBreathingDamage {
                    amount: state.player.powers.fire_breathing,
                });
            }
            _ => {}
        }
    }
    Ok(follow_ups)
}

/// Resolve one captured Fire Breathing pulse (one status/curse onCardDraw).
pub(crate) fn apply_fire_breathing_damage(state: &mut CombatState, amount: i32) -> SimResult<()> {
    if amount <= 0 {
        return Ok(());
    }

    let targets = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster.id)
        .collect::<Vec<MonsterId>>();
    let hand_drill = state.relics.contains(&Relic::HandDrill);

    for target in targets {
        // FireBreathingPower queues DamageAllEnemiesAction (NORMAL damage):
        // block is consumed, and Hand Drill applies when a hit breaks block
        // (FIDL00367 Deca Square → draw Status → FB clears leftover block).
        let (still_alive, broke_block) = {
            let Some(monster) = state
                .monsters
                .iter_mut()
                .find(|monster| monster.id == target && monster.alive)
            else {
                continue;
            };
            let block_before = monster.block;
            let hp_damage = deal_unmodified_damage_to_monster(monster, amount);
            let blocked = block_before - monster.block;
            let broke_block = block_before > 0 && blocked == block_before;
            wake_lagavulin_on_damage(monster, hp_damage);
            (monster.alive, broke_block)
        };
        if still_alive && hand_drill && broke_block {
            if let Some(monster) = state
                .monsters
                .iter_mut()
                .find(|monster| monster.id == target && monster.alive)
            {
                let mut powers = monster.powers;
                crate::relic::apply_monster_vulnerable_with_relics(
                    &mut powers,
                    &state.relics,
                    crate::relic::HAND_DRILL_VULNERABLE,
                )?;
                // Hand Drill's Vulnerable is applied mid-turn from a bot action;
                // it must survive the current end-of-round decay (justApplied).
                monster.vulnerable_just_applied = true;
                monster.powers = powers;
            }
        }
        check_slime_boss_split(state, target);
        if !still_alive {
            crate::combat::transition::apply_monster_death_hooks(state, target)?;
        }
    }
    Ok(())
}

pub(crate) fn evolve_extra_draw_count(state: &CombatState, content_id: ContentId) -> usize {
    // EvolvePower.onCardDraw skips when the player has No Draw.
    if state.player.powers.evolve <= 0 || state.player.cannot_draw {
        return 0;
    }
    if get_card_definition(content_id).is_some_and(|definition| {
        definition.card_type == CardType::Status && !is_curse_content_id(content_id)
    }) {
        state.player.powers.evolve as usize
    } else {
        0
    }
}

fn is_status_or_curse(content_id: crate::ContentId) -> bool {
    is_curse_content_id(content_id)
        || get_card_definition(content_id)
            .is_some_and(|definition| definition.card_type == CardType::Status)
}

pub(crate) fn apply_confusion_cost_randomization(state: &mut CombatState, card: &mut CardInstance) {
    if !state.relics.contains(&Relic::SneckoEye) && state.player.powers.confusion <= 0 {
        return;
    }
    if get_card_definition(card.content_id)
        .is_none_or(|definition| definition.keywords.unplayable || definition.cost < 0)
    {
        return;
    }
    let rng = &mut state.rng.card_random_rng;
    card.temp_cost = Some(rng.random_int(3) as u8);
}

pub(crate) fn shuffle_discard_into_draw_sts(
    state: &mut CombatState,
    rng: &mut StsRng,
) -> SimResult<()> {
    if state.piles.discard_pile.is_empty() {
        return Ok(());
    }

    state.piles.draw_pile.append(&mut state.piles.discard_pile);
    let shuffle_seed = rng.random_long();
    JavaRng::new(shuffle_seed).collections_shuffle(&mut state.piles.draw_pile);
    crate::relic::apply_shuffle_relics(state)
}

pub(crate) fn shuffle_discard_into_draw_with_combat_rng(state: &mut CombatState) -> SimResult<()> {
    if state.piles.discard_pile.is_empty() {
        return Ok(());
    }

    state.piles.draw_pile.append(&mut state.piles.discard_pile);
    let shuffle_seed = state.rng.shuffle_rng.random_long();
    JavaRng::new(shuffle_seed).collections_shuffle(&mut state.piles.draw_pile);
    crate::relic::apply_shuffle_relics(state)
}

pub(crate) fn deep_breath_shuffle_discard_into_draw_with_combat_rng(
    state: &mut CombatState,
) -> SimResult<()> {
    if state.piles.discard_pile.is_empty() {
        return Ok(());
    }

    let discard_shuffle_seed = state.rng.shuffle_rng.random_long();
    JavaRng::new(discard_shuffle_seed).collections_shuffle(&mut state.piles.discard_pile);
    state.piles.draw_pile.append(&mut state.piles.discard_pile);
    let draw_shuffle_seed = state.rng.shuffle_rng.random_long();
    JavaRng::new(draw_shuffle_seed).collections_shuffle(&mut state.piles.draw_pile);
    crate::relic::apply_shuffle_relics(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::monsters::{monster_state, DECA_A0, DONU_A0};
    use crate::ids::MonsterId;

    #[test]
    fn fire_breathing_breaking_block_applies_hand_drill_vulnerable() {
        // FIDL00367: Deca Square block + draw Status Fire Breathing + Hand Drill.
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::HandDrill);
        state.player.powers.fire_breathing = 6;
        state.monsters = vec![
            monster_state(&DECA_A0, MonsterId::new(1)),
            monster_state(&DONU_A0, MonsterId::new(2)),
        ];
        for monster in &mut state.monsters {
            monster.block = 4;
            monster.hp = 50;
            monster.max_hp = 50;
        }
        apply_fire_breathing_damage(&mut state, 6).expect("FB");
        for monster in &state.monsters {
            assert_eq!(monster.block, 0);
            assert_eq!(monster.hp, 48);
            assert_eq!(monster.powers.vulnerable, 2);
            assert!(monster.vulnerable_just_applied);
        }
    }
}
