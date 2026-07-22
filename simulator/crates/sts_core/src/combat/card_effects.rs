use crate::{
    action::{CardPile, HpLossSource, InternalAction},
    card::{CardDefinition, CardType, TargetRequirement},
    combat::{
        damage::{DamageInfo, DamageSource},
        CombatDecisionState, CombatState, HandSelectPurpose,
    },
    content::cards::{
        card_instance_is_upgradeable, get_card_definition, is_curse_content_id,
        ritual_dagger_card_damage, ritual_dagger_card_growth, searing_blow_card_damage, ANGER_ID,
        ANGER_PLUS_ID, APOTHEOSIS_ID, APOTHEOSIS_PLUS_ID, APPARITION_ID, APPARITION_PLUS_ID,
        ARMAMENTS_ID, ARMAMENTS_PLUS_ID, BANDAGE_UP_ID, BANDAGE_UP_PLUS_ID, BARRICADE_ID,
        BARRICADE_PLUS_ID, BASH_ID, BASH_PLUS_ID, BATTLE_TRANCE_ID, BATTLE_TRANCE_PLUS_ID,
        BERSERK_ID, BERSERK_PLUS_ID, BITE_ID, BITE_PLUS_ID, BLIND_ID, BLIND_PLUS_ID,
        BLOODLETTING_ID, BLOODLETTING_PLUS_ID, BLOOD_FOR_BLOOD_ID, BLOOD_FOR_BLOOD_PLUS_ID,
        BODY_SLAM_ID, BODY_SLAM_PLUS_ID, BRUTALITY_ID, BRUTALITY_PLUS_ID, BURNING_PACT_ID,
        BURNING_PACT_PLUS_ID, CHRYSALIS_ID, CHRYSALIS_PLUS_ID, CLASH_ID, CLASH_PLUS_ID, CLEAVE_ID,
        CLEAVE_PLUS_ID, CLOTHESLINE_ID, CLOTHESLINE_PLUS_ID, COMBUST_ID, COMBUST_PLUS_ID,
        CORRUPTION_ID, CORRUPTION_PLUS_ID, DARK_EMBRACE_ID, DARK_EMBRACE_PLUS_ID, DARK_SHACKLES_ID,
        DARK_SHACKLES_PLUS_ID, DAZED_ID, DEEP_BREATH_ID, DEEP_BREATH_PLUS_ID, DEFEND_R_ID,
        DEFEND_R_PLUS_ID, DEMON_FORM_ID, DEMON_FORM_PLUS_ID, DISARM_ID, DISARM_PLUS_ID,
        DISCOVERY_ID, DISCOVERY_PLUS_ID, DOUBLE_TAP_ID, DOUBLE_TAP_PLUS_ID, DRAMATIC_ENTRANCE_ID,
        DRAMATIC_ENTRANCE_PLUS_ID, DROPKICK_ID, DROPKICK_PLUS_ID, DUAL_WIELD_ID,
        DUAL_WIELD_PLUS_ID, ENLIGHTENMENT_ID, ENLIGHTENMENT_PLUS_ID, ENTRENCH_ID, ENTRENCH_PLUS_ID,
        EVOLVE_ID, EVOLVE_PLUS_ID, EXHUME_ID, EXHUME_PLUS_ID, FEED_ID, FEED_PLUS_ID,
        FEEL_NO_PAIN_ID, FEEL_NO_PAIN_PLUS_ID, FIEND_FIRE_ID, FIEND_FIRE_PLUS_ID, FINESSE_ID,
        FINESSE_PLUS_ID, FIRE_BREATHING_ID, FIRE_BREATHING_PLUS_ID, FLAME_BARRIER_ID,
        FLAME_BARRIER_PLUS_ID, FLASH_OF_STEEL_ID, FLASH_OF_STEEL_PLUS_ID, FLEX_ID, FLEX_PLUS_ID,
        FORETHOUGHT_ID, FORETHOUGHT_PLUS_ID, HAND_OF_GREED_ID, HAND_OF_GREED_PLUS_ID, HAVOC_ID,
        HAVOC_PLUS_ID, HEADBUTT_ID, HEADBUTT_PLUS_ID, HEAVY_BLADE_ID, HEAVY_BLADE_PLUS_ID,
        HEMOKINESIS_ID, HEMOKINESIS_PLUS_ID, IMMOLATE_ID, IMMOLATE_PLUS_ID, IMPATIENCE_ID,
        IMPATIENCE_PLUS_ID, INFERNAL_BLADE_ID, INFERNAL_BLADE_PLUS_ID, INFLAME_ID, INFLAME_PLUS_ID,
        INTIMIDATE_ID, INTIMIDATE_PLUS_ID, IRON_WAVE_ID, IRON_WAVE_PLUS_ID, JACK_OF_ALL_TRADES_ID,
        JACK_OF_ALL_TRADES_PLUS_ID, JAX_ID, JAX_PLUS_ID, JUGGERNAUT_ID, JUGGERNAUT_PLUS_ID,
        LIMIT_BREAK_ID, LIMIT_BREAK_PLUS_ID, MADNESS_ID, MADNESS_PLUS_ID, MAGNETISM_ID,
        MAGNETISM_PLUS_ID, MASTER_OF_STRATEGY_ID, MASTER_OF_STRATEGY_PLUS_ID, MAYHEM_ID,
        MAYHEM_PLUS_ID, METALLICIZE_ID, METALLICIZE_PLUS_ID, METAMORPHOSIS_ID,
        METAMORPHOSIS_PLUS_ID, MIND_BLAST_ID, MIND_BLAST_PLUS_ID, OFFERING_ID, OFFERING_PLUS_ID,
        PANACEA_ID, PANACEA_PLUS_ID, PANACHE_ID, PANACHE_PLUS_ID, PANIC_BUTTON_ID,
        PANIC_BUTTON_PLUS_ID, PERFECTED_STRIKE_ID, PERFECTED_STRIKE_PLUS_ID, POMMEL_STRIKE_ID,
        POMMEL_STRIKE_PLUS_ID, POWER_THROUGH_ID, POWER_THROUGH_PLUS_ID, PUMMEL_ID, PUMMEL_PLUS_ID,
        PURITY_ID, PURITY_PLUS_ID, RAGE_ID, RAGE_PLUS_ID, RAMPAGE_ID, RAMPAGE_PLUS_ID, REAPER_ID,
        REAPER_PLUS_ID, RECKLESS_CHARGE_ID, RECKLESS_CHARGE_PLUS_ID, RITUAL_DAGGER_ID, RUPTURE_ID,
        RUPTURE_PLUS_ID, SADISTIC_NATURE_ID, SADISTIC_NATURE_PLUS_ID, SEARING_BLOW_ID,
        SEARING_BLOW_PLUS_ID, SECOND_WIND_ID, SECOND_WIND_PLUS_ID, SECRET_TECHNIQUE_ID,
        SECRET_TECHNIQUE_PLUS_ID, SECRET_WEAPON_ID, SECRET_WEAPON_PLUS_ID, SEEING_RED_ID,
        SEEING_RED_PLUS_ID, SEVER_SOUL_ID, SEVER_SOUL_PLUS_ID, SHOCKWAVE_ID, SHOCKWAVE_PLUS_ID,
        SHRUG_IT_OFF_ID, SHRUG_IT_OFF_PLUS_ID, SLIMED_ID, SPOT_WEAKNESS_ID, SPOT_WEAKNESS_PLUS_ID,
        STRIKE_R_ID, STRIKE_R_PLUS_ID, SWIFT_STRIKE_ID, SWIFT_STRIKE_PLUS_ID, SWORD_BOOMERANG_ID,
        SWORD_BOOMERANG_PLUS_ID, THE_BOMB_DAMAGE, THE_BOMB_ID, THE_BOMB_PLUS_ID, THE_BOMB_TURNS,
        THINKING_AHEAD_ID, THINKING_AHEAD_PLUS_ID, THUNDERCLAP_ID, THUNDERCLAP_PLUS_ID,
        TRANSMUTATION_ID, TRANSMUTATION_PLUS_ID, TRIP_ID, TRIP_PLUS_ID, TRUE_GRIT_ID,
        TRUE_GRIT_PLUS_ID, TWIN_STRIKE_ID, TWIN_STRIKE_PLUS_ID, UPPERCUT_ID, UPPERCUT_PLUS_ID,
        VIOLENCE_ID, VIOLENCE_PLUS_ID, WARCRY_ID, WARCRY_PLUS_ID, WHIRLWIND_ID, WHIRLWIND_PLUS_ID,
        WILD_STRIKE_ID, WILD_STRIKE_PLUS_ID, WOUND_ID,
    },
    content::shop_pool::{
        colorless_discovery_pool, ironclad_combat_attack_discovery_pool,
        ironclad_combat_discovery_pool, ironclad_combat_skill_discovery_pool,
    },
    ids::{CardId, ContentId, MonsterId},
    relic::{
        strike_damage_with_relics, Relic, AKABEKO_DAMAGE, CHEMICAL_X_BONUS_X, PEN_NIB_THRESHOLD,
    },
    CardInstance, MonsterIntent, SimError, SimResult,
};
use std::collections::VecDeque;

