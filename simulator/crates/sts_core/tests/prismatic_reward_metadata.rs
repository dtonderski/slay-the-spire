//! Source-backed reward-preview coverage, not foreign-card combat parity.
use std::collections::BTreeSet;
use sts_core::{
    card::CardType,
    content::{
        cards::get_card_definition,
        reward_card_metadata::{reward_only_card_metadata, REWARD_ONLY_CARD_METADATA},
        shop_pool::shop_card_content_id,
    },
    run::reward::any_color_reward_card_key,
};

#[test]
fn reward_only_metadata_is_a_disjoint_explicit_pool_subset_not_gameplay_definitions() {
    assert_eq!(REWARD_ONLY_CARD_METADATA.len(), 142);
    let mut ids = BTreeSet::new();
    let mut identities = BTreeSet::new();
    let mut counts = [0; 3];
    for metadata in REWARD_ONLY_CARD_METADATA {
        let id = shop_card_content_id(metadata.key);
        assert!(ids.insert(id));
        assert!(identities.insert(metadata.card_id));
        assert_eq!(reward_only_card_metadata(id).unwrap().key, metadata.key);
        assert_eq!(any_color_reward_card_key(id), Some(metadata.key));
        assert!(
            get_card_definition(id).is_none(),
            "metadata is not an effect: {}",
            metadata.key
        );
        counts[match metadata.rarity {
            sts_core::card::CardRarity::Common => 0,
            sts_core::card::CardRarity::Uncommon => 1,
            sts_core::card::CardRarity::Rare => 2,
        }] += 1;
        assert!(metadata.cost >= -2 && metadata.upgraded_cost >= -2);
    }
    assert_eq!(counts, [20, 79, 43]);
}

#[test]
fn desktop_legacy_id_and_upgrade_cost_vectors_are_not_guessed_defaults() {
    // Constructor and upgradeBaseCost in the named desktop-JAR classes.
    for (key, card_id, cost, upgraded, card_type) in [
        ("ACROBATICS", "Acrobatics", 1, 1, CardType::Skill),
        ("BULLSEYE", "Lockon", 1, 1, CardType::Attack),
        ("RUSHDOWN", "Adaptation", 1, 0, CardType::Power),
        ("TERROR", "Terror", 1, 0, CardType::Skill),
        ("BULLET_TIME", "Bullet Time", 3, 2, CardType::Skill),
        ("DEUS_EX_MACHINA", "DeusExMachina", -2, -2, CardType::Skill),
        ("TEMPEST", "Tempest", -1, -1, CardType::Skill),
    ] {
        let metadata = reward_only_card_metadata(shop_card_content_id(key)).unwrap();
        assert_eq!(metadata.card_id, card_id);
        assert_eq!(metadata.cost, cost);
        assert_eq!(metadata.upgraded_cost, upgraded);
        assert_eq!(metadata.card_type, card_type);
    }
}
