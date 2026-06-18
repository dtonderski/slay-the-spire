use crate::{
    combat::damage::deal_unmodified_damage_to_monster,
    combat::CombatPhase,
    potion::{Potion, BLOCK_POTION_BLOCK, FIRE_POTION_DAMAGE},
    RunAction, RunPhase, RunState, SimError, SimResult,
};

pub fn validate_potion_action(run: &RunState, action: RunAction) -> SimResult<()> {
    match action {
        RunAction::UsePotion { slot, target } => {
            if run.phase != RunPhase::Combat {
                return Err(SimError::IllegalAction("potion use requires combat phase"));
            }
            let potion = run
                .potions
                .get(slot)
                .ok_or(SimError::IllegalAction("potion slot is not available"))?;
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

            Ok(())
        }
        RunAction::DiscardPotion { slot } => {
            run.potions
                .get(slot)
                .ok_or(SimError::IllegalAction("potion slot is not available"))?;
            Ok(())
        }
        _ => Err(SimError::IllegalAction("not a potion action")),
    }
}

pub fn apply_potion_action(run: &RunState, action: RunAction) -> SimResult<RunState> {
    validate_potion_action(run, action)?;

    let mut next = run.clone();
    match action {
        RunAction::UsePotion { slot, target } => {
            let potion = next.potions.remove(slot);
            match potion {
                Potion::Fire => {
                    let target = target.expect("validated fire potion target");
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let monster = combat
                        .monsters
                        .iter_mut()
                        .find(|monster| monster.id == target)
                        .expect("validated potion target");
                    deal_unmodified_damage_to_monster(monster, FIRE_POTION_DAMAGE);
                    if combat.monsters.iter().all(|monster| !monster.alive) {
                        combat.phase = CombatPhase::Won;
                    }
                }
                Potion::Block => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.block += BLOCK_POTION_BLOCK;
                }
            }
            let won = next
                .combat
                .as_ref()
                .map(|combat| combat.phase == CombatPhase::Won)
                .unwrap_or(false);
            if won {
                super::reward::enter_reward_screen(&mut next);
            }
        }
        RunAction::DiscardPotion { slot } => {
            next.potions.remove(slot);
        }
        _ => unreachable!("validated potion action"),
    }

    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MonsterId;

    #[test]
    fn fire_potion_deals_twenty_damage_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Fire);
        let monster_id = run.combat.as_ref().expect("combat").monsters[0].id;
        let hp_before = run.combat.as_ref().expect("combat").monsters[0].hp;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(monster_id),
            },
        )
        .expect("use fire potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.monsters[0].hp, hp_before - FIRE_POTION_DAMAGE);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn block_potion_grants_twelve_block_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Block);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use block potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.player.block, BLOCK_POTION_BLOCK);
        assert!(after.potions.is_empty());
    }

    #[test]
    fn block_potion_rejects_target() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Block);
        let monster_id = run.combat.as_ref().expect("combat").monsters[0].id;

        let err = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(monster_id),
            },
        )
        .expect_err("block potion rejects target");

        assert_eq!(
            err,
            SimError::IllegalAction("potion does not take a target")
        );
    }

    #[test]
    fn fire_potion_can_win_combat_and_enter_rewards() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Fire);
        let combat = run.combat.as_mut().expect("combat");
        combat.monsters[0].hp = FIRE_POTION_DAMAGE;
        let monster_id = combat.monsters[0].id;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(monster_id),
            },
        )
        .expect("use lethal fire potion");

        assert_eq!(after.phase, RunPhase::Reward);
        assert!(after.combat.is_none());
        assert!(after.reward.is_some());
        assert!(after.potions.is_empty());
    }

    #[test]
    fn discard_potion_removes_selected_slot() {
        let mut run = RunState::map_fixture();
        run.potions = vec![Potion::Fire, Potion::Fire];

        let after = apply_potion_action(&run, RunAction::DiscardPotion { slot: 0 })
            .expect("discard potion");

        assert_eq!(after.potions, vec![Potion::Fire]);
    }

    #[test]
    fn use_potion_rejects_missing_slot() {
        let run = RunState::combat_fixture();
        let monster_id = run.combat.as_ref().expect("combat").monsters[0].id;

        let err = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(monster_id),
            },
        )
        .expect_err("no potion");

        assert_eq!(err, SimError::IllegalAction("potion slot is not available"));
    }

    #[test]
    fn use_potion_rejects_dead_target() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Fire);

        let err = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(MonsterId::new(999)),
            },
        )
        .expect_err("bad target");

        assert_eq!(err, SimError::IllegalAction("potion target is not alive"));
    }

    #[test]
    fn fire_potion_rejects_missing_target() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Fire);

        let err = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect_err("fire potion needs target");

        assert_eq!(err, SimError::IllegalAction("potion requires a target"));
    }
}
