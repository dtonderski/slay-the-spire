//! Source-backed synthetic legal potion FIFO prefixes, not dedicated trace parity.
use sts_core::adapter_internals::{
    apply_run_action, CardId, CardInstance, Potion, Relic, RunAction, RunPhase, RunState,
};
use sts_core::content::cards::{BURN_ID, STRIKE_R_ID};

fn step(run: &RunState, action: RunAction) -> RunState {
    let before = serde_json::to_value(run).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let next = apply_run_action(run, action).unwrap();
    next.validate().unwrap();
    assert_eq!(serde_json::to_value(run).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(apply_run_action(&restored, action).unwrap()).unwrap()
    );
    next
}

#[test]
fn lethal_draw_callback_cancels_later_selector_but_retains_its_use_heal() {
    for selector in [Potion::Elixir, Potion::GamblersBrew] {
        let mut run = RunState::combat_fixture_with_relics(vec![
            Relic::PotionBelt,
            Relic::ToyOrnithopter,
            Relic::BurningBlood,
        ]);
        run.potions = vec![Potion::Attack, Potion::Swift, Potion::Skill, selector];
        let c = run.combat.as_mut().unwrap();
        c.player.hp = 30;
        c.player.powers.fire_breathing = 6;
        c.monsters[0].hp = 6;
        c.piles.hand = vec![CardInstance::new(CardId::new(100), STRIKE_R_ID)];
        c.piles.draw_pile = (200..220)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        c.piles.draw_pile.last_mut().unwrap().content_id = BURN_ID;
        c.piles.discard_pile.clear();
        c.piles.exhaust_pile.clear();
        run.validate().unwrap();
        for slot in 0..3 {
            run = step(&run, RunAction::UsePotion { slot, target: None });
        }
        run = step(&run, RunAction::ChooseCombatCardReward { index: 0 });
        assert!(run
            .combat
            .as_ref()
            .unwrap()
            .potion_card_reward_choices()
            .is_some());
        assert_eq!(run.combat.as_ref().unwrap().player.hp, 40);
        run = step(
            &run,
            RunAction::UsePotion {
                slot: 3,
                target: None,
            },
        );
        run = step(&run, RunAction::SkipCombatCardReward);
        assert_eq!(run.phase, RunPhase::Reward);
        assert_eq!(run.hp, 56, "all four use heals must precede Burning Blood");
        assert!(run.potions.is_empty());
    }
}
