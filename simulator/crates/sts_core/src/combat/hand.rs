use crate::{
    combat::{transition::apply_on_exhaust_effects, CombatState},
    content::cards::{get_card_definition, BURN_END_TURN_DAMAGE, BURN_ID, DOUBT_ID, REGRET_ID},
    ids::CardId,
};

pub fn resolve_end_of_turn_hand(state: &mut CombatState) {
    apply_burn_damage_in_hand(state);
    apply_regret_damage_in_hand(state);
    exhaust_unplayed_ethereal_cards(state);
}

pub(crate) fn resolve_end_of_turn_doubt(state: &mut CombatState) {
    apply_doubt_weak_in_hand(state);
}

pub(crate) fn discard_end_of_turn_hand(state: &mut CombatState) {
    discard_non_retain_hand(state);
}

fn apply_burn_damage_in_hand(state: &mut CombatState) {
    let burn_copies = state
        .piles
        .hand
        .iter()
        .filter(|card| card.content_id == BURN_ID)
        .count() as i32;

    let mitigated =
        crate::relic::mitigate_hp_loss(&state.relics, burn_copies * BURN_END_TURN_DAMAGE);
    let hp_loss = crate::relic::apply_buffer_to_hp_loss(&mut state.player.powers, mitigated);
    state.player.hp -= hp_loss;
    crate::relic::apply_player_hp_loss_relics(state, hp_loss);
}

fn apply_regret_damage_in_hand(state: &mut CombatState) {
    if state
        .piles
        .hand
        .iter()
        .any(|card| card.content_id == REGRET_ID)
    {
        let mitigated =
            crate::relic::mitigate_hp_loss(&state.relics, state.piles.hand.len() as i32);
        let hp_loss = crate::relic::apply_buffer_to_hp_loss(&mut state.player.powers, mitigated);
        state.player.hp -= hp_loss;
        crate::relic::apply_player_hp_loss_relics(state, hp_loss);
    }
}

fn apply_doubt_weak_in_hand(state: &mut CombatState) {
    let doubt_copies = state
        .piles
        .hand
        .iter()
        .filter(|card| card.content_id == DOUBT_ID)
        .count() as i32;

    if doubt_copies > 0 {
        crate::relic::apply_player_weak_with_relics(
            &mut state.player.powers,
            &state.relics,
            doubt_copies,
        );
    }
}

fn exhaust_unplayed_ethereal_cards(state: &mut CombatState) {
    let ethereal_ids: Vec<CardId> = state
        .piles
        .hand
        .iter()
        .filter(|card| {
            get_card_definition(card.content_id)
                .is_some_and(|definition| definition.keywords.ethereal)
        })
        .map(|card| card.id)
        .collect();

    for card_id in ethereal_ids {
        if let Some(index) = state.piles.hand.iter().position(|card| card.id == card_id) {
            let card = state.piles.hand.remove(index);
            state.piles.exhaust_pile.push(card);
            apply_on_exhaust_effects(state, card_id);
        }
    }
}

