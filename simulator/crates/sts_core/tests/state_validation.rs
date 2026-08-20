use sts_core::run::event::{neow_screen_for_stage, Event, MatchAndKeepCard, MatchAndKeepState};
use sts_core::run::setup_treasure_room;
use sts_core::{
    apply_combat_action,
    card::CardInstance,
    combat::{
        CombatDecisionState, CombatPhase, DrawSelectPurpose, DrawSelectState, HandSelectState,
    },
    content::cards,
    content::shop_pool::shop_card_content_id,
    enter_reward_screen, legal_event_actions, legal_rest_actions, legal_run_decision_actions,
    legal_shop_actions, open_shop_merchant, CardGridScreen, CardId, CardRewardFlow, CombatAction,
    CombatState, ContentId, GridPurpose, MapNodeId, MonsterIntent, Relic,
    RewardContinuation, RewardScreen, RoomKind, RunPhase, RunState, SimError,
};

fn empty_reward_screen(continuation: RewardContinuation) -> RewardScreen {
    RewardScreen {
        continuation,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer: None,
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: CardRewardFlow::None,
    }
}

#[test]
fn explicit_combat_and_run_fixtures_are_valid() {
    CombatState::initial_fixture()
        .validate()
        .expect("combat fixture has explicit valid state");
    RunState::combat_fixture()
        .validate()
        .expect("combat run fixture is valid");
    RunState::map_fixture()
        .validate()
        .expect("map run fixture is valid");
}

#[test]
fn malformed_map_identity_fails_run_and_legal_action_validation() {
    let mut missing_current = RunState::map_fixture();
    missing_current.map.as_mut().expect("map").current_node = MapNodeId::new(u64::MAX);
    assert_eq!(
        missing_current.validate(),
        Err(SimError::UnknownMapNode(MapNodeId::new(u64::MAX)))
    );
    assert_eq!(
        legal_run_decision_actions(&missing_current),
        Err(SimError::UnknownMapNode(MapNodeId::new(u64::MAX)))
    );

    let mut duplicate = RunState::map_fixture();
    let map = duplicate.map.as_mut().expect("map");
    map.map.nodes[1].id = map.map.nodes[0].id;
    assert_eq!(
        duplicate.validate(),
        Err(SimError::InvalidState("map has duplicate node IDs"))
    );

    let mut dangling_child = RunState::map_fixture();
    dangling_child.map.as_mut().expect("map").map.nodes[0]
        .children
        .push(MapNodeId::new(u64::MAX));
    assert_eq!(
        dangling_child.validate(),
        Err(SimError::UnknownMapNode(MapNodeId::new(u64::MAX)))
    );

    let mut mismatched_act = RunState::map_fixture();
    mismatched_act.map.as_mut().expect("map").act = 2;
    assert_eq!(
        mismatched_act.validate(),
        Err(SimError::InvalidState(
            "map state act does not match current node"
        ))
    );
}

#[test]
fn combat_rng_keeps_flattened_snapshot_fields() {
    let json = serde_json::to_value(CombatState::initial_fixture()).expect("combat serializes");
    let object = json.as_object().expect("combat object");

    assert!(object.contains_key("shuffle_rng"));
    assert!(object.contains_key("monster_rng"));
    assert!(object.contains_key("monster_hp_rng"));
    assert!(object.contains_key("card_random_rng"));
    assert!(!object.contains_key("rng"));
}

#[test]
fn missing_or_null_combat_rng_fails_deserialization() {
    let state = CombatState::initial_fixture();
    let mut json = serde_json::to_value(&state).expect("combat serializes");
    json.as_object_mut()
        .expect("combat object")
        .remove("card_random_rng");

    assert!(serde_json::from_value::<CombatState>(json).is_err());

    let mut json = serde_json::to_value(&state).expect("combat serializes");
    json["card_random_rng"] = serde_json::Value::Null;

    assert!(serde_json::from_value::<CombatState>(json).is_err());
}

#[test]
fn duplicate_card_ids_fail_validation() {
    let mut state = CombatState::initial_fixture();
    state.piles.draw_pile[0].id = state.piles.hand[0].id;

    assert_eq!(
        state.validate(),
        Err(SimError::InvalidState(
            "card instance appears in more than one pile"
        ))
    );
}

#[test]
fn duplicate_owned_relics_fail_validation_except_stackable_circlets() {
    let mut run = RunState::map_fixture();
    run.relics = vec![Relic::BurningBlood, Relic::BurningBlood];
    assert_eq!(
        run.validate(),
        Err(SimError::InvalidState("duplicate owned relic"))
    );

    run.relics = vec![Relic::Circlet, Relic::Circlet];
    run.validate().expect("Circlet may be awarded repeatedly");
}

