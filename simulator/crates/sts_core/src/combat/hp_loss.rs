use crate::{
    combat::CombatState,
    content::cards::{BLOOD_FOR_BLOOD_ID, BLOOD_FOR_BLOOD_PLUS_ID},
};

pub(crate) fn apply_player_hp_loss_hooks(state: &mut CombatState, hp_loss: i32) {
    if hp_loss <= 0 {
        return;
    }

    reduce_blood_for_blood_costs(state);
    crate::relic::apply_player_hp_loss_relics(state, hp_loss);
}

pub(crate) fn apply_player_card_hp_loss_hooks(state: &mut CombatState, hp_loss: i32) {
    apply_player_hp_loss_hooks(state, hp_loss);
    if hp_loss > 0 {
        state.player.powers.strength += state.player.powers.rupture;
    }
}

fn reduce_blood_for_blood_costs(state: &mut CombatState) {
    for pile in [
        &mut state.piles.hand,
        &mut state.piles.draw_pile,
        &mut state.piles.discard_pile,
        &mut state.piles.exhaust_pile,
    ] {
        for card in pile {
            if card.content_id == BLOOD_FOR_BLOOD_ID || card.content_id == BLOOD_FOR_BLOOD_PLUS_ID {
                card.blood_for_blood_cost_reduction += 1;
            }
        }
    }
}
