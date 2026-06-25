use crate::{
    action::{CardPile, HpLossSource, InternalAction},
    card::{CardDefinition, CardType, TargetRequirement},
    combat::{
        damage::{DamageInfo, DamageSource},
        CombatState, HandSelectPurpose,
    },
    content::cards::{
        get_card_definition, is_curse_content_id, upgrade_content_id, ANGER_ID, ANGER_PLUS_ID,
        ARMAMENTS_ID, BANDAGE_UP_ID, BARRICADE_ID, BASH_ID, BATTLE_TRANCE_ID,
        BATTLE_TRANCE_PLUS_ID, BERSERK_ID, BLIND_ID, BLOODLETTING_ID, BLOOD_FOR_BLOOD_ID,
        BODY_SLAM_ID, BRUTALITY_ID, BURNING_PACT_ID, CLASH_ID, CLEAVE_ID, CLEAVE_PLUS_ID,
        CLOTHESLINE_ID, COMBUST_ID, CORRUPTION_ID, DARK_EMBRACE_ID, DAZED_ID, DEFEND_R_ID,
        DEMON_FORM_ID, DISARM_ID, DOUBLE_TAP_ID, DRAMATIC_ENTRANCE_ID, DROPKICK_ID, DUAL_WIELD_ID,
        DUAL_WIELD_PLUS_ID, ENTRENCH_ID, EVOLVE_ID, EXHUME_ID, FEED_ID, FEEL_NO_PAIN_ID,
        FIEND_FIRE_ID, FINESSE_ID, FIRE_BREATHING_ID, FLAME_BARRIER_ID, FLASH_OF_STEEL_ID, FLEX_ID,
        FLEX_PLUS_ID, HAVOC_ID, HAVOC_PLUS_ID, HEADBUTT_ID, HEAVY_BLADE_ID, HEMOKINESIS_ID,
        IMMOLATE_ID, INFERNAL_BLADE_ID, INFLAME_ID, INFLAME_PLUS_ID, INTIMIDATE_ID, IRON_WAVE_ID,
        JUGGERNAUT_ID, LIMIT_BREAK_ID, METALLICIZE_ID, OFFERING_ID, PANACEA_ID,
        PERFECTED_STRIKE_ID, POMMEL_STRIKE_ID, POMMEL_STRIKE_PLUS_ID, POWER_THROUGH_ID, PUMMEL_ID,
        RAGE_ID, RAMPAGE_ID, REAPER_ID, RECKLESS_CHARGE_ID, RUPTURE_ID, SEARING_BLOW_ID,
        SEARING_BLOW_PLUS_ID, SECOND_WIND_ID, SEEING_RED_ID, SEEING_RED_PLUS_ID, SEVER_SOUL_ID,
        SHOCKWAVE_ID, SHRUG_IT_OFF_ID, SLIMED_ID, SPOT_WEAKNESS_ID, SPOT_WEAKNESS_PLUS_ID,
        STRIKE_R_ID, STRIKE_R_PLUS_ID, SWIFT_STRIKE_ID, SWORD_BOOMERANG_ID, THUNDERCLAP_ID,
        TRIP_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, TWIN_STRIKE_PLUS_ID, UPPERCUT_ID, WARCRY_ID,
        WARCRY_PLUS_ID, WHIRLWIND_ID, WHIRLWIND_PLUS_ID, WILD_STRIKE_ID, WOUND_ID,
    },
    content::shop_pool::ironclad_combat_attack_discovery_pool,
    ids::{CardId, ContentId, MonsterId},
    relic::{
        strike_damage_with_relics, Relic, AKABEKO_DAMAGE, CHEMICAL_X_BONUS_X, PEN_NIB_THRESHOLD,
    },
    MonsterIntent, SimError, SimResult,
};
use std::collections::VecDeque;

