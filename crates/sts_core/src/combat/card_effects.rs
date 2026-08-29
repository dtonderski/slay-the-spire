use crate::{
    action::{CardPile, HpLossSource, InternalAction},
    card::{CardDefinition, CardType, TargetRequirement},
    combat::{
        cost::effective_card_cost,
        damage::{DamageInfo, DamageSource},
        draw::MAX_HAND_SIZE,
        CombatDecisionState, CombatState, HandSelectPurpose,
    },
    content::cards::{
        card_instance_is_upgradeable, get_card_definition, is_curse_content_id,
        ritual_dagger_card_damage, ritual_dagger_card_growth, searing_blow_card_damage, ANGER_ID,
        ANGER_PLUS_ID, APOTHEOSIS_ID, APOTHEOSIS_PLUS_ID, APPARITION_ID, APPARITION_PLUS_ID,
        ARMAMENTS_ID, ARMAMENTS_PLUS_ID, BACKFLIP_ANY_COLOR_ID, BANDAGE_UP_ID, BANDAGE_UP_PLUS_ID,
        BARRICADE_ID, BARRICADE_PLUS_ID, BASH_ID, BASH_PLUS_ID, BATTLE_TRANCE_ID,
        BATTLE_TRANCE_PLUS_ID, BEAM_CELL_ANY_COLOR_ID, BERSERK_ID, BERSERK_PLUS_ID,
        BIASED_COGNITION_ANY_COLOR_ID, BITE_ID, BITE_PLUS_ID, BLASPHEMY_ID, BLASPHEMY_PLUS_ID,
        BLIND_ID, BLIND_PLUS_ID, BLOODLETTING_ID, BLOODLETTING_PLUS_ID, BLOOD_FOR_BLOOD_ID,
        BLOOD_FOR_BLOOD_PLUS_ID, BODY_SLAM_ID, BODY_SLAM_PLUS_ID, BOWLING_BASH_ANY_COLOR_ID,
        BRUTALITY_ID, BRUTALITY_PLUS_ID, BURNING_PACT_ID, BURNING_PACT_PLUS_ID,
        CHARGE_BATTERY_ANY_COLOR_ID, CHRYSALIS_ID, CHRYSALIS_PLUS_ID, CLASH_ID, CLASH_PLUS_ID,
        CLEAVE_ID, CLEAVE_PLUS_ID, CLOAK_AND_DAGGER_ANY_COLOR_ID, CLOTHESLINE_ID,
        CLOTHESLINE_PLUS_ID, COMBUST_ID, COMBUST_PLUS_ID, COMPILE_DRIVER_ANY_COLOR_ID,
        CORRUPTION_ID, CORRUPTION_PLUS_ID, DAGGER_SPRAY_ANY_COLOR_ID, DARK_EMBRACE_ID,
        DARK_EMBRACE_PLUS_ID, DARK_SHACKLES_ID, DARK_SHACKLES_PLUS_ID, DAZED_ID, DEEP_BREATH_ID,
        DEEP_BREATH_PLUS_ID, DEFEND_R_ID, DEFEND_R_PLUS_ID, DEMON_FORM_ID, DEMON_FORM_PLUS_ID,
        DISARM_ID, DISARM_PLUS_ID, DISCOVERY_ID, DISCOVERY_PLUS_ID, DOPPELGANGER_ANY_COLOR_ID,
        DOUBLE_TAP_ID, DOUBLE_TAP_PLUS_ID, DRAMATIC_ENTRANCE_ID, DRAMATIC_ENTRANCE_PLUS_ID,
        DROPKICK_ID, DROPKICK_PLUS_ID, DUAL_WIELD_ID, DUAL_WIELD_PLUS_ID, EMPTY_MIND_ANY_COLOR_ID,
        ENLIGHTENMENT_ID, ENLIGHTENMENT_PLUS_ID, ENTRENCH_ID, ENTRENCH_PLUS_ID,
        EQUILIBRIUM_ANY_COLOR_ID, EVISCERATE_ANY_COLOR_ID, EVOLVE_ID, EVOLVE_PLUS_ID, EXHUME_ID,
        EXHUME_PLUS_ID, FASTING_ANY_COLOR_ID, FEED_ID, FEED_PLUS_ID, FEEL_NO_PAIN_ID,
        FEEL_NO_PAIN_PLUS_ID, FIEND_FIRE_ID, FIEND_FIRE_PLUS_ID, FINESSE_ID, FINESSE_PLUS_ID,
        FIRE_BREATHING_ID, FIRE_BREATHING_PLUS_ID, FLAME_BARRIER_ID, FLAME_BARRIER_PLUS_ID,
        FLASH_OF_STEEL_ID, FLASH_OF_STEEL_PLUS_ID, FLEX_ID, FLEX_PLUS_ID, FOLLOW_UP_ANY_COLOR_ID,
        FORETHOUGHT_ID, FORETHOUGHT_PLUS_ID, GO_FOR_THE_EYES_ANY_COLOR_ID, HAND_OF_GREED_ID,
        HAND_OF_GREED_PLUS_ID, HAVOC_ID, HAVOC_PLUS_ID, HEADBUTT_ID, HEADBUTT_PLUS_ID,
        HEAVY_BLADE_ID, HEAVY_BLADE_PLUS_ID, HEMOKINESIS_ID, HEMOKINESIS_PLUS_ID,
        HOLOGRAM_ANY_COLOR_ID, IMMOLATE_ID, IMMOLATE_PLUS_ID, IMPATIENCE_ID, IMPATIENCE_PLUS_ID,
        INFERNAL_BLADE_ID, INFERNAL_BLADE_PLUS_ID, INFLAME_ID, INFLAME_PLUS_ID, INTIMIDATE_ID,
        INTIMIDATE_PLUS_ID, IRON_WAVE_ID, IRON_WAVE_PLUS_ID, JACK_OF_ALL_TRADES_ID,
        JACK_OF_ALL_TRADES_PLUS_ID, JAX_ID, JAX_PLUS_ID, JUGGERNAUT_ID, JUGGERNAUT_PLUS_ID,
        JUST_LUCKY_ANY_COLOR_ID, LIKE_WATER_ANY_COLOR_ID, LIMIT_BREAK_ID, LIMIT_BREAK_PLUS_ID,
        MADNESS_ID, MADNESS_PLUS_ID, MAGNETISM_ID, MAGNETISM_PLUS_ID, MASTER_OF_STRATEGY_ID,
        MASTER_OF_STRATEGY_PLUS_ID, MAYHEM_ID, MAYHEM_PLUS_ID, METALLICIZE_ID, METALLICIZE_PLUS_ID,
        METAMORPHOSIS_ID, METAMORPHOSIS_PLUS_ID, MIND_BLAST_ID, MIND_BLAST_PLUS_ID, OFFERING_ID,
        OFFERING_PLUS_ID, PANACEA_ID, PANACEA_PLUS_ID, PANACHE_ID, PANACHE_PLUS_ID,
        PANIC_BUTTON_ID, PANIC_BUTTON_PLUS_ID, PERFECTED_STRIKE_ID, PERFECTED_STRIKE_PLUS_ID,
        POISONED_STAB_ANY_COLOR_ID, POMMEL_STRIKE_ID, POMMEL_STRIKE_PLUS_ID, POWER_THROUGH_ID,
        POWER_THROUGH_PLUS_ID, PRESSURE_POINTS_ANY_COLOR_ID, PROSTRATE_ANY_COLOR_ID, PUMMEL_ID,
        PUMMEL_PLUS_ID, PURITY_ID, PURITY_PLUS_ID, RAGE_ID, RAGE_PLUS_ID, RAMPAGE_ID,
        RAMPAGE_PLUS_ID, REAPER_ID, REAPER_PLUS_ID, RECKLESS_CHARGE_ID, RECKLESS_CHARGE_PLUS_ID,
        RECYCLE_ANY_COLOR_ID, RITUAL_DAGGER_ID, RUPTURE_ID, RUPTURE_PLUS_ID, SADISTIC_NATURE_ID,
        SADISTIC_NATURE_PLUS_ID, SANDS_OF_TIME_ID, SANDS_OF_TIME_PLUS_ID, SEARING_BLOW_ID,
        SEARING_BLOW_PLUS_ID, SECOND_WIND_ID, SECOND_WIND_PLUS_ID, SECRET_TECHNIQUE_ID,
        SECRET_TECHNIQUE_PLUS_ID, SECRET_WEAPON_ID, SECRET_WEAPON_PLUS_ID, SEEING_RED_ID,
        SEEING_RED_PLUS_ID, SEVER_SOUL_ID, SEVER_SOUL_PLUS_ID, SHIV_ANY_COLOR_ID, SHOCKWAVE_ID,
        SHOCKWAVE_PLUS_ID, SHRUG_IT_OFF_ID, SHRUG_IT_OFF_PLUS_ID, SKEWER_ANY_COLOR_ID,
        SKIM_ANY_COLOR_ID, SLIMED_ID, SPOT_WEAKNESS_ID, SPOT_WEAKNESS_PLUS_ID,
        STEAM_BARRIER_ANY_COLOR_ID, STORM_OF_STEEL_ANY_COLOR_ID, STREAMLINE_ANY_COLOR_ID,
        STRIKE_R_ID, STRIKE_R_PLUS_ID, SWIFT_STRIKE_ID, SWIFT_STRIKE_PLUS_ID, SWORD_BOOMERANG_ID,
        SWORD_BOOMERANG_PLUS_ID, THE_BOMB_ID, THE_BOMB_PLUS_ID, THE_BOMB_TURNS, THINKING_AHEAD_ID,
        THINKING_AHEAD_PLUS_ID, THUNDERCLAP_ID, THUNDERCLAP_PLUS_ID, TRANQUILITY_ANY_COLOR_ID,
        TRANSMUTATION_ID, TRANSMUTATION_PLUS_ID, TRIP_ID, TRIP_PLUS_ID, TRUE_GRIT_ID,
        TRUE_GRIT_PLUS_ID, TWIN_STRIKE_ID, TWIN_STRIKE_PLUS_ID, UPPERCUT_ID, UPPERCUT_PLUS_ID,
        VIOLENCE_ID, VIOLENCE_PLUS_ID, WALLOP_ANY_COLOR_ID, WARCRY_ID, WARCRY_PLUS_ID,
        WHIRLWIND_ID, WHIRLWIND_PLUS_ID, WILD_STRIKE_ID, WILD_STRIKE_PLUS_ID, WOUND_ID,
    },
    content::shop_pool::{
        colorless_discovery_pool, ironclad_combat_attack_discovery_pool,
        ironclad_combat_discovery_pool, ironclad_combat_skill_discovery_pool,
    },
    ids::{CardId, ContentId, MonsterId},
    relic::{strike_damage_with_relics, Relic, CHEMICAL_X_BONUS_X},
    CardInstance, SimError, SimResult,
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
        DEFEND_R_ID | DEFEND_R_PLUS_ID => defend_queue(card_id, definition),
        BASH_ID | BASH_PLUS_ID => bash_queue(
            card_id,
            target.expect("validated Bash has a target"),
            definition,
        ),
        SLIMED_ID => slimed_queue(card_id),
        SKIM_ANY_COLOR_ID => skim_queue(card_id, definition),
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
            state,
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
            state,
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
        DAGGER_SPRAY_ANY_COLOR_ID => dagger_spray_queue(state, card_id, definition),
        id if id == crate::content::cards::SWEEPING_BEAM_ANY_COLOR_ID => {
            sweeping_beam_queue(state, card_id, definition)
        }
        id if id == crate::content::cards::HALT_ANY_COLOR_ID => {
            halt_queue(state, card_id, definition)
        }
        id if id == crate::content::cards::SCRAPE_ANY_COLOR_ID => scrape_queue(
            state,
            card_id,
            target.expect("validated Scrape has a target"),
            definition,
        ),
        id if id == crate::content::cards::MALAISE_ANY_COLOR_ID => malaise_queue(
            state,
            card_id,
            target.expect("validated Malaise has a target"),
            *card,
        ),
        id if id == crate::content::cards::FLYING_SLEEVES_ANY_COLOR_ID => flying_sleeves_queue(
            state,
            card_id,
            target.expect("validated Flying Sleeves has a target"),
            definition,
        ),
        IMMOLATE_ID | IMMOLATE_PLUS_ID => immolate_queue(card_id, definition),
        TWIN_STRIKE_ID | TWIN_STRIKE_PLUS_ID => twin_strike_queue(
            state,
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
        id if id == crate::content::cards::CREATIVE_AI_ANY_COLOR_ID => creative_ai_queue(card_id),
        id if id == crate::content::cards::STORM_ANY_COLOR_ID => storm_queue(card_id, *card),
        id if id == crate::content::cards::COOLHEADED_ANY_COLOR_ID => {
            coolheaded_queue(state, card_id, definition)
        }
        id if id == crate::content::cards::CALTROPS_ANY_COLOR_ID => {
            caltrops_queue(state, card_id, definition)
        }
        id if id == crate::content::cards::CAPACITOR_ANY_COLOR_ID => {
            capacitor_queue(state, card_id, definition)
        }
        id if id == crate::content::cards::RECURSION_ANY_COLOR_ID => {
            recursion_queue(state, card_id, definition)
        }
        id if id == crate::content::cards::AFTER_IMAGE_ANY_COLOR_ID => after_image_queue(card_id),
        id if id == crate::content::cards::PRAY_ANY_COLOR_ID => {
            pray_queue(state, card_id, definition)
        }
        id if id == crate::content::cards::INSIGHT_ID => insight_queue(state, card_id, definition),
        id if id == crate::content::cards::CRESCENDO_ANY_COLOR_ID => {
            crescendo_queue(state, card_id, definition)
        }
        MAYHEM_ID | MAYHEM_PLUS_ID => mayhem_queue(card_id),
        FIRE_BREATHING_ID | FIRE_BREATHING_PLUS_ID => fire_breathing_queue(card_id, definition),
        EXHUME_ID | EXHUME_PLUS_ID => exhume_queue(state, card_id),
        DEMON_FORM_ID | DEMON_FORM_PLUS_ID => demon_form_queue(card_id, definition),
        METALLICIZE_ID | METALLICIZE_PLUS_ID => metallicize_queue(card_id, definition),
        POMMEL_STRIKE_ID | POMMEL_STRIKE_PLUS_ID | FLASH_OF_STEEL_ID | FLASH_OF_STEEL_PLUS_ID => {
            pommel_strike_queue(
                state,
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
            state,
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
        EVISCERATE_ANY_COLOR_ID => eviscerate_queue(
            state,
            card_id,
            target.expect("validated Eviscerate has a target"),
            *card,
            definition,
        ),
        SKEWER_ANY_COLOR_ID => skewer_queue(
            state,
            card_id,
            target.expect("validated Skewer has a target"),
            *card,
            definition,
        ),
        STORM_OF_STEEL_ANY_COLOR_ID => storm_of_steel_queue(state, card_id, *card, definition),
        STEAM_BARRIER_ANY_COLOR_ID => steam_barrier_queue(card_id, *card, definition),
        FOLLOW_UP_ANY_COLOR_ID => follow_up_queue(
            state,
            card_id,
            target.expect("validated Follow-Up has a target"),
            *card,
            definition,
        ),
        STREAMLINE_ANY_COLOR_ID => streamline_queue(
            card_id,
            target.expect("validated Streamline has a target"),
            *card,
            definition,
        ),
        WALLOP_ANY_COLOR_ID => wallop_queue(
            card_id,
            target.expect("validated Wallop has a target"),
            *card,
            definition,
        ),
        WHIRLWIND_ID | WHIRLWIND_PLUS_ID => whirlwind_queue(state, card_id, definition),
        TRANSMUTATION_ID | TRANSMUTATION_PLUS_ID => transmutation_queue(state, card_id, definition),
        SECRET_TECHNIQUE_ID | SECRET_TECHNIQUE_PLUS_ID => {
            secret_technique_queue(state, card_id, definition)
        }
        SECRET_WEAPON_ID | SECRET_WEAPON_PLUS_ID => secret_weapon_queue(state, card_id, definition),
        BLASPHEMY_ID | BLASPHEMY_PLUS_ID => blasphemy_queue(card_id, definition),
        SANDS_OF_TIME_ID | SANDS_OF_TIME_PLUS_ID => generic_attack_queue(
            state,
            card_id,
            target.expect("validated Sands of Time has a target"),
            definition,
        ),
        HAVOC_ID | HAVOC_PLUS_ID => havoc_queue(&mut queued_state, card_id, definition, target),
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
        CHARGE_BATTERY_ANY_COLOR_ID => {
            queued_state.player.energy_next_turn = queued_state
                .player
                .energy_next_turn
                .checked_add(1)
                .ok_or(SimError::InvalidState("next-turn energy overflows i32"))?;
            charge_battery_queue(card_id, *card, definition)
        }
        JUST_LUCKY_ANY_COLOR_ID => just_lucky_queue(
            card_id,
            target.expect("validated Just Lucky has a target"),
            *card,
            definition,
        ),
        id if id == crate::content::cards::EVALUATE_ANY_COLOR_ID => {
            evaluate_queue(state, card_id, definition)
        }
        id if id == crate::content::cards::BOOT_SEQUENCE_ANY_COLOR_ID => {
            boot_sequence_queue(state, card_id, definition)
        }
        id if id == crate::content::cards::DARKNESS_ANY_COLOR_ID => {
            darkness_queue(state, card_id, definition)
        }
        id if id == crate::content::cards::CONCLUDE_ANY_COLOR_ID => {
            conclude_queue(state, card_id, definition)
        }
        id if id == crate::content::cards::LEAP_ANY_COLOR_ID => {
            leap_queue(state, card_id, definition)
        }
        id if id == crate::content::cards::LEG_SWEEP_ANY_COLOR_ID => leg_sweep_queue(
            state,
            card_id,
            target.expect("validated Leg Sweep has a target"),
            definition,
        ),
        id if id == crate::content::cards::JUDGEMENT_ANY_COLOR_ID => judgement_queue(
            state,
            card_id,
            target.expect("validated Judgement has a target"),
            definition,
        ),
        id if id == crate::content::cards::FEAR_NO_EVIL_ANY_COLOR_ID => fear_no_evil_queue(
            state,
            card_id,
            target.expect("validated Fear No Evil has a target"),
            definition,
        ),
        id if id == crate::content::cards::BANE_ANY_COLOR_ID => bane_queue(
            state,
            card_id,
            target.expect("validated Bane has a target"),
            definition,
        ),
        id if id == crate::content::cards::STATIC_DISCHARGE_ANY_COLOR_ID => {
            static_discharge_queue(state, card_id, definition)
        }
        id if id == crate::content::cards::EMPTY_BODY_ANY_COLOR_ID => {
            empty_body_queue(state, card_id, definition)
        }
        id if id == crate::content::cards::CRUSH_JOINTS_ANY_COLOR_ID => crush_joints_queue(
            state,
            card_id,
            target.expect("validated Crush Joints has a target"),
            *card,
            definition,
        ),
        GO_FOR_THE_EYES_ANY_COLOR_ID => go_for_the_eyes_queue(
            state,
            card_id,
            target.expect("validated Go for the Eyes has a target"),
            *card,
            definition,
        ),
        BEAM_CELL_ANY_COLOR_ID => beam_cell_queue(
            card_id,
            target.expect("validated Beam Cell has a target"),
            *card,
            definition,
        ),
        BOWLING_BASH_ANY_COLOR_ID => bowling_bash_queue(
            state,
            card_id,
            target.expect("validated Bowling Bash has a target"),
            *card,
            definition,
        ),
        COMPILE_DRIVER_ANY_COLOR_ID => compile_driver_queue(
            state,
            card_id,
            target.expect("validated Compile Driver has a target"),
            *card,
            definition,
        ),
        POISONED_STAB_ANY_COLOR_ID => poisoned_stab_queue(
            card_id,
            target.expect("validated Poisoned Stab has a target"),
            *card,
            definition,
        ),
        HOLOGRAM_ANY_COLOR_ID => hologram_queue(state, card_id, *card, definition),
        FASTING_ANY_COLOR_ID => fasting_queue(card_id, *card),
        LIKE_WATER_ANY_COLOR_ID => like_water_queue(card_id, *card),
        EQUILIBRIUM_ANY_COLOR_ID => {
            queued_state.player.retain_hand_next_turn = true;
            equilibrium_queue(card_id, *card, definition)
        }
        PROSTRATE_ANY_COLOR_ID => prostrate_queue(card_id, *card, definition),
        RECYCLE_ANY_COLOR_ID => recycle_queue(card_id, definition),
        BIASED_COGNITION_ANY_COLOR_ID => biased_cognition_queue(card_id, definition),
        PRESSURE_POINTS_ANY_COLOR_ID => pressure_points_queue(card_id, target, *card, definition),
        EMPTY_MIND_ANY_COLOR_ID => empty_mind_queue(state, card_id, definition),
        TRANQUILITY_ANY_COLOR_ID => tranquility_queue(card_id, definition),
        DOPPELGANGER_ANY_COLOR_ID => doppelganger_queue(card_id),
        BACKFLIP_ANY_COLOR_ID => backflip_queue(card_id, *card, definition),
        CLOAK_AND_DAGGER_ANY_COLOR_ID => cloak_and_dagger_queue(card_id, definition),
        SHIV_ANY_COLOR_ID => {
            let mut shiv = *definition;
            if card.upgrades > 0 {
                shiv.values.damage = Some(required_damage(definition)? + 2);
            }
            generic_attack_queue(
                state,
                card_id,
                target.expect("validated Shiv has a target"),
                &shiv,
            )
        }
        _ if definition.values.damage.is_some()
            && definition.target == crate::TargetRequirement::Enemy =>
        {
            generic_attack_queue(
                state,
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
    // Double Tap, Necronomicon, and Duplication each queue one extra play of
    // the original card (`purgeOnUse` copies). Wrapping an already-wrapped
    // queue nests those copies (FIDL00036: Heavy Blade 4 intangible hits
    // instead of original + Necronomicon + Double Tap).
    let copied_effects = snapshot_copied_card_effects(&queue, card_id);
    let duplication_effects = snapshot_duplication_potion_effects(&queue, card_id);
    if state.duplication_potion_pending || state.duplication_potion_stacks > 0 {
        queue = apply_duplication_potion_to_queue(queue, card_id, duplication_effects);
    }
    if should_apply_necronomicon(state, card, definition)? {
        queue = apply_necronomicon_to_queue(queue, card_id, copied_effects.clone());
    }
    if definition.card_type == CardType::Attack && state.double_tap_pending > 0 {
        queue = apply_double_tap_to_queue(queue, card_id, copied_effects);
    }
    // Pen Nib doubles at damage resolution when the 10th attack play wraps the
    // counter (see apply_on_card_play_relics + pen_nib_double_active). Build-time
    // rewriting only saw the pre-play counter, so a Double Tap copy that is the
    // 10th attack never got the bonus (FIDL00421: 7+14 with counter 8→0).
    // Vigor applies at damage resolution time (like Strength). Consume it after
    // the original Attack card's hits so multi-hit attacks keep the bonus and
    // Double Tap / Necronomicon copies do not.
    apply_vigor_consumption_to_attack_queue(state, definition.card_type, card_id, &mut queue);

    apply_effective_cost_to_played_card_queue(card, definition, &mut queue)?;
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
    // The target removes the top card into limbo before resolving its effect.
    // Keep the staged hand copy for shared card builders, while retaining a
    // limbo marker so pile-sensitive effects do not count that copy as a
    // current combat-pile card.
    staged.piles.limbo.push(card);
    // Put the staged source first so test/debug states with duplicate card IDs
    // cannot redirect effect construction to an unrelated hand card. Validated
    // production states still require globally unique card IDs.
    staged.piles.hand.insert(0, card);
    let (mut queued_state, mut queue) = play_card_queue(&staged, card.id, target)?;
    queued_state
        .piles
        .limbo
        .retain(|candidate| candidate.id != card.id);

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
    // Keep limbo-aware draw actions from the shared builder. The staged card is
    // temporarily in hand so those actions can remove its slot during draws,
    // matching NewQueueCardAction/UseCardAction while the forced card resolves.
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
    let shared_movement_index = queue.iter().position(|action| {
        matches!(
            action,
            InternalAction::MoveCard {
                card_id,
                from: CardPile::Hand,
                ..
            } if *card_id == card.id
        )
    });
    // PutOnDeckAction completes before UseCardAction settles its source. For a
    // force-played top card, keep the source staged until the selection closes
    // and queue the source MoveCard after AwaitHandSelect. Other force-played
    // selects (for example Armaments/Dual Wield) retain their source-specific
    // settlement.
    let delayed_hand_select_moves_source = !force_exhaust
        && queue.iter().any(|action| {
            matches!(
                action,
                InternalAction::AwaitHandSelect { source_card_id, .. }
                    if *source_card_id == card.id
            )
        });
    let put_on_deck_select_index = queue.iter().position(|action| {
        matches!(
            action,
            InternalAction::AwaitHandSelect {
                source_card_id,
                purpose: HandSelectPurpose::WarcryPutOnDraw
                    | HandSelectPurpose::ThinkingAheadPutOnDraw
                    | HandSelectPurpose::ForethoughtPutOnDraw
                    | HandSelectPurpose::ForethoughtPutAnyOnDraw,
            } if *source_card_id == card.id
        )
    });
    // Force-play (Havoc / Mayhem / Distilled Chaos) must not exhaust Exhume or
    // True Grit+ before its exhaust-select closes. Early exhaust queues Dark
    // Embrace / Feel No Pain / Dead Branch as pending_actions under the open
    // screen, desyncing FIDL00253 (True Grit+ still in play at select open;
    // exhaust + on-exhaust settle on CONFIRM). await_exhaust_select parks the
    // source on the decision; confirm_* settles exhaust after the choice.
    // Burning Pact is the same family: Havoc force-exhaust waits for CONFIRM
    // so Charon's Ashes does not hit while the select is open (FIDL01636).
    let selection_defers_source_settlement = queue.iter().any(|action| {
        // Ordinary Mayhem also leaves top-played Exhume/True Grit and Headbutt
        // in cardInUse until their selection closes; only the destination
        // differs from a force-exhausted Havoc play.
        matches!(
            action,
            InternalAction::AwaitExhaustSelect {
                source_card_id,
                purpose: crate::combat::ExhaustSelectPurpose::ExhumeReturnToHand
                    | crate::combat::ExhaustSelectPurpose::TrueGritExhaustOne,
            } if *source_card_id == card.id
        ) || matches!(
            action,
            InternalAction::AwaitDiscardSelect {
                source_card_id,
                purpose: crate::combat::DiscardSelectPurpose::HeadbuttPutOnDraw
                    | crate::combat::DiscardSelectPurpose::HologramReturnToHand,
            } if *source_card_id == card.id
        ) || matches!(
            action,
            // Secret Technique/Weapon queue their filtered draw action before
            // UseCardAction settles the source. Top-play envelopes must leave
            // the source staged until that draw selection closes as well.
            InternalAction::AwaitDrawSelect {
                source_card_id,
                purpose: crate::combat::DrawSelectPurpose::SecretTechniqueSkillToHand
                    | crate::combat::DrawSelectPurpose::SecretWeaponAttackToHand,
            } if *source_card_id == card.id
        )
    });
    let force_exhaust_opens_deferred_source_select = selection_defers_source_settlement
        || (force_exhaust
            && queue.iter().any(|action| {
                matches!(
                    action,
                    // FIDL00242: multi-eligible force-played Dual Wield settles on
                    // CONFIRM so Dark Embrace draws after the select closes. Singleton
                    // force-play still early-exhausts and auto-confirms.
                    InternalAction::AwaitHandSelect {
                        source_card_id,
                        purpose: crate::combat::HandSelectPurpose::DualWieldCopy,
                    } if *source_card_id == card.id
                        && dual_wield_force_play_defers_source_settlement(state, card.id)
                ) || matches!(
                    action,
                    // FIDL01254: force-played Armaments stays in cardInUse until
                    // CONFIRM so Charon's Ashes / Dark Embrace fire after the
                    // upgrade screen closes, not when Havoc first opens it.
                    InternalAction::AwaitHandSelect {
                        source_card_id,
                        purpose: crate::combat::HandSelectPurpose::ArmamentsUpgrade,
                    } if *source_card_id == card.id
                ) || matches!(
                    action,
                    // FIDL01437 / FIDL01593: force-played Forethought stays
                    // staged until CONFIRM. Exhausting it at screen-open drops
                    // the instance, and CONFIRM then fails with UnknownCard.
                    InternalAction::AwaitHandSelect {
                        source_card_id,
                        purpose: crate::combat::HandSelectPurpose::ForethoughtPutOnDraw
                            | crate::combat::HandSelectPurpose::ForethoughtPutAnyOnDraw
                            | crate::combat::HandSelectPurpose::WarcryPutOnDraw
                            | crate::combat::HandSelectPurpose::ThinkingAheadPutOnDraw,
                    } if *source_card_id == card.id
                )
            }));
    let discovery_reward_defers_source_settlement =
        matches!(definition.id, DISCOVERY_ID | DISCOVERY_PLUS_ID);
    // Discovery's reward screen pauses the queue before this movement. Defer
    // its exhaust/Spoon decision until CHOOSE, after DiscoveryAction's final
    // discarded choice generation, while keeping an exhaust destination for
    // the queue shape used by the source-card builder.
    // Force-played Armaments / Dual Wield / Forethought also settle on CONFIRM
    // (FIDL01254 / FIDL01427): do not consume Strange Spoon here or CONFIRM
    // re-rolls a later counter and can exhaust a Spoon-saved Armaments.
    let destination = if (discovery_reward_defers_source_settlement
        && (force_exhaust
            || definition.keywords.exhaust
            || (definition.card_type == CardType::Skill && state.player.powers.corruption > 0)))
        || force_exhaust_opens_deferred_source_select
    {
        CardPile::ExhaustPile
    } else {
        top_draw_card_destination(definition, force_exhaust, shared_destination, state)
    };
    // PlayTop sets exhaustOnUseOnce, but Strange Spoon's onExhaust callback
    // does not roll until UseCardAction settles the forced card. Keep that roll
    // behind every action produced by the card (random hits and generated-card
    // inserts included) instead of deciding the destination while expanding
    // the card-queue item.
    if destination == CardPile::ExhaustPile
        && definition.card_type != CardType::Power
        && queued_state.relics.contains(&Relic::StrangeSpoon)
        && !queued_state
            .defer_strange_spoon_until_source_move
            .contains(&card.id)
    {
        queued_state
            .defer_strange_spoon_until_source_move
            .push(card.id);
    }
    queue.retain(|action| !is_card_move_for(*action, card.id));
    let movement = InternalAction::MoveCard {
        card_id: card.id,
        from: CardPile::Hand,
        to: destination,
    };
    // Top-play keeps Burning Pact in cardInUse while its ExhaustSelect screen
    // is open; the decision owns the staged source until CONFIRM settles it.
    // Havoc still force-exhausts on CONFIRM (FIDL01636 Charon's Ashes).
    let burning_pact_opens_deferred_source_select = queue.iter().any(|action| {
        matches!(
            action,
            InternalAction::AwaitExhaustSelect {
                source_card_id,
                purpose: crate::combat::ExhaustSelectPurpose::BurningPactDraw2
                    | crate::combat::ExhaustSelectPurpose::BurningPactDraw3,
            } if *source_card_id == card.id
        )
    });
    let played_index = queue
        .iter()
        .position(
            |action| matches!(action, InternalAction::PlayCard { card_id } if *card_id == card.id),
        )
        .ok_or(SimError::InvalidState(
            "top-draw card queue has no play action",
        ))?;
    if !delayed_hand_select_moves_source
        && !force_exhaust_opens_deferred_source_select
        && !burning_pact_opens_deferred_source_select
        && !discovery_reward_defers_source_settlement
    {
        // Hand-play builders place MoveCard at the UseCardAction slot among *that*
        // card's bot actions (after damage, before nothing, etc.). Nested Havoc /
        // Mayhem are special: card.use() queues PlayTop before UseCardAction, so a
        // force-exhausted top-played Havoc must settle only after its own PlayTop
        // finishes. Otherwise Dead Branch rolls between the two target burns
        // (T,DB,T,DB) instead of after both (T,T,DB,DB) — FIDL00394 dual Havoc.
        let has_nested_play_top = queue
            .iter()
            .any(|action| matches!(action, InternalAction::PlayTopDrawCard { .. }));
        if has_nested_play_top {
            queue.push_back(movement);
        } else if let Some(index) = put_on_deck_select_index {
            // The source stays in the staged hand while PutOnDeckAction is open;
            // CONFIRM resumes this pending MoveCard after the selected card is
            // placed on top of the draw pile.
            queue.insert(index + 1, movement);
        } else if let Some(index) = shared_movement_index {
            queue.insert(index, movement);
        } else {
            queue.insert(played_index + 1, movement);
        }
    }

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
    definition: &CardDefinition,
    force_exhaust: bool,
    shared_destination: Option<CardPile>,
    state: &CombatState,
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
    CardPile::ExhaustPile
}

fn apply_effective_cost_to_played_card_queue(
    card: &CardInstance,
    definition: &CardDefinition,
    queue: &mut VecDeque<InternalAction>,
) -> SimResult<()> {
    let printed_cost = i32::from(definition.cost);
    let effective_cost = if card.free_to_play_once {
        0
    } else {
        effective_card_cost(card)?
    };
    if effective_cost == printed_cost {
        return Ok(());
    }

    for action in queue.iter_mut() {
        if let InternalAction::SpendEnergy { amount } = action {
            if *amount == printed_cost {
                *amount = effective_cost;
            }
            break;
        }
    }
    Ok(())
}

fn should_apply_necronomicon(
    state: &CombatState,
    card: &CardInstance,
    definition: &CardDefinition,
) -> SimResult<bool> {
    if definition.card_type != CardType::Attack
        || !state.relics.contains(&Relic::Necronomicon)
        || state.relic_counters.necronomicon_used_this_turn
    {
        return Ok(false);
    }
    // NecronomiconPower.onUseCard requires costForTurn >= 2. X-cost cards keep
    // printed cost -1 (and Infernal Blade may overlay 0-cost-this-turn), so the
    // game uses energyOnUse instead: a 2+ Energy Whirlwind is played twice
    // (FIDL01485 Giant Head 376→340).
    let necronomicon_cost = if definition.cost < 0 {
        state.player.energy
    } else {
        effective_card_cost(card)?
    };
    Ok(necronomicon_cost >= 2)
}

fn apply_corruption_to_played_skill_queue(
    state: &CombatState,
    definition: &CardDefinition,
    card_id: CardId,
    queue: &mut VecDeque<InternalAction>,
) {
    if definition.card_type != CardType::Skill
        || definition.cost < 0
        || state.player.powers.corruption <= 0
    {
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
    if own_exhaust_index.is_none() {
        return;
    }

    // StrangeSpoon.onExhaust rolls when UseCardAction would moveToExhaustPile.
    // HexPower.onUseCard addToBots MakeTempCardInDrawPile before that UseCardAction,
    // so the Dazed insert must consume cardRandomRng first (FIDL02399 Limit Break).
    // Violence/Madness already needed the same deferral (FIDL01427 / FIDL02356).
    if !state
        .defer_strange_spoon_until_source_move
        .contains(&card_id)
    {
        state.defer_strange_spoon_until_source_move.push(card_id);
    }
}

fn apply_vigor_consumption_to_attack_queue(
    state: &CombatState,
    card_type: CardType,
    card_id: CardId,
    queue: &mut VecDeque<InternalAction>,
) {
    if card_type != CardType::Attack || state.player.powers.vigor <= 0 {
        return;
    }

    // Prefer clearing after original hits and before any PlayCardCopy so
    // Double Tap / Necronomicon clones do not inherit Vigor.
    if let Some(index) = queue
        .iter()
        .position(|action| matches!(action, InternalAction::PlayCardCopy { .. }))
    {
        queue.insert(index, InternalAction::ConsumeVigor);
        return;
    }
    if let Some(index) = queue
        .iter()
        .rposition(|action| is_card_move_for(*action, card_id))
    {
        queue.insert(index + 1, InternalAction::ConsumeVigor);
        return;
    }
    queue.push_back(InternalAction::ConsumeVigor);
}

pub(crate) fn pen_nib_queue_amount(state: &CombatState, amount: i32) -> i32 {
    // Pen Nib doubles the pre-Weak/Vulnerable attack total, including Strength
    // and Vigor. Bake the double into the queued base so later
    // `base + strength + vigor` yields `2 * (base + strength + vigor)`.
    let additive =
        state.player.powers.strength + state.player.temp_strength + state.player.powers.vigor;
    (amount + additive).max(0) * 2 - additive
}

fn snapshot_copied_card_effects(
    queue: &VecDeque<InternalAction>,
    card_id: CardId,
) -> VecDeque<InternalAction> {
    queue
        .iter()
        .copied()
        .filter_map(|action| duplicated_card_effect(action, card_id))
        .collect()
}

fn snapshot_duplication_potion_effects(
    queue: &VecDeque<InternalAction>,
    card_id: CardId,
) -> VecDeque<InternalAction> {
    queue
        .iter()
        .copied()
        .filter_map(|action| duplicated_card_effect(action, card_id))
        .collect()
}

fn apply_duplication_potion_to_queue(
    mut queue: VecDeque<InternalAction>,
    card_id: CardId,
    mut duplicated_effects: VecDeque<InternalAction>,
) -> VecDeque<InternalAction> {
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
    mut duplicated_effects: VecDeque<InternalAction>,
) -> VecDeque<InternalAction> {
    let rampage_growth = duplicated_effects
        .iter()
        .filter_map(|action| match action {
            InternalAction::IncreaseRampageDamage {
                card_id: source,
                amount,
            } if *source == card_id => Some(*amount),
            _ => None,
        })
        .try_fold(0i32, i32::checked_add);
    if let Some(rampage_growth) = rampage_growth {
        for action in &mut duplicated_effects {
            if let InternalAction::DealDamage {
                info:
                    crate::combat::damage::DamageInfo {
                        source: DamageSource::Card(source),
                        amount,
                        ..
                    },
            } = action
            {
                if *source == card_id {
                    if let Some(adjusted) = amount.checked_add(rampage_growth) {
                        *amount = adjusted;
                    }
                }
            }
        }
    }

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
    mut duplicated_effects: VecDeque<InternalAction>,
) -> VecDeque<InternalAction> {
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
    // A copied card is a new action-manager boundary. Resolve reactions queued
    // by the original card before the copy's effects begin.
    queue.push_back(InternalAction::ResolvePendingMonsterReactions);
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
        | InternalAction::PrepareCardDamage { info }
        | InternalAction::DealDamageAndHealUnblocked { info }
        | InternalAction::DealFeedDamage { info, .. } => Some(info.target),
        InternalAction::DealBodySlamDamage { target, .. }
        | InternalAction::ResolveFiendFire { target, .. } => Some(target),
        InternalAction::GainMonsterBlock { target, .. }
        | InternalAction::ApplyVulnerable { target, .. }
        | InternalAction::ReduceMonsterStrength { target, .. }
        | InternalAction::ReduceMonsterStrengthThisTurn { target, .. }
        | InternalAction::DealUnmodifiedDamage { target, .. }
        | InternalAction::ApplyWeak { target, .. }
        | InternalAction::ApplyWeakIfTargetAttacking { target, .. }
        | InternalAction::LoseMonsterHp { target, .. } => Some(target),
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
            // Blue Candle's HP loss is caused by playing this Curse, so card
            // HP-loss powers such as Rupture must observe the event.
            source: HpLossSource::Card(card_id),
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
            | InternalAction::AwaitCopiedDiscardSelect { .. }
            | InternalAction::AwaitExhaustSelect { .. }
    ) && !is_card_move_for(action, card_id)
}

fn duplicated_card_effect(action: InternalAction, card_id: CardId) -> Option<InternalAction> {
    match action {
        InternalAction::AwaitDiscardSelect {
            source_card_id,
            purpose: crate::combat::DiscardSelectPurpose::HeadbuttPutOnDraw,
        } if source_card_id == card_id => Some(InternalAction::AwaitCopiedDiscardSelect {
            purpose: crate::combat::DiscardSelectPurpose::HeadbuttPutOnDraw,
        }),
        InternalAction::ResolveFollowUpEnergy { .. } => {
            Some(InternalAction::ResolveFollowUpEnergy { should_gain: true })
        }
        action if is_duplicated_card_effect(action, card_id) => Some(action),
        _ => None,
    }
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

fn required_damage(definition: &CardDefinition) -> SimResult<i32> {
    definition.values.damage.ok_or(SimError::InvalidState(
        "card definition is missing required damage",
    ))
}

fn required_block(definition: &CardDefinition) -> SimResult<i32> {
    definition.values.block.ok_or(SimError::InvalidState(
        "card definition is missing required block",
    ))
}

fn attack_damage_with_strike_dummy(
    state: &CombatState,
    definition: &CardDefinition,
) -> SimResult<i32> {
    let damage = required_damage(definition)?;
    Ok(if is_strike_named_definition(definition) {
        strike_damage_with_relics(&state.relics, damage)
    } else {
        damage
    })
}

fn synthetic_any_color_upgrade_damage(definition: &CardDefinition, upgrades: u8) -> i32 {
    if upgrades == 0 {
        return 0;
    }
    use crate::content::cards::{
        FLURRY_OF_BLOWS_ANY_COLOR_ID, FLYING_SLEEVES_ANY_COLOR_ID, SLICE_ANY_COLOR_ID,
        SNEAKY_STRIKE_ANY_COLOR_ID, WINDMILL_STRIKE_ANY_COLOR_ID,
    };
    if definition.id == SLICE_ANY_COLOR_ID {
        // Slice.upgradeDamage(3)
        3
    } else if definition.id == FLURRY_OF_BLOWS_ANY_COLOR_ID
        || definition.id == FLYING_SLEEVES_ANY_COLOR_ID
    {
        // FlurryOfBlows / FlyingSleeves.upgradeDamage(2)
        2
    } else if definition.id == SNEAKY_STRIKE_ANY_COLOR_ID {
        // SneakyStrike.upgradeDamage(4)
        4
    } else if definition.id == WINDMILL_STRIKE_ANY_COLOR_ID {
        // WindmillStrike.upgradeDamage(3)
        3
    } else if definition.id == crate::content::cards::CRUSH_JOINTS_ANY_COLOR_ID {
        // CrushJoints.upgradeDamage(2)
        2
    } else if definition.id == crate::content::cards::SWEEPING_BEAM_ANY_COLOR_ID {
        // SweepingBeam.upgradeDamage(3)
        3
    } else if definition.id == crate::content::cards::BANE_ANY_COLOR_ID {
        // Bane.upgradeDamage(3)
        3
    } else {
        0
    }
}

fn required_vulnerable(definition: &CardDefinition) -> SimResult<i32> {
    definition.values.vulnerable.ok_or(SimError::InvalidState(
        "card definition is missing required vulnerable",
    ))
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
                amount: strike_damage_with_relics(&state.relics, required_damage(definition)?),
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
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    let retain_bonus = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.windmill_retain_damage)
        .unwrap_or(0);
    let amount = attack_damage_with_strike_dummy(state, definition)?
        + synthetic_any_color_upgrade_damage(definition, upgrades)
        + retain_bonus;
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount,
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
                amount: required_damage(definition)?,
            },
            gold: required_vulnerable(definition)?,
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
            amount: required_block(definition)?,
        },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: required_damage(definition)?,
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
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DealBodySlamDamage {
            source: card_id,
            target,
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
            amount: required_block(definition)?,
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
    _state: &CombatState,
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
                amount: required_damage(definition)?,
            },
        },
    ]);

    // Headbutt.use() always queues PutOnDeckAction. Discard emptiness is
    // checked when that action runs, not when the card is queued. Double Tap /
    // Necronomicon copies therefore still auto-put after the original settles
    // into an empty discard (FIDL01747).
    queue.push_back(InternalAction::AwaitDiscardSelect {
        source_card_id: card_id,
        purpose: crate::combat::DiscardSelectPurpose::HeadbuttPutOnDraw,
    });

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
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    state
        .monsters
        .iter()
        .find(|monster| monster.id == target && monster.alive)
        .ok_or(SimError::IllegalAction("Hemokinesis target is not alive"))?;
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        // Each Double Tap copy recalculates card damage immediately before its
        // own use(), after copy-play powers but before Hemokinesis queues HP loss.
        InternalAction::PrepareCardDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: required_damage(definition)?,
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
                amount: required_damage(definition)?,
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
                amount: required_damage(definition)?,
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
                amount: required_damage(definition)?,
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
                amount: (required_damage(definition)? + extra_strength).max(0),
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
    let base_damage = required_damage(definition)? + (strike_bonus * strike_count);
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
            !state
                .piles
                .limbo
                .iter()
                .any(|limbo_card| limbo_card.id == card.id)
        })
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
                amount: attack_damage_with_strike_dummy(state, definition)?,
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
                amount: required_damage(definition)?,
            },
        },
        InternalAction::HealPlayer {
            amount: if definition.id == BITE_PLUS_ID { 3 } else { 2 },
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
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        // MakeTempCardInHandAction runs while the played card is in limbo. It
        // must not count Power Through against either Wound's hand capacity:
        // with nine visible cards, retaining the source would incorrectly
        // discard the second Wound before the source itself is discarded.
        InternalAction::AddGeneratedCardsToHandWhileSourceInLimbo {
            content_id: WOUND_ID,
            source_card_id: card_id,
            count: 2,
            temp_cost: None,
            temp_cost_turn_only: false,
        },
    ]);
    queue.extend([
        InternalAction::GainBlock {
            amount: required_block(definition)?,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]);
    Ok(queue)
}