const DISCOVERY_ACTION_HIDDEN_GENERATIONS: usize = 4;
const DISCOVERY_ACTION_SCREEN_SETTLE_DRAWS: usize = 0;

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
        DEFEND_R_ID | DEFEND_R_PLUS_ID => defend_queue(card_id, definition),
        BASH_ID | BASH_PLUS_ID => bash_queue(
            card_id,
            target.expect("validated Bash has a target"),
            definition,
        ),
        SLIMED_ID => slimed_queue(card_id),
        ANGER_ID | ANGER_PLUS_ID => anger_queue(
            *card,
            card_id,
            target.expect("validated Anger has a target"),
            definition,
        ),
        IRON_WAVE_ID | IRON_WAVE_PLUS_ID => iron_wave_queue(
            card_id,
            target.expect("validated Iron Wave has a target"),
            definition,
        ),
        BODY_SLAM_ID | BODY_SLAM_PLUS_ID => body_slam_queue(
            state,
            card_id,
            target.expect("validated Body Slam has a target"),
            definition,
        ),
        BITE_ID | BITE_PLUS_ID => bite_queue(
            card_id,
            target.expect("validated Bite has a target"),
            definition,
        ),
        CLASH_ID | CLASH_PLUS_ID | SWIFT_STRIKE_ID | SWIFT_STRIKE_PLUS_ID => generic_attack_queue(
            card_id,
            target.expect("validated generic attack has a target"),
            definition,
        ),
        HAND_OF_GREED_ID | HAND_OF_GREED_PLUS_ID => hand_of_greed_queue(
            card_id,
            target.expect("validated Hand of Greed has a target"),
            definition,
        ),
        RITUAL_DAGGER_ID => ritual_dagger_queue(
            card,
            target.expect("validated Ritual Dagger has a target"),
            definition,
        ),
        WILD_STRIKE_ID | WILD_STRIKE_PLUS_ID => wild_strike_queue(
            card_id,
            target.expect("validated Wild Strike has a target"),
            definition,
        ),
        HEAVY_BLADE_ID | HEAVY_BLADE_PLUS_ID => heavy_blade_queue(
            state,
            card_id,
            target.expect("validated Heavy Blade has a target"),
            definition,
        ),
        PERFECTED_STRIKE_ID | PERFECTED_STRIKE_PLUS_ID => perfected_strike_queue(
            state,
            card_id,
            target.expect("validated Perfected Strike has a target"),
            definition,
        ),
        RAMPAGE_ID | RAMPAGE_PLUS_ID => rampage_queue(
            state,
            card_id,
            target.expect("validated Rampage has a target"),
            definition,
        ),
        POWER_THROUGH_ID | POWER_THROUGH_PLUS_ID => power_through_queue(card_id, definition),
        APOTHEOSIS_ID | APOTHEOSIS_PLUS_ID => apotheosis_queue(card_id, definition),
        ARMAMENTS_ID | ARMAMENTS_PLUS_ID => armaments_queue(state, card_id, definition),
        HEADBUTT_ID | HEADBUTT_PLUS_ID => headbutt_queue(
            state,
            card_id,
            target.expect("validated Headbutt has a target"),
            definition,
        ),
        FLAME_BARRIER_ID | FLAME_BARRIER_PLUS_ID => flame_barrier_queue(card_id, definition),
        ENTRENCH_ID | ENTRENCH_PLUS_ID => entrench_queue(card_id),
        RECKLESS_CHARGE_ID | RECKLESS_CHARGE_PLUS_ID => reckless_charge_queue(
            card_id,
            target.expect("validated Reckless Charge has a target"),
            definition,
        ),
        PUMMEL_ID | PUMMEL_PLUS_ID => pummel_queue(
            card_id,
            target.expect("validated Pummel has a target"),
            definition,
        ),
        CLOTHESLINE_ID | CLOTHESLINE_PLUS_ID => clothesline_queue(
            card_id,
            target.expect("validated Clothesline has a target"),
            definition,
        ),
        FIEND_FIRE_ID | FIEND_FIRE_PLUS_ID => fiend_fire_queue(
            state,
            card_id,
            target.expect("validated Fiend Fire has a target"),
            definition,
        ),
        FEED_ID | FEED_PLUS_ID => feed_queue(
            card_id,
            target.expect("validated Feed has a target"),
            definition,
        ),
        REAPER_ID | REAPER_PLUS_ID => reaper_queue(card_id, definition),
        CLEAVE_ID | CLEAVE_PLUS_ID | DRAMATIC_ENTRANCE_ID | DRAMATIC_ENTRANCE_PLUS_ID => {
            cleave_queue(card_id, definition)
        }
        IMMOLATE_ID | IMMOLATE_PLUS_ID => immolate_queue(card_id, definition),
        TWIN_STRIKE_ID | TWIN_STRIKE_PLUS_ID => twin_strike_queue(
            card_id,
            target.expect("validated Twin Strike has a target"),
            definition,
        ),
        DEEP_BREATH_ID | DEEP_BREATH_PLUS_ID => deep_breath_queue(card_id, definition),
        ENLIGHTENMENT_ID | ENLIGHTENMENT_PLUS_ID => enlightenment_queue(state, card_id, definition),
        FINESSE_ID | FINESSE_PLUS_ID => finesse_queue(card_id, definition),
        IMPATIENCE_ID | IMPATIENCE_PLUS_ID => impatience_queue(card_id, definition),
        PANIC_BUTTON_ID | PANIC_BUTTON_PLUS_ID => panic_button_queue(card_id, definition),
        SHRUG_IT_OFF_ID | SHRUG_IT_OFF_PLUS_ID => shrug_it_off_queue(card_id, definition),
        TRUE_GRIT_ID | TRUE_GRIT_PLUS_ID => true_grit_queue(state, card_id, definition),
        BURNING_PACT_ID | BURNING_PACT_PLUS_ID => burning_pact_queue(state, card_id),
        INFERNAL_BLADE_ID | INFERNAL_BLADE_PLUS_ID => {
            infernal_blade_queue(&mut queued_state, card_id, definition)
        }
        CHRYSALIS_ID | CHRYSALIS_PLUS_ID => chrysalis_queue(&mut queued_state, card_id, definition),
        METAMORPHOSIS_ID | METAMORPHOSIS_PLUS_ID => {
            metamorphosis_queue(&mut queued_state, card_id, definition)
        }
        DISCOVERY_ID | DISCOVERY_PLUS_ID => discovery_queue(&mut queued_state, card_id, definition),
        JACK_OF_ALL_TRADES_ID | JACK_OF_ALL_TRADES_PLUS_ID => {
            jack_of_all_trades_queue(&mut queued_state, card_id, definition)
        }
        MADNESS_ID | MADNESS_PLUS_ID => madness_queue(card_id, definition),
        BANDAGE_UP_ID | BANDAGE_UP_PLUS_ID => bandage_up_queue(card_id, definition),
        VIOLENCE_ID | VIOLENCE_PLUS_ID => violence_queue(card_id, definition),
        APPARITION_ID | APPARITION_PLUS_ID => apparition_queue(card_id, definition),
        PANACEA_ID | PANACEA_PLUS_ID => panacea_queue(card_id, definition),
        PANACHE_ID | PANACHE_PLUS_ID => panache_queue(card_id, definition),
        SADISTIC_NATURE_ID | SADISTIC_NATURE_PLUS_ID => sadistic_nature_queue(card_id, definition),
        FORETHOUGHT_ID | FORETHOUGHT_PLUS_ID => forethought_queue(state, card_id, definition),
        PURITY_ID | PURITY_PLUS_ID => purity_queue(state, card_id),
        FEEL_NO_PAIN_ID | FEEL_NO_PAIN_PLUS_ID => feel_no_pain_queue(card_id, definition),
        DARK_EMBRACE_ID | DARK_EMBRACE_PLUS_ID => dark_embrace_queue(card_id, definition),
        COMBUST_ID | COMBUST_PLUS_ID => combust_queue(card_id, definition),
        CORRUPTION_ID | CORRUPTION_PLUS_ID => corruption_queue(card_id),
        BARRICADE_ID | BARRICADE_PLUS_ID => barricade_queue(card_id),
        EVOLVE_ID | EVOLVE_PLUS_ID => evolve_queue(card_id, definition),
        BERSERK_ID | BERSERK_PLUS_ID => berserk_queue(card_id, definition),
        RUPTURE_ID | RUPTURE_PLUS_ID => rupture_queue(card_id, definition),
        JUGGERNAUT_ID | JUGGERNAUT_PLUS_ID => juggernaut_queue(card_id, definition),
        BRUTALITY_ID | BRUTALITY_PLUS_ID => brutality_queue(card_id),
        MAGNETISM_ID | MAGNETISM_PLUS_ID => magnetism_queue(card_id),
        MAYHEM_ID | MAYHEM_PLUS_ID => mayhem_queue(card_id),
        FIRE_BREATHING_ID | FIRE_BREATHING_PLUS_ID => fire_breathing_queue(card_id, definition),
        EXHUME_ID | EXHUME_PLUS_ID => exhume_queue(state, card_id),
        DEMON_FORM_ID | DEMON_FORM_PLUS_ID => demon_form_queue(card_id, definition),
        METALLICIZE_ID | METALLICIZE_PLUS_ID => metallicize_queue(card_id, definition),
        POMMEL_STRIKE_ID | POMMEL_STRIKE_PLUS_ID | FLASH_OF_STEEL_ID | FLASH_OF_STEEL_PLUS_ID => {
            pommel_strike_queue(
                card_id,
                target.expect("validated draw attack has a target"),
                definition,
            )
        }
        MIND_BLAST_ID | MIND_BLAST_PLUS_ID => mind_blast_queue(
            state,
            card_id,
            target.expect("validated Mind Blast has a target"),
            definition,
        ),
        BATTLE_TRANCE_ID | BATTLE_TRANCE_PLUS_ID => battle_trance_queue(card_id, definition),
        DOUBLE_TAP_ID | DOUBLE_TAP_PLUS_ID => double_tap_queue(card_id, definition),
        SEEING_RED_ID | SEEING_RED_PLUS_ID => seeing_red_queue(card_id, definition),
        BLOODLETTING_ID | BLOODLETTING_PLUS_ID => bloodletting_queue(card_id, definition),
        HEMOKINESIS_ID | HEMOKINESIS_PLUS_ID => hemokinesis_queue(
            card_id,
            target.expect("validated Hemokinesis has a target"),
            definition,
        ),
        BLOOD_FOR_BLOOD_ID | BLOOD_FOR_BLOOD_PLUS_ID => blood_for_blood_queue(
            card_id,
            target.expect("validated Blood for Blood has a target"),
            definition,
        ),
        DROPKICK_ID | DROPKICK_PLUS_ID => dropkick_queue(
            state,
            card_id,
            target.expect("validated Dropkick has a target"),
            definition,
        ),
        BLIND_ID | BLIND_PLUS_ID => blind_queue(
            state,
            card_id,
            target.filter(|_| definition.target == TargetRequirement::Enemy),
            definition,
        ),
        TRIP_ID | TRIP_PLUS_ID => trip_queue(
            state,
            card_id,
            target.filter(|_| definition.target == TargetRequirement::Enemy),
            definition,
        ),
        INTIMIDATE_ID | INTIMIDATE_PLUS_ID => intimidate_queue(state, card_id, definition),
        SHOCKWAVE_ID | SHOCKWAVE_PLUS_ID => shockwave_queue(state, card_id, definition),
        DISARM_ID | DISARM_PLUS_ID => disarm_queue(
            card_id,
            target.expect("validated Disarm has a target"),
            definition,
        ),
        DARK_SHACKLES_ID | DARK_SHACKLES_PLUS_ID => dark_shackles_queue(
            card_id,
            target.expect("validated Dark Shackles has a target"),
            definition,
        ),
        RAGE_ID | RAGE_PLUS_ID => rage_queue(card_id, definition),
        INFLAME_ID | INFLAME_PLUS_ID => inflame_queue(card_id, definition),
        FLEX_ID | FLEX_PLUS_ID => flex_queue(card_id, definition),
        JAX_ID | JAX_PLUS_ID => jax_queue(card_id, definition),
        LIMIT_BREAK_ID | LIMIT_BREAK_PLUS_ID => limit_break_queue(state, card_id, definition),
        MASTER_OF_STRATEGY_ID | MASTER_OF_STRATEGY_PLUS_ID => {
            master_of_strategy_queue(card_id, definition)
        }
        THE_BOMB_ID | THE_BOMB_PLUS_ID => the_bomb_queue(card_id, definition),
        OFFERING_ID | OFFERING_PLUS_ID => offering_queue(card_id, definition),
        SPOT_WEAKNESS_ID | SPOT_WEAKNESS_PLUS_ID => {
            spot_weakness_queue(state, card_id, target, definition)
        }
        THUNDERCLAP_ID | THUNDERCLAP_PLUS_ID => thunderclap_queue(state, card_id, definition),
        UPPERCUT_ID | UPPERCUT_PLUS_ID => uppercut_queue(
            card_id,
            target.expect("validated Uppercut has a target"),
            definition,
        ),
        SWORD_BOOMERANG_ID | SWORD_BOOMERANG_PLUS_ID => {
            sword_boomerang_queue(state, card_id, definition)
        }
        WHIRLWIND_ID | WHIRLWIND_PLUS_ID => whirlwind_queue(state, card_id, definition),
        TRANSMUTATION_ID | TRANSMUTATION_PLUS_ID => transmutation_queue(state, card_id, definition),
        SECRET_TECHNIQUE_ID | SECRET_TECHNIQUE_PLUS_ID => {
            secret_technique_queue(state, card_id, definition)
        }
        SECRET_WEAPON_ID | SECRET_WEAPON_PLUS_ID => secret_weapon_queue(state, card_id, definition),
        HAVOC_ID | HAVOC_PLUS_ID => havoc_queue(state, card_id, definition, target),
        WARCRY_ID | WARCRY_PLUS_ID => warcry_queue(state, card_id, definition),
        THINKING_AHEAD_ID | THINKING_AHEAD_PLUS_ID => {
            thinking_ahead_queue(state, card_id, definition)
        }
        DUAL_WIELD_ID | DUAL_WIELD_PLUS_ID => dual_wield_queue(state, card_id, definition),
        SEARING_BLOW_ID | SEARING_BLOW_PLUS_ID => searing_blow_queue(
            state,
            card_id,
            target.expect("validated Searing Blow has a target"),
            definition,
        ),
        SECOND_WIND_ID | SECOND_WIND_PLUS_ID => second_wind_queue(state, card_id, definition),
        SEVER_SOUL_ID | SEVER_SOUL_PLUS_ID => sever_soul_queue(
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
    if state.duplication_potion_pending || state.duplication_potion_stacks > 0 {
        queue = apply_duplication_potion_to_queue(queue, card_id);
    }
    apply_akabeko_to_first_attack_queue(state, definition.card_type, card_id, &mut queue);
    if should_apply_necronomicon(state, card, definition) {
        queue = apply_necronomicon_to_queue(queue, card_id);
    }
    if definition.card_type == CardType::Attack && state.double_tap_pending > 0 {
        queue = apply_double_tap_to_queue(queue, card_id);
    }
    // Pen Nib is consumed by the original card play. Expand copied-card
    // effects first, then modify only the original effects before its card
    // move; otherwise Double Tap/Necronomicon clone the already-doubled
    // damage and incorrectly receive Pen Nib too.
    apply_pen_nib_to_tenth_attack_queue(state, definition.card_type, card_id, &mut queue);

    apply_effective_cost_to_played_card_queue(card, definition, &mut queue);
    apply_corruption_to_played_skill_queue(state, definition, card_id, &mut queue);
    apply_strange_spoon_to_played_card_move(&mut queued_state, definition, card_id, &mut queue);

    Ok((queued_state, queue))
}

