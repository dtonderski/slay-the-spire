//! Production-path regression tests for the hand-played Discovery lifecycle.
//!
//! The installed target burns 15 leftover post-select generateCardChoices
//! generations on `CHOOSE` (ACTION_DUR_FAST 0.25s at 1/60s ticks minus the
//! opening visible offer), then retrieves the visible card and settles the
//! source through the ordinary action queue.

use sts_core::{
    apply_combat_action_on_run, apply_run_action,
    content::{cards::DISCOVERY_ID, shop_pool::burn_all_discovery_card_choice_generations},
    CardId, CardInstance, CombatAction, CombatDecisionState, RunAction, RunState,
};

fn run_with_hand_played_discovery() -> RunState {
    let mut run = RunState::combat_fixture();
    let combat = run.combat.as_mut().expect("combat fixture");
    combat.player.energy = 1;
    combat.player.powers.feel_no_pain = 1;
    combat.piles.hand = vec![CardInstance::new(CardId::new(1), DISCOVERY_ID)];
    combat.piles.discard_pile.clear();
    combat.piles.exhaust_pile.clear();
    run
}

#[test]
fn discovery_open_uses_only_the_visible_offer_generation() {
    let run = run_with_hand_played_discovery();
    let before_counter = run
        .combat
        .as_ref()
        .expect("combat fixture")
        .rng
        .card_random_rng
        .counter();

    let opened = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Discovery opens its card reward");
    let combat = opened.combat.as_ref().expect("combat remains open");

    assert_eq!(
        combat.rng.card_random_rng.counter(),
        before_counter + 3,
        "opening Discovery consumes exactly its visible three-card offer"
    );
    match combat.decision.as_ref() {
        Some(CombatDecisionState::DiscoveryCardReward {
            choices,
            source_card,
            pending_actions,
            ..
        }) => {
            assert_eq!(choices.len(), 3);
            assert_eq!(
                source_card.as_ref().map(|card| card.content_id),
                Some(DISCOVERY_ID)
            );
            assert!(pending_actions.is_empty());
        }
        other => panic!("expected an open Discovery reward, got {other:?}"),
    }
    assert_eq!(
        combat.player.block, 0,
        "source exhaust is deferred until CHOOSE"
    );
}

#[test]
fn discovery_choose_retrieves_visible_offer_and_closes_source_through_queue() {
    let run = run_with_hand_played_discovery();
    let before_counter = run
        .combat
        .as_ref()
        .expect("combat fixture")
        .rng
        .card_random_rng
        .counter();
    let opened = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Discovery opens its card reward");
    let open_counter = opened
        .combat
        .as_ref()
        .expect("combat remains open")
        .rng
        .card_random_rng
        .counter();
    assert_eq!(
        open_counter,
        before_counter + 3,
        "opening Discovery consumes exactly its visible three-card offer"
    );
    let selected_content = opened
        .combat
        .as_ref()
        .expect("combat remains open")
        .discovery_card_reward_choices()
        .expect("Discovery choices")
        .first()
        .expect("visible choice")
        .content_id;

    let chosen = apply_run_action(&opened, RunAction::ChooseCombatCardReward { index: 0 })
        .expect("choose the Discovery card");
    let combat = chosen.combat.as_ref().expect("combat remains open");
    let leftover_counter = {
        let mut rng = opened
            .combat
            .as_ref()
            .expect("combat remains open")
            .rng
            .card_random_rng
            .clone();
        burn_all_discovery_card_choice_generations(&mut rng, 3, 15);
        rng.counter()
    };

    assert_eq!(
        combat.rng.card_random_rng.counter(),
        leftover_counter,
        "CHOOSE burns 15 leftover generateCardChoices generations (0.25s / 1/60s minus opening tick)"
    );
    assert!(combat.decision.is_none(), "the reward closes on CHOOSE");
    assert!(combat.queued_decisions.is_empty());
    assert_eq!(
        combat.piles.hand.last().map(|card| card.content_id),
        Some(selected_content)
    );
    assert!(combat
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.content_id == DISCOVERY_ID));
    assert_eq!(
        combat.player.block, 1,
        "source exhaust reaches Feel No Pain through the normal action queue"
    );
}
