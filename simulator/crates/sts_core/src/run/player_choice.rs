use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    content::cards::{
        SECRET_TECHNIQUE_ID, SECRET_TECHNIQUE_PLUS_ID, SECRET_WEAPON_ID, SECRET_WEAPON_PLUS_ID,
    },
    CombatAction, RunAction, RunDecisionAction, RunPhase, RunState,
};

use super::legal_run_decision_actions;

/// Serialized schema version for [`PlayerChoiceSet`] and [`PlayerChoice`].
pub const PLAYER_CHOICE_SCHEMA_VERSION: u32 = 1;

/// Public, monotonically increasing token owned by the fair environment.
///
/// This value is deliberately independent of simulator state. In particular,
/// it must never be derived from a snapshot hash, RNG state, seed, or internal
/// instance ID. The environment advances it after every accepted choice and
/// supplies the current value to both enumeration and resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DecisionRevision(u64);

impl DecisionRevision {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns the next revision, or `None` if the public counter is exhausted.
    #[must_use]
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// A player-visible combat choice using only decision-local slots.
///
/// Declaration order and fields define the V1 canonical ordering. No variant
/// contains an authoritative card or monster instance ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlayerChoice {
    PlayHandSlot {
        hand_slot: u16,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_slot: Option<u16>,
    },
    EndTurn,
    UsePotionSlot {
        potion_slot: u16,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_slot: Option<u16>,
    },
    DiscardPotionSlot {
        potion_slot: u16,
    },
    ToggleVisibleCard {
        option_slot: u16,
    },
    ChooseVisibleOption {
        option_slot: u16,
    },
    ConfirmSelection,
    SkipSelection,
}

impl PlayerChoice {
    /// Canonical serialized kind names exposed to consumers that mirror this enum.
    pub const KIND_NAMES: [&'static str; 8] = [
        Self::PlayHandSlot {
            hand_slot: 0,
            target_slot: None,
        }
        .kind(),
        Self::EndTurn.kind(),
        Self::UsePotionSlot {
            potion_slot: 0,
            target_slot: None,
        }
        .kind(),
        Self::DiscardPotionSlot { potion_slot: 0 }.kind(),
        Self::ToggleVisibleCard { option_slot: 0 }.kind(),
        Self::ChooseVisibleOption { option_slot: 0 }.kind(),
        Self::ConfirmSelection.kind(),
        Self::SkipSelection.kind(),
    ];

    /// Return the canonical serialized kind for this choice.
    ///
    /// The exhaustive match forces a compiler error when a new enum variant is
    /// added without defining its public kind, while `KIND_NAMES` provides the
    /// ordered schema inventory for language bindings.
    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::PlayHandSlot { .. } => "play_hand_slot",
            Self::EndTurn => "end_turn",
            Self::UsePotionSlot { .. } => "use_potion_slot",
            Self::DiscardPotionSlot { .. } => "discard_potion_slot",
            Self::ToggleVisibleCard { .. } => "toggle_visible_card",
            Self::ChooseVisibleOption { .. } => "choose_visible_option",
            Self::ConfirmSelection => "confirm_selection",
            Self::SkipSelection => "skip_selection",
        }
    }
}

/// Atomic public legal-choice result for one combat decision boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerChoiceSet {
    pub schema_version: u32,
    pub decision_revision: DecisionRevision,
    pub choices: Vec<PlayerChoice>,
}

/// A public choice submitted against the revision on which it was observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerChoiceRequest {
    pub decision_revision: DecisionRevision,
    pub choice: PlayerChoice,
}

/// Stable errors exposed by the public choice boundary.
///
/// Internal validation details are intentionally collapsed into
/// `DecisionUnavailable` so malformed or unsupported authoritative state cannot
/// leak IDs or private mechanic details.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlayerChoiceError {
    NotInCombat,
    DecisionUnavailable,
    StaleDecision,
    InvalidChoice,
}

