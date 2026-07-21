use crate::{
    card::CardInstance,
    combat::CardPiles,
    content::cards::{get_card_definition, BURN_ID},
    ids::{card_instance_id_is_supported, CardId},
    ContentId, SimError, SimResult,
};

fn validate_card_generation(
    piles: &CardPiles,
    content_id: ContentId,
    count: i32,
    allocated_card_id_through: u64,
) -> SimResult<(usize, Option<(u64, u64)>)> {
    if get_card_definition(content_id).is_none() {
        return Err(SimError::UnknownContent(content_id));
    }
    let count = usize::try_from(count).map_err(|_| {
        SimError::InvalidState("generated combat card count is outside the target domain")
    })?;
    let count_u64 = u64::try_from(count).map_err(|_| {
        SimError::InvalidState("generated combat card count is outside the target domain")
    })?;
    if allocated_card_id_through < piles.max_card_instance_id() {
        return Err(SimError::InvalidState(
            "generated combat card allocator trails an existing pile ID",
        ));
    }
    let last_id =
        allocated_card_id_through
            .checked_add(count_u64)
            .ok_or(SimError::InvalidState(
                "generated combat card ID overflows u64",
            ))?;
    if count > 0 && !card_instance_id_is_supported(CardId::new(last_id)) {
        return Err(SimError::InvalidState(
            "generated combat card ID exceeds the target signed range",
        ));
    }
    let generated_ids = if count == 0 {
        None
    } else {
        Some((
            allocated_card_id_through
                .checked_add(1)
                .ok_or(SimError::InvalidState(
                    "generated combat card ID overflows u64",
                ))?,
            last_id,
        ))
    };
    Ok((count, generated_ids))
}

pub(crate) fn add_cards_to_discard(
    piles: &mut CardPiles,
    content_id: ContentId,
    count: i32,
    allocated_card_id_through: u64,
) -> SimResult<()> {
    let (count, generated_ids) =
        validate_card_generation(piles, content_id, count, allocated_card_id_through)?;
    piles.discard_pile.try_reserve(count).map_err(|_| {
        SimError::InvalidState("generated combat discard cards cannot be allocated")
    })?;
    if let Some((first_id, last_id)) = generated_ids {
        for id in first_id..=last_id {
            piles
                .discard_pile
                .push(CardInstance::new(CardId::new(id), content_id));
        }
    }
    Ok(())
}

pub(crate) fn add_cards_to_draw_random_spot(
    piles: &mut CardPiles,
    content_id: ContentId,
    count: i32,
    rng: &mut crate::rng::StsRng,
    allocated_card_id_through: u64,
) -> SimResult<()> {
    let (count, generated_ids) =
        validate_card_generation(piles, content_id, count, allocated_card_id_through)?;
    let final_len = piles
        .draw_pile
        .len()
        .checked_add(count)
        .ok_or(SimError::InvalidState(
            "generated combat draw pile length overflows usize",
        ))?;
    if final_len > i32::MAX as usize + 1 {
        return Err(SimError::InvalidState(
            "generated combat draw pile exceeds the target RNG index range",
        ));
    }
    piles
        .draw_pile
        .try_reserve(count)
        .map_err(|_| SimError::InvalidState("generated combat draw cards cannot be allocated"))?;
    if let Some((first_id, last_id)) = generated_ids {
        for id in first_id..=last_id {
            let card = CardInstance::new(CardId::new(id), content_id);
            if piles.draw_pile.is_empty() {
                piles.draw_pile.push(card);
            } else {
                let max_index = i32::try_from(piles.draw_pile.len() - 1).map_err(|_| {
                    SimError::InvalidState(
                        "generated combat draw pile exceeds the target RNG index range",
                    )
                })?;
                let index = rng.random_int(max_index) as usize;
                piles.draw_pile.insert(index, card);
            }
        }
    }
    Ok(())
}

