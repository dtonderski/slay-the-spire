use super::{
    add_rampage_damage_bonus, checked_add_combat_value, checked_combat_sum, find_combat_card_mut,
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
    let draws_across_revival = hp_loss > 0 && state.player.hp <= 0;
    if matches!(source, HpLossSource::Card(_)) {
        if draws_across_revival {
            crate::combat::hp_loss::apply_player_card_hp_loss_hooks_deferred_draws(state, hp_loss)?;
        } else {
            crate::combat::hp_loss::apply_player_card_hp_loss_hooks(state, hp_loss)?;
        }
    } else if draws_across_revival {
        crate::combat::hp_loss::apply_player_hp_loss_hooks_with_draw_policy(
            state,
            hp_loss,
            crate::relic::HpLossDrawPolicy::DeferDraws,
        )?;
    } else {
        crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss)?;
    }
    // LoseHPAction calls AbstractPlayer.damage: revival happens within this
    // damage frame, before HP-loss relic DrawCardActions or later card effects.
    if draws_across_revival {
        crate::combat::turn::revive_player_if_available(state)?;
        return crate::relic::settle_deferred_hp_loss_draw_relics(state);
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
    if !state.player.cannot_draw {
        state.player.no_draw_precedes_combust = state.player.powers.combust == 0;
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

pub(super) fn resolve_steam_barrier(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    let card = find_combat_card_mut(state, card_id).ok_or(crate::SimError::UnknownCard(card_id))?;
    let definition = crate::content::cards::get_card_definition(card.content_id)
        .ok_or(crate::SimError::UnknownContent(card.content_id))?;
    let printed_block = definition
        .values
        .block
        .ok_or(crate::SimError::InvalidState(
            "Steam Barrier definition is missing block",
        ))?
        + if card.upgrades > 0 { 2 } else { 0 };
    let block = printed_block
        .saturating_sub(card.steam_barrier_block_reduction)
        .max(0);
    card.steam_barrier_block_reduction = checked_combat_sum(card.steam_barrier_block_reduction, 1)?;
    Ok(vec![InternalAction::GainBlock { amount: block }])
}

pub(super) fn resolve_follow_up_energy(should_gain: bool) -> SimResult<Vec<InternalAction>> {
    Ok(should_gain
        .then_some(InternalAction::GainEnergy { amount: 1 })
        .into_iter()
        .collect())
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
    // Fasting applies EnergyDownPower (DEBUFF) after its positive stat gains.
    // Reject only this incoming loss, retaining any previous EnergyDown stack.
    if amount > 0 && state.player.powers.artifact > 0 {
        state.player.powers.artifact -= 1;
        return Ok(Vec::new());
    }
    checked_add_combat_value(&mut state.player.powers.fasting, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_like_water(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.like_water, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_nirvana(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.nirvana, amount)?;
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
    if state.player.powers.combust == 0 && state.player.cannot_draw {
        state.player.no_draw_precedes_combust = true;
    }
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stance {
    Neutral,
    Calm,
    Wrath,
    Divinity,
}

fn current_stance(state: &CombatState) -> Stance {
    if state.player.powers.divinity > 0 {
        Stance::Divinity
    } else if state.player.powers.wrath > 0 {
        Stance::Wrath
    } else if state.player.powers.calm > 0 {
        Stance::Calm
    } else {
        Stance::Neutral
    }
}

fn change_stance(state: &mut CombatState, new_stance: Stance) -> SimResult<Vec<InternalAction>> {
    // ChangeStanceAction is a no-op when the requested stance ID matches the
    // current one. Any actual change runs onExitStance, swaps the stance, then
    // iterates discard for FlurryOfBlows DiscardToHandAction.
    if current_stance(state) == new_stance {
        return Ok(Vec::new());
    }
    let leaving_calm = current_stance(state) == Stance::Calm;
    state.player.powers.calm = i32::from(new_stance == Stance::Calm);
    state.player.powers.wrath = i32::from(new_stance == Stance::Wrath);
    state.player.powers.divinity = i32::from(new_stance == Stance::Divinity);
    let mut follow_ups = Vec::new();
    if leaving_calm {
        // CalmStance.onExitStance addToBots GainEnergyAction(2).
        follow_ups.push(InternalAction::GainEnergy { amount: 2 });
    }
    if new_stance == Stance::Divinity {
        // DivinityStance.onEnterStance addToBots GainEnergyAction(3).
        follow_ups.push(InternalAction::GainEnergy { amount: 3 });
    }
    follow_ups.extend(flurry_discard_to_hand_actions(state));
    Ok(follow_ups)
}

fn flurry_discard_to_hand_actions(state: &CombatState) -> Vec<InternalAction> {
    use crate::content::cards::FLURRY_OF_BLOWS_ANY_COLOR_ID;
    state
        .piles
        .discard_pile
        .iter()
        .filter(|card| card.content_id == FLURRY_OF_BLOWS_ANY_COLOR_ID)
        .map(|card| InternalAction::DiscardToHand { card_id: card.id })
        .collect()
}

pub(super) fn enter_divinity(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    change_stance(state, Stance::Divinity)
}

pub(super) fn enter_calm(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    change_stance(state, Stance::Calm)
}

pub(super) fn enter_wrath(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    change_stance(state, Stance::Wrath)
}

pub(super) fn enter_neutral(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    change_stance(state, Stance::Neutral)
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

pub(super) fn gain_creative_ai(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.creative_ai, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_storm(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.storm, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_after_image(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.after_image, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_static_discharge(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.static_discharge, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_thorns(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.thorns, amount)?;
    Ok(Vec::new())
}

pub(super) fn recurse_rightmost_orb(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    // RedoAction no-ops when the first slot is empty.
    if state.orbs.is_empty() {
        return Ok(Vec::new());
    }
    let orb = state.orbs.remove(0);
    evoke_orb(state, orb)?;
    // ChannelAction(orb, autoEvoke=false) fills the emptied slot.
    if state.max_orbs > 0 {
        state.orbs.insert(0, orb);
    }
    Ok(Vec::new())
}

pub(super) fn increase_max_orbs(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    if amount <= 0 {
        return Err(SimError::InvalidState(
            "max orb slot increase must be positive",
        ));
    }
    // AbstractPlayer.increaseMaxOrbSlots returns only when maxOrbs == 10.
    // Otherwise it adds the complete amount, so Capacitor+ can take 9 to 12.
    if state.max_orbs == 10 {
        return Ok(Vec::new());
    }
    state.max_orbs = state
        .max_orbs
        .checked_add(amount)
        .ok_or(SimError::InvalidState(
            "max orb slot increase overflows i32",
        ))?;
    Ok(Vec::new())
}

pub(super) fn channel_lightning(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    channel_orb(state, crate::combat::CombatOrb::Lightning)
}

pub(super) fn channel_frost(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    channel_orb(state, crate::combat::CombatOrb::Frost)
}

pub(super) fn channel_dark(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    channel_orb(
        state,
        crate::combat::CombatOrb::Dark {
            evoke: DARK_BASE_EVOKE,
        },
    )
}

pub(super) fn dark_impulse(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    // DarkImpulseAction calls Dark.onEndOfTurn on each Dark orb. Dark overrides
    // applyFocus for its passive only: the stored evoke starts at 6, then each
    // pulse grows by max(0, 6 + Focus).
    let growth = focused_orb_amount(state, DARK_BASE_EVOKE);
    for orb in &mut state.orbs {
        if let crate::combat::CombatOrb::Dark { evoke } = orb {
            *evoke = evoke.checked_add(growth).ok_or(SimError::InvalidState(
                "Dark orb evoke amount overflows i32",
            ))?;
        }
    }
    Ok(Vec::new())
}

fn channel_orb(
    state: &mut CombatState,
    orb: crate::combat::CombatOrb,
) -> SimResult<Vec<InternalAction>> {
    // AbstractPlayer.channelOrb no-ops when maxOrbs <= 0.
    if state.max_orbs <= 0 {
        return Ok(Vec::new());
    }
    if state.orbs.len() >= state.max_orbs as usize {
        // A filled slot evokes the oldest orb before the new one lands.
        let evoked = state.orbs.remove(0);
        evoke_orb(state, evoked)?;
    }
    state.orbs.push(orb);
    Ok(Vec::new())
}

fn evoke_orb(state: &mut CombatState, orb: crate::combat::CombatOrb) -> SimResult<()> {
    match orb {
        crate::combat::CombatOrb::Lightning => super::apply_juggernaut_random_damage(
            state,
            focused_orb_amount(state, LIGHTNING_EVOKE_DAMAGE),
        ),
        crate::combat::CombatOrb::Frost => super::apply_player_end_turn_automatic_block_gain(
            state,
            focused_orb_amount(state, FROST_EVOKE_BLOCK),
        ),
        crate::combat::CombatOrb::Dark { evoke } => evoke_dark(state, evoke),
    }
}

const LIGHTNING_PASSIVE_DAMAGE: i32 = 3;
const LIGHTNING_EVOKE_DAMAGE: i32 = 8;
const FROST_PASSIVE_BLOCK: i32 = 2;
const FROST_EVOKE_BLOCK: i32 = 5;
const DARK_BASE_EVOKE: i32 = 6;

fn focused_orb_amount(state: &CombatState, base: i32) -> i32 {
    // AbstractOrb.applyFocus: amount + Focus, floored at 0.
    base.saturating_add(state.player.powers.focus).max(0)
}

pub(super) fn lightning_orb_passive(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    super::apply_juggernaut_random_damage(
        state,
        focused_orb_amount(state, LIGHTNING_PASSIVE_DAMAGE),
    )?;
    Ok(Vec::new())
}

fn evoke_dark(state: &mut CombatState, amount: i32) -> SimResult<()> {
    // DarkOrbEvokeAction scans the monster group in order and replaces its
    // target only for a strictly lower current HP. Ties therefore keep the
    // first living monster and this path consumes no target RNG.
    let Some(target) = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .min_by_key(|monster| monster.hp)
        .map(|monster| monster.id)
    else {
        return Ok(());
    };
    // AbstractOrb.applyLockOn runs before constructing Dark's THORNS
    // DamageInfo. THORNS then bypasses ordinary attack modifiers.
    let amount = if state
        .monsters
        .iter()
        .find(|monster| monster.id == target)
        .is_some_and(|monster| monster.powers.lock_on > 0)
    {
        amount
            .checked_mul(3)
            .ok_or(SimError::InvalidState("Lock-On orb damage overflows i32"))?
            / 2
    } else {
        amount
    };
    super::deal_unmodified_damage_to_living_monster(state, target, amount)
}

pub(crate) fn apply_orb_end_of_turn_passives(state: &mut CombatState) -> SimResult<()> {
    let orbs = state.orbs.clone();
    let dark_growth = focused_orb_amount(state, DARK_BASE_EVOKE);
    for (index, orb) in orbs.into_iter().enumerate() {
        match orb {
            crate::combat::CombatOrb::Lightning => {
                super::apply_juggernaut_random_damage(
                    state,
                    focused_orb_amount(state, LIGHTNING_PASSIVE_DAMAGE),
                )?;
            }
            crate::combat::CombatOrb::Frost => {
                super::apply_player_end_turn_automatic_block_gain(
                    state,
                    focused_orb_amount(state, FROST_PASSIVE_BLOCK),
                )?;
            }
            crate::combat::CombatOrb::Dark { .. } => {
                if let Some(crate::combat::CombatOrb::Dark { evoke }) = state.orbs.get_mut(index) {
                    *evoke = evoke
                        .checked_add(dark_growth)
                        .ok_or(SimError::InvalidState(
                            "Dark orb evoke amount overflows i32",
                        ))?;
                }
            }
        }
    }
    Ok(())
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
    gain_strength_power(state, amount).map(|()| Vec::new())
}

pub(crate) fn gain_strength_power(state: &mut CombatState, amount: i32) -> SimResult<()> {
    // StrengthPower.stackPower clamps the visible power to [-999, 999].
    // This representation separates its temporary component, so apply the
    // bound to their sum while retaining the later temporary-loss amount.
    let current = checked_combat_sum(state.player.powers.strength, state.player.temp_strength)?;
    let bounded = checked_combat_sum(current, amount)?.clamp(-999, 999);
    state.player.powers.strength =
        bounded
            .checked_sub(state.player.temp_strength)
            .ok_or(SimError::InvalidState(
                "strength component subtraction overflows i32",
            ))?;
    Ok(())
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

pub(super) fn gain_dexterity_from_speed_potion(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    state.player.powers.dexterity = state
        .player
        .powers
        .dexterity
        .checked_add(amount)
        .ok_or(SimError::InvalidState(
            "combat potion stat gain overflows i32",
        ))?
        .clamp(-999, 999);
    Ok(Vec::new())
}

pub(super) fn apply_dex_loss_from_speed_potion(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    // Only this incoming DEBUFF is rejected; old nominal debt stays intact.
    // Rejection precedes any arithmetic on the rejected new debt.
    if state.player.powers.artifact > 0 {
        state.player.powers.artifact -= 1;
    } else {
        state.player.temp_dexterity =
            state
                .player
                .temp_dexterity
                .checked_add(amount)
                .ok_or(SimError::InvalidState(
                    "combat potion stat gain overflows i32",
                ))?;
    }
    Ok(Vec::new())
}

pub(super) fn gain_dexterity(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    // Actual Dexterity already includes temporary gains; temp_dexterity is
    // separate nominal loss debt, not another component of this amount.
    state.player.powers.dexterity =
        checked_combat_sum(state.player.powers.dexterity, amount)?.clamp(-999, 999);
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

pub(super) fn gain_strength_from_potion(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    let error = || SimError::InvalidState("combat potion stat gain overflows i32");
    let current = state
        .player
        .powers
        .strength
        .checked_add(state.player.temp_strength)
        .ok_or_else(error)?;
    let bounded = current
        .checked_add(amount)
        .ok_or_else(error)?
        .clamp(-999, 999);
    state.player.powers.strength = bounded
        .checked_sub(state.player.temp_strength)
        .ok_or_else(error)?;
    Ok(Vec::new())
}

pub(super) fn apply_strength_loss_from_flex_potion(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    // LoseStrengthPower application does not change actual Strength. Artifact
    // rejects only this new debt before debt/component arithmetic.
    if state.player.powers.artifact > 0 {
        state.player.powers.artifact -= 1;
        return Ok(Vec::new());
    }
    let error = || SimError::InvalidState("combat potion stat gain overflows i32");
    let debt = state
        .player
        .temp_strength
        .checked_add(amount)
        .ok_or_else(error)?;
    let permanent = state
        .player
        .powers
        .strength
        .checked_sub(amount)
        .ok_or_else(error)?;
    // These are the representation of a newly accepted nominal loss power;
    // the preceding positive application already determined actual Strength.
    state.player.temp_strength = debt;
    state.player.powers.strength = permanent;
    if state.resume_end_turn_after_nilrys_codex && !state.nilrys_end_powers_pending {
        // This loss was created after the old END power window. It expires
        // at the next END, not at the intervening player-start cleanup.
        state.preserve_temp_strength_on_next_start = true;
    }
    Ok(Vec::new())
}

pub(super) fn gain_plated_armor_from_potion(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    if amount == 0 {
        return Ok(Vec::new());
    }
    let current = state.player.powers.plated_armor;
    let next = current.checked_add(amount).ok_or(SimError::InvalidState(
        "combat potion stat gain overflows i32",
    ))?;
    // PlatedArmorPower's constructor keeps raw input; an existing power's
    // stackPower caps only the upper bound. Zero represents absence here.
    state.player.powers.plated_armor = if current == 0 { next } else { next.min(999) };
    Ok(Vec::new())
}

pub(super) fn gain_artifact_from_potion(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    state.player.powers.artifact =
        state
            .player
            .powers
            .artifact
            .checked_add(amount)
            .ok_or(SimError::InvalidState(
                "combat potion stat gain overflows i32",
            ))?;
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

#[cfg(test)]
mod plated_armor_boundary_tests {
    use super::{gain_plated_armor_from_potion, CombatState};
    #[test]
    fn raw_constructor_then_existing_upper_cap() {
        let mut s = CombatState::initial_fixture();
        gain_plated_armor_from_potion(&mut s, 2000).unwrap();
        assert_eq!(s.player.powers.plated_armor, 2000);
        gain_plated_armor_from_potion(&mut s, 1).unwrap();
        assert_eq!(s.player.powers.plated_armor, 999);
    }
    #[test]
    fn stacking_has_no_lower_floor_or_input_preclip() {
        let mut s = CombatState::initial_fixture();
        s.player.powers.plated_armor = 10;
        gain_plated_armor_from_potion(&mut s, -1010).unwrap();
        assert_eq!(s.player.powers.plated_armor, -1000);
    }
    #[test]
    fn no_op_does_not_repair_malformed_old_amount() {
        let mut s = CombatState::initial_fixture();
        s.player.powers.plated_armor = 2000;
        gain_plated_armor_from_potion(&mut s, 0).unwrap();
        assert_eq!(s.player.powers.plated_armor, 2000);
    }
    #[test]
    fn checked_overflow_preserves_state() {
        let mut s = CombatState::initial_fixture();
        s.player.powers.plated_armor = i32::MAX;
        let before = serde_json::to_value(&s).unwrap();
        assert!(matches!(
            gain_plated_armor_from_potion(&mut s, 4),
            Err(crate::SimError::InvalidState(
                "combat potion stat gain overflows i32"
            ))
        ));
        assert_eq!(serde_json::to_value(&s).unwrap(), before);
    }
}