pub(super) fn play_card_queue(
    state: &CombatState,
    card_id: CardId,
    target: Option<MonsterId>,
) -> SimResult<(CombatState, VecDeque<InternalAction>)> {
    let card = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .ok_or(SimError::IllegalAction("card is not in hand"))?;
    let definition =
        get_card_definition(card.content_id).ok_or(SimError::UnknownContent(card.content_id))?;

    let mut queued_state = state.clone();
    let queue = match definition.id {
        _ if definition.keywords.unplayable => {
            unplayable_relic_queue(&state.relics, card_id, card.content_id, definition)
        }
        STRIKE_R_ID | STRIKE_R_PLUS_ID => strike_queue(
            state,
            card_id,
            target.expect("validated Strike has a target"),
            definition,
        ),
        DEFEND_R_ID => defend_queue(card_id, definition),
        BASH_ID => bash_queue(card_id, target.expect("validated Bash has a target")),
        SLIMED_ID => slimed_queue(card_id, target.expect("validated Slimed has a target")),
        ANGER_ID | ANGER_PLUS_ID => anger_queue(
            card_id,
            target.expect("validated Anger has a target"),
            definition,
        ),
        IRON_WAVE_ID => iron_wave_queue(
            card_id,
            target.expect("validated Iron Wave has a target"),
            definition,
        ),
        BODY_SLAM_ID => body_slam_queue(
            state,
            card_id,
            target.expect("validated Body Slam has a target"),
            definition,
        ),
        CLASH_ID | SWIFT_STRIKE_ID => generic_attack_queue(
            card_id,
            target.expect("validated generic attack has a target"),
            definition,
        ),
        WILD_STRIKE_ID => wild_strike_queue(
            card_id,
            target.expect("validated Wild Strike has a target"),
            definition,
        ),
        HEAVY_BLADE_ID => heavy_blade_queue(
            state,
            card_id,
            target.expect("validated Heavy Blade has a target"),
            definition,
        ),
        PERFECTED_STRIKE_ID => perfected_strike_queue(
            state,
            card_id,
            target.expect("validated Perfected Strike has a target"),
            definition,
        ),
        RAMPAGE_ID => rampage_queue(
            state,
            card_id,
            target.expect("validated Rampage has a target"),
            definition,
        ),
        POWER_THROUGH_ID => power_through_queue(card_id, definition),
        ARMAMENTS_ID => armaments_queue(state, card_id, definition),
        HEADBUTT_ID => headbutt_queue(
            state,
            card_id,
            target.expect("validated Headbutt has a target"),
            definition,
        ),
        FLAME_BARRIER_ID => flame_barrier_queue(card_id, definition),
        ENTRENCH_ID => entrench_queue(card_id),
        RECKLESS_CHARGE_ID => reckless_charge_queue(
            card_id,
            target.expect("validated Reckless Charge has a target"),
            definition,
        ),
        PUMMEL_ID => pummel_queue(
            card_id,
            target.expect("validated Pummel has a target"),
            definition,
        ),
        CLOTHESLINE_ID => clothesline_queue(
            card_id,
            target.expect("validated Clothesline has a target"),
            definition,
        ),
        FIEND_FIRE_ID => fiend_fire_queue(
            state,
            card_id,
            target.expect("validated Fiend Fire has a target"),
            definition,
        ),
        FEED_ID => feed_queue(
            card_id,
            target.expect("validated Feed has a target"),
            definition,
        ),
        REAPER_ID => reaper_queue(card_id, definition),
        CLEAVE_ID | CLEAVE_PLUS_ID | DRAMATIC_ENTRANCE_ID => cleave_queue(card_id, definition),
        IMMOLATE_ID => immolate_queue(card_id, definition),
        TWIN_STRIKE_ID | TWIN_STRIKE_PLUS_ID => twin_strike_queue(
            card_id,
            target.expect("validated Twin Strike has a target"),
            definition,
        ),
        FINESSE_ID => finesse_queue(card_id, definition),
        SHRUG_IT_OFF_ID => shrug_it_off_queue(card_id),
        TRUE_GRIT_ID => true_grit_queue(state, card_id),
        BURNING_PACT_ID => burning_pact_queue(state, card_id),
        INFERNAL_BLADE_ID => infernal_blade_queue(&mut queued_state, card_id, definition),
        BANDAGE_UP_ID => bandage_up_queue(card_id, definition),
        PANACEA_ID => panacea_queue(card_id, definition),
        FEEL_NO_PAIN_ID => feel_no_pain_queue(card_id),
        DARK_EMBRACE_ID => dark_embrace_queue(card_id),
        COMBUST_ID => combust_queue(card_id),
        CORRUPTION_ID => corruption_queue(card_id),
        BARRICADE_ID => barricade_queue(card_id),
        EVOLVE_ID => evolve_queue(card_id),
        BERSERK_ID => berserk_queue(card_id, definition),
        RUPTURE_ID => rupture_queue(card_id),
        JUGGERNAUT_ID => juggernaut_queue(card_id, definition),
        BRUTALITY_ID => brutality_queue(card_id),
        FIRE_BREATHING_ID => fire_breathing_queue(card_id, definition),
        EXHUME_ID => exhume_queue(state, card_id),
        DEMON_FORM_ID => demon_form_queue(card_id),
        METALLICIZE_ID => metallicize_queue(card_id, definition),
        POMMEL_STRIKE_ID | POMMEL_STRIKE_PLUS_ID | FLASH_OF_STEEL_ID => pommel_strike_queue(
            card_id,
            target.expect("validated draw attack has a target"),
            definition,
        ),
        BATTLE_TRANCE_ID | BATTLE_TRANCE_PLUS_ID => battle_trance_queue(card_id, definition),
        DOUBLE_TAP_ID => double_tap_queue(card_id, definition),
        SEEING_RED_ID | SEEING_RED_PLUS_ID => seeing_red_queue(card_id, definition),
        BLOODLETTING_ID => bloodletting_queue(card_id, definition),
        HEMOKINESIS_ID => hemokinesis_queue(
            card_id,
            target.expect("validated Hemokinesis has a target"),
            definition,
        ),
        BLOOD_FOR_BLOOD_ID => blood_for_blood_queue(
            card_id,
            target.expect("validated Blood for Blood has a target"),
            definition,
        ),
        DROPKICK_ID => dropkick_queue(
            state,
            card_id,
            target.expect("validated Dropkick has a target"),
            definition,
        ),
        BLIND_ID => blind_queue(state, card_id, definition),
        TRIP_ID => trip_queue(state, card_id, definition),
        INTIMIDATE_ID => intimidate_queue(state, card_id, definition),
        SHOCKWAVE_ID => shockwave_queue(state, card_id, definition),
        DISARM_ID => disarm_queue(
            card_id,
            target.expect("validated Disarm has a target"),
            definition,
        ),
        RAGE_ID => rage_queue(card_id, definition),
        INFLAME_ID | INFLAME_PLUS_ID => inflame_queue(card_id, definition),
        FLEX_ID | FLEX_PLUS_ID => flex_queue(card_id, definition),
        LIMIT_BREAK_ID => limit_break_queue(state, card_id, definition),
        OFFERING_ID => offering_queue(card_id, definition),
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
        SECOND_WIND_ID => second_wind_queue(state, card_id, definition),
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
    apply_akabeko_to_first_attack_queue(state, definition.card_type, card_id, &mut queue);
    apply_pen_nib_to_tenth_attack_queue(state, definition.card_type, card_id, &mut queue);
    if state.duplication_potion_pending {
        queue = apply_duplication_potion_to_queue(queue, card_id);
    }
    if definition.card_type == CardType::Attack && state.double_tap_pending > 0 {
        queue = apply_double_tap_to_queue(queue, card_id, state.double_tap_pending);
    }

    apply_corruption_to_played_skill_queue(state, definition, card_id, &mut queue);
    apply_strange_spoon_to_played_card_move(&mut queued_state, definition, card_id, &mut queue);

    Ok((queued_state, queue))
}

fn apply_corruption_to_played_skill_queue(
    state: &CombatState,
    definition: &CardDefinition,
    card_id: CardId,
    queue: &mut VecDeque<InternalAction>,
) {
    if definition.card_type != CardType::Skill || state.player.powers.corruption <= 0 {
        return;
    }

    for action in queue.iter_mut() {
        if let InternalAction::SpendEnergy { amount } = action {
            *amount = 0;
            break;
        }
    }

    if let Some(InternalAction::MoveCard { to, .. }) = queue.iter_mut().rfind(|action| {
        matches!(
            action,
            InternalAction::MoveCard {
                card_id: moved,
                from: CardPile::Hand,
                ..
            } if *moved == card_id
        )
    }) {
        *to = CardPile::ExhaustPile;
    }
}

fn apply_strange_spoon_to_played_card_move(
    state: &mut CombatState,
    definition: &CardDefinition,
    card_id: CardId,
    queue: &mut VecDeque<InternalAction>,
) {
    if definition.card_type == CardType::Power || !state.relics.contains(&Relic::StrangeSpoon) {
        return;
    }

    let own_exhaust_index = queue.iter().rposition(|action| {
        matches!(
            action,
            InternalAction::MoveCard {
                card_id: moved,
                from: CardPile::Hand,
                to: CardPile::ExhaustPile,
            } if *moved == card_id
        )
    });
    let Some(index) = own_exhaust_index else {
        return;
    };

    let Some(rng) = state.card_random_rng.as_mut() else {
        return;
    };
    if !rng.random_bool() {
        return;
    }

    queue[index] = InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: CardPile::DiscardPile,
    };
}

