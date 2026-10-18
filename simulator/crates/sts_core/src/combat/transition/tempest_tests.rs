//! PC Tempest.use / TempestAction.update / ChannelAction / Lightning.onEvoke.
use super::*;
use crate::content::cards::{HAVOC_ID, PAIN_ID, TEMPEST_ANY_COLOR_ID, VOID_ID};

fn setup(upgraded: bool, energy: i32) -> CombatState {
    let mut s = CombatState::cultist_fixture();
    s.player.authority.relics.clear();
    s.player.energy = energy;
    s.monsters[0].hp = 500;
    s.monsters[0].max_hp = 500;
    s.max_orbs = 1;
    s.orbs.clear();
    let mut c = CardInstance::new(CardId::new(41), TEMPEST_ANY_COLOR_ID);
    c.upgrades = u8::from(upgraded);
    s.piles.hand = vec![c];
    s.piles.draw_pile.clear();
    s.piles.discard_pile.clear();
    s.piles.exhaust_pile.clear();
    s.validate().unwrap();
    s
}
fn play(s: &CombatState) -> CombatTransition {
    apply_combat_action_with_events(
        s,
        CombatAction::PlayCard {
            card_id: CardId::new(41),
            target: None,
        },
    )
    .unwrap()
}

#[test]
fn native_identity_cost_upgrade_channels_energy_exhaust_and_rng() {
    assert_eq!(
        TEMPEST_ANY_COLOR_ID,
        crate::content::shop_pool::shop_card_content_id("TEMPEST")
    );
    let d = get_card_definition(TEMPEST_ANY_COLOR_ID).unwrap();
    assert_eq!(d.cost, -1);
    assert!(d.keywords.exhaust);
    assert_eq!(d.card_type, CardType::Skill);
    for upgraded in [false, true] {
        for energy in 0..=3 {
            let s = setup(upgraded, energy);
            let before = serde_json::to_value(&s).unwrap();
            let _ = crate::combat::legal_combat_actions(&s);
            let n = play(&s).state;
            let channels = energy + i32::from(upgraded);
            let evokes = (channels - 1).max(0);
            assert_eq!(n.orbs.len(), usize::from(channels > 0));
            assert_eq!(n.monsters[0].hp, 500 - 8 * evokes);
            assert_eq!(n.player.energy, 0);
            assert_eq!(
                n.piles
                    .exhaust_pile
                    .iter()
                    .filter(|c| c.id == CardId::new(41))
                    .count(),
                1
            );
            let mut expected = s.rng.card_random_rng.clone();
            for _ in 0..evokes {
                expected.random_int(0);
            }
            assert_eq!(n.rng.card_random_rng, expected);
            assert_eq!(n.rng.shuffle_rng, s.rng.shuffle_rng);
            assert_eq!(serde_json::to_value(&s).unwrap(), before);
        }
    }
}

#[test]
fn chemical_x_focus_no_draw_and_free_once_are_orthogonal() {
    let mut s = setup(false, 0);
    s.player.authority.relics = vec![Relic::ChemicalX];
    s.player.cannot_draw = true;
    s.player.powers.focus = 2;
    s.player.powers.strength = 20;
    s.player.powers.frail = 2;
    let n = play(&s).state;
    assert_eq!(n.monsters[0].hp, 490); // two channels, one (8+2) evoke
    assert_eq!(n.player.energy, 0);
    s.player.energy = 3;
    s.piles.hand[0].free_to_play_once = true;
    let n = play(&s).state;
    assert_eq!(n.player.energy, 3);
    assert_eq!(n.monsters[0].hp, 460); // five channels, four focused evokes
    s.max_orbs = 0;
    s.piles.hand[0].free_to_play_once = false;
    let n = play(&s).state;
    assert!(n.orbs.is_empty());
    assert_eq!(n.player.energy, 0);
    assert_eq!(n.rng.card_random_rng, s.rng.card_random_rng);
}

#[test]
fn native_wrapper_spends_live_total_only_for_positive_effect_and_roundtrips() {
    for (reserved, upgraded, free, expected) in [
        (1, false, false, 0),
        (0, false, false, 5),
        (0, true, false, 0),
        (1, false, true, 5),
    ] {
        let s = setup(false, 3);
        let q = VecDeque::from([
            InternalAction::GainEnergy { amount: 2 },
            InternalAction::ResolveTempest {
                energy_on_use: reserved,
                upgraded,
                free_to_play_once: free,
            },
        ]);
        let raw = serde_json::to_string(&(s.clone(), q.clone())).unwrap();
        let (r, rq): (CombatState, VecDeque<InternalAction>) = serde_json::from_str(&raw).unwrap();
        let n = process_internal_queue(&s, q).unwrap().state;
        let restored = process_internal_queue(&r, rq).unwrap().state;
        assert_eq!(n.player.energy, expected);
        assert_eq!(
            n.orbs.len(),
            usize::from(reserved + i32::from(upgraded) > 0)
        );
        assert_eq!(
            serde_json::to_value(n).unwrap(),
            serde_json::to_value(restored).unwrap()
        );
    }
    let mut s = setup(true, 1);
    s.player.authority.relics.push(Relic::ChemicalX);
    let before = serde_json::to_value(&s).unwrap();
    assert!(player_actions::resolve_tempest(&mut s, i32::MAX, true, false).is_err());
    assert!(player_actions::resolve_tempest(&mut s, -1, false, false).is_err());
    assert_eq!(before, serde_json::to_value(s).unwrap());
}