pub(super) fn play_top_draw_card_queue(
    state: &CombatState,
    card: CardInstance,
    target: Option<MonsterId>,
    force_exhaust: bool,
) -> SimResult<(CombatState, VecDeque<InternalAction>)> {
    let definition =
        get_card_definition(card.content_id).ok_or(SimError::UnknownContent(card.content_id))?;
    validate_havoc_target(definition, target, false)?;

    // Card effect builders operate on the authoritative hand-play shape. A
    // top-draw play stages its extracted card in that slot while the shared
    // builder constructs effects, then changes only the play envelope: no
    // energy payment and a top-draw-specific final destination.
    let mut staged = state.clone();
    // Put the staged source first so test/debug states with duplicate card IDs
    // cannot redirect effect construction to an unrelated hand card. Validated
    // production states still require globally unique card IDs.
    staged.piles.hand.insert(0, card);
    let (mut queued_state, mut queue) = play_card_queue(&staged, card.id, target)?;

    queue.retain(|action| {
        !matches!(
            action,
            InternalAction::SpendEnergy { .. } | InternalAction::SpendCardEnergy { .. }
        ) && !matches!(
            action,
            InternalAction::RemoveCard {
                card_id,
                from: CardPile::Hand,
            } if *card_id == card.id
        )
    });
    for action in &mut queue {
        if let InternalAction::DrawCardsWhilePlayedCardIsInLimbo { card_id, count } = *action {
            if card_id == card.id {
                *action = InternalAction::DrawCards { count };
            }
        }
    }
    if definition.id == WHIRLWIND_ID || definition.id == WHIRLWIND_PLUS_ID {
        queue = coalesce_top_draw_all_enemy_hits(queue, card.id);
    }

    let shared_destination = queue.iter().rev().find_map(|action| match action {
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to,
        } if *card_id == card.id => Some(*to),
        _ => None,
    });
    let destination = top_draw_card_destination(
        &mut queued_state,
        definition,
        force_exhaust,
        shared_destination,
    );
    queue.retain(|action| !is_card_move_for(*action, card.id));
    let movement = InternalAction::MoveCard {
        card_id: card.id,
        from: CardPile::Hand,
        to: destination,
    };
    let played_index = queue
        .iter()
        .position(
            |action| matches!(action, InternalAction::PlayCard { card_id } if *card_id == card.id),
        )
        .ok_or(SimError::InvalidState(
            "top-draw card queue has no play action",
        ))?;
    queue.insert(played_index + 1, movement);

    Ok((queued_state, queue))
}

