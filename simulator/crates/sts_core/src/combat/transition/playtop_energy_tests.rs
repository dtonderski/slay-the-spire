//! PC PlayTopCardAction -> NewQueueCardAction captures CardQueueItem.energyOnUse
//! before older addToBot actions drain. FIDL01405/585 is the natural witness.
use super::*;
use crate::content::cards::{
    DEFEND_R_ID, HAVOC_ID, HAVOC_PLUS_ID, MALAISE_ANY_COLOR_ID, SKEWER_ANY_COLOR_ID,
    TRANSMUTATION_ID, WHIRLWIND_ID, WHIRLWIND_PLUS_ID,
};

fn setup(content: ContentId) -> CombatState {
    let mut s = CombatState::cultist_fixture();
    s.monsters[0].hp = 500;
    s.monsters[0].max_hp = 500;
    s.player.energy = 3;
    s.player.authority.relics = vec![Relic::Sundial];
    s.sundial_shuffles = 2;
    s.piles.hand = vec![CardInstance::new(CardId::new(99), HAVOC_ID)];
    s.piles.draw_pile.clear();
    s.piles.discard_pile = vec![CardInstance::new(CardId::new(41), content)];
    s.piles.exhaust_pile.clear();
    s.validate().unwrap();
    s
}

#[test]
fn havoc_sundial_whirlwind_retains_selected_energy_and_free_autoplay() {
    for havoc in [HAVOC_ID, HAVOC_PLUS_ID] {
        for whirlwind in [WHIRLWIND_ID, WHIRLWIND_PLUS_ID] {
            let mut s = setup(whirlwind);
            s.piles.hand[0].content_id = havoc;
            let x = if havoc == HAVOC_ID { 2 } else { 3 };
            let damage = if whirlwind == WHIRLWIND_ID { 5 } else { 8 };
            let before = serde_json::to_value(&s).unwrap();
            let n = apply_combat_action(
                &s,
                CombatAction::PlayCard {
                    card_id: CardId::new(99),
                    target: None,
                },
            )
            .unwrap();
            assert_eq!(n.monsters[0].hp, 500 - x * damage);
            assert_eq!(n.player.energy, x + 2);
            assert_eq!(n.sundial_shuffles, 3);
            assert!(n.piles.exhaust_pile.iter().any(|c| c.id == CardId::new(41)));
            assert_eq!(n.rng.shuffle_rng.counter(), s.rng.shuffle_rng.counter() + 1);
            let mut expected = s.rng.card_random_rng.clone();
            expected.random_int(0); // Havoc.use always reserves its target.
            assert_eq!(n.rng.card_random_rng, expected);
            assert_eq!(serde_json::to_value(&s).unwrap(), before);
            n.validate().unwrap();
        }
    }
}

#[test]
fn chemical_x_and_necronomicon_use_reserved_x_not_later_sundial_energy() {
    for x in 0..=2 {
        for chemical in [false, true] {
            let mut s = setup(WHIRLWIND_ID);
            s.piles.hand[0].content_id = HAVOC_PLUS_ID;
            s.player.energy = x;
            s.player.authority.relics.push(Relic::Necronomicon);
            if chemical {
                s.player.authority.relics.push(Relic::ChemicalX);
            }
            let n = apply_combat_action(
                &s,
                CombatAction::PlayCard {
                    card_id: CardId::new(99),
                    target: None,
                },
            )
            .unwrap();
            let hits = (x + if chemical { 2 } else { 0 }) * if x >= 2 { 2 } else { 1 };
            assert_eq!(n.monsters[0].hp, 500 - 5 * hits);
            assert_eq!(n.player.energy, x + 2);
            assert_eq!(n.relic_counters.necronomicon_used_this_turn, x >= 2);
        }
    }
}

#[test]
fn typed_x_queues_keep_reserved_energy_across_later_energy_changes() {
    for content in [
        WHIRLWIND_ID,
        SKEWER_ANY_COLOR_ID,
        MALAISE_ANY_COLOR_ID,
        TRANSMUTATION_ID,
    ] {
        let mut s = setup(content);
        let card = s.piles.discard_pile.pop().unwrap();
        s.piles.limbo.push(card);
        s.player.authority.relics.clear();
        let target = (get_card_definition(content).unwrap().target == TargetRequirement::Enemy)
            .then_some(s.monsters[0].id);
        // The ordinary queue changes the live energy 3 -> 5 before service;
        // the card item still owns X=2. No temporary mutation of player energy.
        let q = VecDeque::from([
            InternalAction::GainEnergy { amount: 2 },
            InternalAction::ResolveTopDrawCard {
                card_id: card.id,
                target,
                exhaust_played_card: true,
                energy_on_use: Some(2),
            },
        ]);
        let n = process_internal_queue(&s, q).unwrap().state;
        assert_eq!(n.player.energy, 5);
        match content {
            WHIRLWIND_ID => assert_eq!(n.monsters[0].hp, 490),
            SKEWER_ANY_COLOR_ID => assert_eq!(n.monsters[0].hp, 486),
            MALAISE_ANY_COLOR_ID => {
                assert_eq!(n.monsters[0].powers.strength, -2);
                assert_eq!(n.monsters[0].powers.weak, 2);
            }
            TRANSMUTATION_ID => assert_eq!(n.piles.hand.len(), 3), // original Havoc + two generated cards
            _ => unreachable!(),
        }
    }
    let mut s = setup(WHIRLWIND_ID);
    let card = s.piles.discard_pile.pop().unwrap();
    s.piles.limbo.push(card);
    let n = process_internal_queue(
        &s,
        VecDeque::from([
            InternalAction::SpendEnergy { amount: 2 },
            InternalAction::ResolveTopDrawCard {
                card_id: card.id,
                target: None,
                exhaust_played_card: true,
                energy_on_use: Some(3),
            },
        ]),
    )
    .unwrap()
    .state;
    assert_eq!(n.monsters[0].hp, 485);
    assert_eq!(n.player.energy, 1);
}