fn apply_akabeko_to_first_attack_queue(
    state: &CombatState,
    card_type: CardType,
    card_id: CardId,
    queue: &mut VecDeque<InternalAction>,
) {
    if card_type != CardType::Attack
        || !state.relics.contains(&Relic::Akabeko)
        || state.relic_counters.attacks_played_this_combat > 0
    {
        return;
    }

    for action in queue {
        match action {
            InternalAction::DealDamage {
                info:
                    DamageInfo {
                        source: DamageSource::Card(source),
                        amount,
                        ..
                    },
            } if *source == card_id => {
                *amount += AKABEKO_DAMAGE;
            }
            InternalAction::DealDamageAll { source, amount } if *source == card_id => {
                *amount += AKABEKO_DAMAGE;
            }
            InternalAction::DealDamageAllAndHealUnblocked { source, amount }
                if *source == card_id =>
            {
                *amount += AKABEKO_DAMAGE;
            }
            InternalAction::DealFeedDamage {
                info:
                    DamageInfo {
                        source: DamageSource::Card(source),
                        amount,
                        ..
                    },
                ..
            } if *source == card_id => {
                *amount += AKABEKO_DAMAGE;
            }
            _ => {}
        }
    }
}

fn apply_pen_nib_to_tenth_attack_queue(
    state: &CombatState,
    card_type: CardType,
    card_id: CardId,
    queue: &mut VecDeque<InternalAction>,
) {
    if card_type != CardType::Attack
        || !state.relics.contains(&Relic::PenNib)
        || state.relic_counters.pen_nib_attacks_played + 1 != PEN_NIB_THRESHOLD
    {
        return;
    }

    for action in queue {
        match action {
            InternalAction::DealDamage {
                info:
                    DamageInfo {
                        source: DamageSource::Card(source),
                        amount,
                        ..
                    },
            } if *source == card_id => {
                *amount *= 2;
            }
            InternalAction::DealDamageAll { source, amount } if *source == card_id => {
                *amount *= 2;
            }
            InternalAction::DealDamageAllAndHealUnblocked { source, amount }
                if *source == card_id =>
            {
                *amount *= 2;
            }
            InternalAction::DealFeedDamage {
                info:
                    DamageInfo {
                        source: DamageSource::Card(source),
                        amount,
                        ..
                    },
                ..
            } if *source == card_id => {
                *amount *= 2;
            }
            _ => {}
        }
    }
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

fn apply_double_tap_to_queue(
    mut queue: VecDeque<InternalAction>,
    card_id: CardId,
    count: i32,
) -> VecDeque<InternalAction> {
    let mut duplicated_effects = VecDeque::new();
    for _ in 0..count {
        duplicated_effects.extend(
            queue
                .iter()
                .copied()
                .filter(|action| is_duplicated_card_effect(*action, card_id)),
        );
    }

    let final_move = queue
        .back()
        .copied()
        .filter(|action| is_card_move_for(*action, card_id));
    if final_move.is_some() {
        queue.pop_back();
    }

    queue.push_front(InternalAction::ConsumeDoubleTap);
    queue.append(&mut duplicated_effects);
    if let Some(action) = final_move {
        queue.push_back(action);
    }

    queue
}

fn unplayable_relic_queue(
    relics: &[Relic],
    card_id: CardId,
    content_id: ContentId,
    definition: &'static CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    if !crate::relic::can_play_unplayable_card_with_relics(relics, definition.card_type, content_id)
    {
        return Err(SimError::IllegalAction("card is unplayable"));
    }

    let mut queue = VecDeque::from([InternalAction::PlayCard { card_id }]);
    if is_curse_content_id(content_id) {
        queue.push_back(InternalAction::LoseHp {
            amount: crate::relic::BLUE_CANDLE_HP_LOSS,
            source: HpLossSource::Other,
        });
    }
    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: CardPile::ExhaustPile,
    });
    Ok(queue)
}

