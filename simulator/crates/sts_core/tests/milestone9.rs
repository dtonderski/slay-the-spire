use sts_core::adapter_internals::{
    apply_event_action, apply_map_action_on_run, apply_rest_action, apply_run_action,
    content::cards::upgrade_content_id,
    content::cards::ANGER_ID,
    content::cards::{PARASITE_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID},
    content::character::IRONCLAD_A0_BASE_HP,
    enter_event_screen, leave_shop_room, legal_event_actions, legal_map_actions_on_run,
    legal_rest_actions, legal_shop_actions, rest_heal_amount, validate_event_action, CardId,
    CardInstance, Event, EventAction, EventChoice, EventScreen, MapAction, MapNodeId, Potion,
    Relic, RestAction, RoomKind, RunAction, RunPhase, RunState, ShopCardSlot, ShopPotionSlot,
    ShopRelicSlot, ShopScreen, SimError, FIRE_POTION_DAMAGE, GOLDEN_SHRINE_GOLD, VAJRA_STRENGTH,
};

const SHOP_ANGER_PRICE: i32 = 50;
const SHOP_VAJRA_PRICE: i32 = 150;
const SHOP_FIRE_POTION_PRICE: i32 = 50;

fn legacy_fixed_shop_screen(next_card_id: u64) -> ShopScreen {
    ShopScreen {
        cards: vec![ShopCardSlot {
            card: CardInstance::new(CardId::new(next_card_id), ANGER_ID),
            price: SHOP_ANGER_PRICE,
            sold: false,
        }],
        relics: vec![ShopRelicSlot {
            relic_key: Relic::Vajra,
            price: SHOP_VAJRA_PRICE,
            sold: false,
        }],
        potions: vec![ShopPotionSlot {
            potion: Potion::Fire,
            price: SHOP_FIRE_POTION_PRICE,
            sold: false,
        }],
        remove_cost: 75,
        remove_available: true,
        sale_slot: None,
    }
}

fn enter_fixed_event_screen_fixture(run: &mut RunState) {
    let screen = sts_core::adapter_internals::event_screen(Event::GoldenShrine);
    run.phase = RunPhase::Event;
    run.event = Some(screen);
}

fn reach_shop_via_left_branch() -> RunState {
    let mut run = RunState::map_fixture();
    for node_id in [MapNodeId::new(1), MapNodeId::new(3), MapNodeId::new(4)] {
        run = apply_map_action_on_run(&run, MapAction::ChooseNode { node_id }).expect("reach shop");
        if run.phase == RunPhase::Combat {
            let combat = run.combat.take().expect("combat owner");
            run.run_player = Some(combat.player.authority);
            run.phase = RunPhase::Idle;
        } else if run.phase == RunPhase::Event {
            run.phase = RunPhase::Idle;
            run.event = None;
        } else if run.phase == RunPhase::Treasure {
            run.phase = RunPhase::Idle;
            run.treasure_room = None;
        }
    }
    let mut run = apply_run_action(&run, RunAction::EnterShop).expect("open merchant");
    run.shop = Some(legacy_fixed_shop_screen(
        run.next_card_instance_id()
            .expect("fixture has card ID allocation headroom"),
    ));
    run
}

fn proceed_rest(run: &RunState) -> RunState {
    apply_rest_action(run, RestAction::Proceed).expect("proceed from rest")
}

fn rest_fixture() -> RunState {
    let mut run = RunState::map_fixture();
    run.phase = RunPhase::Rest;
    run.current_room_override = Some(RoomKind::Rest);
    run
}

fn valid_map_actions(run: &RunState) -> Vec<MapAction> {
    legal_map_actions_on_run(run).expect("valid run map state")
}

fn valid_event_actions(run: &RunState) -> Vec<EventAction> {
    legal_event_actions(run).expect("valid run event state")
}

fn valid_rest_actions(run: &RunState) -> Vec<RestAction> {
    legal_rest_actions(run).expect("valid run rest state")
}

fn valid_shop_actions(run: &RunState) -> Vec<RunAction> {
    legal_shop_actions(run).expect("valid run shop state")
}

fn leave_shop_merchant_and_room(mut run: RunState) -> RunState {
    run = apply_run_action(&run, RunAction::LeaveShop).expect("leave merchant");
    leave_shop_room(&mut run);
    run
}

#[test]
fn rest_heal_restores_thirty_percent_max_hp_floored() {
    let mut run = rest_fixture();
    run.hp = 30;
    run.max_hp = 80;

    let after = apply_rest_action(&run, RestAction::Heal).expect("heal applies");
    let after = proceed_rest(&after);

    assert_eq!(rest_heal_amount(80), 24);
    assert_eq!(after.hp, 54);
    assert_eq!(after.phase, RunPhase::Idle);
}

