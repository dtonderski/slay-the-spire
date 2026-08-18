use super::{
    add_ritual_dagger_damage_bonus, apply_or_queue_spikes_to_player, checked_add_combat_value,
    checked_combat_sum, deal_attack_damage_to_all_living, living_monster_mut,
    living_monster_mut_opt, push_attack_block_follow_ups, queue_monster_death_hooks,
    random_living_monster_id,
};
use crate::{
    action::InternalAction,
    combat::{
        card_effects::pen_nib_queue_amount,
        damage::{deal_damage_info_to_monster_with_result, DamageInfo, DamageSource},
        CombatState,
    },
    content::monsters::{
        awakened_one_is_half_dead, check_slime_boss_split, guardian_accumulate_hp_damage,
        wake_lagavulin_on_damage, DARKLING_ID,
    },
    ids::{CardId, MonsterId},
    SimError, SimResult,
};

fn apply_pen_nib_to_card_damage_info(state: &CombatState, mut info: DamageInfo) -> DamageInfo {
    if state.pen_nib_double_active && matches!(info.source, DamageSource::Card(_)) {
        info.amount = pen_nib_queue_amount(state, info.amount);
    }
    info
}

fn apply_pen_nib_to_card_damage_amount(state: &CombatState, source: CardId, amount: i32) -> i32 {
    let _ = source;
    if state.pen_nib_double_active {
        pen_nib_queue_amount(state, amount)
    } else {
        amount
    }
}

pub(super) fn deal_damage(
    state: &mut CombatState,
    info: DamageInfo,
) -> SimResult<Vec<InternalAction>> {
    if living_monster_mut_opt(state, info.target).is_none() {
        return Ok(Vec::new());
    }
    let info = apply_pen_nib_to_card_damage_info(state, info);
    let player_powers = state.player.powers;
    let temp_strength = state.player.temp_strength;
    let relics = state.relics.clone();
    let (
        spikes,
        monster_content_id,
        still_alive,
        hand_drill_applies,
        curl_up_block,
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
            relics.contains(&crate::Relic::HandDrill) && damage.broke_block,
            damage.curl_up_block,
            damage.malleable_block,
        )
    };
    let mut follow_ups = Vec::new();
    push_attack_block_follow_ups(
        state,
        &mut follow_ups,
        info.target,
        monster_content_id,
        still_alive,
        curl_up_block,
        malleable_block,
    );
    if still_alive && hand_drill_applies {
        // Hand Drill's ApplyPowerAction is addToBot from DamageAction. Queue it
        // behind the remaining card hits so a multi-hit attack cannot consume
        // the newly applied Vulnerable on a later hit.
        follow_ups.push(InternalAction::ApplyVulnerable {
            target: info.target,
            amount: crate::relic::HAND_DRILL_VULNERABLE,
        });
    }
    check_slime_boss_split(state, info.target);
    if !still_alive {
        follow_ups.extend(queue_monster_death_hooks(state, info.target)?);
    }
    apply_or_queue_spikes_to_player(state, monster_content_id, spikes)?;
    Ok(follow_ups)
}

pub(super) fn deal_body_slam_damage(
    state: &mut CombatState,
    source: CardId,
    target: crate::ids::MonsterId,
) -> SimResult<Vec<InternalAction>> {
    deal_damage(
        state,
        DamageInfo {
            source: DamageSource::Card(source),
            target,
            amount: state.player.block,
        },
    )
}

pub(super) fn deal_damage_random_enemy(
    state: &mut CombatState,
    source: CardId,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    if let Some(target) = random_living_monster_id(state) {
        let amount = apply_pen_nib_to_card_damage_amount(state, source, amount);
        let player_powers = state.player.powers;
        let temp_strength = state.player.temp_strength;
        let relics = state.relics.clone();
        let (
            spikes,
            monster_content_id,
            still_alive,
            hand_drill_applies,
            curl_up_block,
            malleable_block,
        ) = {
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
                damage.curl_up_block,
                damage.malleable_block,
            )
        };
        let mut follow_ups = Vec::new();
        push_attack_block_follow_ups(
            state,
            &mut follow_ups,
            target,
            monster_content_id,
            still_alive,
            curl_up_block,
            malleable_block,
        );
        if still_alive && hand_drill_applies {
            follow_ups.push(InternalAction::ApplyVulnerable {
                target,
                amount: crate::relic::HAND_DRILL_VULNERABLE,
            });
        }
        check_slime_boss_split(state, target);
        if !still_alive {
            follow_ups.extend(queue_monster_death_hooks(state, target)?);
        }
        apply_or_queue_spikes_to_player(state, monster_content_id, spikes)?;
        return Ok(follow_ups);
    }
    Ok(Vec::new())
}

