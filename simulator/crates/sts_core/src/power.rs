use crate::{SimError, SimResult};
use serde::{Deserialize, Serialize};

/// Player powers whose `onCardDraw` callbacks enqueue follow-up actions.
///
/// CommunicationMod/source action order is the runtime power-list order, not
/// the lexical order of the scalar fields in [`PlayerPowers`].  Combat keeps
/// this small authoritative order separately so queued draw callbacks can
/// preserve source FIFO semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawTriggerPower {
    Evolve,
    FireBreathing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PlayerPowers {
    pub strength: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub mantra: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub calm: i32,
    /// Watcher Wrath stance: double attack damage.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub wrath: i32,
    pub weak: i32,
    pub dexterity: i32,
    pub frail: i32,
    pub vulnerable: i32,
    pub ritual: i32,
    /// DemonFormPower: +amount Strength at start of turn after draw.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub demon_form: i32,
    pub metallicize: i32,
    pub regen: i32,
    pub thorns: i32,
    pub plated_armor: i32,
    pub artifact: i32,
    pub feel_no_pain: i32,
    pub dark_embrace: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub barricade: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub evolve: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub berserk: i32,
    /// Fasting (`EnergyDownPower`): lose this much Energy at turn start and
    /// block later Dexterity gains from cards.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub fasting: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub rupture: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub juggernaut: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub brutality: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub mayhem: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub combust: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub combust_damage: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub fire_breathing: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub corruption: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub magnetism: i32,
    /// StormPower: Channel this many Lightning whenever a Power is played.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub storm: i32,
    /// AfterImagePower: Gain this much block whenever a card is played.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub after_image: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub panache: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub panache_cards_played: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub buffer: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub intangible: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub sadistic_nature: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub hex: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub confusion: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub entangled: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub constricted: i32,
    /// Temporary attack bonus consumed by the next Attack card (VigorPower).
    /// Added to each hit of that card, then removed when the card is played.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub vigor: i32,
    /// Watcher Divinity stance: attack damage is tripled while > 0.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub divinity: i32,
    /// EndTurnDeathPower from Blasphemy: kill the player at end of turn while > 0.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub end_turn_death: i32,
    /// Time Eater's one-stack DrawReductionPower remains visible after its
    /// first reduced opening draw and expires after the following one.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub draw_reduction: i32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub draw_reduction_first_draw_seen: bool,
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MonsterPowers {
    pub vulnerable: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub mark: i32,
    pub weak: i32,
    pub strength: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub artifact: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub flight: i32,
    /// Byrd reached zero Flight during the current card's action queue. The
    /// target keeps Flight's damage reduction until that queue settles.
    #[serde(default, skip_serializing_if = "is_false")]
    pub flight_grounding_pending: bool,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub intangible: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub plated_armor: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub painful_stabs: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub book_stab_count: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub explosive: i32,
    pub ritual: i32,
    pub spikes: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub spiker_thorns_buffs: i32,
    pub curl_up: i32,
    /// Gremlin Nob enrage stacks (Anger); each stack adds +1 attack damage.
    pub anger: i32,
    /// Lagavulin sleep stance and burning-elite block gain per turn.
    pub metallicize: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub regeneration: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub malleable: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub malleable_base: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub spore_cloud: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub minion: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub strength_up: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub slow: i32,
    /// Cards played toward Time Eater's twelve-card Time Warp threshold.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub time_warp: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub beat_of_death: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub invincible: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub invincible_max: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub heart_buff_count: i32,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Slay the Spire-style vulnerable bonus: attack damage is increased by 50%, floored.
#[must_use]
pub fn attack_damage_with_vulnerable(base: i32, vulnerable: i32) -> i32 {
    attack_damage_with_vulnerable_bonus(base, vulnerable, 1, 2)
}

#[must_use]
pub fn attack_damage_with_vulnerable_bonus(
    base: i32,
    vulnerable: i32,
    bonus_numerator: i32,
    bonus_denominator: i32,
) -> i32 {
    if vulnerable > 0 {
        base + base * bonus_numerator / bonus_denominator
    } else {
        base
    }
}

/// Player attack modifiers applied before target vulnerable:
/// 1. add strength, temp strength, and vigor
/// 2. if weak, multiply by 0.75 and floor via integer `base * 3 / 4`
/// 3. apply target vulnerable
#[must_use]
pub fn calculate_attack_damage(
    base: i32,
    player: PlayerPowers,
    temp_strength: i32,
    target_vulnerable: i32,
) -> i32 {
    let with_strength = (base + player.strength + temp_strength + player.vigor).max(0);
    let with_weak = if player.weak > 0 {
        with_strength * 3 / 4
    } else {
        with_strength
    };

    attack_damage_with_vulnerable(with_weak, target_vulnerable)
}

/// Block from cards: add dexterity, then apply frail reduction (25%, floored).
#[must_use]
pub fn calculate_block(base: i32, player: PlayerPowers) -> i32 {
    let with_dexterity = (base + player.dexterity).max(0);
    if player.frail > 0 {
        with_dexterity * 3 / 4
    } else {
        with_dexterity
    }
}

pub fn apply_player_weak(powers: &mut PlayerPowers, amount: i32) -> SimResult<bool> {
    apply_player_debuff(powers, amount, |powers, amount| {
        powers.weak = powers
            .weak
            .checked_add(amount)
            .ok_or(SimError::InvalidState(
                "player Weak application overflows i32",
            ))?;
        Ok(())
    })
}

pub fn apply_player_vulnerable(powers: &mut PlayerPowers, amount: i32) -> SimResult<bool> {
    apply_player_debuff(powers, amount, |powers, amount| {
        powers.vulnerable = powers
            .vulnerable
            .checked_add(amount)
            .ok_or(SimError::InvalidState(
                "player Vulnerable application overflows i32",
            ))?;
        Ok(())
    })
}

pub fn apply_player_frail(powers: &mut PlayerPowers, amount: i32) -> SimResult<bool> {
    apply_player_debuff(powers, amount, |powers, amount| {
        powers.frail = powers
            .frail
            .checked_add(amount)
            .ok_or(SimError::InvalidState(
                "player Frail application overflows i32",
            ))?;
        Ok(())
    })
}

pub fn apply_player_hex(powers: &mut PlayerPowers, amount: i32) -> SimResult<bool> {
    apply_player_debuff(powers, amount, |powers, amount| {
        powers.hex = powers
            .hex
            .checked_add(amount)
            .ok_or(SimError::InvalidState(
                "player Hex application overflows i32",
            ))?;
        Ok(())
    })
}

pub fn apply_player_confusion(powers: &mut PlayerPowers) -> SimResult<bool> {
    apply_player_debuff(powers, 1, |powers, _| {
        powers.confusion = powers.confusion.max(1);
        Ok(())
    })
}

pub fn apply_player_entangled(powers: &mut PlayerPowers, amount: i32) -> SimResult<bool> {
    apply_player_debuff(powers, amount, |powers, amount| {
        powers.entangled = powers
            .entangled
            .checked_add(amount)
            .ok_or(SimError::InvalidState(
                "player Entangled application overflows i32",
            ))?;
        Ok(())
    })
}

pub fn apply_player_constricted(powers: &mut PlayerPowers, amount: i32) -> SimResult<bool> {
    apply_player_debuff(powers, amount, |powers, amount| {
        powers.constricted = powers.constricted.max(amount);
        Ok(())
    })
}

/// Time Eater Head Slam's `DrawReductionPower` is a debuff (`ApplyPowerAction`).
/// Artifact consumes the application instead of shrinking `gameHandSize`.
pub fn apply_player_draw_reduction(powers: &mut PlayerPowers, amount: i32) -> SimResult<bool> {
    apply_player_debuff(powers, amount, |powers, amount| {
        powers.draw_reduction =
            powers
                .draw_reduction
                .checked_add(amount)
                .ok_or(SimError::InvalidState(
                    "player Draw Reduction application overflows i32",
                ))?;
        powers.draw_reduction_first_draw_seen = false;
        Ok(())
    })
}

pub fn reduce_player_strength(powers: &mut PlayerPowers, amount: i32) -> SimResult<bool> {
    apply_player_debuff(powers, amount, |powers, amount| {
        powers.strength = powers
            .strength
            .checked_sub(amount)
            .ok_or(SimError::InvalidState(
                "player Strength reduction underflows i32",
            ))?;
        Ok(())
    })
}

pub fn reduce_player_dexterity(powers: &mut PlayerPowers, amount: i32) -> SimResult<bool> {
    apply_player_debuff(powers, amount, |powers, amount| {
        powers.dexterity = powers
            .dexterity
            .checked_sub(amount)
            .ok_or(SimError::InvalidState(
                "player Dexterity reduction underflows i32",
            ))?;
        Ok(())
    })
}

pub fn apply_monster_weak(powers: &mut MonsterPowers, amount: i32) -> SimResult<bool> {
    apply_monster_debuff(powers, amount, |powers, amount| {
        powers.weak = powers
            .weak
            .checked_add(amount)
            .ok_or(SimError::InvalidState(
                "monster Weak application overflows i32",
            ))?;
        Ok(())
    })
}

pub fn apply_monster_vulnerable(powers: &mut MonsterPowers, amount: i32) -> SimResult<bool> {
    apply_monster_debuff(powers, amount, |powers, amount| {
        powers.vulnerable = powers
            .vulnerable
            .checked_add(amount)
            .ok_or(SimError::InvalidState(
                "monster Vulnerable application overflows i32",
            ))?;
        Ok(())
    })
}

pub fn reduce_monster_strength(powers: &mut MonsterPowers, amount: i32) -> SimResult<bool> {
    apply_monster_debuff(powers, amount, |powers, amount| {
        powers.strength = powers
            .strength
            .checked_sub(amount)
            .ok_or(SimError::InvalidState(
                "monster Strength reduction underflows i32",
            ))?;
        Ok(())
    })
}

pub fn clear_player_debuffs(powers: &mut PlayerPowers) {
    if powers.strength < 0 {
        powers.strength = 0;
    }
    if powers.dexterity < 0 {
        powers.dexterity = 0;
    }
    powers.weak = 0;
    powers.frail = 0;
    powers.vulnerable = 0;
    powers.hex = 0;
    powers.confusion = 0;
    powers.entangled = 0;
    powers.constricted = 0;
}

fn apply_player_debuff(
    powers: &mut PlayerPowers,
    amount: i32,
    apply: impl FnOnce(&mut PlayerPowers, i32) -> SimResult<()>,
) -> SimResult<bool> {
    if amount <= 0 {
        return Ok(false);
    }

    let mut next = *powers;
    if next.artifact > 0 {
        next.artifact -= 1;
        *powers = next;
        Ok(false)
    } else {
        apply(&mut next, amount)?;
        *powers = next;
        Ok(true)
    }
}

fn apply_monster_debuff(
    powers: &mut MonsterPowers,
    amount: i32,
    apply: impl FnOnce(&mut MonsterPowers, i32) -> SimResult<()>,
) -> SimResult<bool> {
    if amount <= 0 {
        return Ok(false);
    }

    if powers.artifact > 0 {
        powers.artifact -= 1;
        Ok(false)
    } else {
        apply(powers, amount)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_debuff_arithmetic_fails_closed() {
        let cases = [
            (
                PlayerPowers {
                    weak: i32::MAX,
                    ..PlayerPowers::default()
                },
                apply_player_weak as fn(&mut PlayerPowers, i32) -> SimResult<bool>,
                SimError::InvalidState("player Weak application overflows i32"),
            ),
            (
                PlayerPowers {
                    vulnerable: i32::MAX,
                    ..PlayerPowers::default()
                },
                apply_player_vulnerable,
                SimError::InvalidState("player Vulnerable application overflows i32"),
            ),
            (
                PlayerPowers {
                    frail: i32::MAX,
                    ..PlayerPowers::default()
                },
                apply_player_frail,
                SimError::InvalidState("player Frail application overflows i32"),
            ),
            (
                PlayerPowers {
                    hex: i32::MAX,
                    ..PlayerPowers::default()
                },
                apply_player_hex,
                SimError::InvalidState("player Hex application overflows i32"),
            ),
            (
                PlayerPowers {
                    entangled: i32::MAX,
                    ..PlayerPowers::default()
                },
                apply_player_entangled,
                SimError::InvalidState("player Entangled application overflows i32"),
            ),
            (
                PlayerPowers {
                    strength: i32::MIN,
                    ..PlayerPowers::default()
                },
                reduce_player_strength,
                SimError::InvalidState("player Strength reduction underflows i32"),
            ),
            (
                PlayerPowers {
                    dexterity: i32::MIN,
                    ..PlayerPowers::default()
                },
                reduce_player_dexterity,
                SimError::InvalidState("player Dexterity reduction underflows i32"),
            ),
        ];

        for (mut powers, apply, expected_error) in cases {
            let before = powers;
            assert_eq!(apply(&mut powers, 1), Err(expected_error));
            assert_eq!(powers, before);
        }
    }

    #[test]
    fn player_debuff_artifact_and_non_positive_amounts_preserve_semantics() {
        let mut artifact = PlayerPowers {
            weak: i32::MAX,
            artifact: 1,
            ..PlayerPowers::default()
        };
        assert_eq!(apply_player_weak(&mut artifact, 1), Ok(false));
        assert_eq!(artifact.weak, i32::MAX);
        assert_eq!(artifact.artifact, 0);

        let mut non_positive = PlayerPowers {
            artifact: 1,
            ..PlayerPowers::default()
        };
        assert_eq!(apply_player_vulnerable(&mut non_positive, 0), Ok(false));
        assert_eq!(reduce_player_strength(&mut non_positive, -1), Ok(false));
        assert_eq!(non_positive.artifact, 1);
    }

    #[test]
    fn valid_player_debuffs_report_that_they_landed() {
        let mut powers = PlayerPowers {
            strength: 3,
            dexterity: 2,
            ..PlayerPowers::default()
        };

        assert_eq!(apply_player_weak(&mut powers, 2), Ok(true));
        assert_eq!(apply_player_vulnerable(&mut powers, 3), Ok(true));
        assert_eq!(apply_player_frail(&mut powers, 4), Ok(true));
        assert_eq!(apply_player_hex(&mut powers, 1), Ok(true));
        assert_eq!(apply_player_entangled(&mut powers, 1), Ok(true));
        assert_eq!(apply_player_constricted(&mut powers, 5), Ok(true));
        assert_eq!(reduce_player_strength(&mut powers, 4), Ok(true));
        assert_eq!(reduce_player_dexterity(&mut powers, 3), Ok(true));
        assert_eq!(powers.weak, 2);
        assert_eq!(powers.vulnerable, 3);
        assert_eq!(powers.frail, 4);
        assert_eq!(powers.hex, 1);
        assert_eq!(powers.entangled, 1);
        assert_eq!(powers.constricted, 5);
        assert_eq!(powers.strength, -1);
        assert_eq!(powers.dexterity, -1);
    }

    #[test]
    fn draw_reduction_is_blocked_by_artifact() {
        let mut powers = PlayerPowers {
            artifact: 1,
            ..PlayerPowers::default()
        };
        assert_eq!(apply_player_draw_reduction(&mut powers, 1), Ok(false));
        assert_eq!(powers.draw_reduction, 0);
        assert_eq!(powers.artifact, 0);
        assert_eq!(apply_player_draw_reduction(&mut powers, 1), Ok(true));
        assert_eq!(powers.draw_reduction, 1);
        assert!(!powers.draw_reduction_first_draw_seen);
    }

    #[test]
    fn monster_debuff_arithmetic_fails_closed() {
        let cases = [
            (
                MonsterPowers {
                    weak: i32::MAX,
                    ..MonsterPowers::default()
                },
                apply_monster_weak as fn(&mut MonsterPowers, i32) -> SimResult<bool>,
                SimError::InvalidState("monster Weak application overflows i32"),
            ),
            (
                MonsterPowers {
                    vulnerable: i32::MAX,
                    ..MonsterPowers::default()
                },
                apply_monster_vulnerable,
                SimError::InvalidState("monster Vulnerable application overflows i32"),
            ),
            (
                MonsterPowers {
                    strength: i32::MIN,
                    ..MonsterPowers::default()
                },
                reduce_monster_strength,
                SimError::InvalidState("monster Strength reduction underflows i32"),
            ),
        ];

        for (mut powers, apply, expected_error) in cases {
            let before = powers;
            assert_eq!(apply(&mut powers, 1), Err(expected_error));
            assert_eq!(powers, before);
        }
    }

    #[test]
    fn monster_debuff_artifact_and_non_positive_amounts_preserve_semantics() {
        let mut artifact = MonsterPowers {
            weak: i32::MAX,
            artifact: 1,
            ..MonsterPowers::default()
        };
        assert_eq!(apply_monster_weak(&mut artifact, 1), Ok(false));
        assert_eq!(artifact.weak, i32::MAX);
        assert_eq!(artifact.artifact, 0);

        let mut non_positive = MonsterPowers {
            artifact: 1,
            ..MonsterPowers::default()
        };
        assert_eq!(apply_monster_vulnerable(&mut non_positive, 0), Ok(false));
        assert_eq!(reduce_monster_strength(&mut non_positive, -1), Ok(false));
        assert_eq!(non_positive.artifact, 1);
    }

    #[test]
    fn valid_monster_debuffs_report_that_they_landed() {
        let mut powers = MonsterPowers {
            strength: 3,
            ..MonsterPowers::default()
        };

        assert_eq!(apply_monster_weak(&mut powers, 2), Ok(true));
        assert_eq!(apply_monster_vulnerable(&mut powers, 3), Ok(true));
        assert_eq!(reduce_monster_strength(&mut powers, 4), Ok(true));
        assert_eq!(powers.weak, 2);
        assert_eq!(powers.vulnerable, 3);
        assert_eq!(powers.strength, -1);
    }
}