pub(crate) fn upgrade_burns_and_add_upgraded_to_discard(
    piles: &mut CardPiles,
    count: i32,
    allocated_card_id_through: u64,
) -> SimResult<()> {
    let (count, generated_ids) =
        validate_card_generation(piles, BURN_ID, count, allocated_card_id_through)?;
    for card in piles
        .discard_pile
        .iter()
        .chain(piles.draw_pile.iter())
        .filter(|card| card.content_id == BURN_ID)
    {
        card.upgrades
            .checked_add(1)
            .ok_or(SimError::InvalidState("Burn upgrade count overflows u8"))?;
    }
    piles
        .discard_pile
        .try_reserve(count)
        .map_err(|_| SimError::InvalidState("generated upgraded Burns cannot be allocated"))?;

    for card in piles
        .discard_pile
        .iter_mut()
        .chain(piles.draw_pile.iter_mut())
        .filter(|card| card.content_id == BURN_ID)
    {
        card.upgrades += 1;
    }
    if let Some((first_id, last_id)) = generated_ids {
        for id in first_id..=last_id {
            let mut burn = CardInstance::new(CardId::new(id), BURN_ID);
            burn.upgrades = 1;
            piles.discard_pile.push(burn);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{content::cards::BURN_ID, rng::StsRng};

    #[test]
    fn random_draw_insertion_requires_and_consumes_rng() {
        let mut piles = CardPiles {
            hand: Vec::new(),
            draw_pile: vec![
                CardInstance::new(CardId::new(1), BURN_ID),
                CardInstance::new(CardId::new(2), BURN_ID),
            ],
            discard_pile: Vec::new(),
            exhaust_pile: Vec::new(),
        };
        let mut rng = StsRng::new(17);

        add_cards_to_draw_random_spot(&mut piles, BURN_ID, 2, &mut rng, 2)
            .expect("Burn generation is valid");

        assert_eq!(rng.counter(), 2);
        assert_eq!(piles.draw_pile.len(), 4);
        let mut ids = piles
            .draw_pile
            .iter()
            .map(|card| card.id.get())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 2, 3, 4]);
    }

    #[test]
    fn generated_cards_start_after_the_authoritative_external_maximum() {
        let mut piles = CardPiles {
            hand: vec![CardInstance::new(CardId::new(1), BURN_ID)],
            draw_pile: Vec::new(),
            discard_pile: Vec::new(),
            exhaust_pile: Vec::new(),
        };

        add_cards_to_discard(&mut piles, BURN_ID, 2, 100)
            .expect("external authoritative IDs are reserved");

        assert_eq!(
            piles
                .discard_pile
                .iter()
                .map(|card| card.id.get())
                .collect::<Vec<_>>(),
            vec![101, 102]
        );
    }

    #[test]
    fn generated_card_id_overflow_does_not_mutate_piles_or_rng() {
        let mut piles = CardPiles {
            hand: Vec::new(),
            draw_pile: vec![CardInstance::new(CardId::new(1), BURN_ID)],
            discard_pile: Vec::new(),
            exhaust_pile: Vec::new(),
        };
        let piles_before = piles.clone();
        let mut rng = StsRng::new(17);
        let rng_before = rng.clone();

        let result =
            add_cards_to_draw_random_spot(&mut piles, BURN_ID, 1, &mut rng, i64::MAX as u64);

        assert_eq!(
            result,
            Err(SimError::InvalidState(
                "generated combat card ID exceeds the target signed range"
            ))
        );
        assert_eq!(piles, piles_before);
        assert_eq!(rng, rng_before);
    }

    #[test]
    fn burn_upgrade_overflow_does_not_mutate_piles() {
        let mut burn = CardInstance::new(CardId::new(1), BURN_ID);
        burn.upgrades = u8::MAX;
        let mut piles = CardPiles {
            hand: Vec::new(),
            draw_pile: vec![burn],
            discard_pile: Vec::new(),
            exhaust_pile: Vec::new(),
        };
        let piles_before = piles.clone();

        let result = upgrade_burns_and_add_upgraded_to_discard(&mut piles, 3, 1);

        assert_eq!(
            result,
            Err(SimError::InvalidState("Burn upgrade count overflows u8"))
        );
        assert_eq!(piles, piles_before);
    }
}
