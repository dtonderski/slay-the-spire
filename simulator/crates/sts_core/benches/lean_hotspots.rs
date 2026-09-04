use std::{hint::black_box, time::Instant};

use sts_core::adapter_internals::{
    apply_combat_action, apply_combat_action_with_events, apply_run_decision_action,
    legal_run_decision_actions, validate_run_decision_action, CardGridScreen, CombatAction,
    GridPurpose, RoomKind, RunDecisionAction, RunPhase, RunState,
};

const SAMPLES: usize = 2_000;

fn median_nanos(mut samples: Vec<u128>) -> u128 {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn measure(mut operation: impl FnMut()) -> u128 {
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        operation();
        samples.push(started.elapsed().as_nanos());
    }
    median_nanos(samples)
}

fn main() {
    let map = RunState::map_fixture();
    let combat = RunState::combat_fixture();
    let combat_action = legal_run_decision_actions(&combat)
        .expect("combat fixture must enumerate")
        .into_iter()
        .find_map(|action| match action {
            RunDecisionAction::Combat(action @ CombatAction::PlayCard { .. }) => Some(action),
            _ => None,
        })
        .expect("combat fixture must contain a playable card");
    let combat_state = combat.combat.as_ref().expect("combat fixture owns combat");

    let mut rest = RunState::map_fixture();
    rest.phase = RunPhase::Rest;
    rest.current_room_override = Some(RoomKind::Rest);
    rest.event = None;
    let rest_action = legal_run_decision_actions(&rest).expect("rest fixture enumerates")[0];

    let mut grid = rest.clone();
    grid.card_grid = Some(CardGridScreen {
        cards: grid.deck.clone(),
        purpose: GridPurpose::RestSmith,
        selected: None,
        selected_indices: Vec::new(),
    });
    let grid_action = RunDecisionAction::GridSelect { index: 0 };
    let map_action = legal_run_decision_actions(&map).expect("map fixture enumerates")[0];

    for run in [&map, &combat, &rest, &grid] {
        let actions = legal_run_decision_actions(run).expect("fixture query succeeds");
        assert!(actions
            .iter()
            .all(|action| validate_run_decision_action(run, *action).is_ok()));
    }

    let map_legal_ns = measure(|| {
        black_box(legal_run_decision_actions(black_box(&map)).expect("map query succeeds"));
    });
    let combat_legal_ns = measure(|| {
        black_box(legal_run_decision_actions(black_box(&combat)).expect("combat query succeeds"));
    });
    let map_transition_ns = measure(|| {
        black_box(
            apply_run_decision_action(black_box(&map), map_action)
                .expect("map transition succeeds"),
        );
    });
    let rest_transition_ns = measure(|| {
        black_box(
            apply_run_decision_action(black_box(&rest), rest_action)
                .expect("rest transition succeeds"),
        );
    });
    let grid_transition_ns = measure(|| {
        black_box(
            apply_run_decision_action(black_box(&grid), grid_action)
                .expect("grid transition succeeds"),
        );
    });
    let integrated_combat_ns = measure(|| {
        black_box(
            apply_run_decision_action(black_box(&combat), RunDecisionAction::Combat(combat_action))
                .expect("integrated combat transition succeeds"),
        );
    });
    let no_events_ns = measure(|| {
        black_box(
            apply_combat_action(black_box(combat_state), combat_action)
                .expect("combat action succeeds"),
        );
    });
    let with_events_ns = measure(|| {
        black_box(
            apply_combat_action_with_events(black_box(combat_state), combat_action)
                .expect("combat action succeeds"),
        );
    });

    let without_events =
        apply_combat_action(combat_state, combat_action).expect("combat action succeeds");
    let with_events = apply_combat_action_with_events(combat_state, combat_action)
        .expect("combat action succeeds");
    assert_eq!(without_events, with_events.state);
    assert!(!with_events.event_log.is_empty());

    println!("lean_hotspots samples={SAMPLES}");
    println!("legal/map median_ns={map_legal_ns}");
    println!("legal/combat median_ns={combat_legal_ns}");
    println!("transition/map median_ns={map_transition_ns}");
    println!("transition/rest median_ns={rest_transition_ns}");
    println!("transition/grid median_ns={grid_transition_ns}");
    println!("transition/combat_integrated median_ns={integrated_combat_ns}");
    println!("transition/no_events median_ns={no_events_ns}");
    println!("transition/with_events median_ns={with_events_ns}");
}
