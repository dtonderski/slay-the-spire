use crate::{
    action::{CardPile, CombatAction, InternalAction},
    card::{CardDefinition, CardType, TargetRequirement},
    combat::{
        apply_burning_blood,
        damage::{deal_damage_info_to_monster, reflect_spikes_to_player, DamageInfo, DamageSource},
        validate_combat_action, CombatPhase,
    },
    content::cards::{
        get_card_definition, ANGER_ID, ANGER_PLUS_ID, BASH_ID, BATTLE_TRANCE_ID,
        BATTLE_TRANCE_PLUS_ID, BURNING_PACT_ID, CLEAVE_ID, CLEAVE_PLUS_ID, DARK_EMBRACE_ID,
        DEFEND_R_ID, DEMON_FORM_ID, DRAMATIC_ENTRANCE_ID, DUAL_WIELD_ID, DUAL_WIELD_PLUS_ID,
        FEEL_NO_PAIN_ID, FLEX_ID, FLEX_PLUS_ID, HAVOC_ID, HAVOC_PLUS_ID, IMMOLATE_ID, INFLAME_ID,
        INFLAME_PLUS_ID, METALLICIZE_ID, POMMEL_STRIKE_ID, POMMEL_STRIKE_PLUS_ID, SEARING_BLOW_ID,
        SEARING_BLOW_PLUS_ID, SEEING_RED_ID, SEEING_RED_PLUS_ID, SEVER_SOUL_ID, SHRUG_IT_OFF_ID,
        SLIMED_ID, SPOT_WEAKNESS_ID, SPOT_WEAKNESS_PLUS_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID,
        SWORD_BOOMERANG_ID, THUNDERCLAP_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, TWIN_STRIKE_PLUS_ID,
        UPPERCUT_ID, WARCRY_ID, WARCRY_PLUS_ID, WHIRLWIND_ID, WHIRLWIND_PLUS_ID,
    },
    content::monsters::{
        check_slime_boss_split, get_monster_definition, guardian_on_hp_damage,
        wake_lagavulin_on_damage,
    },
    ids::{CardId, ContentId, MonsterId},
    power::calculate_block,
    rng::SimulatorRng,
    CardInstance, CombatState, MonsterIntent, MonsterState, SimError, SimResult,
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

pub fn apply_play_top_draw_card_action(
    state: &CombatState,
    target: Option<MonsterId>,
) -> SimResult<CombatState> {
    Ok(process_internal_queue(
        state,
        VecDeque::from([InternalAction::PlayTopDrawCard { target }]),
    )?
    .state)
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
        CLEAVE_ID | CLEAVE_PLUS_ID | DRAMATIC_ENTRANCE_ID => cleave_queue(card_id, definition),
        IMMOLATE_ID => immolate_queue(card_id, definition),
        TWIN_STRIKE_ID | TWIN_STRIKE_PLUS_ID => twin_strike_queue(
            card_id,
            target.expect("validated Twin Strike has a target"),
            definition,
        ),
        SHRUG_IT_OFF_ID => shrug_it_off_queue(card_id),
        TRUE_GRIT_ID => true_grit_queue(state, card_id),
        BURNING_PACT_ID => burning_pact_queue(state, card_id),
        FEEL_NO_PAIN_ID => feel_no_pain_queue(card_id),
        DARK_EMBRACE_ID => dark_embrace_queue(card_id),
        DEMON_FORM_ID => demon_form_queue(card_id),
        METALLICIZE_ID => metallicize_queue(card_id, definition),
        POMMEL_STRIKE_ID | POMMEL_STRIKE_PLUS_ID => pommel_strike_queue(
            card_id,
            target.expect("validated Pommel Strike has a target"),
            definition,
        ),
        BATTLE_TRANCE_ID | BATTLE_TRANCE_PLUS_ID => battle_trance_queue(card_id, definition),
        SEEING_RED_ID | SEEING_RED_PLUS_ID => seeing_red_queue(card_id, definition),
        INFLAME_ID | INFLAME_PLUS_ID => inflame_queue(card_id, definition),
        FLEX_ID | FLEX_PLUS_ID => flex_queue(card_id, definition),
        SPOT_WEAKNESS_ID | SPOT_WEAKNESS_PLUS_ID => spot_weakness_queue(state, card_id, definition),
        THUNDERCLAP_ID => thunderclap_queue(state, card_id, definition),
        UPPERCUT_ID => uppercut_queue(
            card_id,
            target.expect("validated Uppercut has a target"),
            definition,
        ),
        SWORD_BOOMERANG_ID => sword_boomerang_queue(state, card_id, definition),
        WHIRLWIND_ID | WHIRLWIND_PLUS_ID => whirlwind_queue(state, card_id, definition),
        HAVOC_ID | HAVOC_PLUS_ID => havoc_queue(state, card_id, definition, target),
        WARCRY_ID | WARCRY_PLUS_ID => warcry_queue(state, card_id, definition),
        DUAL_WIELD_ID | DUAL_WIELD_PLUS_ID => dual_wield_queue(state, card_id, definition),
        SEARING_BLOW_ID | SEARING_BLOW_PLUS_ID => generic_attack_queue(
            card_id,
            target.expect("validated Searing Blow has a target"),
            definition,
        ),
        SEVER_SOUL_ID => sever_soul_queue(
            state,
            card_id,
            target.expect("validated Sever Soul has a target"),
            definition,
        ),
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

    let mut queue = queue?;
    if state.duplication_potion_pending {
        queue = apply_duplication_potion_to_queue(queue, card_id);
    }

    process_internal_queue(state, queue)
}

fn apply_duplication_potion_to_queue(
    mut queue: VecDeque<InternalAction>,
    card_id: CardId,
) -> VecDeque<InternalAction> {
    let mut duplicated_effects = queue
        .iter()
        .copied()
        .filter(|action| is_duplicated_card_effect(*action, card_id))
        .collect::<VecDeque<_>>();

    let final_move = queue
        .back()
        .copied()
        .filter(|action| is_card_move_for(*action, card_id));
    if final_move.is_some() {
        queue.pop_back();
    }

    queue.push_front(InternalAction::ConsumeDuplicationPotion);
    queue.append(&mut duplicated_effects);
    if let Some(action) = final_move {
        queue.push_back(action);
    }

    queue
}

fn is_duplicated_card_effect(action: InternalAction, card_id: CardId) -> bool {
    !matches!(
        action,
        InternalAction::PlayCard { .. }
            | InternalAction::SpendEnergy { .. }
            | InternalAction::SpendCardEnergy { .. }
            | InternalAction::MoveCard { .. }
            | InternalAction::AwaitHandSelect { .. }
    ) && !is_card_move_for(action, card_id)
}

fn is_card_move_for(action: InternalAction, card_id: CardId) -> bool {
    matches!(action, InternalAction::MoveCard { card_id: moved, .. } if moved == card_id)
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

fn sword_boomerang_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let living_targets = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster.id)
        .collect::<Vec<_>>();
    let first = living_targets
        .first()
        .copied()
        .ok_or(SimError::InvalidState(
            "Sword Boomerang requires a living monster",
        ))?;
    let last = living_targets.last().copied().unwrap_or(first);
    let damage = definition.values.damage.unwrap_or(0);

    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target: first,
                amount: damage,
            },
        },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target: last,
                amount: damage,
            },
        },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target: last,
                amount: damage,
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