pub(super) fn resolve_fiend_fire(
    state: &mut CombatState,
    source_card_id: CardId,
    target: MonsterId,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    use crate::action::CardPile;

    // Snapshot the other hand cards at resolve time. Dead Branch refills must
    // not be exhausted by Fiend Fire (and must not steal exhaust slots from
    // Sentinel / etc. — FIDL584b energy from Sentinel on-exhaust). Double Tap
    // copies re-snapshot, so an empty hand yields zero hits (FIDL00237).
    let mut pending_exhaust: Vec<CardId> = state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != source_card_id)
        .map(|card| card.id)
        .collect();
    let hits = pending_exhaust.len();
    let mut follow_ups = Vec::new();
    // Pick order with the same cardRandomRng pattern as ExhaustRandomHandCardExcept
    // (one random_int per remaining snapshot card), without exhausting Dead Branch
    // refills that land mid-resolve.
    while !pending_exhaust.is_empty() {
        let pick = state
            .rng
            .card_random_rng
            .random_int((pending_exhaust.len() - 1) as i32) as usize;
        let card_id = pending_exhaust.remove(pick);
        if state.piles.hand.iter().all(|card| card.id != card_id) {
            continue;
        }
        super::move_card(state, card_id, CardPile::Hand, CardPile::ExhaustPile)?;
        super::apply_on_exhaust_effects_except_bot_queued_powers(state, card_id)?;
        if state.player.powers.feel_no_pain > 0 {
            follow_ups.push(InternalAction::GainBlockFromExhaust {
                amount: state.player.powers.feel_no_pain,
            });
        }
        // Fiend Fire exhausts the whole snapshot before its addToBot
        // callbacks resolve. Keep Dead Branch as a queued callback rather than
        // materializing it during the batch; this preserves the target order
        // of Dead Branch hand fills followed by Dark Embrace draws and lets the
        // normal hand-cap path send overflow to discard.
        if let Some(db) = super::dead_branch_follow_up(state) {
            follow_ups.push(db);
        }
        follow_ups.extend(super::dark_embrace_then_necronomicurse_follow_ups(
            state, card_id,
        )?);
    }
    // A skipped selection remains owned by the closed hand-selection screen,
    // but Fiend Fire exhausts that residual selectedCards batch along with its
    // visible hand snapshot even when no preceding empty END set the legacy
    // marker (FIDL01563).
    if state.pending_hidden_hand_card_exhausts_with_fiend_fire
        || !state.pending_hidden_hand_card_until_end_turn.is_empty()
    {
        let pending_hidden = std::mem::take(&mut state.pending_hidden_hand_card_until_end_turn);
        state.pending_hidden_hand_card_exhausts_with_fiend_fire = false;
        for card in pending_hidden {
            state.piles.exhaust_pile.push(card);
            super::apply_on_exhaust_effects_except_bot_queued_powers(state, card.id)?;
            if state.player.powers.feel_no_pain > 0 {
                follow_ups.push(InternalAction::GainBlockFromExhaust {
                    amount: state.player.powers.feel_no_pain,
                });
            }
            follow_ups.extend(super::dark_embrace_then_necronomicurse_follow_ups(
                state, card.id,
            )?);
        }
    }
    if state.piles.hand.is_empty() {
        super::apply_unceasing_top_after_hand_emptied(state)?;
    }
    for _ in 0..hits {
        if living_monster_mut_opt(state, target).is_none() {
            break;
        }
        let hit_follow_ups = deal_damage(
            state,
            DamageInfo {
                source: DamageSource::Card(source_card_id),
                target,
                amount,
            },
        )?;
        follow_ups.extend(hit_follow_ups);
    }
    Ok(follow_ups)
}

