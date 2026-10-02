use sts_core::adapter_internals::{CardId, InternalAction};

#[test]
fn renamed_cost_reduction_action_preserves_snapshot_wire_name() {
    let action = InternalAction::ReduceCardCostForCombat {
        card_id: CardId::new(12),
        amount: 1,
    };
    let value = serde_json::to_value(action).expect("serialize");
    assert!(value.get("ReduceHandCardCostForCombat").is_some());
    assert!(
        matches!(serde_json::from_value::<InternalAction>(value).expect("deserialize"),
        InternalAction::ReduceCardCostForCombat { card_id, amount: 1 } if card_id == CardId::new(12))
    );
}
