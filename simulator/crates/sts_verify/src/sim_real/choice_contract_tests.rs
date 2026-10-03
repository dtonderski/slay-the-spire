use super::*;
use sts_core::adapter_internals::{CardRewardFlow, ShopPotionSlot, ShopScreen};

fn fixtures() -> Value {
    // Synthetic contract fixtures shared with actual producer and client tests.
    // They do not hydrate replay or establish real-game parity.
    serde_json::from_str(include_str!(
        "../../../../mods/CommunicationMod/src/test/resources/choice-contract.json"
    ))
    .unwrap()
}

#[test]
fn full_belt_does_not_reindex_a_card_after_a_potion_reward() {
    let fixtures = fixtures();
    let mut run = RunState::seeded_ironclad(1, 0);
    for _ in 0..run.potion_capacity() {
        run.gain_potion(Potion::Fire).unwrap();
    }
    run.reward = Some(RewardScreen {
        continuation: RewardContinuation::Map,
        choices: vec![],
        queued_card_rewards: vec![],
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: Some(Potion::Fire),
        potion_offers: vec![],
        relic_offer: None,
        pending_relic_offer: None,
        queued_relic_offers: vec![],
        boss_relic_choices: vec![],
        card_reward_flow: CardRewardFlow::pending(1),
    });
    let before = serde_json::to_value(&run).unwrap();
    let choices = sim_reward_combat_choices(&run, run.reward.as_ref().unwrap());
    assert_eq!(
        json!(choices),
        fixtures["cases"]["reward_full"]["game_state"]["choice_list"]
    );
    let index = fixtures["cases"]["reward_full"]["game_state"]["selectable_choice_indices"][0]
        .as_u64()
        .unwrap() as usize;
    assert_eq!(index, 1);
    assert_eq!(
        seed_start_bind_reward_choose_action(&run, index).unwrap(),
        RunAction::OpenCardReward
    );
    assert_eq!(serde_json::to_value(&run).unwrap(), before);
    run.take_potion_slot(0).unwrap();
    assert_eq!(
        json!(sim_reward_combat_choices(
            &run,
            run.reward.as_ref().unwrap()
        )),
        fixtures["cases"]["reward_open"]["game_state"]["choice_list"]
    );
}

#[test]
fn shop_sozu_and_capacity_do_not_change_offer_labels_or_indices() {
    let fixtures = fixtures();
    for (name, sozu, full) in [
        ("shop_sozu", true, false),
        ("shop_full", false, true),
        ("shop_open", false, false),
    ] {
        let mut run = RunState::seeded_ironclad(1, 0);
        if sozu {
            run.gain_relic(Relic::Sozu).unwrap();
        }
        if full {
            for _ in 0..run.potion_capacity() {
                run.gain_potion(Potion::Fire).unwrap();
            }
        }
        run.shop = Some(ShopScreen {
            cards: vec![],
            relics: vec![],
            potions: vec![ShopPotionSlot {
                potion: Potion::Fire,
                price: 7,
                sold: false,
            }],
            remove_cost: 25,
            remove_available: true,
            sale_slot: None,
        });
        let before = serde_json::to_value(&run).unwrap();
        assert_eq!(
            json!(seed_start_shop_trace_choice_labels(&run)),
            fixtures["cases"][name]["game_state"]["choice_list"],
            "{name}"
        );
        assert_eq!(serde_json::to_value(&run).unwrap(), before);
    }
}