fn coalesce_top_draw_all_enemy_hits(
    mut queue: VecDeque<InternalAction>,
    card_id: CardId,
) -> VecDeque<InternalAction> {
    let mut grouped = VecDeque::with_capacity(queue.len());
    while let Some(action) = queue.pop_front() {
        let InternalAction::DealDamageAll { source, amount } = action else {
            grouped.push_back(action);
            continue;
        };
        if source != card_id {
            grouped.push_back(action);
            continue;
        }

        let mut times = 1;
        while matches!(
            queue.front(),
            Some(InternalAction::DealDamageAll {
                source: next_source,
                amount: next_amount,
            }) if *next_source == source && *next_amount == amount
        ) {
            queue.pop_front();
            times += 1;
        }
        if times == 1 {
            grouped.push_back(action);
        } else {
            grouped.push_back(InternalAction::DealDamageAllRepeated {
                source,
                amount,
                times,
            });
        }
    }
    grouped
}

fn top_draw_card_destination(
    state: &mut CombatState,
    definition: &CardDefinition,
    force_exhaust: bool,
    shared_destination: Option<CardPile>,
) -> CardPile {
    if definition.card_type == CardType::Power {
        return CardPile::DiscardPile;
    }
    let shared_destination_is_authoritative = !force_exhaust
        || definition.keywords.exhaust
        || (definition.card_type == CardType::Skill && state.player.powers.corruption > 0);
    if shared_destination_is_authoritative {
        if let Some(destination) = shared_destination {
            return destination;
        }
    }

    let exhaust = force_exhaust
        || definition.keywords.exhaust
        || (definition.card_type == CardType::Skill && state.player.powers.corruption > 0);
    if !exhaust {
        return CardPile::DiscardPile;
    }
    if state.relics.contains(&Relic::StrangeSpoon) && state.rng.card_random_rng.random_bool() {
        CardPile::DiscardPile
    } else {
        CardPile::ExhaustPile
    }
}

fn apply_effective_cost_to_played_card_queue(
    card: &CardInstance,
    definition: &CardDefinition,
    queue: &mut VecDeque<InternalAction>,
) {
    let printed_cost = i32::from(definition.cost);
    let effective_cost = effective_card_cost_for_queue(card, definition);
    if effective_cost == printed_cost {
        return;
    }

    for action in queue.iter_mut() {
        if let InternalAction::SpendEnergy { amount } = action {
            if *amount == printed_cost {
                *amount = effective_cost;
            }
            break;
        }
    }
}

fn effective_card_cost_for_queue(card: &CardInstance, definition: &CardDefinition) -> i32 {
    if let Some(cost) = card.temp_cost {
        return i32::from(cost);
    }
    if definition.id == BLOOD_FOR_BLOOD_ID || definition.id == BLOOD_FOR_BLOOD_PLUS_ID {
        return (i32::from(definition.cost) - card.blood_for_blood_cost_reduction).max(0);
    }
    i32::from(definition.cost)
}

fn should_apply_necronomicon(
    state: &CombatState,
    card: &CardInstance,
    definition: &CardDefinition,
) -> bool {
    definition.card_type == CardType::Attack
        && state.relics.contains(&Relic::Necronomicon)
        && !state.relic_counters.necronomicon_used_this_turn
        && effective_card_cost_for_queue(card, definition) >= 2
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

    let rng = &mut state.rng.card_random_rng;
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
        if is_card_move_for(*action, card_id) {
            break;
        }
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
            InternalAction::DealDamageRandomEnemy { source, amount } if *source == card_id => {
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
        if is_card_move_for(*action, card_id) {
            break;
        }
        match action {
            InternalAction::DealDamage {
                info:
                    DamageInfo {
                        source: DamageSource::Card(source),
                        amount,
                        ..
                    },
            } if *source == card_id => {
                *amount = pen_nib_queue_amount(state, *amount);
            }
            InternalAction::DealDamageRandomEnemy { source, amount } if *source == card_id => {
                *amount = pen_nib_queue_amount(state, *amount);
            }
            InternalAction::DealDamageAll { source, amount } if *source == card_id => {
                *amount = pen_nib_queue_amount(state, *amount);
            }
            InternalAction::DealDamageAllAndHealUnblocked { source, amount }
                if *source == card_id =>
            {
                *amount = pen_nib_queue_amount(state, *amount);
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
                *amount = pen_nib_queue_amount(state, *amount);
            }
            _ => {}
        }
    }
}

fn pen_nib_queue_amount(state: &CombatState, amount: i32) -> i32 {
    let strength = state.player.powers.strength + state.player.temp_strength;
    (amount + strength).max(0) * 2 - strength
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

    let mut delayed_prevention = VecDeque::new();
    let mut immediate_queue = VecDeque::new();
    while let Some(action) = queue.pop_front() {
        if matches!(action, InternalAction::PreventBlockGain { .. }) {
            delayed_prevention.push_back(action);
        } else {
            immediate_queue.push_back(action);
        }
    }
    queue = immediate_queue;

    queue.push_front(InternalAction::ConsumeDuplicationPotion);
    queue.append(&mut delayed_prevention);
    if let Some(action) = final_move {
        queue.push_back(action);
    }
    append_copied_card_effects(&mut queue, card_id, &mut duplicated_effects);

    queue
}

fn apply_double_tap_to_queue(
    mut queue: VecDeque<InternalAction>,
    card_id: CardId,
) -> VecDeque<InternalAction> {
    let mut duplicated_effects = VecDeque::new();
    duplicated_effects.extend(
        queue
            .iter()
            .copied()
            .filter(|action| is_duplicated_card_effect(*action, card_id)),
    );

    let final_move = queue
        .back()
        .copied()
        .filter(|action| is_card_move_for(*action, card_id));
    if final_move.is_some() {
        queue.pop_back();
    }

    queue.push_front(InternalAction::ConsumeDoubleTap);
    if let Some(action) = final_move {
        queue.push_back(action);
    }
    append_copied_card_effects(&mut queue, card_id, &mut duplicated_effects);

    queue
}