fn infernal_blade_queue(
    state: &mut CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let generated = infernal_blade_generated_attack(state);
    let add_generated = if state.piles.hand.len() >= MAX_HAND_SIZE {
        // MakeTempCardInHandAction observes the played card in limbo. A full
        // visible hand therefore still has one slot for Infernal Blade's
        // generated attack while its source card is resolving.
        InternalAction::AddGeneratedCardsToHandWhileSourceInLimbo {
            content_id: generated,
            source_card_id: card_id,
            count: 1,
            temp_cost: Some(0),
            temp_cost_turn_only: true,
        }
    } else {
        InternalAction::AddGeneratedCardToPile {
            content_id: generated,
            to: CardPile::Hand,
            temp_cost: Some(0),
            temp_cost_turn_only: true,
        }
    };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        add_generated,
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
    // Do not open the reward during queue build: Hex onUseCard follow-ups after
    // PlayCard must still run SpendEnergy / OpenDiscovery first. Opening early
    // parked those actions behind the reward (FIDL00233 energy/source lag).
    let _ = state;
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

pub(crate) fn open_discovery_card_reward_for_play(
    state: &mut CombatState,
    _source_card_id: CardId,
) -> SimResult<()> {
    let next_card_id = state.reserve_card_instance_ids(3)?;
    let pool = discovery_card_pool();
    let content_choices = discovery_choices_from_pool(&mut state.rng.card_random_rng, &pool);
    // DiscoveryAction generates this visible offer during its opening update.
    // Any discarded post-selection update belongs to CHOOSE, after the reward
    // screen has closed and before the selected card is retrieved.
    state.decision = Some(CombatDecisionState::DiscoveryCardReward {
        choices: content_choices
            .into_iter()
            .enumerate()
            .map(|(index, content_id)| {
                CardInstance::new(CardId::new(next_card_id + index as u64), content_id)
            })
            .collect(),
        source_card: None,
        source_card_force_exhaust: state.play_top_force_exhaust_active,
        source_card_play_top: state.play_top_resolving_depth > 0,
        pending_actions: std::collections::VecDeque::new(),
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

pub(crate) fn discovery_card_pool() -> Vec<ContentId> {
    // Discovery samples the complete target source pool. Unsupported card
    // mechanics may stop replay when the generated card is later used, but
    // filtering the offer changes the RNG bound and can select a different
    // supported card.
    ironclad_combat_discovery_pool().to_vec()
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
    if state.piles.hand.len() >= MAX_HAND_SIZE {
        for _ in 0..generated_count {
            let generated = jack_of_all_trades_generated_colorless(state);
            queue.push_back(InternalAction::AddGeneratedCardsToHandWhileSourceInLimbo {
                content_id: generated,
                source_card_id: card_id,
                count: 1,
                temp_cost: None,
                temp_cost_turn_only: false,
            });
        }
    } else {
        for _ in 0..generated_count {
            let generated = jack_of_all_trades_generated_colorless(state);
            queue.push_back(InternalAction::AddGeneratedCardToPile {
                content_id: generated,
                to: CardPile::Hand,
                temp_cost: None,
                temp_cost_turn_only: false,
            });
        }
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
        InternalAction::SetRandomHandCardCostForCombat {
            amount: 0,
            excluded_card_id: card_id,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
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
    // Card text: shuffle a Dazed (one) into the draw pile.
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: required_damage(definition)?,
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
    let damage = required_damage(definition)?;
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
                amount: required_damage(definition)?,
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
        InternalAction::SpendCardEnergy { card_id },
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
            amount: required_vulnerable(definition)?,
        });
    } else {
        for monster in state.monsters.iter().filter(|monster| monster.alive) {
            queue.push_back(InternalAction::ApplyVulnerable {
                target: monster.id,
                amount: required_vulnerable(definition)?,
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
        let amount = required_vulnerable(definition)?;
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
    let damage = required_damage(definition)?;

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
            amount: required_block(definition)?,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn doppelganger_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        },
    ]))
}

fn tranquility_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::EnterCalm,
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        },
    ]))
}

