use super::*;

pub(super) fn bind_pending_room_resolution<'a>(
    state: &'a LiveState,
    step: &SlayTheDataPreflightStep,
) -> Result<&'a LegalAction, String> {
    if state
        .raw
        .pointer("/summary/screen_name")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|screen| screen.eq_ignore_ascii_case("FTUE"))
    {
        return bind_matching_live_action(state, "CLICK LEFT 1080 700 250", |action| {
            action.kind == LegalActionKind::Confirm
                && action.label.eq_ignore_ascii_case("Dismiss tutorial")
        });
    }
    if state.phase == LivePhase::Map {
        if let Some(symbol) = route_symbol_from_step(step) {
            if symbol.eq_ignore_ascii_case("B") {
                let boss_actions = state
                    .legal_actions
                    .iter()
                    .filter(|action| {
                        action.enabled
                            && action.kind == LegalActionKind::ChooseMapNode
                            && action.label.eq_ignore_ascii_case("boss")
                    })
                    .collect::<Vec<_>>();
                return match boss_actions.as_slice() {
                    [action] => Ok(action),
                    [] => Err("pending boss room has no enabled live boss action".to_owned()),
                    _ => Err("pending boss room has multiple enabled live boss actions".to_owned()),
                };
            }
            let matches = state
                .legal_actions
                .iter()
                .filter(|action| action.enabled && action.kind == LegalActionKind::ChooseMapNode)
                .filter(|action| map_action_matches_symbol(state, action, symbol))
                .collect::<Vec<_>>();
            if let Some(action) = matches.into_iter().next() {
                return Ok(action);
            }
            return Err(format!(
                "pending room resolution route symbol {symbol:?} has no live map match"
            ));
        }
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| action.enabled && action.kind == LegalActionKind::ChooseMapNode)
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err("pending room resolution has no live map choices".to_owned()),
            _ => Err("pending room resolution has multiple live map choices".to_owned()),
        };
    }
    if state.phase == LivePhase::Event {
        if current_event_name(state).is_some_and(|name| name == "Golden Idol") {
            if let Some(action) = unique_event_choice_by_label(state, "outrun") {
                return Ok(action);
            }
        }
        if let Some(action) = unique_event_choice_by_label(state, "continue") {
            return Ok(action);
        }
        if let Some(action) = unique_event_choice_by_label(state, "play") {
            return Ok(action);
        }
        if let Some(action) = unique_event_choice_by_label(state, "spin") {
            return Ok(action);
        }
        if current_event_name(state).is_some_and(|name| name == "Wheel of Change") {
            if let Some(action) = unique_enabled_event_choice(state) {
                return Ok(action);
            }
        }
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| {
                action.enabled
                    && action.kind == LegalActionKind::EventChoice
                    && action.label.eq_ignore_ascii_case("leave")
            })
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err("pending room resolution has no live event leave choice".to_owned()),
            _ => Err("pending room resolution has multiple live event leave choices".to_owned()),
        };
    }
    if state.phase == LivePhase::Neow {
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| {
                action.enabled
                    && action.kind == LegalActionKind::ChooseNeow
                    && action.label.eq_ignore_ascii_case("leave")
            })
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err("pending room resolution has no live Neow leave choice".to_owned()),
            _ => Err("pending room resolution has multiple live Neow leave choices".to_owned()),
        };
    }
    if state.phase == LivePhase::Reward && is_grid_screen(state) && grid_confirm_up(state) {
        return bind_matching_live_action(state, "CONFIRM", |action| {
            action.kind == LegalActionKind::Confirm && action.label.eq_ignore_ascii_case("confirm")
        });
    }
    if state.phase == LivePhase::Reward {
        return reward_flush_action_before_high_level_step(state, "pending room resolution");
    }
    if state.phase == LivePhase::Rest {
        return bind_matching_live_action(state, "PROCEED", |action| {
            action.kind == LegalActionKind::Confirm && action.label.eq_ignore_ascii_case("proceed")
        });
    }
    if live_screen_type(state).is_some_and(|screen| screen == "CHEST") {
        if let Ok(action) = bind_matching_live_action(state, "CHOOSE 0", |action| {
            action.kind == LegalActionKind::Confirm && action.label.eq_ignore_ascii_case("open")
        }) {
            return Ok(action);
        }
        return bind_matching_live_action(state, "PROCEED", |action| {
            action.kind == LegalActionKind::Confirm && action.label.eq_ignore_ascii_case("proceed")
        });
    }
    if state.phase == LivePhase::Shop
        || live_screen_type(state).is_some_and(|screen| screen == "SHOP_SCREEN")
    {
        return unique_leave_shop_action(state).ok_or_else(|| {
            "pending room resolution has no unique live shop leave choice".to_owned()
        });
    }
    if live_screen_type(state).is_some_and(|screen| screen == "SHOP_ROOM") {
        return bind_matching_live_action(state, "PROCEED", |action| {
            action.kind == LegalActionKind::Confirm && action.label.eq_ignore_ascii_case("proceed")
        });
    }
    Err("SlayTheData guided step pending_room_resolution has no dynamic binding".to_owned())
}

