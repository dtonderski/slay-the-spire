use crate::model::{
    ActionId, BridgeId, BridgeStatus, Character, LegalAction, LegalActionKind, LiveError,
    LivePhase, LiveResult, LiveState, RunConfig,
};
use serde_json::json;
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

/// Bridge backend contract. Real CommunicationMod support and fake tests share this surface.
pub trait BridgeManager {
    fn list_bridges(&self) -> LiveResult<Vec<BridgeStatus>>;
    /// Captures authoritative persistent profile inputs before a run starts.
    /// Bridges without a dedicated pre-run snapshot retain the post-START
    /// state fallback in `SessionStore` by returning `None`.
    fn profile_snapshot(
        &mut self,
        _bridge_id: &BridgeId,
    ) -> LiveResult<Option<sts_verify::TraceProfile>> {
        Ok(None)
    }
    fn start_run(&mut self, bridge_id: &BridgeId, config: &RunConfig) -> LiveResult<LiveState>;
    fn start_verification_run(
        &mut self,
        _bridge_id: &BridgeId,
        _config: &RunConfig,
        _starting_hp: i32,
    ) -> LiveResult<LiveState> {
        Err(LiveError::InvalidAction(
            "bridge does not support START_VERIFY".to_owned(),
        ))
    }
    fn abandon_run(&mut self, bridge_id: &BridgeId) -> LiveResult<LiveState>;
    fn request_state(&mut self, bridge_id: &BridgeId) -> LiveResult<LiveState>;
    fn send_action(&mut self, bridge_id: &BridgeId, action: &LegalAction) -> LiveResult<LiveState>;
    fn kill_bridge(&mut self, bridge_id: &BridgeId) -> LiveResult<()>;
    fn kill_all(&mut self) -> LiveResult<usize>;
}

#[derive(Debug, Clone)]
struct FakeBridge {
    status: BridgeStatus,
    state: LiveState,
}

#[derive(Debug, Clone)]
pub struct FakeBridgeManager {
    bridges: HashMap<BridgeId, FakeBridge>,
    next_sequence: u64,
}

impl Default for FakeBridgeManager {
    fn default() -> Self {
        Self::with_default_bridge()
    }
}

impl FakeBridgeManager {
    pub fn with_default_bridge() -> Self {
        let mut manager = Self {
            bridges: HashMap::new(),
            next_sequence: 1,
        };
        manager.add_bridge(BridgeId("fake-bridge-1".to_owned()));
        manager
    }

    pub fn add_bridge(&mut self, id: BridgeId) {
        let state = LiveState {
            sequence: 0,
            phase: LivePhase::Menu,
            legal_actions: vec![request_state_action()],
            raw: json!({"screen": "menu"}),
        };
        let status = BridgeStatus {
            id: id.clone(),
            process_id: Some(1001),
            client_id: Some("fake-client".to_owned()),
            connected: true,
            last_heartbeat_ms: Some(now_ms()),
        };
        self.bridges.insert(id, FakeBridge { status, state });
    }

    fn bridge_mut(&mut self, bridge_id: &BridgeId) -> LiveResult<&mut FakeBridge> {
        self.bridges
            .get_mut(bridge_id)
            .ok_or_else(|| LiveError::NotFound(format!("bridge {}", bridge_id.0)))
    }

    fn next_state_sequence(&mut self) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        sequence
    }
}

impl BridgeManager for FakeBridgeManager {
    fn list_bridges(&self) -> LiveResult<Vec<BridgeStatus>> {
        Ok(self
            .bridges
            .values()
            .map(|bridge| bridge.status.clone())
            .collect())
    }