fn apply_necronomicon_to_queue(
    mut queue: VecDeque<InternalAction>,
    card_id: CardId,
) -> VecDeque<InternalAction> {
    let mut duplicated_effects = VecDeque::new();
    duplicated_effects.extend(
        queue
            .iter()
            .copied()
            .filter(|action| is_duplicated_card_effect(*action, card_id)),
    );

    let final_move = queue
        .back()
        .copied()
        .filter(|action| is_card_move_for(*action, card_id));
    if final_move.is_some() {
        queue.pop_back();
    }

    queue.push_front(InternalAction::ConsumeNecronomicon);
    if let Some(action) = final_move {
        queue.push_back(action);
    }
    append_copied_card_effects(&mut queue, card_id, &mut duplicated_effects);

    queue
}

fn append_copied_card_effects(
    queue: &mut VecDeque<InternalAction>,
    card_id: CardId,
    duplicated_effects: &mut VecDeque<InternalAction>,
) {
    let required_target = copied_card_required_living_target(duplicated_effects);
    if let Some(target) = required_target {
        queue.push_back(InternalAction::SkipCopiedCardEffectsIfTargetDead { target });
    }
    queue.push_back(InternalAction::SkipCopiedCardEffectsIfCombatDone);
    queue.push_back(InternalAction::PlayCardCopy { card_id });
    queue.append(duplicated_effects);
    queue.push_back(InternalAction::EndCopiedCardEffects);
}

fn copied_card_required_living_target(effects: &VecDeque<InternalAction>) -> Option<MonsterId> {
    let mut target = None;
    for action in effects {
        let Some(next) = action_required_living_target(*action) else {
            continue;
        };
        if target.is_some_and(|existing| existing != next) {
            return None;
        }
        target = Some(next);
    }
    target
}

fn action_required_living_target(action: InternalAction) -> Option<MonsterId> {
    match action {
        InternalAction::DealDamage { info }
        | InternalAction::DealDamageAndHealUnblocked { info }
        | InternalAction::DealFeedDamage { info, .. } => Some(info.target),
        InternalAction::GainMonsterBlock { target, .. }
        | InternalAction::ApplyVulnerable { target, .. }
        | InternalAction::ReduceMonsterStrength { target, .. }
        | InternalAction::ReduceMonsterStrengthThisTurn { target, .. }
        | InternalAction::DealUnmodifiedDamage { target, .. }
        | InternalAction::ApplyWeak { target, .. } => Some(target),
        _ => None,
    }
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
            | InternalAction::PreventBlockGain { .. }
            | InternalAction::MoveCard { .. }
            | InternalAction::AwaitHandSelect { .. }
            | InternalAction::AwaitDrawSelect { .. }
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
        InternalAction::SpendCardEnergy { card_id },
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

fn hand_of_greed_queue(
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DealHandOfGreedDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: definition.values.damage.unwrap_or(0),
            },
            gold: definition.values.vulnerable.unwrap_or(0),
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn ritual_dagger_queue(
    card: &CardInstance,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let amount = ritual_dagger_card_damage(card)?.ok_or(SimError::InvalidState(
        "Ritual Dagger queue received a different card",
    ))?;
    let growth = ritual_dagger_card_growth(card).ok_or(SimError::InvalidState(
        "Ritual Dagger queue received a different card",
    ))?;
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id: card.id },
        InternalAction::SpendCardEnergy { card_id: card.id },
        InternalAction::DealRitualDaggerDamage {
            info: DamageInfo {
                source: DamageSource::Card(card.id),
                target,
                amount,
            },
            growth,
        },
        InternalAction::MoveCard {
            card_id: card.id,
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
        InternalAction::GainBlock {
            amount: definition.values.block.unwrap_or(0),
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

    if definition.id == ARMAMENTS_PLUS_ID {
        queue.push_back(InternalAction::UpgradeHandCardsExcept { card_id });
        queue.push_back(InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        });
        return Ok(queue);
    }

    match upgradeable_other_hand_card_ids(state, card_id).as_slice() {
        [] => {
            queue.push_back(InternalAction::MoveCard {
                card_id,
                from: CardPile::Hand,
                to: CardPile::DiscardPile,
            });
        }
        [target] => {
            queue.push_back(InternalAction::UpgradeHandCard { card_id: *target });
            queue.push_back(InternalAction::MoveCard {
                card_id,
                from: CardPile::Hand,
                to: card_move_destination(definition),
            });
        }
        _ => {
            queue.push_back(InternalAction::AwaitHandSelect {
                source_card_id: card_id,
                purpose: HandSelectPurpose::ArmamentsUpgrade,
            });
        }
    }

    Ok(queue)
}

fn apotheosis_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::UpgradeCombatCards,
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
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

fn upgradeable_other_hand_card_ids(state: &CombatState, exclude_id: CardId) -> Vec<CardId> {
    state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != exclude_id && card_instance_is_upgradeable(card))
        .map(|card| card.id)
        .collect()
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
        InternalAction::GainEnergy {
            amount: if definition.id == BLOODLETTING_PLUS_ID {
                3
            } else {
                2
            },
        },
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
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: definition.values.damage.unwrap_or(0),
            },
        },
        InternalAction::LoseHp {
            amount: 2,
            source: HpLossSource::Card(card_id),
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
            max_hp_gain: if definition.id == FEED_PLUS_ID { 4 } else { 3 },
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
    let strength_multiplier = if definition.id == HEAVY_BLADE_PLUS_ID {
        5
    } else {
        3
    };
    let extra_strength =
        (strength_multiplier - 1) * (state.player.powers.strength + state.player.temp_strength);
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
    let strike_bonus = if definition.id == PERFECTED_STRIKE_PLUS_ID {
        3
    } else {
        2
    };
    let base_damage = definition.values.damage.unwrap_or(0) + (strike_bonus * strike_count);
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

pub(super) fn combat_strike_named_card_count(state: &CombatState) -> usize {
    state
        .piles
        .hand
        .iter()
        .chain(state.piles.draw_pile.iter())
        .chain(state.piles.discard_pile.iter())
        .filter(|card| {
            get_card_definition(card.content_id)
                .map(is_strike_named_definition)
                .unwrap_or(false)
        })
        .count()
}

fn is_strike_named_definition(definition: &CardDefinition) -> bool {
    definition.key.contains("STRIKE")
        || definition.key.contains("Strike")
        || definition.name.contains("Strike")
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
        InternalAction::AddGeneratedCardToDrawPileRandomSpot {
            content_id: WOUND_ID,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn bite_queue(
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
        InternalAction::HealPlayer { amount: 2 },
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
    let card = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .ok_or(SimError::IllegalAction("card is not in hand"))?;
    if card.content_id != RAMPAGE_ID && card.content_id != RAMPAGE_PLUS_ID {
        return Err(SimError::InvalidState(
            "Rampage queue received a different card",
        ));
    }
    let base_damage = definition.values.damage.ok_or(SimError::InvalidState(
        "Rampage definition is missing damage",
    ))?;
    let damage = base_damage
        .checked_add(card.rampage_damage_bonus)
        .ok_or(SimError::InvalidState("Rampage damage overflows i32"))?;

    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: damage,
            },
        },
        InternalAction::IncreaseRampageDamage {
            card_id,
            amount: if definition.id == RAMPAGE_PLUS_ID {
                8
            } else {
                5
            },
        },
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
        InternalAction::AddGeneratedCardToPile {
            content_id: WOUND_ID,
            to: CardPile::Hand,
            temp_cost: None,
            temp_cost_turn_only: false,
        },
        InternalAction::AddGeneratedCardToPile {
            content_id: WOUND_ID,
            to: CardPile::Hand,
            temp_cost: None,
            temp_cost_turn_only: false,
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
            temp_cost_turn_only: true,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

pub(crate) fn infernal_blade_generated_attack(state: &mut CombatState) -> ContentId {
    let pool = infernal_blade_modeled_attack_pool();
    let rng = &mut state.rng.card_random_rng;
    let index = rng.random_int((pool.len() - 1) as i32) as usize;
    pool[index]
}

fn metamorphosis_queue(
    state: &mut CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);

    let generated_count = if definition.id == METAMORPHOSIS_PLUS_ID {
        5
    } else {
        3
    };
    for generated in metamorphosis_generated_attacks(state, generated_count) {
        queue.push_back(
            InternalAction::AddGeneratedCardToDrawPileRandomSpotWithCost {
                content_id: generated,
                temp_cost: generated_card_zero_cost_if_positive(generated),
                temp_cost_turn_only: false,
            },
        );
    }

    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });

    Ok(queue)
}

