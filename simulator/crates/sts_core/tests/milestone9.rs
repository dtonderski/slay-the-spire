use sts_core::{
    apply_map_action_on_run, apply_rest_action, apply_run_action,
    content::cards::ANGER_ID,
    content::cards::{STRIKE_R_ID, STRIKE_R_PLUS_ID},
    content::character::IRONCLAD_A0_BASE_HP,
    legal_map_actions_on_run, legal_rest_actions, legal_shop_actions, rest_heal_amount, MapAction,
    MapNodeId, RestAction, RoomKind, RunAction, RunPhase, RunState, SimError, SHOP_ANGER_PRICE,
};

fn reach_shop_via_left_branch() -> RunState {
    let mut run = RunState::map_fixture();
    for node_id in [MapNodeId::new(1), MapNodeId::new(3), MapNodeId::new(4)] {
        run = apply_map_action_on_run(&run, MapAction::ChooseNode { node_id }).expect("reach shop");
    }
    run
}

#[test]
fn rest_heal_restores_thirty_percent_max_hp_floored() {
    let mut run = RunState::map_fixture();
    run.phase = RunPhase::Rest;
    run.player_hp = 30;
    run.player_max_hp = 80;

    let after = apply_rest_action(&run, RestAction::Heal).expect("heal applies");

    assert_eq!(rest_heal_amount(80), 24);
    assert_eq!(after.player_hp, 54);
    assert_eq!(after.phase, RunPhase::Idle);
}

#[test]
fn rest_heal_caps_at_max_hp() {
    let mut run = RunState::map_fixture();
    run.phase = RunPhase::Rest;
    run.player_hp = IRONCLAD_A0_BASE_HP - 3;

    let after = apply_rest_action(&run, RestAction::Heal).expect("heal applies");

    assert_eq!(after.player_hp, IRONCLAD_A0_BASE_HP);
}

#[test]
fn entering_rest_room_exposes_heal_and_blocks_map_actions() {
    let mut run = RunState::map_fixture();
    run.player_hp = 40;

    assert!(!legal_map_actions_on_run(&run).is_empty());

    run = apply_map_action_on_run(
        &run,
        MapAction::ChooseNode {
            node_id: MapNodeId::new(2),
        },
    )
    .expect("enter rest");

    assert_eq!(run.phase, RunPhase::Rest);
    assert_eq!(
        run.map
            .as_ref()
            .and_then(|map| map.map.node(map.current_node))
            .map(|node| node.room_kind),
        Some(RoomKind::Rest)
    );
    let mut expected = vec![RestAction::Heal];
    for card in &run.deck {
        expected.push(RestAction::RemoveCard { card_id: card.id });
        if sts_core::content::cards::upgrade_content_id(card.content_id).is_some() {
            expected.push(RestAction::Smith { card_id: card.id });
        }
    }
    assert_eq!(legal_rest_actions(&run), expected);
    assert!(legal_map_actions_on_run(&run).is_empty());
}

#[test]
fn heal_then_map_traversal_continues() {
    let mut run = RunState::map_fixture();
    run.player_hp = 40;

    run = apply_map_action_on_run(
        &run,
        MapAction::ChooseNode {
            node_id: MapNodeId::new(2),
        },
    )
    .expect("enter rest");
    run = apply_rest_action(&run, RestAction::Heal).expect("heal");

    assert_eq!(run.phase, RunPhase::Idle);
    assert_eq!(run.player_hp, 64);
    assert_eq!(
        legal_map_actions_on_run(&run),
        vec![MapAction::ChooseNode {
            node_id: MapNodeId::new(3)
        }]
    );
}

#[test]
fn rest_heal_is_illegal_outside_rest_phase() {
    let run = RunState::map_fixture();

    let err = apply_rest_action(&run, RestAction::Heal).expect_err("not at rest");

    assert_eq!(
        err,
        SimError::IllegalAction("rest actions require rest phase")
    );
}

#[test]
fn smith_upgrades_strike_r_to_strike_r_plus() {
    let mut run = RunState::map_fixture();
    run.phase = RunPhase::Rest;
    let strike_id = run
        .deck
        .iter()
        .find(|card| card.content_id == STRIKE_R_ID)
        .expect("strike in deck")
        .id;

    let after =
        apply_rest_action(&run, RestAction::Smith { card_id: strike_id }).expect("smith applies");

    assert_eq!(after.count_content_in_deck(STRIKE_R_PLUS_ID), 1);
    assert_eq!(after.count_content_in_deck(STRIKE_R_ID), 4);
    assert_eq!(after.phase, RunPhase::Idle);
}