#[test]
fn source_exhaust_branch_reservation_precedes_channels() {
    let mut s = setup(true, 3);
    s.player.authority.relics = vec![Relic::DeadBranch];
    s.player.powers.feel_no_pain = 3;
    let n = play(&s);
    let exhausted = n.event_log.iter().position(|a| matches!(a,InternalAction::CardExhausted { card_id } if *card_id == CardId::new(41))).unwrap();
    let channel = n
        .event_log
        .iter()
        .position(|a| matches!(a, InternalAction::ChannelLightning))
        .unwrap();
    assert!(exhausted < channel, "{:?}", n.event_log);
    assert_eq!(n.state.piles.hand.len(), 1);
    assert_eq!(n.state.player.block, 3);
    assert_eq!(n.state.monsters[0].hp, 476);
}

#[test]
fn havoc_uses_reserved_x_and_autoplay_free_flag_after_sundial() {
    let mut s = setup(true, 3);
    let card = s.piles.hand.pop().unwrap();
    s.piles
        .hand
        .push(CardInstance::new(CardId::new(99), HAVOC_ID));
    s.piles.discard_pile.push(card);
    s.player.authority.relics = vec![Relic::Sundial];
    s.sundial_shuffles = 2;
    let n = apply_combat_action(
        &s,
        CombatAction::PlayCard {
            card_id: CardId::new(99),
            target: None,
        },
    )
    .unwrap();
    assert_eq!(n.player.energy, 4);
    assert_eq!(n.monsters[0].hp, 484); // X=2 + upgrade1, not live4 +1
    assert!(n.piles.exhaust_pile.iter().any(|c| c.id == card.id));
}

#[test]
fn copied_uses_keep_original_x_but_are_free_and_do_not_reexhaust_source() {
    for stacks in [1, 2] {
        let mut s = setup(true, 3);
        s.duplication_potion_pending = true;
        s.duplication_potion_stacks = stacks;
        let n = play(&s);
        let wrappers: Vec<_> = n
            .event_log
            .iter()
            .filter_map(|a| {
                if let InternalAction::ResolveTempest {
                    energy_on_use,
                    free_to_play_once,
                    ..
                } = a
                {
                    Some((*energy_on_use, *free_to_play_once))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(wrappers, vec![(3, false), (3, true)]);
        assert_eq!(n.state.monsters[0].hp, 444); // eight channels, seven evokes
        assert_eq!(
            n.state
                .piles
                .exhaust_pile
                .iter()
                .filter(|c| c.id == CardId::new(41))
                .count(),
            1
        );
        assert_eq!(n.event_log.iter().filter(|a|matches!(a,InternalAction::CardExhausted { card_id } if *card_id == CardId::new(41))).count(),1);
        assert_eq!(n.state.player.energy, 0);
    }
}

#[test]
fn pain_cube_void_resolves_before_wrapper_without_changing_reserved_x() {
    let mut s = setup(false, 3);
    s.player.authority.relics = vec![Relic::RunicCube];
    s.piles
        .hand
        .push(CardInstance::new(CardId::new(98), PAIN_ID));
    s.piles
        .draw_pile
        .push(CardInstance::new(CardId::new(97), VOID_ID));
    let before_hp = s.player.hp;
    let n = play(&s);
    let loss = n
        .event_log
        .iter()
        .position(|a| matches!(a, InternalAction::LoseHp { .. }))
        .unwrap();
    let wrapper = n
        .event_log
        .iter()
        .position(|a| matches!(a, InternalAction::ResolveTempest { .. }))
        .unwrap();
    assert!(loss < wrapper);
    assert_eq!(n.state.player.hp, before_hp - 1);
    assert_eq!(n.state.player.energy, 0);
    assert_eq!(n.state.monsters[0].hp, 484);
    assert!(n.state.piles.hand.iter().any(|c| c.id == CardId::new(97)));
}

#[test]
fn lethal_clear_cancels_existing_wrappers_channels_but_not_newly_created_work() {
    let mut s = setup(true, 3);
    let target = s.monsters[0].id;
    s.monsters[0].hp = 1;
    let q = VecDeque::from([
        InternalAction::DealUnmodifiedDamage { target, amount: 1 },
        InternalAction::ResolveTempest {
            energy_on_use: 3,
            upgraded: true,
            free_to_play_once: false,
        },
        InternalAction::ChannelLightning,
        InternalAction::ChannelFrost,
        InternalAction::ChannelDark,
        InternalAction::MoveCard {
            card_id: CardId::new(41),
            from: CardPile::Hand,
            to: CardPile::ExhaustPile,
        },
    ]);
    let n = process_internal_queue_with_event_recording(&s, q, true).unwrap();
    assert!(n.state.orbs.is_empty());
    assert_eq!(n.state.player.energy, 3);
    assert!(n
        .state
        .piles
        .exhaust_pile
        .iter()
        .any(|c| c.id == CardId::new(41)));
    let later = process_internal_queue_with_event_recording(
        &n.state,
        VecDeque::from([InternalAction::ResolveTempest {
            energy_on_use: 1,
            upgraded: false,
            free_to_play_once: false,
        }]),
        true,
    )
    .unwrap();
    assert!(later
        .event_log
        .iter()
        .any(|a| matches!(a, InternalAction::ChannelLightning)));
    assert_eq!(later.state.orbs.len(), 1);
    assert_eq!(later.state.player.energy, 0);
}
