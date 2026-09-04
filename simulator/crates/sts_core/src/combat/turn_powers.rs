use crate::combat::{CombatState, MonsterState, PlayerState};
use crate::content::cards::COMBUST_HP_LOSS;
use crate::content::monsters::{
    awakened_one_is_half_dead, check_slime_boss_split, wake_lagavulin_on_damage,
};
use crate::relic::{
    apply_hand_drill_if_broke_block, heal_combat_player_with_relics,
    heal_player_in_combat_with_relics, Relic,
};
use crate::{MonsterId, SimError, SimResult};

/// `DemonFormPower.atStartOfTurnPostDraw` applies Strength after the hand draw.
pub(crate) fn apply_demon_form_strength_post_draw(state: &mut CombatState) -> SimResult<()> {
    let amount = state.player.powers.demon_form;
    if amount <= 0 {
        return Ok(());
    }
    state.player.powers.strength =
        state
            .player
            .powers
            .strength
            .checked_add(amount)
            .ok_or(SimError::InvalidState(
                "combat integer addition overflows i32",
            ))?;
    Ok(())
}

pub fn apply_end_of_player_turn_powers(state: &mut CombatState) -> SimResult<()> {
    apply_player_end_of_turn_powers_for_combat_state(state, true)?;
    apply_end_of_turn_constricted(state)?;
    if state.player.hp <= 0 {
        return Ok(());
    }
    let mut deferred = None;
    apply_end_of_turn_combust(state, &mut deferred)?;
    if state.player.hp <= 0 {
        return Ok(());
    }
    let _ = apply_end_of_turn_bomb_timers(state, &mut deferred)?;
    Ok(())
}

pub fn apply_end_of_player_turn_powers_before_hand(state: &mut CombatState) -> SimResult<()> {
    let mut deferred = None;
    let _ = apply_end_of_player_turn_powers_before_hand_inner(state, &mut deferred, true)?;
    Ok(())
}

/// Resolve end-turn powers while retaining death callbacks until the hand
/// discard action settles. This is the action-queue path used by a normal END;
/// direct callers keep the immediate helper above.
pub(crate) fn apply_end_of_player_turn_powers_before_hand_deferred(
    state: &mut CombatState,
    deferred: &mut Vec<crate::combat::transition::DeferredMonsterDeath>,
) -> SimResult<()> {
    let _ =
        apply_end_of_player_turn_powers_before_hand_deferred_with_combust(state, deferred, true)?;
    Ok(())
}

/// `apply_combust` is false when Burn/Decay/Regret still have to auto-play.
/// Those cards are `CardQueueItem`s from `callEndOfTurnActions` and resolve
/// before `AbstractRoom.endTurn` queues Combust `LoseHPAction` (FIDL01762).
pub(crate) fn apply_end_of_player_turn_powers_before_hand_deferred_with_combust(
    state: &mut CombatState,
    deferred: &mut Vec<crate::combat::transition::DeferredMonsterDeath>,
    apply_combust: bool,
) -> SimResult<bool> {
    let mut deferred = Some(deferred);
    apply_end_of_player_turn_powers_before_hand_inner(state, &mut deferred, apply_combust)
}

fn apply_end_of_player_turn_powers_before_hand_inner(
    state: &mut CombatState,
    deferred: &mut Option<&mut Vec<crate::combat::transition::DeferredMonsterDeath>>,
    apply_combust: bool,
) -> SimResult<bool> {
    apply_player_end_of_turn_powers_for_combat_state(state, false)?;
    if state.player.hp <= 0 {
        return Ok(false);
    }
    // When Combust will run, Constricted resolves first (power-list order:
    // older Constricted before later Combust). Orichalcum block is already on
    // the player, so Constricted THORNS can consume it before Combust LoseHP
    // and the lethal all-enemy hit (FIDL00440: +6 block, Constricted 10, two
    // Combust stacks → −6 HP). Without Combust, Constricted stays after hand
    // so Metallicize can absorb Decay (FIDL00415) and leftover block can absorb
    // Burn before Constricted THORNS (FIDL00061).
    if apply_combust && state.player.powers.combust > 0 {
        apply_end_of_turn_constricted(state)?;
        if state.player.hp <= 0 {
            return Ok(false);
        }
    }
    if apply_combust {
        apply_end_of_turn_combust(state, deferred)?;
        if state.player.hp <= 0 {
            return Ok(false);
        }
    }
    apply_end_of_turn_bomb_timers(state, deferred)
}

