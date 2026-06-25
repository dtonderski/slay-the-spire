use crate::{
    card::CardType,
    combat::{damage::deal_unmodified_damage_to_monster, CombatState},
    content::{
        cards::{get_card_definition, is_curse_content_id},
        monsters::{check_slime_boss_split, guardian_on_hp_damage, wake_lagavulin_on_damage},
    },
    ids::{ContentId, MonsterId},
    rng::{JavaRng, RngStream, SimulatorRng, StsRng},
    CardInstance, Relic,
};

/// CommunicationMod lists draw piles bottom-first; the game draws from the top (last entry).
fn draw_card_from_pile_top(state: &mut CombatState) -> Option<CardInstance> {
    state.piles.draw_pile.pop()
}

pub fn draw_cards(state: &mut CombatState, count: usize, rng: &mut SimulatorRng) {
    for _ in 0..count {
        if state.piles.draw_pile.is_empty() {
            shuffle_discard_into_draw(state, rng);
        }

        if state.piles.draw_pile.is_empty() {
            break;
        }

        if let Some(mut card) = draw_card_from_pile_top(state) {
            let content_id = card.content_id;
            apply_snecko_eye_cost_randomization(state, &mut card);
            state.piles.hand.push(card);
            apply_fire_breathing_on_draw(state, content_id);
            draw_cards(state, evolve_extra_draw_count(state, content_id), rng);
        }
    }
}

pub fn draw_cards_with_sts_rng(state: &mut CombatState, count: usize, rng: &mut StsRng) {
    for _ in 0..count {
        if state.piles.draw_pile.is_empty() {
            shuffle_discard_into_draw_sts(state, rng);
        }

        if state.piles.draw_pile.is_empty() {
            break;
        }

        if let Some(mut card) = draw_card_from_pile_top(state) {
            let content_id = card.content_id;
            apply_snecko_eye_cost_randomization(state, &mut card);
            state.piles.hand.push(card);
            apply_fire_breathing_on_draw(state, content_id);
            draw_cards_with_sts_rng(state, evolve_extra_draw_count(state, content_id), rng);
        }
    }
}

pub(crate) fn draw_cards_without_shuffle(state: &mut CombatState, count: usize) {
    for _ in 0..count {
        if state.piles.draw_pile.is_empty() {
            break;
        }

        if let Some(mut card) = draw_card_from_pile_top(state) {
            let content_id = card.content_id;
            apply_snecko_eye_cost_randomization(state, &mut card);
            state.piles.hand.push(card);
            apply_fire_breathing_on_draw(state, content_id);
            draw_cards_without_shuffle(state, evolve_extra_draw_count(state, content_id));
        }
    }
}

fn apply_fire_breathing_on_draw(state: &mut CombatState, content_id: crate::ContentId) {
    let amount = state.player.powers.fire_breathing;
    if amount <= 0 || !is_status_or_curse(content_id) {
        return;
    }

    let targets = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster.id)
        .collect::<Vec<MonsterId>>();

    for target in targets {
        let still_alive = {
            let Some(monster) = state
                .monsters
                .iter_mut()
                .find(|monster| monster.id == target && monster.alive)
            else {
                continue;
            };
            let hp_damage = deal_unmodified_damage_to_monster(monster, amount);
            wake_lagavulin_on_damage(monster, hp_damage);
            guardian_on_hp_damage(monster, hp_damage);
            monster.alive
        };
        check_slime_boss_split(state, target);
        if !still_alive {
            crate::relic::apply_monster_death_relics(state);
        }
    }
}

pub(crate) fn evolve_extra_draw_count(state: &CombatState, content_id: ContentId) -> usize {
    if state.player.powers.evolve <= 0 {
        return 0;
    }
    if get_card_definition(content_id)
        .is_some_and(|definition| definition.card_type == CardType::Status)
    {
        state.player.powers.evolve as usize
    } else {
        0
    }
}

fn is_status_or_curse(content_id: crate::ContentId) -> bool {
    is_curse_content_id(content_id)
        || get_card_definition(content_id)
            .is_some_and(|definition| definition.card_type == CardType::Status)
}

pub(crate) fn apply_snecko_eye_cost_randomization(
    state: &mut CombatState,
    card: &mut CardInstance,
) {
    if !state.relics.contains(&Relic::SneckoEye) {
        return;
    }
    if !get_card_definition(card.content_id).is_some_and(|definition| definition.cost > 0) {
        return;
    }
    let Some(rng) = state.card_random_rng.as_mut() else {
        return;
    };
    card.temp_cost = Some(rng.random_int(3) as u8);
}

fn shuffle_discard_into_draw(state: &mut CombatState, rng: &mut SimulatorRng) {
    if state.piles.discard_pile.is_empty() {
        return;
    }

    state.piles.draw_pile.append(&mut state.piles.discard_pile);

    for index in (1..state.piles.draw_pile.len()).rev() {
        let swap_with = rng.next_usize(RngStream::Shuffle, "combat::draw::shuffle", index + 1);
        state.piles.draw_pile.swap(index, swap_with);
    }
    crate::relic::apply_shuffle_relics(state);
}