fn is_duplicated_card_effect(action: InternalAction, card_id: CardId) -> bool {
    !matches!(
        action,
        InternalAction::ConsumeDuplicationPotion
            | InternalAction::ConsumeDoubleTap
            | InternalAction::PlayCard { .. }
            | InternalAction::SpendEnergy { .. }
            | InternalAction::SpendCardEnergy { .. }
            | InternalAction::MoveCard { .. }
            | InternalAction::AwaitHandSelect { .. }
            | InternalAction::AwaitDiscardSelect { .. }
            | InternalAction::AwaitExhaustSelect { .. }
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

fn strike_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 1 },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: strike_damage_with_relics(
                    &state.relics,
                    definition.values.damage.unwrap_or(0),
                ),
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

fn iron_wave_queue(
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

fn body_slam_queue(
    state: &CombatState,
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
                amount: state.player.block,
            },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn armaments_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainBlock {
            amount: definition.values.block.unwrap_or(0),
        },
    ]);

    if has_upgradeable_other_hand_card(state, card_id) {
        queue.push_back(InternalAction::AwaitHandSelect {
            source_card_id: card_id,
            purpose: HandSelectPurpose::ArmamentsUpgrade,
        });
    } else {
        queue.push_back(InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        });
    }

    Ok(queue)
}

