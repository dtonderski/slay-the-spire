//! Hand-played Discovery post-pick RNG vs force-exhaust (Havoc) Discovery.
//!
//! Permanent oracles:
//! - `random-fidelity-f019eccf586137c4` (FIDL00226): hand-played Discovery then
//!   Magnetism must roll Dramatic Entrance (seven post-pick generations).
//! - `random-fidelity-1a50b5ada2264b05`: Havoc→Discovery then Infernal Blade
//!   must roll Blood for Blood (two generations + two settle draws).

use sts_core::content::cards::{
    get_card_definition, ARMAMENTS_ID, BLIND_ID, BLOOD_FOR_BLOOD_ID, DARK_SHACKLES_ID,
    DEEP_BREATH_ID, DRAMATIC_ENTRANCE_ID, ENLIGHTENMENT_ID, MAYHEM_ID, TRANSMUTATION_ID,
    TWIN_STRIKE_ID, WARCRY_ID,
};
use sts_core::content::shop_pool::{
    burn_all_discovery_card_choice_draws, burn_all_discovery_card_choice_generations,
    colorless_discovery_pool, ironclad_combat_attack_discovery_pool,
    ironclad_combat_discovery_pool,
};
use sts_core::ids::ContentId;
use sts_core::rng::StsRng;

fn disc_choices(rng: &mut StsRng, pool: &[ContentId]) -> Vec<ContentId> {
    let mut choices = Vec::new();
    while choices.len() < 3 {
        let idx = rng.random_int((pool.len() - 1) as i32) as usize;
        let id = pool[idx];
        if !choices.contains(&id) {
            choices.push(id);
        }
    }
    choices
}

fn magnetism(rng: &mut StsRng) -> ContentId {
    let pool: Vec<_> = colorless_discovery_pool()
        .into_iter()
        .filter(|id| get_card_definition(*id).is_some())
        .collect();
    pool[rng.random_int((pool.len() - 1) as i32) as usize]
}

/// Hand-played Discovery post-pick: seven full choice generations (FIDL00226).
#[test]
fn hand_played_discovery_seven_pick_gens_yield_dramatic_entrance() {
    let ic = ironclad_combat_discovery_pool();
    // card_random state immediately before PLAY Discovery on FIDL00226 step 24.
    let mut r = StsRng::from_raw_state(17743558243545444171, 9704550745339910859, 2);
    let vis = disc_choices(&mut r, ic);
    assert_eq!(vis, vec![WARCRY_ID, ARMAMENTS_ID, TWIN_STRIKE_ID]);
    for _ in 0..4 {
        let _ = disc_choices(&mut r, ic);
    }
    assert_eq!(r.counter(), 18, "open: 1 visible + 4 hidden generations");
    for _ in 0..7 {
        let _ = disc_choices(&mut r, ic);
    }
    let mag = magnetism(&mut r);
    assert_eq!(
        mag, DRAMATIC_ENTRANCE_ID,
        "hand-played Discovery pick must leave source-pool Magnetism on Dramatic Entrance"
    );
}

/// FIDL00226 keeps the selected hand-played action alive across later ENDs.
#[test]
fn hand_played_discovery_staged_settlement_matches_magnetism_sequence() {
    let ic = ironclad_combat_discovery_pool();
    let mut r = StsRng::from_raw_state(17743558243545444171, 9704550745339910859, 2);
    let _ = disc_choices(&mut r, ic);
    for _ in 0..4 {
        let _ = disc_choices(&mut r, ic);
    }
    for _ in 0..7 {
        let _ = disc_choices(&mut r, ic);
    }
    assert_eq!(magnetism(&mut r), DRAMATIC_ENTRANCE_ID);

    burn_all_discovery_card_choice_generations(&mut r, 3, 26);
    burn_all_discovery_card_choice_draws(&mut r, 1);
    assert_eq!(magnetism(&mut r), TRANSMUTATION_ID);
    assert_eq!(magnetism(&mut r), BLIND_ID);

    burn_all_discovery_card_choice_generations(&mut r, 3, 11);
    burn_all_discovery_card_choice_draws(&mut r, 2);
    assert_eq!(magnetism(&mut r), DARK_SHACKLES_ID);
    burn_all_discovery_card_choice_draws(&mut r, 1);
    assert_eq!(magnetism(&mut r), ENLIGHTENMENT_ID);
    burn_all_discovery_card_choice_draws(&mut r, 2);
    assert_eq!(magnetism(&mut r), MAYHEM_ID);
    burn_all_discovery_card_choice_draws(&mut r, 1);
    assert_eq!(magnetism(&mut r), DEEP_BREATH_ID);
}

/// Force-exhaust Discovery post-pick: two gens + two settle draws (1a50b5 oracle).
#[test]
fn force_exhaust_discovery_two_gens_two_settle_yield_blood_for_blood() {
    let ic = ironclad_combat_discovery_pool();
    let atk = ironclad_combat_attack_discovery_pool();
    // card_random state immediately before Havoc→Discovery open on 1a50b5 step 280.
    let mut r = StsRng::from_raw_state(15021999097217022216, 16315838232280718434, 0);
    let _ = disc_choices(&mut r, ic);
    for _ in 0..4 {
        let _ = disc_choices(&mut r, ic);
    }
    // Live open ends at counter 16 (rejection variance + open settlement).
    while r.counter() < 16 {
        let _ = r.random_int((ic.len() - 1) as i32);
    }
    burn_all_discovery_card_choice_generations(&mut r, 3, 2);
    burn_all_discovery_card_choice_draws(&mut r, 2);
    assert_eq!(r.counter(), 24);
    let card = atk[r.random_int((atk.len() - 1) as i32) as usize];
    assert_eq!(
        card, BLOOD_FOR_BLOOD_ID,
        "Havoc Discovery pick must leave Infernal Blade on Blood for Blood"
    );
}
