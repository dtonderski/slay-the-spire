use crate::{
    action::{CardPile, CombatAction, InternalAction},
    card::CardDefinition,
    combat::{
        apply_burning_blood,
        damage::{deal_damage_info_to_monster, DamageInfo, DamageSource},
        validate_combat_action, CombatPhase,
    },
    content::cards::{
        get_card_definition, ANGER_ID, ANGER_PLUS_ID, BASH_ID, CLEAVE_ID, CLEAVE_PLUS_ID,
        DEFEND_R_ID, SHRUG_IT_OFF_ID, SLIMED_ID, STRIKE_R_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID,
        TWIN_STRIKE_PLUS_ID,
    },
    ids::{CardId, ContentId, MonsterId},
    power::calculate_block,
    rng::SimulatorRng,
    CardInstance, CombatState, MonsterState, SimError, SimResult,
};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatTransition {
    pub state: CombatState,
    pub event_log: Vec<InternalAction>,
}

pub fn apply_combat_action(state: &CombatState, action: CombatAction) -> SimResult<CombatState> {
    Ok(apply_combat_action_with_events(state, action)?.state)
}

pub fn apply_combat_action_with_events(
    state: &CombatState,
    action: CombatAction,
) -> SimResult<CombatTransition> {
    validate_combat_action(state, action)?;

    match action {
        CombatAction::PlayCard { card_id, target } => apply_play_card(state, card_id, target),
        CombatAction::EndTurn => Ok(CombatTransition {
            state: crate::combat::end_player_turn(state),
            event_log: Vec::new(),
        }),
    }
}

fn apply_play_card(
    state: &CombatState,
    card_id: CardId,
    target: Option<MonsterId>,
) -> SimResult<CombatTransition> {
    let card = find_hand_card(state, card_id)?;
    let definition =
        get_card_definition(card.content_id).ok_or(SimError::UnknownContent(card.content_id))?;

    let queue = match definition.id {
        STRIKE_R_ID => strike_queue(card_id, target.expect("validated Strike has a target")),
        DEFEND_R_ID => defend_queue(card_id, definition),
        BASH_ID => bash_queue(card_id, target.expect("validated Bash has a target")),
        SLIMED_ID => slimed_queue(card_id, target.expect("validated Slimed has a target")),
        ANGER_ID | ANGER_PLUS_ID => anger_queue(
            card_id,
            target.expect("validated Anger has a target"),
            definition,
        ),
        CLEAVE_ID | CLEAVE_PLUS_ID => cleave_queue(card_id, definition),
        TWIN_STRIKE_ID | TWIN_STRIKE_PLUS_ID => twin_strike_queue(
            card_id,
            target.expect("validated Twin Strike has a target"),
            definition,
        ),
        SHRUG_IT_OFF_ID => shrug_it_off_queue(card_id),
        TRUE_GRIT_ID => true_grit_queue(state, card_id),
        _ if definition.values.damage.is_some()
            && definition.target == crate::TargetRequirement::Enemy =>
        {
            generic_attack_queue(
                card_id,
                target.expect("validated attack has a target"),
                definition,
            )
        }
        _ if definition.values.block.is_some() => generic_skill_queue(card_id, definition),
        _ => Err(SimError::IllegalAction(
            "card transition is not implemented",
        ))?,
    };

    process_internal_queue(state, queue?)
}

fn card_move_destination(definition: &CardDefinition) -> CardPile {
    if definition.keywords.exhaust {
        CardPile::ExhaustPile
    } else {
        CardPile::DiscardPile
    }
}

