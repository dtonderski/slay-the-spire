use crate::{
    card::{CardInstance, CardType},
    content::cards::{get_card_definition, BLOOD_FOR_BLOOD_ID, BLOOD_FOR_BLOOD_PLUS_ID},
    SimError, SimResult,
};

pub(crate) fn validate_combat_card_cost_metadata(card: &CardInstance) -> SimResult<()> {
    if card.temp_cost_turn_only && card.temp_cost.is_none() {
        return Err(SimError::InvalidState(
            "turn-only card cost flag has no temporary cost",
        ));
    }
    if card.combat_cost_under_turn_override.is_some() && !card.temp_cost_turn_only {
        return Err(SimError::InvalidState(
            "hidden combat cost has no turn-only override",
        ));
    }
    if card.blood_for_blood_cost_reduction < 0 {
        return Err(SimError::InvalidState(
            "Blood for Blood cost reduction cannot be negative",
        ));
    }
    if card.blood_for_blood_cost_reduction != 0
        && card.content_id != BLOOD_FOR_BLOOD_ID
        && card.content_id != BLOOD_FOR_BLOOD_PLUS_ID
    {
        return Err(SimError::InvalidState(
            "non-Blood-for-Blood card carries cost-reduction metadata",
        ));
    }
    Ok(())
}

pub(crate) fn printed_card_cost(card: &CardInstance) -> SimResult<i32> {
    validate_combat_card_cost_metadata(card)?;
    // MadnessAction / Confusion write AbstractCard.cost as well as costForTurn.
    // A combat-long temp_cost is that written cost (FIDL01609 second Madness).
    if let Some(cost) = card.temp_cost {
        if !card.temp_cost_turn_only {
            return Ok(i32::from(cost));
        }
    }
    if let Some(cost) = card.combat_cost_under_turn_override {
        return Ok(i32::from(cost));
    }
    get_card_definition(card.content_id)
        .map(|definition| {
            // Recursion / Crescendo upgradeBaseCost(0). Synthetic plus cards keep
            // the base content id and only increment upgrades.
            if card.upgrades > 0
                && matches!(
                    card.content_id,
                    crate::content::cards::RECURSION_ANY_COLOR_ID
                        | crate::content::cards::CRESCENDO_ANY_COLOR_ID
                )
            {
                0
            } else if card.upgrades > 0
                && card.content_id == crate::content::cards::CREATIVE_AI_ANY_COLOR_ID
            {
                // CreativeAI.upgradeBaseCost(2)
                2
            } else {
                i32::from(definition.cost)
            }
        })
        .ok_or(SimError::UnknownContent(card.content_id))
}

pub(crate) fn set_card_cost_for_turn(card: &mut CardInstance, cost: u8) -> SimResult<()> {
    validate_combat_card_cost_metadata(card)?;
    if !card.temp_cost_turn_only {
        card.combat_cost_under_turn_override = card.temp_cost;
    }
    card.temp_cost = Some(cost);
    card.temp_cost_turn_only = true;
    Ok(())
}

pub(crate) fn set_randomized_combat_cost_if_changed(
    card: &mut CardInstance,
    cost: u8,
) -> SimResult<()> {
    if printed_card_cost(card)? != i32::from(cost) {
        card.temp_cost = Some(cost);
        card.combat_cost_under_turn_override = None;
        card.temp_cost_turn_only = false;
    }
    Ok(())
}

pub(crate) fn reduce_card_cost_for_combat(card: &mut CardInstance, amount: u8) -> SimResult<()> {
    let displayed = card.temp_cost;
    if card.temp_cost_turn_only && displayed.is_some_and(|cost| cost > 0) {
        // AbstractCard.modifyCostForCombat first reduces costForTurn when it is
        // positive, then copies that value into the combat-long `cost` field.
        let reduced = displayed
            .expect("positive displayed cost")
            .saturating_sub(amount);
        card.temp_cost = Some(reduced);
        card.combat_cost_under_turn_override = Some(reduced);
    } else {
        // A zero turn override stays zero this turn while the hidden combat cost
        // is reduced for later turns.
        let reduced = u8::try_from(printed_card_cost(card)?)
            .map_err(|_| SimError::InvalidState("card cost is outside the supported range"))?
            .saturating_sub(amount);
        if card.temp_cost_turn_only {
            card.combat_cost_under_turn_override = Some(reduced);
        } else {
            card.temp_cost = Some(reduced);
            card.combat_cost_under_turn_override = None;
            card.temp_cost_turn_only = false;
        }
    }
    Ok(())
}