fn metamorphosis_generated_attacks(state: &mut CombatState, count: usize) -> Vec<ContentId> {
    let pool = infernal_blade_modeled_attack_pool();
    let mut generated = vec![pool[0]; count];
    let rng = &mut state.rng.card_random_rng;

    for content_id in &mut generated {
        let index = rng.random_int((pool.len() - 1) as i32) as usize;
        *content_id = pool[index];
    }
    generated
}

fn discovery_queue(
    state: &mut CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    open_discovery_card_reward(state, card_id)?;
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::OpenDiscoveryCardReward {
            source_card_id: card_id,
        },
    ]))
}

#[allow(clippy::reversed_empty_ranges)]
fn open_discovery_card_reward(state: &mut CombatState, _source_card_id: CardId) -> SimResult<()> {
    let next_card_id = state.reserve_card_instance_ids(3)?;
    let pool = discovery_modeled_card_pool();
    let rng = &mut state.rng.card_random_rng;
    let content_choices = discovery_choices_from_pool(rng, &pool);
    // Target DiscoveryAction.generate*Choices runs at the top of every update(),
    // before checking whether the reward screen is already open. Fast-mode actions
    // therefore burn extra invisible choice generations after the visible choices.
    for _ in 0..DISCOVERY_ACTION_HIDDEN_GENERATIONS {
        let _ = discovery_choices_from_pool(rng, &pool);
    }
    // The live CommunicationMod/SuperFastMode verifier environment consistently advances
    // one more card-random draw while the card reward screen settles before control
    // returns to the next combat action. Keep this as a named generic DiscoveryAction
    // timing draw rather than folding it into the full hidden-generation count.
    for _ in 0..DISCOVERY_ACTION_SCREEN_SETTLE_DRAWS {
        let _ = rng.random_int((pool.len() - 1) as i32);
    }

    state.decision = Some(CombatDecisionState::DiscoveryCardReward {
        choices: content_choices
            .into_iter()
            .enumerate()
            .map(|(index, content_id)| {
                CardInstance::new(CardId::new(next_card_id + index as u64), content_id)
            })
            .collect(),
        source_card: None,
    });
    Ok(())
}

fn discovery_choices_from_pool(rng: &mut crate::rng::StsRng, pool: &[ContentId]) -> Vec<ContentId> {
    let mut choices = Vec::with_capacity(3);
    while choices.len() < 3 {
        let index = rng.random_int((pool.len() - 1) as i32) as usize;
        let content_id = pool[index];
        if !choices.contains(&content_id) {
            choices.push(content_id);
        }
    }
    choices
}

pub(crate) fn discovery_modeled_card_pool() -> Vec<ContentId> {
    ironclad_combat_discovery_pool()
        .iter()
        .copied()
        .filter(|content_id| get_card_definition(*content_id).is_some())
        .collect()
}

pub(crate) fn infernal_blade_modeled_attack_pool() -> Vec<ContentId> {
    // RNG must use the complete target source pool even when a generated card's
    // mechanics are not modeled yet. Filtering changes the random bound and can
    // select a different card among otherwise supported entries.
    ironclad_combat_attack_discovery_pool()
}

fn chrysalis_queue(
    state: &mut CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let generated_count = if definition.id == CHRYSALIS_PLUS_ID {
        5
    } else {
        3
    };
    let generated = chrysalis_generated_skills(state, generated_count);
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);
    for content_id in generated {
        queue.push_back(
            InternalAction::AddGeneratedCardToDrawPileRandomSpotWithCost {
                content_id,
                temp_cost: generated_card_zero_cost_if_positive(content_id),
                temp_cost_turn_only: false,
            },
        );
    }
    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });
    Ok(queue)
}

pub(crate) fn chrysalis_generated_skills(state: &mut CombatState, count: usize) -> Vec<ContentId> {
    let pool = chrysalis_modeled_skill_pool();
    let rng = &mut state.rng.card_random_rng;

    (0..count)
        .map(|_| {
            let index = rng.random_int((pool.len() - 1) as i32) as usize;
            pool[index]
        })
        .collect()
}

pub(crate) fn chrysalis_modeled_skill_pool() -> Vec<ContentId> {
    ironclad_combat_skill_discovery_pool()
        .into_iter()
        .filter(|content_id| {
            get_card_definition(*content_id)
                .is_some_and(|definition| definition.card_type == CardType::Skill)
        })
        .collect()
}

fn generated_card_zero_cost_if_positive(content_id: ContentId) -> Option<u8> {
    get_card_definition(content_id).and_then(|definition| (definition.cost > 0).then_some(0))
}

fn jack_of_all_trades_queue(
    state: &mut CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let generated_count = if definition.id == JACK_OF_ALL_TRADES_PLUS_ID {
        2
    } else {
        1
    };
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);
    for _ in 0..generated_count {
        let generated = jack_of_all_trades_generated_colorless(state);
        queue.push_back(InternalAction::AddGeneratedCardToPile {
            content_id: generated,
            to: CardPile::Hand,
            temp_cost: None,
            temp_cost_turn_only: false,
        });
    }
    queue.extend([InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    }]);
    Ok(queue)
}

fn jack_of_all_trades_generated_colorless(state: &mut CombatState) -> ContentId {
    let pool = colorless_discovery_pool();
    let rng = &mut state.rng.card_random_rng;
    let index = rng.random_int((pool.len() - 1) as i32) as usize;
    pool[index]
}

