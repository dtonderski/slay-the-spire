use super::*;
use crate::{
    card::CardType,
    content::{card_library_types, cards::get_card_definition, shop_pool::shop_card_content_id},
    run::{CardRewardFlow, RewardContinuation, RewardScreen},
};
fn card(key: &str, id: u64) -> CardInstance {
    CardInstance::new(CardId::new(id), shop_card_content_id(key))
}
#[test]
fn pc_type_metadata_covers_every_existing_pool_key_and_agrees_with_modeled_cards() {
    let mut counts = [0usize; 3];
    let mut seen = std::collections::BTreeSet::new();
    for key in ANY_COLOR_COMMON_CARDS
        .iter()
        .chain(ANY_COLOR_UNCOMMON_CARDS)
        .chain(ANY_COLOR_RARE_CARDS)
    {
        assert!(seen.insert(key));
        let ty = card_library_types::card_type(key).expect("every pool key has PC metadata");
        let index = match ty {
            CardType::Attack => 0,
            CardType::Skill => 1,
            CardType::Power => 2,
            CardType::Status => panic!("regular reward pool contains status"),
        };
        counts[index] += 1;
        if let Some(def) = get_card_definition(shop_card_content_id(key)) {
            assert_eq!(ty, def.card_type, "{key}");
        }
    }
    assert_eq!(seen.len(), 320);
    assert_eq!(counts, [109, 156, 55]);
    assert_eq!(card_library_types::card_type("not-a-card"), None);
}
#[test]
fn unknown_effects_use_exact_native_type_for_all_three_eggs_without_inventing_support() {
    for (key, ty) in [
        ("BARRAGE", CardType::Attack),
        ("AGGREGATE", CardType::Skill),
        ("HELLO_WORLD", CardType::Power),
    ] {
        let base = card(key, 1000);
        assert!(get_card_definition(base.content_id).is_none());
        for egg in [Relic::MoltenEgg, Relic::ToxicEgg, Relic::FrozenEgg] {
            let r = RunState::seeded_ironclad(1, 0);
            let mut r = r;
            r.relics = vec![egg];
            let expected = matches!(
                (egg, ty),
                (Relic::MoltenEgg, CardType::Attack)
                    | (Relic::ToxicEgg, CardType::Skill)
                    | (Relic::FrozenEgg, CardType::Power)
            );
            let n = r.card_after_card_add_relics(base).unwrap();
            assert_eq!(n.upgrades, u8::from(expected), "{key}/{egg:?}");
            assert_eq!(n.content_id, base.content_id);
            assert_eq!(
                r.card_after_card_add_relics(n).unwrap(),
                n,
                "preview then obtain is not two upgrades"
            );
            assert!(
                get_card_definition(n.content_id).is_none(),
                "type metadata does not implement effects"
            );
        }
    }
}
#[test]
fn no_egg_preserves_unknown_effect_card_and_an_unknown_identity_still_fails_closed() {
    let r = RunState::seeded_ironclad(7, 0);
    let c = card("BARRAGE", 1000);
    assert_eq!(r.card_after_card_add_relics(c).unwrap(), c);
    assert!(r
        .card_after_card_add_relics(CardInstance::new(
            CardId::new(1001),
            ContentId::new(u64::MAX)
        ))
        .is_err());
}
#[test]
fn preview_and_real_obtain_preserve_generic_upgrade_and_do_not_draw_rng() {
    let mut r = RunState::seeded_ironclad(7, 0);
    r.relics = vec![Relic::MoltenEgg];
    let before = serde_json::to_value(&r).unwrap();
    let ((preview, ()), events) = crate::rng::capture_rng_trace(|| {
        let preview = r.card_after_card_add_relics(card("BARRAGE", 1000)).unwrap();
        let mut obtained = r.clone();
        obtained.add_deck_card(preview).unwrap();
        assert_eq!(obtained.deck.last().unwrap().upgrades, 1);
        (preview, ())
    });
    assert_eq!(preview.upgrades, 1);
    assert!(events.is_empty());
    assert_eq!(serde_json::to_value(r).unwrap(), before);
}
#[test]
fn frozen_egg_on_equip_upgrades_only_powers_in_current_and_queued_unknown_effect_offers() {
    let mut r = RunState::seeded_ironclad(7, 0);
    r.phase = RunPhase::Reward;
    r.reward = Some(RewardScreen {
        continuation: RewardContinuation::None,
        choices: vec![
            card("BARRAGE", 1000),
            card("AGGREGATE", 1001),
            card("HELLO_WORLD", 1002),
        ],
        queued_card_rewards: vec![vec![card("BARRAGE", 1003), card("HELLO_WORLD", 1004)]],
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer: None,
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: CardRewardFlow::pending(1),
    });
    r.gain_relic(Relic::FrozenEgg).unwrap();
    let a = r.reward.as_ref().unwrap();
    assert_eq!(
        a.choices.iter().map(|c| c.upgrades).collect::<Vec<_>>(),
        vec![0, 0, 1]
    );
    assert_eq!(
        a.queued_card_rewards[0]
            .iter()
            .map(|c| c.upgrades)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    r.gain_relic(Relic::MoltenEgg).unwrap();
    let a = r.reward.as_ref().unwrap();
    assert_eq!(
        a.choices.iter().map(|c| c.upgrades).collect::<Vec<_>>(),
        vec![1, 0, 1]
    );
}
#[test]
fn every_known_pool_card_remains_unchanged_under_a_nonmatching_egg() {
    for key in ANY_COLOR_COMMON_CARDS
        .iter()
        .chain(ANY_COLOR_UNCOMMON_CARDS)
        .chain(ANY_COLOR_RARE_CARDS)
    {
        let ty = card_library_types::card_type(key).unwrap();
        let mut r = RunState::seeded_ironclad(11, 0);
        r.relics = vec![if ty == CardType::Power {
            Relic::ToxicEgg
        } else {
            Relic::FrozenEgg
        }];
        let c = card(key, 1000);
        assert_eq!(r.card_after_card_add_relics(c).unwrap(), c, "{key}");
    }
}
