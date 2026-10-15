//! Source-backed robustness, not dedicated real-game interaction parity.
//! PC 12-18-2022 AbstractPlayer.<init> reduces potionSlots at A11+;
//! PotionBelt.onEquip adds two slots. JAR SHA256:
//! cfad868ac8d65a88e71a0bf096fb09f78811e553effe0787c5309a655e081673.
use sts_core::adapter_internals::{Relic, RunState};
use sts_core::potion::Potion;

#[test]
fn ascension_eleven_reduces_base_capacity() {
    let run = RunState::try_seeded_ironclad(0, 11).unwrap();
    println!(
        "reproduction={}",
        serde_json::json!({"seed": 0, "ascension": 11, "initial_setup": run,
            "accepted_actions": [], "external_inputs": [],
            "observed_capacity": run.potion_capacity(), "expected_capacity": 2})
    );
    assert_eq!(run.potion_capacity(), 2);
}

#[test]
fn capacity_boundary_covers_every_supported_ascension() {
    for ascension in 0..=20 {
        let run = RunState::try_seeded_ironclad(18, ascension).unwrap();
        assert_eq!(run.potion_capacity(), if ascension >= 11 { 2 } else { 3 });
    }
}

#[test]
fn potion_belt_adds_two_to_ascension_adjusted_capacity() {
    for (ascension, expected) in [(0, 5), (10, 5), (11, 4), (20, 4)] {
        let mut run = RunState::try_seeded_ironclad(29, ascension).unwrap();
        run.gain_relic(Relic::PotionBelt).unwrap();
        assert_eq!(run.potion_capacity(), expected);
        for _ in 0..expected {
            run.gain_potion(Potion::Fire).unwrap();
        }
        run.validate().unwrap();
        let before = serde_json::to_value(&run).unwrap();
        assert!(run.gain_potion(Potion::Block).is_err());
        assert_eq!(serde_json::to_value(&run).unwrap(), before);
    }
}

#[test]
fn high_ascension_full_inventory_rejects_another_potion_atomically() {
    let mut run = RunState::try_seeded_ironclad(37, 20).unwrap();
    run.gain_potion(Potion::Fire).unwrap();
    run.gain_potion(Potion::Block).unwrap();
    assert_eq!(run.open_potion_slots(), 0);
    let before = serde_json::to_value(&run).unwrap();
    assert!(run.gain_potion(Potion::Energy).is_err());
    assert_eq!(serde_json::to_value(&run).unwrap(), before);
    assert_eq!(run.occupied_potion_slots().len(), 2);
    assert_eq!(run.potion_at_slot(2), None);
}

#[test]
fn restore_preserves_capacity_and_slot_legality() {
    let mut run = RunState::try_seeded_ironclad(43, 11).unwrap();
    run.gain_potion(Potion::Fire).unwrap();
    run.gain_potion(Potion::Block).unwrap();
    let restored: RunState = serde_json::from_value(serde_json::to_value(&run).unwrap()).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored.potion_capacity(), 2);
    assert_eq!(
        restored.occupied_potion_slots(),
        run.occupied_potion_slots()
    );
    let mut original_next = run.clone();
    let mut restored_next = restored;
    assert!(original_next.take_potion_slot(2).is_err());
    assert!(restored_next.take_potion_slot(2).is_err());
    assert_eq!(original_next, run);
    assert_eq!(restored_next, run);
    assert_eq!(original_next.take_potion_slot(0).unwrap(), Potion::Fire);
    assert_eq!(restored_next.take_potion_slot(0).unwrap(), Potion::Fire);
    assert_eq!(original_next, restored_next);
}
