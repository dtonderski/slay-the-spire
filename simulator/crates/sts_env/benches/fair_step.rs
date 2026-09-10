use std::{hint::black_box, time::Instant};

use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatPhase, RunPhase, RunState,
};
use sts_env::{FairCombatPhase, FairEnvironment, FairRunScreen, PublicChoiceRequest};

const SAMPLES: usize = 2_000;
const TRAJECTORY_SAMPLES: usize = 200;
const TRAJECTORY_STEPS: usize = 32;
const SEED: u64 = 1;
const ASCENSION: u8 = 0;

fn median_nanos(mut samples: Vec<u128>) -> u128 {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn measure(operation: impl FnMut()) -> u128 {
    measure_n(SAMPLES, operation)
}

fn measure_n(samples: usize, mut operation: impl FnMut()) -> u128 {
    let mut collected = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = Instant::now();
        operation();
        collected.push(started.elapsed().as_nanos());
    }
    median_nanos(collected)
}

fn measure_with_setup<T>(setup: impl FnMut() -> T, operation: impl FnMut(T)) -> u128 {
    measure_n_with_setup(SAMPLES, setup, operation)
}

fn measure_n_with_setup<T>(
    samples: usize,
    mut setup: impl FnMut() -> T,
    mut operation: impl FnMut(T),
) -> u128 {
    let mut collected = Vec::with_capacity(samples);
    for _ in 0..samples {
        let input = setup();
        let started = Instant::now();
        operation(input);
        collected.push(started.elapsed().as_nanos());
    }
    median_nanos(collected)
}

fn seeded_run() -> RunState {
    let run = RunState::try_seeded_ironclad(SEED, ASCENSION).expect("seeded run");
    run.validate().expect("seeded run is valid");
    run
}

fn seeded_env() -> FairEnvironment {
    FairEnvironment::new_ironclad(SEED, ASCENSION).expect("environment")
}

fn first_combat_run() -> RunState {
    let mut run = seeded_run();
    for _ in 0..100 {
        if run.phase == RunPhase::Combat
            && run
                .combat
                .as_ref()
                .is_some_and(|combat| combat.phase == CombatPhase::WaitingForPlayer)
        {
            return run;
        }
        let actions = legal_run_decision_actions(&run).expect("legal actions");
        let action = *actions.first().expect("setup action");
        run = apply_run_decision_action(&run, action).expect("setup apply");
    }
    panic!("could not reach first combat")
}

fn first_combat_env() -> FairEnvironment {
    let mut env = seeded_env();
    for _ in 0..100 {
        let decision = env.decision().expect("decision");
        if let FairRunScreen::Combat(combat) = &decision.observation.screen {
            if combat.phase == FairCombatPhase::WaitingForPlayer {
                return env;
            }
        }
        let choice = *decision.choices.first().expect("setup choice");
        env.step(PublicChoiceRequest {
            revision: decision.revision,
            choice,
        })
        .expect("setup step");
    }
    panic!("could not reach first combat")
}

fn walk_checked(mut run: RunState, steps: usize) -> RunState {
    for _ in 0..steps {
        let actions = legal_run_decision_actions(&run).expect("legal actions");
        let Some(action) = actions.first().copied() else {
            break;
        };
        run = apply_run_decision_action(&run, action).expect("checked apply");
    }
    run
}

fn walk_fair(mut env: FairEnvironment, steps: usize) -> FairEnvironment {
    for _ in 0..steps {
        let decision = env.decision().expect("decision");
        let Some(choice) = decision.choices.first().copied() else {
            break;
        };
        env.step(PublicChoiceRequest {
            revision: decision.revision,
            choice,
        })
        .expect("fair step");
    }
    env
}

fn main() {
    let initial_run = seeded_run();
    let initial_env = seeded_env();
    let initial_action = legal_run_decision_actions(&initial_run).expect("initial actions")[0];
    let initial_decision = initial_env.decision().expect("initial decision");
    let initial_request = PublicChoiceRequest {
        revision: initial_decision.revision,
        choice: *initial_decision.choices.first().expect("initial choice"),
    };

    let combat_run = first_combat_run();
    let combat_env = first_combat_env();
    let combat_action = legal_run_decision_actions(&combat_run).expect("combat actions")[0];
    let combat_decision = combat_env.decision().expect("combat decision");
    let combat_request = PublicChoiceRequest {
        revision: combat_decision.revision,
        choice: *combat_decision.choices.first().expect("combat choice"),
    };

    let checked_after = walk_checked(initial_run.clone(), TRAJECTORY_STEPS);
    let fair_after = walk_fair(initial_env.clone(), TRAJECTORY_STEPS);
    let fair_after_decision = fair_after.decision().expect("fair trajectory decision");
    let checked_legal = legal_run_decision_actions(&checked_after).expect("checked legal");
    assert_eq!(checked_legal.len(), fair_after_decision.choices.len());
    assert_eq!(
        fair_after.observation().expect("fair observation"),
        fair_after_decision.observation
    );

    let initial_checked_ns = measure(|| {
        black_box(
            apply_run_decision_action(black_box(&initial_run), initial_action)
                .expect("initial checked apply"),
        );
    });
    let initial_fair_ns = measure_with_setup(
        || initial_env.clone(),
        |mut env| {
            black_box(env.step(initial_request).expect("initial fair step"));
        },
    );
    let combat_checked_ns = measure(|| {
        black_box(
            apply_run_decision_action(black_box(&combat_run), combat_action)
                .expect("combat checked apply"),
        );
    });
    let combat_fair_ns = measure_with_setup(
        || combat_env.clone(),
        |mut env| {
            black_box(env.step(combat_request).expect("combat fair step"));
        },
    );
    let trajectory_checked_ns = measure_n_with_setup(
        TRAJECTORY_SAMPLES,
        || initial_run.clone(),
        |run| {
            black_box(walk_checked(run, TRAJECTORY_STEPS));
        },
    );
    let trajectory_fair_ns = measure_n_with_setup(
        TRAJECTORY_SAMPLES,
        || initial_env.clone(),
        |env| {
            black_box(walk_fair(env, TRAJECTORY_STEPS));
        },
    );

    println!("fair_step samples={SAMPLES} trajectory_samples={TRAJECTORY_SAMPLES} steps={TRAJECTORY_STEPS} seed={SEED}");
    println!("checked/initial_apply median_ns={initial_checked_ns}");
    println!("fair/initial_step median_ns={initial_fair_ns}");
    println!("checked/combat_apply median_ns={combat_checked_ns}");
    println!("fair/combat_step median_ns={combat_fair_ns}");
    println!("checked/trajectory_{TRAJECTORY_STEPS} median_ns={trajectory_checked_ns}");
    println!("fair/trajectory_{TRAJECTORY_STEPS} median_ns={trajectory_fair_ns}");
}
