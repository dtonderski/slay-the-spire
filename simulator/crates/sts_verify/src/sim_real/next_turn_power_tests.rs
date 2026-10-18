use super::*;
#[test]
fn observed_next_turn_powers_include_both_native_energized_colors() {
    let message = json!({"game_state":{"screen_type":"NONE","combat_state":{"player":{"powers":[
        {"id":"Energized","amount":2},{"id":"EnergizedBlue","amount":3},{"id":"Draw Card","amount":4}
    ]}}}});
    let before = message.clone();
    let subset = seed_start_combat_observed_subset(&message);
    assert_eq!(subset["combat_player_energized"], 5);
    assert_eq!(subset["combat_player_draw_card"], 4);
    assert_eq!(message, before);
    let empty = seed_start_combat_observed_subset(
        &json!({"game_state":{"screen_type":"NONE","combat_state":{"player":{"powers":[]}}}}),
    );
    assert_eq!(empty["combat_player_energized"], 0);
    assert_eq!(empty["combat_player_draw_card"], 0);
}