#[test]
fn unknown_content_fails_validation() {
    let mut state = CombatState::initial_fixture();
    state.piles.hand[0].content_id = ContentId::new(u64::MAX);

    assert_eq!(
        state.validate(),
        Err(SimError::UnknownContent(ContentId::new(u64::MAX)))
    );
}

#[test]
fn missing_variable_monster_damage_fails_before_action_execution() {
    let mut state = CombatState::red_louse_fixture();
    state.monsters[0].rolled_attack_damage = None;
    let original = state.clone();

    assert_eq!(
        state.validate(),
        Err(SimError::InvalidState(
            "monster requires rolled attack damage"
        ))
    );
    assert_eq!(
        apply_combat_action(&state, CombatAction::EndTurn),
        Err(SimError::InvalidState(
            "monster requires rolled attack damage"
        ))
    );
    assert_eq!(state, original);
}

#[test]
fn unresolved_initial_ai_roll_fails_before_action_execution() {
    let mut state = CombatState::initial_fixture();
    state.monsters[0].intent = MonsterIntent::PendingAiRoll;
    let original = state.clone();

    assert_eq!(
        state.validate(),
        Err(SimError::InvalidState(
            "combat monster intent is pending AI roll"
        ))
    );
    assert_eq!(
        apply_combat_action(&state, CombatAction::EndTurn),
        Err(SimError::InvalidState(
            "combat monster intent is pending AI roll"
        ))
    );
    assert_eq!(state, original);
}

#[test]
fn invalid_player_bounds_fail_validation() {
    let mut state = CombatState::initial_fixture();
    state.player.hp = state.player.max_hp + 1;

    assert_eq!(
        state.validate(),
        Err(SimError::InvalidState("combat player HP is out of bounds"))
    );
}

#[test]
fn inconsistent_combust_authorities_fail_validation() {
    for (stacks, damage) in [(1, 0), (1, 6), (1, 9), (0, 5), (-1, 0)] {
        let mut state = CombatState::initial_fixture();
        state.player.powers.combust = stacks;
        state.player.powers.combust_damage = damage;

        assert_eq!(
            state.validate(),
            Err(SimError::InvalidState(
                "Combust power state is inconsistent"
            )),
            "stacks={stacks} damage={damage}"
        );
    }

    for (stacks, damage) in [(0, 0), (1, 5), (1, 7), (2, 10), (2, 12), (2, 14)] {
        let mut state = CombatState::initial_fixture();
        state.player.powers.combust = stacks;
        state.player.powers.combust_damage = damage;

        state
            .validate()
            .unwrap_or_else(|error| panic!("stacks={stacks} damage={damage}: {error}"));
    }
}

#[test]
fn nonpositive_monster_max_hp_fails_before_action_execution() {
    let mut state = CombatState::initial_fixture();
    state.monsters[0].hp = 0;
    state.monsters[0].max_hp = 0;
    let original = state.clone();

    assert_eq!(
        state.validate(),
        Err(SimError::InvalidState(
            "combat monster HP, block, or stolen gold is out of bounds"
        ))
    );
    assert_eq!(
        apply_combat_action(&state, CombatAction::EndTurn),
        Err(SimError::InvalidState(
            "combat monster HP, block, or stolen gold is out of bounds"
        ))
    );
    assert_eq!(state, original);
}

#[test]
fn combat_decisions_have_one_active_value_and_an_ordered_queue() {
    let mut state = CombatState::initial_fixture();
    let source_card_id = state.piles.hand[0].id;
    state.decision = Some(CombatDecisionState::HandSelect {
        state: HandSelectState {
            purpose: Default::default(),
            source_card_id,
            selected_hand_index: None,
            selected_hand_indices: Vec::new(),
            dual_wield_restore_on_confirm: Vec::new(),
            dual_wield_force_exhaust: false,
        },
        pending_actions: Default::default(),
    });
    state
        .queued_decisions
        .push_back(CombatDecisionState::DrawSelect {
            state: DrawSelectState {
                purpose: DrawSelectPurpose::SecretTechniqueSkillToHand,
                source_card_id,
                selectable_card_ids: Vec::new(),
                selected_draw_index: None,
                pending_actions: Default::default(),
            },
        });

    state
        .validate()
        .expect("one active decision and its queue validate");
    assert!(state.hand_select().is_some());
    assert_eq!(state.queued_decisions.len(), 1);
}