#[test]
fn smith_then_map_traversal_continues() {
    let mut run = RunState::map_fixture();
    run = apply_map_action_on_run(
        &run,
        MapAction::ChooseNode {
            node_id: MapNodeId::new(2),
        },
    )
    .expect("enter rest");
    let strike_id = run.deck[0].id;

    run = apply_rest_action(&run, RestAction::Smith { card_id: strike_id }).expect("smith");

    assert_eq!(run.phase, RunPhase::Idle);
    assert_eq!(run.deck[0].content_id, STRIKE_R_PLUS_ID);
    assert_eq!(
        legal_map_actions_on_run(&run),
        vec![MapAction::ChooseNode {
            node_id: MapNodeId::new(3)
        }]
    );
}

#[test]
fn smith_is_illegal_outside_rest_phase() {
    let run = RunState::map_fixture();
    let strike_id = run.deck[0].id;

    let err =
        apply_rest_action(&run, RestAction::Smith { card_id: strike_id }).expect_err("not at rest");

    assert_eq!(
        err,
        SimError::IllegalAction("rest actions require rest phase")
    );
}

#[test]
fn entering_shop_room_exposes_anger_and_blocks_map_actions() {
    let run = reach_shop_via_left_branch();

    assert_eq!(run.phase, RunPhase::Shop);
    assert_eq!(
        run.map
            .as_ref()
            .and_then(|map| map.map.node(map.current_node))
            .map(|node| node.room_kind),
        Some(RoomKind::Shop)
    );
    let shop = run.shop.as_ref().expect("shop screen");
    assert_eq!(shop.cards.len(), 1);
    assert_eq!(shop.cards[0].price, SHOP_ANGER_PRICE);
    assert_eq!(shop.cards[0].card.content_id, ANGER_ID);
    assert_eq!(
        legal_shop_actions(&run),
        vec![RunAction::BuyShopCard { slot: 0 }]
    );
    assert!(legal_map_actions_on_run(&run).is_empty());
}

#[test]
fn buy_shop_card_then_map_traversal_continues() {
    let run = reach_shop_via_left_branch();
    let gold_before = run.gold;
    let deck_len_before = run.deck.len();

    let run = apply_run_action(&run, RunAction::BuyShopCard { slot: 0 }).expect("buy anger");

    assert_eq!(run.phase, RunPhase::Idle);
    assert!(run.shop.is_none());
    assert_eq!(run.gold, gold_before - SHOP_ANGER_PRICE);
    assert_eq!(run.deck.len(), deck_len_before + 1);
    assert_eq!(run.count_content_in_deck(ANGER_ID), 1);
    assert_eq!(
        legal_map_actions_on_run(&run),
        vec![MapAction::ChooseNode {
            node_id: MapNodeId::new(5)
        }]
    );
}

#[test]
fn buy_shop_card_is_illegal_outside_shop_phase() {
    let run = RunState::map_fixture();

    let err = apply_run_action(&run, RunAction::BuyShopCard { slot: 0 }).expect_err("not at shop");

    assert_eq!(
        err,
        SimError::IllegalAction("shop actions require shop phase")
    );
}

#[test]
fn remove_card_at_rest_drops_strike_from_deck() {
    let mut run = RunState::map_fixture();
    run = apply_map_action_on_run(
        &run,
        MapAction::ChooseNode {
            node_id: MapNodeId::new(2),
        },
    )
    .expect("enter rest");
    let strike_id = run
        .deck
        .iter()
        .find(|card| card.content_id == STRIKE_R_ID)
        .expect("strike in deck")
        .id;
    let deck_len_before = run.deck.len();

    let after = apply_rest_action(&run, RestAction::RemoveCard { card_id: strike_id })
        .expect("remove applies");

    assert_eq!(after.deck.len(), deck_len_before - 1);
    assert!(!after.deck.iter().any(|card| card.id == strike_id));
    assert_eq!(after.count_content_in_deck(STRIKE_R_ID), 4);
    assert_eq!(after.phase, RunPhase::Idle);
}