fn strike_queue(card_id: CardId, target: MonsterId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 1 },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: 6,
            },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn generic_attack_queue(
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: definition.values.damage.unwrap_or(0),
            },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn defend_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 1 },
        InternalAction::GainBlock { amount: 5 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn generic_skill_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainBlock {
            amount: definition.values.block.unwrap_or(0),
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn slimed_queue(card_id: CardId, target: MonsterId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 1 },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: 0,
            },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        },
        InternalAction::AddCardToPile {
            content_id: SLIMED_ID,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn bash_queue(card_id: CardId, target: MonsterId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 2 },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: 8,
            },
        },
        InternalAction::ApplyVulnerable { target, amount: 2 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn anger_queue(
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: definition.values.damage.unwrap_or(0),
            },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
        InternalAction::AddCardToPile {
            content_id: definition.id,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn cleave_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DealDamageAll {
            source: card_id,
            amount: definition.values.damage.unwrap_or(0),
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn twin_strike_queue(
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let damage = definition.values.damage.unwrap_or(0);
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: damage,
            },
        },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: damage,
            },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn shrug_it_off_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 1 },
        InternalAction::GainBlock { amount: 8 },
        InternalAction::DrawCards { count: 1 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn true_grit_exhaust_target(state: &CombatState, true_grit_id: CardId) -> Option<CardId> {
    state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != true_grit_id)
        .min_by_key(|card| card.id.get())
        .map(|card| card.id)
}

fn true_grit_queue(state: &CombatState, card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 1 },
        InternalAction::GainBlock { amount: 7 },
    ]);

    if let Some(exhaust_target) = true_grit_exhaust_target(state, card_id) {
        queue.push_back(InternalAction::MoveCard {
            card_id: exhaust_target,
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        });
    }

    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: CardPile::DiscardPile,
    });

    Ok(queue)
}

fn process_internal_queue(
    state: &CombatState,
    mut queue: VecDeque<InternalAction>,
) -> SimResult<CombatTransition> {
    let mut next = state.clone();
    let mut event_log = Vec::new();

    while let Some(internal_action) = queue.pop_front() {
        apply_internal_action(&mut next, internal_action)?;
        event_log.push(internal_action);
    }

    if next.monsters.iter().all(|monster| !monster.alive) {
        next.phase = CombatPhase::Won;
        apply_burning_blood(&mut next);
    } else {
        next.phase = CombatPhase::WaitingForPlayer;
    }

    Ok(CombatTransition {
        state: next,
        event_log,
    })
}

fn apply_internal_action(state: &mut CombatState, action: InternalAction) -> SimResult<()> {
    match action {
        InternalAction::PlayCard { .. } => Ok(()),
        InternalAction::SpendEnergy { amount } => {
            state.player.energy -= amount;
            Ok(())
        }
        InternalAction::DealDamage { info } => {
            let player_powers = state.player.powers;
            let monster = living_monster_mut(state, info.target)?;
            deal_damage_info_to_monster(monster, info, player_powers);
            Ok(())
        }
        InternalAction::DealDamageAll { source, amount } => {
            let player_powers = state.player.powers;
            let targets: Vec<MonsterId> = state
                .monsters
                .iter()
                .filter(|monster| monster.alive)
                .map(|monster| monster.id)
                .collect();
            for target in targets {
                let monster = living_monster_mut(state, target)?;
                deal_damage_info_to_monster(
                    monster,
                    DamageInfo {
                        source: DamageSource::Card(source),
                        target,
                        amount,
                    },
                    player_powers,
                );
            }
            Ok(())
        }
        InternalAction::GainBlock { amount } => {
            let gained = calculate_block(amount, state.player.powers);
            state.player.block += gained;
            Ok(())
        }
        InternalAction::ApplyVulnerable { target, amount } => {
            let monster = living_monster_mut(state, target)?;
            monster.powers.vulnerable += amount;
            Ok(())
        }
        InternalAction::MoveCard { card_id, from, to } => move_card(state, card_id, from, to),
        InternalAction::AddCardToPile { content_id, to } => {
            add_card_to_pile(state, content_id, to);
            Ok(())
        }
        InternalAction::DrawCards { count } => {
            let mut rng = SimulatorRng::new(0);
            crate::combat::draw::draw_cards(state, count, &mut rng);
            Ok(())
        }
    }
}