#[test]
fn queued_combat_decision_without_active_predecessor_fails_validation() {
    let mut state = CombatState::initial_fixture();
    let source_card_id = state.piles.hand[0].id;
    state
        .queued_decisions
        .push_back(CombatDecisionState::DrawSelect {
            state: DrawSelectState {
                purpose: DrawSelectPurpose::SecretTechniqueSkillToHand,
                source_card_id,
                selectable_card_ids: Vec::new(),
                selected_draw_index: None,
                pending_actions: Default::default(),
            },
        });

    assert_eq!(
        state.validate(),
        Err(SimError::InvalidState(
            "queued combat decision has no active predecessor"
        ))
    );
}

#[test]
fn run_phase_ownership_is_validated() {
    let mut run = RunState::combat_fixture();
    run.phase = RunPhase::Idle;

    assert_eq!(
        run.validate(),
        Err(SimError::InvalidState(
            "combat state exists outside combat phase"
        ))
    );
}

#[test]
fn orphaned_run_screens_and_room_state_fail_validation() {
    let mut reward = RunState::map_fixture();
    reward.reward = Some(empty_reward_screen(RewardContinuation::None));
    assert_eq!(
        reward.validate(),
        Err(SimError::InvalidState(
            "reward screen exists outside reward phase"
        ))
    );

    let mut event = RunState::map_fixture();
    event.event = Some(sts_core::event_screen(Event::GoldenShrine));
    assert_eq!(
        event.validate(),
        Err(SimError::InvalidState(
            "event screen exists outside event, reward, or terminal complete phase"
        ))
    );

    let mut shop = RunState::map_fixture();
    sts_core::enter_shop_room(&mut shop).expect("shop fixture generation succeeds");
    shop.phase = RunPhase::Idle;
    assert_eq!(
        shop.validate(),
        Err(SimError::InvalidState(
            "shop screen exists outside shop or reward phase"
        ))
    );

    let mut treasure = RunState::map_fixture();
    setup_treasure_room(&mut treasure);
    assert_eq!(
        treasure.validate(),
        Err(SimError::InvalidState(
            "treasure room exists outside treasure or reward phase"
        ))
    );

    let mut rest = RunState::map_fixture();
    rest.current_room_override = Some(RoomKind::Rest);
    rest.rest_room_complete = true;
    assert_eq!(
        rest.validate(),
        Err(SimError::InvalidState(
            "completed rest room exists outside rest or reward phase"
        ))
    );

    let mut boss = RunState::map_fixture();
    boss.current_room_override = Some(RoomKind::Boss);
    boss.boss_chest_opened = true;
    assert_eq!(
        boss.validate(),
        Err(SimError::InvalidState(
            "boss chest state exists outside reward or treasure phase"
        ))
    );
}

#[test]
fn typed_reward_continuations_retain_only_their_authoritative_owner() {
    let mut event = RunState::seeded_ironclad(1, 0);
    event.phase = RunPhase::Reward;
    event.event = Some(neow_screen_for_stage(&event, 2));
    event.reward = Some(empty_reward_screen(RewardContinuation::Neow));
    event
        .validate()
        .expect("Neow reward retains its event owner");

    let mut shop = RunState::map_fixture();
    sts_core::enter_shop_room(&mut shop).expect("shop fixture generation succeeds");
    open_shop_merchant(&mut shop).expect("merchant opens");
    shop.phase = RunPhase::Reward;
    shop.reward = Some(empty_reward_screen(RewardContinuation::Shop));
    shop.validate()
        .expect("shop reward retains its merchant owner");

    let mut treasure = RunState::map_fixture();
    treasure.current_room_override = Some(RoomKind::Treasure);
    setup_treasure_room(&mut treasure);
    treasure.phase = RunPhase::Reward;
    treasure.reward = Some(empty_reward_screen(RewardContinuation::Map));
    treasure
        .validate()
        .expect("chest reward retains its treasure-room owner");
}

#[test]
fn card_grid_purpose_requires_its_phase_owner() {
    let mut run = RunState::map_fixture();
    run.card_grid = Some(CardGridScreen {
        cards: run.deck.clone(),
        purpose: GridPurpose::RestSmith,
        selected: None,
        selected_indices: Vec::new(),
    });

    assert_eq!(
        run.validate(),
        Err(SimError::InvalidState(
            "card grid purpose has no authoritative phase owner"
        ))
    );
}

#[test]
fn overlay_state_cannot_replace_the_required_phase_owner() {
    let mut reward = RunState::map_fixture();
    reward.phase = RunPhase::Reward;
    reward.card_grid = Some(CardGridScreen {
        cards: reward.deck.clone(),
        purpose: GridPurpose::EmptyCage { remaining: 2 },
        selected: None,
        selected_indices: Vec::new(),
    });
    assert_eq!(
        reward.validate(),
        Err(SimError::InvalidState("reward phase has no reward screen"))
    );

    let mut event = RunState::map_fixture();
    event.phase = RunPhase::Event;
    event.card_grid = Some(CardGridScreen {
        cards: event.deck.clone(),
        purpose: GridPurpose::EventRemove,
        selected: None,
        selected_indices: Vec::new(),
    });
    assert_eq!(
        event.validate(),
        Err(SimError::InvalidState("event phase has no event screen"))
    );
    assert_eq!(
        legal_event_actions(&event),
        Err(SimError::InvalidState("event phase has no event screen"))
    );
}