pub(super) fn deal_hand_of_greed_damage(
    state: &mut CombatState,
    info: DamageInfo,
    gold: i32,
) -> SimResult<Vec<InternalAction>> {
    if living_monster_mut_opt(state, info.target).is_none() {
        return Ok(Vec::new());
    }
    let info = apply_pen_nib_to_card_damage_info(state, info);
    let player_powers = state.player.powers;
    let temp_strength = state.player.temp_strength;
    let relics = state.relics.clone();
    let (
        spikes,
        monster_content_id,
        still_alive,
        minion,
        half_dead_nonfatal,
        hand_drill_applies,
        curl_up_block,
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
            // Darkling / Awakened One first-deaths are half-dead, not true
            // fatals for Hand of Greed gold (FIDL00245, FIDL01480).
            (monster.content_id == DARKLING_ID && monster.escaped)
                || awakened_one_is_half_dead(monster),
            relics.contains(&crate::Relic::HandDrill) && damage.broke_block,
            damage.curl_up_block,
            damage.malleable_block,
        )
    };
    let mut follow_ups = Vec::new();
    push_attack_block_follow_ups(
        state,
        &mut follow_ups,
        info.target,
        monster_content_id,
        still_alive,
        curl_up_block,
        malleable_block,
    );
    if still_alive && hand_drill_applies {
        follow_ups.push(InternalAction::ApplyVulnerable {
            target: info.target,
            amount: crate::relic::HAND_DRILL_VULNERABLE,
        });
    }
    check_slime_boss_split(state, info.target);
    if !still_alive {
        let darkling_pack_defeated = monster_content_id == DARKLING_ID
            && state
                .monsters
                .iter()
                .filter(|monster| monster.content_id == DARKLING_ID)
                .all(|monster| !monster.alive);
        let nonfatal_half_dead = if monster_content_id == DARKLING_ID {
            !darkling_pack_defeated
        } else {
            half_dead_nonfatal
        };
        if !minion && !nonfatal_half_dead {
            checked_add_combat_value(&mut state.combat_gold_gained, gold.max(0))?;
        }
        follow_ups.extend(queue_monster_death_hooks(state, info.target)?);
    }
    apply_or_queue_spikes_to_player(state, monster_content_id, spikes)?;
    Ok(follow_ups)
}

pub(super) fn deal_damage_and_heal_unblocked(
    state: &mut CombatState,
    info: DamageInfo,
) -> SimResult<Vec<InternalAction>> {
    if living_monster_mut_opt(state, info.target).is_none() {
        return Ok(Vec::new());
    }
    let info = apply_pen_nib_to_card_damage_info(state, info);
    let player_powers = state.player.powers;
    let temp_strength = state.player.temp_strength;
    let relics = state.relics.clone();
    let (
        hp_damage,
        spikes,
        monster_content_id,
        still_alive,
        hand_drill_applies,
        curl_up_block,
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
            damage.curl_up_block,
            damage.malleable_block,
        )
    };
    let mut follow_ups = Vec::new();
    push_attack_block_follow_ups(
        state,
        &mut follow_ups,
        info.target,
        monster_content_id,
        still_alive,
        curl_up_block,
        malleable_block,
    );
    crate::relic::heal_combat_player_with_relics(state, hp_damage)?;
    if still_alive && hand_drill_applies {
        follow_ups.push(InternalAction::ApplyVulnerable {
            target: info.target,
            amount: crate::relic::HAND_DRILL_VULNERABLE,
        });
    }
    check_slime_boss_split(state, info.target);
    if !still_alive {
        follow_ups.extend(queue_monster_death_hooks(state, info.target)?);
    }
    apply_or_queue_spikes_to_player(state, monster_content_id, spikes)?;
    Ok(follow_ups)
}

pub(super) fn deal_feed_damage(
    state: &mut CombatState,
    info: DamageInfo,
    max_hp_gain: i32,
) -> SimResult<Vec<InternalAction>> {
    if living_monster_mut_opt(state, info.target).is_none() {
        return Ok(Vec::new());
    }
    let info = apply_pen_nib_to_card_damage_info(state, info);
    let player_powers = state.player.powers;
    let temp_strength = state.player.temp_strength;
    let relics = state.relics.clone();
    let (
        spikes,
        monster_content_id,
        still_alive,
        minion,
        hand_drill_applies,
        curl_up_block,
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
            damage.curl_up_block,
            damage.malleable_block,
        )
    };
    let mut follow_ups = Vec::new();
    push_attack_block_follow_ups(
        state,
        &mut follow_ups,
        info.target,
        monster_content_id,
        still_alive,
        curl_up_block,
        malleable_block,
    );
    if still_alive && hand_drill_applies {
        follow_ups.push(InternalAction::ApplyVulnerable {
            target: info.target,
            amount: crate::relic::HAND_DRILL_VULNERABLE,
        });
    }
    check_slime_boss_split(state, info.target);
    if !still_alive {
        // Darklings become half-dead and escape until the complete pack has
        // been defeated. Feed triggers only when this kill actually finishes
        // that pack; a half-dead Darkling must not award the max-HP gain.
        let darkling_pack_defeated = monster_content_id != DARKLING_ID
            || state
                .monsters
                .iter()
                .filter(|monster| monster.content_id == DARKLING_ID)
                .all(|monster| !monster.alive);
        if !minion && darkling_pack_defeated {
            // Mark of the Bloom blocks the heal half of increaseMaxHp, not the
            // max-HP award itself. Magic Flower still multiplies only the heal.
            let max_hp = checked_combat_sum(state.player.max_hp, max_hp_gain)?;
            let hp = if state.mark_of_bloom {
                state.player.hp
            } else {
                let hp_gain =
                    crate::relic::combat_healing_amount_with_relics(max_hp_gain, &state.relics);
                let missing_hp = max_hp
                    .checked_sub(state.player.hp)
                    .ok_or(SimError::InvalidState("combat HP bounds overflow i32"))?;
                checked_combat_sum(state.player.hp, hp_gain.min(missing_hp))?
            };
            state.player.max_hp = max_hp;
            state.player.hp = hp;
            crate::relic::sync_red_skull_strength(state)?;
        }
        follow_ups.extend(queue_monster_death_hooks(state, info.target)?);
    }
    apply_or_queue_spikes_to_player(state, monster_content_id, spikes)?;
    Ok(follow_ups)
}