fn empty_mind_queue(
    _state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    // EmptyMind.use addToBots DrawCardAction then ChangeStance(Neutral).
    // CalmStance.onExitStance supplies the +2 energy; same-stance Neutral is a no-op.
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DrawCards { count: 2 },
        InternalAction::ExitCalm,
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn pressure_points_queue(
    card_id: CardId,
    target: Option<MonsterId>,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let target = target.ok_or(SimError::IllegalAction("Pressure Points requires a target"))?;
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::ApplyMark {
            target,
            amount: 8 + 3 * i32::from(card.upgrades > 0),
        },
        InternalAction::TriggerMarks,
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn biased_cognition_queue(
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
            to: CardPile::DiscardPile,
        },
    ]))
}

fn recycle_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::AwaitExhaustSelect {
            source_card_id: card_id,
            purpose: crate::combat::ExhaustSelectPurpose::RecycleExhaustOne,
        },
    ]))
}

fn backflip_queue(
    card_id: CardId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainBlock {
            amount: required_block(definition)? + 3 * i32::from(card.upgrades > 0),
        },
        InternalAction::DrawCards { count: 2 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn cloak_and_dagger_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainBlock {
            amount: required_block(definition)?,
        },
        InternalAction::AddGeneratedCardsToHandWhileSourceInLimbo {
            content_id: SHIV_ANY_COLOR_ID,
            source_card_id: card_id,
            count: 1,
            temp_cost: Some(0),
            temp_cost_turn_only: false,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn prostrate_queue(
    card_id: CardId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgraded = card.upgrades > 0;
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainBlock {
            amount: required_block(definition)?,
        },
        InternalAction::GainMantra {
            amount: if upgraded { 3 } else { 2 },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn equilibrium_queue(
    card_id: CardId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainBlock {
            amount: required_block(definition)? + if card.upgrades > 0 { 3 } else { 0 },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn fasting_queue(card_id: CardId, card: CardInstance) -> SimResult<VecDeque<InternalAction>> {
    let magic = 3 + i32::from(card.upgrades > 0);
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainStrength { amount: magic },
        InternalAction::GainDexterity { amount: magic },
        InternalAction::GainFasting { amount: 1 },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn hologram_queue(
    _state: &CombatState,
    card_id: CardId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let block = required_block(definition)? + 2 * i32::from(card.upgrades > 0);
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainBlock { amount: block },
        InternalAction::AwaitDiscardSelect {
            source_card_id: card_id,
            purpose: crate::combat::DiscardSelectPurpose::HologramReturnToHand,
        },
    ]))
}

fn poisoned_stab_queue(
    card_id: CardId,
    target: MonsterId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let damage = required_damage(definition)? + 2 * i32::from(card.upgrades > 0);
    let poison = 3 + i32::from(card.upgrades > 0);
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
        InternalAction::ApplyPoison {
            target,
            amount: poison,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn compile_driver_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let amount = required_damage(definition)? + 3 * i32::from(card.upgrades > 0);
    let mut seen_lightning = false;
    let mut seen_frost = false;
    let mut seen_dark = false;
    for orb in &state.orbs {
        match orb {
            crate::combat::CombatOrb::Lightning => seen_lightning = true,
            crate::combat::CombatOrb::Frost => seen_frost = true,
            crate::combat::CombatOrb::Dark { .. } => seen_dark = true,
        }
    }
    let draw_count = usize::from(seen_lightning) + usize::from(seen_frost) + usize::from(seen_dark);
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount,
            },
        },
    ]);
    if draw_count > 0 {
        queue.push_back(InternalAction::DrawCardsWhilePlayedCardIsInLimbo {
            card_id,
            count: draw_count,
        });
    }
    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });
    Ok(queue)
}