/// Combust `atEndOfTurn` is `addToBot` from `AbstractRoom.endTurn`, after
/// end-turn autoplay cards have already resolved.
pub(crate) fn apply_deferred_end_of_turn_combust(
    state: &mut CombatState,
    deferred: &mut Vec<crate::combat::transition::DeferredMonsterDeath>,
) -> SimResult<()> {
    let mut deferred = Some(deferred);
    apply_end_of_turn_combust(state, &mut deferred)
}

/// Whether Constricted already ran in the pre-hand Combust window this end-turn.
#[must_use]
pub(crate) fn constricted_resolved_before_hand_with_combust(state: &CombatState) -> bool {
    // After before_hand, combust stacks are unchanged; the flag is "had combust
    // when before_hand ran". Callers invoke this after before_hand with the
    // same combust > 0 check used inside before_hand.
    state.player.powers.combust > 0
}

pub(crate) fn apply_end_of_player_turn_regeneration(state: &mut CombatState) -> SimResult<()> {
    if state.player.powers.regen > 0 {
        heal_combat_player_with_relics(state, state.player.powers.regen)?;
        state.player.powers.regen -= 1;
    }
    Ok(())
}

fn apply_player_end_of_turn_powers_for_combat_state(
    state: &mut CombatState,
    apply_regeneration: bool,
) -> SimResult<()> {
    if state.player.powers.ritual > 0 {
        state.player.powers.strength = state
            .player
            .powers
            .strength
            .checked_add(state.player.powers.ritual)
            .ok_or(SimError::InvalidState(
                "combat integer addition overflows i32",
            ))?;
    }
    if state.player.powers.like_water > 0
        && state.player.powers.calm > 0
        && !state.time_warp_end_powers_applied
    {
        crate::combat::transition::apply_player_end_turn_automatic_block_gain(
            state,
            state.player.powers.like_water,
        )?
    }
    if state.player.powers.metallicize > 0 && !state.time_warp_end_powers_applied {
        crate::combat::transition::apply_player_end_turn_automatic_block_gain(
            state,
            state.player.powers.metallicize,
        )?
    }
    if state.player.powers.plated_armor > 0 && !state.time_warp_end_powers_applied {
        crate::combat::transition::apply_player_end_turn_automatic_block_gain(
            state,
            state.player.powers.plated_armor,
        )?
    }
    if apply_regeneration {
        apply_end_of_player_turn_regeneration(state)?;
    }
    if state.player.powers.entangled > 0 {
        state.player.powers.entangled = 0;
    }
    Ok(())
}

pub(crate) fn apply_end_of_turn_constricted(state: &mut CombatState) -> SimResult<()> {
    if state.player.powers.constricted <= 0 {
        return Ok(());
    }
    // Constricted queues a DamageAction with DamageType.THORNS in the target
    // runtime. Unlike HP_LOSS, that damage consumes player block first.
    let hp_loss =
        crate::combat::hp_loss::lose_player_blockable_hp(state, state.player.powers.constricted);
    crate::combat::hp_loss::apply_player_hp_loss_hooks_deferred_draw_followups_bypass_no_draw(
        state, hp_loss,
    )?;
    crate::combat::turn::revive_player_if_available(state)
}

pub fn apply_player_end_of_turn_powers(player: &mut PlayerState) {
    apply_player_end_of_turn_powers_with_relics(player, &[]);
}

