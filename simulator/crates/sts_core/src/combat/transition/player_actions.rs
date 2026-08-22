use super::{
    add_rampage_damage_bonus, checked_add_combat_value, checked_combat_sum,
    set_random_hand_card_cost_for_combat, upgrade_combat_cards, upgrade_hand_card,
    upgrade_hand_cards_except,
};
use crate::{
    action::{HpLossSource, InternalAction},
    combat::{state::BombTimer, CombatState},
    ids::CardId,
    power::DrawTriggerPower,
    SimError, SimResult,
};

pub(super) fn gain_energy(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.energy, amount)?;
    Ok(Vec::new())
}

/// EnergyPanel.useEnergy floors at zero after subtracting `amount`.
pub(super) fn lose_energy(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    state.player.energy = (state.player.energy - amount).max(0);
    Ok(Vec::new())
}

pub(super) fn lose_hp(
    state: &mut CombatState,
    amount: i32,
    source: HpLossSource,
) -> SimResult<Vec<InternalAction>> {
    let hp_loss = crate::combat::hp_loss::lose_player_hp(state, amount);
    if matches!(source, HpLossSource::Card(_)) {
        crate::combat::hp_loss::apply_player_card_hp_loss_hooks(state, hp_loss)?;
    } else {
        crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss)?;
    }
    Ok(Vec::new())
}

pub(super) fn set_cannot_draw(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    // NoDrawPower is a DEBUFF. ApplyPowerAction consumes Artifact instead of
    // applying it (FIDL01594: Panacea Artifact blocks Battle Trance No Draw,
    // so later Flex stays temporary).
    if state.player.powers.artifact > 0 {
        state.player.powers.artifact -= 1;
        return Ok(Vec::new());
    }
    state.player.cannot_draw = true;
    Ok(Vec::new())
}

pub(super) fn gain_rage(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.temp_rage_block, amount)?;
    Ok(Vec::new())
}

pub(super) fn set_random_hand_card_cost(
    state: &mut CombatState,
    amount: u8,
    excluded_card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    set_random_hand_card_cost_for_combat(state, amount, excluded_card_id)?;
    Ok(Vec::new())
}

pub(super) fn upgrade_hand_cards_other_than(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    upgrade_hand_cards_except(state, card_id)?;
    Ok(Vec::new())
}

pub(super) fn upgrade_one_hand_card(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    upgrade_hand_card(state, card_id)?;
    Ok(Vec::new())
}

pub(super) fn increase_rampage_damage(
    state: &mut CombatState,
    card_id: CardId,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    add_rampage_damage_bonus(state, card_id, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_feel_no_pain(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.feel_no_pain, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_dark_embrace(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.dark_embrace, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_barricade(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    state.player.powers.barricade = state.player.powers.barricade.max(amount);
    Ok(Vec::new())
}

pub(super) fn gain_evolve(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    let was_active = state.player.powers.evolve > 0;
    checked_add_combat_value(&mut state.player.powers.evolve, amount)?;
    state.update_draw_trigger_power_order(
        DrawTriggerPower::Evolve,
        was_active,
        state.player.powers.evolve > 0,
    );
    Ok(Vec::new())
}

pub(super) fn gain_berserk(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.berserk, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_fasting(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.fasting, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_rupture(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.rupture, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_juggernaut(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.juggernaut, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_brutality(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.brutality, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_mayhem(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.mayhem, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_panache(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.panache, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_combust(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    let combust = checked_combat_sum(state.player.powers.combust, 1)?;
    let combust_damage = checked_combat_sum(state.player.powers.combust_damage, amount)?;
    state.player.powers.combust = combust;
    state.player.powers.combust_damage = combust_damage;
    Ok(Vec::new())
}

pub(super) fn gain_double_tap(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.double_tap_pending, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_fire_breathing(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    let was_active = state.player.powers.fire_breathing > 0;
    checked_add_combat_value(&mut state.player.powers.fire_breathing, amount)?;
    state.update_draw_trigger_power_order(
        DrawTriggerPower::FireBreathing,
        was_active,
        state.player.powers.fire_breathing > 0,
    );
    Ok(Vec::new())
}

pub(super) fn gain_corruption(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    state.player.powers.corruption = state.player.powers.corruption.max(amount);
    Ok(Vec::new())
}

pub(super) fn enter_divinity(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    state.player.powers.divinity = 1;
    Ok(Vec::new())
}

pub(super) fn apply_end_turn_death(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    state.player.powers.end_turn_death = 1;
    Ok(Vec::new())
}

pub(super) fn gain_sadistic_nature(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.sadistic_nature, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_magnetism(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.magnetism, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_storm(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.storm, amount)?;
    Ok(Vec::new())
}

pub(super) fn channel_lightning(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    // AbstractPlayer.channelOrb no-ops when maxOrbs <= 0.
    if state.max_orbs <= 0 {
        return Ok(Vec::new());
    }
    if state.orbs.len() >= state.max_orbs as usize {
        // A filled slot evokes the oldest orb before the new Lightning lands.
        let _evoked = state.orbs.remove(0);
        super::apply_juggernaut_random_damage(state, LIGHTNING_EVOKE_DAMAGE)?;
    }
    state.orbs.push(crate::combat::CombatOrb::Lightning);
    Ok(Vec::new())
}

const LIGHTNING_PASSIVE_DAMAGE: i32 = 3;
const LIGHTNING_EVOKE_DAMAGE: i32 = 8;

pub(super) fn lightning_orb_passive(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    super::apply_juggernaut_random_damage(state, LIGHTNING_PASSIVE_DAMAGE)?;
    Ok(Vec::new())
}

pub(super) fn arm_the_bomb(
    state: &mut CombatState,
    turns: i32,
    damage: i32,
) -> SimResult<Vec<InternalAction>> {
    state.bomb_timers.push(BombTimer {
        turns_remaining: turns,
        damage,
    });
    Ok(Vec::new())
}

pub(super) fn gain_metallicize(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.metallicize, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_strength(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.strength, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_mantra(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.mantra, amount)?;
    while state.player.powers.mantra >= 10 {
        state.player.powers.mantra -= 10;
        state.player.energy = state
            .player
            .energy
            .checked_add(3)
            .ok_or(SimError::InvalidState("mantra energy gain overflows i32"))?;
        state.player.powers.divinity = 1;
    }
    Ok(Vec::new())
}

pub(super) fn gain_dexterity(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    if amount > 0 && state.player.powers.fasting > 0 {
        return Ok(Vec::new());
    }
    checked_add_combat_value(&mut state.player.powers.dexterity, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_temp_strength(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    // Flex applies Strength and a debuff that removes it at end of turn.
    // Artifact blocks that debuff when it is created, consuming one Artifact
    // and leaving the gained Strength permanent.
    if state.player.powers.artifact > 0 {
        let strength = checked_combat_sum(state.player.powers.strength, amount)?;
        state.player.powers.artifact -= 1;
        state.player.powers.strength = strength;
    } else {
        checked_add_combat_value(&mut state.player.temp_strength, amount)?;
    }
    Ok(Vec::new())
}

pub(super) fn gain_intangible(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.intangible, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_ritual(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.demon_form, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_artifact(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.artifact, amount)?;
    Ok(Vec::new())
}

pub(super) fn upgrade_all_combat_cards(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    upgrade_combat_cards(state)?;
    Ok(Vec::new())
}