fn bowling_bash_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let amount = required_damage(definition)? + 3 * i32::from(card.upgrades > 0);
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
    ]);
    for _ in state.monsters.iter().filter(|monster| monster.alive) {
        queue.push_back(InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount,
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

fn like_water_queue(card_id: CardId, card: CardInstance) -> SimResult<VecDeque<InternalAction>> {
    let amount = 5 + 2 * i32::from(card.upgrades > 0);
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainLikeWater { amount },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn beam_cell_queue(
    card_id: CardId,
    target: MonsterId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let damage = definition.values.damage.ok_or(SimError::InvalidState(
        "Beam Cell definition is missing damage",
    ))? + if card.upgrades > 0 { 1 } else { 0 };
    let vulnerable = definition.values.vulnerable.ok_or(SimError::InvalidState(
        "Beam Cell definition is missing Vulnerable",
    ))? + if card.upgrades > 0 { 1 } else { 0 };
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
        InternalAction::ApplyVulnerable {
            target,
            amount: vulnerable,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn crush_joints_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = card.upgrades;
    let amount = attack_damage_with_strike_dummy(state, definition)?
        + synthetic_any_color_upgrade_damage(definition, upgrades);
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount,
            },
        },
    ]);
    // CrushJointsAction: if cardsPlayedThisCombat[-2] is a Skill, apply Vulnerable.
    if state.last_played_card_type == Some(CardType::Skill) {
        queue.push_back(InternalAction::ApplyVulnerable {
            target,
            amount: if upgrades > 0 { 2 } else { 1 },
        });
    }
    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });
    Ok(queue)
}

