use crate::{
    combat::{MonsterState, PlayerState},
    content::monsters::{
        guardian_on_hp_damage, large_acid_slime_on_hp_damage, DARKLING_ID, GREMLIN_WARRIOR_ID,
        TRANSIENT_ID,
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
    pub malleable_block: Option<i32>,
}

pub fn deal_unmodified_damage_to_monster(monster: &mut MonsterState, amount: i32) -> i32 {
    let amount = cap_monster_damage_with_intangible(monster, amount);
    let blocked = monster.block.min(amount);
    monster.block -= blocked;
    let hp_damage = monster.hp.max(0).min(amount - blocked);
    monster.hp -= hp_damage;

    if monster.hp <= 0 {
        monster.hp = 0;
        monster.alive = false;
        monster.block = 0;
        if monster.content_id == DARKLING_ID {
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

fn deal_attack_damage_to_monster(
    monster: &mut MonsterState,
    relics: &[Relic],
    amount: i32,
) -> AttackDamageResult {
    let amount = if monster.powers.flight > 0 {
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
        if monster.content_id == DARKLING_ID {
            monster.escaped = true;
            monster.intent = crate::MonsterIntent::Attack { damage: 0 };
            monster.powers = Default::default();
        }
    } else if hp_damage > 0 && monster.powers.curl_up > 0 {
        monster.block += monster.powers.curl_up;
        monster.powers.curl_up = 0;
    }
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
            monster.intent = crate::MonsterIntent::Stun;
        }
    }
    reduce_monster_plated_armor_after_hp_damage(monster, hp_damage);
    large_acid_slime_on_hp_damage(monster, hp_damage);
    transient_shifting_on_hp_damage(monster, hp_damage);

    AttackDamageResult {
        hp_damage,
        broke_block: block_before > 0 && blocked == block_before,
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
        relics,
    );
    deal_attack_damage_to_monster(monster, relics, amount)
}

fn calculate_player_attack_damage(
    base: i32,
    player: PlayerPowers,
    temp_strength: i32,
    target_vulnerable: i32,
    relics: &[Relic],
) -> i32 {
    let mut amount = (base + player.strength + temp_strength).max(0) as f64;
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
