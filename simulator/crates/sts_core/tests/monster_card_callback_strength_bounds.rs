//! Synthetic legal card prefixes, not dedicated interaction-trace parity.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CardId, CardInstance, CombatAction,
    ContentId, MonsterId, RunDecisionAction, RunState,
};
use sts_core::content::cards::{DEFEND_R_ID, INFLAME_ID, STRIKE_R_ID};
use sts_core::content::monsters::{AWAKENED_ONE_ID, GREMLIN_NOB_ID};
fn setup(
    monster: ContentId,
    card: ContentId,
    strength: i32,
    ascension: u8,
    count: usize,
) -> RunState {
    let mut r = RunState::combat_fixture();
    r.ascension = ascension;
    let c = r.combat.as_mut().unwrap();
    c.ascension = ascension;
    c.piles.hand = vec![CardInstance::new(CardId::new(100), card)];
    c.piles.draw_pile.clear();
    c.piles.discard_pile.clear();
    let template = c.monsters[0].clone();
    c.monsters.clear();
    for index in 0..count {
        let mut m = template.clone();
        m.id = MonsterId::new(index as u64 + 1);
        m.content_id = monster;
        m.hp = 100;
        m.max_hp = 100;
        m.powers.strength = strength;
        m.powers.anger = if monster == GREMLIN_NOB_ID {
            if ascension >= 18 {
                3
            } else {
                2
            }
        } else {
            0
        };
        c.monsters.push(m);
    }
    r.validate().unwrap();
    r
}
fn play(r: &RunState) -> RunState {
    let card = r.combat.as_ref().unwrap().piles.hand[0];
    let target = if card.content_id == STRIKE_R_ID {
        Some(MonsterId::new(1))
    } else {
        None
    };
    let a = RunDecisionAction::Combat(CombatAction::PlayCard {
        card_id: card.id,
        target,
    });
    assert!(legal_run_decision_actions(r).unwrap().contains(&a));
    let before = serde_json::to_value(r).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let n = apply_run_decision_action(r, a).unwrap();
    n.validate().unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&n).unwrap(),
        serde_json::to_value(apply_run_decision_action(&restored, a).unwrap()).unwrap()
    );
    println!(
        "monster_card_strength_transition={}",
        serde_json::json!({"initial":before,"action":a,"result":n})
    );
    n
}
#[test]
fn enrage_caps_skill_strength() {
    let n = play(&setup(GREMLIN_NOB_ID, DEFEND_R_ID, 998, 0, 1));
    assert_eq!(n.combat.unwrap().monsters[0].powers.strength, 999);
}
#[test]
fn ascended_enrage_caps_skill_strength() {
    let n = play(&setup(GREMLIN_NOB_ID, DEFEND_R_ID, 997, 18, 1));
    assert_eq!(n.combat.unwrap().monsters[0].powers.strength, 999);
}
#[test]
fn curiosity_caps_power_strength() {
    let n = play(&setup(AWAKENED_ONE_ID, INFLAME_ID, 999, 0, 1));
    assert_eq!(n.combat.unwrap().monsters[0].powers.strength, 999);
}
#[test]
fn ascended_curiosity_caps_power_strength() {
    let n = play(&setup(AWAKENED_ONE_ID, INFLAME_ID, 998, 19, 1));
    assert_eq!(n.combat.unwrap().monsters[0].powers.strength, 999);
}
#[test]
fn two_enrage_owners_are_capped_independently() {
    let n = play(&setup(GREMLIN_NOB_ID, DEFEND_R_ID, 998, 0, 2));
    assert!(n
        .combat
        .unwrap()
        .monsters
        .iter()
        .all(|m| m.powers.strength == 999));
}
#[test]
fn ordinary_enrage_control() {
    let n = play(&setup(GREMLIN_NOB_ID, DEFEND_R_ID, -3, 0, 1));
    assert_eq!(n.combat.unwrap().monsters[0].powers.strength, -1);
}
#[test]
fn ordinary_curiosity_control() {
    let n = play(&setup(AWAKENED_ONE_ID, INFLAME_ID, -3, 19, 1));
    assert_eq!(n.combat.unwrap().monsters[0].powers.strength, -1);
}
#[test]
fn attacks_do_not_trigger_either_callback_control() {
    for monster in [GREMLIN_NOB_ID, AWAKENED_ONE_ID] {
        let n = play(&setup(monster, STRIKE_R_ID, 999, 0, 1));
        assert_eq!(n.combat.unwrap().monsters[0].powers.strength, 999);
    }
}
#[test]
fn positive_callbacks_do_not_consume_artifact() {
    for (monster, card, ascension) in [
        (GREMLIN_NOB_ID, DEFEND_R_ID, 0),
        (AWAKENED_ONE_ID, INFLAME_ID, 19),
    ] {
        let mut r = setup(monster, card, 998, ascension, 1);
        r.combat.as_mut().unwrap().monsters[0].powers.artifact = 1;
        let n = play(&r);
        let m = &n.combat.unwrap().monsters[0];
        assert_eq!(m.powers.strength, 999);
        assert_eq!(m.powers.artifact, 1);
    }
}
#[test]
fn second_form_has_no_curiosity_callback() {
    let mut r = setup(AWAKENED_ONE_ID, INFLAME_ID, 998, 19, 1);
    r.combat.as_mut().unwrap().monsters[0].mode_shift = 1;
    let n = play(&r);
    assert_eq!(n.combat.unwrap().monsters[0].powers.strength, 998);
}
#[test]
fn dead_enrage_owner_is_not_gained_or_repaired() {
    let mut r = setup(GREMLIN_NOB_ID, DEFEND_R_ID, 998, 0, 2);
    let m = &mut r.combat.as_mut().unwrap().monsters[0];
    m.alive = false;
    m.hp = 0;
    let n = play(&r);
    let c = n.combat.unwrap();
    assert_eq!(c.monsters[0].powers.strength, 998);
    assert_eq!(c.monsters[1].powers.strength, 999);
}
fn malformed_overflow_keeps_input(monster: ContentId, card: ContentId) {
    let r = setup(monster, card, i32::MAX, 0, 1);
    let before = serde_json::to_value(&r).unwrap();
    let a = RunDecisionAction::Combat(CombatAction::PlayCard {
        card_id: CardId::new(100),
        target: None,
    });
    assert_eq!(
        apply_run_decision_action(&r, a).unwrap_err().to_string(),
        "invalid state: combat integer addition overflows i32"
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}
#[test]
fn malformed_enrage_overflow_keeps_existing_error_and_input() {
    malformed_overflow_keeps_input(GREMLIN_NOB_ID, DEFEND_R_ID);
}
#[test]
fn malformed_curiosity_overflow_keeps_existing_error_and_input() {
    malformed_overflow_keeps_input(AWAKENED_ONE_ID, INFLAME_ID);
}
#[test]
fn capped_enrage_changes_following_attack_damage() {
    let mut r = setup(GREMLIN_NOB_ID, DEFEND_R_ID, 998, 0, 1);
    let c = r.combat.as_mut().unwrap();
    c.player.hp = 2000;
    c.player.max_hp = 2000;
    c.monsters[0].intent = sts_core::adapter_internals::MonsterIntent::Attack { damage: 14 };
    let n = play(&r);
    let a = RunDecisionAction::Combat(CombatAction::EndTurn);
    assert!(legal_run_decision_actions(&n).unwrap().contains(&a));
    let before = serde_json::to_value(&n).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let after = apply_run_decision_action(&n, a).unwrap();
    after.validate().unwrap();
    assert_eq!(serde_json::to_value(&n).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&after).unwrap(),
        serde_json::to_value(apply_run_decision_action(&restored, a).unwrap()).unwrap()
    );
    assert_eq!(after.combat.unwrap().player.hp, 992);
}