fn go_for_the_eyes_queue(
    _state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let damage = definition.values.damage.ok_or(SimError::InvalidState(
        "Go for the Eyes definition is missing damage",
    ))? + if card.upgrades > 0 { 1 } else { 0 };
    let mut queue = VecDeque::from([
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
    ]);
    queue.push_back(InternalAction::ApplyWeakIfTargetAttacking {
        target,
        amount: if card.upgrades > 0 { 2 } else { 1 },
    });
    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });
    Ok(queue)
}

fn just_lucky_queue(
    card_id: CardId,
    target: MonsterId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let damage = definition.values.damage.ok_or(SimError::InvalidState(
        "Just Lucky definition is missing damage",
    ))? + if card.upgrades > 0 { 1 } else { 0 };
    let block = definition.values.block.ok_or(SimError::InvalidState(
        "Just Lucky definition is missing block",
    ))? + if card.upgrades > 0 { 1 } else { 0 };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::AwaitDrawSelect {
            source_card_id: card_id,
            purpose: crate::combat::DrawSelectPurpose::Scry,
        },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: damage,
            },
        },
        InternalAction::GainBlock { amount: block },
    ]))
}

fn charge_battery_queue(
    card_id: CardId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::GainBlock {
            amount: required_block(definition)? + if card.upgrades > 0 { 3 } else { 0 },
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
            amount: required_block(definition)?,
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
            amount: required_vulnerable(definition)?,
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
            amount: required_damage(definition)?,
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

fn creative_ai_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainCreativeAI { amount: 1 },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn crescendo_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    let _ = upgrades;
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::EnterWrath,
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn empty_body_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // EmptyBody.baseBlock is 7; upgradeBlock(3). ChangeStance(Neutral).
    // Same-stance Neutral is a no-op; leaving Calm grants energy via onExitStance.
    let block = required_block(definition)? + if upgrades > 0 { 3 } else { 0 };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainBlock { amount: block },
        InternalAction::ExitCalm,
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn judgement_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // Judgement.baseMagicNumber is 30; upgradeMagicNumber(10).
    let threshold = 30 + if upgrades > 0 { 10 } else { 0 };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::ExecuteJudgement { target, threshold },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn conclude_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // Conclude.upgradeDamage(4). DamageAllEnemiesAction resolves before the
    // separately queued PressEndTurnButtonAction.
    let damage = required_damage(definition)? + if upgrades > 0 { 4 } else { 0 };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DealDamageAll {
            source: card_id,
            amount: damage,
        },
        // PressEndTurnButtonAction is already queued before UseCardAction, but
        // the request does not execute end-turn hand discard synchronously.
        InternalAction::ForceEndTurn,
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
        InternalAction::SettleForcedEndTurn,
    ]))
}

fn bane_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    let amount = required_damage(definition)? + if upgrades > 0 { 3 } else { 0 };
    let info = DamageInfo {
        source: DamageSource::Card(card_id),
        target,
        amount,
    };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DealDamage { info },
        // BaneAction.hasPower("Poison") is evaluated by this action, after the
        // first DamageAction and any earlier queued state changes have resolved.
        InternalAction::DealBaneDamageIfPoisoned { info },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn leap_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // Leap.upgradeBlock(3).
    let block = required_block(definition)? + if upgrades > 0 { 3 } else { 0 };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainBlock { amount: block },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn leg_sweep_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // LegSweep.upgradeBlock(3) / upgradeMagicNumber(1).
    let block = required_block(definition)? + if upgrades > 0 { 3 } else { 0 };
    let weak = 2 + i32::from(upgrades > 0);
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::ApplyWeak {
            target,
            amount: weak,
        },
        InternalAction::GainBlock { amount: block },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn fear_no_evil_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // FearNoEvil.upgradeDamage(3). ChangeStance(Calm) if the enemy intends to attack.
    let damage = required_damage(definition)? + if upgrades > 0 { 3 } else { 0 };
    let enter_calm = state
        .monsters
        .iter()
        .find(|monster| monster.id == target)
        .is_some_and(|monster| monster_intent_is_attack(monster.intent));
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: damage,
            },
        },
    ]);
    if enter_calm {
        queue.push_back(InternalAction::EnterCalm);
    }
    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });
    Ok(queue)
}

