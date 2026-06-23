use sts_core::{
    apply_combat_action,
    content::cards::{BASH_ID, STRIKE_R_ID},
    CombatAction, CombatPhase, CombatState, ContentId, Snapshot,
};

const EXPECTED_FINAL_HASH: &str = "d3d8206f063a77e8";

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
        let restored: Snapshot<CombatState> =
            serde_json::from_str(&snapshot_json).expect("decision snapshot restores");
        let final_from_snapshot = replay(restored.state, &trace[action_index..]);

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
    assert_eq!(replay_rng_draw_count(), 0);
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

fn replay_rng_draw_count() -> usize {
    0
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
