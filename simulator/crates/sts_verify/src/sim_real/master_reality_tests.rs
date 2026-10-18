use super::*;
#[test]
fn master_reality_is_compared_by_native_presence_not_the_nonstacking_amount() {
    for amount in [-1, 0] {
        let message = json!({"game_state":{"screen_type":"NONE","combat_state":{"player":{"powers":[{"id":"MasterRealityPower","amount":amount}]}}}});
        let before = message.clone();
        let projection = seed_start_combat_observed_subset(&message);
        assert_eq!(projection["combat_player_master_reality"], true);
        assert_eq!(message, before);
    }
    let absent = seed_start_combat_observed_subset(
        &json!({"game_state":{"screen_type":"NONE","combat_state":{"player":{"powers":[]}}}}),
    );
    assert_eq!(absent["combat_player_master_reality"], false);
}