#[test]
fn screen_backed_phases_require_their_authoritative_state() {
    let mut shop = RunState::map_fixture();
    shop.phase = RunPhase::Shop;
    assert_eq!(
        shop.validate(),
        Err(SimError::InvalidState("shop phase has no shop screen"))
    );
    assert_eq!(
        legal_shop_actions(&shop),
        Err(SimError::InvalidState("shop phase has no shop screen"))
    );

    let mut treasure = RunState::map_fixture();
    treasure.phase = RunPhase::Treasure;
    assert_eq!(
        treasure.validate(),
        Err(SimError::InvalidState(
            "treasure phase has no treasure room"
        ))
    );
}

#[test]
fn rest_phase_requires_a_rest_room() {
    let mut run = RunState::map_fixture();
    run.phase = RunPhase::Rest;

    assert_eq!(
        run.validate(),
        Err(SimError::InvalidState("rest phase is not in a rest room"))
    );
    assert_eq!(
        legal_rest_actions(&run),
        Err(SimError::InvalidState("rest phase is not in a rest room"))
    );
}

#[test]
fn match_and_keep_authoritative_state_is_validated() {
    let mut run = RunState::seeded_ironclad(1, 0);
    run.phase = RunPhase::Event;
    run.event = Some(sts_core::event_screen(Event::MatchAndKeep));
    run.match_and_keep = Some(MatchAndKeepState {
        cards: vec![MatchAndKeepCard {
            content_id: cards::STRIKE_R_ID,
            revealed: false,
            matched: false,
        }],
        attempts_remaining: 5,
        first_flipped_index: Some(1),
        second_flipped_index: None,
        matched_cards: Vec::new(),
        game_done: false,
    });

    assert_eq!(
        run.validate(),
        Err(SimError::InvalidState(
            "Match and Keep flipped card index is out of bounds"
        ))
    );

    let state = run.match_and_keep.as_mut().expect("state exists");
    state.first_flipped_index = None;
    state.cards[0].content_id = ContentId::new(999_999);
    assert_eq!(
        run.validate(),
        Err(SimError::UnknownContent(ContentId::new(999_999)))
    );
}

#[test]
fn duplicate_run_deck_ids_fail_validation() {
    let mut run = RunState::map_fixture();
    run.deck[1].id = run.deck[0].id;

    assert_eq!(
        run.validate(),
        Err(SimError::InvalidState(
            "duplicate run deck card instance ID"
        ))
    );
}

#[test]
fn known_pending_card_without_event_authority_is_invalid() {
    let mut run = RunState::map_fixture();
    run.pending_obtain_cards.push(cards::ANGER_ID);

    assert_eq!(
        run.validate(),
        Err(SimError::InvalidState(
            "pending obtain cards do not match event authority"
        ))
    );
}

#[test]
fn known_prismatic_reward_identity_is_valid_in_reward_and_deck() {
    let mut run = RunState::map_fixture();
    let mut combat = CombatState::initial_fixture();
    combat.phase = CombatPhase::Won;
    for monster in &mut combat.monsters {
        monster.hp = 0;
        monster.alive = false;
    }
    run.phase = RunPhase::Combat;
    run.combat = Some(combat);
    enter_reward_screen(&mut run).expect("reward entry succeeds");
    let content_id = shop_card_content_id("REACH_HEAVEN");
    let reward_id = CardId::new(
        run.next_card_instance_id()
            .expect("fixture has card ID allocation headroom"),
    );
    run.reward
        .as_mut()
        .expect("reward screen")
        .choices
        .push(CardInstance::new(reward_id, content_id));

    run.validate()
        .expect("known unmodeled card may remain a visible reward choice");

    let deck_id = CardId::new(
        run.next_card_instance_id()
            .expect("fixture has card ID allocation headroom"),
    );
    run.deck.push(CardInstance::new(deck_id, content_id));
    run.validate()
        .expect("known Prismatic reward identity may enter the master deck");

    let unknown_content = ContentId::new(u64::MAX - 1);
    let unknown_id = CardId::new(
        run.next_card_instance_id()
            .expect("fixture has card ID allocation headroom"),
    );
    run.deck
        .push(CardInstance::new(unknown_id, unknown_content));
    assert_eq!(
        run.validate(),
        Err(SimError::UnknownContent(unknown_content))
    );
}