impl fmt::Display for PlayerChoiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NotInCombat => "public combat choices require combat phase",
            Self::DecisionUnavailable => "public combat decision is unavailable",
            Self::StaleDecision => "public combat decision is stale",
            Self::InvalidChoice => "public combat choice is invalid",
        };
        f.write_str(message)
    }
}

impl std::error::Error for PlayerChoiceError {}

/// Projects authoritative combat legality into canonical public choices.
///
/// The function is pure: it consumes no RNG and does not mutate `run`.
pub fn player_choices(
    run: &RunState,
    decision_revision: DecisionRevision,
) -> Result<PlayerChoiceSet, PlayerChoiceError> {
    let choices = projected_choices(run)?.into_keys().collect();
    Ok(PlayerChoiceSet {
        schema_version: PLAYER_CHOICE_SCHEMA_VERSION,
        decision_revision,
        choices,
    })
}

/// Resolves a public choice back to the existing authoritative action.
///
/// `current_revision` must be the fair environment's current public revision.
/// Revision comparison happens before state inspection, so stale submissions
/// always receive the same public error.
pub fn resolve_player_choice(
    run: &RunState,
    current_revision: DecisionRevision,
    request: PlayerChoiceRequest,
) -> Result<RunDecisionAction, PlayerChoiceError> {
    if request.decision_revision != current_revision {
        return Err(PlayerChoiceError::StaleDecision);
    }

    projected_choices(run)?
        .get(&request.choice)
        .copied()
        .ok_or(PlayerChoiceError::InvalidChoice)
}

fn projected_choices(
    run: &RunState,
) -> Result<BTreeMap<PlayerChoice, RunDecisionAction>, PlayerChoiceError> {
    if run.phase != RunPhase::Combat {
        return Err(PlayerChoiceError::NotInCombat);
    }

    let actions =
        legal_run_decision_actions(run).map_err(|_| PlayerChoiceError::DecisionUnavailable)?;
    let mut choices = BTreeMap::new();
    for action in actions {
        let Some(choice) = project_action(run, action)? else {
            continue;
        };
        if choices.insert(choice, action).is_some() {
            // Two distinct authoritative commands must never collapse into one
            // public descriptor: resolving such a choice would be ambiguous.
            return Err(PlayerChoiceError::DecisionUnavailable);
        }
    }
    Ok(choices)
}