fn discard_non_retain_hand(state: &mut CombatState) {
    if state.relics.contains(&crate::Relic::RunicPyramid) {
        return;
    }

    let mut retained = Vec::new();
    let mut discarded = Vec::new();

    for card in state.piles.hand.drain(..) {
        if get_card_definition(card.content_id).is_some_and(|definition| definition.keywords.retain)
        {
            retained.push(card);
        } else {
            discarded.push(card);
        }
    }

    discarded.reverse();
    state.piles.hand = retained;
    state.piles.discard_pile.extend(discarded);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        content::cards::{
            DAZED_ID, DEFEND_R_ID, DOUBT, ETHEREAL_STRIKE_ID, RETAIN_DEFEND_ID, WOUND_ID,
        },
        ids::CardId,
        CardInstance,
    };

    #[test]
    fn wound_in_hand_is_not_a_legal_play_action() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), WOUND_ID)];

        assert!(!crate::legal_combat_actions(&state)
            .iter()
            .any(|action| matches!(action, crate::CombatAction::PlayCard { .. })));
    }

    #[test]
    fn wound_round_trips_through_combat_state_json() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), WOUND_ID)];

        let json = serde_json::to_string(&state).expect("state serializes");
        let restored: CombatState = serde_json::from_str(&json).expect("state deserializes");

        assert_eq!(restored.piles.hand[0].content_id, WOUND_ID);
    }

    #[test]
    fn dazed_clogs_hand_without_play_actions() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), DAZED_ID),
            CardInstance::new(CardId::new(21), WOUND_ID),
        ];

        assert_eq!(
            crate::legal_combat_actions(&state)
                .into_iter()
                .filter(|action| matches!(action, crate::CombatAction::PlayCard { .. }))
                .count(),
            0
        );
    }

    #[test]
    fn burn_in_hand_deals_damage_at_end_of_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.monsters[0].alive = false;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), crate::content::cards::BURN_ID),
            CardInstance::new(CardId::new(21), crate::content::cards::BURN_ID),
        ];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.player.hp, 16);
    }

    #[test]
    fn tungsten_rod_reduces_burn_hp_loss_at_end_of_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.monsters[0].alive = false;
        state.relics = vec![crate::Relic::TungstenRod];
        state.piles.hand = vec![CardInstance::new(
            CardId::new(20),
            crate::content::cards::BURN_ID,
        )];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.player.hp, 19);
    }

    #[test]
    fn buffer_prevents_burn_hp_loss_at_end_of_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.powers.buffer = 1;
        state.monsters[0].alive = false;
        state.piles.hand = vec![CardInstance::new(
            CardId::new(20),
            crate::content::cards::BURN_ID,
        )];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.player.hp, 20);
        assert_eq!(next.player.powers.buffer, 0);
    }

    #[test]
    fn regret_in_hand_deals_damage_equal_to_hand_size() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.monsters[0].alive = false;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), REGRET_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
            CardInstance::new(CardId::new(22), crate::content::cards::STRIKE_R_ID),
        ];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.player.hp, 17);
    }

    #[test]
    fn doubt_is_status_curse_and_unplayable() {
        assert_eq!(DOUBT.id, DOUBT_ID);
        assert_eq!(DOUBT.card_type, crate::card::CardType::Status);
        assert!(DOUBT.keywords.unplayable);
        assert!(crate::content::cards::is_curse_content_id(DOUBT_ID));
    }

    #[test]
    fn doubt_in_hand_applies_weak_at_end_of_turn_then_discards() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].alive = false;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DOUBT_ID)];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.player.powers.weak, 1);
        assert_eq!(next.piles.hand.len(), 0);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == DOUBT_ID));
    }

    #[test]
    fn multiple_doubts_stack_weak_at_end_of_turn() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].alive = false;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), DOUBT_ID),
            CardInstance::new(CardId::new(21), DOUBT_ID),
        ];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.player.powers.weak, 2);
        assert_eq!(
            next.piles
                .discard_pile
                .iter()
                .filter(|card| card.content_id == DOUBT_ID)
                .count(),
            2
        );
    }

    #[test]
    fn runic_pyramid_keeps_doubt_after_it_applies_weak() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].alive = false;
        state.relics = vec![crate::Relic::RunicPyramid];
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DOUBT_ID)];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.player.powers.weak, 1);
        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, DOUBT_ID);
        assert!(next.piles.discard_pile.is_empty());
    }

    #[test]
    fn doubt_weak_composes_with_existing_player_weak_lifecycle() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].alive = false;
        state.player.powers.weak = 2;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DOUBT_ID)];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.player.powers.weak, 2);
    }

    #[test]
    fn artifact_blocks_doubt_weak() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].alive = false;
        state.player.powers.artifact = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DOUBT_ID)];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.player.powers.weak, 0);
        assert_eq!(next.player.powers.artifact, 0);
    }

    #[test]
    fn ginger_prevents_doubt_weak_without_consuming_artifact() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].alive = false;
        state.player.powers.artifact = 1;
        state.relics = vec![crate::Relic::Ginger];
        state.piles.hand = vec![CardInstance::new(CardId::new(20), DOUBT_ID)];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.player.powers.weak, 0);
        assert_eq!(next.player.powers.artifact, 1);
    }

    #[test]
    fn tungsten_rod_reduces_regret_hp_loss_at_end_of_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.monsters[0].alive = false;
        state.relics = vec![crate::Relic::TungstenRod];
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), REGRET_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
        ];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.player.hp, 19);
    }

    #[test]
    fn unplayed_ethereal_card_exhausts_at_end_of_turn() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), ETHEREAL_STRIKE_ID)];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);

        assert!(next.piles.hand.is_empty());
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == ETHEREAL_STRIKE_ID));
    }

    #[test]
    fn retain_card_stays_in_hand_across_end_turn() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), RETAIN_DEFEND_ID)];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, RETAIN_DEFEND_ID);
    }

    #[test]
    fn runic_pyramid_keeps_non_retain_hand_cards_at_end_of_turn() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![crate::Relic::RunicPyramid];
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), crate::content::cards::STRIKE_R_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
        ];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);
        let hand: Vec<_> = next.piles.hand.iter().map(|card| card.id).collect();

        assert_eq!(hand, vec![CardId::new(20), CardId::new(21)]);
        assert!(next.piles.discard_pile.is_empty());
    }

    #[test]
    fn runic_pyramid_still_exhausts_unplayed_ethereal_cards() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![crate::Relic::RunicPyramid];
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), ETHEREAL_STRIKE_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
        ];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].id, CardId::new(21));
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == ETHEREAL_STRIKE_ID));
    }

    #[test]
    fn end_turn_discards_hand_from_top_to_bottom() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].alive = false;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(20), crate::content::cards::STRIKE_R_ID),
            CardInstance::new(CardId::new(21), DEFEND_R_ID),
            CardInstance::new(CardId::new(22), crate::content::cards::BASH_ID),
        ];
        state.piles.draw_pile.clear();

        let next = crate::combat::end_player_turn(&state);
        let discarded: Vec<_> = next.piles.discard_pile.iter().map(|card| card.id).collect();

        assert_eq!(
            discarded,
            vec![CardId::new(22), CardId::new(21), CardId::new(20)]
        );
    }
}