fn sever_soul_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);
    for card in &state.piles.hand {
        let Some(card_definition) = get_card_definition(card.content_id) else {
            continue;
        };
        if card.id != card_id && card_definition.card_type != CardType::Attack {
            queue.push_back(InternalAction::MoveCard {
                card_id: card.id,
                from: CardPile::Hand,
                to: CardPile::ExhaustPile,
            });
        }
    }
    queue.extend([
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
    ]);
    Ok(queue)
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
            to: card_move_destination(definition),
        },
    ]))
}

fn immolate_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let mut queue = cleave_queue(card_id, definition)?;
    let move_card = queue
        .pop_back()
        .expect("cleave queue ends by moving the played card");
    queue.push_back(InternalAction::AddCardToPile {
        content_id: crate::content::cards::BURN_ID,
        to: CardPile::DiscardPile,
    });
    queue.push_back(move_card);
    Ok(queue)
}

fn thunderclap_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DealDamageAll {
            source: card_id,
            amount: definition.values.damage.unwrap_or(0),
        },
    ]);

    for monster in state.monsters.iter().filter(|monster| monster.alive) {
        queue.push_back(InternalAction::ApplyVulnerable {
            target: monster.id,
            amount: definition.values.vulnerable.unwrap_or(0),
        });
    }

    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: CardPile::DiscardPile,
    });

    Ok(queue)
}

fn uppercut_queue(
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
        InternalAction::ApplyVulnerable {
            target,
            amount: definition.values.vulnerable.unwrap_or(0),
        },
        InternalAction::ApplyWeak { target, amount: 1 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn whirlwind_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let x = state.player.energy;
    if x < 1 {
        return Err(SimError::IllegalAction(
            "Whirlwind requires at least 1 energy",
        ));
    }

    let damage = definition.values.damage.unwrap_or(0);
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: x },
    ]);

    for _ in 0..x {
        queue.push_back(InternalAction::DealDamageAll {
            source: card_id,
            amount: damage,
        });
    }

    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: CardPile::DiscardPile,
    });

    Ok(queue)
}

fn havoc_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
    target: Option<MonsterId>,
) -> SimResult<VecDeque<InternalAction>> {
    if state.piles.draw_pile.is_empty() {
        return Err(SimError::IllegalAction("Havoc requires a draw pile card"));
    }

    let top_definition = top_draw_card_definition(state)
        .ok_or(SimError::IllegalAction("Havoc requires a draw pile card"))?;
    validate_havoc_target(top_definition, target)?;

    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::PlayTopDrawCard { target },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn warcry_draw_count(definition: &CardDefinition) -> usize {
    if definition.id == WARCRY_PLUS_ID {
        2
    } else {
        1
    }
}

fn warcry_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    if lowest_other_hand_card(state, card_id).is_none() {
        return Err(SimError::IllegalAction(
            "Warcry requires another card in hand",
        ));
    }

    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DrawCards {
            count: warcry_draw_count(definition),
        },
        InternalAction::AwaitHandSelect {
            source_card_id: card_id,
        },
    ]))
}

fn lowest_attack_or_power_in_hand(state: &CombatState, exclude_id: CardId) -> Option<CardId> {
    state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != exclude_id)
        .filter(|card| {
            get_card_definition(card.content_id).is_some_and(|definition| {
                definition.card_type == CardType::Attack || definition.card_type == CardType::Power
            })
        })
        .min_by_key(|card| card.id.get())
        .map(|card| card.id)
}

fn dual_wield_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let copy_target = lowest_attack_or_power_in_hand(state, card_id).ok_or(
        SimError::IllegalAction("Dual Wield requires an attack or power"),
    )?;

    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::CopyHandCardToHand {
            card_id: copy_target,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        },
    ]))
}

#[must_use]
pub fn top_draw_card_definition(state: &CombatState) -> Option<&'static CardDefinition> {
    state
        .piles
        .draw_pile
        .first()
        .and_then(|card| get_card_definition(card.content_id))
}

