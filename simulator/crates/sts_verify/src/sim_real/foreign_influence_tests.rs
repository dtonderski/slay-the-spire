use super::*;
#[test]
fn cross_color_combat_offer_ids_use_the_same_output_normalization_as_observed_cards() {
    let cards = json!([{"id":"Conclude","upgrades":0},{"id":"Gash","upgrades":0},{"id":"Skewer","upgrades":0}]);
    let expected = card_reward_ids_from_value(Some(&cards));
    let actual: Vec<_> = ["CONCLUDE", "CLAW", "SKEWER"]
        .map(sts_core::adapter_internals::content::shop_pool::shop_card_content_id)
        .into_iter()
        .map(simulated_card_reward_identity)
        .collect();
    assert_eq!(expected, actual);
    assert_eq!(
        actual,
        json!(["conclude", "claw", "skewer"])
            .as_array()
            .unwrap()
            .to_vec()
    );
}
