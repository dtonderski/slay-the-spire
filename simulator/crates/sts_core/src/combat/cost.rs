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
    get_card_definition(card.content_id)
        .map(|definition| i32::from(definition.cost))
        .ok_or(SimError::UnknownContent(card.content_id))
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
    fn blood_for_blood_reduction_clamps_at_zero() {
        let mut card = CardInstance::new(CardId::new(1), BLOOD_FOR_BLOOD_ID);
        card.blood_for_blood_cost_reduction = i32::MAX;

        assert_eq!(effective_card_cost(&card), Ok(0));
        assert_eq!(printed_card_cost(&card), Ok(4));
    }
}