fn validate_havoc_target(
    top_definition: &CardDefinition,
    target: Option<MonsterId>,
) -> SimResult<()> {
    match top_definition.target {
        TargetRequirement::Enemy if target.is_some() => Ok(()),
        TargetRequirement::Enemy => {
            Err(SimError::IllegalAction("Havoc top card requires a target"))
        }
        TargetRequirement::AllEnemies if target.is_none() => Ok(()),
        TargetRequirement::AllEnemies => Err(SimError::IllegalAction(
            "Havoc top card cannot have a target",
        )),
        TargetRequirement::None if target.is_none() => Ok(()),
        TargetRequirement::None => Err(SimError::IllegalAction(
            "Havoc top card cannot have a target",
        )),
    }
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

fn lowest_other_hand_card(state: &CombatState, exclude_id: CardId) -> Option<CardId> {
    state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != exclude_id)
        .min_by_key(|card| card.id.get())
        .map(|card| card.id)
}

fn true_grit_exhaust_target(state: &CombatState, true_grit_id: CardId) -> Option<CardId> {
    lowest_other_hand_card(state, true_grit_id)
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

fn burning_pact_queue(state: &CombatState, card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 1 },
    ]);

    if let Some(exhaust_target) = lowest_other_hand_card(state, card_id) {
        queue.push_back(InternalAction::MoveCard {
            card_id: exhaust_target,
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        });
    }

    queue.push_back(InternalAction::DrawCards { count: 2 });
    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: CardPile::DiscardPile,
    });

    Ok(queue)
}

