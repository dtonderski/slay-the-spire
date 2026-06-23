use crate::{
    combat::{transition::apply_on_exhaust_effects, CombatState},
    content::cards::{get_card_definition, BURN_END_TURN_DAMAGE, BURN_ID, REGRET_ID},
    ids::CardId,
};

pub fn resolve_end_of_turn_hand(state: &mut CombatState) {
    apply_burn_damage_in_hand(state);
    apply_regret_damage_in_hand(state);
    exhaust_unplayed_ethereal_cards(state);
    discard_non_retain_hand(state);
}

fn apply_burn_damage_in_hand(state: &mut CombatState) {
    let burn_copies = state
        .piles
        .hand
        .iter()
        .filter(|card| card.content_id == BURN_ID)
        .count() as i32;

    state.player.hp -= burn_copies * BURN_END_TURN_DAMAGE;
}

fn apply_regret_damage_in_hand(state: &mut CombatState) {
    if state
        .piles
        .hand
        .iter()
        .any(|card| card.content_id == REGRET_ID)
    {
        state.player.hp -= state.piles.hand.len() as i32;
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
            apply_on_exhaust_effects(state);
        }
    }
}

fn discard_non_retain_hand(state: &mut CombatState) {
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
        content::cards::{DAZED_ID, DEFEND_R_ID, ETHEREAL_STRIKE_ID, RETAIN_DEFEND_ID, WOUND_ID},
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
