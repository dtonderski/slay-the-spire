use super::*;

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
