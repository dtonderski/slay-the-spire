use super::{
    apply_monster_death_hooks, apply_or_queue_spikes_to_player, apply_player_vulnerable_debuff,
    checked_add_combat_value, deal_attack_damage_to_all_living, living_monster_mut,
    living_monster_mut_opt, push_malleable_block_follow_up, queue_monster_death_hooks,
    random_living_monster_id,
};
use crate::{
    action::InternalAction,
    combat::{
        damage::{deal_damage_info_to_monster_with_result, DamageInfo, DamageSource},
        CombatState,
    },
    content::monsters::{
        check_slime_boss_split, guardian_accumulate_hp_damage, wake_lagavulin_on_damage,
    },
    ids::CardId,
    SimResult,
};

pub(super) fn deal_damage(
    state: &mut CombatState,
    info: DamageInfo,
) -> SimResult<Vec<InternalAction>> {
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
        apply_player_vulnerable_debuff(state, info.target, crate::relic::HAND_DRILL_VULNERABLE)?;
    }
    check_slime_boss_split(state, info.target);
    if !still_alive {
        queue_monster_death_hooks(state, info.target)?;
    }
    apply_or_queue_spikes_to_player(state, monster_content_id, spikes)?;
    Ok(follow_ups)
}

pub(super) fn deal_damage_random_enemy(
    state: &mut CombatState,
    source: CardId,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
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
            apply_player_vulnerable_debuff(state, target, crate::relic::HAND_DRILL_VULNERABLE)?;
        }
        check_slime_boss_split(state, target);
        if !still_alive {
            apply_monster_death_hooks(state, target)?;
        }
        apply_or_queue_spikes_to_player(state, monster_content_id, spikes)?;
        return Ok(follow_ups);
    }
    Ok(Vec::new())
}

pub(super) fn deal_hand_of_greed_damage(
    state: &mut CombatState,
    info: DamageInfo,
    gold: i32,
) -> SimResult<Vec<InternalAction>> {
    if living_monster_mut_opt(state, info.target).is_none() {
        return Ok(Vec::new());
    }
    let player_powers = state.player.powers;
    let temp_strength = state.player.temp_strength;
    let relics = state.relics.clone();
    let (spikes, monster_content_id, still_alive, minion, hand_drill_applies, malleable_block) = {
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
        apply_player_vulnerable_debuff(state, info.target, crate::relic::HAND_DRILL_VULNERABLE)?;
    }
    check_slime_boss_split(state, info.target);
    if !still_alive {
        if !minion {
            checked_add_combat_value(&mut state.combat_gold_gained, gold.max(0))?;
        }
        apply_monster_death_hooks(state, info.target)?;
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
    let player_powers = state.player.powers;
    let temp_strength = state.player.temp_strength;
    let relics = state.relics.clone();
    let (hp_damage, spikes, monster_content_id, still_alive, hand_drill_applies, malleable_block) = {
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
    crate::relic::heal_combat_player_with_relics(state, hp_damage)?;
    if still_alive && hand_drill_applies {
        apply_player_vulnerable_debuff(state, info.target, crate::relic::HAND_DRILL_VULNERABLE)?;
    }
    check_slime_boss_split(state, info.target);
    if !still_alive {
        apply_monster_death_hooks(state, info.target)?;
    }
    apply_or_queue_spikes_to_player(state, monster_content_id, spikes)?;
    Ok(follow_ups)
}

pub(super) fn deal_damage_all(
    state: &mut CombatState,
    source: CardId,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    let (_, follow_ups) = deal_attack_damage_to_all_living(state, source, amount)?;
    Ok(follow_ups)
}

pub(super) fn deal_damage_all_repeated(
    state: &mut CombatState,
    source: CardId,
    amount: i32,
    times: i32,
) -> SimResult<Vec<InternalAction>> {
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
    let (hp_damage, follow_ups) = deal_attack_damage_to_all_living(state, source, amount)?;
    crate::relic::heal_combat_player_with_relics(state, hp_damage)?;
    Ok(follow_ups)
}