fn project_action(
    run: &RunState,
    action: RunDecisionAction,
) -> Result<Option<PlayerChoice>, PlayerChoiceError> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(PlayerChoiceError::DecisionUnavailable)?;

    match action {
        RunDecisionAction::Combat(CombatAction::PlayCard { card_id, target }) => {
            let (hand_index, card) = combat
                .piles
                .hand
                .iter()
                .enumerate()
                .find(|(_, card)| card.id == card_id)
                .ok_or(PlayerChoiceError::DecisionUnavailable)?;
            if matches!(
                card.content_id,
                SECRET_TECHNIQUE_ID
                    | SECRET_TECHNIQUE_PLUS_ID
                    | SECRET_WEAPON_ID
                    | SECRET_WEAPON_PLUS_ID
            ) {
                // These authoritative plays are legal only when hidden draw-pile
                // composition satisfies their card-specific prerequisite. The
                // V1 fair boundary does not carry that public-knowledge contract,
                // so omit them rather than leaking the prerequisite through the
                // shape of the public choice set.
                return Ok(None);
            }
            let hand_slot = public_slot(hand_index)?;
            let target_slot = target
                .map(|monster_id| {
                    combat
                        .monsters
                        .iter()
                        .position(|monster| monster.id == monster_id)
                        .ok_or(PlayerChoiceError::DecisionUnavailable)
                        .and_then(public_slot)
                })
                .transpose()?;
            Ok(Some(PlayerChoice::PlayHandSlot {
                hand_slot,
                target_slot,
            }))
        }
        RunDecisionAction::Combat(CombatAction::EndTurn) => Ok(Some(PlayerChoice::EndTurn)),
        RunDecisionAction::Run(RunAction::UsePotion { slot, target }) => {
            let target_slot = target
                .map(|monster_id| {
                    combat
                        .monsters
                        .iter()
                        .position(|monster| monster.id == monster_id)
                        .ok_or(PlayerChoiceError::DecisionUnavailable)
                        .and_then(public_slot)
                })
                .transpose()?;
            Ok(Some(PlayerChoice::UsePotionSlot {
                potion_slot: public_slot(slot)?,
                target_slot,
            }))
        }
        RunDecisionAction::Run(RunAction::DiscardPotion { slot }) => {
            Ok(Some(PlayerChoice::DiscardPotionSlot {
                potion_slot: public_slot(slot)?,
            }))
        }
        RunDecisionAction::Run(
            RunAction::ChooseHandSelect { index }
            | RunAction::ChooseDrawSelect { index }
            | RunAction::ChooseDiscardSelect { index }
            | RunAction::ChooseExhaustSelect { index },
        ) => Ok(Some(PlayerChoice::ToggleVisibleCard {
            option_slot: public_slot(index)?,
        })),
        RunDecisionAction::Run(RunAction::ChooseCombatCardReward { index }) => {
            Ok(Some(PlayerChoice::ChooseVisibleOption {
                option_slot: public_slot(index)?,
            }))
        }
        RunDecisionAction::Run(
            RunAction::ConfirmHandSelect
            | RunAction::ConfirmDrawSelect
            | RunAction::ConfirmDiscardSelect
            | RunAction::ConfirmExhaustSelect,
        ) => Ok(Some(PlayerChoice::ConfirmSelection)),
        RunDecisionAction::Run(RunAction::SkipCombatCardReward) => {
            Ok(Some(PlayerChoice::SkipSelection))
        }
        _ => Err(PlayerChoiceError::DecisionUnavailable),
    }
}

