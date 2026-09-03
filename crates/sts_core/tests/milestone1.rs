use sts_core::{
    apply_combat_action,
    content::cards::{BASH_ID, STRIKE_R_ID},
    CombatAction, CombatPhase, CombatState, ContentId,
};

const EXPECTED_FINAL_HASH: &str = "7ba6025e448f4e09";

#[test]
fn full_replay_final_hash_matches_expected() {
    let final_state = replay(milestone1_fixture(), &winning_trace());

    assert_eq!(final_state.phase, CombatPhase::Won);
    assert_eq!(
        final_state
            .snapshot()
            .hash()
            .expect("final state hashes")
            .to_string(),
        EXPECTED_FINAL_HASH
    );
}

#[test]
fn replay_from_every_decision_snapshot_matches_final_hash() {
    let initial = milestone1_fixture();
    let trace = winning_trace();
    let expected_final = replay(initial.clone(), &trace)
        .snapshot()
        .hash()
        .expect("final state hashes");

    let mut state = initial;
    for action_index in 0..trace.len() {
        let snapshot_json = state
            .snapshot()
            .canonical_json()
            .expect("decision snapshot serializes");
        let document: serde_json::Value =
            serde_json::from_str(&snapshot_json).expect("decision snapshot parses");
        assert_eq!(
            document
                .get("schema_version")
                .and_then(serde_json::Value::as_u64),
            Some(8)
        );
        let restored: CombatState = serde_json::from_value(
            document
                .get("state")
                .cloned()
                .expect("combat snapshot has state"),
        )
        .expect("decision snapshot restores");
        let final_from_snapshot = replay(restored, &trace[action_index..]);

        assert_eq!(
            final_from_snapshot
                .snapshot()
                .hash()
                .expect("snapshot replay hashes"),
            expected_final
        );

        state = apply_combat_action(&state, trace[action_index]).expect("trace action applies");
    }
}

#[test]
fn golden_replay_consumes_no_rng_draws() {
    let initial = milestone1_fixture();
    let before = rng_counters(&initial);
    let final_state = replay(initial, &winning_trace());
    assert_eq!(rng_counters(&final_state), before);
}

fn milestone1_fixture() -> CombatState {
    let mut state = CombatState::initial_fixture();
    state.monsters[0].hp = 14;
    state
}

fn winning_trace() -> Vec<CombatAction> {
    let state = milestone1_fixture();

    vec![
        CombatAction::PlayCard {
            card_id: hand_card_id(&state, BASH_ID),
            target: Some(state.monsters[0].id),
        },
        CombatAction::PlayCard {
            card_id: hand_card_id(&state, STRIKE_R_ID),
            target: Some(state.monsters[0].id),
        },
    ]
}

fn replay(mut state: CombatState, trace: &[CombatAction]) -> CombatState {
    for action in trace {
        state = apply_combat_action(&state, *action).expect("trace action applies");
    }
    state
}

fn rng_counters(state: &CombatState) -> [u32; 4] {
    [
        state.rng.shuffle_rng.counter(),
        state.rng.monster_rng.counter(),
        state.rng.monster_hp_rng.counter(),
        state.rng.card_random_rng.counter(),
    ]
}

fn hand_card_id(state: &CombatState, content_id: ContentId) -> sts_core::CardId {
    state
        .piles
        .hand
        .iter()
        .find(|card| card.content_id == content_id)
        .expect("card is in hand")
        .id
}
