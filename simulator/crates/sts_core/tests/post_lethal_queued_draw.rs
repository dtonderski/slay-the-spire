//! Source-backed synthetic legal combat prefixes, not interaction-trace parity.
use sts_core::adapter_internals::{
    apply_combat_action, legal_combat_actions, CardId, CardInstance, CombatAction, CombatState,
    MonsterId, RunState,
};
use sts_core::content::cards::{DROPKICK_ID, POMMEL_STRIKE_ID, POMMEL_STRIKE_PLUS_ID, STRIKE_R_ID};
fn setup(upgraded: bool, lethal: bool) -> CombatState {
    let r = RunState::combat_fixture();
    let mut c = r.combat.unwrap();
    c.piles.hand = vec![CardInstance::new(
        CardId::new(100),
        if upgraded {
            POMMEL_STRIKE_PLUS_ID
        } else {
            POMMEL_STRIKE_ID
        },
    )];
    c.piles.draw_pile = (200..210)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    c.piles.discard_pile.clear();
    c.piles.exhaust_pile.clear();
    c.player.powers.confusion = 1;
    c.monsters[0].hp = if lethal { 1 } else { 100 };
    c.monsters[0].max_hp = 100;
    c.validate().unwrap();
    c
}
fn play(c: &CombatState) -> CombatState {
    let a = CombatAction::PlayCard {
        card_id: CardId::new(100),
        target: Some(MonsterId::new(1)),
    };
    assert!(legal_combat_actions(c).unwrap().contains(&a));
    let before = serde_json::to_value(c).unwrap();
    let restored: CombatState = serde_json::from_value(before.clone()).unwrap();
    let n = apply_combat_action(c, a).unwrap();
    n.validate().unwrap();
    assert_eq!(serde_json::to_value(c).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&n).unwrap(),
        serde_json::to_value(apply_combat_action(&restored, a).unwrap()).unwrap()
    );
    println!(
        "post_lethal_transition={}",
        serde_json::json!({"initial":before,"action":a,"result":n})
    );
    n
}
#[test]
fn lethal_pommel_does_not_draw_or_advance_confusion_rng() {
    let c = setup(false, true);
    let n = play(&c);
    assert_eq!(
        n.rng.card_random_rng.counter(),
        c.rng.card_random_rng.counter()
    );
    assert!(n.piles.hand.is_empty());
    assert_eq!(n.piles.draw_pile.len(), 10);
    assert_eq!(n.piles.discard_pile.len(), 1);
}
#[test]
fn lethal_upgraded_pommel_cancels_both_draws() {
    let c = setup(true, true);
    let n = play(&c);
    assert_eq!(
        n.rng.card_random_rng.counter(),
        c.rng.card_random_rng.counter()
    );
    assert!(n.piles.hand.is_empty());
    assert_eq!(n.piles.draw_pile.len(), 10);
}
#[test]
fn lethal_dropkick_cancels_queued_energy_and_draw() {
    let mut c = setup(false, true);
    c.piles.hand = vec![CardInstance::new(CardId::new(100), DROPKICK_ID)];
    c.monsters[0].powers.vulnerable = 1;
    let n = play(&c);
    assert_eq!(n.player.energy, 2);
    assert_eq!(
        n.rng.card_random_rng.counter(),
        c.rng.card_random_rng.counter()
    );
    assert!(n.piles.hand.is_empty());
    assert_eq!(n.piles.draw_pile.len(), 10);
}
#[test]
fn surviving_target_keeps_draw_and_confusion_control() {
    let c = setup(false, false);
    let n = play(&c);
    assert_eq!(n.piles.hand.len(), 1);
    assert_eq!(
        n.rng.card_random_rng.counter(),
        c.rng.card_random_rng.counter() + 1
    );
}
#[test]
fn killing_one_of_two_targets_keeps_draw_control() {
    let mut c = setup(false, true);
    let mut other = c.monsters[0].clone();
    other.id = MonsterId::new(2);
    other.hp = 100;
    c.monsters.push(other);
    let n = play(&c);
    assert_eq!(n.piles.hand.len(), 1);
    assert_eq!(
        n.rng.card_random_rng.counter(),
        c.rng.card_random_rng.counter() + 1
    );
}
