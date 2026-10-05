//! Initialized legacy publication diagnostics, not natural legal-action prefixes.
use sts_core::adapter_internals::{
    end_player_turn, CardId, CardInstance, CombatState, Relic, RunState,
};
use sts_core::combat::settle_queued_end_turn_discard_after_rejected_command;
use sts_core::content::cards::{BURN_ID, DAZED_ID, STRIKE_R_ID};
fn setup() -> CombatState {
    let mut c = RunState::combat_fixture().combat.unwrap();
    c.player.authority.relics.clear();
    c.piles.hand = vec![CardInstance::new(CardId::new(100), DAZED_ID)];
    c.piles.draw_pile = vec![CardInstance::new(CardId::new(200), STRIKE_R_ID)];
    c.piles.discard_pile.clear();
    c.resume_end_turn_after_nilrys_codex = true;
    c.nilrys_end_powers_pending = false;
    c.monsters[0].hp = 4000;
    c.monsters[0].max_hp = 4000;
    c.validate().unwrap();
    c
}
fn settle(c: &CombatState) -> CombatState {
    let original = serde_json::to_value(c).unwrap();
    let mut n = c.clone();
    let mut restored: CombatState = serde_json::from_value(original.clone()).unwrap();
    settle_queued_end_turn_discard_after_rejected_command(&mut n).unwrap();
    n.validate().unwrap();
    settle_queued_end_turn_discard_after_rejected_command(&mut restored).unwrap();
    assert_eq!(
        serde_json::to_value(&n).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert_eq!(serde_json::to_value(c).unwrap(), original);
    println!(
        "deferred_hand_diagnostic={}",
        serde_json::json!({"initial":original,"published":n})
    );
    n
}
#[test]
fn burn_consumption_is_not_undone_by_block_restoration() {
    let mut c = setup();
    c.piles.hand[0].content_id = BURN_ID;
    c.player.block = 10;
    let n = settle(&c);
    assert_eq!(n.player.block, 8);
}
#[test]
fn juggernaut_survives_deferred_full_block_gain() {
    let mut c = setup();
    c.player.block = 998;
    c.player.powers.feel_no_pain = 6;
    c.player.powers.juggernaut = 5;
    let pending = settle(&c);
    assert_eq!(
        pending.player.block, 998,
        "GainBlockAction is still pending"
    );
    let n = end_player_turn(&pending).unwrap();
    assert_eq!(n.monsters[0].hp, 3995);
}
#[test]
fn dead_branch_generated_card_remains_owned_until_insertion() {
    let mut c = setup();
    c.player.authority.relics.push(Relic::DeadBranch);
    let pending = settle(&c);
    assert!(pending.piles.hand.is_empty());
    let n = end_player_turn(&pending).unwrap();
    assert_eq!(
        n.piles.hand.len()
            + n.piles.draw_pile.len()
            + n.piles.discard_pile.len()
            + n.piles.exhaust_pile.len()
            + n.piles.limbo.len(),
        3,
        "initial two physical cards plus generated card"
    );
}
#[test]
fn dark_embrace_lethal_draw_precedes_the_monster_attack() {
    let mut c = setup();
    c.player.powers.dark_embrace = 1;
    c.player.powers.fire_breathing = 6;
    c.piles.draw_pile[0].content_id = BURN_ID;
    c.monsters[0].hp = 6;
    c.monsters[0].max_hp = 6;
    let hp = c.player.hp;
    let pending = settle(&c);
    let n = end_player_turn(&pending).unwrap();
    assert_eq!(
        n.player.hp, hp,
        "queued on-exhaust draw must kill before takeTurn"
    );
}
#[test]
fn opening_publication_retains_exhaust_callbacks_too() {
    let mut c = setup();
    c.opening_end_turn_pending = true;
    c.resume_end_turn_after_nilrys_codex = false;
    c.player.powers.feel_no_pain = 6;
    c.player.powers.juggernaut = 5;
    let n = end_player_turn(&settle(&c)).unwrap();
    assert_eq!(n.monsters[0].hp, 3995);
}

#[test]
fn two_clipped_gains_produce_two_nominal_callbacks() {
    let mut c = setup();
    c.piles
        .hand
        .push(CardInstance::new(CardId::new(101), DAZED_ID));
    c.player.block = 999;
    c.player.powers.feel_no_pain = 6;
    c.player.powers.juggernaut = 5;
    let pending = settle(&c);
    let json = serde_json::to_value(&pending).unwrap();
    assert_eq!(
        json["pending_end_turn_hand_resolution"]["ethereal_follow_ups"],
        serde_json::json!([{"GainBlock":{"amount":6}},{"GainBlock":{"amount":6}}])
    );
    let n = end_player_turn(&pending).unwrap();
    assert_eq!(n.monsters[0].hp, 3990);
}

#[test]
fn serialized_resume_consumes_callbacks_once() {
    let mut c = setup();
    c.player.powers.feel_no_pain = 6;
    c.player.powers.juggernaut = 5;
    let pending = settle(&c);
    let original = serde_json::to_value(&pending).unwrap();
    let restored: CombatState = serde_json::from_value(original.clone()).unwrap();
    let n = end_player_turn(&pending).unwrap();
    n.validate().unwrap();
    assert_eq!(
        serde_json::to_value(&n).unwrap(),
        serde_json::to_value(end_player_turn(&restored).unwrap()).unwrap()
    );
    assert_eq!(serde_json::to_value(&pending).unwrap(), original);
    assert!(n.pending_end_turn_hand_resolution.is_none());
    assert_eq!(n.monsters[0].hp, 3995);
    let again = end_player_turn(&n).unwrap();
    assert_eq!(again.monsters[0].hp, 3995);
}

#[test]
fn pending_generated_cards_participate_in_duplicate_id_validation() {
    let mut c = setup();
    c.player.authority.relics.push(Relic::DeadBranch);
    let pending = settle(&c);
    let mut value = serde_json::to_value(&pending).unwrap();
    value["pending_end_turn_hand_resolution"]["ethereal_follow_ups"][0]["DeadBranch"]["id"] =
        serde_json::to_value(c.piles.draw_pile[0].id).unwrap();
    let malformed: CombatState = serde_json::from_value(value).unwrap();
    assert!(malformed.validate().is_err());
}

#[test]
fn missing_legacy_callback_context_is_rejected_not_inferred() {
    let mut c = setup();
    c.pending_end_turn_feel_no_pain_block = 1;
    let before = serde_json::to_value(&c).unwrap();
    assert_eq!(
        end_player_turn(&c),
        Err(sts_core::adapter_internals::SimError::InvalidState(
            "legacy deferred block requires per-exhaust action context"
        ))
    );
    assert_eq!(serde_json::to_value(&c).unwrap(), before);
}

#[test]
fn rejected_publication_rolls_back_whole_state() {
    let mut c = setup();
    c.nilrys_end_powers_pending = true;
    c.player.cannot_draw = true;
    c.player.no_draw_precedes_combust = true;
    c.player.powers.strength = i32::MAX;
    c.player.powers.ritual = 1;
    let before = serde_json::to_value(&c).unwrap();
    assert!(settle_queued_end_turn_discard_after_rejected_command(&mut c).is_err());
    assert_eq!(serde_json::to_value(&c).unwrap(), before);
}

#[test]
fn occupied_or_missing_publication_context_rejects_without_repair() {
    let mut pending = settle(&setup());
    pending.resume_end_turn_after_nilrys_codex = true;
    let before = serde_json::to_value(&pending).unwrap();
    assert_eq!(
        settle_queued_end_turn_discard_after_rejected_command(&mut pending),
        Err(sts_core::adapter_internals::SimError::InvalidState(
            "deferred end-turn hand callbacks already occupied"
        ))
    );
    assert_eq!(serde_json::to_value(&pending).unwrap(), before);
    pending.resume_end_turn_after_nilrys_codex = false;
    pending.time_warp_end_turn_pre_discard_settled = false;
    let before = serde_json::to_value(&pending).unwrap();
    assert!(pending.validate().is_err());
    assert_eq!(
        end_player_turn(&pending),
        Err(sts_core::adapter_internals::SimError::InvalidState(
            "deferred end-turn hand callbacks have no publication marker"
        ))
    );
    assert_eq!(serde_json::to_value(&pending).unwrap(), before);
}

#[test]
fn an_open_codex_offer_cannot_be_drained_by_publication_helper() {
    let c = RunState::combat_fixture_with_relics(vec![Relic::NilrysCodex])
        .combat
        .unwrap();
    let mut paused = end_player_turn(&c).unwrap();
    assert!(paused.decision.is_some());
    assert!(paused.resume_end_turn_after_nilrys_codex);
    let before = serde_json::to_value(&paused).unwrap();
    assert_eq!(
        settle_queued_end_turn_discard_after_rejected_command(&mut paused),
        Err(sts_core::adapter_internals::SimError::IllegalAction(
            "end-turn publication requires a closed selection"
        ))
    );
    assert_eq!(serde_json::to_value(&paused).unwrap(), before);
}

#[test]
fn no_callback_discard_control() {
    let c = setup();
    let n = settle(&c);
    assert!(n.piles.hand.is_empty());
    assert_eq!(n.piles.exhaust_pile.len(), 1);
    assert_eq!(n.player.block, 0);
}