pub(super) fn deal_ritual_dagger_damage(
    state: &mut CombatState,
    info: DamageInfo,
    growth: i32,
) -> SimResult<Vec<InternalAction>> {
    if living_monster_mut_opt(state, info.target).is_none() {
        return Ok(Vec::new());
    }
    let info = apply_pen_nib_to_card_damage_info(state, info);
    let player_powers = state.player.powers;
    let temp_strength = state.player.temp_strength;
    let relics = state.relics.clone();
    let (
        spikes,
        monster_content_id,
        still_alive,
        minion,
        half_dead_darkling,
        hand_drill_applies,
        curl_up_block,
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
            monster.content_id == DARKLING_ID && monster.escaped,
            relics.contains(&crate::Relic::HandDrill) && damage.broke_block,
            damage.curl_up_block,
            damage.malleable_block,
        )
    };
    let mut follow_ups = Vec::new();
    push_attack_block_follow_ups(
        state,
        &mut follow_ups,
        info.target,
        monster_content_id,
        still_alive,
        curl_up_block,
        malleable_block,
    );
    if still_alive && hand_drill_applies {
        follow_ups.push(InternalAction::ApplyVulnerable {
            target: info.target,
            amount: crate::relic::HAND_DRILL_VULNERABLE,
        });
    }
    check_slime_boss_split(state, info.target);
    if !still_alive {
        if !minion && !half_dead_darkling {
            let DamageSource::Card(source_card_id) = info.source;
            add_ritual_dagger_damage_bonus(state, source_card_id, growth)?;
        }
        follow_ups.extend(queue_monster_death_hooks(state, info.target)?);
    }
    apply_or_queue_spikes_to_player(state, monster_content_id, spikes)?;
    Ok(follow_ups)
}

pub(super) fn deal_damage_all(
    state: &mut CombatState,
    source: CardId,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    let amount = apply_pen_nib_to_card_damage_amount(state, source, amount);
    let (_, follow_ups) = deal_attack_damage_to_all_living(state, source, amount)?;
    Ok(follow_ups)
}

pub(super) fn deal_damage_all_repeated(
    state: &mut CombatState,
    source: CardId,
    amount: i32,
    times: i32,
) -> SimResult<Vec<InternalAction>> {
    let amount = apply_pen_nib_to_card_damage_amount(state, source, amount);
    let initial_malleable = state
        .monsters
        .iter()
        .map(|monster| (monster.id, monster.powers.malleable))
        .collect::<Vec<_>>();
    let mut follow_ups = Vec::new();
    for _ in 0..times {
        let (_, hit_follow_ups) = deal_attack_damage_to_all_living(state, source, amount)?;
        follow_ups.extend(
            hit_follow_ups
                .into_iter()
                .filter(|follow_up| !matches!(follow_up, InternalAction::GainMonsterBlock { .. })),
        );
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

pub(super) fn deal_damage_all_and_heal_unblocked(
    state: &mut CombatState,
    source: CardId,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    let amount = apply_pen_nib_to_card_damage_amount(state, source, amount);
    let (hp_damage, follow_ups) = deal_attack_damage_to_all_living(state, source, amount)?;
    crate::relic::heal_combat_player_with_relics(state, hp_damage)?;
    Ok(follow_ups)
}