pub fn apply_player_end_of_turn_powers_with_relics(player: &mut PlayerState, relics: &[Relic]) {
    if player.powers.ritual > 0 {
        player.powers.strength += player.powers.ritual;
    }
    if player.powers.like_water > 0 && player.powers.calm > 0 && player.no_block_turns == 0 {
        player.block += player.powers.like_water;
    }
    if player.powers.metallicize > 0 && player.no_block_turns == 0 {
        player.block += player.powers.metallicize;
    }
    if player.powers.plated_armor > 0 && player.no_block_turns == 0 {
        player.block += player.powers.plated_armor;
    }
    if player.powers.regen > 0 {
        let max_hp = player.max_hp;
        let regen = player.powers.regen;
        heal_player_in_combat_with_relics(&mut player.hp, max_hp, regen, relics);
        player.powers.regen -= 1;
    }
    if player.powers.frail > 0 {
        player.powers.frail -= 1;
    }
    if player.powers.entangled > 0 {
        player.powers.entangled = 0;
    }
    if player.powers.constricted > 0 {
        player.hp = (player.hp - player.powers.constricted).max(0);
    }
}

fn apply_end_of_turn_combust(
    state: &mut CombatState,
    deferred: &mut Option<&mut Vec<crate::combat::transition::DeferredMonsterDeath>>,
) -> SimResult<()> {
    // CombustPower.atEndOfTurn returns immediately when
    // MonsterGroup.areMonstersBasicallyDead(). Metallicize/Juggernaut can
    // empty the field before this power is queued (FIDL02206).
    if state
        .monsters
        .iter()
        .all(|monster| !monster.alive && !awakened_one_is_half_dead(monster))
    {
        return Ok(());
    }
    let combust_stacks = state.player.powers.combust.max(0);
    if combust_stacks > 0 {
        // Stacked Combust is one LoseHPAction whose hpLoss field is increased by
        // one per stack. Card-loss hooks such as Rupture therefore fire once,
        // not once for every point of HP lost.
        let hp_loss = lose_player_hp(state, combust_stacks * COMBUST_HP_LOSS);
        // RunicCube.wasHPLost addToTop(DrawCardAction) so the trigger card
        // arrives before DiscardAtEndOfTurnAction. Evolve / Fire Breathing
        // callbacks are addToBot behind that discard (FIDL01335 / FIDL01565).
        crate::combat::hp_loss::apply_player_card_hp_loss_hooks_deferred_draw_followups(
            state, hp_loss,
        )?;
        crate::combat::turn::revive_player_if_available(state)?;
        if state.player.hp <= 0 {
            return Ok(());
        }
    }
    deal_combust_damage_to_living_monsters(state, deferred)
}

fn lose_player_hp(state: &mut CombatState, amount: i32) -> i32 {
    crate::combat::hp_loss::lose_player_hp(state, amount)
}

fn deal_combust_damage_to_living_monsters(
    state: &mut CombatState,
    deferred: &mut Option<&mut Vec<crate::combat::transition::DeferredMonsterDeath>>,
) -> SimResult<()> {
    // Combust is end-of-turn player power damage before the enemy phase. A form-1
    // Awakened One first-kill here still receives REBIRTH on that same enemy
    // phase (permanent FIDL00368 / FIDL00395). Do not set defer_awakened_one_rebirth:
    // deferring matches open FIDL00391's half-dead player turn but breaks those
    // permanents' same-END Dark Echo. Mid-turn kills (FIDL00378) already rebirth
    // on the next END without a flag.
    deal_unmodified_damage_to_living_monsters(state, state.player.powers.combust_damage, deferred)?;
    Ok(())
}