fn shuffle_discard_into_draw_sts(state: &mut CombatState, rng: &mut StsRng) {
    if state.piles.discard_pile.is_empty() {
        return;
    }

    state.piles.draw_pile.append(&mut state.piles.discard_pile);
    let shuffle_seed = rng.random_long();
    JavaRng::new(shuffle_seed).collections_shuffle(&mut state.piles.draw_pile);
    crate::relic::apply_shuffle_relics(state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{legal_combat_actions, CardInstance, ContentId, Relic};

    #[test]
    fn draw_pile_draws_from_top_of_bottom_first_export_order() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(crate::CardId::new(10), ContentId::new(1)),
            CardInstance::new(crate::CardId::new(11), ContentId::new(2)),
            CardInstance::new(crate::CardId::new(12), ContentId::new(3)),
        ];
        let mut rng = SimulatorRng::new(1);

        draw_cards(&mut state, 2, &mut rng);

        assert_eq!(state.piles.hand[0].id, crate::CardId::new(12));
        assert_eq!(state.piles.hand[1].id, crate::CardId::new(11));
        assert_eq!(state.piles.draw_pile.len(), 1);
        assert_eq!(state.piles.draw_pile[0].id, crate::CardId::new(10));
    }

    #[test]
    fn draw_order_is_deterministic_without_shuffle() {
        let mut first = fixture_with_draw_pile();
        let mut second = fixture_with_draw_pile();
        let mut first_rng = SimulatorRng::new(1);
        let mut second_rng = SimulatorRng::new(1);

        draw_cards(&mut first, 2, &mut first_rng);
        draw_cards(&mut second, 2, &mut second_rng);

        assert_eq!(first.piles.hand, second.piles.hand);
        assert_eq!(first.piles.hand[0].id, crate::CardId::new(11));
        assert_eq!(first.piles.hand[1].id, crate::CardId::new(10));
        assert!(first_rng.log().is_empty());
    }

    #[test]
    fn shuffle_consumes_logged_placeholder_rng() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![
            CardInstance::new(crate::CardId::new(20), ContentId::new(1)),
            CardInstance::new(crate::CardId::new(21), ContentId::new(1)),
            CardInstance::new(crate::CardId::new(22), ContentId::new(1)),
        ];
        let mut rng = SimulatorRng::new(3);

        draw_cards(&mut state, 2, &mut rng);

        assert_eq!(state.piles.hand.len(), 2);
        assert_eq!(rng.log().len(), 2);
        assert!(rng
            .log()
            .iter()
            .all(|draw| draw.stream == RngStream::Shuffle));
    }

    #[test]
    fn the_abacus_grants_block_when_discard_is_shuffled_into_draw() {
        let mut state = fixture_with_discard_only();
        state.relics = vec![Relic::TheAbacus];
        state.player.block = 2;
        let mut rng = SimulatorRng::new(3);

        draw_cards(&mut state, 1, &mut rng);

        assert_eq!(state.player.block, 2 + crate::relic::THE_ABACUS_BLOCK);
    }

    #[test]
    fn the_abacus_does_not_trigger_without_shuffle() {
        let mut state = fixture_with_draw_pile();
        state.relics = vec![Relic::TheAbacus];
        state.player.block = 2;
        let mut rng = SimulatorRng::new(3);

        draw_cards(&mut state, 1, &mut rng);

        assert_eq!(state.player.block, 2);
    }

    #[test]
    fn sundial_grants_energy_on_every_third_shuffle() {
        let mut state = fixture_with_discard_only();
        state.relics = vec![Relic::Sundial];
        state.player.energy = 1;
        let mut rng = SimulatorRng::new(3);

        for _ in 0..2 {
            draw_cards(&mut state, 1, &mut rng);
            state.piles.hand.clear();
            state.piles.draw_pile.clear();
            state.piles.discard_pile =
                vec![CardInstance::new(crate::CardId::new(40), ContentId::new(1))];
        }
        draw_cards(&mut state, 1, &mut rng);

        assert_eq!(
            state.relic_counters.sundial_shuffles,
            crate::relic::SUNDIAL_THRESHOLD
        );
        assert_eq!(state.player.energy, 1 + crate::relic::SUNDIAL_ENERGY);
    }

    #[test]
    fn sundial_does_not_grant_energy_before_third_shuffle() {
        let mut state = fixture_with_discard_only();
        state.relics = vec![Relic::Sundial];
        state.player.energy = 1;
        let mut rng = SimulatorRng::new(3);

        draw_cards(&mut state, 1, &mut rng);

        assert_eq!(state.relic_counters.sundial_shuffles, 1);
        assert_eq!(state.player.energy, 1);
    }

    #[test]
    fn placeholder_shuffle_is_deterministic_but_not_claimed_game_compatible() {
        let mut first = fixture_with_discard_only();
        let mut second = fixture_with_discard_only();
        let mut first_rng = SimulatorRng::new(99);
        let mut second_rng = SimulatorRng::new(99);

        draw_cards(&mut first, 3, &mut first_rng);
        draw_cards(&mut second, 3, &mut second_rng);

        assert_eq!(first.piles.hand, second.piles.hand);
        assert_eq!(first_rng.log(), second_rng.log());
    }

    #[test]
    fn legal_actions_and_serialization_consume_no_rng() {
        let state = CombatState::initial_fixture();
        let rng = SimulatorRng::new(5);
        let before_log_len = rng.log().len();

        let _actions = legal_combat_actions(&state);
        let _json = serde_json::to_string(&state).expect("state serializes");

        assert_eq!(rng.log().len(), before_log_len);
    }

    fn fixture_with_draw_pile() -> CombatState {
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(crate::CardId::new(10), ContentId::new(1)),
            CardInstance::new(crate::CardId::new(11), ContentId::new(1)),
        ];
        state
    }

    fn fixture_with_discard_only() -> CombatState {
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![
            CardInstance::new(crate::CardId::new(30), ContentId::new(1)),
            CardInstance::new(crate::CardId::new(31), ContentId::new(1)),
            CardInstance::new(crate::CardId::new(32), ContentId::new(1)),
        ];
        state
    }
}