fn headbutt_queue(
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
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: definition.values.damage.unwrap_or(0),
            },
        },
    ]);

    if state.piles.discard_pile.is_empty() {
        queue.push_back(InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        });
    } else {
        queue.push_back(InternalAction::AwaitDiscardSelect {
            source_card_id: card_id,
            purpose: crate::combat::DiscardSelectPurpose::HeadbuttPutOnDraw,
        });
    }

    Ok(queue)
}

fn has_upgradeable_other_hand_card(state: &CombatState, exclude_id: CardId) -> bool {
    state
        .piles
        .hand
        .iter()
        .any(|card| card.id != exclude_id && upgrade_content_id(card.content_id).is_some())
}

fn entrench_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DoublePlayerBlock,
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn bloodletting_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::LoseHp {
            amount: 3,
            source: HpLossSource::Card(card_id),
        },
        InternalAction::GainEnergy { amount: 2 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn hemokinesis_queue(
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::LoseHp {
            amount: 2,
            source: HpLossSource::Card(card_id),
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

fn blood_for_blood_queue(
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
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

fn feed_queue(
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DealFeedDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: definition.values.damage.unwrap_or(0),
            },
            max_hp_gain: 3,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn dropkick_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let target_has_vulnerable = state
        .monsters
        .iter()
        .find(|monster| monster.id == target)
        .map(|monster| monster.powers.vulnerable > 0)
        .unwrap_or(false);
    let mut queue = VecDeque::from([
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
    ]);

    if target_has_vulnerable {
        queue.push_back(InternalAction::GainEnergy { amount: 1 });
        queue.push_back(InternalAction::DrawCards { count: 1 });
    }

    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });

    Ok(queue)
}

fn heavy_blade_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let extra_strength = 2 * (state.player.powers.strength + state.player.temp_strength);
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: (definition.values.damage.unwrap_or(0) + extra_strength).max(0),
            },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn perfected_strike_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let strike_count = combat_strike_named_card_count(state) as i32;
    let base_damage = definition.values.damage.unwrap_or(0) + (2 * strike_count);
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: strike_damage_with_relics(&state.relics, base_damage),
            },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn combat_strike_named_card_count(state: &CombatState) -> usize {
    state
        .piles
        .hand
        .iter()
        .chain(state.piles.draw_pile.iter())
        .chain(state.piles.discard_pile.iter())
        .chain(state.piles.exhaust_pile.iter())
        .filter(|card| {
            get_card_definition(card.content_id)
                .map(|definition| {
                    definition.key.contains("STRIKE")
                        || definition.key.contains("Strike")
                        || definition.name.contains("Strike")
                })
                .unwrap_or(false)
        })
        .count()
}