pub(crate) fn monster_intent_is_attack(intent: crate::MonsterIntent) -> bool {
    use crate::MonsterIntent as I;
    match intent {
        I::Attack { .. }
        | I::AttackMultiple { .. }
        | I::AttackApplyPlayerWeak { .. }
        | I::AttackApplyPlayerVulnerable { .. }
        | I::AttackApplyPlayerWeakAndVulnerable { .. }
        | I::AttackApplyPlayerFrailAndVulnerable { .. }
        | I::AttackApplyPlayerFrailAndWeak { .. }
        | I::AttackApplyPlayerFrail { .. }
        | I::AttackHealSelf { .. }
        | I::AttackMultipleApplyPlayerWeak { .. }
        | I::AttackAddVoidToDraw { .. }
        | I::AttackMultipleAddDazedToDiscard { .. }
        | I::AttackAddSlimedToDiscard { .. }
        | I::AttackAddWoundsToDiscard { .. }
        | I::AttackStealGold { .. }
        | I::AttackMultipleUpgradeBurns { .. }
        | I::AddBurnToDiscardAndDraw { .. } => true,
        I::AttackAndBlock { damage, .. } | I::AddBurnToDiscard { damage, .. } => damage > 0,
        _ => false,
    }
}

fn darkness_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::ChannelDark,
    ]);
    if upgrades > 0 {
        // Darkness.use queues DarkImpulseAction only when upgraded.
        queue.push_back(InternalAction::DarkImpulse);
    }
    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: card_move_destination(definition),
    });
    Ok(queue)
}

fn static_discharge_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // StaticDischarge.baseMagicNumber is 1; upgradeMagicNumber(1).
    let amount = 1 + i32::from(upgrades > 0);
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainStaticDischarge { amount },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn boot_sequence_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // BootSequence.baseBlock is 10; upgradeBlock(3). Innate + Exhaust.
    let block = required_block(definition)? + if upgrades > 0 { 3 } else { 0 };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainBlock { amount: block },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn evaluate_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // Evaluate.baseBlock is 6; upgradeBlock(4). Always shuffles Insight.
    let block = required_block(definition)? + if upgrades > 0 { 4 } else { 0 };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainBlock { amount: block },
        InternalAction::AddGeneratedCardToDrawPileRandomSpot {
            content_id: crate::content::cards::INSIGHT_ID,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn pray_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // Pray.baseMagicNumber is 3; upgradeMagicNumber(1). Always shuffles Insight.
    let mantra = 3 + i32::from(upgrades > 0);
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainMantra { amount: mantra },
        InternalAction::AddGeneratedCardToDrawPileRandomSpot {
            content_id: crate::content::cards::INSIGHT_ID,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn insight_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // Insight.baseMagicNumber is 2; upgradeMagicNumber(1).
    let draw = 2 + i32::from(upgrades > 0);
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DrawCards {
            count: draw as usize,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn after_image_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    // AfterImage.use applies AfterImagePower(1). Upgrade only sets isInnate.
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainAfterImage { amount: 1 },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn recursion_queue(
    _state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    // SpendCardEnergy resolves the effective combat-local cost, including
    // Recursion+'s zero base cost and a later Confusion/Madness temp cost.
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::RecurseRightmostOrb,
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn capacitor_queue(
    state: &CombatState,
    card_id: CardId,
    _definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // Capacitor.baseMagicNumber is 2; upgradeMagicNumber(1).
    let amount = 2 + i32::from(upgrades > 0);
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::IncreaseMaxOrbs { amount },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn caltrops_queue(
    state: &CombatState,
    card_id: CardId,
    _definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // Caltrops.baseMagicNumber is 3; upgradeMagicNumber(2).
    let amount = 3 + if upgrades > 0 { 2 } else { 0 };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainThorns { amount },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn coolheaded_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // Coolheaded.baseMagicNumber is 1; upgradeMagicNumber(1).
    let draw = 1 + i32::from(upgrades > 0);
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::ChannelFrost,
        InternalAction::DrawCards {
            count: draw as usize,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn storm_queue(card_id: CardId, _card: CardInstance) -> SimResult<VecDeque<InternalAction>> {
    // Storm.use applies StormPower(baseMagicNumber=1). Upgrade only sets isInnate.
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 1 },
        InternalAction::GainStorm { amount: 1 },
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
    // AbstractDungeon copies colorlessCardPool into srcColorlessCardPool with
    // CardGroup.addToBottom, which prepends each entry. Combat Magnetism rolls
    // that source pool after HEALING-tagged Bandage Up is filtered the same way
    // discovery generation filters it (FIDL00226 Dramatic Entrance oracles).
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
            amount: required_damage(definition)?,
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
            amount: required_damage(definition)?,
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
    _state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::ExhaustAllNonAttackCards {
            excluded_card_id: card_id,
        },
    ]);
    queue.extend([
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount: required_damage(definition)?,
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
    let block_per_card = required_block(definition)?;
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
    let _ = state; // hand size is resolved at action time (Double Tap copies).
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::ResolveFiendFire {
            source_card_id: card_id,
            target,
            amount: required_damage(definition)?,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
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

fn skim_queue(card_id: CardId, definition: &CardDefinition) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DrawCards { count: 3 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
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
                amount: required_damage(definition)?,
            },
        },
        InternalAction::ApplyVulnerable {
            target,
            amount: required_vulnerable(definition)?,
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
                amount: required_damage(definition)?
                    + if card.upgrades > 0 && definition.id == ANGER_ID {
                        // Anger.upgradeDamage(2) when the instance is upgraded
                        // but still carries the base content id.
                        2
                    } else {
                        0
                    },
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

fn halt_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // Halt.applyPowers computes its base block and 9/14 additional Wrath block
    // separately, including Dexterity/Frail on each value. HaltAction then queues
    // exactly one GainBlockAction for their precomputed sum.
    let base = required_block(definition)? + i32::from(upgrades > 0);
    let base = crate::power::calculate_block(base, state.player.powers);
    let amount = if state.player.powers.wrath > 0 {
        let additional = if upgrades > 0 { 14 } else { 9 };
        base.checked_add(crate::power::calculate_block(
            additional,
            state.player.powers,
        ))
        .ok_or(SimError::InvalidState("Halt block overflows i32"))?
    } else {
        base
    };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::GainPrecomputedCardBlock { amount },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn flying_sleeves_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    let amount =
        required_damage(definition)? + synthetic_any_color_upgrade_damage(definition, upgrades);
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount,
            },
        },
        InternalAction::DealDamage {
            info: DamageInfo {
                source: DamageSource::Card(card_id),
                target,
                amount,
            },
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn scrape_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    let damage = required_damage(definition)? + if upgrades > 0 { 3 } else { 0 };
    let draw = 4 + usize::from(upgrades > 0);
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
        InternalAction::DrawThenScrapeDiscard { count: draw },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn sweeping_beam_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    let damage =
        required_damage(definition)? + synthetic_any_color_upgrade_damage(definition, upgrades);
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DealDamageAll {
            source: card_id,
            amount: damage,
        },
        InternalAction::DrawCards { count: 1 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
}

fn dagger_spray_queue(
    state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.upgrades)
        .unwrap_or(0);
    // DaggerSpray.upgradeDamage(2) for each of the two all-enemy hits.
    let damage = required_damage(definition)? + if upgrades > 0 { 2 } else { 0 };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendCardEnergy { card_id },
        InternalAction::DealDamageAll {
            source: card_id,
            amount: damage,
        },
        InternalAction::DealDamageAll {
            source: card_id,
            amount: damage,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
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
            amount: required_damage(definition)?,
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
    // Trace-backed surface: single all-enemy hit + Burn in discard. Full
    // ImmolateAction non-Attack exhaust × damage-per-exhaust is not what
    // CommunicationMod witnesses show (FIDL00238 step 196 keeps skills).
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
            amount: required_damage(definition)?,
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
            amount: required_damage(definition)?,
        },
    ]);

    for monster in state.monsters.iter().filter(|monster| monster.alive) {
        queue.push_back(InternalAction::ApplyVulnerable {
            target: monster.id,
            amount: required_vulnerable(definition)?,
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
                amount: required_damage(definition)?,
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
            amount: required_vulnerable(definition)?,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn malaise_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    card: CardInstance,
) -> SimResult<VecDeque<InternalAction>> {
    // MalaiseAction: X = energyOnUse; +2 Chemical X; +1 if upgraded. If X>0,
    // ApplyPower Strength -X then Weak X; spend energy unless freeToPlayOnce.
    let mut x = state.player.energy;
    if state.relics.contains(&Relic::ChemicalX) {
        x = x.saturating_add(CHEMICAL_X_BONUS_X);
    }
    if card.upgrades > 0 {
        x = x.saturating_add(1);
    }
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: if card.free_to_play_once {
                0
            } else {
                state.player.energy
            },
        },
    ]);
    if x > 0 {
        queue.push_back(InternalAction::ReduceMonsterStrength { target, amount: x });
        queue.push_back(InternalAction::ApplyWeak { target, amount: x });
    }
    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: CardPile::ExhaustPile,
    });
    Ok(queue)
}

fn eviscerate_queue(
    _state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let damage = required_damage(definition)? + if card.upgrades > 0 { 2 } else { 0 };
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);
    for _ in 0..3 {
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
        to: CardPile::DiscardPile,
    });
    Ok(queue)
}

fn skewer_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let uses = x_cost_uses_with_chemical_x(state).max(0);
    let damage = required_damage(definition)? + if card.upgrades > 0 { 3 } else { 0 };
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: if card.free_to_play_once {
                0
            } else {
                state.player.energy
            },
        },
    ]);
    for _ in 0..uses {
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
        to: CardPile::DiscardPile,
    });
    Ok(queue)
}

fn steam_barrier_queue(
    card_id: CardId,
    _card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::ResolveSteamBarrier { card_id },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn follow_up_queue(
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let damage = required_damage(definition)? + if card.upgrades > 0 { 4 } else { 0 };
    let mut queue = VecDeque::from([
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
    ]);
    queue.push_back(InternalAction::ResolveFollowUpEnergy {
        should_gain: state.last_played_card_type == Some(CardType::Attack),
    });
    queue.push_back(InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: CardPile::DiscardPile,
    });
    Ok(queue)
}