fn add_card_to_pile(state: &mut CombatState, content_id: ContentId, to: CardPile) {
    let next_id = CardId::new(state.piles.max_card_instance_id() + 1);
    let card = CardInstance::new(next_id, content_id);
    match to {
        CardPile::DiscardPile => state.piles.discard_pile.push(card),
        CardPile::DrawPile => state.piles.draw_pile.push(card),
        CardPile::Hand => state.piles.hand.push(card),
        CardPile::ExhaustPile => state.piles.exhaust_pile.push(card),
    }
}

fn living_monster_mut(state: &mut CombatState, target: MonsterId) -> SimResult<&mut MonsterState> {
    state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == target && monster.alive)
        .ok_or(SimError::IllegalAction("target is not a living monster"))
}

fn find_hand_card(state: &CombatState, card_id: CardId) -> SimResult<CardInstance> {
    state
        .piles
        .hand
        .iter()
        .copied()
        .find(|card| card.id == card_id)
        .ok_or(SimError::UnknownCard(card_id))
}

fn remove_card_from_hand(state: &mut CombatState, card_id: CardId) -> SimResult<CardInstance> {
    let index = state
        .piles
        .hand
        .iter()
        .position(|card| card.id == card_id)
        .ok_or(SimError::UnknownCard(card_id))?;

    Ok(state.piles.hand.remove(index))
}