fn public_slot(index: usize) -> Result<u16, PlayerChoiceError> {
    u16::try_from(index).map_err(|_| PlayerChoiceError::DecisionUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        combat::{
            CombatDecisionState, CombatPhase, CombatRngState, HandSelectPurpose, HandSelectState,
        },
        content::cards::{
            DEFEND_R_ID, HAVOC_ID, SECRET_TECHNIQUE_ID, SECRET_TECHNIQUE_PLUS_ID, SECRET_WEAPON_ID,
            SECRET_WEAPON_PLUS_ID, STRIKE_R_ID,
        },
        CardId, CardInstance, MonsterId, MonsterIntent, Potion, Relic,
    };

    const REVISION: DecisionRevision = DecisionRevision::new(7);

    #[test]
    fn projects_and_resolves_visible_slots_without_serializing_internal_ids() {
        let run = RunState::combat_fixture();
        let combat = run.combat.as_ref().expect("combat fixture");
        let strike_id = combat.piles.hand[0].id;
        let monster_id = combat.monsters[0].id;

        let decision = player_choices(&run, REVISION).expect("public combat choices");
        let strike = PlayerChoice::PlayHandSlot {
            hand_slot: 0,
            target_slot: Some(0),
        };
        assert!(decision.choices.contains(&strike));
        assert!(decision.choices.contains(&PlayerChoice::EndTurn));
        assert_eq!(
            resolve_player_choice(
                &run,
                REVISION,
                PlayerChoiceRequest {
                    decision_revision: REVISION,
                    choice: strike,
                },
            ),
            Ok(RunDecisionAction::Combat(CombatAction::PlayCard {
                card_id: strike_id,
                target: Some(monster_id),
            }))
        );

        let serialized = serde_json::to_string(&decision).expect("choice set serializes");
        assert!(!serialized.contains("card_id"));
        assert!(!serialized.contains("monster_id"));
        assert!(!serialized.contains(&format!("card:{}", strike_id.get())));
        assert!(!serialized.contains(&format!("monster:{}", monster_id.get())));
    }

    #[test]
    fn canonical_choices_ignore_hidden_draw_order_and_rng_state() {
        let mut first = RunState::combat_fixture();
        let combat = first.combat.as_mut().expect("combat fixture");
        combat.piles.hand[0].content_id = HAVOC_ID;
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(900), STRIKE_R_ID),
            CardInstance::new(CardId::new(901), DEFEND_R_ID),
        ];

        let mut second = first.clone();
        second
            .combat
            .as_mut()
            .expect("combat fixture")
            .piles
            .draw_pile
            .reverse();
        second.event_rng_seed = 123_456;
        second.reward_rng_seed = 654_321;
        second.combat.as_mut().expect("combat fixture").rng =
            CombatRngState::deterministic_fixture(999);

        let before_first = first.clone();
        let before_second = second.clone();
        let first_choices = player_choices(&first, REVISION).expect("first choices");
        let second_choices = player_choices(&second, REVISION).expect("second choices");

        assert_eq!(first_choices, second_choices);
        assert_eq!(
            serde_json::to_vec(&first_choices).expect("first choices serialize"),
            serde_json::to_vec(&second_choices).expect("second choices serialize")
        );
        assert_eq!(first, before_first);
        assert_eq!(second, before_second);
    }

    #[test]
    fn hidden_draw_composition_does_not_change_public_choice_shape() {
        for (card_id, eligible_draw_card, ineligible_draw_card) in [
            (SECRET_TECHNIQUE_ID, DEFEND_R_ID, STRIKE_R_ID),
            (SECRET_TECHNIQUE_PLUS_ID, DEFEND_R_ID, STRIKE_R_ID),
            (SECRET_WEAPON_ID, STRIKE_R_ID, DEFEND_R_ID),
            (SECRET_WEAPON_PLUS_ID, STRIKE_R_ID, DEFEND_R_ID),
        ] {
            let mut eligible = RunState::combat_fixture();
            let mut ineligible = eligible.clone();
            for run in [&mut eligible, &mut ineligible] {
                let combat = run.combat.as_mut().expect("combat fixture");
                combat.piles.hand[0].content_id = card_id;
            }
            eligible
                .combat
                .as_mut()
                .expect("combat fixture")
                .piles
                .draw_pile[0]
                .content_id = eligible_draw_card;
            ineligible
                .combat
                .as_mut()
                .expect("combat fixture")
                .piles
                .draw_pile[0]
                .content_id = ineligible_draw_card;

            let eligible_choices = player_choices(&eligible, REVISION).expect("eligible choices");
            let ineligible_choices =
                player_choices(&ineligible, REVISION).expect("ineligible choices");
            assert_eq!(eligible_choices, ineligible_choices);
            assert_eq!(
                serde_json::to_vec(&eligible_choices).expect("eligible choices serialize"),
                serde_json::to_vec(&ineligible_choices).expect("ineligible choices serialize")
            );
            let secret_action = RunDecisionAction::Combat(CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            });
            assert!(legal_run_decision_actions(&eligible)
                .expect("eligible authoritative choices")
                .contains(&secret_action));
            assert!(!legal_run_decision_actions(&ineligible)
                .expect("ineligible authoritative choices")
                .contains(&secret_action));
            assert!(!eligible_choices.choices.iter().any(|choice| {
                matches!(
                    choice,
                    PlayerChoice::PlayHandSlot {
                        hand_slot: 0,
                        target_slot: None,
                    }
                )
            }));
            let request = PlayerChoiceRequest {
                decision_revision: REVISION,
                choice: PlayerChoice::PlayHandSlot {
                    hand_slot: 0,
                    target_slot: None,
                },
            };
            assert_eq!(
                resolve_player_choice(&eligible, REVISION, request),
                Err(PlayerChoiceError::InvalidChoice)
            );
            assert_eq!(
                resolve_player_choice(&ineligible, REVISION, request),
                Err(PlayerChoiceError::InvalidChoice)
            );
        }
    }

    #[test]
    fn hidden_intent_does_not_change_choices_or_public_errors() {
        let first = RunState::combat_fixture_with_relics(vec![Relic::RunicDome]);
        let mut second = first.clone();
        second.combat.as_mut().expect("combat fixture").monsters[0].intent =
            MonsterIntent::Block { block: 99 };

        assert_eq!(
            player_choices(&first, REVISION),
            player_choices(&second, REVISION)
        );

        let invalid = PlayerChoiceRequest {
            decision_revision: REVISION,
            choice: PlayerChoice::UsePotionSlot {
                potion_slot: u16::MAX,
                target_slot: Some(u16::MAX),
            },
        };
        assert_eq!(
            resolve_player_choice(&first, REVISION, invalid),
            resolve_player_choice(&second, REVISION, invalid)
        );
        assert_eq!(
            resolve_player_choice(&first, REVISION, invalid),
            Err(PlayerChoiceError::InvalidChoice)
        );
    }

    #[test]
    fn internal_id_renumbering_does_not_change_public_choices_or_order() {
        let first = RunState::combat_fixture();
        let mut second = first.clone();
        let combat = second.combat.as_mut().expect("combat fixture");
        for card in combat
            .piles
            .hand
            .iter_mut()
            .chain(combat.piles.draw_pile.iter_mut())
            .chain(combat.piles.discard_pile.iter_mut())
            .chain(combat.piles.exhaust_pile.iter_mut())
            .chain(combat.piles.limbo.iter_mut())
        {
            card.id = CardId::new(card.id.get() + 10_000);
        }
        for monster in &mut combat.monsters {
            monster.id = MonsterId::new(monster.id.get() + 10_000);
        }

        let first_choices = player_choices(&first, REVISION).expect("first choices");
        let second_choices = player_choices(&second, REVISION).expect("renumbered choices");
        assert_eq!(first_choices, second_choices);
        assert_eq!(
            serde_json::to_vec(&first_choices).expect("first choices serialize"),
            serde_json::to_vec(&second_choices).expect("second choices serialize")
        );
    }

    #[test]
    fn stale_and_invalid_requests_have_stable_public_errors() {
        let mut hidden_variant = RunState::combat_fixture();
        hidden_variant.event_rng_seed = 77;
        hidden_variant.combat.as_mut().expect("combat fixture").rng =
            CombatRngState::deterministic_fixture(88);

        let stale = PlayerChoiceRequest {
            decision_revision: DecisionRevision::new(6),
            choice: PlayerChoice::EndTurn,
        };
        assert_eq!(
            resolve_player_choice(&RunState::combat_fixture(), REVISION, stale),
            Err(PlayerChoiceError::StaleDecision)
        );
        assert_eq!(
            resolve_player_choice(&hidden_variant, REVISION, stale),
            Err(PlayerChoiceError::StaleDecision)
        );

        let invalid = PlayerChoiceRequest {
            decision_revision: REVISION,
            choice: PlayerChoice::PlayHandSlot {
                hand_slot: u16::MAX,
                target_slot: Some(u16::MAX),
            },
        };
        assert_eq!(
            resolve_player_choice(&RunState::combat_fixture(), REVISION, invalid),
            Err(PlayerChoiceError::InvalidChoice)
        );
        assert_eq!(
            resolve_player_choice(&hidden_variant, REVISION, invalid),
            Err(PlayerChoiceError::InvalidChoice)
        );
    }

    #[test]
    fn potion_and_selection_actions_use_public_slots() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::Energy, Potion::Fire];
        run.empty_potion_slots = vec![2];

        let choices = player_choices(&run, REVISION)
            .expect("potion choices")
            .choices;
        assert!(choices.contains(&PlayerChoice::UsePotionSlot {
            potion_slot: 0,
            target_slot: None,
        }));
        assert!(choices.contains(&PlayerChoice::UsePotionSlot {
            potion_slot: 1,
            target_slot: Some(0),
        }));
        assert!(choices.contains(&PlayerChoice::DiscardPotionSlot { potion_slot: 0 }));
        assert!(choices.contains(&PlayerChoice::DiscardPotionSlot { potion_slot: 1 }));
        assert_all_choices_resolve_to_authoritative_legality(&run, &choices);

        let combat = run.combat.as_mut().expect("combat fixture");
        let source_card_id = combat.piles.hand[0].id;
        combat.decision = Some(CombatDecisionState::HandSelect {
            state: HandSelectState {
                purpose: HandSelectPurpose::WarcryPutOnDraw,
                source_card_id,
                selected_hand_index: None,
                selected_hand_indices: Vec::new(),
                dual_wield_restore_on_confirm: Vec::new(),
                dual_wield_force_exhaust: false,
            },
            pending_actions: Default::default(),
        });
        let choices = player_choices(&run, REVISION)
            .expect("selection choices")
            .choices;
        assert!(choices.contains(&PlayerChoice::ToggleVisibleCard { option_slot: 0 }));
        assert!(!choices.contains(&PlayerChoice::EndTurn));

        let selected = super::super::apply_run_decision_action(
            &run,
            RunDecisionAction::Run(RunAction::ChooseHandSelect { index: 0 }),
        )
        .expect("hand selection applies");
        assert!(player_choices(&selected, REVISION)
            .expect("selected hand choices")
            .choices
            .contains(&PlayerChoice::ConfirmSelection));
    }

    #[test]
    fn visible_combat_reward_options_and_skip_are_projected() {
        let mut run = RunState::combat_fixture();
        run.combat.as_mut().expect("combat fixture").decision =
            Some(CombatDecisionState::PotionCardReward {
                choices: vec![
                    CardInstance::new(CardId::new(900), STRIKE_R_ID),
                    CardInstance::new(CardId::new(901), DEFEND_R_ID),
                ],
                reward_kind: crate::combat::PotionCardRewardKind::Attack,
            });

        let choices = player_choices(&run, REVISION)
            .expect("reward choices")
            .choices;
        assert!(choices.contains(&PlayerChoice::ChooseVisibleOption { option_slot: 0 }));
        assert!(choices.contains(&PlayerChoice::ChooseVisibleOption { option_slot: 1 }));
        assert!(choices.contains(&PlayerChoice::SkipSelection));
        assert_all_choices_resolve_to_authoritative_legality(&run, &choices);
    }

    #[test]
    fn non_combat_and_malformed_states_collapse_to_public_errors() {
        assert_eq!(
            player_choices(&RunState::map_fixture(), REVISION),
            Err(PlayerChoiceError::NotInCombat)
        );

        let mut malformed = RunState::combat_fixture();
        let combat = malformed.combat.as_mut().expect("combat fixture");
        combat.piles.draw_pile[0].id = combat.piles.hand[0].id;
        assert_eq!(
            player_choices(&malformed, REVISION),
            Err(PlayerChoiceError::DecisionUnavailable)
        );
    }

    #[test]
    fn non_player_combat_phases_have_no_public_potion_choices() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::Energy];
        run.empty_potion_slots = vec![1, 2];
        run.combat.as_mut().expect("combat fixture").phase = CombatPhase::MonsterTurn;

        let decision = player_choices(&run, REVISION).expect("public combat choices");
        assert!(decision.choices.is_empty());
    }

    #[test]
    fn revision_is_explicit_monotonic_public_data() {
        assert_eq!(REVISION.get(), 7);
        assert_eq!(REVISION.checked_next(), Some(DecisionRevision::new(8)));
        assert_eq!(DecisionRevision::new(u64::MAX).checked_next(), None);
    }

    fn assert_all_choices_resolve_to_authoritative_legality(
        run: &RunState,
        choices: &[PlayerChoice],
    ) {
        let authoritative =
            legal_run_decision_actions(run).expect("authoritative legal actions enumerate");
        for choice in choices {
            let resolved = resolve_player_choice(
                run,
                REVISION,
                PlayerChoiceRequest {
                    decision_revision: REVISION,
                    choice: *choice,
                },
            )
            .expect("public choice resolves");
            assert!(
                authoritative.contains(&resolved),
                "resolved action {resolved:?} must be authoritative legality"
            );
        }
    }
}
