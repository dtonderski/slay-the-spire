//! Synthetic legal map-to-combat prefixes; not dedicated real-game trace parity.
use sts_core::adapter_internals::{
    apply_map_action_on_run, legal_map_actions_on_run, CardId, CardInstance, MapAction, MapNodeId,
    Relic, RoomKind, RunState,
};
use sts_core::content::cards::REGRET_ID;
fn setup(curses: u64, relics: Vec<Relic>, lifts: u32, elite: bool) -> RunState {
    let mut r = RunState::map_fixture();
    r.relics = relics;
    r.girya_lifts = lifts;
    r.deck = (100..100 + curses)
        .map(|id| CardInstance::new(CardId::new(id), REGRET_ID))
        .collect();
    if elite {
        r.map
            .as_mut()
            .unwrap()
            .map
            .nodes
            .iter_mut()
            .find(|n| n.id == MapNodeId::new(1))
            .unwrap()
            .room_kind = RoomKind::Elite;
    }
    r.validate().unwrap();
    r
}
fn enter(r: &RunState) -> RunState {
    let a = MapAction::ChooseNode {
        node_id: MapNodeId::new(1),
    };
    assert!(legal_map_actions_on_run(r).unwrap().contains(&a));
    let before = serde_json::to_value(r).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let next = apply_map_action_on_run(r, a).unwrap();
    next.validate().unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), before);
    assert_eq!(
        serde_json::to_value(apply_map_action_on_run(&restored, a).unwrap()).unwrap(),
        serde_json::to_value(&next).unwrap()
    );
    println!(
        "startup_strength_transition={}",
        serde_json::json!({"initial":before,"map_action":a,"result":next})
    );
    next
}
#[test]
fn duvu_caps_large_curse_count() {
    let r = enter(&setup(1000, vec![Relic::DuVuDoll], 0, false));
    assert_eq!(r.combat.as_ref().unwrap().player.powers.strength, 999);
}
#[test]
fn vajra_caps_strength_after_duvu() {
    let r = enter(&setup(999, vec![Relic::DuVuDoll, Relic::Vajra], 0, false));
    assert_eq!(r.combat.as_ref().unwrap().player.powers.strength, 999);
}
#[test]
fn girya_caps_strength_after_duvu() {
    let r = enter(&setup(997, vec![Relic::DuVuDoll, Relic::Girya], 3, false));
    assert_eq!(r.combat.as_ref().unwrap().player.powers.strength, 999);
}
#[test]
fn sling_and_duvu_cap_elite_strength() {
    let r = enter(&setup(
        998,
        vec![Relic::DuVuDoll, Relic::SlingOfCourage],
        0,
        true,
    ));
    assert_eq!(r.combat.as_ref().unwrap().player.powers.strength, 999);
}
#[test]
fn combined_startup_gains_remain_bounded() {
    let r = enter(&setup(
        999,
        vec![
            Relic::DuVuDoll,
            Relic::Girya,
            Relic::Vajra,
            Relic::SlingOfCourage,
        ],
        3,
        true,
    ));
    assert_eq!(r.combat.as_ref().unwrap().player.powers.strength, 999);
}
#[test]
fn ordinary_startup_control() {
    let r = enter(&setup(
        3,
        vec![Relic::DuVuDoll, Relic::Girya, Relic::Vajra],
        3,
        false,
    ));
    assert_eq!(r.combat.as_ref().unwrap().player.powers.strength, 7);
}
#[test]
fn no_duvu_control_does_not_grant_curse_strength() {
    let r = enter(&setup(1000, vec![Relic::Vajra], 0, false));
    assert_eq!(r.combat.as_ref().unwrap().player.powers.strength, 1);
}