fn wild_strike_queue(
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
        InternalAction::AddCardToPile {
            content_id: WOUND_ID,
            to: CardPile::DrawPile,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn rampage_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let bonus = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.rampage_damage_bonus)
        .unwrap_or(0);

    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: definition.values.damage.unwrap_or(0) + bonus,
            },
        },
        InternalAction::IncreaseRampageDamage { card_id, amount: 5 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn power_through_queue(
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
        InternalAction::AddCardToPile {
            content_id: WOUND_ID,
            to: CardPile::Hand,
        },
        InternalAction::AddCardToPile {
            content_id: WOUND_ID,
            to: CardPile::Hand,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn infernal_blade_queue(
    state: &mut CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let generated = infernal_blade_generated_attack(state);
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::AddGeneratedCardToPile {
            content_id: generated,
            to: CardPile::Hand,
            temp_cost: Some(0),
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn infernal_blade_generated_attack(state: &mut CombatState) -> ContentId {
    let pool = infernal_blade_modeled_attack_pool();
    let Some(rng) = state.card_random_rng.as_mut() else {
        return pool[0];
    };
    let index = rng.random_int((pool.len() - 1) as i32) as usize;
    pool[index]
}

pub(crate) fn infernal_blade_modeled_attack_pool() -> Vec<ContentId> {
    ironclad_combat_attack_discovery_pool()
        .into_iter()
        .filter(|content_id| {
            get_card_definition(*content_id)
                .is_some_and(|definition| definition.card_type == CardType::Attack)
        })
        .collect()
}

fn bandage_up_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::HealPlayer { amount: 4 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn panacea_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainArtifact { amount: 1 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn reckless_charge_queue(
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
        InternalAction::AddCardToPile {
            content_id: DAZED_ID,
            to: CardPile::DrawPile,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn pummel_queue(
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let damage = definition.values.damage.unwrap_or(0);
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);

    for _ in 0..4 {
        queue.push_back(InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: damage,
            },
        });
    }

    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });

    Ok(queue)
}

fn clothesline_queue(
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
        InternalAction::ApplyWeak { target, amount: 2 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn intimidate_queue(
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

    for monster in state.monsters.iter().filter(|monster| monster.alive) {
        queue.push_back(InternalAction::ApplyWeak {
            target: monster.id,
            amount: 1,
        });
    }

    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });

    Ok(queue)
}

fn blind_queue(
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

    for monster in state.monsters.iter().filter(|monster| monster.alive) {
        queue.push_back(InternalAction::ApplyWeak {
            target: monster.id,
            amount: 2,
        });
    }

    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });

    Ok(queue)
}

fn trip_queue(
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

    for monster in state.monsters.iter().filter(|monster| monster.alive) {
        queue.push_back(InternalAction::ApplyVulnerable {
            target: monster.id,
            amount: definition.values.vulnerable.unwrap_or(0),
        });
    }

    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });

    Ok(queue)
}

fn shockwave_queue(
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

    for monster in state.monsters.iter().filter(|monster| monster.alive) {
        queue.push_back(InternalAction::ApplyWeak {
            target: monster.id,
            amount: 3,
        });
        queue.push_back(InternalAction::ApplyVulnerable {
            target: monster.id,
            amount: definition.values.vulnerable.unwrap_or(0),
        });
        queue.push_back(InternalAction::ReduceMonsterStrength {
            target: monster.id,
            amount: 3,
        });
    }

    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });

    Ok(queue)
}

