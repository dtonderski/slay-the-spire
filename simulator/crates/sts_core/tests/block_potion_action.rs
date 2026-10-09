//! Source-backed synthetic legal prefixes, not dedicated interaction-trace parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action, CardId, CardInstance, CombatAction, Potion,
    Relic, RunAction, RunState,
};
use sts_core::content::cards::STRIKE_R_ID;
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
    c.monsters[0].hp = 1000;
    c.monsters[0].max_hp = 1000;
    r.validate().unwrap();
    r
}
fn step(r: &RunState, a: Option<RunAction>) -> RunState {
    let before = serde_json::to_value(r).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let apply = |s: &RunState| match a {
        Some(a) => apply_run_action(s, a),
        None => apply_combat_action_on_run(s, CombatAction::EndTurn),
    };
    let next = apply(r).unwrap();
    next.validate().unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(apply(&restored).unwrap()).unwrap()
    );
    println!(
        "block_potion_transition={}",
        serde_json::json!({"initial":before,"run_action":a,"end_turn":a.is_none(),"result":next})
    );
    next
}
fn drink(r: &RunState) -> RunState {
    let slot = (0..r.potion_capacity())
        .find(|s| r.potion_at_slot(*s).is_some())
        .unwrap();
    step(r, Some(RunAction::UsePotion { slot, target: None }))
}
#[test]
fn block_potion_caps_block() {
    let mut r = setup(vec![], vec![Potion::Block]);
    r.combat.as_mut().unwrap().player.block = 998;
    let next = drink(&r);
    assert_eq!(next.combat.as_ref().unwrap().player.block, 999);
}
#[test]
fn bark_block_potion_caps_block() {
    let mut r = setup(vec![Relic::SacredBark], vec![Potion::Block]);
    r.combat.as_mut().unwrap().player.block = 990;
    let next = drink(&r);
    assert_eq!(next.combat.as_ref().unwrap().player.block, 999);
}
#[test]
fn block_potion_triggers_juggernaut() {
    let mut r = setup(vec![], vec![Potion::Block]);
    r.combat.as_mut().unwrap().player.powers.juggernaut = 5;
    let next = drink(&r);
    assert_eq!(next.combat.as_ref().unwrap().monsters[0].hp, 995);
}
#[test]
fn block_at_cap_still_triggers_nominal_gain_callback() {
    let mut r = setup(vec![], vec![Potion::Block]);
    let c = r.combat.as_mut().unwrap();
    c.player.block = 999;
    c.player.powers.juggernaut = 5;
    let next = drink(&r);
    assert_eq!(next.combat.as_ref().unwrap().player.block, 999);
    assert_eq!(next.combat.as_ref().unwrap().monsters[0].hp, 995);
}
#[test]
fn block_waits_behind_potion_reward() {
    let opened = drink(&setup(vec![], vec![Potion::Attack, Potion::Block]));
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.block, 0);
    let next = step(
        &pending,
        Some(RunAction::ChooseCombatCardReward { index: 0 }),
    );
    assert_eq!(next.combat.as_ref().unwrap().player.block, 12);
}
#[test]
fn block_waits_behind_toolbox_reward() {
    let r = setup(vec![Relic::Toolbox], vec![Potion::Block]);
    let pending = drink(&r);
    assert_eq!(pending.combat.as_ref().unwrap().player.block, 0);
    let next = step(
        &pending,
        Some(RunAction::ChooseCombatCardReward { index: 0 }),
    );
    assert_eq!(next.combat.as_ref().unwrap().player.block, 12);
}
#[test]
fn block_waits_behind_nilry_then_protects_before_monster_turn() {
    let r = setup(vec![Relic::NilrysCodex], vec![Potion::Block]);
    let opened = step(&r, None);
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.block, 0);
    let next = step(&pending, Some(RunAction::SkipCombatCardReward));
    assert_eq!(next.combat.as_ref().unwrap().player.hp, 80);
    assert_eq!(next.combat.as_ref().unwrap().monsters[0].moves_executed, 1);
}
#[test]
fn block_and_use_heal_wait_behind_reward() {
    let mut r = setup(
        vec![Relic::ToyOrnithopter],
        vec![Potion::Attack, Potion::Block],
    );
    r.combat.as_mut().unwrap().player.hp = 30;
    let opened = drink(&r);
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.hp, 30);
    assert_eq!(pending.combat.as_ref().unwrap().player.block, 0);
    let next = step(
        &pending,
        Some(RunAction::ChooseCombatCardReward { index: 0 }),
    );
    assert_eq!(next.combat.as_ref().unwrap().player.hp, 40);
    assert_eq!(next.combat.as_ref().unwrap().player.block, 12);
}
#[test]
fn block_hook_waits_after_earlier_snecko_randomizer_in_fifo_queue() {
    let mut r = setup(
        vec![],
        vec![Potion::Attack, Potion::Block, Potion::SneckoOil],
    );
    r.combat.as_mut().unwrap().player.powers.juggernaut = 5;
    let mut other = r.combat.as_ref().unwrap().monsters[0].clone();
    other.id = sts_core::adapter_internals::MonsterId::new(2);
    r.combat.as_mut().unwrap().monsters.push(other);
    let mut no_hook_initial = r.clone();
    no_hook_initial
        .combat
        .as_mut()
        .unwrap()
        .player
        .powers
        .juggernaut = 0;
    let no_hook = drink(&drink(&drink(&no_hook_initial)));
    let opened = drink(&r);
    let blocked = drink(&opened);
    let pending = drink(&blocked);
    let control = step(
        &no_hook,
        Some(RunAction::ChooseCombatCardReward { index: 0 }),
    );
    let next = step(
        &pending,
        Some(RunAction::ChooseCombatCardReward { index: 0 }),
    );
    assert_eq!(
        next.combat.as_ref().unwrap().piles.hand,
        control.combat.as_ref().unwrap().piles.hand
    );
    assert_eq!(
        next.combat
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .map(|m| m.hp)
            .sum::<i32>(),
        1995
    );
    assert_eq!(
        next.card_random_rng_counter,
        next.combat.as_ref().unwrap().rng.card_random_rng.counter()
    );
}
#[test]
fn queued_use_heal_precedes_lethal_juggernaut_follow_up() {
    let mut r = setup(vec![Relic::ToyOrnithopter], vec![Potion::Block]);
    let c = r.combat.as_mut().unwrap();
    c.player.hp = 30;
    c.player.powers.juggernaut = 5;
    c.monsters[0].hp = 5;
    c.monsters[0].max_hp = 5;
    let next = drink(&r);
    assert_eq!(next.hp, 35);
    assert_ne!(next.phase, sts_core::adapter_internals::RunPhase::Combat);
}
#[test]
fn potion_block_ignores_card_block_modifiers_control() {
    let mut r = setup(vec![], vec![Potion::Block]);
    let c = r.combat.as_mut().unwrap();
    c.player.no_block_turns = 2;
    c.player.powers.dexterity = 20;
    c.player.powers.frail = 1;
    let next = drink(&r);
    assert_eq!(next.combat.as_ref().unwrap().player.block, 12);
}
#[test]
fn nilry_lethal_power_retains_queued_gain_block_diagnostic() {
    // Direct paused-turn infrastructure diagnostic: not an accepted gameplay
    // prefix or real-game parity evidence. Check the retained combat block
    // before the run's victory handoff discards the combat payload.
    let mut c = setup(vec![], vec![]).combat.unwrap();
    c.monsters[0].hp = 5;
    c.player.powers.combust = 1;
    c.player.powers.combust_damage = 5;
    c.resume_end_turn_after_nilrys_codex = true;
    c.nilrys_end_powers_pending = true;
    c.pending_nilrys_codex_potion_actions
        .push_back(sts_core::adapter_internals::InternalAction::GainBlockFromPotion { amount: 12 });
    let before = serde_json::to_value(&c).unwrap();
    let restored = serde_json::from_value(before.clone()).unwrap();
    let next = sts_core::adapter_internals::combat::end_player_turn(&c).unwrap();
    assert_eq!(serde_json::to_value(&c).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(
            sts_core::adapter_internals::combat::end_player_turn(&restored).unwrap()
        )
        .unwrap()
    );
    assert_eq!(next.phase, sts_core::adapter_internals::CombatPhase::Won);
    assert_eq!(next.player.block, 12);
}

#[test]
fn malformed_overflow_keeps_potion_error_and_immutable_input() {
    let mut r = setup(vec![], vec![Potion::Block]);
    r.combat.as_mut().unwrap().player.block = i32::MAX;
    let before = serde_json::to_value(&r).unwrap();
    assert!(matches!(
        apply_run_action(
            &r,
            RunAction::UsePotion {
                slot: 0,
                target: None
            }
        ),
        Err(sts_core::SimError::InvalidState(
            "combat potion stat gain overflows i32"
        ))
    ));
    assert_eq!(serde_json::to_value(r).unwrap(), before);
}
