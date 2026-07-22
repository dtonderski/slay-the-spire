use super::{deal_attack_damage_to_all_living, living_monster_mut_opt};
use crate::{action::InternalAction, combat::CombatState, ids::CardId, SimResult};

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