fn feel_no_pain_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 1 },
        InternalAction::GainFeelNoPain { amount: 1 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn dark_embrace_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 1 },
        InternalAction::GainDarkEmbrace { amount: 1 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn demon_form_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainRitual { amount: 2 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn metallicize_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainMetallicize {
            amount: definition.values.block.unwrap_or(0),
        },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn pommel_strike_queue(
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
        InternalAction::DrawCards { count: 1 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn battle_trance_draw_count(definition: &CardDefinition) -> usize {
    if definition.id == BATTLE_TRANCE_PLUS_ID {
        3
    } else {
        2
    }
}

fn battle_trance_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DrawCards {
            count: battle_trance_draw_count(definition),
        },
        InternalAction::SetCannotDraw,
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn seeing_red_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainEnergy { amount: 2 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn inflame_strength_amount(definition: &CardDefinition) -> i32 {
    if definition.id == INFLAME_PLUS_ID {
        3
    } else {
        2
    }
}

fn inflame_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainStrength {
            amount: inflame_strength_amount(definition),
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn flex_temp_strength_amount(definition: &CardDefinition) -> i32 {
    if definition.id == FLEX_PLUS_ID {
        4
    } else {
        2
    }
}

fn flex_queue(card_id: CardId, definition: &CardDefinition) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainTempStrength {
            amount: flex_temp_strength_amount(definition),
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn spot_weakness_strength_amount(definition: &CardDefinition) -> i32 {
    if definition.id == SPOT_WEAKNESS_PLUS_ID {
        4
    } else {
        3
    }
}

fn any_monster_intends_attack(state: &CombatState) -> bool {
    state.monsters.iter().any(|monster| {
        monster.alive
            && matches!(
                monster.intent,
                MonsterIntent::Attack { .. } | MonsterIntent::AttackMultiple { .. }
            )
    })
}

fn spot_weakness_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);

    if any_monster_intends_attack(state) {
        queue.push_back(InternalAction::GainStrength {
            amount: spot_weakness_strength_amount(definition),
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
        let follow_ups = apply_internal_action(&mut next, internal_action)?;
        event_log.push(internal_action);
        for follow_up in follow_ups {
            queue.push_back(follow_up);
        }
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

fn apply_internal_action(
    state: &mut CombatState,
    action: InternalAction,
) -> SimResult<Vec<InternalAction>> {
    match action {
        InternalAction::ConsumeDuplicationPotion => {
            state.duplication_potion_pending = false;
            Ok(Vec::new())
        }
        InternalAction::PlayCard { card_id } => {
            let card = find_hand_card(state, card_id)?;
            let definition = get_card_definition(card.content_id)
                .ok_or(SimError::UnknownContent(card.content_id))?;
            apply_enrage_on_card_type(state, definition.card_type);
            Ok(crate::relic::apply_on_card_play_relics(
                state,
                definition.card_type,
            ))
        }
        InternalAction::SpendEnergy { amount } => {
            state.player.energy -= amount;
            Ok(Vec::new())
        }
        InternalAction::SpendCardEnergy { card_id } => {
            let cost = effective_hand_card_cost(state, card_id);
            state.player.energy -= cost;
            Ok(Vec::new())
        }
        InternalAction::DealDamage { info } => {
            let player_powers = state.player.powers;
            let temp_strength = state.player.temp_strength;
            let (spikes, still_alive) = {
                let monster = living_monster_mut(state, info.target)?;
                let spikes = monster.powers.spikes;
                let hp_damage =
                    deal_damage_info_to_monster(monster, info, player_powers, temp_strength);
                wake_lagavulin_on_damage(monster, hp_damage);
                guardian_on_hp_damage(monster, hp_damage);
                (spikes, monster.alive)
            };
            check_slime_boss_split(state, info.target);
            if still_alive && spikes > 0 {
                reflect_spikes_to_player(&mut state.player, spikes);
            }
            Ok(Vec::new())
        }
        InternalAction::DealDamageAll { source, amount } => {
            let player_powers = state.player.powers;
            let temp_strength = state.player.temp_strength;
            let targets: Vec<(MonsterId, i32)> = state
                .monsters
                .iter()
                .filter(|monster| monster.alive)
                .map(|monster| (monster.id, monster.powers.spikes))
                .collect();
            for (target, spikes) in targets {
                let still_alive = {
                    let monster = living_monster_mut(state, target)?;
                    let hp_damage = deal_damage_info_to_monster(
                        monster,
                        DamageInfo {
                            source: DamageSource::Card(source),
                            target,
                            amount,
                        },
                        player_powers,
                        temp_strength,
                    );
                    wake_lagavulin_on_damage(monster, hp_damage);
                    guardian_on_hp_damage(monster, hp_damage);
                    monster.alive
                };
                check_slime_boss_split(state, target);
                if still_alive && spikes > 0 {
                    reflect_spikes_to_player(&mut state.player, spikes);
                }
            }
            Ok(Vec::new())
        }
        InternalAction::GainBlock { amount } => {
            let gained = calculate_block(amount, state.player.powers);
            state.player.block += gained;
            Ok(Vec::new())
        }
        InternalAction::ApplyVulnerable { target, amount } => {
            if let Some(monster) = living_monster_mut_opt(state, target) {
                monster.powers.vulnerable += amount;
            }
            Ok(Vec::new())
        }
        InternalAction::ApplyWeak { target, amount } => {
            if let Some(monster) = living_monster_mut_opt(state, target) {
                monster.powers.weak += amount;
            }
            Ok(Vec::new())
        }
        InternalAction::MoveCard { card_id, from, to } => {
            move_card(state, card_id, from, to)?;
            if to == CardPile::ExhaustPile {
                Ok(vec![InternalAction::CardExhausted { card_id }])
            } else {
                Ok(Vec::new())
            }
        }
        InternalAction::RemoveCard { card_id, from } => {
            remove_card_from_pile(state, card_id, from)?;
            Ok(Vec::new())
        }
        InternalAction::AddCardToPile { content_id, to } => {
            add_card_to_pile(state, content_id, to);
            Ok(Vec::new())
        }
        InternalAction::DrawCards { count } => {
            player_draw_cards(state, count);
            Ok(Vec::new())
        }
        InternalAction::GainEnergy { amount } => {
            state.player.energy += amount;
            Ok(Vec::new())
        }
        InternalAction::SetCannotDraw => {
            state.player.cannot_draw = true;
            Ok(Vec::new())
        }
        InternalAction::GainFeelNoPain { amount } => {
            state.player.powers.feel_no_pain += amount;
            Ok(Vec::new())
        }
        InternalAction::GainDarkEmbrace { amount } => {
            state.player.powers.dark_embrace += amount;
            Ok(Vec::new())
        }
        InternalAction::GainMetallicize { amount } => {
            state.player.powers.metallicize += amount;
            Ok(Vec::new())
        }
        InternalAction::GainStrength { amount } => {
            state.player.powers.strength += amount;
            Ok(Vec::new())
        }
        InternalAction::GainTempStrength { amount } => {
            state.player.temp_strength += amount;
            Ok(Vec::new())
        }
        InternalAction::GainRitual { amount } => {
            state.player.powers.ritual += amount;
            Ok(Vec::new())
        }
        InternalAction::CardExhausted { .. } => {
            apply_on_exhaust_effects(state);
            Ok(Vec::new())
        }
        InternalAction::PlayTopDrawCard { target } => apply_play_top_draw_card(state, target),
        InternalAction::PutHandCardOnTopOfDraw { card_id } => {
            let card = remove_card_from_pile(state, card_id, CardPile::Hand)?;
            state.piles.draw_pile.insert(0, card);
            Ok(Vec::new())
        }
        InternalAction::CopyHandCardToHand { card_id } => {
            let card = find_hand_card(state, card_id)?;
            let next_id = CardId::new(state.piles.max_card_instance_id() + 1);
            state
                .piles
                .hand
                .push(CardInstance::new(next_id, card.content_id));
            Ok(Vec::new())
        }
        InternalAction::AwaitHandSelect { source_card_id } => {
            state.hand_select = Some(crate::combat::HandSelectState {
                source_card_id,
                selected_hand_index: None,
            });
            Ok(Vec::new())
        }
    }
}

pub(crate) fn apply_on_exhaust_effects(state: &mut CombatState) {
    if state.player.powers.feel_no_pain > 0 {
        state.player.block += 3 * state.player.powers.feel_no_pain;
    }
    if state.player.powers.dark_embrace > 0 {
        player_draw_cards(state, state.player.powers.dark_embrace as usize);
    }
}

pub(crate) fn player_draw_cards(state: &mut CombatState, count: usize) {
    if state.player.cannot_draw {
        return;
    }
    if let Some(mut rng) = state.shuffle_rng.take() {
        crate::combat::draw::draw_cards_with_sts_rng(state, count, &mut rng);
        state.shuffle_rng = Some(rng);
    } else {
        let mut rng = SimulatorRng::new(0);
        crate::combat::draw::draw_cards(state, count, &mut rng);
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
    living_monster_mut_opt(state, target)
        .ok_or(SimError::IllegalAction("target is not a living monster"))
}

fn living_monster_mut_opt(state: &mut CombatState, target: MonsterId) -> Option<&mut MonsterState> {
    state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == target && monster.alive)
}

fn apply_enrage_on_card_type(state: &mut CombatState, card_type: CardType) {
    if card_type != CardType::Skill {
        return;
    }

    for monster in &mut state.monsters {
        if !monster.alive {
            continue;
        }
        if let Some(monster_def) = get_monster_definition(monster.content_id) {
            if monster_def.enrage_weak_on_skill > 0 {
                monster.powers.anger += monster_def.enrage_weak_on_skill;
            }
        }
    }
}

fn apply_play_top_draw_card(
    state: &mut CombatState,
    target: Option<MonsterId>,
) -> SimResult<Vec<InternalAction>> {
    if state.piles.draw_pile.is_empty() {
        return Err(SimError::IllegalAction("draw pile is empty"));
    }

    let card = state
        .piles
        .draw_pile
        .pop()
        .ok_or(SimError::IllegalAction("draw pile is empty"))?;
    let card_id = card.id;
    let definition =
        get_card_definition(card.content_id).ok_or(SimError::UnknownContent(card.content_id))?;

    validate_havoc_target(definition, target)?;
    apply_enrage_on_card_type(state, definition.card_type);

    let mut follow_ups = Vec::new();

    match definition.id {
        STRIKE_R_ID
        | STRIKE_R_PLUS_ID
        | ANGER_ID
        | ANGER_PLUS_ID
        | POMMEL_STRIKE_ID
        | POMMEL_STRIKE_PLUS_ID
        | SEARING_BLOW_ID
        | SEARING_BLOW_PLUS_ID
        | BASH_ID => {
            let target = target.expect("validated havoc attack target");
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: definition.values.damage.unwrap_or(0),
                },
            });
        }
        TWIN_STRIKE_ID | TWIN_STRIKE_PLUS_ID => {
            let target = target.expect("validated havoc attack target");
            let damage = definition.values.damage.unwrap_or(0);
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: damage,
                },
            });
            follow_ups.push(InternalAction::DealDamage {
                info: DamageInfo {
                    source: DamageSource::Card(card_id),
                    target,
                    amount: damage,
                },
            });
        }
        CLEAVE_ID | CLEAVE_PLUS_ID | DRAMATIC_ENTRANCE_ID => {
            follow_ups.push(InternalAction::DealDamageAll {
                source: card_id,
                amount: definition.values.damage.unwrap_or(0),
            });
        }
        DEFEND_R_ID => {
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
        }
        SHRUG_IT_OFF_ID => {
            follow_ups.push(InternalAction::GainBlock { amount: 8 });
            follow_ups.push(InternalAction::DrawCards { count: 1 });
        }
        _ if definition.values.block.is_some() => {
            follow_ups.push(InternalAction::GainBlock {
                amount: definition.values.block.unwrap_or(0),
            });
        }
        _ => {}
    }

    state.piles.exhaust_pile.push(card);
    follow_ups.push(InternalAction::CardExhausted { card_id });

    Ok(follow_ups)
}

pub fn choose_hand_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    let hand_index = hand_select_ui_to_hand_index(state, ui_index)?;
    let hand_select = state
        .hand_select
        .as_mut()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    hand_select.selected_hand_index = Some(hand_index);
    Ok(())
}

pub fn hand_select_ui_to_hand_index(state: &CombatState, ui_index: usize) -> SimResult<usize> {
    let source_card_id = state
        .hand_select
        .as_ref()
        .ok_or(SimError::IllegalAction("no hand select is open"))?
        .source_card_id;
    let selectable: Vec<usize> = state
        .piles
        .hand
        .iter()
        .enumerate()
        .filter(|(_, card)| card.id != source_card_id)
        .map(|(index, _)| index)
        .collect();
    selectable
        .get(ui_index)
        .copied()
        .ok_or(SimError::IllegalAction("hand select index out of range"))
}

pub fn confirm_hand_select(state: &mut CombatState) -> SimResult<()> {
    let hand_select = state
        .hand_select
        .take()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    let index = hand_select
        .selected_hand_index
        .ok_or(SimError::IllegalAction("hand select choice is required"))?;
    let put_back = state.piles.hand[index].id;
    if put_back == hand_select.source_card_id {
        return Err(SimError::IllegalAction(
            "cannot put Warcry on top of the draw pile",
        ));
    }
    let card = remove_card_from_pile(state, put_back, CardPile::Hand)?;
    state.piles.draw_pile.push(card);
    let warcry = remove_card_from_pile(state, hand_select.source_card_id, CardPile::Hand)?;
    state.piles.exhaust_pile.push(warcry);
    apply_on_exhaust_effects(state);
    Ok(())
}

pub fn open_discard_select(state: &mut CombatState) -> SimResult<()> {
    if state.piles.discard_pile.is_empty() {
        return Err(SimError::IllegalAction("discard pile is empty"));
    }
    state.discard_select = Some(crate::combat::DiscardSelectState {
        selected_discard_index: None,
    });
    Ok(())
}

pub fn choose_discard_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    discard_select_ui_to_discard_index(state, ui_index)?;
    let discard_select = state
        .discard_select
        .as_mut()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    discard_select.selected_discard_index = Some(ui_index);
    Ok(())
}

pub fn discard_select_ui_to_discard_index(
    state: &CombatState,
    ui_index: usize,
) -> SimResult<usize> {
    state
        .discard_select
        .as_ref()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    if ui_index >= state.piles.discard_pile.len() {
        return Err(SimError::IllegalAction("discard select index out of range"));
    }
    Ok(ui_index)
}

pub fn confirm_liquid_memories_select(state: &mut CombatState) -> SimResult<()> {
    let discard_select = state
        .discard_select
        .take()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    let index = discard_select
        .selected_discard_index
        .ok_or(SimError::IllegalAction("discard select choice is required"))?;
    let mut card = state
        .piles
        .discard_pile
        .get(index)
        .copied()
        .ok_or(SimError::IllegalAction("discard select index out of range"))?;
    state.piles.discard_pile.remove(index);
    card.temp_cost = Some(0);
    state.piles.hand.push(card);
    Ok(())
}

pub fn open_exhaust_select(state: &mut CombatState) -> SimResult<()> {
    state.exhaust_select = Some(crate::combat::ExhaustSelectState {
        selected_hand_indices: Vec::new(),
    });
    Ok(())
}

pub fn choose_exhaust_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    exhaust_select_ui_to_hand_index(state, ui_index)?;
    let exhaust_select = state
        .exhaust_select
        .as_mut()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    if let Some(position) = exhaust_select
        .selected_hand_indices
        .iter()
        .position(|index| *index == ui_index)
    {
        exhaust_select.selected_hand_indices.remove(position);
    } else {
        exhaust_select.selected_hand_indices.push(ui_index);
        exhaust_select.selected_hand_indices.sort_unstable();
    }
    Ok(())
}

