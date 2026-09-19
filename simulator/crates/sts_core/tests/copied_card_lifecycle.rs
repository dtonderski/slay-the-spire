use sts_core::adapter_internals::{
    apply_combat_action,
    combat::transition::{choose_hand_select, confirm_hand_select},
    content::{
        cards,
        monsters::{monster_state, FIXED_SIMPLE_MONSTER},
    },
    CardId, CardInstance, CombatAction, CombatState, MonsterId,
};

fn cultist_combat() -> CombatState {
    let mut state = CombatState::initial_fixture();
    state.monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    state
}

#[test]
fn duplicated_warcry_put_back_resolves_while_original_is_in_limbo() {
    let mut state = cultist_combat();
    state.player.energy = 3;
    state.duplication_potion_stacks = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::WARCRY_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(4), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(5), cards::STRIKE_R_ID),
    ];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(6), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(7), cards::BASH_ID),
        CardInstance::new(CardId::new(8), cards::STRIKE_R_ID),
    ];

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("duplicated Warcry opens the original put-back");

    assert!(next
        .piles
        .limbo
        .iter()
        .any(|card| card.id == CardId::new(1)));
    assert!(
        next.hand_select().is_some_and(|select| !select.copy_owned),
        "original Warcry select is not copy-owned"
    );

    choose_hand_select(&mut next, 0).expect("select a card for original Warcry");
    confirm_hand_select(&mut next).expect("original Warcry confirm runs copied work");

    assert!(
        next.piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(1)),
        "original Warcry settles exactly once after its select"
    );
    if next.hand_select().is_some() {
        assert!(
            next.hand_select().is_some_and(|select| select.copy_owned),
            "copy-owned Warcry may open a second put-back"
        );
        choose_hand_select(&mut next, 0).expect("select a card for copied Warcry");
        confirm_hand_select(&mut next)
            .expect("copied Warcry confirm does not require the original");
    }
    assert!(next.hand_select().is_none());
    assert_eq!(
        next.piles
            .exhaust_pile
            .iter()
            .filter(|card| card.content_id == cards::WARCRY_ID)
            .count(),
        1,
        "the copy is purge-on-use and must not exhaust a second Warcry"
    );
}

#[test]
fn duplicated_warcry_plus_put_back_resolves_while_original_is_in_limbo() {
    let mut state = cultist_combat();
    state.player.energy = 3;
    state.duplication_potion_stacks = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::WARCRY_PLUS_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(4), cards::STRIKE_R_ID),
    ];
    state.piles.draw_pile = vec![
        CardInstance::new(CardId::new(5), cards::DEFEND_R_ID),
        CardInstance::new(CardId::new(6), cards::BASH_ID),
        CardInstance::new(CardId::new(7), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(8), cards::STRIKE_R_ID),
    ];

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("duplicated Warcry+ opens the original put-back");
    choose_hand_select(&mut next, 0).expect("select a card for original Warcry+");
    confirm_hand_select(&mut next).expect("original Warcry+ confirm runs copied work");
    if next.hand_select().is_some() {
        choose_hand_select(&mut next, 0).expect("select a card for copied Warcry+");
        confirm_hand_select(&mut next).expect("copied Warcry+ confirm");
    }
    assert!(next.hand_select().is_none());
    assert!(next
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.id == CardId::new(1)));
}

#[test]
fn duplicated_offering_draws_after_original_exhausts() {
    let mut state = cultist_combat();
    state.player.energy = 3;
    state.player.hp = 80;
    state.duplication_potion_stacks = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::OFFERING_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(4), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(5), cards::STRIKE_R_ID),
    ];
    state.piles.draw_pile = (6..=20)
        .map(|id| CardInstance::new(CardId::new(id), cards::STRIKE_R_ID))
        .collect();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("duplicated Offering resolves both draws");

    assert_eq!(next.player.hp, 68);
    assert_eq!(next.player.energy, 7);
    assert_eq!(next.piles.hand.len(), 10);
    assert!(next
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.id == CardId::new(1)));
    assert_eq!(next.duplication_potion_stacks, 0);
}

#[test]
fn duplicated_master_of_strategy_draws_after_original_exhausts() {
    let mut state = cultist_combat();
    state.player.energy = 3;
    state.duplication_potion_stacks = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::MASTER_OF_STRATEGY_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(4), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(5), cards::STRIKE_R_ID),
    ];
    state.piles.draw_pile = (6..=20)
        .map(|id| CardInstance::new(CardId::new(id), cards::STRIKE_R_ID))
        .collect();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("duplicated Master of Strategy resolves both draws");

    assert_eq!(next.piles.hand.len(), 10);
    assert!(next
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.id == CardId::new(1)));
}