fn apply_end_of_turn_bomb_timers(
    state: &mut CombatState,
    deferred: &mut Option<&mut Vec<crate::combat::transition::DeferredMonsterDeath>>,
) -> SimResult<bool> {
    if state.bomb_timers.is_empty() {
        return Ok(false);
    }

    let timers = std::mem::take(&mut state.bomb_timers);
    let mut bomb_caused_terminal = false;
    for mut timer in timers {
        timer.turns_remaining -= 1;
        if timer.turns_remaining <= 0 {
            let had_living_monster = state.monsters.iter().any(|monster| monster.alive);
            deal_unmodified_damage_to_living_monsters(state, timer.damage, deferred)?;
            let all_basically_dead = state
                .monsters
                .iter()
                .all(|monster| !monster.alive && !awakened_one_is_half_dead(monster));
            bomb_caused_terminal |= had_living_monster && all_basically_dead;
            if state.player.hp <= 0 || all_basically_dead {
                return Ok(bomb_caused_terminal);
            }
        } else {
            state.bomb_timers.push(timer);
        }
    }
    Ok(bomb_caused_terminal)
}

fn deal_unmodified_damage_to_living_monsters(
    state: &mut CombatState,
    amount: i32,
    deferred: &mut Option<&mut Vec<crate::combat::transition::DeferredMonsterDeath>>,
) -> SimResult<()> {
    let targets = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster.id)
        .collect::<Vec<MonsterId>>();

    let relics = state.player.authority.relics.clone();
    for target in targets {
        // Prior death hooks (e.g. Gremlin Horn / multi-enemy) may already have
        // killed a later collected target — skip rather than panic (FIDL00408).
        let Some(monster) = state
            .monsters
            .iter_mut()
            .find(|monster| monster.id == target && monster.alive)
        else {
            continue;
        };
        let (killed, broke_block) = {
            // End-of-turn damage (Combust, bombs) must not enter Guardian Mode
            // Shift immediately: defensive block is queued after monster
            // pre-turn loseBlock in the target action manager. Accumulate only
            // here; `resolve_deferred_guardian_mode_shifts` runs after clear.
            let block_before = monster.block;
            let hp_damage =
                crate::combat::damage::deal_unmodified_damage_to_monster_deferred_guardian(
                    monster, amount,
                );
            crate::content::monsters::guardian_accumulate_hp_damage(monster, hp_damage);
            wake_lagavulin_on_damage(monster, hp_damage);
            (!monster.alive, block_before > 0 && monster.block == 0)
        };
        if let Some(monster) = state
            .monsters
            .iter_mut()
            .find(|monster| monster.id == target)
        {
            apply_hand_drill_if_broke_block(monster, &relics, !killed, broke_block)?;
        }
        check_slime_boss_split(state, target);
        if killed {
            if let Some(events) = deferred.as_deref_mut() {
                crate::combat::transition::queue_end_turn_monster_death(state, target, events)?;
            } else {
                crate::combat::transition::apply_monster_death_hooks(state, target)?;
            }
        }
    }
    Ok(())
}

pub fn apply_end_of_monster_turn_powers(monster: &mut MonsterState) -> SimResult<()> {
    apply_end_of_monster_turn_powers_with_ritual(monster, true)
}

pub fn apply_end_of_monster_turn_powers_without_ritual(
    monster: &mut MonsterState,
) -> SimResult<()> {
    apply_end_of_monster_turn_powers_with_ritual(monster, false)
}

fn apply_end_of_monster_turn_powers_with_ritual(
    monster: &mut MonsterState,
    apply_ritual: bool,
) -> SimResult<()> {
    let mut strength = monster.powers.strength;
    if apply_ritual && monster.powers.ritual > 0 {
        strength = strength
            .checked_add(monster.powers.ritual)
            .ok_or(SimError::InvalidState(
                "monster end-turn arithmetic overflow",
            ))?;
    }
    // GenericStrengthUpPower (Orb Walker): gain Strength at end of turn.
    if monster.powers.strength_up > 0 {
        strength =
            strength
                .checked_add(monster.powers.strength_up)
                .ok_or(SimError::InvalidState(
                    "monster end-turn arithmetic overflow",
                ))?;
    }
    let mut block = monster.block;
    if monster.powers.metallicize > 0 {
        block = block
            .checked_add(monster.powers.metallicize)
            .ok_or(SimError::InvalidState(
                "monster end-turn arithmetic overflow",
            ))?;
    }
    if monster.powers.plated_armor > 0 {
        block = block
            .checked_add(monster.powers.plated_armor)
            .ok_or(SimError::InvalidState(
                "monster end-turn arithmetic overflow",
            ))?;
    }
    monster.powers.strength = strength;
    monster.block = block;
    Ok(())
}