pub fn exhaust_select_ui_to_hand_index(state: &CombatState, ui_index: usize) -> SimResult<usize> {
    state
        .exhaust_select
        .as_ref()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    if ui_index >= state.piles.hand.len() {
        return Err(SimError::IllegalAction("exhaust select index out of range"));
    }
    Ok(ui_index)
}

pub fn confirm_exhaust_select(state: &mut CombatState) -> SimResult<()> {
    let exhaust_select = state
        .exhaust_select
        .take()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    let mut selected = exhaust_select.selected_hand_indices;
    selected.sort_unstable();
    selected.dedup();
    for index in selected.into_iter().rev() {
        if index >= state.piles.hand.len() {
            return Err(SimError::IllegalAction("exhaust select index out of range"));
        }
        let card = state.piles.hand.remove(index);
        state.piles.exhaust_pile.push(card);
        apply_on_exhaust_effects(state);
    }
    Ok(())
}

fn remove_card_from_pile(
    state: &mut CombatState,
    card_id: CardId,
    pile: CardPile,
) -> SimResult<CardInstance> {
    let cards = match pile {
        CardPile::Hand => &mut state.piles.hand,
        CardPile::DrawPile => &mut state.piles.draw_pile,
        CardPile::DiscardPile => &mut state.piles.discard_pile,
        CardPile::ExhaustPile => &mut state.piles.exhaust_pile,
    };
    let index = cards
        .iter()
        .position(|card| card.id == card_id)
        .ok_or(SimError::UnknownCard(card_id))?;
    Ok(cards.remove(index))
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

fn effective_hand_card_cost(state: &CombatState, card_id: CardId) -> i32 {
    let card = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .expect("hand card");
    if let Some(cost) = card.temp_cost {
        return i32::from(cost);
    }
    get_card_definition(card.content_id)
        .map(|definition| i32::from(definition.cost))
        .unwrap_or(0)
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
            if !card.combat_only {
                state.piles.discard_pile.push(card);
            }
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
        ANGER_ID, ANGER_PLUS_ID, BASH_ID, BATTLE_TRANCE_ID, BATTLE_TRANCE_PLUS_ID, BURNING_PACT_ID,
        CLEAVE_ID, CLEAVE_PLUS_ID, DARK_EMBRACE_ID, DEFEND_R_ID, DUAL_WIELD_ID, FEEL_NO_PAIN_ID,
        FLEX_ID, FLEX_PLUS_ID, HAVOC_ID, INFLAME_ID, INFLAME_PLUS_ID, POMMEL_STRIKE_ID,
        POMMEL_STRIKE_PLUS_ID, SEARING_BLOW_ID, SEEING_RED_ID, SEEING_RED_PLUS_ID, SEVER_SOUL_ID,
        SHRUG_IT_OFF_ID, SLIMED_ID, SPOT_WEAKNESS_ID, SPOT_WEAKNESS_PLUS_ID, STRIKE_R_ID,
        TRUE_GRIT_ID, TWIN_STRIKE_ID, TWIN_STRIKE_PLUS_ID, WHIRLWIND_ID, WHIRLWIND_PLUS_ID,
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
    fn duplication_potion_pending_doubles_next_attack_without_extra_energy() {
        let mut state = CombatState::initial_fixture();
        state.duplication_potion_pending = true;

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 12);
        assert_eq!(next.player.energy, state.player.energy - 1);
        assert!(!next.duplication_potion_pending);
    }

    #[test]
    fn duplication_potion_pending_doubles_next_skill_block_once() {
        let mut state = CombatState::initial_fixture();
        state.duplication_potion_pending = true;

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(next.player.block, state.player.block + 10);
        assert_eq!(next.player.energy, state.player.energy - 1);
        assert!(!next.duplication_potion_pending);
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
    fn lethal_bash_does_not_fail_when_vulnerable_target_dies() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].hp = 2;

        let next = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");

        assert!(next.monsters[0].hp <= 0);
        assert!(!next.monsters[0].alive);
        assert_eq!(next.phase, CombatPhase::Won);
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
    fn dramatic_entrance_deals_eight_to_all_enemies_and_exhausts() {
        use crate::content::cards::DRAMATIC_ENTRANCE_ID;

        let state = two_monster_hand(DRAMATIC_ENTRANCE_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Dramatic Entrance applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 8);
        assert_eq!(next.monsters[1].hp, state.monsters[1].hp - 8);
        assert_eq!(next.player.energy, state.player.energy);
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, DRAMATIC_ENTRANCE_ID);
        assert!(next.piles.hand.is_empty());
    }

    #[test]
    fn whirlwind_at_three_energy_hits_all_enemies_three_times() {
        let state = two_monster_hand(WHIRLWIND_ID);
        assert_eq!(state.player.energy, 3);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Whirlwind applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 15);
        assert_eq!(next.monsters[1].hp, state.monsters[1].hp - 15);
        assert_eq!(next.player.energy, 0);
    }

    #[test]
    fn whirlwind_plus_at_three_energy_hits_all_enemies_three_times() {
        let state = two_monster_hand(WHIRLWIND_PLUS_ID);
        assert_eq!(state.player.energy, 3);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Whirlwind+ applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 24);
        assert_eq!(next.monsters[1].hp, state.monsters[1].hp - 24);
        assert_eq!(next.player.energy, 0);
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

    #[test]
    fn burning_pact_exhausts_lowest_other_hand_card_and_draws_two() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(25), BURNING_PACT_ID),
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(40), STRIKE_R_ID),
            CardInstance::new(CardId::new(41), DEFEND_R_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Burning Pact applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
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
            .any(|card| card.id == CardId::new(40)));
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(41)));
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(30)));
    }

    #[test]
    fn burning_pact_with_only_itself_in_hand_draws_two_without_exhaust() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(25), BURNING_PACT_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(40), STRIKE_R_ID),
            CardInstance::new(CardId::new(41), DEFEND_R_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Burning Pact applies");

        assert!(next.piles.exhaust_pile.is_empty());
        assert_eq!(next.piles.hand.len(), 2);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(25)));
    }

    #[test]
    fn feel_no_pain_grants_power_and_moves_to_discard() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), FEEL_NO_PAIN_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Feel No Pain applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.player.powers.feel_no_pain, 1);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn feel_no_pain_grants_block_when_card_exhausted() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.feel_no_pain = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(25), BURNING_PACT_ID),
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
        ];
        state.piles.draw_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Burning Pact applies");

        assert_eq!(next.player.block, 3);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn dark_embrace_grants_power_and_moves_to_discard() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DARK_EMBRACE_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Dark Embrace applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.player.powers.dark_embrace, 1);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn dark_embrace_draws_when_card_exhausted() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.dark_embrace = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(25), BURNING_PACT_ID),
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(40), STRIKE_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Burning Pact applies");

        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(40)));
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn card_exhausted_event_log_records_on_exhaust_hook() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(25), TRUE_GRIT_ID),
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
        ];

        let transition = apply_combat_action_with_events(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("True Grit applies");

        assert!(transition
            .event_log
            .contains(&InternalAction::CardExhausted {
                card_id: CardId::new(20),
            }));
    }

    #[test]
    fn pommel_strike_deals_nine_damage_draws_one_and_moves_to_discard() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), POMMEL_STRIKE_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Pommel Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 9);
        assert_eq!(next.player.energy, state.player.energy - 1);
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
    fn pommel_strike_plus_deals_twelve_damage() {
        let state = hand_only(POMMEL_STRIKE_PLUS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Pommel Strike+ applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 12);
    }

    #[test]
    fn battle_trance_draws_two_sets_cannot_draw_and_moves_to_discard() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), BATTLE_TRANCE_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Battle Trance applies");

        assert_eq!(next.player.energy, state.player.energy);
        assert!(next.player.cannot_draw);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(30)));
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(31)));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn battle_trance_plus_draws_three_cards() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), BATTLE_TRANCE_PLUS_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
            CardInstance::new(CardId::new(32), BASH_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Battle Trance+ applies");

        assert_eq!(next.piles.hand.len(), 3);
        assert!(next.player.cannot_draw);
    }

    #[test]
    fn seeing_red_spends_one_and_gains_two_energy() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), SEEING_RED_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Seeing Red applies");

        assert_eq!(next.player.energy, state.player.energy + 1);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn seeing_red_plus_gains_two_energy_without_spending() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), SEEING_RED_PLUS_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Seeing Red+ applies");

        assert_eq!(next.player.energy, 3);
    }

    #[test]
    fn battle_trance_blocks_later_draw_from_shrug_it_off() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), BATTLE_TRANCE_ID),
            CardInstance::new(CardId::new(25), SHRUG_IT_OFF_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(32), BASH_ID),
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), DEFEND_R_ID),
        ];

        let after_trance = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Battle Trance applies");
        let after_shrug = apply_combat_action(
            &after_trance,
            CombatAction::PlayCard {
                card_id: CardId::new(25),
                target: None,
            },
        )
        .expect("Shrug It Off applies");

        assert_eq!(after_shrug.player.block, 8);
        assert_eq!(after_shrug.piles.hand.len(), 2);
        assert!(!after_shrug
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(32)));
    }

    #[test]
    fn inflame_grants_two_strength_and_moves_to_discard() {
        let state = hand_only(INFLAME_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Inflame applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.player.powers.strength, 2);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
    }

    #[test]
    fn inflame_plus_grants_three_strength() {
        let state = hand_only(INFLAME_PLUS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Inflame+ applies");

        assert_eq!(next.player.powers.strength, 3);
    }

    #[test]
    fn flex_grants_two_temp_strength_at_zero_cost() {
        let state = hand_only(FLEX_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Flex applies");

        assert_eq!(next.player.energy, state.player.energy);
        assert_eq!(next.player.temp_strength, 2);
        assert_eq!(next.player.powers.strength, 0);
    }

    #[test]
    fn flex_plus_grants_four_temp_strength() {
        let state = hand_only(FLEX_PLUS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Flex+ applies");

        assert_eq!(next.player.temp_strength, 4);
    }

    #[test]
    fn flex_temp_strength_boosts_strike_damage() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), FLEX_ID),
            CardInstance::new(CardId::new(21), STRIKE_R_ID),
        ];

        let after_flex = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Flex applies");

        let after_strike = apply_combat_action(
            &after_flex,
            CombatAction::PlayCard {
                card_id: CardId::new(21),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Strike applies");

        assert_eq!(after_strike.monsters[0].hp, state.monsters[0].hp - 8);
    }

    #[test]
    fn flex_temp_strength_clears_after_end_turn() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), FLEX_ID)];
        state.piles.draw_pile.clear();

        let after_flex = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Flex applies");
        assert_eq!(after_flex.player.temp_strength, 2);

        let next_turn = crate::combat::end_player_turn(&after_flex);

        assert_eq!(next_turn.player.temp_strength, 0);
    }

    #[test]
    fn spot_weakness_grants_three_strength_when_enemy_intends_attack() {
        let state = hand_only(SPOT_WEAKNESS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Spot Weakness applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
        assert_eq!(next.player.powers.strength, 3);
        assert_eq!(next.monsters[0].powers.weak, 0);
    }

    #[test]
    fn spot_weakness_does_nothing_on_ritual_intent() {
        let mut state = CombatState::cultist_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), SPOT_WEAKNESS_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Spot Weakness applies");

        assert_eq!(next.player.powers.strength, 0);
    }

    #[test]
    fn spot_weakness_plus_grants_four_strength() {
        let state = hand_only(SPOT_WEAKNESS_PLUS_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Spot Weakness+ applies");

        assert_eq!(next.player.powers.strength, 4);
    }

    #[test]
    fn spot_weakness_boosts_follow_up_attack_damage() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), SPOT_WEAKNESS_ID),
            CardInstance::new(CardId::new(21), STRIKE_R_ID),
        ];
        state.piles.draw_pile.clear();

        let after_spot = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Spot Weakness applies");
        assert_eq!(after_spot.player.powers.strength, 3);

        let after_strike = apply_combat_action(
            &after_spot,
            CombatAction::PlayCard {
                card_id: CardId::new(21),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Strike applies");

        assert_eq!(after_strike.monsters[0].hp, state.monsters[0].hp - 9);
    }

    #[test]
    fn gremlin_nob_enrage_applies_two_anger_on_skill_play() {
        let state = CombatState::gremlin_nob_fixture();

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(next.monsters[0].powers.anger, 2);
    }

    #[test]
    fn gremlin_nob_enrage_does_not_trigger_on_strike() {
        let state = CombatState::gremlin_nob_fixture();

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.monsters[0].powers.anger, 0);
    }

    #[test]
    fn gremlin_nob_enrage_stacks_on_multiple_skills() {
        let mut state = CombatState::gremlin_nob_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
        ];

        let after_first = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("first Defend applies");
        let after_second = apply_combat_action(
            &after_first,
            CombatAction::PlayCard {
                card_id: CardId::new(21),
                target: None,
            },
        )
        .expect("second Defend applies");

        assert_eq!(after_second.monsters[0].powers.anger, 4);
    }

    #[test]
    fn havoc_plays_top_strike_and_exhausts_it() {
        let mut state = hand_only(HAVOC_ID);
        state.piles.hand.clear();
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(20), HAVOC_ID));
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Havoc applies");

        assert_eq!(next.monsters[0].hp, 34);
        assert!(next.piles.draw_pile.is_empty());
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, STRIKE_R_ID);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == HAVOC_ID));
    }

    #[test]
    fn warcry_draws_puts_card_on_draw_pile_and_exhausts() {
        let mut state = hand_only(WARCRY_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), WARCRY_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let mut after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Warcry opens hand select");

        assert!(after_play.hand_select.is_some());
        choose_hand_select(&mut after_play, 0).expect("choose defend");
        confirm_hand_select(&mut after_play).expect("confirm hand select");

        assert_eq!(after_play.piles.draw_pile[0].content_id, DEFEND_R_ID);
        assert_eq!(after_play.piles.draw_pile.len(), 1);
        assert_eq!(after_play.piles.hand.len(), 1);
        assert_eq!(after_play.piles.hand[0].content_id, STRIKE_R_ID);
        assert_eq!(after_play.piles.exhaust_pile[0].content_id, WARCRY_ID);
    }

    #[test]
    fn warcry_hand_select_puts_regret_on_draw_pile_top() {
        use crate::content::cards::REGRET_ID;

        let mut state = hand_only(WARCRY_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
            CardInstance::new(CardId::new(4), DEFEND_R_ID),
            CardInstance::new(CardId::new(5), WARCRY_ID),
            CardInstance::new(CardId::new(6), REGRET_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(30), STRIKE_R_ID)];

        let mut after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(5),
                target: None,
            },
        )
        .expect("Warcry opens hand select");

        choose_hand_select(&mut after_play, 4).expect("choose regret");
        confirm_hand_select(&mut after_play).expect("confirm hand select");

        assert_eq!(
            after_play.piles.draw_pile.last().unwrap().content_id,
            REGRET_ID
        );
        assert_eq!(after_play.piles.exhaust_pile[0].content_id, WARCRY_ID);
        assert!(after_play.hand_select.is_none());
    }

    #[test]
    fn warcry_plus_draws_two_cards() {
        let mut state = hand_only(WARCRY_PLUS_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), WARCRY_PLUS_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(30), STRIKE_R_ID),
            CardInstance::new(CardId::new(31), STRIKE_R_ID),
        ];

        let mut after_play = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Warcry+ opens hand select");

        choose_hand_select(&mut after_play, 0).expect("choose defend");
        confirm_hand_select(&mut after_play).expect("confirm hand select");

        assert_eq!(after_play.piles.hand.len(), 2);
        assert_eq!(after_play.piles.draw_pile[0].content_id, DEFEND_R_ID);
    }

    #[test]
    fn dual_wield_copies_attack_to_hand_and_exhausts() {
        let mut state = hand_only(DUAL_WIELD_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), DUAL_WIELD_ID),
            CardInstance::new(CardId::new(21), ANGER_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Dual Wield applies");

        assert_eq!(
            next.piles
                .hand
                .iter()
                .filter(|card| card.content_id == ANGER_ID)
                .count(),
            2
        );
        assert_eq!(next.piles.exhaust_pile[0].content_id, DUAL_WIELD_ID);
    }

    #[test]
    fn searing_blow_deals_twelve_damage() {
        let state = hand_only(SEARING_BLOW_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Searing Blow applies");

        assert_eq!(next.monsters[0].hp, 28);
    }

    #[test]
    fn sever_soul_deals_sixteen_damage() {
        let state = hand_only(SEVER_SOUL_ID);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Sever Soul applies");

        assert_eq!(next.monsters[0].hp, 24);
        assert_eq!(next.player.energy, 1);
        assert_eq!(next.piles.discard_pile[0].content_id, SEVER_SOUL_ID);
    }

    #[test]
    fn sever_soul_exhausts_non_attack_cards_in_hand() {
        let mut state = hand_only(SEVER_SOUL_ID);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), SEVER_SOUL_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
            CardInstance::new(CardId::new(22), ANGER_ID),
            CardInstance::new(CardId::new(23), BATTLE_TRANCE_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Sever Soul applies");

        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![DEFEND_R_ID, BATTLE_TRANCE_ID]
        );
        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![ANGER_ID]
        );
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
