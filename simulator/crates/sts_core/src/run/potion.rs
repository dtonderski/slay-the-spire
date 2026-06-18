use crate::{
    combat::damage::deal_unmodified_damage_to_monster,
    combat::CombatPhase,
    potion::{
        Potion, BLOCK_POTION_BLOCK, FEAR_POTION_WEAK, FIRE_POTION_DAMAGE, GAMBLE_POTION_LOSS_GOLD,
        GAMBLE_POTION_WIN_GOLD,
    },
    rng::{RngStream, SimulatorRng},
    RunAction, RunPhase, RunState, SimError, SimResult,
};

pub fn validate_potion_action(run: &RunState, action: RunAction) -> SimResult<()> {
    match action {
        RunAction::UsePotion { slot, target } => {
            let potion = run
                .potions
                .get(slot)
                .ok_or(SimError::IllegalAction("potion slot is not available"))?;

            if potion.requires_combat() {
                if run.phase != RunPhase::Combat {
                    return Err(SimError::IllegalAction("potion use requires combat phase"));
                }
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
                Potion::Fear => {
                    let target = target.expect("validated fear potion target");
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let monster = combat
                        .monsters
                        .iter_mut()
                        .find(|monster| monster.id == target)
                        .expect("validated potion target");
                    monster.powers.weak += FEAR_POTION_WEAK;
                }
                Potion::Gamble => {
                    let mut rng = SimulatorRng::new(next.potion_rng_seed);
                    let win = rng.next_bool(RngStream::Potion, "gamble_potion");
                    next.potion_rng_seed = rng.seed_state();
                    if win {
                        next.gold += GAMBLE_POTION_WIN_GOLD;
                    } else {
                        next.gold = (next.gold - GAMBLE_POTION_LOSS_GOLD).max(0);
                    }
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
    fn gamble_potion_wins_gold_deterministically_for_seed() {
        let mut run = RunState::map_fixture();
        run.potion_rng_seed = 42;
        run.potions.push(Potion::Gamble);
        let gold_before = run.gold;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use gamble potion");

        assert_ne!(after.potion_rng_seed, run.potion_rng_seed);
        assert!(
            after.gold == gold_before + GAMBLE_POTION_WIN_GOLD
                || after.gold == (gold_before - GAMBLE_POTION_LOSS_GOLD).max(0)
        );
        assert!(after.potions.is_empty());
    }

    #[test]
    fn gamble_potion_round_trips_rng_seed_through_run_json() {
        let mut run = RunState::map_fixture();
        run.potion_rng_seed = 99;
        run.potions.push(Potion::Gamble);

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("use gamble potion");

        let json = serde_json::to_string(&after).expect("run serializes");
        let restored: RunState = serde_json::from_str(&json).expect("run deserializes");
        assert_eq!(restored.potion_rng_seed, after.potion_rng_seed);
    }

    #[test]
    fn gamble_potion_works_outside_combat() {
        let mut run = RunState::map_fixture();
        run.potions.push(Potion::Gamble);

        apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("gamble outside combat");
    }

    #[test]
    fn fear_potion_applies_weak_and_is_consumed() {
        let mut run = RunState::combat_fixture();
        run.potions.push(Potion::Fear);
        let monster_id = run.combat.as_ref().expect("combat").monsters[0].id;

        let after = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(monster_id),
            },
        )
        .expect("use fear potion");

        let combat = after.combat.expect("combat continues");
        assert_eq!(combat.monsters[0].powers.weak, FEAR_POTION_WEAK);
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
