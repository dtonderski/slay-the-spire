use sts_core::adapter_internals::{
    apply_run_action, legal_run_decision_actions, CardId, CardInstance, Potion, RunAction,
    RunDecisionAction, RunState,
};
use sts_core::content::cards::{JUST_LUCKY_ANY_COLOR_ID, STRIKE_R_ID};
fn setup() -> RunState {
    let mut r = RunState::combat_fixture();
    r.potions = vec![Potion::DistilledChaos, Potion::Elixir];
    r.empty_potion_slots = vec![2];
    let c = r.combat.as_mut().unwrap();
    c.piles.hand = vec![
        CardInstance::new(CardId::new(40), STRIKE_R_ID),
        CardInstance::new(CardId::new(41), STRIKE_R_ID),
    ];
    c.piles.draw_pile = (20..27)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    c.piles
        .draw_pile
        .push(CardInstance::new(CardId::new(14), JUST_LUCKY_ANY_COLOR_ID));
    c.piles.discard_pile.clear();
    for m in &mut c.monsters {
        m.hp = 500;
        m.max_hp = 500;
    }
    r.validate().unwrap();
    r
}
fn step(r: &RunState, a: RunAction) -> RunState {
    assert!(legal_run_decision_actions(r)
        .unwrap()
        .contains(&RunDecisionAction::Run(a)));
    let original = serde_json::to_value(r).unwrap();
    let n = apply_run_action(r, a).unwrap();
    n.validate().unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), original);
    let restored: RunState = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(apply_run_action(&restored, a).unwrap()).unwrap(),
        serde_json::to_value(&n).unwrap()
    );
    println!(
        "chaos_transition={}",
        serde_json::json!({"initial_state":original,"action":RunDecisionAction::Run(a),"result":n})
    );
    n
}
#[test]
fn chaos_scry_swift_draws_before_siblings_without_drawing_held_tops() {
    let mut r = setup();
    r.potions[1] = Potion::Swift;
    let scry = step(
        &r,
        RunAction::UsePotion {
            slot: 0,
            target: None,
        },
    );
    let queued = step(
        &scry,
        RunAction::UsePotion {
            slot: 1,
            target: None,
        },
    );
    let selected = step(&queued, RunAction::ChooseDrawSelect { index: 0 });
    let n = step(&selected, RunAction::ConfirmDrawSelect);
    assert_eq!(
        n.combat
            .as_ref()
            .unwrap()
            .piles
            .hand
            .iter()
            .map(|c| c.id)
            .collect::<Vec<_>>(),
        vec![
            CardId::new(40),
            CardId::new(41),
            CardId::new(23),
            CardId::new(22),
            CardId::new(21)
        ]
    );
}
#[test]
fn chaos_scry_gambler_selector_excludes_held_siblings() {
    let mut r = setup();
    r.potions[1] = Potion::GamblersBrew;
    let scry = step(
        &r,
        RunAction::UsePotion {
            slot: 0,
            target: None,
        },
    );
    let queued = step(
        &scry,
        RunAction::UsePotion {
            slot: 1,
            target: None,
        },
    );
    let selected = step(&queued, RunAction::ChooseDrawSelect { index: 0 });
    let n = step(&selected, RunAction::ConfirmDrawSelect);
    assert_eq!(
        n.combat
            .as_ref()
            .unwrap()
            .piles
            .hand
            .iter()
            .map(|c| c.id)
            .collect::<Vec<_>>(),
        vec![CardId::new(40), CardId::new(41)]
    );
    let n = step(&n, RunAction::ConfirmExhaustSelect);
    assert!(n.combat.as_ref().unwrap().decision.is_none());
}
#[test]
fn chaos_scry_without_potion_control() {
    let scry = step(
        &setup(),
        RunAction::UsePotion {
            slot: 0,
            target: None,
        },
    );
    let selected = step(&scry, RunAction::ChooseDrawSelect { index: 0 });
    let n = step(&selected, RunAction::ConfirmDrawSelect);
    assert!(n.combat.as_ref().unwrap().decision.is_none());
}
#[test]
fn potion_selector_runs_before_paused_chaos_siblings() {
    let scry = step(
        &setup(),
        RunAction::UsePotion {
            slot: 0,
            target: None,
        },
    );
    let queued = step(
        &scry,
        RunAction::UsePotion {
            slot: 1,
            target: None,
        },
    );
    let selected = step(&queued, RunAction::ChooseDrawSelect { index: 0 });
    let opened = step(&selected, RunAction::ConfirmDrawSelect);
    let c = opened.combat.as_ref().unwrap();
    assert_eq!(
        c.piles.hand.iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![CardId::new(40), CardId::new(41)]
    );
    let n = step(&opened, RunAction::ChooseExhaustSelect { index: 0 });
    let n = step(&n, RunAction::ChooseExhaustSelect { index: 0 });
    let n = step(&n, RunAction::ConfirmExhaustSelect);
    assert!(n.combat.as_ref().unwrap().decision.is_none());
}