#[test]
fn rest_heal_caps_at_max_hp() {
    let mut run = rest_fixture();
    run.hp = IRONCLAD_A0_BASE_HP - 3;

    let after = apply_rest_action(&run, RestAction::Heal).expect("heal applies");

    assert_eq!(after.hp, IRONCLAD_A0_BASE_HP);
}

#[test]
fn entering_rest_room_exposes_heal_and_blocks_map_actions() {
    let mut run = RunState::map_fixture();
    run.hp = 40;

    assert!(!valid_map_actions(&run).is_empty());

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
    if run
        .deck
        .iter()
        .any(|card| upgrade_content_id(card.content_id).is_some())
    {
        expected.push(RestAction::OpenSmith);
    }
    for card in &run.deck {
        if upgrade_content_id(card.content_id).is_some() {
            expected.push(RestAction::Smith { card_id: card.id });
        }
    }
    assert_eq!(valid_rest_actions(&run), expected);
    assert!(valid_map_actions(&run).is_empty());
}

#[test]
fn heal_then_map_traversal_continues() {
    let mut run = RunState::map_fixture();
    run.hp = 40;

    run = apply_map_action_on_run(
        &run,
        MapAction::ChooseNode {
            node_id: MapNodeId::new(2),
        },
    )
    .expect("enter rest");
    run = apply_rest_action(&run, RestAction::Heal).expect("heal");
    run = proceed_rest(&run);

    assert_eq!(run.phase, RunPhase::Idle);
    assert_eq!(run.hp, 64);
    assert_eq!(
        valid_map_actions(&run),
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
    let run = rest_fixture();
    let strike_id = run
        .deck
        .iter()
        .find(|card| card.content_id == STRIKE_R_ID)
        .expect("strike in deck")
        .id;

    let after =
        apply_rest_action(&run, RestAction::Smith { card_id: strike_id }).expect("smith applies");
    let after = proceed_rest(&after);

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
    run = proceed_rest(&run);

    assert_eq!(run.phase, RunPhase::Idle);
    assert_eq!(run.deck[0].content_id, STRIKE_R_PLUS_ID);
    assert_eq!(
        valid_map_actions(&run),
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
fn explicit_legacy_shop_fixture_exposes_anger_and_blocks_map_actions() {
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
    let relic = shop.relics[0];
    assert_eq!(relic.relic_key, Relic::Vajra);
    assert_eq!(relic.price, SHOP_VAJRA_PRICE);
    let potion = shop.potions[0];
    assert_eq!(potion.potion, Potion::Fire);
    assert_eq!(potion.price, SHOP_FIRE_POTION_PRICE);
    assert_eq!(
        valid_shop_actions(&run),
        vec![
            RunAction::OpenShopRemove,
            RunAction::BuyShopCard { slot: 0 },
            RunAction::BuyShopPotion { slot: 0 },
            RunAction::LeaveShop,
        ]
    );
    assert!(valid_map_actions(&run).is_empty());
}

#[test]
fn buy_shop_card_then_map_traversal_continues() {
    let run = reach_shop_via_left_branch();
    let gold_before = run.gold;
    let deck_len_before = run.deck.len();

    let run = leave_shop_merchant_and_room(
        apply_run_action(&run, RunAction::BuyShopCard { slot: 0 }).expect("buy anger"),
    );

    assert_eq!(run.phase, RunPhase::Idle);
    assert!(run.shop.is_none());
    assert_eq!(run.gold, gold_before - SHOP_ANGER_PRICE);
    assert_eq!(run.deck.len(), deck_len_before + 1);
    assert_eq!(run.count_content_in_deck(ANGER_ID), 1);
    assert_eq!(
        valid_map_actions(&run),
        vec![MapAction::ChooseNode {
            node_id: MapNodeId::new(5)
        }]
    );
}

#[test]
fn buy_shop_potion_adds_fire_potion_to_belt() {
    let run = reach_shop_via_left_branch();

    let run = leave_shop_merchant_and_room(
        apply_run_action(&run, RunAction::BuyShopPotion { slot: 0 }).expect("buy potion"),
    );

    assert_eq!(run.phase, RunPhase::Idle);
    assert_eq!(run.potions, vec![Potion::Fire]);
}

#[test]
fn fire_potion_deals_twenty_damage_and_is_consumed() {
    let mut run = RunState::combat_fixture();
    run.potions.push(Potion::Fire);
    let monster_id = run.combat.as_ref().expect("combat").monsters[0].id;
    let hp_before = run.combat.as_ref().expect("combat").monsters[0].hp;

    let run = apply_run_action(
        &run,
        RunAction::UsePotion {
            slot: 0,
            target: Some(monster_id),
        },
    )
    .expect("use fire potion");

    let combat = run.combat.expect("combat continues");
    assert_eq!(combat.monsters[0].hp, hp_before - FIRE_POTION_DAMAGE);
    assert!(run.potions.is_empty());
}

#[test]
fn toy_ornithopter_heals_when_attack_potion_opens_combat_card_reward() {
    let mut run = RunState::combat_fixture();
    run.relics.push(Relic::ToyOrnithopter);
    run.hp = 57;
    let combat = run.combat.as_mut().expect("combat");
    combat.player.hp = 57;
    run.potions.push(Potion::Attack);
    let hp_before = run.hp;

    let run = apply_run_action(
        &run,
        RunAction::UsePotion {
            slot: 0,
            target: None,
        },
    )
    .expect("use attack potion");

    let combat = run.combat.as_ref().expect("combat continues");
    assert_eq!(combat.player.hp, hp_before);
    assert_eq!(run.hp, hp_before);
    assert!(combat.potion_card_reward_choices().is_some());
    assert!(run.potions.is_empty());

    let run = apply_run_action(&run, RunAction::ChooseCombatCardReward { index: 0 })
        .expect("choose attack potion card");

    let combat = run.combat.as_ref().expect("combat continues");
    assert_eq!(combat.player.hp, hp_before + 5);
    assert_eq!(run.hp, hp_before + 5);
    assert!(combat.potion_card_reward_choices().is_none());
    assert!(run.potions.is_empty());
}

#[test]
fn lethal_fire_potion_enters_reward_phase() {
    let mut run = RunState::combat_fixture();
    run.potions.push(Potion::Fire);
    let combat = run.combat.as_mut().expect("combat");
    combat.monsters[0].hp = FIRE_POTION_DAMAGE;
    let monster_id = combat.monsters[0].id;

    let run = apply_run_action(
        &run,
        RunAction::UsePotion {
            slot: 0,
            target: Some(monster_id),
        },
    )
    .expect("use lethal fire potion");

    assert_eq!(run.phase, RunPhase::Reward);
    assert!(run.combat.is_none());
    assert!(run.reward.is_some());
    assert!(run.potions.is_empty());
}

#[test]
fn discard_potion_removes_it_from_belt() {
    let mut run = RunState::map_fixture();
    run.potions = vec![Potion::Fire];

    let run = apply_run_action(&run, RunAction::DiscardPotion { slot: 0 }).expect("discard potion");

    assert!(run.potions.is_empty());
}

#[test]
fn entering_event_fixture_exposes_canonical_golden_shrine() {
    let mut run = RunState::map_fixture();

    enter_fixed_event_screen_fixture(&mut run);

    assert_eq!(run.phase, RunPhase::Event);
    let event = run.event.as_ref().expect("event screen");
    assert_eq!(event.event, Event::GoldenShrine);
    assert_eq!(event.choices.len(), 3);
    assert_eq!(
        valid_event_actions(&run),
        vec![
            EventAction::Choose { choice_index: 0 },
            EventAction::Choose { choice_index: 1 },
            EventAction::Choose { choice_index: 2 },
        ]
    );
    assert!(valid_map_actions(&run).is_empty());
}

#[test]
fn locked_event_choices_are_not_legal_or_valid() {
    let mut run = RunState::seeded_ironclad(1, 0);
    run.phase = RunPhase::Event;
    run.event = Some(EventScreen {
        event: Event::WingStatue,
        choices: vec![
            EventChoice {
                label: "Pray".to_owned(),
            },
            EventChoice {
                label: "Locked".to_owned(),
            },
            EventChoice {
                label: "Leave".to_owned(),
            },
        ],
        stage: 0,
        event_data: 0,
    });

    assert_eq!(
        valid_event_actions(&run),
        vec![
            EventAction::Choose { choice_index: 0 },
            EventAction::Choose { choice_index: 2 },
        ]
    );
    assert_eq!(
        validate_event_action(&run, EventAction::Choose { choice_index: 1 }),
        Err(SimError::IllegalAction("event choice is locked"))
    );
    assert_eq!(
        apply_event_action(&run, EventAction::Choose { choice_index: 1 }),
        Err(SimError::IllegalAction("event choice is locked"))
    );
}

#[test]
fn golden_shrine_choice_grants_gold_and_returns_to_map() {
    let mut run = RunState::map_fixture();
    enter_fixed_event_screen_fixture(&mut run);
    let gold_before = run.gold;

    let after_pray =
        apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("choose shrine");

    assert_eq!(after_pray.phase, RunPhase::Event);
    assert_eq!(after_pray.gold, gold_before + GOLDEN_SHRINE_GOLD);
    assert_eq!(
        valid_event_actions(&after_pray),
        vec![EventAction::Choose { choice_index: 0 }]
    );
    assert!(valid_map_actions(&after_pray).is_empty());

    let run = apply_event_action(&after_pray, EventAction::Choose { choice_index: 0 })
        .expect("leave shrine");

    assert_eq!(run.phase, RunPhase::Idle);
    assert!(run.event.is_none());
    assert_eq!(run.gold, gold_before + GOLDEN_SHRINE_GOLD);
    assert_eq!(
        valid_map_actions(&run),
        vec![
            MapAction::ChooseNode {
                node_id: MapNodeId::new(1)
            },
            MapAction::ChooseNode {
                node_id: MapNodeId::new(2)
            },
        ]
    );
}

#[test]
fn event_actions_are_unavailable_outside_event_phase() {
    let run = RunState::map_fixture();

    assert!(valid_event_actions(&run).is_empty());
    let err = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
        .expect_err("not at event");

    assert_eq!(
        err,
        SimError::IllegalAction("event actions require event phase")
    );
}

#[test]
fn event_rng_selects_fixed_event_deterministically() {
    let mut first = RunState::map_fixture();
    let mut second = RunState::map_fixture();
    first.event_rng_seed = 19;
    second.event_rng_seed = 19;

    enter_event_screen(&mut first).expect("first event entry succeeds");
    enter_event_screen(&mut second).expect("second event entry succeeds");

    assert_eq!(first.phase, RunPhase::Event);
    assert_eq!(first.event, second.event);
    assert_eq!(first.event_rng_counter, second.event_rng_counter);
    assert!(!first.act1_event_list.is_empty() || !first.act1_shrine_list.is_empty());
}

#[test]
fn buy_shop_relic_adds_vajra_and_applies_on_next_combat() {
    let mut run = reach_shop_via_left_branch();
    run.gold = SHOP_VAJRA_PRICE;

    let run = leave_shop_merchant_and_room(
        apply_run_action(&run, RunAction::BuyShopRelic { slot: 0 }).expect("buy vajra"),
    );

    assert_eq!(run.phase, RunPhase::Idle);
    assert_eq!(run.relics, vec![Relic::Vajra]);
    let combat = run
        .init_combat(sts_core::adapter_internals::CombatState::initial_fixture())
        .expect("combat initializes");
    assert_eq!(combat.player.powers.strength, VAJRA_STRENGTH);
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
    run.relics.push(Relic::PeacePipe);
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
    let after = proceed_rest(&after);

    assert_eq!(after.deck.len(), deck_len_before - 1);
    assert!(!after.deck.iter().any(|card| card.id == strike_id));
    assert_eq!(after.count_content_in_deck(STRIKE_R_ID), 4);
    assert_eq!(after.phase, RunPhase::Idle);
}

#[test]
fn peace_pipe_opens_a_rest_remove_grid() {
    let mut run = RunState::map_fixture();
    run.relics.push(Relic::PeacePipe);
    run = apply_map_action_on_run(
        &run,
        MapAction::ChooseNode {
            node_id: MapNodeId::new(2),
        },
    )
    .expect("enter rest");

    let opened = apply_rest_action(&run, RestAction::OpenRemove).expect("open toke");
    let grid = opened.card_grid.as_ref().expect("rest remove grid");
    assert_eq!(
        grid.purpose,
        sts_core::adapter_internals::GridPurpose::RestRemove
    );
    assert_eq!(grid.cards.len(), run.deck.len());
    assert_eq!(opened.phase, RunPhase::Rest);
}

#[test]
fn removing_parasite_at_rest_loses_three_max_hp() {
    let mut run = RunState::map_fixture();
    run.relics.push(Relic::PeacePipe);
    run.max_hp = 50;
    run.hp = 50;
    let parasite_id = CardId::new(
        run.next_card_instance_id()
            .expect("fixture has card ID allocation headroom"),
    );
    run.deck.push(CardInstance::new(parasite_id, PARASITE_ID));
    run = apply_map_action_on_run(
        &run,
        MapAction::ChooseNode {
            node_id: MapNodeId::new(2),
        },
    )
    .expect("enter rest");

    let after = apply_rest_action(
        &run,
        RestAction::RemoveCard {
            card_id: parasite_id,
        },
    )
    .expect("remove Parasite applies");

    assert_eq!(after.max_hp, 47);
    assert_eq!(after.hp, 47);
    assert!(!after.deck.iter().any(|card| card.content_id == PARASITE_ID));
}
