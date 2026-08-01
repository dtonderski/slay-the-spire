use crate::{
    combat::{MonsterState, PlayerState},
    content::monsters::{
        guardian_on_hp_damage, large_acid_slime_on_hp_damage, mark_awakened_one_half_dead,
        DARKLING_ID, GREMLIN_WARRIOR_ID, TRANSIENT_ID,
    },
    ids::{CardId, MonsterId},
    power::PlayerPowers,
    relic::Relic,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageInfo {
    pub source: DamageSource,
    pub target: MonsterId,
    pub amount: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageSource {
    Card(CardId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttackDamageResult {
    pub hp_damage: i32,
    pub broke_block: bool,
    pub curl_up_block: Option<i32>,
    pub malleable_block: Option<i32>,
}

pub fn deal_unmodified_damage_to_monster(monster: &mut MonsterState, amount: i32) -> i32 {
    deal_unmodified_damage_to_monster_inner(monster, amount, true)
}

/// HP_LOSS-style damage (e.g. Charon's Ashes): ignores Block, does not trigger Malleable.
pub fn deal_hp_loss_damage_to_monster(monster: &mut MonsterState, amount: i32) -> i32 {
    let amount = cap_monster_damage_with_intangible(monster, amount);
    let hp_damage = monster.hp.max(0).min(amount);
    monster.hp -= hp_damage;
    if monster.hp <= 0 {
        monster.hp = 0;
        monster.alive = false;
        monster.block = 0;
        if mark_awakened_one_half_dead(monster) {
        } else if monster.content_id == DARKLING_ID {
            monster.escaped = true;
            monster.intent = crate::MonsterIntent::Attack { damage: 0 };
            monster.powers = Default::default();
        }
    }
    guardian_on_hp_damage(monster, hp_damage);
    large_acid_slime_on_hp_damage(monster, hp_damage);
    transient_shifting_on_hp_damage(monster, hp_damage);
    hp_damage
}

/// Darkling Life Link (RegrowPower): when every Darkling is half-dead, they all
/// die for real and the encounter can end.
///
/// Source: `Darkling.damage` — after a Darkling first reaches 0 HP it sets
/// `halfDead` and then:
/// ```text
/// allDead = true
/// for each monster with id "Darkling":
///     if !halfDead: allDead = false
/// if allDead:
///     room.cannotLose = false
///     this.halfDead = false
///     for each monster: monster.die()
/// ```
/// `die()` only permanently kills while `!cannotLose`, so half-dead Darklings
/// otherwise stay in limbo for the COUNT → REINCARNATE sequence.
///
/// Sim mapping: half-dead is `!alive && escaped`. Permanent death clears
/// `escaped` so remaining Darklings do not take regrow turns.
///
/// Returns true when Life Link permanently killed the pack.
#[must_use]
pub fn resolve_darkling_life_link(monsters: &mut [MonsterState]) -> bool {
    let mut saw_darkling = false;
    let mut all_half_dead = true;
    for monster in monsters.iter() {
        if monster.content_id != DARKLING_ID {
            continue;
        }
        saw_darkling = true;
        if monster.alive || !monster.escaped {
            all_half_dead = false;
            break;
        }
    }
    if !saw_darkling || !all_half_dead {
        return false;
    }
    for monster in monsters.iter_mut() {
        if monster.content_id != DARKLING_ID {
            continue;
        }
        monster.alive = false;
        monster.escaped = false;
        monster.hp = 0;
        monster.block = 0;
    }
    true
}

/// Applies direct damage without resolving Guardian's Mode Shift immediately.
///
/// Multi-hit thorns are queued once per hit in the target game. Guardian's
/// defensive block is queued after that complete attack, so a hit that reaches
/// zero Mode Shift must not make the later thorns hits strike the new block.
pub(crate) fn deal_unmodified_damage_to_monster_deferred_guardian(
    monster: &mut MonsterState,
    amount: i32,
) -> i32 {
    deal_unmodified_damage_to_monster_inner(monster, amount, false)
}

fn deal_unmodified_damage_to_monster_inner(
    monster: &mut MonsterState,
    amount: i32,
    resolve_guardian_mode_shift: bool,
) -> i32 {
    let amount = cap_monster_damage_with_intangible(monster, amount);
    let blocked = monster.block.min(amount);
    monster.block -= blocked;
    let hp_damage = monster.hp.max(0).min(amount - blocked);
    monster.hp -= hp_damage;

    if monster.hp <= 0 {
        monster.hp = 0;
        monster.alive = false;
        monster.block = 0;
        if mark_awakened_one_half_dead(monster) {
            // The first death resolves on the Awakened One's next monster turn.
        } else if monster.content_id == DARKLING_ID {
            monster.escaped = true;
            monster.intent = crate::MonsterIntent::Attack { damage: 0 };
            monster.powers = Default::default();
        }
    }
    if resolve_guardian_mode_shift {
        guardian_on_hp_damage(monster, hp_damage);
    }
    large_acid_slime_on_hp_damage(monster, hp_damage);
    transient_shifting_on_hp_damage(monster, hp_damage);

    hp_damage
}

fn deal_attack_damage_to_monster(
    monster: &mut MonsterState,
    relics: &[Relic],
    amount: i32,
) -> AttackDamageResult {
    let amount = if monster.powers.flight > 0 || monster.powers.flight_grounding_pending {
        amount / 2
    } else {
        amount
    };
    let amount = cap_monster_damage_with_intangible(monster, amount);
    let block_before = monster.block;
    let blocked = monster.block.min(amount);
    monster.block -= blocked;
    let unblocked =
        crate::relic::apply_attack_damage_relics_to_unblocked_damage(relics, amount - blocked);
    let hp_damage = monster.hp.max(0).min(unblocked);
    monster.hp -= hp_damage;

    if monster.hp <= 0 {
        monster.hp = 0;
        monster.alive = false;
        monster.block = 0;
        if mark_awakened_one_half_dead(monster) {
            // The first death resolves on the Awakened One's next monster turn.
        } else if monster.content_id == DARKLING_ID {
            monster.escaped = true;
            monster.intent = crate::MonsterIntent::Attack { damage: 0 };
            monster.powers = Default::default();
        }
    }
    let curl_up_block = if monster.alive && hp_damage > 0 && monster.powers.curl_up > 0 {
        let amount = monster.powers.curl_up;
        monster.powers.curl_up = 0;
        Some(amount)
    } else {
        None
    };
    let malleable_block = if monster.alive && hp_damage > 0 && monster.powers.malleable > 0 {
        let amount = monster.powers.malleable;
        monster.powers.malleable += 1;
        Some(amount)
    } else {
        None
    };
    if monster.alive
        && hp_damage > 0
        && monster.content_id == GREMLIN_WARRIOR_ID
        && monster.powers.anger > 0
    {
        monster.powers.strength += monster.powers.anger;
    }
    if monster.alive && hp_damage > 0 && monster.powers.flight > 0 {
        monster.powers.flight -= 1;
        if monster.powers.flight == 0 {
            monster.powers.flight_grounding_pending = true;
        }
    }
    reduce_monster_plated_armor_after_hp_damage(monster, hp_damage);
    large_acid_slime_on_hp_damage(monster, hp_damage);
    transient_shifting_on_hp_damage(monster, hp_damage);

    AttackDamageResult {
        hp_damage,
        broke_block: block_before > 0 && blocked == block_before,
        curl_up_block,
        malleable_block,
    }
}

fn cap_monster_damage_with_intangible(monster: &MonsterState, amount: i32) -> i32 {
    if monster.powers.intangible > 0 && amount > 1 {
        1
    } else {
        amount
    }
}

fn reduce_monster_plated_armor_after_hp_damage(monster: &mut MonsterState, hp_damage: i32) {
    if !monster.alive || hp_damage <= 0 || monster.powers.plated_armor <= 0 {
        return;
    }

    monster.powers.plated_armor -= 1;
    if monster.powers.plated_armor == 0 {
        monster.intent = crate::MonsterIntent::Stun;
    }
}

fn transient_shifting_on_hp_damage(monster: &mut MonsterState, hp_damage: i32) {
    if !monster.alive || hp_damage <= 0 || monster.content_id != TRANSIENT_ID {
        return;
    }

    monster.powers.strength -= hp_damage;
    monster.temp_strength_down += hp_damage;
}

pub fn deal_damage_info_to_monster(
    monster: &mut MonsterState,
    info: DamageInfo,
    player: PlayerPowers,
    temp_strength: i32,
    relics: &[Relic],
) -> i32 {
    deal_damage_info_to_monster_with_result(monster, info, player, temp_strength, relics).hp_damage
}

pub fn deal_damage_info_to_monster_with_result(
    monster: &mut MonsterState,
    info: DamageInfo,
    player: PlayerPowers,
    temp_strength: i32,
    relics: &[Relic],
) -> AttackDamageResult {
    let amount = calculate_player_attack_damage(
        info.amount,
        player,
        temp_strength,
        monster.powers.vulnerable,
        monster.powers.slow.saturating_sub(1),
        relics,
    );
    deal_attack_damage_to_monster(monster, relics, amount)
}

fn calculate_player_attack_damage(
    base: i32,
    player: PlayerPowers,
    temp_strength: i32,
    target_vulnerable: i32,
    target_slow: i32,
    relics: &[Relic],
) -> i32 {
    // VigorPower.atDamageGive adds to NORMAL attack damage like Strength, before
    // Weak / Vulnerable / Slow multipliers. It is consumed after the Attack card.
    let mut amount = (base + player.strength + temp_strength + player.vigor).max(0) as f64;
    if player.weak > 0 {
        amount *= 0.75;
    }
    if target_vulnerable > 0 {
        amount *= if relics.contains(&Relic::PaperPhrog) {
            1.75
        } else {
            1.5
        };
    }
    if target_slow > 0 {
        amount *= 1.0 + f64::from(target_slow) * 0.1;
    }
    // Divinity stance (Blasphemy): triple attack damage.
    if player.divinity > 0 {
        amount *= 3.0;
    }
    amount.floor().max(0.0) as i32
}

/// Reflects thorns-style spikes damage to the player after an attack hits the monster.
pub fn reflect_spikes_to_player(player: &mut PlayerState, relics: &[Relic], spikes: i32) -> i32 {
    if spikes <= 0 {
        return 0;
    }

    let incoming = crate::combat::hp_loss::cap_player_damage_with_intangible(player, spikes);
    let blocked = player.block.min(incoming);
    player.block -= blocked;
    let mitigated = crate::relic::mitigate_hp_loss(relics, incoming - blocked);
    let hp_loss = crate::relic::apply_buffer_to_hp_loss(&mut player.powers, mitigated);
    player.hp -= hp_loss;
    hp_loss
}
