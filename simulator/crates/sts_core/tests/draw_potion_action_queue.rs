//! Source-backed synthetic queue ordering, not dedicated real-game trace parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action, CardId, CardInstance, CombatAction, MonsterId,
    Potion, Relic, RunAction, RunState,
};
use sts_core::content::cards::{BURN_ID, SECRET_WEAPON_ID, STRIKE_R_ID};
fn setup(relics: Vec<Relic>, potions: Vec<Potion>) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    r.potions = potions;
    let c = r.combat.as_mut().unwrap();
    c.piles.hand = vec![CardInstance::new(CardId::new(100), STRIKE_R_ID)];
    c.piles.draw_pile = (200..220)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    c.piles.discard_pile.clear();
    c.piles.exhaust_pile.clear();
    r.validate().unwrap();
    r
}
fn step(r: &RunState, a: RunAction) -> RunState {
    apply(r, Some(a), None)
}
fn apply(r: &RunState, a: Option<RunAction>, ca: Option<CombatAction>) -> RunState {
    let before = serde_json::to_value(r).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let execute = |s: &RunState| match a {
        Some(a) => apply_run_action(s, a),
        None => apply_combat_action_on_run(s, ca.unwrap()),
    };
    let next = execute(r).unwrap();
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(execute(&restored).unwrap()).unwrap()
    );
    assert_eq!(serde_json::to_value(r).unwrap(), before);
    next.validate().unwrap();
    println!(
        "draw_potion_transition={}",
        serde_json::json!({"initial":before,"run_action":a,"combat_action":ca,"result":next})
    );
    next
}
fn drink(r: &RunState) -> RunState {
    let slot = (0..r.potion_capacity())
        .find(|s| r.potion_at_slot(*s).is_some())
        .unwrap();
    step(r, RunAction::UsePotion { slot, target: None })
}
#[test]
fn snecko_keeps_draw_time_confusion_rng_before_hand_randomization() {
    let mut r = setup(vec![Relic::SneckoEye], vec![Potion::SneckoOil]);
    r.combat.as_mut().unwrap().player.powers.confusion = 1;
    let before = r.card_random_rng_counter;
    let next = drink(&r);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.piles.hand.len(), 6);
    assert_eq!(next.card_random_rng_counter, before + 11);
    assert_eq!(
        next.card_random_rng_counter,
        c.rng.card_random_rng.counter()
    );
}
#[test]
fn hand_randomization_precedes_bot_queued_fire_breathing_and_horn_draw() {
    let mut r = setup(vec![Relic::GremlinHorn], vec![Potion::SneckoOil]);
    let c = r.combat.as_mut().unwrap();
    c.player.powers.fire_breathing = 6;
    c.piles.draw_pile.last_mut().unwrap().content_id = BURN_ID;
    c.monsters[0].hp = 6;
    c.monsters[0].max_hp = 6;
    let mut other = c.monsters[0].clone();
    other.id = MonsterId::new(2);
    other.hp = 100;
    other.max_hp = 100;
    c.monsters.push(other);
    let before = r.card_random_rng_counter;
    let next = drink(&r);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.piles.hand.len(), 7);
    assert!(!c.monsters[0].alive);
    assert_eq!(c.monsters[1].hp, 94);
    assert_eq!(c.piles.hand.last().unwrap().id, CardId::new(214));
    assert_eq!(c.piles.hand.last().unwrap().temp_cost, None);
    assert_eq!(next.card_random_rng_counter, before + 5);
    assert_eq!(
        next.card_random_rng_counter,
        c.rng.card_random_rng.counter()
    );
}
fn reward_heal_case(p: Potion) {
    let mut r = setup(vec![Relic::ToyOrnithopter], vec![Potion::Attack, p]);
    r.combat.as_mut().unwrap().player.hp = 30;
    let opened = drink(&r);
    let pending = drink(&opened);
    assert_eq!(
        pending.combat.as_ref().unwrap().player.hp,
        30,
        "queued potion heal must wait for the reward"
    );
    let next = step(&pending, RunAction::ChooseCombatCardReward { index: 0 });
    assert_eq!(next.combat.as_ref().unwrap().player.hp, 40);
}
#[test]
fn swift_heal_waits_behind_reward() {
    reward_heal_case(Potion::Swift);
}
#[test]
fn snecko_heal_waits_behind_reward() {
    reward_heal_case(Potion::SneckoOil);
}
#[test]
fn swift_heal_waits_behind_secret_weapon_grid() {
    let mut r = setup(vec![Relic::ToyOrnithopter], vec![Potion::Swift]);
    let c = r.combat.as_mut().unwrap();
    c.player.hp = 30;
    c.piles.hand = vec![
        CardInstance::new(CardId::new(100), SECRET_WEAPON_ID),
        CardInstance::new(CardId::new(101), STRIKE_R_ID),
    ];
    let opened = apply(
        &r,
        None,
        Some(CombatAction::PlayCard {
            card_id: CardId::new(100),
            target: None,
        }),
    );
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.hp, 30);
    // This run-level choice auto-confirms Secret Weapon's single retrieval.
    let next = step(&pending, RunAction::ChooseDrawSelect { index: 0 });
    assert_eq!(next.combat.as_ref().unwrap().player.hp, 35);
}
#[test]
fn draw_potion_heal_waits_behind_nilry_then_resumes_once() {
    for p in [Potion::Swift, Potion::SneckoOil] {
        let mut r = setup(vec![Relic::NilrysCodex, Relic::ToyOrnithopter], vec![p]);
        r.combat.as_mut().unwrap().player.hp = 30;
        let opened = apply(&r, None, Some(CombatAction::EndTurn));
        let pending = drink(&opened);
        assert_eq!(pending.combat.as_ref().unwrap().player.hp, 30);
        let next = step(&pending, RunAction::SkipCombatCardReward);
        let c = next.combat.as_ref().unwrap();
        assert_eq!(c.player.hp, 29);
        assert_eq!(c.monsters[0].moves_executed, 1);
        assert_eq!(
            next.card_random_rng_counter,
            c.rng.card_random_rng.counter()
        );
    }
}
#[test]
fn bark_confusion_draw_stops_at_hand_cap_and_keeps_all_rng_draws() {
    let mut r = setup(
        vec![Relic::SneckoEye, Relic::SacredBark],
        vec![Potion::SneckoOil],
    );
    r.combat.as_mut().unwrap().player.powers.confusion = 1;
    let before = r.card_random_rng_counter;
    let next = drink(&r);
    assert_eq!(next.combat.as_ref().unwrap().piles.hand.len(), 10);
    assert_eq!(next.card_random_rng_counter, before + 19);
}
#[test]
fn full_hand_snecko_does_not_draw_but_randomizes_all_existing_cards() {
    let mut r = setup(vec![], vec![Potion::SneckoOil]);
    r.combat.as_mut().unwrap().piles.hand = (100..110)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    let next = drink(&r);
    assert_eq!(
        next.combat.as_ref().unwrap().piles.draw_pile,
        r.combat.as_ref().unwrap().piles.draw_pile
    );
    assert_eq!(next.card_random_rng_counter, r.card_random_rng_counter + 10);
}
#[test]
fn ordinary_swift_control_preserves_rng() {
    let r = setup(vec![], vec![Potion::Swift]);
    let next = drink(&r);
    assert_eq!(next.combat.as_ref().unwrap().piles.hand.len(), 4);
    assert_eq!(next.card_random_rng_counter, r.card_random_rng_counter);
}
#[test]
fn ordinary_snecko_control_randomizes_six_playable_cards() {
    let r = setup(vec![], vec![Potion::SneckoOil]);
    let next = drink(&r);
    assert_eq!(next.combat.as_ref().unwrap().piles.hand.len(), 6);
    assert_eq!(next.card_random_rng_counter, r.card_random_rng_counter + 6);
}
#[test]
fn no_draw_snecko_control_randomizes_existing_hand_only() {
    let mut r = setup(vec![], vec![Potion::SneckoOil]);
    r.combat.as_mut().unwrap().player.cannot_draw = true;
    let next = drink(&r);
    assert_eq!(next.combat.as_ref().unwrap().piles.hand.len(), 1);
    assert_eq!(next.card_random_rng_counter, r.card_random_rng_counter + 1);
}