pub(crate) fn effective_card_cost(card: &CardInstance) -> SimResult<i32> {
    validate_combat_card_cost_metadata(card)?;
    if let Some(cost) = card.temp_cost {
        return Ok(i32::from(cost));
    }

    let printed = printed_card_cost(card)?;
    if card.content_id == BLOOD_FOR_BLOOD_ID || card.content_id == BLOOD_FOR_BLOOD_PLUS_ID {
        return Ok(printed
            .checked_sub(card.blood_for_blood_cost_reduction)
            .ok_or(SimError::InvalidState(
                "Blood for Blood effective cost overflows i32",
            ))?
            .max(0));
    }
    Ok(printed)
}

pub(crate) fn effective_card_cost_with_corruption(
    card: &CardInstance,
    corruption_active: bool,
) -> SimResult<i32> {
    let definition =
        get_card_definition(card.content_id).ok_or(SimError::UnknownContent(card.content_id))?;
    if corruption_active && definition.card_type == CardType::Skill {
        validate_combat_card_cost_metadata(card)?;
        return Ok(0);
    }
    effective_card_cost(card)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{content::cards::STRIKE_R_ID, CardId, ContentId};

    #[test]
    fn card_cost_metadata_fails_closed() {
        let mut negative = CardInstance::new(CardId::new(1), BLOOD_FOR_BLOOD_ID);
        negative.blood_for_blood_cost_reduction = -1;
        assert_eq!(
            effective_card_cost(&negative),
            Err(SimError::InvalidState(
                "Blood for Blood cost reduction cannot be negative"
            ))
        );

        let mut wrong_card = CardInstance::new(CardId::new(1), STRIKE_R_ID);
        wrong_card.blood_for_blood_cost_reduction = 1;
        assert_eq!(
            effective_card_cost(&wrong_card),
            Err(SimError::InvalidState(
                "non-Blood-for-Blood card carries cost-reduction metadata"
            ))
        );

        let mut missing_cost = CardInstance::new(CardId::new(1), STRIKE_R_ID);
        missing_cost.temp_cost_turn_only = true;
        assert_eq!(
            effective_card_cost(&missing_cost),
            Err(SimError::InvalidState(
                "turn-only card cost flag has no temporary cost"
            ))
        );

        let unknown = CardInstance::new(CardId::new(1), ContentId::new(999_999));
        assert_eq!(
            effective_card_cost(&unknown),
            Err(SimError::UnknownContent(ContentId::new(999_999)))
        );
    }

    #[test]
    fn combat_long_temp_cost_is_the_printed_cost() {
        let mut card = CardInstance::new(CardId::new(1), STRIKE_R_ID);
        card.temp_cost = Some(0);
        card.temp_cost_turn_only = false;
        assert_eq!(printed_card_cost(&card), Ok(0));

        let mut turn_only = CardInstance::new(CardId::new(1), STRIKE_R_ID);
        turn_only.temp_cost = Some(0);
        turn_only.temp_cost_turn_only = true;
        assert_eq!(printed_card_cost(&turn_only), Ok(1));
    }

    #[test]
    fn randomized_cost_replaces_both_cost_fields_only_when_changed() {
        let mut changed = CardInstance::new(CardId::new(1), STRIKE_R_ID);
        changed.temp_cost = Some(0);
        changed.temp_cost_turn_only = true;
        changed.combat_cost_under_turn_override = Some(1);
        set_randomized_combat_cost_if_changed(&mut changed, 2).expect("cost changes");
        assert_eq!(changed.temp_cost, Some(2));
        assert!(!changed.temp_cost_turn_only);
        assert_eq!(changed.combat_cost_under_turn_override, None);

        let mut unchanged = CardInstance::new(CardId::new(2), STRIKE_R_ID);
        unchanged.temp_cost = Some(0);
        unchanged.temp_cost_turn_only = true;
        unchanged.combat_cost_under_turn_override = Some(1);
        set_randomized_combat_cost_if_changed(&mut unchanged, 1).expect("cost stays");
        assert_eq!(unchanged.temp_cost, Some(0));
        assert!(unchanged.temp_cost_turn_only);
        assert_eq!(unchanged.combat_cost_under_turn_override, Some(1));
    }

    #[test]
    fn combat_cost_reduction_preserves_zero_turn_override() {
        let mut card = CardInstance::new(CardId::new(1), STRIKE_R_ID);
        card.temp_cost = Some(0);
        card.temp_cost_turn_only = true;
        reduce_card_cost_for_combat(&mut card, 1).expect("cost reduces");
        assert_eq!(effective_card_cost(&card), Ok(0));
        assert_eq!(printed_card_cost(&card), Ok(0));
        assert_eq!(card.combat_cost_under_turn_override, Some(0));
    }

    #[test]
    fn blood_for_blood_reduction_clamps_at_zero() {
        let mut card = CardInstance::new(CardId::new(1), BLOOD_FOR_BLOOD_ID);
        card.blood_for_blood_cost_reduction = i32::MAX;

        assert_eq!(effective_card_cost(&card), Ok(0));
        assert_eq!(printed_card_cost(&card), Ok(4));
    }
}