#[test]
fn duplicated_transmutation_generates_without_original_in_hand() {
    let mut state = cultist_combat();
    state.player.energy = 2;
    state.duplication_potion_stacks = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::TRANSMUTATION_ID),
        CardInstance::new(CardId::new(2), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(3), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(4), cards::STRIKE_R_ID),
        CardInstance::new(CardId::new(5), cards::STRIKE_R_ID),
    ];
    state.piles.draw_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("duplicated Transmutation generates for original and copy");

    assert_eq!(next.player.energy, 0);
    assert!(next
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.id == CardId::new(1)));
    let generated = next
        .piles
        .hand
        .iter()
        .chain(next.piles.discard_pile.iter())
        .filter(|card| {
            card.id != CardId::new(1)
                && card.content_id != cards::STRIKE_R_ID
                && card.temp_cost == Some(0)
        })
        .count();
    assert_eq!(
        generated, 4,
        "X=2 generates twice without relocating source"
    );
}

#[test]
fn duplicated_power_through_generates_wounds_without_original_in_hand() {
    let mut state = cultist_combat();
    state.player.energy = 1;
    state.duplication_potion_stacks = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), cards::POWER_THROUGH_ID)];
    state.piles.draw_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("duplicated Power Through generates both Wound pairs");

    let wounds = next
        .piles
        .hand
        .iter()
        .chain(next.piles.discard_pile.iter())
        .filter(|card| card.content_id == cards::WOUND_ID)
        .count();
    assert_eq!(wounds, 4);
    assert!(next
        .piles
        .discard_pile
        .iter()
        .any(|card| card.id == CardId::new(1)));
}

#[test]
fn duplicated_armaments_does_not_fail_after_upgrading_its_only_target() {
    let mut state = cultist_combat();
    state.player.energy = 3;
    state.duplication_potion_stacks = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(15), cards::ARMAMENTS_ID),
        CardInstance::new(CardId::new(11), cards::CURSE_OF_THE_BELL_ID),
        CardInstance::new(CardId::new(5), cards::THUNDERCLAP_ID),
    ];
    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(15),
            target: None,
        },
    )
    .expect("duplicated Armaments resolves after the original upgrade");
    assert_eq!(next.duplication_potion_stacks, 0);
    assert!(next
        .piles
        .discard_pile
        .iter()
        .any(|card| card.id == CardId::new(15)));
    assert_eq!(
        next.piles
            .hand
            .iter()
            .filter(|card| card.content_id == cards::THUNDERCLAP_PLUS_ID)
            .count(),
        1
    );
}

#[test]
fn duplicated_forethought_auto_place_does_not_replay_the_same_card() {
    let mut state = cultist_combat();
    state.player.energy = 3;
    state.duplication_potion_stacks = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::FORETHOUGHT_ID),
        CardInstance::new(CardId::new(2), cards::CURSE_OF_THE_BELL_ID),
    ];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(3), cards::STRIKE_R_ID)];
    state.piles.discard_pile.clear();

    let next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("duplicated Forethought auto-place must not fail on the copy");

    assert!(next.hand_select().is_none());
    assert_eq!(next.duplication_potion_stacks, 0);
    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.draw_pile.len(), 2);
    assert_eq!(
        next.piles.draw_pile[0].content_id,
        cards::CURSE_OF_THE_BELL_ID
    );
    assert!(next.piles.draw_pile[0].free_to_play_once);
    assert_eq!(next.piles.draw_pile[1].content_id, cards::STRIKE_R_ID);
    assert_eq!(
        next.piles
            .discard_pile
            .iter()
            .filter(|card| card.content_id == cards::FORETHOUGHT_ID)
            .count(),
        1,
        "the copy is purge-on-use and must not discard a second Forethought"
    );
}

#[test]
fn duplicated_forethought_leftover_singleton_is_auto_placed() {
    let mut state = cultist_combat();
    state.player.energy = 3;
    state.duplication_potion_stacks = 1;
    state.piles.hand = vec![
        CardInstance::new(CardId::new(1), cards::FORETHOUGHT_ID),
        CardInstance::new(CardId::new(2), cards::BASH_ID),
        CardInstance::new(CardId::new(3), cards::DEFEND_R_ID),
    ];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(4), cards::STRIKE_R_ID)];
    state.piles.discard_pile.clear();

    let mut next = apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("duplicated Forethought with two other cards opens the original select");
    assert!(
        next.hand_select().is_some_and(|select| !select.copy_owned),
        "original Forethought select is not copy-owned"
    );

    choose_hand_select(&mut next, 0).expect("select Bash for original Forethought");
    confirm_hand_select(&mut next)
        .expect("copy auto-places the leftover singleton without a second select");

    assert!(next.hand_select().is_none());
    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.draw_pile.len(), 3);
    assert_eq!(next.piles.draw_pile[0].content_id, cards::DEFEND_R_ID);
    assert!(next.piles.draw_pile[0].free_to_play_once);
    assert_eq!(next.piles.draw_pile[1].content_id, cards::BASH_ID);
    assert!(next.piles.draw_pile[1].free_to_play_once);
    assert_eq!(next.piles.draw_pile[2].content_id, cards::STRIKE_R_ID);
    assert_eq!(
        next.piles
            .discard_pile
            .iter()
            .filter(|card| card.content_id == cards::FORETHOUGHT_ID)
            .count(),
        1
    );
}