pub(super) fn bind_dynamic_card_reward_step<'a>(
    state: &'a LiveState,
    step: &SlayTheDataPreflightStep,
    intent: &SlayTheDataReplayStepKind,
) -> Result<&'a LegalAction, String> {
    let card_reward = match intent {
        SlayTheDataReplayStepKind::CardReward { picked, skipped } => Some((picked, *skipped)),
        _ => None,
    };
    if card_reward.is_some_and(|(_, skipped)| skipped) && state.phase == LivePhase::Reward {
        return reward_flush_action_before_high_level_step(state, "pending skipped card reward");
    }
    if state.phase == LivePhase::Reward {
        let Some(target) = card_reward
            .and_then(|(picked, _)| picked.as_ref())
            .map(|card| card.raw.as_str())
        else {
            return Err("pending card reward has no concrete SlayTheData pick".to_owned());
        };
        if is_card_reward_screen(state) {
            if grid_confirm_up(state) {
                return bind_matching_live_action(state, "CONFIRM", |action| {
                    action.kind == LegalActionKind::Confirm
                        && action.label.eq_ignore_ascii_case("confirm")
                });
            }
            return first_card_label_match(state, target).ok_or_else(|| {
                format!("pending card reward target {target:?} has no live grid label match")
            });
        }
        if let Some(action) = reward_choice_by_label(state, "card") {
            return Ok(action);
        }
        return reward_flush_action_before_high_level_step(state, "pending card reward");
    }
    Err(format!(
        "SlayTheData guided step {} has no dynamic binding",
        step.code
    ))
}

pub(super) fn bind_neow_step<'a>(
    state: &'a LiveState,
    step: &SlayTheDataPreflightStep,
) -> Result<&'a LegalAction, String> {
    if step.code == "pending_neow_followup" && is_grid_screen(state) {
        return bind_neow_followup_grid_action(state);
    }
    if step.code == "pending_neow_followup" && state.phase == LivePhase::Reward {
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| action.enabled && action.kind == LegalActionKind::ChooseReward)
            .collect::<Vec<_>>();
        if let Some(action) = matches.into_iter().next() {
            return Ok(action);
        }
        return reward_flush_action_before_high_level_step(state, "pending Neow follow-up");
    }
    if step.code == "pending_neow_followup" && state.phase == LivePhase::Neow {
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| {
                action.enabled
                    && action.kind == LegalActionKind::ChooseNeow
                    && action.label.eq_ignore_ascii_case("leave")
            })
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err("pending Neow follow-up has no live Neow leave choice".to_owned()),
            _ => Err("pending Neow follow-up has multiple live Neow leave choices".to_owned()),
        };
    }
    if step.code == "legal_neow_leave" && state.phase == LivePhase::Reward {
        if let Some(action) = first_enabled_reward_choice(state) {
            return Ok(action);
        }
        return reward_flush_action_before_high_level_step(state, "legal Neow leave");
    }
    Err(format!(
        "SlayTheData guided step {} has no dynamic binding",
        step.code
    ))
}