fn streamline_queue(
    card_id: CardId,
    target: MonsterId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let damage = required_damage(definition)? + if card.upgrades > 0 { 5 } else { 0 };
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
        InternalAction::ReduceHandCardCostForCombat { card_id, amount: 1 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn wallop_queue(
    card_id: CardId,
    target: MonsterId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let damage = required_damage(definition)? + if card.upgrades > 0 { 3 } else { 0 };
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DealDamageAndGainBlockUnblocked {
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

fn storm_of_steel_queue(
    _state: &CombatState,
    card_id: CardId,
    card: CardInstance,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::ResolveStormOfSteel {
            source_card_id: card_id,
            upgraded: card.upgrades > 0,
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

    let damage = required_damage(definition)?;
    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: x },
    ]);

    // Whirlwind.use() only addToBots WhirlwindAction. RagePower.onUseCard then
    // addToBots GainBlockAction (UseCardAction constructor) before that wrapper
    // addToBots DamageAllEnemiesAction. Immediate Strike damage stays ahead of
    // Rage; these deferred hits do not (FIDL01782 Spiker thorns).
    if state.player.temp_rage_block > 0 {
        queue.push_back(InternalAction::GainBlockDirect {
            amount: state.player.temp_rage_block,
        });
    }

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

    // STS removes the played card from hand before MakeTempCardInHandAction
    // resolves (cardInUse / limbo). Generating while Transmutation still occupies
    // a hand slot under-fills by one (FIDL00413 X-cost → PLAY 10).
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: x },
        InternalAction::AddRandomColorlessCardsToHandWhileSourceInLimbo {
            source_card_id: card_id,
            count: uses.max(0) as usize,
            temp_cost: Some(0),
            upgrade: definition.id == TRANSMUTATION_PLUS_ID,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        },
    ]))
}

fn havoc_queue(
    state: &mut CombatState,
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

    let mut queue = VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
    ]);

    // Havoc's source settlement is normally a discard MoveCard; Corruption
    // rewrites that move to exhaust.
    let source_exhausts = definition.keywords.exhaust
        || (definition.card_type == CardType::Skill && state.player.powers.corruption > 0);
    let settle = InternalAction::MoveCard {
        card_id,
        from: CardPile::Hand,
        to: if source_exhausts {
            CardPile::ExhaustPile
        } else {
            CardPile::DiscardPile
        },
    };
    // Havoc.use constructs PlayTopCardAction(getRandomMonster(...), exhaust).
    // Corruption burns that roll at use-time before self-exhaust / Dead Branch
    // (FIDL00441). Empty-draw Corruption still burns before refill (FIDL00428).
    // Non-Corruption uses PlayTop-time random_living_target (Hex mid-insert
    // order on FIDL00428 with Letter Opener).
    let (play_top_target, random_living_target) = if source_exhausts {
        let rolled = target.or_else(|| {
            let living: Vec<_> = state
                .monsters
                .iter()
                .filter(|monster| monster.alive)
                .map(|monster| monster.id)
                .collect();
            if living.is_empty() {
                None
            } else {
                let index = state
                    .rng
                    .card_random_rng
                    .random_int((living.len() - 1) as i32) as usize;
                living.get(index).copied()
            }
        });
        (rolled, false)
    } else {
        (target, true)
    };
    let play_top = InternalAction::PlayTopDrawCard {
        target: play_top_target,
        exhaust_played_card: true,
        random_living_target,
    };
    // Havoc.use addToBot's PlayTopCardAction before AbstractPlayer.useCard
    // addToBottom's the outer UseCardAction. PlayTop removes the selected card
    // into limbo and returns ResolveTopDrawCard as a card-queue follow-up; the
    // outer source MoveCard therefore runs after extraction but before the
    // forced card is serviced. This ordering is unconditional in the target.
    queue.push_back(play_top);
    queue.push_back(settle);

    Ok(queue)
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
        // UseCardAction temporarily removes the played card from hand while
        // DrawCardAction resolves. This matters at the 10-card hand limit:
        // Warcry draws before its PutOnDeckAction selection opens, so the
        // source must not consume a hand slot during that draw.
        InternalAction::DrawCardsWhilePlayedCardIsInLimbo {
            card_id,
            count: warcry_draw_count(definition),
        },
        InternalAction::AwaitHandSelect {
            source_card_id: card_id,
            purpose: HandSelectPurpose::WarcryPutOnDraw,
        },
    ]))
}

fn thinking_ahead_queue(
    _state: &CombatState,
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        // Thinking Ahead also draws before opening its hand selection. Mirror
        // UseCardAction's temporary cardInUse/limbo slot so a full hand can
        // receive both drawn cards before the selection settles.
        InternalAction::DrawCardsWhilePlayedCardIsInLimbo { card_id, count: 2 },
        // ThinkingAhead.use() checks hand.size() while the source is still in
        // hand, so PutOnDeckAction is always queued. After limbo + draw, the
        // select opens when the remaining hand is larger than 1.
        InternalAction::AwaitHandSelect {
            source_card_id: card_id,
            purpose: HandSelectPurpose::ThinkingAheadPutOnDraw,
        },
    ]))
}

fn draw_pile_cards_of_type(state: &CombatState, card_type: CardType) -> Vec<CardId> {
    state
        .piles
        .draw_pile
        .iter()
        .filter(|card| {
            get_card_definition(card.content_id)
                .is_some_and(|definition| definition.card_type == card_type)
        })
        .map(|card| card.id)
        .collect()
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

    match draw_pile_cards_of_type(state, CardType::Skill).as_slice() {
        [selected_card_id] => {
            queue.extend([
                InternalAction::MoveCard {
                    card_id: *selected_card_id,
                    from: CardPile::DrawPile,
                    to: CardPile::Hand,
                },
                InternalAction::MoveCard {
                    card_id,
                    from: CardPile::Hand,
                    to: card_move_destination(definition),
                },
            ]);
        }
        [] => queue.push_back(InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        }),
        _ => queue.push_back(InternalAction::AwaitDrawSelect {
            source_card_id: card_id,
            purpose: crate::combat::DrawSelectPurpose::SecretTechniqueSkillToHand,
        }),
    }

    Ok(queue)
}

fn blasphemy_queue(
    card_id: CardId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    // Blasphemy: ChangeStance(Divinity) + EndTurnDeathPower, then exhaust.
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::EnterDivinity,
        InternalAction::ApplyEndTurnDeath,
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        },
    ]))
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

    match draw_pile_cards_of_type(state, CardType::Attack).as_slice() {
        [selected_card_id] => {
            queue.extend([
                InternalAction::MoveCard {
                    card_id: *selected_card_id,
                    from: CardPile::DrawPile,
                    to: CardPile::Hand,
                },
                InternalAction::MoveCard {
                    card_id,
                    from: CardPile::Hand,
                    to: card_move_destination(definition),
                },
            ]);
        }
        [] => queue.push_back(InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
        }),
        _ => queue.push_back(InternalAction::AwaitDrawSelect {
            source_card_id: card_id,
            purpose: crate::combat::DrawSelectPurpose::SecretWeaponAttackToHand,
        }),
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

fn dual_wield_force_play_defers_source_settlement(
    state: &CombatState,
    source_card_id: CardId,
) -> bool {
    // Count Attack/Power cards that would remain eligible after Dual Wield leaves
    // the hand (force-play stages the source in hand briefly).
    let eligible = state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != source_card_id)
        .filter(|card| {
            get_card_definition(card.content_id).is_some_and(|definition| {
                matches!(definition.card_type, CardType::Attack | CardType::Power)
            })
        })
        .count();
    eligible > 1
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
    state: &CombatState,
    card_id: CardId,
    target: MonsterId,
    definition: &CardDefinition,
) -> SimResult<VecDeque<InternalAction>> {
    let damage = attack_damage_with_strike_dummy(state, definition)?;
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
            amount: required_block(definition)?,
        },
        InternalAction::DrawCardsWhilePlayedCardIsInLimbo { card_id, count: 1 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
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
            amount: required_block(definition)?,
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
            damage: required_damage(definition)?,
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
            amount: required_block(definition)?,
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
    // Enlightenment checks the card's current cost, including dynamic
    // reductions such as Blood for Blood's tookDamage counter. Using the
    // printed definition cost incorrectly re-inflates an already-free BfB.
    crate::combat::cost::effective_card_cost(card).unwrap_or(i32::MAX)
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
            amount: required_block(definition)?,
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
            amount: required_block(definition)?,
        },
    ]);

    if state.piles.hand.iter().any(|card| card.id != card_id) {
        if definition.id == TRUE_GRIT_PLUS_ID {
            queue.push_back(InternalAction::AwaitExhaustSelect {
                source_card_id: card_id,
                purpose: crate::combat::ExhaustSelectPurpose::TrueGritExhaustOne,
            });
            return Ok(queue);
        } else if other_hand_cards(state, card_id).len() == 1 {
            // Target ExhaustAction takes its non-random "exhaust all" path
            // when this is the only card left in hand, so it does not advance
            // cardRandomRng for unupgraded True Grit.
            let target_card_id = other_hand_cards(state, card_id)[0];
            queue.push_back(InternalAction::MoveCard {
                card_id: target_card_id,
                from: CardPile::Hand,
                to: CardPile::ExhaustPile,
            });
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

    let other_hand_card_ids = other_hand_cards(state, card_id);
    if other_hand_card_ids.is_empty() {
        queue.push_back(InternalAction::DrawCards { count: draw_count });
        queue.push_back(InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        });
        return Ok(queue);
    }

    if other_hand_card_ids.len() == 1 {
        // Target ExhaustAction(1, false) exhausts all cards without opening
        // HandCardSelectScreen when the played card is already in limbo and
        // only one other card remains in hand.
        queue.push_back(InternalAction::MoveCard {
            card_id: other_hand_card_ids[0],
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        });
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
            amount: required_damage(definition)?,
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
            amount: required_damage(definition)?,
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
            amount: required_block(definition)?,
        },
        InternalAction::RemoveCard {
            card_id,
            from: CardPile::Hand,
        },
    ]))
}

