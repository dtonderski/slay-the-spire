//! Source-backed synthetic legal gameplay prefixes; not interaction-trace parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action, CardId, CardInstance, CombatAction, MonsterId,
    Potion, Relic, RunAction, RunState,
};
use sts_core::content::cards::{DEFEND_R_ID, ENTRENCH_ID, IRON_WAVE_ID, STRIKE_R_ID};
fn setup(relics: Vec<Relic>, potions: Vec<Potion>) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    r.potions = potions;
    let c = r.combat.as_mut().unwrap();
    c.piles.hand = vec![
        CardInstance::new(CardId::new(100), DEFEND_R_ID),
        CardInstance::new(CardId::new(101), ENTRENCH_ID),
        CardInstance::new(CardId::new(102), STRIKE_R_ID),
    ];
    c.piles.draw_pile = (200..220)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    c.piles.discard_pile.clear();
    c.piles.exhaust_pile.clear();
    c.monsters[0].hp = 1000;
    c.monsters[0].max_hp = 1000;
    r.validate().unwrap();
    r
}
fn combat(r: &RunState, a: CombatAction) -> RunState {
    step(r, Some(a), None)
}
fn run(r: &RunState, a: RunAction) -> RunState {
    step(r, None, Some(a))
}
fn step(r: &RunState, c: Option<CombatAction>, a: Option<RunAction>) -> RunState {
    let before = serde_json::to_value(r).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let apply = |s: &RunState| match c {
        Some(c) => apply_combat_action_on_run(s, c),
        None => apply_run_action(s, a.unwrap()),
    };
    let next = apply(r).unwrap();
    next.validate().unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(apply(&restored).unwrap()).unwrap()
    );
    println!(
        "player_block_transition={}",
        serde_json::json!({"initial":before,"combat_action":c,"run_action":a,"result":next})
    );
    next
}
#[test]
fn card_gain_caps_block() {
    let mut r = setup(vec![], vec![]);
    r.combat.as_mut().unwrap().player.block = 998;
    let n = combat(
        &r,
        CombatAction::PlayCard {
            card_id: CardId::new(100),
            target: None,
        },
    );
    assert_eq!(n.combat.as_ref().unwrap().player.block, 999);
}
#[test]
fn entrench_caps_block() {
    let mut r = setup(vec![], vec![]);
    r.combat.as_mut().unwrap().player.block = 600;
    let n = combat(
        &r,
        CombatAction::PlayCard {
            card_id: CardId::new(101),
            target: None,
        },
    );
    assert_eq!(n.combat.as_ref().unwrap().player.block, 999);
}
#[test]
fn rage_gain_caps_block() {
    let mut r = setup(vec![], vec![]);
    let c = r.combat.as_mut().unwrap();
    c.player.block = 998;
    c.player.temp_rage_block = 3;
    let n = combat(
        &r,
        CombatAction::PlayCard {
            card_id: CardId::new(102),
            target: Some(MonsterId::new(1)),
        },
    );
    assert_eq!(n.combat.as_ref().unwrap().player.block, 999);
}
#[test]
fn feel_no_pain_exhaust_gain_caps_block() {
    let mut r = setup(vec![], vec![Potion::Elixir]);
    let c = r.combat.as_mut().unwrap();
    c.player.block = 998;
    c.player.powers.feel_no_pain = 3;
    let s = run(
        &r,
        RunAction::UsePotion {
            slot: 0,
            target: None,
        },
    );
    let s = run(&s, RunAction::ChooseExhaustSelect { index: 0 });
    let n = run(&s, RunAction::ConfirmExhaustSelect);
    assert_eq!(n.combat.as_ref().unwrap().player.block, 999);
}
#[test]
fn metallicize_caps_before_monster_damage() {
    let mut r = setup(vec![], vec![]);
    let c = r.combat.as_mut().unwrap();
    c.player.block = 998;
    c.player.powers.metallicize = 3;
    c.player.powers.barricade = 1;
    let n = combat(&r, CombatAction::EndTurn);
    assert_eq!(n.combat.as_ref().unwrap().player.block, 993);
}
#[test]
fn abacus_shuffle_gain_caps_block() {
    let mut r = setup(vec![Relic::TheAbacus], vec![Potion::Swift]);
    let c = r.combat.as_mut().unwrap();
    c.player.block = 998;
    c.piles.discard_pile = std::mem::take(&mut c.piles.draw_pile);
    let n = run(
        &r,
        RunAction::UsePotion {
            slot: 0,
            target: None,
        },
    );
    assert_eq!(n.combat.as_ref().unwrap().player.block, 999);
}
#[test]
fn already_capped_gain_keeps_juggernaut_callback() {
    let mut r = setup(vec![], vec![]);
    let c = r.combat.as_mut().unwrap();
    c.player.block = 999;
    c.player.powers.juggernaut = 5;
    let n = combat(
        &r,
        CombatAction::PlayCard {
            card_id: CardId::new(100),
            target: None,
        },
    );
    assert_eq!(n.combat.as_ref().unwrap().player.block, 999);
    assert_eq!(n.combat.as_ref().unwrap().monsters[0].hp, 995);
}
#[test]
fn precomputed_iron_wave_block_caps_and_keeps_nominal_callback() {
    let mut r = setup(vec![], vec![]);
    let c = r.combat.as_mut().unwrap();
    c.player.block = 998;
    c.player.powers.juggernaut = 5;
    c.piles.hand = vec![CardInstance::new(CardId::new(100), IRON_WAVE_ID)];
    let n = combat(
        &r,
        CombatAction::PlayCard {
            card_id: CardId::new(100),
            target: Some(MonsterId::new(1)),
        },
    );
    assert_eq!(n.combat.as_ref().unwrap().player.block, 999);
    assert_eq!(n.combat.as_ref().unwrap().monsters[0].hp, 990);
}
#[test]
fn entrench_at_cap_keeps_nominal_callback() {
    let mut r = setup(vec![], vec![]);
    let c = r.combat.as_mut().unwrap();
    c.player.block = 999;
    c.player.powers.juggernaut = 5;
    let n = combat(
        &r,
        CombatAction::PlayCard {
            card_id: CardId::new(101),
            target: None,
        },
    );
    assert_eq!(n.combat.as_ref().unwrap().player.block, 999);
    assert_eq!(n.combat.as_ref().unwrap().monsters[0].hp, 995);
}
#[test]
fn capped_card_respects_frail_dex_and_no_block_control() {
    let mut r = setup(vec![], vec![]);
    let c = r.combat.as_mut().unwrap();
    c.player.block = 995;
    c.player.powers.dexterity = 1;
    c.player.powers.frail = 1;
    let n = combat(
        &r,
        CombatAction::PlayCard {
            card_id: CardId::new(100),
            target: None,
        },
    );
    assert_eq!(n.combat.as_ref().unwrap().player.block, 999);
    let mut prevented = r.clone();
    prevented.combat.as_mut().unwrap().player.no_block_turns = 2;
    let n = combat(
        &prevented,
        CombatAction::PlayCard {
            card_id: CardId::new(100),
            target: None,
        },
    );
    assert_eq!(n.combat.as_ref().unwrap().player.block, 995);
}
#[test]
fn ordinary_card_gain_control() {
    let mut r = setup(vec![], vec![]);
    r.combat.as_mut().unwrap().player.block = 10;
    let n = combat(
        &r,
        CombatAction::PlayCard {
            card_id: CardId::new(100),
            target: None,
        },
    );
    assert_eq!(n.combat.as_ref().unwrap().player.block, 15);
}
#[test]
fn malformed_entrench_overflow_control() {
    let mut r = setup(vec![], vec![]);
    r.combat.as_mut().unwrap().player.block = i32::MAX;
    let before = serde_json::to_value(&r).unwrap();
    assert!(matches!(
        apply_combat_action_on_run(
            &r,
            CombatAction::PlayCard {
                card_id: CardId::new(101),
                target: None
            }
        ),
        Err(sts_core::SimError::InvalidState(
            "combat integer multiplication overflows i32"
        ))
    ));
    assert_eq!(serde_json::to_value(r).unwrap(), before);
}
