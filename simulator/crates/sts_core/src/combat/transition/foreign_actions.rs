//! Execution handlers for cross-color use actions. Unsupported variants are
//! explicit errors until their rules are implemented; never no-op fallbacks.
use crate::{
    action::{CardPile, ForeignAction, OrbKind, TempCardDestination},
    combat::{CombatOrb, CombatState},
    content::cards::upgrade_card_instance,
    InternalAction, SimError, SimResult,
};

pub(super) fn apply(
    state: &mut CombatState,
    action: ForeignAction,
) -> SimResult<Vec<InternalAction>> {
    match action {
        ForeignAction::CopiedUse { card, target } => {
            let effects = crate::combat::prismatic::play_card_queue(state, card, target, true)?;
            Ok(effects
                .into_iter()
                .filter(|action| {
                    !matches!(
                        action,
                        InternalAction::PlayCard { .. }
                            | InternalAction::SpendCardEnergy { .. }
                            | InternalAction::SpendEnergy { .. }
                            | InternalAction::MoveCard { .. }
                            | InternalAction::Foreign(ForeignAction::StageInLimbo { .. })
                    )
                })
                .collect())
        }
        ForeignAction::StageInLimbo { card_id } => {
            let card = super::remove_card_from_hand(state, card_id)?;
            state.piles.limbo.push(card);
            Ok(Vec::new())
        }
        ForeignAction::Channel { orb } => {
            if state.max_orbs <= 0 {
                return Ok(Vec::new());
            }
            if state.orbs.len() >= state.max_orbs as usize {
                // AbstractPlayer.channelOrb addToTops EvokeOrbAction followed
                // by ChannelAction, so evoke callbacks run before the retry.
                return Ok(vec![
                    InternalAction::Foreign(ForeignAction::EvokeFirst { remove: true }),
                    InternalAction::Foreign(ForeignAction::Channel { orb }),
                ]);
            }
            let orb = match orb {
                OrbKind::Lightning => CombatOrb::Lightning,
                OrbKind::Frost => CombatOrb::Frost,
                OrbKind::Dark => CombatOrb::Dark { evoke: 6 },
                OrbKind::Plasma => CombatOrb::Plasma,
            };
            super::player_actions::channel_orb(state, orb)
        }
        ForeignAction::EvokeFirst { remove } => {
            let Some(&orb) = state.orbs.first() else {
                return Ok(Vec::new());
            };
            if remove {
                state.orbs.remove(0);
            }
            super::player_actions::evoke_orb(state, orb)
        }
        ForeignAction::MakeTempCard {
            content_id,
            destination,
            count,
            upgraded,
        } => {
            if count < 0 {
                return Err(SimError::InvalidState("negative generated-card count"));
            }
            for _ in 0..count {
                let mut card = super::make_generated_card(state, content_id)?;
                if upgraded {
                    card = upgrade_card_instance(card)?.ok_or(SimError::InvalidState(
                        "generated card has no available upgrade",
                    ))?;
                }
                match destination {
                    TempCardDestination::Hand => {
                        if state.piles.hand.len() < super::MAX_HAND_SIZE {
                            super::apply_corruption_cost_to_generated_hand_card(state, &mut card);
                            super::push_card_to_pile(state, card, CardPile::Hand);
                        } else {
                            super::push_card_to_pile(state, card, CardPile::DiscardPile);
                        }
                    }
                    TempCardDestination::DiscardPile => {
                        super::push_card_to_pile(state, card, CardPile::DiscardPile);
                    }
                    TempCardDestination::DrawPileRandom => {
                        // CardGroup.addToRandomSpot rolls among existing positions;
                        // an empty pile has no draw. Preserve attribution here.
                        if state.piles.draw_pile.is_empty() {
                            state.piles.push_draw_top(card);
                        } else {
                            let index = state
                                .rng
                                .card_random_rng
                                .random_int((state.piles.draw_pile.len() - 1) as i32)
                                as usize;
                            state.piles.insert_draw_unknown_index(index, card);
                        }
                    }
                }
            }
            Ok(Vec::new())
        }
        ForeignAction::Gash { card_id, amount } => {
            use crate::content::prismatic::CLAW_ANY_COLOR_ID;
            for card in state
                .piles
                .hand
                .iter_mut()
                .chain(state.piles.draw_pile.iter_mut())
                .chain(state.piles.discard_pile.iter_mut())
                .chain(state.piles.limbo.iter_mut().filter(|c| c.id == card_id))
            {
                if card.content_id == CLAW_ANY_COLOR_ID {
                    card.base_damage_delta = card
                        .base_damage_delta
                        .checked_add(amount)
                        .ok_or(SimError::InvalidState("Claw damage growth overflows i32"))?;
                }
            }
            Ok(Vec::new())
        }
        ForeignAction::AggregateEnergy { divide } => {
            if divide <= 0 {
                return Err(SimError::InvalidState("Aggregate divisor must be positive"));
            }
            let size = i32::try_from(state.piles.draw_pile.len())
                .map_err(|_| SimError::InvalidState("Aggregate draw pile exceeds i32"))?;
            super::player_actions::gain_energy(state, size / divide)
        }
        ForeignAction::DoubleEnergy => {
            super::player_actions::gain_energy(state, state.player.energy)
        }
        ForeignAction::Expertise { hand_size } => {
            let current = i32::try_from(state.piles.hand.len())
                .map_err(|_| SimError::InvalidState("hand size exceeds i32"))?;
            let draw = hand_size
                .checked_sub(current)
                .ok_or(SimError::InvalidState("Expertise draw overflows i32"))?;
            Ok(if draw > 0 {
                vec![InternalAction::DrawCards {
                    count: draw as usize,
                }]
            } else {
                Vec::new()
            })
        }
        ForeignAction::GainVigor { amount } => {
            state.player.powers.vigor = state
                .player
                .powers
                .vigor
                .checked_add(amount)
                .ok_or(SimError::InvalidState("Vigor gain overflows i32"))?;
            Ok(Vec::new())
        }
        ForeignAction::GainBuffer { amount } => {
            state.player.powers.buffer = state
                .player
                .powers
                .buffer
                .checked_add(amount)
                .ok_or(SimError::InvalidState("Buffer gain overflows i32"))?;
            Ok(Vec::new())
        }
        ForeignAction::GainFocus { amount } => {
            if amount < 0 {
                crate::power::reduce_player_focus(
                    &mut state.player.powers,
                    amount
                        .checked_neg()
                        .ok_or(SimError::InvalidState("Focus reduction overflows i32"))?,
                )?;
            } else {
                let had_focus = state.player.powers.focus != 0;
                let focus = state
                    .player
                    .powers
                    .focus
                    .checked_add(amount)
                    .ok_or(SimError::InvalidState("Focus gain overflows i32"))?;
                state.player.powers.focus = if had_focus {
                    focus.clamp(-999, 999)
                } else {
                    focus
                };
            }
            Ok(Vec::new())
        }
        ForeignAction::MultiplyPoison { target, multiplier } => {
            let Some(monster) = super::living_monster_mut_opt(state, target) else {
                return Ok(Vec::new());
            };
            let poison = monster.powers.poison;
            if poison <= 0 {
                return Ok(Vec::new());
            }
            let amount = poison
                .checked_mul(multiplier)
                .ok_or(SimError::InvalidState(
                    "Catalyst poison multiplication overflows i32",
                ))?;
            Ok(vec![InternalAction::ApplyPoison { target, amount }])
        }
        _ => Err(SimError::InvalidState(
            "cross-color action is not implemented",
        )),
    }
}