fn madness_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
        InternalAction::SetRandomHandCardCostForCombat { amount: 0 },
    ]))
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
        InternalAction::HealPlayer {
            amount: if definition.id == BANDAGE_UP_PLUS_ID {
                6
            } else {
                4
            },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn violence_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let count = if definition.id == VIOLENCE_PLUS_ID {
        4
    } else {
        3
    };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DrawRandomAttacksFromDrawPile { count },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn apparition_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainIntangible { amount: 1 },
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
    let amount = if definition.id == PANACEA_PLUS_ID {
        2
    } else {
        1
    };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainArtifact { amount },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn forethought_queue(
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

    let other_hand_cards = state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != card_id)
        .count();
    if other_hand_cards == 0 {
        queue.push_back(InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        });
    } else if definition.id == FORETHOUGHT_ID && other_hand_cards == 1 {
        let target_card_id = lowest_other_hand_card(state, card_id)
            .expect("counted one other hand card for base Forethought");
        queue.push_back(InternalAction::ForethoughtAutoMove {
            source_card_id: card_id,
            card_id: target_card_id,
        });
    } else {
        let purpose = if definition.id == FORETHOUGHT_PLUS_ID {
            HandSelectPurpose::ForethoughtPutAnyOnDraw
        } else {
            HandSelectPurpose::ForethoughtPutOnDraw
        };
        queue.push_back(InternalAction::AwaitHandSelect {
            source_card_id: card_id,
            purpose,
        });
    }

    Ok(queue)
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
        InternalAction::AddGeneratedCardToDrawPileRandomSpot {
            content_id: DAZED_ID,
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
    let hits = if definition.id == PUMMEL_PLUS_ID {
        5
    } else {
        4
    };
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);

    for _ in 0..hits {
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
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: definition.values.damage.unwrap_or(0),
            },
        },
        InternalAction::ApplyWeak {
            target,
            amount: if definition.id == CLOTHESLINE_PLUS_ID {
                3
            } else {
                2
            },
        },
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
            amount: if definition.id == INTIMIDATE_PLUS_ID {
                2
            } else {
                1
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

fn blind_queue(
    state: &CombatState,
    card_id: CardId,
    target: Option<MonsterId>,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);

    if definition.target == TargetRequirement::Enemy {
        queue.push_back(InternalAction::ApplyWeak {
            target: target.expect("validated Blind has a target"),
            amount: 2,
        });
    } else {
        for monster in state.monsters.iter().filter(|monster| monster.alive) {
            queue.push_back(InternalAction::ApplyWeak {
                target: monster.id,
                amount: 2,
            });
        }
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
    target: Option<MonsterId>,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);

    if definition.target == TargetRequirement::Enemy {
        queue.push_back(InternalAction::ApplyVulnerable {
            target: target.expect("validated Trip has a target"),
            amount: definition.values.vulnerable.unwrap_or(0),
        });
    } else {
        for monster in state.monsters.iter().filter(|monster| monster.alive) {
            queue.push_back(InternalAction::ApplyVulnerable {
                target: monster.id,
                amount: definition.values.vulnerable.unwrap_or(0),
            });
        }
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
        let amount = definition.values.vulnerable.unwrap_or(0);
        queue.push_back(InternalAction::ApplyWeak {
            target: monster.id,
            amount,
        });
        queue.push_back(InternalAction::ApplyVulnerable {
            target: monster.id,
            amount,
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
        InternalAction::ReduceMonsterStrength {
            target,
            amount: if definition.id == DISARM_PLUS_ID {
                3
            } else {
                2
            },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn dark_shackles_queue(
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::ReduceMonsterStrengthThisTurn {
            target,
            amount: if definition.id == DARK_SHACKLES_PLUS_ID {
                15
            } else {
                9
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
    state
        .monsters
        .iter()
        .any(|monster| monster.alive)
        .then_some(())
        .ok_or(SimError::InvalidState(
            "Sword Boomerang requires a living monster",
        ))?;
    let damage = definition.values.damage.unwrap_or(0);

    let hits = if definition.id == SWORD_BOOMERANG_PLUS_ID {
        4
    } else {
        3
    };
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);

    for _ in 0..hits {
        queue.push_back(InternalAction::DealDamageRandomEnemy {
            source: card_id,
            amount: damage,
        });
    }

    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });

    Ok(queue)
}

fn defend_queue(
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
        InternalAction::GainRage {
            amount: if definition.id == RAGE_PLUS_ID { 5 } else { 3 },
        },
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
        InternalAction::GainDoubleTap {
            amount: if definition.id == DOUBLE_TAP_PLUS_ID {
                2
            } else {
                1
            },
        },
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

fn evolve_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainEvolve {
            amount: if definition.id == EVOLVE_PLUS_ID {
                2
            } else {
                1
            },
        },
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

fn rupture_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainRupture {
            amount: if definition.id == RUPTURE_PLUS_ID {
                2
            } else {
                1
            },
        },
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

fn mayhem_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainMayhem { amount: 1 },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn magnetism_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainMagnetism { amount: 1 },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

pub(crate) fn magnetism_generated_colorless_card(state: &mut CombatState) -> ContentId {
    let pool = magnetism_modeled_colorless_pool();
    let rng = &mut state.rng.card_random_rng;
    let index = rng.random_int((pool.len() - 1) as i32) as usize;
    pool[index]
}

pub(crate) fn magnetism_modeled_colorless_pool() -> Vec<ContentId> {
    colorless_discovery_pool()
        .into_iter()
        .filter(|content_id| get_card_definition(*content_id).is_some())
        .collect()
}

fn panache_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainPanache {
            amount: definition.values.damage.unwrap_or(0),
        },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn sadistic_nature_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainSadisticNature {
            amount: definition.values.damage.unwrap_or(0),
        },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn exhume_queue(state: &CombatState, card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    let exhumable_cards = exhumable_card_ids(state);
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
    ]);

    match exhumable_cards.as_slice() {
        [] => queue.push_back(InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        }),
        [exhumed_card_id] => {
            queue.push_back(InternalAction::ReturnExhaustCardToHand {
                card_id: *exhumed_card_id,
            });
            queue.push_back(InternalAction::MoveCard {
                card_id,
                from: CardPile::Hand,
                to: CardPile::ExhaustPile,
            });
        }
        _ => queue.push_back(InternalAction::AwaitExhaustSelect {
            source_card_id: card_id,
            purpose: crate::combat::ExhaustSelectPurpose::ExhumeReturnToHand,
        }),
    }

    Ok(queue)
}

fn purity_queue(state: &CombatState, card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
    ]);
    if state.piles.hand.iter().any(|card| card.id != card_id) {
        queue.push_back(InternalAction::AwaitExhaustSelect {
            source_card_id: card_id,
            purpose: crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3,
        });
    } else {
        queue.push_back(InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        });
    }
    Ok(queue)
}

fn exhumable_card_ids(state: &CombatState) -> Vec<CardId> {
    state
        .piles
        .exhaust_pile
        .iter()
        .filter(|card| card.content_id != EXHUME_ID && card.content_id != EXHUME_PLUS_ID)
        .map(|card| card.id)
        .collect()
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
    let exhaust_count = exhaust_targets.len();
    let block_per_card = definition.values.block.unwrap_or(0);
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
    if block_per_card > 0 {
        for _ in 0..exhaust_count {
            queue.push_back(InternalAction::GainBlock {
                amount: block_per_card,
            });
        }
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
    let exhaust_count = other_hand_cards(state, card_id).len();
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);
    for _ in 0..exhaust_count {
        queue.push_back(InternalAction::ExhaustRandomHandCardExcept {
            excluded_card_id: card_id,
        });
    }
    for _ in 0..exhaust_count {
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

fn slimed_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 1 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        },
    ]))
}

fn bash_queue(
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 2 },
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
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn anger_queue(
    card: CardInstance,
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
        InternalAction::AddStatEquivalentCopyToPile {
            card,
            to: CardPile::DiscardPile,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
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
        InternalAction::SpendCardEnergy { card_id },
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
    queue.push_back(InternalAction::AddGeneratedCardToPile {
        content_id: crate::content::cards::BURN_ID,
        to: CardPile::DiscardPile,
        temp_cost: None,
        temp_cost_turn_only: false,
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
        InternalAction::ApplyWeak {
            target,
            amount: if definition.id == UPPERCUT_PLUS_ID {
                2
            } else {
                1
            },
        },
        InternalAction::ApplyVulnerable {
            target,
            amount: definition.values.vulnerable.unwrap_or(0),
        },
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

fn x_cost_uses_with_chemical_x(state: &CombatState) -> i32 {
    let chemical_x_bonus = if state.relics.contains(&Relic::ChemicalX) {
        CHEMICAL_X_BONUS_X
    } else {
        0
    };
    state.player.energy + chemical_x_bonus
}

fn transmutation_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let x = state.player.energy;
    let uses = x_cost_uses_with_chemical_x(state);
    if uses < 1 {
        return Err(SimError::IllegalAction(
            "Transmutation requires at least 1 energy",
        ));
    }

    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: x },
    ]);

    for _ in 0..uses {
        queue.push_back(InternalAction::AddRandomColorlessCardToHand {
            temp_cost: Some(0),
            upgrade: definition.id == TRANSMUTATION_PLUS_ID,
        });
    }

    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: CardPile::ExhaustPile,
    });

    Ok(queue)
}

fn havoc_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
    target: Option<MonsterId>,
) -> SimResult<VecDeque<InternalAction>> {
    if let Some(top_definition) = top_draw_card_definition(state) {
        validate_havoc_target(top_definition, target, true)?;
    } else if state.piles.discard_pile.is_empty() && target.is_some() {
        return Err(SimError::IllegalAction(
            "Havoc top card cannot have a target",
        ));
    }

    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::PlayTopDrawCard {
            target,
            exhaust_played_card: true,
            random_living_target: true,
        },
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
    _state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
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

fn thinking_ahead_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DrawCards { count: 2 },
    ]);

    if lowest_other_hand_card(state, card_id).is_some() {
        queue.push_back(InternalAction::AwaitHandSelect {
            source_card_id: card_id,
            purpose: HandSelectPurpose::ThinkingAheadPutOnDraw,
        });
    } else {
        queue.push_back(InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: if definition.keywords.exhaust {
                CardPile::ExhaustPile
            } else {
                CardPile::DiscardPile
            },
        });
    }

    Ok(queue)
}

fn draw_pile_has_skill(state: &CombatState) -> bool {
    state.piles.draw_pile.iter().any(|card| {
        get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == CardType::Skill)
    })
}

fn draw_pile_has_attack(state: &CombatState) -> bool {
    state.piles.draw_pile.iter().any(|card| {
        get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == CardType::Attack)
    })
}

fn secret_technique_queue(
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

    if draw_pile_has_skill(state) {
        queue.push_back(InternalAction::AwaitDrawSelect {
            source_card_id: card_id,
            purpose: crate::combat::DrawSelectPurpose::SecretTechniqueSkillToHand,
        });
    } else {
        queue.push_back(InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        });
    }

    Ok(queue)
}

fn secret_weapon_queue(
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

    if draw_pile_has_attack(state) {
        queue.push_back(InternalAction::AwaitDrawSelect {
            source_card_id: card_id,
            purpose: crate::combat::DrawSelectPurpose::SecretWeaponAttackToHand,
        });
    } else {
        queue.push_back(InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        });
    }

    Ok(queue)
}

fn has_attack_or_power_in_hand(state: &CombatState, exclude_id: CardId) -> bool {
    state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != exclude_id)
        .any(|card| {
            get_card_definition(card.content_id).is_some_and(|definition| {
                definition.card_type == CardType::Attack || definition.card_type == CardType::Power
            })
        })
}

