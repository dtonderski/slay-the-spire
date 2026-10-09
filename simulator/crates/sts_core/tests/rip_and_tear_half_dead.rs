use sts_core::adapter_internals::{
    apply_combat_action,
    content::{
        cards::RIP_AND_TEAR_ANY_COLOR_ID,
        monsters::{mark_awakened_one_half_dead, monster_state, AWAKENED_ONE_A0},
    },
    legal_combat_actions, CardId, CardInstance, CombatAction, CombatState, MonsterId, Relic,
};

fn check_half_dead(upgraded: bool, copied: bool) {
    // Reduced from endurance fuzz seed 150554. RipAndTear.use queues two
    // NewRipAndTearAction instances; AttackDamageRandomEnemyAction.update
    // completes without damage when MonsterGroup.getRandomMonster returns null.
    let mut state = CombatState::initial_fixture();
    let mut card = CardInstance::new(CardId::new(1), RIP_AND_TEAR_ANY_COLOR_ID);
    card.upgrades = u8::from(upgraded);
    if copied {
        card.temp_cost = Some(2);
        state.player.authority.relics = vec![Relic::Necronomicon];
    }
    state.player.energy = 3;
    state.piles.hand = vec![card];
    state.piles.discard_pile.clear();
    let mut awakened = monster_state(&AWAKENED_ONE_A0, MonsterId::new(1));
    assert!(mark_awakened_one_half_dead(&mut awakened));
    state.monsters = vec![awakened];
    let action = CombatAction::PlayCard {
        card_id: CardId::new(1),
        target: None,
    };
    assert!(legal_combat_actions(&state).unwrap().contains(&action));
    let next =
        apply_combat_action(&state, action).expect("legal random attack must fizzle normally");
    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.discard_pile, vec![card]);
    assert_eq!(next.player.energy, if copied { 1 } else { 2 });
    assert_eq!(next.monsters, state.monsters);
    assert_eq!(
        next.rng, state.rng,
        "no eligible target means no random target draw"
    );
    assert_eq!(next.relic_counters.necronomicon_used_this_turn, copied);
    next.validate().unwrap();
}

#[test]
fn base_rip_and_tear_plays_against_half_dead_awakened_one() {
    check_half_dead(false, false);
}
#[test]
fn upgraded_rip_and_tear_plays_against_half_dead_awakened_one() {
    check_half_dead(true, false);
}
#[test]
fn copied_base_rip_and_tear_fizzles_without_relocating_original() {
    check_half_dead(false, true);
}
#[test]
fn copied_upgraded_rip_and_tear_fizzles_without_relocating_original() {
    check_half_dead(true, true);
}