fn pommel_strike_queue(
    state: &CombatState,
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
                amount: attack_damage_with_strike_dummy(state, definition)?,
            },
        },
        InternalAction::DrawCardsWhilePlayedCardIsInLimbo {
            card_id,
            count: draw_count,
        },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
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
    // Draw while the played card is in limbo (not occupying a hand slot). Plain
    // DrawCards at max hand size skips every draw, then exhausts the source and
    // leaves the hand one short (archived schema-v0 witness
    // random-fidelity-809d00fe, PLAY 10 after Master of Strategy under Runic Pyramid).
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy {
            amount: i32::from(definition.cost),
        },
        InternalAction::DrawCardsWhilePlayedCardIsInLimbo {
            card_id,
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
        InternalAction::DrawCardsWhilePlayedCardIsInLimboWithoutEvolve {
            card_id,
            count: battle_trance_draw_count(definition),
        },
        // No Draw must be active before Corruption/exhaust settles the source,
        // or Dark Embrace would draw after Battle Trance's own draws.
        InternalAction::SetCannotDraw,
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: card_move_destination(definition),
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
    // Target Offering.use() addToBot's LoseHP → GainEnergy → DrawCard while the
    // played card is in limbo (not occupying a hand slot), then UseCardAction
    // exhausts. Hex onUseCard inserts Dazed after those draws (push_follow_up
    // places Hex before MoveCard). Plain DrawCards after MoveCard let Hex steal
    // a draw (18-33-54); plain DrawCards before Move with the source still in
    // hand hits max-hand and skips a draw (live-regression-2026-07-02).
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
        InternalAction::DrawCardsWhilePlayedCardIsInLimbo {
            card_id,
            count: draw_count,
        },
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
            // Mirror AbstractMonster.isAttacking / CM attack intent buckets.
            // Nemesis pure burn and Time Eater Ripple carry zero damage and are
            // DEBUFF / DEFEND_DEBUFF rather than attacks.
            monster_intent_is_attack(monster.intent)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::CombatState;
    use crate::content::monsters::NEMESIS_ID;
    use crate::ids::MonsterId;
    use crate::MonsterIntent;

    #[test]
    fn havoc_always_extracts_top_before_outer_source_settlement() {
        for top in [DEFEND_R_ID, POMMEL_STRIKE_ID, DEEP_BREATH_ID] {
            let havoc_id = CardId::new(100);
            let mut state = CombatState::initial_fixture();
            state.piles.hand = vec![CardInstance::new(havoc_id, HAVOC_ID)];
            state.piles.draw_pile = vec![CardInstance::new(CardId::new(101), top)];
            state.piles.discard_pile.clear();
            let target = (top == POMMEL_STRIKE_ID).then_some(state.monsters[0].id);
            let definition = get_card_definition(HAVOC_ID).expect("Havoc definition");

            let queue = havoc_queue(&mut state, havoc_id, definition, target).expect("Havoc queue");
            let source_move = queue
                .iter()
                .position(|action| {
                    matches!(
                        action,
                        InternalAction::MoveCard { card_id, .. } if *card_id == havoc_id
                    )
                })
                .expect("Havoc source movement");
            let play_top = queue
                .iter()
                .position(|action| matches!(action, InternalAction::PlayTopDrawCard { .. }))
                .expect("Havoc PlayTop");
            assert!(play_top < source_move);
        }
    }

    #[test]
    fn spot_weakness_ignores_pure_debuff_and_defend_debuff_intents() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].content_id = NEMESIS_ID;
        let target = state.monsters[0].id;
        state.monsters[0].intent = MonsterIntent::AddBurnToDiscard {
            count: 3,
            damage: 0,
        };
        assert!(
            !monster_intends_attack(&state, target),
            "Nemesis burn DEBUFF must not count as attack intent"
        );
        state.monsters[0].intent = MonsterIntent::AttackAndBlock {
            damage: 0,
            block: 20,
        };
        assert!(
            !monster_intends_attack(&state, target),
            "Time Eater Ripple DEFEND_DEBUFF must not count as attack intent"
        );
        state.monsters[0].intent = MonsterIntent::AddBurnToDiscard {
            count: 3,
            damage: 10,
        };
        assert!(
            monster_intends_attack(&state, target),
            "burn with damage is ATTACK_DEBUFF"
        );
        state.monsters[0].intent = MonsterIntent::AttackAndBlock {
            damage: 12,
            block: 5,
        };
        assert!(
            monster_intends_attack(&state, target),
            "attack+block with damage is attack intent"
        );
    }

    #[test]
    fn steam_barrier_uses_common_rarity() {
        assert_eq!(
            get_card_definition(STEAM_BARRIER_ANY_COLOR_ID)
                .expect("Steam Barrier definition")
                .rarity,
            Some(crate::card::CardRarity::Common)
        );
    }

    #[test]
    fn eviscerate_draw_and_manual_discard_update_turn_cost() {
        let eviscerate = CardInstance::new(CardId::new(100), EVISCERATE_ANY_COLOR_ID);
        let discard = CardInstance::new(CardId::new(101), STRIKE_R_ID);
        let mut state = CombatState::initial_fixture();
        state.total_discarded_this_turn = 1;
        state.piles.hand = vec![discard];
        state.piles.draw_pile = vec![eviscerate];
        state.piles.discard_pile.clear();

        crate::combat::draw::draw_cards_with_combat_rng(&mut state, 1)
            .expect("Eviscerate draw resolves");
        assert_eq!(
            crate::combat::cost::effective_card_cost(&state.piles.hand[1]),
            Ok(2)
        );

        let transition = crate::combat::transition::process_internal_queue(
            &state,
            VecDeque::from([InternalAction::ManualDiscardCard {
                card_id: CardId::new(101),
            }]),
        )
        .expect("manual discard resolves callbacks");
        assert_eq!(transition.state.total_discarded_this_turn, 2);
        let eviscerate = transition
            .state
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == EVISCERATE_ANY_COLOR_ID)
            .expect("Eviscerate remains in hand");
        assert_eq!(crate::combat::cost::effective_card_cost(eviscerate), Ok(1));
        assert!(eviscerate.temp_cost_turn_only);
    }

    #[test]
    fn upgraded_storm_of_steel_manually_discards_and_makes_upgraded_shivs() {
        let mut storm = CardInstance::new(CardId::new(100), STORM_OF_STEEL_ANY_COLOR_ID);
        storm.upgrades = 1;
        let eviscerate = CardInstance::new(CardId::new(101), EVISCERATE_ANY_COLOR_ID);
        let strike = CardInstance::new(CardId::new(102), STRIKE_R_ID);
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![storm, eviscerate, strike];
        state.piles.discard_pile.clear();
        let definition =
            get_card_definition(STORM_OF_STEEL_ANY_COLOR_ID).expect("Storm definition");
        let queue = storm_of_steel_queue(&state, storm.id, storm, definition).expect("Storm queue");

        let transition = crate::combat::transition::process_internal_queue(&state, queue)
            .expect("Storm resolves");
        assert_eq!(transition.state.total_discarded_this_turn, 2);
        let shivs = transition
            .state
            .piles
            .hand
            .iter()
            .filter(|card| card.content_id == SHIV_ANY_COLOR_ID)
            .collect::<Vec<_>>();
        assert_eq!(shivs.len(), 2);
        assert!(shivs.iter().all(|card| card.upgrades == 1));
        let eviscerate = transition
            .state
            .piles
            .discard_pile
            .iter()
            .find(|card| card.content_id == EVISCERATE_ANY_COLOR_ID)
            .expect("Eviscerate was manually discarded");
        assert_eq!(crate::combat::cost::effective_card_cost(eviscerate), Ok(1));

        let shiv_id = shivs[0].id;
        let target = transition.state.monsters[0].id;
        let hp_before = transition.state.monsters[0].hp;
        let (_, queue) =
            play_card_queue(&transition.state, shiv_id, Some(target)).expect("upgraded Shiv queue");
        let after_shiv =
            crate::combat::transition::process_internal_queue(&transition.state, queue)
                .expect("upgraded Shiv resolves");
        assert_eq!(after_shiv.state.monsters[0].hp, hp_before - 6);
    }

    #[test]
    fn streamline_preserves_zero_turn_override_over_reduced_combat_cost() {
        let mut streamline = CardInstance::new(CardId::new(100), STREAMLINE_ANY_COLOR_ID);
        streamline.temp_cost = Some(0);
        streamline.temp_cost_turn_only = true;
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![streamline];
        state.piles.discard_pile.clear();
        let target = state.monsters[0].id;
        let definition =
            get_card_definition(STREAMLINE_ANY_COLOR_ID).expect("Streamline definition");
        let queue = streamline_queue(streamline.id, target, streamline, definition)
            .expect("Streamline queue");

        let transition = crate::combat::transition::process_internal_queue(&state, queue)
            .expect("Streamline resolves");
        let streamline = transition
            .state
            .piles
            .discard_pile
            .iter()
            .find(|card| card.content_id == STREAMLINE_ANY_COLOR_ID)
            .expect("Streamline discarded");
        assert_eq!(crate::combat::cost::effective_card_cost(streamline), Ok(0));
        assert_eq!(crate::combat::cost::printed_card_cost(streamline), Ok(1));
        assert!(streamline.temp_cost_turn_only);
        assert_eq!(streamline.combat_cost_under_turn_override, Some(1));
    }

    #[test]
    fn copied_dynamic_cards_recompute_at_each_card_use_boundary() {
        let target = MonsterId::new(1);

        let mut storm = CardInstance::new(CardId::new(100), STORM_OF_STEEL_ANY_COLOR_ID);
        storm.upgrades = 1;
        let mut storm_state = CombatState::initial_fixture();
        storm_state.duplication_potion_pending = true;
        storm_state.piles.hand = vec![
            storm,
            CardInstance::new(CardId::new(101), EVISCERATE_ANY_COLOR_ID),
            CardInstance::new(CardId::new(102), STRIKE_R_ID),
        ];
        storm_state.piles.discard_pile.clear();
        let storm_next = crate::combat::transition::apply_combat_action(
            &storm_state,
            crate::CombatAction::PlayCard {
                card_id: storm.id,
                target: None,
            },
        )
        .expect("copied Storm re-reads its replacement Shiv hand");
        assert_eq!(storm_next.total_discarded_this_turn, 4);
        assert_eq!(
            storm_next
                .piles
                .hand
                .iter()
                .filter(|card| card.content_id == SHIV_ANY_COLOR_ID && card.upgrades == 1)
                .count(),
            2
        );

        let steam = CardInstance::new(CardId::new(200), STEAM_BARRIER_ANY_COLOR_ID);
        let mut steam_state = CombatState::initial_fixture();
        steam_state.duplication_potion_pending = true;
        steam_state.piles.hand = vec![steam];
        steam_state.piles.discard_pile.clear();
        let steam_next = crate::combat::transition::apply_combat_action(
            &steam_state,
            crate::CombatAction::PlayCard {
                card_id: steam.id,
                target: None,
            },
        )
        .expect("copied Steam Barrier uses its reduced second value");
        assert_eq!(steam_next.player.block, 11);
        assert_eq!(
            steam_next.piles.discard_pile[0].steam_barrier_block_reduction,
            2
        );

        let follow_up = CardInstance::new(CardId::new(300), FOLLOW_UP_ANY_COLOR_ID);
        let mut follow_state = CombatState::initial_fixture();
        follow_state.player.energy = 2;
        follow_state.duplication_potion_pending = true;
        follow_state.piles.hand = vec![follow_up];
        follow_state.piles.discard_pile.clear();
        let follow_next = crate::combat::transition::apply_combat_action(
            &follow_state,
            crate::CombatAction::PlayCard {
                card_id: follow_up.id,
                target: Some(target),
            },
        )
        .expect("copied Follow-Up sees the original Attack before it");
        assert_eq!(follow_next.player.energy, 2);
        assert_eq!(follow_next.relic_counters.cards_played_this_turn, 2);
    }

    #[test]
    fn wallop_block_uses_ordinary_gain_block_semantics() {
        let wallop = CardInstance::new(CardId::new(100), WALLOP_ANY_COLOR_ID);
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![wallop];
        state.piles.discard_pile.clear();
        state.player.no_block_turns = 1;
        state.player.powers.juggernaut = 5;
        let target = state.monsters[0].id;
        let hp_before = state.monsters[0].hp;
        let definition = get_card_definition(WALLOP_ANY_COLOR_ID).expect("Wallop definition");
        let queue = wallop_queue(wallop.id, target, wallop, definition).expect("Wallop queue");

        let transition = crate::combat::transition::process_internal_queue(&state, queue)
            .expect("Wallop resolves");
        assert_eq!(transition.state.player.block, 0);
        assert_eq!(transition.state.monsters[0].hp, hp_before - 9);

        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![wallop];
        state.piles.discard_pile.clear();
        state.player.powers.juggernaut = 5;
        state.player.powers.dexterity = 2;
        state.player.powers.frail = 1;
        let target = state.monsters[0].id;
        let hp_before = state.monsters[0].hp;
        let queue = wallop_queue(wallop.id, target, wallop, definition).expect("Wallop queue");
        let transition = crate::combat::transition::process_internal_queue(&state, queue)
            .expect("Wallop and Juggernaut resolve");
        assert_eq!(transition.state.player.block, 9);
        assert_eq!(transition.state.monsters[0].hp, hp_before - 14);
    }

    #[test]
    fn card_effect_builders_reject_missing_required_values() {
        let state = CombatState::initial_fixture();
        let card_id = CardId::new(1);
        let target = MonsterId::new(1);

        let mut strike = *get_card_definition(STRIKE_R_ID).expect("Strike definition");
        strike.values.damage = None;
        assert_eq!(
            strike_queue(&state, card_id, target, &strike),
            Err(SimError::InvalidState(
                "card definition is missing required damage"
            ))
        );

        let mut defend = *get_card_definition(DEFEND_R_ID).expect("Defend definition");
        defend.values.block = None;
        assert_eq!(
            defend_queue(card_id, &defend),
            Err(SimError::InvalidState(
                "card definition is missing required block"
            ))
        );

        let mut bash = *get_card_definition(BASH_ID).expect("Bash definition");
        bash.values.vulnerable = None;
        assert_eq!(
            bash_queue(card_id, target, &bash),
            Err(SimError::InvalidState(
                "card definition is missing required vulnerable"
            ))
        );
    }
}