#[test]
fn shuffle_callbacks_wait_for_parent_source_then_run_before_card_queue() {
    let mut s = setup(WHIRLWIND_ID);
    s.player.authority.relics.push(Relic::TheAbacus);
    s.player.powers.feel_no_pain = 3;
    s.player.no_block_turns = 1;
    s.player.powers.dexterity = 7;
    s.player.powers.frail = 2;
    let n = process_internal_queue_with_event_recording(
        &s,
        VecDeque::from([
            InternalAction::PlayTopDrawCard {
                target: None,
                exhaust_played_card: true,
                random_living_target: false,
            },
            InternalAction::MoveCard {
                card_id: CardId::new(99),
                from: CardPile::Hand,
                to: CardPile::ExhaustPile,
            },
        ]),
        true,
    )
    .unwrap();
    let position = |f: fn(&InternalAction) -> bool| n.event_log.iter().position(f).unwrap();
    let source = position(
        |a| matches!(a, InternalAction::MoveCard { card_id, .. } if *card_id == CardId::new(99)),
    );
    let block = position(|a| matches!(a, InternalAction::GainBlockFromShuffle { .. }));
    let gain = position(|a| matches!(a, InternalAction::GainEnergy { .. }));
    let top = position(|a| matches!(a, InternalAction::ResolveTopDrawCard { .. }));
    assert!(
        source < block && block < gain && gain < top,
        "{:?}",
        n.event_log
    );
    assert_eq!(n.state.player.block, 12); // Abacus 6 + two independent FNP 3
    assert_eq!(n.state.player.energy, 5);
    assert_eq!(n.state.monsters[0].hp, 485);
}

#[test]
fn queued_energy_context_roundtrips_without_rng_or_energy_consumption_at_extraction() {
    let mut s = setup(WHIRLWIND_ID);
    let before_card_rng = s.rng.card_random_rng.clone();
    let before_shuffle = s.rng.shuffle_rng.counter();
    let q: VecDeque<_> = apply_play_top_draw_card(&mut s, None, true, false)
        .unwrap()
        .into();
    assert_eq!(s.player.energy, 3);
    assert_eq!(s.rng.card_random_rng, before_card_rng);
    assert_eq!(s.rng.shuffle_rng.counter(), before_shuffle + 1);
    assert!(q.iter().any(|a| matches!(
        a,
        InternalAction::ResolveTopDrawCard {
            energy_on_use: Some(3),
            ..
        }
    )));
    let raw = serde_json::to_string(&(s.clone(), q.clone())).unwrap();
    let (restored, restored_q): (CombatState, VecDeque<InternalAction>) =
        serde_json::from_str(&raw).unwrap();
    let _ = crate::combat::legal_combat_actions(&s);
    assert_eq!(raw, serde_json::to_string(&(s.clone(), q.clone())).unwrap());
    assert_eq!(
        raw,
        serde_json::to_string(&(restored.clone(), restored_q.clone())).unwrap()
    );
    let a = process_internal_queue(&s, q).unwrap().state;
    let b = process_internal_queue(&restored, restored_q).unwrap().state;
    assert_eq!(
        serde_json::to_value(a).unwrap(),
        serde_json::to_value(b).unwrap()
    );
}

#[test]
fn missing_or_negative_pending_x_context_is_not_guessed_from_current_energy() {
    for context in [None, Some(-1)] {
        let mut s = setup(WHIRLWIND_ID);
        let card = s.piles.discard_pile.pop().unwrap();
        s.piles.limbo.push(card);
        assert!(process_internal_queue(
            &s,
            VecDeque::from([InternalAction::ResolveTopDrawCard {
                card_id: card.id,
                target: None,
                exhaust_played_card: true,
                energy_on_use: context
            }])
        )
        .is_err());
    }
    // The old optional field is immaterial to a fixed-cost card.
    let mut s = setup(DEFEND_R_ID);
    let card = s.piles.discard_pile.pop().unwrap();
    s.piles.limbo.push(card);
    let n = process_internal_queue(
        &s,
        VecDeque::from([InternalAction::ResolveTopDrawCard {
            card_id: card.id,
            target: None,
            exhaust_played_card: true,
            energy_on_use: None,
        }]),
    )
    .unwrap()
    .state;
    assert_eq!(n.player.block, 5);
    assert_eq!(n.player.energy, 3);
}