fn disarm_queue(
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::ReduceMonsterStrength { target, amount: 2 },
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

fn rage_queue(card_id: CardId, definition: &CardDefinition) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainRage { amount: 3 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn double_tap_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainDoubleTap { amount: 1 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn barricade_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainBarricade { amount: 1 },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn evolve_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainEvolve { amount: 1 },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn berserk_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::ApplyPlayerVulnerable {
            amount: definition.values.vulnerable.unwrap_or(0),
        },
        InternalAction::GainBerserk { amount: 1 },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn rupture_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainRupture { amount: 1 },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn corruption_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainCorruption { amount: 1 },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn juggernaut_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainJuggernaut {
            amount: definition.values.damage.unwrap_or(0),
        },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn brutality_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainBrutality { amount: 1 },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn exhume_queue(state: &CombatState, card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    if !has_exhumable_card(state) {
        return Err(SimError::IllegalAction("Exhume requires an exhumable card"));
    }

    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::AwaitExhaustSelect {
            source_card_id: card_id,
            purpose: crate::combat::ExhaustSelectPurpose::ExhumeReturnToHand,
        },
    ]))
}

fn has_exhumable_card(state: &CombatState) -> bool {
    state
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.content_id != EXHUME_ID)
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

fn second_wind_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let exhaust_targets = non_attack_hand_cards_except(state, card_id);
    let block = definition.values.block.unwrap_or(0) * exhaust_targets.len() as i32;
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);
    for exhaust_target in exhaust_targets {
        queue.push_back(InternalAction::MoveCard {
            card_id: exhaust_target,
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        });
    }
    if block > 0 {
        queue.push_back(InternalAction::GainBlock { amount: block });
    }
    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });
    Ok(queue)
}

fn non_attack_hand_cards_except(state: &CombatState, exclude_id: CardId) -> Vec<CardId> {
    state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != exclude_id)
        .filter(|card| {
            get_card_definition(card.content_id)
                .is_some_and(|definition| definition.card_type != CardType::Attack)
        })
        .map(|card| card.id)
        .collect()
}

fn fiend_fire_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let exhaust_targets = other_hand_cards(state, card_id);
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);
    for exhaust_target in &exhaust_targets {
        queue.push_back(InternalAction::MoveCard {
            card_id: *exhaust_target,
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        });
    }
    for _ in &exhaust_targets {
        queue.push_back(InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: definition.values.damage.unwrap_or(0),
            },
        });
    }
    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });
    Ok(queue)
}

fn other_hand_cards(state: &CombatState, exclude_id: CardId) -> Vec<CardId> {
    state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != exclude_id)
        .map(|card| card.id)
        .collect()
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

fn reaper_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DealDamageAllAndHealUnblocked {
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
    let chemical_x_bonus = if state.relics.contains(&Relic::ChemicalX) {
        CHEMICAL_X_BONUS_X
    } else {
        0
    };
    if x + chemical_x_bonus < 1 {
        return Err(SimError::IllegalAction(
            "Whirlwind requires at least 1 energy",
        ));
    }

    let damage = definition.values.damage.unwrap_or(0);
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: x },
    ]);

    for _ in 0..(x + chemical_x_bonus) {
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
            purpose: HandSelectPurpose::WarcryPutOnDraw,
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

pub(super) fn validate_havoc_target(
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

fn finesse_queue(
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
        InternalAction::DrawCards { count: 1 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn flame_barrier_queue(
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
        InternalAction::GainTemporaryThorns { amount: 4 },
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

fn combust_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainCombust { amount: 1 },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn demon_form_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainRitual { amount: 2 },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn fire_breathing_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainFireBreathing {
            amount: definition.values.damage.unwrap_or(0),
        },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
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

fn limit_break_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainStrength {
            amount: state.player.powers.strength,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn offering_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::LoseHp {
            amount: 6,
            source: HpLossSource::Card(card_id),
        },
        InternalAction::GainEnergy { amount: 2 },
        InternalAction::DrawCards { count: 3 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
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
