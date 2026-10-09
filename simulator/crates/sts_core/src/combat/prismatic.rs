//! Cross-color card use queues, transcribed from the audited PC 12-18-2022
//! `com.megacrit.cardcrawl.cards.{green,blue,purple}` classes.
//!
//! Registration is not implementation: cards without a completed use handler
//! fail explicitly. No fallback attack, substitute, or no-op is permitted.

use crate::{
    action::{CardPile, ForeignAction, InternalAction, OrbKind, TempCardDestination},
    card::CardInstance,
    combat::{CombatState, DamageInfo, DamageSource},
    content::{cards, prismatic::*},
    MonsterId, SimError, SimResult,
};
use std::collections::VecDeque;

/// GameActionManager onPlayCard pile scans and public card-play history.
/// ForceField.triggerOnCardPlayed updates hand, discard, and draw; it does not
/// visit the exhaust pile or the source in limbo.
pub(crate) fn on_card_play(
    state: &mut CombatState,
    card_type: crate::card::CardType,
) -> SimResult<()> {
    state.foreign.previous_played_card_type = state.last_played_card_type;
    if card_type == crate::card::CardType::Attack {
        state.foreign.attacks_played_this_turn = state
            .foreign
            .attacks_played_this_turn
            .checked_add(1)
            .ok_or(SimError::InvalidState("attack history overflows u32"))?;
    }
    if card_type == crate::card::CardType::Power {
        state.foreign.powers_played_this_combat = state
            .foreign
            .powers_played_this_combat
            .checked_add(1)
            .ok_or(SimError::InvalidState("power history overflows u32"))?;
        for card in state
            .piles
            .hand
            .iter_mut()
            .chain(state.piles.discard_pile.iter_mut())
            .chain(state.piles.draw_pile.iter_mut())
        {
            if card.content_id == FORCE_FIELD_ANY_COLOR_ID {
                super::cost::update_card_cost(card, -1)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn play_card_queue(
    state: &CombatState,
    card: CardInstance,
    target: Option<MonsterId>,
    purge_on_use: bool,
) -> SimResult<VecDeque<InternalAction>> {
    let spec =
        prismatic_card_spec(card.content_id).ok_or(SimError::UnknownContent(card.content_id))?;
    let upgraded = card.upgrades > 0;
    let mut damage = spec
        .damage(upgraded)
        .checked_add(card.base_damage_delta)
        .ok_or(SimError::InvalidState(
            "cross-color base damage overflows i32",
        ))?;
    if matches!(
        card.content_id,
        METEOR_STRIKE_ANY_COLOR_ID | THUNDER_STRIKE_ANY_COLOR_ID
    ) {
        damage = crate::relic::strike_damage_with_relics(&state.player.authority.relics, damage);
    }
    if card.content_id == BRILLIANCE_ANY_COLOR_ID {
        damage = damage
            .checked_add(state.foreign.mantra_gained_this_combat)
            .ok_or(SimError::InvalidState("Brilliance damage overflows i32"))?;
    }
    if card.content_id == BLIZZARD_ANY_COLOR_ID {
        damage = i32::try_from(state.foreign.frost_channeled_this_combat)
            .ok()
            .and_then(|count| count.checked_mul(spec.magic(upgraded)))
            .ok_or(SimError::InvalidState("Blizzard damage overflows i32"))?;
    }
    let block = spec
        .block(upgraded)
        .checked_add(card.base_block_delta)
        .ok_or(SimError::InvalidState(
            "cross-color base block overflows i32",
        ))?;
    let magic = spec.magic(upgraded);
    let hit = || -> SimResult<InternalAction> {
        Ok(InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card.id),
                target: target.ok_or(SimError::IllegalAction("card requires an enemy target"))?,
                amount: damage,
            },
        })
    };
    let all = InternalAction::DealDamageAll {
        source: card.id,
        amount: damage,
    };
    let gain_block = InternalAction::GainBlock { amount: block };
    let draw = |count| InternalAction::DrawCards { count };
    let channel = |orb| InternalAction::Foreign(ForeignAction::Channel { orb });
    let make = |content_id, destination, count| {
        InternalAction::Foreign(ForeignAction::MakeTempCard {
            content_id,
            destination,
            count,
            upgraded: false,
        })
    };
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id: card.id },
        InternalAction::SpendCardEnergy { card_id: card.id },
        InternalAction::Foreign(ForeignAction::StageInLimbo { card_id: card.id }),
    ]);
    use TempCardDestination::{DiscardPile, DrawPileRandom, Hand};
    match card.content_id {
        BACKSTAB_ANY_COLOR_ID
        | ENDLESS_AGONY_ANY_COLOR_ID
        | MASTERFUL_STAB_ANY_COLOR_ID
        | SIGNATURE_MOVE_ANY_COLOR_ID
        | WEAVE_ANY_COLOR_ID
        | BRILLIANCE_ANY_COLOR_ID
        | SMITE_ID
        | THROUGH_VIOLENCE_ID => queue.push_back(hit()?),
        CONSECRATE_ANY_COLOR_ID
        | DIE_DIE_DIE_ANY_COLOR_ID
        | GRAND_FINALE_ANY_COLOR_ID
        | BLIZZARD_ANY_COLOR_ID => {
            queue.push_back(all);
        }
        DEFLECT_ANY_COLOR_ID | FORCE_FIELD_ANY_COLOR_ID | PERSEVERANCE_ANY_COLOR_ID | SAFETY_ID => {
            queue.push_back(gain_block)
        }
        AUTO_SHIELDS_ANY_COLOR_ID => {
            if state.player.block == 0 {
                queue.push_back(gain_block);
            }
        }
        STACK_ANY_COLOR_ID => {
            let base = i32::try_from(state.piles.discard_pile.len())
                .map_err(|_| SimError::InvalidState("Stack pile size exceeds i32"))?
                .checked_add(if upgraded { 3 } else { 0 })
                .ok_or(SimError::InvalidState("Stack block overflows i32"))?;
            queue.push_back(InternalAction::GainBlock { amount: base });
        }
        SPIRIT_SHIELD_ANY_COLOR_ID => {
            let count = state
                .piles
                .hand
                .iter()
                .filter(|c| purge_on_use || c.id != card.id)
                .count();
            let base = i32::try_from(count)
                .ok()
                .and_then(|count| count.checked_mul(magic))
                .ok_or(SimError::InvalidState("Spirit Shield block overflows i32"))?;
            queue.push_back(InternalAction::GainBlock { amount: base });
        }
        CLAW_ANY_COLOR_ID => {
            queue.extend([
                hit()?,
                InternalAction::Foreign(ForeignAction::Gash {
                    card_id: card.id,
                    amount: magic,
                }),
            ]);
        }
        CATALYST_ANY_COLOR_ID => {
            queue.push_back(InternalAction::Foreign(ForeignAction::MultiplyPoison {
                target: target.ok_or(SimError::IllegalAction("card requires an enemy target"))?,
                multiplier: if upgraded { 2 } else { 1 },
            }))
        }
        AGGREGATE_ANY_COLOR_ID => {
            queue.push_back(InternalAction::Foreign(ForeignAction::AggregateEnergy {
                divide: magic,
            }))
        }
        DOUBLE_ENERGY_ANY_COLOR_ID => {
            queue.push_back(InternalAction::Foreign(ForeignAction::DoubleEnergy))
        }
        DEFRAGMENT_ANY_COLOR_ID => {
            queue.push_back(InternalAction::Foreign(ForeignAction::GainFocus {
                amount: magic,
            }))
        }
        HYPERBEAM_ANY_COLOR_ID => queue.extend([
            all,
            InternalAction::Foreign(ForeignAction::GainFocus { amount: -magic }),
        ]),
        REPROGRAM_ANY_COLOR_ID => queue.extend([
            InternalAction::Foreign(ForeignAction::GainFocus { amount: -magic }),
            InternalAction::GainStrength { amount: magic },
            InternalAction::GainDexterity { amount: magic },
        ]),
        EXPERTISE_ANY_COLOR_ID => {
            queue.push_back(InternalAction::Foreign(ForeignAction::Expertise {
                hand_size: magic,
            }))
        }
        SCRAWL_ANY_COLOR_ID => queue.push_back(InternalAction::Foreign(ForeignAction::Expertise {
            hand_size: 10,
        })),
        BUFFER_ANY_COLOR_ID => {
            queue.push_back(InternalAction::Foreign(ForeignAction::GainBuffer {
                amount: magic,
            }))
        }
        WREATH_OF_FLAME_ANY_COLOR_ID => {
            queue.push_back(InternalAction::Foreign(ForeignAction::GainVigor {
                amount: magic,
            }))
        }
        EXPUNGER_ID => {
            for _ in 0..card.x_magic {
                queue.push_back(hit()?);
            }
        }
        BALL_LIGHTNING_ANY_COLOR_ID | COLD_SNAP_ANY_COLOR_ID => {
            queue.push_back(hit()?);
            let orb = if card.content_id == BALL_LIGHTNING_ANY_COLOR_ID {
                OrbKind::Lightning
            } else {
                OrbKind::Frost
            };
            for _ in 0..magic {
                queue.push_back(channel(orb));
            }
        }
        BLADE_DANCE_ANY_COLOR_ID => {
            queue.push_back(make(cards::SHIV_ANY_COLOR_ID, Hand, magic));
        }
        EMPTY_FIST_ANY_COLOR_ID => {
            queue.extend([hit()?, InternalAction::ExitCalm]);
        }
        SUCKER_PUNCH_ANY_COLOR_ID => {
            queue.extend([
                hit()?,
                InternalAction::ApplyWeak {
                    target: target
                        .ok_or(SimError::IllegalAction("card requires an enemy target"))?,
                    amount: magic,
                },
            ]);
        }
        TURBO_ANY_COLOR_ID => {
            queue.extend([
                InternalAction::GainEnergy { amount: magic },
                make(cards::VOID_ID, DiscardPile, 1),
            ]);
        }
        CARVE_REALITY_ANY_COLOR_ID => {
            queue.extend([hit()?, make(SMITE_ID, Hand, 1)]);
        }
        CHILL_ANY_COLOR_ID => {
            for _ in 0..state
                .monsters
                .iter()
                .filter(|m| m.alive && !m.escaped)
                .count()
            {
                for _ in 0..magic {
                    queue.push_back(channel(OrbKind::Frost));
                }
            }
        }
        DASH_ANY_COLOR_ID => queue.extend([gain_block, hit()?]),
        DECEIVE_REALITY_ANY_COLOR_ID => queue.extend([gain_block, make(SAFETY_ID, Hand, 1)]),
        DOOM_AND_GLOOM_ANY_COLOR_ID => queue.extend([all, channel(OrbKind::Dark)]),
        FOOTWORK_ANY_COLOR_ID => queue.push_back(InternalAction::GainDexterity { amount: magic }),
        FUSION_ANY_COLOR_ID => {
            for _ in 0..magic {
                queue.push_back(channel(OrbKind::Plasma));
            }
        }
        GLACIER_ANY_COLOR_ID => {
            queue.push_back(gain_block);
            for _ in 0..magic {
                queue.push_back(channel(OrbKind::Frost));
            }
        }
        REACH_HEAVEN_ANY_COLOR_ID => {
            queue.extend([hit()?, make(THROUGH_VIOLENCE_ID, DrawPileRandom, 1)]);
        }
        RIDDLE_WITH_HOLES_ANY_COLOR_ID => {
            for _ in 0..5 {
                queue.push_back(hit()?);
            }
        }
        OVERCLOCK_ANY_COLOR_ID => {
            queue.extend([draw(magic as usize), make(cards::BURN_ID, DiscardPile, 1)])
        }
        TERROR_ANY_COLOR_ID => queue.push_back(InternalAction::ApplyVulnerable {
            target: target.ok_or(SimError::IllegalAction("card requires an enemy target"))?,
            amount: 99,
        }),
        WHEEL_KICK_ANY_COLOR_ID => queue.extend([hit()?, draw(magic as usize)]),
        WORSHIP_ANY_COLOR_ID => queue.push_back(InternalAction::GainMantra { amount: magic }),
        ADRENALINE_ANY_COLOR_ID => queue.extend([
            InternalAction::GainEnergy {
                amount: if upgraded { 2 } else { 1 },
            },
            draw(2),
        ]),
        ALPHA_ANY_COLOR_ID => queue.push_back(make(BETA_ID, DrawPileRandom, 1)),
        CORE_SURGE_ANY_COLOR_ID => {
            queue.extend([hit()?, InternalAction::GainArtifact { amount: magic }])
        }
        METEOR_STRIKE_ANY_COLOR_ID => {
            queue.push_back(hit()?);
            for _ in 0..magic {
                queue.push_back(channel(OrbKind::Plasma));
            }
        }
        RAINBOW_ANY_COLOR_ID => queue.extend([
            channel(OrbKind::Lightning),
            channel(OrbKind::Frost),
            channel(OrbKind::Dark),
        ]),
        MIRACLE_ID => queue.push_back(InternalAction::GainEnergy {
            amount: if upgraded { 2 } else { 1 },
        }),
        BETA_ID => queue.push_back(make(OMEGA_ID, DrawPileRandom, 1)),
        BECOME_ALMIGHTY_ID => queue.push_back(InternalAction::GainStrength { amount: magic }),
        // Remaining queues are deliberately fail-closed until their real
        // actions and lifecycle hooks have been implemented and tested.
        _ => return Err(SimError::UnsupportedMechanic(card.content_id)),
    }
    let keywords = spec.keywords(upgraded);
    queue.push_back(InternalAction::MoveCard {
        card_id: card.id,
        from: CardPile::Hand,
        to: if keywords.exhaust {
            CardPile::ExhaustPile
        } else {
            CardPile::DiscardPile
        },
    });
    Ok(queue)
}