pub(super) fn bind_guided_event_step<'a>(
    state: &'a LiveState,
    step: &SlayTheDataPreflightStep,
    intent: &SlayTheDataReplayStepKind,
) -> Result<&'a LegalAction, String> {
    if is_grid_screen(state) {
        if grid_confirm_up(state) {
            return bind_matching_live_action(state, "CONFIRM", |action| {
                action.kind == LegalActionKind::Confirm
                    && action.label.eq_ignore_ascii_case("confirm")
            });
        }
        let targets = guided_event_grid_targets(intent);
        if targets.is_empty() {
            return Err("guided event grid has no target card".to_owned());
        }
        let selected_count = grid_selected_card_count(state);
        if let Some(action) = targets
            .iter()
            .skip(selected_count)
            .chain(targets.iter().take(selected_count))
            .find_map(|target| first_card_label_match(state, target))
        {
            return Ok(action);
        }
        if drug_dealer_test_subject_step(intent) {
            // SlayTheData records the two transformed outputs, not the two
            // source cards removed from the deck. Prefer expendable starter
            // cards while the two-card selection grid remains open. Selected
            // cards remain clickable in CommunicationMod, so advance through
            // the ordered candidates by the selected-card count instead of
            // toggling the first card on and off.
            let selected_count = grid_selected_card_count(state);
            let starter_choices = state
                .legal_actions
                .iter()
                .filter(|action| {
                    action.enabled
                        && action.kind == LegalActionKind::ChooseReward
                        && ["strike", "defend"]
                            .into_iter()
                            .any(|target| campfire_grid_label_matches_target(&action.label, target))
                })
                .collect::<Vec<_>>();
            let all_choices = state
                .legal_actions
                .iter()
                .filter(|action| action.enabled && action.kind == LegalActionKind::ChooseReward)
                .collect::<Vec<_>>();
            if let Some(action) = starter_choices
                .get(selected_count)
                .or_else(|| all_choices.get(selected_count))
                .copied()
            {
                return Ok(action);
            }
        }
        return Err(format!(
            "guided event grid targets {targets:?} have no enabled live grid label match"
        ));
    }
    if state.phase == LivePhase::Reward {
        return reward_flush_action_before_high_level_step(state, "guided event choice");
    }
    if state.phase == LivePhase::Event {
        if let Some(action) = unique_event_choice_by_label(state, "continue") {
            return Ok(action);
        }
        // Wheel of Change has no player-selected outcome: RNG has already
        // chosen the recorded result, and each stage exposes exactly one
        // button (Play, spin, prize!, then Leave). Bind that sole action even
        // though SlayTheData names the result rather than the button label.
        if current_event_name(state).is_some_and(|name| name == "Wheel of Change") {
            if let Some(action) = unique_enabled_event_choice(state) {
                return Ok(action);
            }
        }
        let enabled_event_choices = state
            .legal_actions
            .iter()
            .filter(|action| action.enabled && action.kind == LegalActionKind::EventChoice)
            .collect::<Vec<_>>();
        if let [action] = enabled_event_choices.as_slice() {
            if action.label.eq_ignore_ascii_case("leave") {
                return Ok(action);
            }
        }
        if current_event_name(state).is_some_and(|name| name == "Golden Shrine") {
            if let Some(action) = unique_event_choice_by_label(state, "leave") {
                return Ok(action);
            }
        }
        if current_event_name(state).is_some_and(|name| name == "Big Fish") {
            if let Some(action) = unique_event_choice_by_label(state, "leave") {
                return Ok(action);
            }
        }
        let event_intent = match intent {
            SlayTheDataReplayStepKind::EventChoice {
                event_name,
                player_choice,
                relics_lost,
                ..
            } => Some((
                event_name.as_deref(),
                player_choice.as_deref(),
                relics_lost.as_slice(),
            )),
            _ => None,
        };
        if event_intent
            .and_then(|(event_name, _, _)| event_name)
            .is_some_and(|event_name| event_name == "Match and Keep!")
        {
            if let Some(action) = unique_event_choice_by_label(state, "leave") {
                return Ok(action);
            }
            if let Some(action) = unique_event_choice_by_label(state, "play") {
                return Ok(action);
            }
            if let Some(action) = bind_match_and_keep_action(state, intent)? {
                return Ok(action);
            }
        }
        let Some(choice) = event_intent.and_then(|(_, choice, _)| choice) else {
            return Err("guided event choice has no concrete SlayTheData choice".to_owned());
        };
        if current_event_name(state).is_some_and(|name| name == "Golden Idol") {
            if let Some(action) = unique_event_choice_by_label(state, "take") {
                return Ok(action);
            }
        }
        if normalize_live_label(current_event_name(state).unwrap_or_default()).replace(' ', "")
            == "nloth"
            && normalize_live_label(choice).replace(' ', "") == "tradedrelic"
        {
            let Some(relic) = event_intent
                .and_then(|(_, _, relics_lost)| relics_lost.first())
                .map(String::as_str)
            else {
                return Err("N'loth trade has no recorded lost relic".to_owned());
            };
            let target = normalize_live_label(relic).replace(' ', "");
            let matches = state
                .legal_actions
                .iter()
                .filter(|action| {
                    action.enabled
                        && action.kind == LegalActionKind::EventChoice
                        && normalize_live_label(&action.label)
                            .replace(' ', "")
                            .contains(&target)
                })
                .collect::<Vec<_>>();
            return match matches.as_slice() {
                [action] => Ok(action),
                [] => Err(format!(
                    "N'loth trade has no live action for lost relic {relic:?}"
                )),
                _ => Err(format!(
                    "N'loth trade has multiple live actions for lost relic {relic:?}"
                )),
            };
        }
        let event_name = current_event_name(state)
            .filter(|name| !name.trim().is_empty())
            .or_else(|| event_intent.and_then(|(event_name, _, _)| event_name))
            .unwrap_or_default();
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| action.enabled && action.kind == LegalActionKind::EventChoice)
            .filter(|action| {
                event_label_matches_choice_for_event(event_name, &action.label, choice)
            })
            .collect::<Vec<_>>();
        if matches.len() > 1 {
            if let Some(preferred) = preferred_event_label(event_name, choice) {
                let preferred_matches = matches
                    .iter()
                    .copied()
                    .filter(|action| {
                        normalize_live_label(&action.label).replace(' ', "") == preferred
                    })
                    .collect::<Vec<_>>();
                if let [action] = preferred_matches.as_slice() {
                    return Ok(action);
                }
            }
        }
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err(format!(
                "guided event choice {choice:?} has no live label match"
            )),
            _ => Err(format!(
                "guided event choice {choice:?} matched multiple live actions"
            )),
        };
    }
    Err(format!(
        "SlayTheData guided step {} has no dynamic binding",
        step.code
    ))
}

