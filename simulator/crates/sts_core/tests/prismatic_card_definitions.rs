//! Cross-check the hand-transcribed Prismatic gameplay table against an
//! independently generated desktop-JAR audit table.
use std::collections::BTreeSet;
use sts_core::{
    card::CardRarity,
    content::{
        cards::get_card_definition, prismatic::prismatic_card_spec, shop_pool::shop_card_content_id,
    },
    run::reward::any_color_reward_card_key,
};

#[allow(dead_code)]
mod desktop {
    include!("data/prismatic_desktop_metadata.rs");
}

#[test]
fn every_formerly_unmodeled_pool_card_has_a_matching_gameplay_definition() {
    assert_eq!(desktop::REWARD_ONLY_CARD_METADATA.len(), 142);
    let mut ids = BTreeSet::new();
    let mut counts = [0; 3];
    for audit in desktop::REWARD_ONLY_CARD_METADATA {
        let id = shop_card_content_id(audit.key);
        assert!(ids.insert(id));
        assert_eq!(any_color_reward_card_key(id), Some(audit.key));
        let spec = prismatic_card_spec(id).unwrap_or_else(|| panic!("{}", audit.key));
        let definition = get_card_definition(id).expect("registered");
        assert_eq!(definition, &spec.definition, "{}", audit.key);
        assert_eq!(spec.source_card_id, audit.card_id, "{}", audit.key);
        assert_eq!(spec.definition.cost, audit.cost, "{}", audit.key);
        assert_eq!(spec.upgraded_cost, audit.upgraded_cost, "{}", audit.key);
        assert_eq!(spec.definition.card_type, audit.card_type, "{}", audit.key);
        assert_eq!(spec.definition.rarity, Some(audit.rarity), "{}", audit.key);
        counts[match audit.rarity {
            CardRarity::Common => 0,
            CardRarity::Uncommon => 1,
            CardRarity::Rare => 2,
        }] += 1;
    }
    assert_eq!(counts, [20, 79, 43]);
}

#[test]
fn every_any_color_reward_pool_entry_is_now_a_gameplay_definition() {
    for rarity in [CardRarity::Common, CardRarity::Uncommon, CardRarity::Rare] {
        for key in sts_core::run::reward::any_color_reward_pool(rarity) {
            let id = shop_card_content_id(key);
            assert!(get_card_definition(id).is_some(), "{key}");
        }
    }
}