    fn start_run(&mut self, bridge_id: &BridgeId, config: &RunConfig) -> LiveResult<LiveState> {
        let sequence = self.next_state_sequence();
        let bridge = self.bridge_mut(bridge_id)?;
        let state = LiveState {
            sequence,
            phase: LivePhase::Neow,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("talk".to_owned()),
                    kind: LegalActionKind::ChooseNeow,
                    label: "Talk".to_owned(),
                    enabled: true,
                    command: json!({"kind": "choose_neow", "choice": 0}),
                    disabled_reason: None,
                },
                request_state_action(),
            ],
            raw: json!({
                "screen": "neow",
                "character": match config.character { Character::Ironclad => "ironclad" },
                "ascension": config.ascension,
                "seed": config.seed,
            }),
        };
        bridge.status.last_heartbeat_ms = Some(now_ms());
        bridge.state = state.clone();
        Ok(state)
    }

    fn start_verification_run(
        &mut self,
        bridge_id: &BridgeId,
        config: &RunConfig,
        starting_hp: i32,
    ) -> LiveResult<LiveState> {
        let mut state = self.start_run(bridge_id, config)?;
        state.raw["verification_starting_hp"] = json!(starting_hp);
        self.bridge_mut(bridge_id)?.state = state.clone();
        Ok(state)
    }

    fn abandon_run(&mut self, bridge_id: &BridgeId) -> LiveResult<LiveState> {
        let sequence = self.next_state_sequence();
        let bridge = self.bridge_mut(bridge_id)?;
        let state = LiveState {
            sequence,
            phase: LivePhase::Menu,
            legal_actions: vec![request_state_action()],
            raw: json!({"screen": "menu", "abandoned": true}),
        };
        bridge.status.last_heartbeat_ms = Some(now_ms());
        bridge.state = state.clone();
        Ok(state)
    }

    fn request_state(&mut self, bridge_id: &BridgeId) -> LiveResult<LiveState> {
        let sequence = self.next_state_sequence();
        let bridge = self.bridge_mut(bridge_id)?;
        let mut state = bridge.state.clone();
        state.sequence = sequence;
        bridge.status.last_heartbeat_ms = Some(now_ms());
        bridge.state = state.clone();
        Ok(state)
    }

    fn send_action(&mut self, bridge_id: &BridgeId, action: &LegalAction) -> LiveResult<LiveState> {
        if !action.enabled {
            return Err(LiveError::InvalidAction(
                action
                    .disabled_reason
                    .clone()
                    .unwrap_or_else(|| "action disabled".to_owned()),
            ));
        }
        let sequence = self.next_state_sequence();
        let bridge = self.bridge_mut(bridge_id)?;
        let state = next_fake_state(sequence, action);
        bridge.status.last_heartbeat_ms = Some(now_ms());
        bridge.state = state.clone();
        Ok(state)
    }

    fn kill_bridge(&mut self, bridge_id: &BridgeId) -> LiveResult<()> {
        self.bridges
            .remove(bridge_id)
            .ok_or_else(|| LiveError::NotFound(format!("bridge {}", bridge_id.0)))?;
        Ok(())
    }

    fn kill_all(&mut self) -> LiveResult<usize> {
        let count = self.bridges.len();
        self.bridges.clear();
        Ok(count)
    }
}

fn next_fake_state(sequence: u64, action: &LegalAction) -> LiveState {
    match action.kind {
        LegalActionKind::ChooseNeow => LiveState {
            sequence,
            phase: LivePhase::Combat,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("strike-jaw-worm".to_owned()),
                    kind: LegalActionKind::PlayCard,
                    label: "Strike -> Jaw Worm".to_owned(),
                    enabled: true,
                    command: json!({"kind": "play_card", "card_instance_id": "strike-1", "target_id": "jaw-worm"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("end-turn".to_owned()),
                    kind: LegalActionKind::EndTurn,
                    label: "End turn".to_owned(),
                    enabled: true,
                    command: json!({"kind": "end_turn"}),
                    disabled_reason: None,
                },
                request_state_action(),
            ],
            raw: json!({"screen": "combat", "monster": "jaw_worm"}),
        },
        LegalActionKind::PlayCard | LegalActionKind::EndTurn => LiveState {
            sequence,
            phase: LivePhase::Combat,
            legal_actions: vec![request_state_action()],
            raw: json!({"screen": "combat", "last_action": action.command}),
        },
        _ => LiveState {
            sequence,
            phase: LivePhase::Unknown,
            legal_actions: vec![request_state_action()],
            raw: json!({"last_action": action.command}),
        },
    }
}

fn request_state_action() -> LegalAction {
    LegalAction {
        id: ActionId("request-state".to_owned()),
        kind: LegalActionKind::RequestState,
        label: "Request state".to_owned(),
        enabled: true,
        command: json!({"kind": "request_state"}),
        disabled_reason: None,
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RunSeed, SessionId, TraceRecord};

    #[test]
    fn fake_bridge_starts_and_advances_state() {
        let mut bridge = FakeBridgeManager::with_default_bridge();
        let bridge_id = BridgeId("fake-bridge-1".to_owned());
        let config = RunConfig {
            character: Character::Ironclad,
            ascension: 0,
            seed: RunSeed::External("CODEX04".to_owned()),
            profile: None,
        };

        let neow = bridge.start_run(&bridge_id, &config).unwrap();
        assert_eq!(neow.phase, LivePhase::Neow);
        assert!(neow.legal_actions.iter().any(|a| a.id.0 == "talk"));

        let combat = bridge
            .send_action(&bridge_id, &neow.legal_actions[0])
            .unwrap();
        assert_eq!(combat.phase, LivePhase::Combat);
        assert!(combat
            .legal_actions
            .iter()
            .any(|a| a.kind == LegalActionKind::PlayCard));
    }

    #[test]
    fn fake_bridge_kills_all_bridges() {
        let mut bridge = FakeBridgeManager::with_default_bridge();
        assert_eq!(bridge.kill_all().unwrap(), 1);
        assert!(bridge.list_bridges().unwrap().is_empty());
    }

    #[test]
    fn trace_record_bridge_ids_serialize() {
        let record = TraceRecord::Metadata {
            schema: 1,
            source: "test".to_owned(),
            session_id: SessionId("s".to_owned()),
            bridge_id: BridgeId("b".to_owned()),
            run_config: None,
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"bridge_id\""));
    }
}