pub(super) fn bind_guided_shop_step<'a>(
    state: &'a LiveState,
    step: &SlayTheDataPreflightStep,
    intent: &SlayTheDataReplayStepKind,
) -> Result<&'a LegalAction, String> {
    match step.code.as_str() {
        "guided_shop_purchase" => bind_guided_shop_purchase(state, intent),
        "guided_shop_purge" => bind_guided_shop_purge(state, intent),
        _ => Err(format!(
            "SlayTheData step {} is not a guided shop step",
            step.code
        )),
    }
}

pub(super) fn bind_guided_campfire_step<'a>(
    state: &'a LiveState,
    intent: &SlayTheDataReplayStepKind,
) -> Result<&'a LegalAction, String> {
    if state.phase == LivePhase::Reward && !is_grid_screen(state) {
        return reward_flush_action_before_high_level_step(state, "guided campfire");
    }
    if state.phase == LivePhase::Rest
        && !state
            .legal_actions
            .iter()
            .any(|action| action.enabled && action.kind == LegalActionKind::RestSite)
    {
        let proceed = state
            .legal_actions
            .iter()
            .filter(|action| {
                action.enabled
                    && action.kind == LegalActionKind::Confirm
                    && action.label.eq_ignore_ascii_case("proceed")
            })
            .collect::<Vec<_>>();
        if !proceed.is_empty() {
            return match proceed.as_slice() {
                [action] => Ok(action),
                _ => Err("guided campfire matched multiple live Proceed actions".to_owned()),
            };
        }
    }
    let SlayTheDataReplayStepKind::Campfire { key, target_card } = intent else {
        return Err("guided campfire has no typed campfire intent".to_owned());
    };
    let Some(key) = key.as_deref() else {
        return Err("guided campfire has no concrete SlayTheData key".to_owned());
    };
    if state.phase == LivePhase::Rest {
        if should_override_campfire_with_rest(state) {
            let rest = state
                .legal_actions
                .iter()
                .filter(|action| action.enabled && action.kind == LegalActionKind::RestSite)
                .filter(|action| action.label.eq_ignore_ascii_case("rest"))
                .collect::<Vec<_>>();
            return match rest.as_slice() {
                [action] => Ok(action),
                [] => Err("low-HP campfire override has no enabled Rest action".to_owned()),
                _ => Err("low-HP campfire override matched multiple Rest actions".to_owned()),
            };
        }
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| action.enabled && action.kind == LegalActionKind::RestSite)
            .filter(|action| campfire_label_matches_key(&action.label, key))
            .collect::<Vec<_>>();
        if matches.is_empty()
            && key.eq_ignore_ascii_case("SMITH")
            && live_state_has_relic(state, "Fusion Hammer")
        {
            let rest = state
                .legal_actions
                .iter()
                .filter(|action| action.enabled && action.kind == LegalActionKind::RestSite)
                .filter(|action| action.label.eq_ignore_ascii_case("rest"))
                .collect::<Vec<_>>();
            return match rest.as_slice() {
                [action] => Ok(action),
                [] => Err("Fusion Hammer fallback has no enabled Rest action".to_owned()),
                _ => Err("Fusion Hammer fallback matched multiple Rest actions".to_owned()),
            };
        }
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err(format!(
                "guided campfire key {key:?} has no live rest label match"
            )),
            _ => Err(format!(
                "guided campfire key {key:?} matched multiple live rest actions"
            )),
        };
    }
    if is_grid_screen(state) {
        if grid_confirm_up(state) {
            return bind_matching_live_action(state, "CONFIRM", |action| {
                action.kind == LegalActionKind::Confirm
                    && action.label.eq_ignore_ascii_case("confirm")
            });
        }
        let Some(target) = target_card.as_ref().map(|card| card.raw.as_str()) else {
            return Err("guided campfire grid has no target card".to_owned());
        };
        return first_card_label_match(state, target).ok_or_else(|| {
            format!("guided campfire target {target:?} has no live grid label match")
        });
    }
    Err("SlayTheData guided step guided_campfire has no dynamic binding".to_owned())
}