fn move_card(
    state: &mut CombatState,
    card_id: CardId,
    from: CardPile,
    to: CardPile,
) -> SimResult<()> {
    let card = match from {
        CardPile::Hand => remove_card_from_hand(state, card_id)?,
        CardPile::DrawPile | CardPile::DiscardPile | CardPile::ExhaustPile => {
            return Err(SimError::IllegalAction(
                "card move source is not implemented",
            ));
        }
    };

    match to {
        CardPile::DiscardPile => {
            state.piles.discard_pile.push(card);
            Ok(())
        }
        CardPile::ExhaustPile => {
            state.piles.exhaust_pile.push(card);
            Ok(())
        }
        CardPile::Hand | CardPile::DrawPile => Err(SimError::IllegalAction(
            "card move destination is not implemented",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{
        ANGER_ID, ANGER_PLUS_ID, BASH_ID, CLEAVE_ID, CLEAVE_PLUS_ID, DEFEND_R_ID, SHRUG_IT_OFF_ID,
        SLIMED_ID, STRIKE_R_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, TWIN_STRIKE_PLUS_ID,
    };

    #[test]
    fn strike_decreases_monster_hp_by_six() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 6);
    }

    #[test]
    fn strike_decreases_energy_by_one() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
    }

    #[test]
    fn strike_moves_card_from_hand_to_discard() {
        let state = CombatState::initial_fixture();
        let strike_id = hand_strike_id(&state);

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert!(!next.piles.hand.iter().any(|card| card.id == strike_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == strike_id));
    }

    #[test]
    fn strike_can_win_combat() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].hp = 6;

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.phase, CombatPhase::Won);
        assert!(!next.monsters[0].alive);
    }

    #[test]
    fn invalid_target_returns_error_and_preserves_state() {
        let state = CombatState::initial_fixture();
        let before = state.snapshot().hash().expect("state hashes before");

        let result = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: hand_strike_id(&state),
                target: Some(MonsterId::new(99)),
            },
        );

        assert_eq!(
            result,
            Err(SimError::IllegalAction("target is not a living monster"))
        );
        assert_eq!(state.snapshot().hash().expect("state hashes after"), before);
    }

    #[test]
    fn defend_increases_player_block_by_five() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(next.player.block, state.player.block + 5);
    }

    #[test]
    fn defend_decreases_energy_by_one() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
    }

    #[test]
    fn defend_moves_card_from_hand_to_discard() {
        let state = CombatState::initial_fixture();
        let defend_id = hand_card_id(&state, DEFEND_R_ID);

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert!(!next.piles.hand.iter().any(|card| card.id == defend_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == defend_id));
    }

    #[test]
    fn bash_decreases_monster_hp_by_eight() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 8);
    }

    #[test]
    fn bash_adds_two_vulnerable_to_monster() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");

        assert_eq!(next.monsters[0].powers.vulnerable, 2);
    }

    #[test]
    fn bash_decreases_energy_by_two_and_moves_to_discard() {
        let state = CombatState::initial_fixture();
        let bash_id = hand_card_id(&state, BASH_ID);

        let next = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");

        assert_eq!(next.player.energy, state.player.energy - 2);
        assert!(!next.piles.hand.iter().any(|card| card.id == bash_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == bash_id));
    }

    #[test]
    fn bash_is_illegal_with_less_than_two_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;

        assert_eq!(
            apply_combat_action(&state, bash_action(&state)),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn bash_then_strike_deals_expected_damage_with_vulnerable() {
        let state = CombatState::initial_fixture();
        let after_bash = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");
        let after_strike =
            apply_combat_action(&after_bash, strike_action(&after_bash)).expect("Strike applies");

        assert_eq!(after_bash.monsters[0].hp, state.monsters[0].hp - 8);
        assert_eq!(after_bash.monsters[0].powers.vulnerable, 2);
        assert_eq!(after_strike.monsters[0].hp, after_bash.monsters[0].hp - 9);
    }

    #[test]
    fn defend_with_dexterity_gains_extra_block() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.dexterity = 2;

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(next.player.block, 7);
    }

    #[test]
    fn defend_with_frail_gains_reduced_block() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.frail = 1;

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(next.player.block, 3);
    }

    #[test]
    fn slimed_exhausts_and_adds_slimed_to_discard() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), SLIMED_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Slimed applies");

        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == SLIMED_ID));
    }

    #[test]
    fn strike_event_log_records_ordered_internal_actions() {
        let state = CombatState::initial_fixture();
        let strike_id = hand_strike_id(&state);

        let transition =
            apply_combat_action_with_events(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard { card_id: strike_id },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::DealDamage {
                    info: DamageInfo {
                        source: DamageSource::Card(strike_id),
                        target: MonsterId::new(1),
                        amount: 6,
                    },
                },
                InternalAction::MoveCard {
                    card_id: strike_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn defend_event_log_records_gain_block() {
        let state = CombatState::initial_fixture();
        let defend_id = hand_card_id(&state, DEFEND_R_ID);

        let transition =
            apply_combat_action_with_events(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard { card_id: defend_id },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::GainBlock { amount: 5 },
                InternalAction::MoveCard {
                    card_id: defend_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn bash_damage_event_log_includes_source_target_and_amount() {
        let state = CombatState::initial_fixture();
        let bash_id = hand_card_id(&state, BASH_ID);

        let transition =
            apply_combat_action_with_events(&state, bash_action(&state)).expect("Bash applies");

        assert!(transition.event_log.contains(&InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(bash_id),
                target: MonsterId::new(1),
                amount: 8,
            },
        }));
    }

    #[test]
    fn anger_deals_six_damage_without_spending_energy() {
        let state = hand_only(ANGER_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Anger applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 6);
        assert_eq!(next.player.energy, state.player.energy);
    }

    #[test]
    fn anger_adds_copy_to_discard() {
        let state = hand_only(ANGER_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Anger applies");

        assert_eq!(next.piles.discard_pile.len(), 2);
        assert_eq!(
            next.piles
                .discard_pile
                .iter()
                .filter(|card| card.content_id == ANGER_ID)
                .count(),
            2
        );
    }

    #[test]
    fn anger_plus_deals_seven_damage_and_copies_upgraded_card() {
        let state = hand_only(ANGER_PLUS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Anger+ applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 7);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == ANGER_PLUS_ID));
    }

    #[test]
    fn anger_played_twice_grows_discard_copies() {
        let state = hand_only(ANGER_ID);

        let after_first = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("first Anger applies");
        let anger_copy = after_first
            .piles
            .discard_pile
            .iter()
            .find(|card| card.content_id == ANGER_ID && card.id != CardId::new(20))
            .expect("Anger copy is in discard")
            .id;
        let mut second_hand = after_first.clone();
        second_hand
            .piles
            .discard_pile
            .retain(|card| card.id != anger_copy);
        second_hand.piles.hand = vec![CardInstance::new(anger_copy, ANGER_ID)];

        let after_second = apply_combat_action(
            &second_hand,
            CombatAction::PlayCard {
                card_id: anger_copy,
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("second Anger applies");

        assert_eq!(
            after_second
                .piles
                .discard_pile
                .iter()
                .filter(|card| card.content_id == ANGER_ID)
                .count(),
            3
        );
    }

    #[test]
    fn cleave_deals_eight_to_all_enemies() {
        let state = two_monster_hand(CLEAVE_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Cleave applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 8);
        assert_eq!(next.monsters[1].hp, state.monsters[1].hp - 8);
        assert_eq!(next.player.energy, state.player.energy - 1);
    }

    #[test]
    fn cleave_plus_deals_nine_to_all_enemies() {
        let state = two_monster_hand(CLEAVE_PLUS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Cleave+ applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 9);
        assert_eq!(next.monsters[1].hp, state.monsters[1].hp - 9);
    }

    #[test]
    fn twin_strike_deals_five_damage_twice() {
        let state = hand_only(TWIN_STRIKE_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Twin Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 10);
        assert_eq!(next.player.energy, state.player.energy - 1);
    }

    #[test]
    fn twin_strike_plus_deals_six_damage_twice() {
        let state = hand_only(TWIN_STRIKE_PLUS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Twin Strike+ applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 12);
    }

    #[test]
    fn twin_strike_with_vulnerable_deals_bonus_on_both_hits() {
        let mut state = hand_only(TWIN_STRIKE_ID);
        state.monsters[0].powers.vulnerable = 1;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Twin Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 14);
    }

    #[test]
    fn shrug_it_off_gains_eight_block_draws_one_and_moves_to_discard() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), SHRUG_IT_OFF_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Shrug It Off applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.player.block, 8);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(30)));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn shrug_it_off_event_log_records_draw_cards() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), SHRUG_IT_OFF_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let transition = apply_combat_action_with_events(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Shrug It Off applies");

        assert!(transition
            .event_log
            .contains(&InternalAction::DrawCards { count: 1 }));
    }

    #[test]
    fn true_grit_gains_seven_block_and_exhausts_lowest_other_hand_card() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(25), TRUE_GRIT_ID),
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("True Grit applies");

        assert_eq!(next.player.block, 7);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(25)));
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(30)));
    }

    #[test]
    fn true_grit_with_only_itself_in_hand_does_not_exhaust() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(25), TRUE_GRIT_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("True Grit applies");

        assert_eq!(next.player.block, 7);
        assert!(next.piles.exhaust_pile.is_empty());
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(25)));
    }

    fn hand_only(content_id: crate::ContentId) -> CombatState {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), content_id)];
        state
    }

    fn two_monster_hand(content_id: crate::ContentId) -> CombatState {
        use crate::content::monsters::{monster_state, FIXED_SIMPLE_MONSTER};

        let mut state = hand_only(content_id);
        state
            .monsters
            .push(monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(2)));
        state
    }

    fn strike_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_strike_id(state),
            target: Some(MonsterId::new(1)),
        }
    }

    fn hand_strike_id(state: &CombatState) -> CardId {
        hand_card_id(state, STRIKE_R_ID)
    }

    fn defend_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, DEFEND_R_ID),
            target: None,
        }
    }

    fn bash_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, BASH_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn hand_card_id(state: &CombatState, content_id: crate::ContentId) -> CardId {
        state
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == content_id)
            .expect("card is in hand")
            .id
    }
}
