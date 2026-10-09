//! Direct internal-queue diagnostics, separate from enumerated legal prefixes.
use super::*;
use crate::content::cards::STRIKE_R_ID;
fn setup() -> CombatState {
    let mut state = crate::run::RunState::combat_fixture().combat.unwrap();
    state.piles.hand = vec![CardInstance::new(CardId::new(100), STRIKE_R_ID)];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(200), STRIKE_R_ID)];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    state.player.hp = 30;
    state.player.powers.confusion = 1;
    state.monsters[0].hp = 1;
    state
}
fn run(state: &CombatState, queue: VecDeque<InternalAction>) -> CombatState {
    let before = serde_json::to_value(state).unwrap();
    let restored: CombatState = serde_json::from_value(before.clone()).unwrap();
    let result = process_internal_queue(state, queue.clone()).unwrap().state;
    result.validate().unwrap();
    assert_eq!(serde_json::to_value(state).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        serde_json::to_value(process_internal_queue(&restored, queue).unwrap().state).unwrap()
    );
    result
}
fn damage() -> InternalAction {
    InternalAction::DealDamage {
        info: DamageInfo {
            source: DamageSource::Card(CardId::new(100)),
            target: MonsterId::new(1),
            amount: 10,
        },
    }
}
fn tail() -> VecDeque<InternalAction> {
    VecDeque::from([
        InternalAction::DrawCards { count: 1 },
        InternalAction::RandomizeHandCostsForSneckoOil,
        InternalAction::GainEnergy { amount: 2 },
        InternalAction::GainEnergyFromPotion { amount: 2 },
        InternalAction::HealPlayer { amount: 5 },
        InternalAction::GainBlockFromPotion { amount: 12 },
        InternalAction::MoveCard {
            card_id: CardId::new(100),
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ])
}
#[test]
fn queued_cost_draw_and_energy_cancel_but_heal_block_and_use_settlement_survive() {
    let state = setup();
    let mut queue = tail();
    queue.push_front(damage());
    let n = run(&state, queue);
    assert_eq!(
        n.rng.card_random_rng.counter(),
        state.rng.card_random_rng.counter()
    );
    assert!(n.piles.hand.is_empty());
    assert_eq!(n.piles.draw_pile.len(), 1);
    assert_eq!(n.player.energy, 3);
    assert_eq!(n.player.hp, 35);
    assert_eq!(n.player.block, 12);
    assert_eq!(n.piles.discard_pile.len(), 1);
}
#[test]
fn actions_enqueued_after_clear_are_not_repeatedly_cancelled() {
    let mut state = setup();
    state.monsters[0].alive = false;
    state.monsters[0].hp = 0;
    let n = run(&state, tail());
    assert_eq!(n.rng.card_random_rng.counter(), 3);
    assert_eq!(n.player.energy, 7);
    assert_eq!(n.piles.hand.len(), 1);
}
#[test]
fn later_thorns_damage_clears_newly_pending_actions_again() {
    // DamageAction with THORNS still updates in a dead room and invokes clear.
    let mut state = setup();
    state.monsters[0].alive = false;
    state.monsters[0].hp = 0;
    let mut queue = tail();
    queue.push_front(InternalAction::DealThornsDamageToPlayer { amount: 1 });
    let n = run(&state, queue);
    assert_eq!(
        n.rng.card_random_rng.counter(),
        state.rng.card_random_rng.counter()
    );
    assert_eq!(n.player.energy, 3);
    assert_eq!(n.player.hp, 34);
}

#[test]
fn later_damage_all_clears_newly_pending_actions_again() {
    let mut state = setup();
    state.monsters[0].alive = false;
    state.monsters[0].hp = 0;
    let mut queue = tail();
    queue.push_front(InternalAction::FireBreathingDamage { amount: 6 });
    let n = run(&state, queue);
    assert_eq!(
        n.rng.card_random_rng.counter(),
        state.rng.card_random_rng.counter()
    );
    assert_eq!(n.player.energy, 3);
}

#[test]
fn potion_offer_enqueued_after_clear_is_not_perpetually_cancelled() {
    let mut state = setup();
    state.monsters[0].alive = false;
    state.monsters[0].hp = 0;
    let n = run(
        &state,
        VecDeque::from([InternalAction::OpenPotionCardReward {
            reward_kind: crate::combat::PotionCardRewardKind::Skill,
        }]),
    );
    assert!(n.rng.card_random_rng.counter() > state.rng.card_random_rng.counter());
}

#[test]
fn awakened_one_half_death_does_not_cancel_queued_draw_cost_or_energy() {
    let mut state = setup();
    state.monsters[0].content_id = crate::content::monsters::AWAKENED_ONE_ID;
    state.monsters[0].mode_shift = 0;
    let mut queue = tail();
    queue.push_front(damage());
    let n = run(&state, queue);
    assert!(awakened_one_is_half_dead(&n.monsters[0]));
    assert_eq!(n.rng.card_random_rng.counter(), 3);
    assert_eq!(n.player.energy, 7);
    assert_eq!(n.piles.hand.len(), 1);
}