fn bind_guided_shop_purchase<'a>(
    state: &'a LiveState,
    intent: &SlayTheDataReplayStepKind,
) -> Result<&'a LegalAction, String> {
    let SlayTheDataReplayStepKind::ShopPurchase { item: purchase, .. } = intent else {
        return Err("guided shop purchase has no concrete SlayTheData item".to_owned());
    };
    if state.phase == LivePhase::Map {
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| action.enabled && action.kind == LegalActionKind::ChooseMapNode)
            .filter(|action| map_action_matches_symbol(state, action, "$"))
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err("guided shop purchase has no live shop map node".to_owned()),
            _ => Err("guided shop purchase matched multiple live shop map nodes".to_owned()),
        };
    }
    if state.phase == LivePhase::Reward {
        return reward_flush_action_before_high_level_step(state, "guided shop purchase");
    }
    if live_screen_type(state).is_some_and(|screen| screen == "SHOP_ROOM") {
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| {
                action.enabled
                    && action.kind == LegalActionKind::Confirm
                    && action.label.eq_ignore_ascii_case("shop")
            })
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err("guided shop purchase has no live shop entry action".to_owned()),
            _ => Err("guided shop purchase matched multiple shop entry actions".to_owned()),
        };
    }
    if state.phase == LivePhase::Shop {
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| action.enabled && action.kind == LegalActionKind::ShopBuy)
            .filter(|action| shop_label_matches_purchase(&action.label, purchase))
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err(format!(
                "guided shop purchase {purchase:?} has no enabled live shop label match"
            )),
            _ => Err(format!(
                "guided shop purchase {purchase:?} matched multiple live shop actions"
            )),
        };
    }
    Err("SlayTheData guided step guided_shop_purchase has no dynamic binding".to_owned())
}

fn bind_guided_shop_purge<'a>(
    state: &'a LiveState,
    intent: &SlayTheDataReplayStepKind,
) -> Result<&'a LegalAction, String> {
    let SlayTheDataReplayStepKind::ShopPurge { card } = intent else {
        return Err("guided shop purge has no concrete SlayTheData target".to_owned());
    };
    let target = card.raw.as_str();
    if state.phase == LivePhase::Map {
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| action.enabled && action.kind == LegalActionKind::ChooseMapNode)
            .filter(|action| map_action_matches_symbol(state, action, "$"))
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err("guided shop purge has no live shop map node".to_owned()),
            _ => Err("guided shop purge matched multiple live shop map nodes".to_owned()),
        };
    }
    if live_screen_type(state).is_some_and(|screen| screen == "SHOP_ROOM") {
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| {
                action.enabled
                    && action.kind == LegalActionKind::Confirm
                    && action.label.eq_ignore_ascii_case("shop")
            })
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err("guided shop purge has no live shop entry action".to_owned()),
            _ => Err("guided shop purge matched multiple shop entry actions".to_owned()),
        };
    }
    if is_grid_screen(state) {
        if grid_confirm_up(state) {
            return bind_matching_live_action(state, "CONFIRM", |action| {
                action.kind == LegalActionKind::Confirm
                    && action.label.eq_ignore_ascii_case("confirm")
            });
        }
        return first_card_label_match(state, target).ok_or_else(|| {
            format!("guided shop purge target {target:?} has no live grid label match")
        });
    }
    if state.phase == LivePhase::Reward {
        return reward_flush_action_before_high_level_step(state, "guided shop purge");
    }
    if state.phase == LivePhase::Shop {
        return state
            .legal_actions
            .iter()
            .find(|action| {
                action.enabled
                    && action.kind == LegalActionKind::ShopBuy
                    && action.label.eq_ignore_ascii_case("purge")
            })
            .ok_or_else(|| "guided shop purge has no enabled live purge action".to_owned());
    }
    Err("SlayTheData guided step guided_shop_purge has no dynamic binding".to_owned())
}