fn dual_wield_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    if !has_attack_or_power_in_hand(state, card_id) {
        return Err(SimError::IllegalAction(
            "Dual Wield requires an attack or power",
        ));
    }

    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::AwaitHandSelect {
            source_card_id: card_id,
            purpose: HandSelectPurpose::DualWieldCopy,
        },
    ]))
}

fn searing_blow_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let card = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .ok_or(SimError::IllegalAction("card is not in hand"))?;
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: searing_blow_card_damage(card)?.ok_or(SimError::InvalidState(
                    "Searing Blow queue received a different card",
                ))?,
            },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

#[must_use]
pub fn top_draw_card_definition(state: &CombatState) -> Option<&'static CardDefinition> {
    state
        .piles
        .draw_pile
        .last()
        .and_then(|card| get_card_definition(card.content_id))
}

pub(super) fn validate_havoc_target(
    top_definition: &CardDefinition,
    target: Option<MonsterId>,
    allow_random_enemy_target: bool,
) -> SimResult<()> {
    match top_definition.target {
        TargetRequirement::Enemy if target.is_some() => Ok(()),
        TargetRequirement::Enemy if allow_random_enemy_target => Ok(()),
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

fn shrug_it_off_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainBlock {
            amount: definition.values.block.unwrap_or(0),
        },
        InternalAction::DrawCardsWhilePlayedCardIsInLimbo { card_id, count: 1 },
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

fn the_bomb_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::ArmTheBomb {
            turns: THE_BOMB_TURNS,
            damage: definition.values.damage.unwrap_or(THE_BOMB_DAMAGE),
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn deep_breath_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let count = if definition.id == DEEP_BREATH_PLUS_ID {
        2
    } else {
        1
    };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DeepBreathShuffleDiscardIntoDraw,
        InternalAction::DrawCards { count },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn impatience_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DrawCardsIfNoAttacksInHand {
            count: if definition.id == IMPATIENCE_PLUS_ID {
                3
            } else {
                2
            },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn panic_button_queue(
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
        InternalAction::PreventBlockGain { turns: 2 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn enlightenment_queue(
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

    queue.extend(enlightenment_cost_actions(
        state,
        card_id,
        definition.id == ENLIGHTENMENT_PLUS_ID,
    ));
    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: CardPile::DiscardPile,
    });

    Ok(queue)
}

pub(super) fn enlightenment_cost_actions(
    state: &CombatState,
    exclude_id: CardId,
    combat_long: bool,
) -> Vec<InternalAction> {
    state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != exclude_id)
        .filter(|card| hand_card_cost_before_enlightenment(card) > 1)
        .map(|card| {
            if combat_long {
                InternalAction::SetHandCardCostForCombat {
                    card_id: card.id,
                    cost: 1,
                }
            } else {
                InternalAction::SetHandCardCostForTurn {
                    card_id: card.id,
                    cost: 1,
                }
            }
        })
        .collect()
}

fn hand_card_cost_before_enlightenment(card: &crate::CardInstance) -> i32 {
    card.temp_cost.map_or_else(
        || {
            get_card_definition(card.content_id)
                .map(|definition| i32::from(definition.cost))
                .unwrap_or(0)
        },
        i32::from,
    )
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
        InternalAction::GainTemporaryThorns {
            amount: flame_barrier_thorns_amount(definition),
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

pub(crate) fn flame_barrier_thorns_amount(definition: &CardDefinition) -> i32 {
    if definition.id == FLAME_BARRIER_PLUS_ID {
        6
    } else {
        4
    }
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

fn true_grit_queue(
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

    if state.piles.hand.iter().any(|card| card.id != card_id) {
        if definition.id == TRUE_GRIT_PLUS_ID {
            queue.push_back(InternalAction::AwaitExhaustSelect {
                source_card_id: card_id,
                purpose: crate::combat::ExhaustSelectPurpose::TrueGritExhaustOne,
            });
            return Ok(queue);
        } else {
            queue.push_back(InternalAction::ExhaustRandomHandCardExcept {
                excluded_card_id: card_id,
            });
        }
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

    let draw_count = if state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.content_id)
        == Some(BURNING_PACT_PLUS_ID)
    {
        3
    } else {
        2
    };

    if lowest_other_hand_card(state, card_id).is_none() {
        queue.push_back(InternalAction::DrawCards { count: draw_count });
        queue.push_back(InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        });
        return Ok(queue);
    }

    queue.push_back(InternalAction::AwaitExhaustSelect {
        source_card_id: card_id,
        purpose: if draw_count == 3 {
            crate::combat::ExhaustSelectPurpose::BurningPactDraw3
        } else {
            crate::combat::ExhaustSelectPurpose::BurningPactDraw2
        },
    });

    Ok(queue)
}

fn feel_no_pain_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainFeelNoPain {
            amount: if definition.id == FEEL_NO_PAIN_PLUS_ID {
                4
            } else {
                3
            },
        },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn dark_embrace_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainDarkEmbrace { amount: 1 },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn combust_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainCombust {
            amount: definition.values.damage.unwrap_or(0),
        },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn demon_form_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainRitual {
            amount: demon_form_strength_gain(definition),
        },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn demon_form_strength_gain(definition: &CardDefinition) -> i32 {
    if definition.id == DEMON_FORM_PLUS_ID {
        3
    } else {
        2
    }
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
    let draw_count = if definition.id == POMMEL_STRIKE_PLUS_ID {
        2
    } else {
        1
    };
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
        InternalAction::DrawCards { count: draw_count },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn mind_blast_queue(
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
                amount: i32::try_from(state.piles.draw_pile.len())
                    .expect("draw pile count fits in i32"),
            },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn master_of_strategy_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DrawCards {
            count: if definition.id == MASTER_OF_STRATEGY_PLUS_ID {
                4
            } else {
                3
            },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn battle_trance_draw_count(definition: &CardDefinition) -> usize {
    if definition.id == BATTLE_TRANCE_PLUS_ID {
        4
    } else {
        3
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
        InternalAction::DrawCardsWhilePlayedCardIsInLimbo {
            card_id,
            count: battle_trance_draw_count(definition),
        },
        InternalAction::SetCannotDraw,
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
            to: CardPile::ExhaustPile,
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

pub(crate) fn flex_temp_strength_amount(definition: &CardDefinition) -> i32 {
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

fn jax_queue(card_id: CardId, definition: &CardDefinition) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::LoseHp {
            amount: definition.values.damage.expect("J.A.X. HP loss"),
            source: HpLossSource::Card(card_id),
        },
        InternalAction::GainStrength {
            amount: definition.values.vulnerable.expect("J.A.X. Strength"),
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
            amount: state.player.powers.strength + state.player.temp_strength,
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
    let draw_count = if definition.id == OFFERING_PLUS_ID {
        5
    } else {
        3
    };
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
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
        InternalAction::DrawCards { count: draw_count },
    ]))
}

fn spot_weakness_strength_amount(definition: &CardDefinition) -> i32 {
    if definition.id == SPOT_WEAKNESS_PLUS_ID {
        4
    } else {
        3
    }
}

fn spot_weakness_queue(
    state: &CombatState,
    card_id: CardId,
    target: Option<MonsterId>,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);

    if target.is_some_and(|target| monster_intends_attack(state, target)) {
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

fn monster_intends_attack(state: &CombatState, target: MonsterId) -> bool {
    state
        .monsters
        .iter()
        .find(|monster| monster.id == target && monster.alive)
        .is_some_and(|monster| {
            matches!(
                monster.intent,
                MonsterIntent::Attack { .. }
                    | MonsterIntent::AttackAndBlock { .. }
                    | MonsterIntent::AttackApplyPlayerWeak { .. }
                    | MonsterIntent::AttackApplyPlayerFrail { .. }
                    | MonsterIntent::AttackApplyPlayerVulnerable { .. }
                    | MonsterIntent::AttackApplyPlayerWeakAndVulnerable { .. }
                    | MonsterIntent::AttackApplyPlayerFrailAndWeak { .. }
                    | MonsterIntent::AttackHealSelf { .. }
                    | MonsterIntent::AttackAddWoundsToDiscard { .. }
                    | MonsterIntent::AttackAddSlimedToDiscard { .. }
                    | MonsterIntent::AttackMultiple { .. }
                    | MonsterIntent::AttackStealGold { .. }
            )
        })
}