pub fn monster_attack_damage(monster: &MonsterState, base: i32) -> SimResult<i32> {
    let with_strength = base
        .checked_add(monster.powers.strength)
        .ok_or(SimError::InvalidState(
            "monster attack damage arithmetic overflow",
        ))?
        .max(0);
    if monster.powers.weak > 0 {
        Ok(i32::try_from(i64::from(with_strength) * 3 / 4)
            .map_err(|_| SimError::InvalidState("monster attack damage arithmetic overflow"))?)
    } else {
        Ok(with_strength)
    }
}

/// Monster attack damage after monster Weak and player Vulnerable.
pub fn monster_damage_to_player(
    player: &PlayerState,
    monster: &MonsterState,
    base: i32,
) -> SimResult<i32> {
    monster_damage_to_player_with_relics(player, monster, base, &[])
}

/// Monster attack damage after monster Weak, player Vulnerable, and relics.
pub fn monster_damage_to_player_with_relics(
    player: &PlayerState,
    monster: &MonsterState,
    base: i32,
    relics: &[Relic],
) -> SimResult<i32> {
    let damage = base
        .checked_add(monster.powers.strength)
        .ok_or(SimError::InvalidState(
            "monster attack damage arithmetic overflow",
        ))?
        .max(0);
    let mut numerator = i128::from(damage);
    let mut denominator = 1_i128;
    if monster.powers.weak > 0 {
        numerator *= 3;
        denominator *= 4;
    }
    if player.powers.vulnerable > 0 {
        if relics.contains(&Relic::OddMushroom) {
            numerator *= 5;
            denominator *= 4;
        } else {
            numerator *= 3;
            denominator *= 2;
        }
    }
    // WrathStance.atDamageReceive doubles NORMAL incoming attack damage.
    if player.powers.wrath > 0 {
        numerator *= 2;
    }
    i32::try_from(numerator / denominator)
        .map_err(|_| SimError::InvalidState("monster attack damage arithmetic overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::CombatState;

    #[test]
    fn monster_weak_and_player_vulnerable_truncate_once() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].powers.strength = 3;
        state.monsters[0].powers.weak = 1;
        state.player.powers.vulnerable = 1;

        assert_eq!(
            monster_damage_to_player(&state.player, &state.monsters[0], 18),
            Ok(23)
        );
    }

    #[test]
    fn monster_damage_to_player_respects_odd_mushroom() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.vulnerable = 2;

        assert_eq!(
            monster_damage_to_player_with_relics(
                &state.player,
                &state.monsters[0],
                5,
                &[Relic::OddMushroom],
            ),
            Ok(6)
        );
    }

    #[test]
    fn monster_attack_damage_rejects_unrepresentable_values() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].powers.strength = i32::MAX;

        assert_eq!(
            monster_damage_to_player(&state.player, &state.monsters[0], 1),
            Err(SimError::InvalidState(
                "monster attack damage arithmetic overflow"
            ))
        );

        state.monsters[0].powers.strength = 0;
        assert_eq!(
            monster_damage_to_player(&state.player, &state.monsters[0], i32::MAX),
            Ok(i32::MAX)
        );

        state.player.powers.vulnerable = 1;
        assert_eq!(
            monster_damage_to_player(&state.player, &state.monsters[0], i32::MAX),
            Err(SimError::InvalidState(
                "monster attack damage arithmetic overflow"
            ))
        );
    }

    #[test]
    fn stacked_combust_triggers_rupture_once_for_the_combined_hp_loss() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.powers.combust = 2;
        state.player.powers.combust_damage = 10;
        state.player.powers.rupture = 2;
        let monster_hp = state.monsters[0].hp;

        apply_end_of_player_turn_powers(&mut state).expect("end-turn powers resolve");

        assert_eq!(state.player.hp, 18);
        assert_eq!(state.player.powers.strength, 2);
        assert_eq!(state.monsters[0].hp, monster_hp - 10);
    }

    #[test]
    fn constricted_damage_consumes_block_before_hp_loss_hooks() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 100;
        state.player.block = 11;
        state.player.powers.constricted = 10;
        state.player.authority.relics = vec![Relic::SelfFormingClay];

        apply_end_of_player_turn_powers(&mut state).expect("end-turn powers resolve");

        assert_eq!(state.player.hp, 100);
        assert_eq!(state.player.block, 1);
        assert_eq!(state.relic_counters.self_forming_clay_next_turn_block, 0);
    }

    #[test]
    fn end_turn_decay_is_blocked_before_constricted_and_does_not_trigger_rupture() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 100;
        state.player.block = 6;
        state.player.powers.metallicize = 6;
        state.player.powers.constricted = 10;
        state.player.powers.rupture = 1;
        state.piles.hand = vec![crate::CardInstance::new(
            crate::CardId::new(1),
            crate::content::cards::DECAY_ID,
        )];

        apply_end_of_player_turn_powers_before_hand(&mut state)
            .expect("pre-hand end-turn powers resolve");
        crate::combat::hand::resolve_end_of_turn_hand(&mut state)
            .expect("Decay resolves in hand order");
        apply_end_of_turn_constricted(&mut state).expect("Constricted resolves after hand");

        assert_eq!(state.player.hp, 100);
        assert_eq!(state.player.block, 0);
        assert_eq!(state.player.powers.strength, 0);
    }

    #[test]
    fn combust_decrements_guardian_mode_shift_once_per_hp_damage() {
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![crate::content::monsters::monster_state_for_ascension(
            &crate::content::monsters::GUARDIAN_A0,
            crate::MonsterId::new(1),
            0,
        )];
        state.player.powers.combust = 1;
        state.player.powers.combust_damage = 5;

        apply_end_of_player_turn_powers(&mut state).expect("end-turn powers resolve");

        let guardian = &state.monsters[0];
        assert_eq!(guardian.hp, 235);
        assert_eq!(guardian.mode_shift, 25);
        assert!(!guardian.in_defensive_mode);
    }

    #[test]
    fn combust_mode_shift_defers_entry_until_after_monster_block_clear() {
        // Target: Combust DamageAllEnemies queues ChangeState; GainBlock(20) is
        // itself queued from changeState and lands after MonsterStartTurn
        // loseBlock. End-of-turn powers alone must only accumulate Mode Shift.
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![crate::content::monsters::monster_state_for_ascension(
            &crate::content::monsters::GUARDIAN_A0,
            crate::MonsterId::new(1),
            0,
        )];
        state.monsters[0].mode_shift = 4;
        state.monsters[0].block = 1;
        state.player.powers.combust = 1;
        state.player.powers.combust_damage = 5;
        state.player.hp = 100;
        state.player.block = 0;

        apply_end_of_player_turn_powers(&mut state).expect("end-turn powers resolve");

        let guardian = &state.monsters[0];
        // 5 damage: 1 absorbed by block, 4 HP → Mode Shift depleted, entry deferred.
        assert_eq!(guardian.hp, 236);
        assert_eq!(guardian.mode_shift, 0);
        assert!(!guardian.in_defensive_mode);
        assert_eq!(guardian.block, 0);

        crate::content::monsters::resolve_deferred_guardian_mode_shifts(&mut state.monsters);
        let guardian = &state.monsters[0];
        assert!(guardian.in_defensive_mode);
        assert_eq!(
            guardian.block,
            crate::content::monsters::GUARDIAN_DEFENSIVE_BLOCK
        );
        assert_eq!(guardian.defensive_turns_remaining, 3);
        assert!(matches!(
            guardian.intent,
            crate::MonsterIntent::GuardianCloseUp { sharp_hide: 3 }
        ));
    }
}
