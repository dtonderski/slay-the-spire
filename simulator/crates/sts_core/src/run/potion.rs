use crate::{
    card::{CardInstance, CardType, TargetRequirement},
    combat::damage::deal_unmodified_damage_to_monster,
    combat::transition::{
        apply_monster_death_hooks, apply_play_top_draw_card_action, choose_discard_select,
        choose_draw_select, choose_exhaust_select, choose_hand_select,
        close_discovery_source_card_with_force_exhaust,
        confirm_burning_pact_select_skipped_retrieval_with_time_warp_policy,
        confirm_discard_select, confirm_draw_select, confirm_exhaust_select_with_dead_branch_count,
        confirm_exhume_select_skipped_return, confirm_gambling_chip_select_skipped_retrieval,
        confirm_hand_select, confirm_hand_select_skipped_put_on_deck_retrieval,
        confirm_hand_select_skipped_put_on_deck_retrieval_with_time_warp_policy,
        confirm_hand_select_time_warp_status_lag, confirm_hand_select_with_time_warp_policy,
        confirm_headbutt_select_skipped_retrieval, confirm_recycle_select_skipped_retrieval,
        discard_select_ui_to_discard_index, draw_select_ui_to_draw_index,
        exhaust_select_ui_to_hand_index, flush_pending_player_spikes_damage_if_ready,
        hand_select_ui_to_hand_index, open_discard_select_with_max_choices, open_exhaust_select,
        open_gambling_chip_select, player_draw_cards, player_shuffle_discard_into_draw,
        top_draw_card_definition,
    },
    combat::{
        apply_burning_blood, CombatDecisionState, CombatPhase, CombatState, DiscardSelectPurpose,
        ExhaustSelectPurpose, HandSelectPurpose, PendingPotionCardRewardSettlement,
        PotionCardRewardKind,
    },
    content::cards::{get_card_definition, upgrade_card_instance, DISCOVERY_ID, DISCOVERY_PLUS_ID},
    content::monsters::{wake_lagavulin_on_damage, AWAKENED_ONE_ID},
    content::shop_pool::{
        burn_all_discovery_card_choice_generations, burn_colorless_discovery_card_choice_draws,
        burn_colorless_discovery_card_choice_generations, burn_discovery_card_choice_draws,
        burn_discovery_card_choice_generations, colorless_discovery_card_choices,
        discovery_card_choices,
    },
    ids::{CardId, MonsterId},
    map::RoomKind,
    potion::{
        Potion, ANCIENT_POTION_ARTIFACT, BLOCK_POTION_BLOCK, BLOOD_POTION_HEAL_PERCENT,
        CULTIST_POTION_RITUAL, DEXTERITY_POTION_DEXTERITY, ENERGY_POTION_ENERGY,
        ESSENCE_OF_STEEL_PLATED_ARMOR, EXPLOSIVE_POTION_DAMAGE, FEAR_POTION_VULNERABLE,
        FIRE_POTION_DAMAGE, FLEX_POTION_TEMP_STRENGTH, FRUIT_JUICE_MAX_HP,
        HEART_OF_IRON_METALLICIZE, LIQUID_BRONZE_THORNS, REGEN_POTION_REGEN, SNECKO_OIL_DRAW,
        SPEED_POTION_TEMP_DEXTERITY, STRENGTH_POTION_STRENGTH, SWIFT_POTION_DRAW, WEAK_POTION_WEAK,
    },
    power::{apply_monster_vulnerable, apply_monster_weak},
    relic::Relic,
    rng::StsRng,
    run::reward::{
        apply_dead_branch_for_exhaust_count, enter_final_boss_victory, target_elite_combat_gold,
        target_normal_combat_gold, target_potion_reward_offer, target_random_combat_potion,
        target_random_potion,
    },
    run::state::RunRngStream,
    RunAction, RunPhase, RunState, SimError, SimResult,
};

// DiscoveryAction generates choices at the top of every update while its
// fast-duration action settles. Real CommunicationMod traces prove that the
// target burn differs when the reward is picked versus skipped, and that the
// colorless generation branch settles two updates later than typed cards in
// this verifier environment.
const DISCOVERY_ACTION_PICKED_HIDDEN_GENERATIONS: usize = 9;
const COLORLESS_DISCOVERY_ACTION_PICKED_HIDDEN_GENERATIONS: usize = 11;
const DISCOVERY_ACTION_PICKED_SCREEN_SETTLE_DRAWS: usize = 1;
const DISCOVERY_ACTION_SKIPPED_HIDDEN_GENERATIONS: usize = 6;
const DISCOVERY_ACTION_SKIPPED_SCREEN_SETTLE_DRAWS: usize = 3;
const POTION_DISCOVERY_POST_PICKED_HIDDEN_GENERATIONS: usize = 12;
const POTION_DISCOVERY_POST_PICKED_SCREEN_SETTLE_DRAWS: usize = 1;

pub fn validate_potion_action(run: &RunState, action: RunAction) -> SimResult<()> {
    run.validate()?;

    if run.phase == RunPhase::Combat {
        let combat = run
            .combat
            .as_ref()
            .ok_or(SimError::InvalidState("combat state is missing"))?;
        if combat.phase != CombatPhase::WaitingForPlayer {
            return Err(SimError::IllegalAction(
                "combat is not waiting for player input",
            ));
        }
    }

    match action {
        RunAction::UsePotion { slot, target } => {
            let potion = run
                .potion_at_slot(slot)
                .ok_or(SimError::IllegalAction("potion slot is not available"))?;

            if potion == Potion::Fairy {
                return Err(SimError::IllegalAction("Fairy is passive"));
            }

            if potion.requires_combat() {
                if run.phase != RunPhase::Combat {
                    return Err(SimError::IllegalAction("potion use requires combat phase"));
                }
                let combat = run
                    .combat
                    .as_ref()
                    .ok_or(SimError::InvalidState("combat state is missing"))?;

                if potion.requires_target() {
                    let Some(target) = target else {
                        return Err(SimError::IllegalAction("potion requires a target"));
                    };
                    if !combat
                        .monsters
                        .iter()
                        .any(|monster| monster.id == target && monster.alive)
                    {
                        return Err(SimError::IllegalAction("potion target is not alive"));
                    }
                } else if target.is_some() {
                    return Err(SimError::IllegalAction("potion does not take a target"));
                }
                if potion == Potion::SmokeBomb && current_room_kind(run) == Some(RoomKind::Boss) {
                    return Err(SimError::IllegalAction(
                        "Smoke Bomb cannot be used in boss combat",
                    ));
                }
            } else if target.is_some() {
                return Err(SimError::IllegalAction("potion does not take a target"));
            }

            Ok(())
        }
        RunAction::DiscardPotion { slot } => {
            run.potion_at_slot(slot)
                .ok_or(SimError::IllegalAction("potion slot is not available"))?;
            Ok(())
        }
        RunAction::ChooseCombatCardReward { index } => {
            validate_combat_card_reward_choice(run, index)
        }
        RunAction::SkipCombatCardReward => validate_combat_card_reward_skip(run),
        RunAction::ChooseHandSelect { index } => validate_hand_select_choice(run, index),
        RunAction::ConfirmHandSelect => validate_hand_select_confirm(run),
        RunAction::ChooseDrawSelect { index } => validate_draw_select_choice(run, index),
        RunAction::ConfirmDrawSelect => validate_draw_select_confirm(run),
        RunAction::ChooseDiscardSelect { index } => validate_discard_select_choice(run, index),
        RunAction::ConfirmDiscardSelect => validate_discard_select_confirm(run),
        RunAction::ChooseExhaustSelect { index } => validate_exhaust_select_choice(run, index),
        RunAction::ConfirmExhaustSelect => validate_exhaust_select_confirm(run),
        _ => Err(SimError::IllegalAction("not a potion action")),
    }
}

fn current_room_kind(run: &RunState) -> Option<RoomKind> {
    run.current_room_override.or_else(|| {
        run.map.as_ref().and_then(|map_state| {
            map_state
                .map
                .node(map_state.current_node)
                .map(|node| node.room_kind)
        })
    })
}

pub fn validate_combat_card_reward_skip(run: &RunState) -> SimResult<()> {
    let combat = run.combat.as_ref().ok_or(SimError::IllegalAction(
        "combat card reward requires combat",
    ))?;
    if combat.potion_card_reward_choices().is_some()
        || matches!(
            combat.decision.as_ref(),
            Some(CombatDecisionState::NilrysCodexCardReward { .. })
        )
    {
        Ok(())
    } else {
        Err(SimError::IllegalAction(
            "no skippable combat card reward is open",
        ))
    }
}

pub fn validate_combat_card_reward_choice(run: &RunState, index: usize) -> SimResult<()> {
    let combat = run.combat.as_ref().ok_or(SimError::IllegalAction(
        "combat card reward requires combat",
    ))?;
    let choices = combat
        .combat_card_reward_choices()
        .ok_or(SimError::IllegalAction("no combat card reward is open"))?;
    if index >= choices.len() {
        return Err(SimError::IllegalAction(
            "combat card reward index out of range",
        ));
    }
    Ok(())
}

pub fn validate_hand_select_choice(run: &RunState, index: usize) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("hand select requires combat"))?;
    let hand_index = hand_select_ui_to_hand_index(combat, index)?;
    let hand_select = combat
        .hand_select()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    if hand_select.purpose != HandSelectPurpose::ForethoughtPutAnyOnDraw
        && hand_select.selected_hand_index == Some(hand_index)
    {
        return Err(SimError::IllegalAction(
            "hand select choice is already selected",
        ));
    }
    Ok(())
}

pub fn validate_hand_select_confirm(run: &RunState) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("hand select requires combat"))?;
    let hand_select = combat
        .hand_select()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    if hand_select.purpose != HandSelectPurpose::ForethoughtPutAnyOnDraw
        && hand_select.selected_hand_index.is_none()
    {
        return Err(SimError::IllegalAction("hand select choice is required"));
    }
    Ok(())
}

pub fn validate_draw_select_choice(run: &RunState, index: usize) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("draw select requires combat"))?;
    let draw_index = draw_select_ui_to_draw_index(combat, index)?;
    let draw_select = combat
        .draw_select()
        .ok_or(SimError::IllegalAction("no draw select is open"))?;
    if draw_select.purpose != crate::combat::DrawSelectPurpose::Scry
        && draw_select.selected_draw_index == Some(draw_index)
    {
        return Err(SimError::IllegalAction(
            "draw select choice is already selected",
        ));
    }
    Ok(())
}

pub fn validate_draw_select_confirm(run: &RunState) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("draw select requires combat"))?;
    let draw_select = combat
        .draw_select()
        .ok_or(SimError::IllegalAction("no draw select is open"))?;
    if draw_select.selected_draw_index.is_none()
        && draw_select.purpose != crate::combat::DrawSelectPurpose::Scry
    {
        return Err(SimError::IllegalAction("draw select choice is required"));
    }
    Ok(())
}

pub fn validate_discard_select_choice(run: &RunState, index: usize) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("discard select requires combat"))?;
    discard_select_ui_to_discard_index(combat, index)?;
    Ok(())
}

pub fn validate_discard_select_confirm(run: &RunState) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("discard select requires combat"))?;
    let discard_select = combat
        .discard_select()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    if discard_select.selected_discard_index.is_none() {
        return Err(SimError::IllegalAction("discard select choice is required"));
    }
    Ok(())
}

pub fn validate_exhaust_select_choice(run: &RunState, index: usize) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("exhaust select requires combat"))?;
    exhaust_select_ui_to_hand_index(combat, index)?;
    Ok(())
}

pub fn validate_exhaust_select_confirm(run: &RunState) -> SimResult<()> {
    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::IllegalAction("exhaust select requires combat"))?;
    let exhaust_select = combat
        .exhaust_select()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    if exhaust_select.purpose == ExhaustSelectPurpose::ExhumeReturnToHand
        && exhaust_select.selected_hand_indices.is_empty()
    {
        return Err(SimError::IllegalAction("exhaust select choice is required"));
    }
    Ok(())
}

pub fn apply_hand_select_choice(run: &RunState, index: usize) -> SimResult<RunState> {
    validate_hand_select_choice(run, index)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    choose_hand_select(combat, index)?;
    Ok(next)
}

/// Confirm a hand select without consuming Time Warp's forced end-turn.
/// CommunicationMod can snapshot the post-CONFIRM piles before that END drains
/// the hand (FIDL01274 Warcry under Time Warp).
pub fn apply_hand_select_confirm_without_time_warp_end(run: &RunState) -> SimResult<RunState> {
    validate_hand_select_confirm(run)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    let exhaust_before = combat.piles.exhaust_pile.len();
    confirm_hand_select_with_time_warp_policy(&mut combat, false)?;
    let exhaust_count = combat
        .piles
        .exhaust_pile
        .len()
        .saturating_sub(exhaust_before);
    apply_dead_branch_for_exhaust_count(&mut next, &mut combat, exhaust_count)?;
    next.combat = Some(combat);
    Ok(next)
}

/// Confirm a 12th-card Warcry without PutOnDeck, autoplay end-turn statuses,
/// and leave Time Warp's remaining EndTurn queued (FIDL01274).
pub fn apply_hand_select_confirm_time_warp_status_lag(run: &RunState) -> SimResult<RunState> {
    validate_hand_select_confirm(run)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    let exhaust_before = combat.piles.exhaust_pile.len();
    confirm_hand_select_time_warp_status_lag(&mut combat)?;
    let exhaust_count = combat
        .piles
        .exhaust_pile
        .len()
        .saturating_sub(exhaust_before);
    apply_dead_branch_for_exhaust_count(&mut next, &mut combat, exhaust_count)?;
    next.combat = Some(combat);
    Ok(next)
}

pub fn apply_hand_select_confirm(run: &RunState) -> SimResult<RunState> {
    validate_hand_select_confirm(run)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    let exhaust_before = combat.piles.exhaust_pile.len();
    confirm_hand_select(&mut combat)?;
    let exhaust_count = combat
        .piles
        .exhaust_pile
        .len()
        .saturating_sub(exhaust_before);
    apply_dead_branch_for_exhaust_count(&mut next, &mut combat, exhaust_count)?;
    settle_run_after_select_confirm(next, combat)
}

/// Confirm put-on-deck hand select without retrieving the selected card, then
/// apply the same post-confirm Dead Branch settlement as a normal CONFIRM.
///
/// Used by seed-start replay when `PutOnDeckAction` skipped retrieval: the
/// selected card stays outside every pile, but Warcry (etc.) still exhausts
/// and Dead Branch still rolls into hand.
pub fn apply_hand_select_confirm_skipped_put_on_deck_retrieval(
    run: &RunState,
) -> SimResult<(RunState, CardInstance)> {
    validate_hand_select_confirm(run)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    let exhaust_before = combat.piles.exhaust_pile.len();
    let selected = confirm_hand_select_skipped_put_on_deck_retrieval(&mut combat)?;
    let exhaust_count = combat
        .piles
        .exhaust_pile
        .len()
        .saturating_sub(exhaust_before);
    // Park the limbo card in the combat limbo pile while Dead Branch reserves
    // new instance IDs. Otherwise max_authoritative_card_instance_id ignores the
    // removed selected card and Dead Branch can reuse its CardId, later failing
    // validation when the limbo card re-enters discard at end of turn.
    combat.piles.limbo.push(selected);
    apply_dead_branch_for_exhaust_count(&mut next, &mut combat, exhaust_count)?;
    let selected = combat.piles.limbo.pop().ok_or(SimError::InvalidState(
        "skipped put-on-deck limbo card missing after Dead Branch settlement",
    ))?;
    next.combat = Some(combat);
    Ok((next, selected))
}

/// Like [`apply_hand_select_confirm_skipped_put_on_deck_retrieval`], but leave
/// Time Warp's forced end-turn queued. CommunicationMod can publish the
/// 12th-card Warcry CONFIRM with the hand still held (FIDL01365).
pub fn apply_hand_select_confirm_skipped_put_on_deck_retrieval_without_time_warp_end(
    run: &RunState,
) -> SimResult<(RunState, CardInstance)> {
    validate_hand_select_confirm(run)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    let exhaust_before = combat.piles.exhaust_pile.len();
    let selected = confirm_hand_select_skipped_put_on_deck_retrieval_with_time_warp_policy(
        &mut combat,
        false,
    )?;
    let exhaust_count = combat
        .piles
        .exhaust_pile
        .len()
        .saturating_sub(exhaust_before);
    combat.piles.limbo.push(selected);
    apply_dead_branch_for_exhaust_count(&mut next, &mut combat, exhaust_count)?;
    let selected = combat.piles.limbo.pop().ok_or(SimError::InvalidState(
        "skipped put-on-deck limbo card missing after Dead Branch settlement",
    ))?;
    next.combat = Some(combat);
    Ok((next, selected))
}

/// Confirm an ordinary Burning Pact exhaust select whose selected-card
/// retrieval was skipped by the target game's selection-screen update.
///
/// The returned card is deliberately kept outside every combat pile. The
/// verifier places it in `pending_hidden_hand_card_until_end_turn` only after
/// the observed post-CONFIRM frame matches this candidate exactly.
pub fn apply_exhaust_select_confirm_skipped_burning_pact_retrieval(
    run: &RunState,
) -> SimResult<(RunState, CardInstance)> {
    apply_exhaust_select_confirm_skipped_burning_pact_retrieval_with_time_warp_policy(run, true)
}

/// Variant used by the verifier's pre-forced-END publication candidate.
pub fn apply_exhaust_select_confirm_skipped_burning_pact_retrieval_without_time_warp_end(
    run: &RunState,
) -> SimResult<(RunState, CardInstance)> {
    apply_exhaust_select_confirm_skipped_burning_pact_retrieval_with_time_warp_policy(run, false)
}

fn apply_exhaust_select_confirm_skipped_burning_pact_retrieval_with_time_warp_policy(
    run: &RunState,
    settle_time_warp: bool,
) -> SimResult<(RunState, CardInstance)> {
    validate_exhaust_select_confirm(run)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    let selected = confirm_burning_pact_select_skipped_retrieval_with_time_warp_policy(
        &mut combat,
        settle_time_warp,
    )?;
    // Source exhaust already queues Dead Branch inside the combat confirm
    // (same handled count as a normal Burning Pact CONFIRM). Re-applying from
    // the exhaust-pile delta minted a second card while the selected card was
    // still unretrieved (FIDL01534 Headbutt vs Bloodletting).
    let next = settle_run_after_select_confirm(next, combat)?;
    Ok((next, selected))
}

/// Confirm Gambling Chip / Gambler's Brew select when selected-card retrieval
/// and replacement draws were skipped by the target selection-screen update.
///
/// Selected cards are parked in `pending_hidden_hand_card_until_end_turn` and
/// stay outside every serialized pile until a non-empty end-turn discard.
pub fn apply_exhaust_select_confirm_skipped_gambling_chip_retrieval(
    run: &RunState,
) -> SimResult<RunState> {
    validate_exhaust_select_confirm(run)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    confirm_gambling_chip_select_skipped_retrieval(&mut combat)?;
    settle_run_after_select_confirm(next, combat)
}

/// Confirm an ordinary Recycle select whose selected-card retrieval was skipped
/// by the target game's selection-screen update. The selected card remains
/// outside combat piles and is returned for verifier-local end-turn parking.
pub fn apply_exhaust_select_confirm_skipped_recycle_retrieval(
    run: &RunState,
) -> SimResult<(RunState, CardInstance)> {
    validate_exhaust_select_confirm(run)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    let selected = confirm_recycle_select_skipped_retrieval(&mut combat)?;
    let next = settle_run_after_select_confirm(next, combat)?;
    Ok((next, selected))
}

pub fn apply_draw_select_choice(run: &RunState, index: usize) -> SimResult<RunState> {
    validate_draw_select_choice(run, index)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    choose_draw_select(&mut combat, index)?;

    // Secret Technique/Weapon close their one-card search immediately. Scry
    // keeps the selected top card and its queued card effects behind the
    // explicit CONFIRM boundary.
    if combat
        .draw_select()
        .is_some_and(|select| select.purpose != crate::combat::DrawSelectPurpose::Scry)
    {
        let exhaust_before = combat.piles.exhaust_pile.len();
        let handled_dead_branch_count = confirm_draw_select(&mut combat)?;
        let exhaust_count = combat
            .piles
            .exhaust_pile
            .len()
            .saturating_sub(exhaust_before);
        apply_dead_branch_for_exhaust_count(
            &mut next,
            &mut combat,
            exhaust_count.saturating_sub(handled_dead_branch_count),
        )?;
    }
    next.combat = Some(combat);
    Ok(next)
}

/// Close Secret Technique / Secret Weapon without retrieving the selected card.
pub fn apply_draw_select_choice_skipped_retrieval(
    run: &RunState,
    index: usize,
) -> SimResult<RunState> {
    validate_draw_select_choice(run, index)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    choose_draw_select(&mut combat, index)?;
    let exhaust_before = combat.piles.exhaust_pile.len();
    let handled_dead_branch_count =
        crate::combat::confirm_draw_select_skipped_retrieval(&mut combat)?;
    let exhaust_count = combat
        .piles
        .exhaust_pile
        .len()
        .saturating_sub(exhaust_before);
    apply_dead_branch_for_exhaust_count(
        &mut next,
        &mut combat,
        exhaust_count.saturating_sub(handled_dead_branch_count),
    )?;
    next.combat = Some(combat);
    Ok(next)
}

pub fn apply_draw_select_confirm(run: &RunState) -> SimResult<RunState> {
    validate_draw_select_confirm(run)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    let exhaust_before = combat.piles.exhaust_pile.len();
    let handled_dead_branch_count = confirm_draw_select(&mut combat)?;
    let exhaust_count = combat
        .piles
        .exhaust_pile
        .len()
        .saturating_sub(exhaust_before);
    apply_dead_branch_for_exhaust_count(
        &mut next,
        &mut combat,
        exhaust_count.saturating_sub(handled_dead_branch_count),
    )?;
    next.combat = Some(combat);
    Ok(next)
}

/// Close Headbutt without retrieving the selected discard card.
pub fn apply_discard_select_choice_skipped_retrieval(
    run: &RunState,
    index: usize,
) -> SimResult<RunState> {
    validate_discard_select_choice(run, index)?;
    let mut next = run.clone();
    if next
        .combat
        .as_ref()
        .and_then(CombatState::discard_select)
        .map(|select| select.purpose)
        != Some(DiscardSelectPurpose::HeadbuttPutOnDraw)
    {
        return Err(SimError::IllegalAction(
            "skipped discard retrieval requires Headbutt discard select",
        ));
    }
    let mut combat = next.combat.take().expect("validated combat");
    choose_discard_select(&mut combat, index)?;
    let exhaust_before = combat.piles.exhaust_pile.len();
    let handled_dead_branch_count = confirm_headbutt_select_skipped_retrieval(&mut combat)?;
    let exhaust_count = combat
        .piles
        .exhaust_pile
        .len()
        .saturating_sub(exhaust_before);
    apply_dead_branch_for_exhaust_count(
        &mut next,
        &mut combat,
        exhaust_count.saturating_sub(handled_dead_branch_count),
    )?;
    flush_pending_player_spikes_damage_if_ready(&mut combat)?;
    next.combat = Some(combat);
    Ok(next)
}

pub fn apply_discard_select_choice(run: &RunState, index: usize) -> SimResult<RunState> {
    validate_discard_select_choice(run, index)?;
    let mut next = run.clone();
    let purpose = next
        .combat
        .as_ref()
        .and_then(CombatState::discard_select)
        .map(|select| select.purpose)
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    let mut combat = next.combat.take().expect("validated combat");
    choose_discard_select(&mut combat, index)?;
    if purpose == DiscardSelectPurpose::HeadbuttPutOnDraw
        || (purpose == DiscardSelectPurpose::LiquidMemoriesReturnToHand
            && combat
                .discard_select()
                .is_some_and(|select| select.max_choices == 1))
    {
        let exhaust_before = combat.piles.exhaust_pile.len();
        let handled_dead_branch_count = confirm_discard_select(&mut combat)?;
        let exhaust_count = combat
            .piles
            .exhaust_pile
            .len()
            .saturating_sub(exhaust_before);
        apply_dead_branch_for_exhaust_count(
            &mut next,
            &mut combat,
            exhaust_count.saturating_sub(handled_dead_branch_count),
        )?;
        flush_pending_player_spikes_damage_if_ready(&mut combat)?;
    }
    next.combat = Some(combat);
    Ok(next)
}

pub fn apply_discard_select_confirm(run: &RunState) -> SimResult<RunState> {
    validate_discard_select_confirm(run)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    let exhaust_before = combat.piles.exhaust_pile.len();
    let handled_dead_branch_count = confirm_discard_select(&mut combat)?;
    let exhaust_count = combat
        .piles
        .exhaust_pile
        .len()
        .saturating_sub(exhaust_before);
    apply_dead_branch_for_exhaust_count(
        &mut next,
        &mut combat,
        exhaust_count.saturating_sub(handled_dead_branch_count),
    )?;
    flush_pending_player_spikes_damage_if_ready(&mut combat)?;
    next.combat = Some(combat);
    Ok(next)
}

pub fn apply_exhaust_select_choice(run: &RunState, index: usize) -> SimResult<RunState> {
    validate_exhaust_select_choice(run, index)?;
    let mut next = run.clone();
    let purpose = next
        .combat
        .as_ref()
        .and_then(CombatState::exhaust_select)
        .map(|select| select.purpose)
        .expect("validated exhaust select");
    let combat = next.combat.as_mut().expect("validated combat");
    choose_exhaust_select(combat, index)?;
    if purpose == ExhaustSelectPurpose::ExhumeReturnToHand {
        let mut combat = next.combat.take().expect("validated combat");
        let before = combat.clone();
        let exhaust_before = combat.piles.exhaust_pile.len();
        let handled_dead_branch_count =
            confirm_exhaust_select_with_dead_branch_count(&mut combat, true)?;
        let exhaust_count = exhaust_count_for_confirmed_select(&before, &combat, exhaust_before);
        apply_dead_branch_for_exhaust_count(
            &mut next,
            &mut combat,
            exhaust_count.saturating_sub(handled_dead_branch_count),
        )?;
        next.combat = Some(combat);
    }
    Ok(next)
}

/// Resolve a force-played Exhume selection without returning the selected card.
///
/// CommunicationMod can publish a stable post-selection frame after Exhume's
/// source settles but before its selected-card retrieval effect runs. The
/// authoritative core keeps normal Exhume retrieval in `apply_exhaust_select_choice`;
/// this separate entry point is used only by the strict verifier's source-backed
/// skipped-retrieval reconciliation.
pub fn apply_exhaust_select_choice_skipped_exhume(
    run: &RunState,
    index: usize,
) -> SimResult<RunState> {
    validate_exhaust_select_choice(run, index)?;
    let mut next = run.clone();
    let purpose = next
        .combat
        .as_ref()
        .and_then(CombatState::exhaust_select)
        .map(|select| select.purpose)
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    if purpose != ExhaustSelectPurpose::ExhumeReturnToHand {
        return Err(SimError::IllegalAction(
            "skipped retrieval requires ExhumeReturnToHand",
        ));
    }
    let mut combat = next.combat.take().expect("validated combat");
    let before = combat.clone();
    let exhaust_before = combat.piles.exhaust_pile.len();
    choose_exhaust_select(&mut combat, index)?;
    confirm_exhume_select_skipped_return(&mut combat)?;
    let exhaust_count = exhaust_count_for_confirmed_select(&before, &combat, exhaust_before);
    apply_dead_branch_for_exhaust_count(&mut next, &mut combat, exhaust_count)?;
    next.combat = Some(combat);
    Ok(next)
}

/// Confirm an exhaust select without consuming Time Warp's forced end-turn.
/// CommunicationMod can snapshot the post-CONFIRM piles before that END drains
/// the hand (FIDL01358 Burning Pact under Time Warp).
pub fn apply_exhaust_select_confirm_without_time_warp_end(run: &RunState) -> SimResult<RunState> {
    validate_exhaust_select_confirm(run)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    let before = combat.clone();
    let exhaust_before = combat.piles.exhaust_pile.len();
    let handled_dead_branch_count =
        confirm_exhaust_select_with_dead_branch_count(&mut combat, false)?;
    let exhaust_count = exhaust_count_for_confirmed_select(&before, &combat, exhaust_before);
    apply_dead_branch_for_exhaust_count(
        &mut next,
        &mut combat,
        exhaust_count.saturating_sub(handled_dead_branch_count),
    )?;
    next.combat = Some(combat);
    Ok(next)
}

/// Confirm exhaust select, keep Time Warp's forced END pending, and apply
/// Metallicize's pre-discard block (FIDL01694 Burning Pact CONFIRM).
pub fn apply_exhaust_select_confirm_time_warp_metallicize_lag(
    run: &RunState,
) -> SimResult<RunState> {
    let mut next = apply_exhaust_select_confirm_without_time_warp_end(run)?;
    if let Some(combat) = next.combat.as_mut() {
        crate::combat::apply_time_warp_lag_metallicize_keep_hand(combat)?;
    }
    Ok(next)
}

pub fn apply_exhaust_select_confirm(run: &RunState) -> SimResult<RunState> {
    validate_exhaust_select_confirm(run)?;
    let mut next = run.clone();
    let mut combat = next.combat.take().expect("validated combat");
    let before = combat.clone();
    let exhaust_before = combat.piles.exhaust_pile.len();
    let handled_dead_branch_count =
        confirm_exhaust_select_with_dead_branch_count(&mut combat, true)?;
    let exhaust_count = exhaust_count_for_confirmed_select(&before, &combat, exhaust_before);
    apply_dead_branch_for_exhaust_count(
        &mut next,
        &mut combat,
        exhaust_count.saturating_sub(handled_dead_branch_count),
    )?;
    // Burning Pact + Feel No Pain can queue Juggernaut damage that kills the
    // last monster during CONFIRM (15ab4cc step 1102). Unlike PlayCard, select
    // confirms do not go through apply_combat_action_on_run's Won → reward path.
    settle_run_after_select_confirm(next, combat)
}

/// Attach post-select combat and open rewards when CONFIRM left combat Won.
fn settle_run_after_select_confirm(mut next: RunState, combat: CombatState) -> SimResult<RunState> {
    next.store_rng_counter(RunRngStream::CardRandom, &combat.rng.card_random_rng);
    next.player_hp = combat.player.hp;
    next.player_max_hp = combat.player.max_hp;
    let won = combat.phase == CombatPhase::Won;
    next.combat = Some(combat);
    if won {
        enter_combat_reward_for_current_room(&mut next)?;
    }
    Ok(next)
}

fn exhaust_count_for_confirmed_select(
    before: &CombatState,
    after: &CombatState,
    exhaust_before: usize,
) -> usize {
    let Some(select) = before.exhaust_select() else {
        return after
            .piles
            .exhaust_pile
            .len()
            .saturating_sub(exhaust_before);
    };
    if select.purpose != ExhaustSelectPurpose::ExhumeReturnToHand {
        return after
            .piles
            .exhaust_pile
            .len()
            .saturating_sub(exhaust_before);
    }
    let Some(source_card_id) = select.source_card_id else {
        return 0;
    };
    let source_started_in_hand = before
        .piles
        .hand
        .iter()
        .any(|card| card.id == source_card_id);
    let source_started_in_select = select
        .source_card
        .is_some_and(|card| card.id == source_card_id);
    let source_ended_in_exhaust = after
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.id == source_card_id);
    usize::from((source_started_in_hand || source_started_in_select) && source_ended_in_exhaust)
}

pub fn apply_combat_card_reward_choice(run: &RunState, index: usize) -> SimResult<RunState> {
    validate_combat_card_reward_choice(run, index)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    let played_discovery_card_id = matches!(
        combat.decision.as_ref(),
        Some(CombatDecisionState::DiscoveryCardReward {
            source_card: Some(_),
            ..
        })
    )
    .then(|| combat.next_card_instance_id())
    .transpose()?
    .map(CardId::new);
    let decision = combat
        .decision
        .take()
        .ok_or(SimError::IllegalAction("no combat card reward is open"))?;
    match decision {
        CombatDecisionState::PotionCardReward {
            choices,
            reward_kind,
        } => {
            let card_id = CardId::new(combat.next_card_instance_id()?);
            settle_potion_card_reward_rng(combat, reward_kind, true);
            combat.pending_potion_card_reward_settlement =
                Some(PendingPotionCardRewardSettlement {
                    reward_kind,
                    generations_remaining: POTION_DISCOVERY_POST_PICKED_HIDDEN_GENERATIONS as u32,
                    end_turns_remaining: 2,
                });
            let choice = choices[index];
            let mut card = CardInstance::combat_generated(card_id, choice.content_id, 0);
            card.temp_cost_turn_only = true;
            // CommunicationMod exposes potion-generated cards after the cards that
            // were already in hand, unlike Toolbox and Discovery rewards.
            combat.piles.hand.push(card);
            crate::relic::apply_potion_use_relics_to_combat(combat)?;
            next.player_hp = combat.player.hp;
            next.card_random_rng_counter = combat.rng.card_random_rng.counter();
        }
        CombatDecisionState::DiscoveryCardReward {
            choices,
            source_card,
            source_card_force_exhaust,
            source_card_play_top,
            pending_actions,
        } => {
            let card_id = if let Some(card_id) = played_discovery_card_id {
                card_id
            } else {
                CardId::new(combat.next_card_instance_id()?)
            };
            // DiscoveryAction.update generates a discarded three-card offer at
            // the start of every post-select pulse. SuperFastMode leaves two
            // pulses when Awakened One is in the fight and 6+ cards remain
            // after the source left (FIDL01561 Sludge Void). The same 6+
            // remaining-hand shape against other encounters stays one pulse
            // (FIDL01309 Wild Strike Wound; FIDL01248 / FIDL01255). Remaining
            // hands of 3–5 against Awakened One stay one pulse while another
            // enemy is still alive (FIDL01665 Cultist+AO). A five-card remaining
            // hand against solo living Awakened One still takes two pulses
            // (FIDL01357 leftover Sludge Void at draw index 7, not 4).
            // Two remaining cards with a status still take two pulses (FIDL01357
            // Wild Strike Wound after Defend+Dazed). Two remaining attacks/skills
            // stay one pulse (FIDL01614 Infernal Blade after Strike+Infernal Blade).
            // Another Discovery still in hand still needs two (FIDL01630 first pick). A
            // Magnetism-generated Discovery played among the first two cards
            // of the turn leaves two pulses when the remaining hand is small
            // or another Magnetism-generated card is still in hand (FIDL01787
            // Transmutation vs Enlightenment; later Magnetism Discoveries on
            // the same run). Remaining hand of 5 with no other generated card
            // stays one pulse (FIDL01582 The Bomb, not Blind). A lone early-turn retrieve
            // from a 6+ card hand settles in one pulse (FIDL01787 Writhing Mass
            // Flash of Steel).
            // The same source later in the turn stays one pulse (FIDL01255
            // player-played Discovery, FIDL01623). Havoc PlayTop force-exhausts
            // the source before DiscoveryAction; that path keeps pulsing through
            // leftover ExhaustSpecificCardAction / ShowCard settlement and burns
            // six discarded generations (FIDL01614 Infernal Blade after
            // Havoc-Discovery). Mayhem PlayTop does not force-exhaust, but
            // leftover SuperFastMode already drained the extra Magnetism pulse
            // before CHOOSE, so early-turn Magnetism retrieve stays one pulse
            // (FIDL01255). Hand-played Discovery on the same FIDL01614 run stays
            // one pulse (steps 325, 366) unless Hex is up with two living
            // enemies, which leaves two pulses so the parked Hex Dazed insert
            // uses the observed addToRandomSpot index (FIDL01614 Chosen+Cultist).
            // Solo Chosen Hex stays one pulse (FIDL01561).
            let generations = discovery_post_select_generations(
                combat,
                source_card.as_ref(),
                source_card_force_exhaust,
                source_card_play_top,
            );
            burn_all_discovery_card_choice_generations(
                &mut combat.rng.card_random_rng,
                3,
                generations,
            );
            let choice = choices[index];
            let mut card = CardInstance::combat_generated(card_id, choice.content_id, 0);
            card.temp_cost_turn_only = true;
            // DiscoveryAction adds the generated card after the cards already in hand.
            combat.piles.hand.push(card);
            // card.use() follow-ups queued behind DiscoveryAction (for example
            // Hex's Dazed insertion) resolve after the selected card is
            // retrieved but before UseCardAction settles the Discovery source.
            if !pending_actions.is_empty() {
                let transition =
                    crate::combat::transition::process_internal_queue(combat, pending_actions)?;
                *combat = transition.state;
            }
            close_discovery_source_card_with_force_exhaust(
                combat,
                source_card,
                source_card_force_exhaust,
            )?;
            combat.play_top_force_exhaust_active = false;
            next.card_random_rng_counter = combat.rng.card_random_rng.counter();
        }
        CombatDecisionState::ToolboxCardReward { choices } => {
            let choice = choices[index];
            let card_id = CardId::new(combat.next_card_instance_id()?);
            combat.piles.hand.insert(
                0,
                CardInstance {
                    combat_only: true,
                    ..CardInstance::new(card_id, choice.content_id)
                },
            );
            next.card_random_rng_counter = combat.rng.card_random_rng.counter();
            crate::relic::settle_pending_opening_combat_actions(combat)?;
            crate::relic::settle_pending_start_of_turn_relic_actions(combat)?;
            next.card_random_rng_counter = combat.rng.card_random_rng.counter();
        }
        CombatDecisionState::NilrysCodexCardReward { choices } => {
            let choice = choices[index];
            // Shuffle the chosen card into a random draw-pile spot (combat-only).
            // Do not finish end-turn here: CommunicationMod captures the closed
            // reward with the pre-discard hand still visible. The remainder of
            // end-turn resumes on the next combat command (see replay) or when
            // `end_player_turn` is invoked with `resume_end_turn_after_nilrys_codex`.
            crate::combat::transition::add_generated_card_to_draw_pile_random_spot_public(
                combat,
                choice.content_id,
            )?;
            next.card_random_rng_counter = combat.rng.card_random_rng.counter();
            // The second Nilry offer closes the paused end-turn queue itself;
            // no additional END command is emitted before the monster turn.
            if combat.nilrys_codex_end_turn_stage == 3 {
                *combat = crate::combat::turn::end_player_turn(combat)?;
            } else {
                // callEndOfTurnActions queued card autoplays (Regret/Burn)
                // continue after the first Codex screen closes and before the
                // leftover second EndTurn / Combust (FIDL01597 CHOOSE 470).
                crate::combat::hand::resolve_end_of_turn_playing_cards_for_time_warp_lag(combat)?;
                next.player_hp = combat.player.hp;
            }
        }
        other => {
            combat.decision = Some(other);
            return Err(SimError::IllegalAction("no combat card reward is open"));
        }
    }
    combat.activate_next_queued_decision_if_idle();
    Ok(next)
}

/// Close a Discovery card reward without retrieving the selected card.
///
/// If `DiscoveryAction` completes on its opening update (`tickDuration`
/// finishes after `customCombatOpen`), the card-reward screen stays up but no
/// resumed update runs. `generateCardChoices` therefore does not burn a
/// discarded post-select generation, and `ShowCardAndAddToHandAction` is never
/// queued. `UseCardAction` still settles the Discovery source after the screen
/// closes.
pub fn apply_combat_card_reward_choice_skipped_discovery_retrieval(
    run: &RunState,
    index: usize,
) -> SimResult<RunState> {
    validate_combat_card_reward_choice(run, index)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    match combat.decision.take() {
        Some(CombatDecisionState::DiscoveryCardReward {
            source_card,
            source_card_force_exhaust,
            pending_actions,
            ..
        }) => {
            if !pending_actions.is_empty() {
                let transition =
                    crate::combat::transition::process_internal_queue(combat, pending_actions)?;
                *combat = transition.state;
            }
            close_discovery_source_card_with_force_exhaust(
                combat,
                source_card,
                source_card_force_exhaust,
            )?;
            combat.play_top_force_exhaust_active = false;
            next.card_random_rng_counter = combat.rng.card_random_rng.counter();
            combat.activate_next_queued_decision_if_idle();
            Ok(next)
        }
        other => {
            combat.decision = other;
            Err(SimError::IllegalAction(
                "skipped Discovery retrieval requires an open Discovery card reward",
            ))
        }
    }
}

/// Close a Toolbox card reward without retrieving the selected card.
///
/// Opening-combat actions queued behind Toolbox (hand draw, start-of-combat
/// block, Happy Flower energy) still settle. The chosen colorless card is never
/// created when the reward screen closes before ShowCardAndAddToHandAction.
pub fn apply_combat_card_reward_choice_skipped_toolbox_retrieval(
    run: &RunState,
    index: usize,
) -> SimResult<RunState> {
    validate_combat_card_reward_choice(run, index)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    match combat.decision.take() {
        Some(CombatDecisionState::ToolboxCardReward { .. }) => {
            next.card_random_rng_counter = combat.rng.card_random_rng.counter();
            crate::relic::settle_pending_opening_combat_actions(combat)?;
            crate::relic::settle_pending_start_of_turn_relic_actions(combat)?;
            next.card_random_rng_counter = combat.rng.card_random_rng.counter();
            combat.activate_next_queued_decision_if_idle();
            Ok(next)
        }
        other => {
            combat.decision = other;
            Err(SimError::IllegalAction(
                "skipped Toolbox retrieval requires an open Toolbox card reward",
            ))
        }
    }
}

pub fn apply_combat_card_reward_skip(run: &RunState) -> SimResult<RunState> {
    validate_combat_card_reward_skip(run)?;
    let mut next = run.clone();
    let combat = next.combat.as_mut().expect("validated combat");
    match combat.decision.take() {
        Some(CombatDecisionState::PotionCardReward { reward_kind, .. }) => {
            settle_potion_card_reward_rng(combat, reward_kind, false);
            combat.pending_potion_card_reward_settlement =
                Some(PendingPotionCardRewardSettlement {
                    reward_kind,
                    generations_remaining: POTION_DISCOVERY_POST_PICKED_HIDDEN_GENERATIONS as u32,
                    end_turns_remaining: 2,
                });
            crate::relic::apply_potion_use_relics_to_combat(combat)?;
            next.player_hp = combat.player.hp;
            next.card_random_rng_counter = combat.rng.card_random_rng.counter();
            combat.activate_next_queued_decision_if_idle();
            Ok(next)
        }
        Some(CombatDecisionState::NilrysCodexCardReward { .. }) => {
            // First-offer SKIP leaves end-turn queued (FIDL01772 leftover
            // EndTurn). Second-offer SKIP (stage 3) continues that queue:
            // no insert, then monsters and the next hand.
            if combat.nilrys_codex_end_turn_stage == 3 {
                let finished = crate::combat::turn::end_player_turn(combat)?;
                next.player_hp = finished.player.hp;
                next.player_max_hp = finished.player.max_hp;
                next.card_random_rng_counter = finished.rng.card_random_rng.counter();
                next.combat = Some(finished);
                if let Some(combat) = next.combat.as_mut() {
                    combat.activate_next_queued_decision_if_idle();
                }
                return Ok(next);
            }
            crate::combat::hand::resolve_end_of_turn_playing_cards_for_time_warp_lag(combat)?;
            next.player_hp = combat.player.hp;
            next.card_random_rng_counter = combat.rng.card_random_rng.counter();
            combat.activate_next_queued_decision_if_idle();
            Ok(next)
        }
        other => {
            combat.decision = other;
            Err(SimError::IllegalAction(
                "no skippable combat card reward is open",
            ))
        }
    }
}

/// Post-select `generateCardChoices` pulses after a Havoc / Distilled Chaos
/// PlayTop Discovery. PlayTop queues `ExhaustSpecificCardAction` on top of
/// `DiscoveryAction`; under SuperFastMode the resumed action keeps generating
/// discarded offers until that leftover settlement finishes.
const PLAY_TOP_FORCE_EXHAUST_DISCOVERY_POST_SELECT_GENERATIONS: usize = 6;

fn discovery_post_select_generations(
    combat: &CombatState,
    source_card: Option<&CardInstance>,
    source_card_force_exhaust: bool,
    source_card_play_top: bool,
) -> usize {
    if source_card_force_exhaust {
        return PLAY_TOP_FORCE_EXHAUST_DISCOVERY_POST_SELECT_GENERATIONS;
    }
    let fighting_awakened_one = combat
        .monsters
        .iter()
        .any(|monster| monster.content_id == AWAKENED_ONE_ID);
    let living_monsters = combat
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .count();
    let hexed_with_two_living = combat.player.powers.hex > 0 && living_monsters >= 2;
    let another_discovery_in_hand = combat
        .piles
        .hand
        .iter()
        .any(|card| matches!(card.content_id, DISCOVERY_ID | DISCOVERY_PLUS_ID));
    let another_magnetism_card_in_hand = combat
        .piles
        .hand
        .iter()
        .any(|card| card.magnetism_generated);
    let early_magnetism_generated_source = combat.player.powers.magnetism > 0
        && source_card.is_some_and(|card| card.magnetism_generated)
        && combat.relic_counters.cards_played_this_turn <= 2
        && !source_card_play_top;
    let remaining_status = combat.piles.hand.iter().any(|card| {
        get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == CardType::Status)
    });
    let tiny_remaining_hand_with_status = combat.piles.hand.len() <= 2 && remaining_status;
    if another_discovery_in_hand
        || hexed_with_two_living
        || (fighting_awakened_one
            && (combat.piles.hand.len() >= 6
                || (combat.piles.hand.len() >= 5 && living_monsters <= 1)))
        || tiny_remaining_hand_with_status
        || (early_magnetism_generated_source
            && (combat.piles.hand.len() < 5 || another_magnetism_card_in_hand))
    {
        2
    } else {
        1
    }
}

fn settle_potion_card_reward_rng(
    combat: &mut CombatState,
    kind: PotionCardRewardKind,
    picked: bool,
) {
    let rng = &mut combat.rng.card_random_rng;
    let (mut hidden_generations, settle_draws) = if picked {
        (
            DISCOVERY_ACTION_PICKED_HIDDEN_GENERATIONS,
            DISCOVERY_ACTION_PICKED_SCREEN_SETTLE_DRAWS,
        )
    } else {
        (
            DISCOVERY_ACTION_SKIPPED_HIDDEN_GENERATIONS,
            DISCOVERY_ACTION_SKIPPED_SCREEN_SETTLE_DRAWS,
        )
    };
    if picked && kind == PotionCardRewardKind::Colorless {
        hidden_generations = COLORLESS_DISCOVERY_ACTION_PICKED_HIDDEN_GENERATIONS;
    }
    match kind {
        PotionCardRewardKind::Attack => {
            burn_discovery_card_choice_generations(rng, CardType::Attack, 3, hidden_generations);
            burn_discovery_card_choice_draws(rng, CardType::Attack, settle_draws);
        }
        PotionCardRewardKind::Skill => {
            burn_discovery_card_choice_generations(rng, CardType::Skill, 3, hidden_generations);
            burn_discovery_card_choice_draws(rng, CardType::Skill, settle_draws);
        }
        PotionCardRewardKind::Power => {
            burn_discovery_card_choice_generations(rng, CardType::Power, 3, hidden_generations);
            burn_discovery_card_choice_draws(rng, CardType::Power, settle_draws);
        }
        PotionCardRewardKind::Colorless => {
            burn_colorless_discovery_card_choice_generations(rng, 3, hidden_generations);
            burn_colorless_discovery_card_choice_draws(rng, settle_draws);
        }
    }
}

pub(crate) fn settle_pending_potion_card_reward_rng(combat: &mut CombatState) -> SimResult<()> {
    let Some(mut pending) = combat.pending_potion_card_reward_settlement.take() else {
        return Ok(());
    };
    if pending.end_turns_remaining > 1 {
        pending.end_turns_remaining -= 1;
        combat.pending_potion_card_reward_settlement = Some(pending);
        return Ok(());
    }
    let generations = usize::try_from(pending.generations_remaining).map_err(|_| {
        SimError::InvalidState("pending potion card reward generations exceed usize")
    })?;
    match pending.reward_kind {
        PotionCardRewardKind::Attack => burn_discovery_card_choice_generations(
            &mut combat.rng.card_random_rng,
            CardType::Attack,
            3,
            generations,
        ),
        PotionCardRewardKind::Skill => burn_discovery_card_choice_generations(
            &mut combat.rng.card_random_rng,
            CardType::Skill,
            3,
            generations,
        ),
        PotionCardRewardKind::Power => burn_discovery_card_choice_generations(
            &mut combat.rng.card_random_rng,
            CardType::Power,
            3,
            generations,
        ),
        PotionCardRewardKind::Colorless => burn_colorless_discovery_card_choice_generations(
            &mut combat.rng.card_random_rng,
            3,
            generations,
        ),
    }
    match pending.reward_kind {
        PotionCardRewardKind::Attack => burn_discovery_card_choice_draws(
            &mut combat.rng.card_random_rng,
            CardType::Attack,
            POTION_DISCOVERY_POST_PICKED_SCREEN_SETTLE_DRAWS,
        ),
        PotionCardRewardKind::Skill => burn_discovery_card_choice_draws(
            &mut combat.rng.card_random_rng,
            CardType::Skill,
            POTION_DISCOVERY_POST_PICKED_SCREEN_SETTLE_DRAWS,
        ),
        PotionCardRewardKind::Power => burn_discovery_card_choice_draws(
            &mut combat.rng.card_random_rng,
            CardType::Power,
            POTION_DISCOVERY_POST_PICKED_SCREEN_SETTLE_DRAWS,
        ),
        PotionCardRewardKind::Colorless => burn_colorless_discovery_card_choice_draws(
            &mut combat.rng.card_random_rng,
            POTION_DISCOVERY_POST_PICKED_SCREEN_SETTLE_DRAWS,
        ),
    }
    Ok(())
}

fn distilled_chaos_target(
    combat: &mut CombatState,
    target: TargetRequirement,
) -> SimResult<Option<MonsterId>> {
    if target != TargetRequirement::Enemy {
        return Ok(None);
    }

    let living = combat
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster.id)
        .collect::<Vec<_>>();
    if living.is_empty() {
        return Err(SimError::IllegalAction("no living monsters to target"));
    }

    let index = combat
        .rng
        .card_random_rng
        .random_int((living.len() - 1) as i32) as usize;
    Ok(Some(living[index]))
}

fn randomize_playable_hand_costs_for_snecko_oil(combat: &mut CombatState, rng: &mut StsRng) {
    for card in &mut combat.piles.hand {
        let Some(definition) = get_card_definition(card.content_id) else {
            continue;
        };
        if definition.keywords.unplayable || definition.cost < 0 {
            continue;
        }
        card.temp_cost = Some(rng.random_int(3) as u8);
    }
}

fn potion_multiplier(run: &RunState) -> i32 {
    if run.relics.contains(&crate::Relic::SacredBark) {
        2
    } else {
        1
    }
}

fn checked_potion_stat_gain(current: i32, base_amount: i32, multiplier: i32) -> SimResult<i32> {
    base_amount
        .checked_mul(multiplier)
        .and_then(|amount| current.checked_add(amount))
        .ok_or(SimError::InvalidState(
            "combat potion stat gain overflows i32",
        ))
}

fn blood_potion_heal(max_hp: i32, multiplier: i32) -> SimResult<i32> {
    let heal =
        i64::from(max_hp) * i64::from(BLOOD_POTION_HEAL_PERCENT) * i64::from(multiplier) / 100;
    i32::try_from(heal).map_err(|_| SimError::InvalidState("Blood Potion heal exceeds i32"))
}

pub fn apply_potion_action(run: &RunState, action: RunAction) -> SimResult<RunState> {
    validate_potion_action(run, action)?;

    let mut next = run.clone();
    match action {
        RunAction::UsePotion { slot, target } => {
            let entropic_brew_had_open_slot = next.potion_at_slot(slot)
                == Some(Potion::EntropicBrew)
                && next.open_potion_slots() > 0;
            let potion = next.take_potion_slot(slot)?;
            if potion == Potion::Cultist {
                if let Some(CombatDecisionState::ExhaustSelect { state, .. }) = next
                    .combat
                    .as_mut()
                    .and_then(|combat| combat.decision.as_mut())
                {
                    state.interrupted_by_cultist_potion = true;
                }
            }
            let multiplier = potion_multiplier(&next);
            let mut defer_potion_use_relics = false;
            let mut victory_healing_applied = false;
            match potion {
                Potion::Fire => {
                    let target = target.expect("validated fire potion target");
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let killed = {
                        let monster = combat
                            .monsters
                            .iter_mut()
                            .find(|monster| monster.id == target)
                            .expect("validated potion target");
                        let hp_damage = deal_unmodified_damage_to_monster(
                            monster,
                            FIRE_POTION_DAMAGE * multiplier,
                        );
                        wake_lagavulin_on_damage(monster, hp_damage);
                        !monster.alive
                    };
                    if killed {
                        apply_monster_death_hooks(combat, target)?;
                    }
                    if combat.monsters.iter().all(|monster| !monster.alive) {
                        combat.phase = CombatPhase::Won;
                    }
                }
                Potion::Block => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.block = checked_potion_stat_gain(
                        combat.player.block,
                        BLOCK_POTION_BLOCK,
                        multiplier,
                    )?;
                }
                Potion::Fear => {
                    let target = target.expect("validated fear potion target");
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let monster = combat
                        .monsters
                        .iter_mut()
                        .find(|monster| monster.id == target)
                        .expect("validated potion target");
                    apply_monster_vulnerable(
                        &mut monster.powers,
                        FEAR_POTION_VULNERABLE * multiplier,
                    )?;
                }
                Potion::Blood => {
                    if let Some(combat) = next.combat.as_mut() {
                        let heal = blood_potion_heal(combat.player.max_hp, multiplier)?;
                        crate::relic::heal_combat_player_with_relics(combat, heal)?;
                    } else {
                        let heal = blood_potion_heal(next.player_max_hp, multiplier)?;
                        next.heal_player(heal)?;
                    }
                }
                Potion::Ancient => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.artifact = checked_potion_stat_gain(
                        combat.player.powers.artifact,
                        ANCIENT_POTION_ARTIFACT,
                        multiplier,
                    )?;
                }
                Potion::HeartOfIron => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.metallicize = checked_potion_stat_gain(
                        combat.player.powers.metallicize,
                        HEART_OF_IRON_METALLICIZE,
                        multiplier,
                    )?;
                }
                Potion::Cultist => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.ritual = checked_potion_stat_gain(
                        combat.player.powers.ritual,
                        CULTIST_POTION_RITUAL,
                        multiplier,
                    )?;
                }
                Potion::Dexterity => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.dexterity = checked_potion_stat_gain(
                        combat.player.powers.dexterity,
                        DEXTERITY_POTION_DEXTERITY,
                        multiplier,
                    )?;
                }
                Potion::Energy => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.energy = combat
                        .player
                        .energy
                        .checked_add(ENERGY_POTION_ENERGY * multiplier)
                        .ok_or(SimError::InvalidState(
                            "Energy Potion energy gain overflows i32",
                        ))?;
                }
                Potion::EssenceOfSteel => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.plated_armor = checked_potion_stat_gain(
                        combat.player.powers.plated_armor,
                        ESSENCE_OF_STEEL_PLATED_ARMOR,
                        multiplier,
                    )?;
                }
                Potion::Explosive => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let targets = combat
                        .monsters
                        .iter()
                        .filter(|monster| monster.alive)
                        .map(|monster| monster.id)
                        .collect::<Vec<_>>();
                    for target in targets {
                        let killed = {
                            let monster = combat
                                .monsters
                                .iter_mut()
                                .find(|monster| monster.id == target)
                                .expect("target was collected from combat");
                            let hp_damage = deal_unmodified_damage_to_monster(
                                monster,
                                EXPLOSIVE_POTION_DAMAGE * multiplier,
                            );
                            wake_lagavulin_on_damage(monster, hp_damage);
                            !monster.alive
                        };
                        if killed {
                            apply_monster_death_hooks(combat, target)?;
                        }
                    }
                    if combat.monsters.iter().all(|monster| !monster.alive) {
                        combat.phase = CombatPhase::Won;
                    }
                }
                Potion::LiquidBronze => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.thorns = checked_potion_stat_gain(
                        combat.player.powers.thorns,
                        LIQUID_BRONZE_THORNS,
                        multiplier,
                    )?;
                }
                Potion::Regen => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.regen = checked_potion_stat_gain(
                        combat.player.powers.regen,
                        REGEN_POTION_REGEN,
                        multiplier,
                    )?;
                }
                Potion::Strength => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.powers.strength = checked_potion_stat_gain(
                        combat.player.powers.strength,
                        STRENGTH_POTION_STRENGTH,
                        multiplier,
                    )?;
                }
                Potion::Flex => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.player.temp_strength = checked_potion_stat_gain(
                        combat.player.temp_strength,
                        FLEX_POTION_TEMP_STRENGTH,
                        multiplier,
                    )?;
                }
                Potion::Speed => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let dexterity = checked_potion_stat_gain(
                        combat.player.powers.dexterity,
                        SPEED_POTION_TEMP_DEXTERITY,
                        multiplier,
                    )?;
                    let temp_dexterity = checked_potion_stat_gain(
                        combat.player.temp_dexterity,
                        SPEED_POTION_TEMP_DEXTERITY,
                        multiplier,
                    )?;
                    combat.player.powers.dexterity = dexterity;
                    combat.player.temp_dexterity = temp_dexterity;
                }
                Potion::Swift => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    player_draw_cards(combat, SWIFT_POTION_DRAW * multiplier as usize)?;
                }
                Potion::SneckoOil => {
                    let mut rng = next.card_random_rng();
                    let combat = next.combat.as_mut().expect("validated combat state");
                    player_draw_cards(combat, SNECKO_OIL_DRAW * multiplier as usize)?;
                    randomize_playable_hand_costs_for_snecko_oil(combat, &mut rng);
                    combat.rng.card_random_rng = rng.clone();
                    next.card_random_rng_counter = rng.counter();
                }
                Potion::SmokeBomb => {
                    if matches!(
                        current_room_kind(&next),
                        Some(RoomKind::Combat | RoomKind::Elite)
                    ) {
                        // The target's normal room escape path constructs and
                        // then hides the ordinary gold reward. Its amount is
                        // not observable on the empty reward screen, but the
                        // treasureRng draw affects the next combat reward.
                        let mut treasure_rng = next.rng_for_stream(RunRngStream::Treasure);
                        if current_room_kind(&next) == Some(RoomKind::Elite) {
                            let _ = target_elite_combat_gold(&mut treasure_rng);
                        } else {
                            let _ = target_normal_combat_gold(&mut treasure_rng);
                        }
                        next.store_rng_counter(RunRngStream::Treasure, &treasure_rng);

                        // AbstractRoom.addPotionToRewards still performs the
                        // ordinary drop roll before Smoke Bomb hides the
                        // reward screen. A hit also selects a potion, even
                        // though that potion is never displayed. Smoke Bomb
                        // marks the room as smoked; it does not mark the
                        // monsters as escaped, so the source's normal 40%
                        // base chance still applies here.
                    }

                    if matches!(
                        current_room_kind(&next),
                        Some(RoomKind::Combat | RoomKind::Elite | RoomKind::Event)
                    ) {
                        let mut potion_rng = next.rng_for_stream(RunRngStream::Potion);
                        let reward_count = if current_room_kind(&next) == Some(RoomKind::Elite) {
                            2
                        } else {
                            1
                        };
                        let potion_count = next.potions.len();
                        let potion_capacity = next.potion_capacity();
                        let has_white_beast_statue = next.relics.contains(&Relic::WhiteBeastStatue);
                        let _hidden_potion_offer = target_potion_reward_offer(
                            &mut potion_rng,
                            &mut next.potion_chance,
                            reward_count,
                            potion_count,
                            potion_capacity,
                            has_white_beast_statue,
                        )?;
                        next.store_rng_counter(RunRngStream::Potion, &potion_rng);
                    }
                    let mut combat = next.combat.take().expect("validated combat state");
                    combat.phase = CombatPhase::Won;
                    apply_burning_blood(&mut combat)?;
                    next.player_hp = combat.player.hp;
                    next.player_max_hp = combat.player.max_hp;
                    next.pending_event_combat_gold_offer = 0;
                    next.pending_event_combat_gold_bonus = 0;
                    next.pending_event_combat_elite_gold = false;
                    next.pending_event_combat_relic_offer = None;
                    next.reward = None;
                    next.phase = RunPhase::Idle;
                }
                Potion::Elixir => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    open_exhaust_select(combat)?;
                }
                Potion::BlessingOfTheForge => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    for card in &mut combat.piles.hand {
                        if let Some(upgraded) = upgrade_card_instance(*card)? {
                            *card = upgraded;
                        }
                    }
                }
                Potion::Duplication => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    combat.duplication_potion_stacks =
                        checked_potion_stat_gain(combat.duplication_potion_stacks, 1, multiplier)?;
                    combat.duplication_potion_pending = false;
                }
                Potion::DistilledChaos => {
                    let mut combat = next.combat.take().expect("validated combat state");
                    // DistilledChaosPotion constructs every PlayTopCardAction
                    // up front and chooses a random living monster for each
                    // action, even when the eventual top card has no target.
                    // Those cardRandomRng draws precede all played-card effects.
                    let queued_targets = (0..3 * multiplier)
                        .map(|_| distilled_chaos_target(&mut combat, TargetRequirement::Enemy))
                        .collect::<SimResult<Vec<_>>>()?;
                    // Each PlayTopCardAction moves its selected card to limbo,
                    // but the queued cards do not resolve until all three
                    // actions have selected. If selection finds an empty draw
                    // pile, it shuffles the discard pile while the earlier
                    // selections remain held out. Session 35 reaches that
                    // branch on the third action of a second Distilled Chaos.
                    let mut queued_cards = Vec::with_capacity((3 * multiplier) as usize);
                    for _ in 0..3 * multiplier {
                        if combat.piles.draw_pile.is_empty()
                            && !combat.piles.discard_pile.is_empty()
                        {
                            player_shuffle_discard_into_draw(&mut combat)?;
                        }
                        let Some(card) = combat.piles.draw_pile.pop() else {
                            break;
                        };
                        combat.piles.limbo.push(card);
                        queued_cards.push(card);
                    }
                    let mut queued_plays = queued_cards
                        .into_iter()
                        .zip(queued_targets)
                        .collect::<std::collections::VecDeque<_>>();
                    while let Some((card, queued_target)) = queued_plays.pop_front() {
                        if combat.phase != CombatPhase::WaitingForPlayer {
                            for (held_card, _) in queued_plays.iter().rev() {
                                let index = combat
                                    .piles
                                    .limbo
                                    .iter()
                                    .position(|candidate| candidate.id == held_card.id)
                                    .ok_or(SimError::InvalidState(
                                        "Distilled Chaos held card is missing from limbo",
                                    ))?;
                                combat.piles.limbo.remove(index);
                                combat.piles.draw_pile.push(*held_card);
                            }
                            break;
                        }
                        let limbo_index = combat
                            .piles
                            .limbo
                            .iter()
                            .position(|candidate| candidate.id == card.id)
                            .ok_or(SimError::InvalidState(
                                "Distilled Chaos queued card is missing from limbo",
                            ))?;
                        combat.piles.limbo.remove(limbo_index);
                        combat.piles.draw_pile.push(card);
                        let top_definition = top_draw_card_definition(&combat)
                            .ok_or(SimError::IllegalAction("draw pile is empty"))?;
                        let target = if top_definition.target == TargetRequirement::Enemy {
                            queued_target
                        } else {
                            None
                        };
                        combat = apply_play_top_draw_card_action(&combat, target)?;
                        victory_healing_applied |= combat.phase == CombatPhase::Won;
                        if let Some(CombatDecisionState::HandSelect {
                            pending_actions, ..
                        }) = combat.decision.as_mut()
                        {
                            for (held_card, _) in queued_plays.iter().rev() {
                                let index = combat
                                    .piles
                                    .limbo
                                    .iter()
                                    .position(|candidate| candidate.id == held_card.id)
                                    .ok_or(SimError::InvalidState(
                                        "Distilled Chaos held card is missing from limbo",
                                    ))?;
                                combat.piles.limbo.remove(index);
                                combat.piles.draw_pile.push(*held_card);
                            }
                            pending_actions.extend(queued_plays.drain(..).map(|(_, target)| {
                                crate::InternalAction::PlayTopDrawCard {
                                    target,
                                    exhaust_played_card: false,
                                    random_living_target: false,
                                }
                            }));
                            break;
                        }
                    }
                    next.card_random_rng_counter = combat.rng.card_random_rng.counter();
                    next.combat = Some(combat);
                }
                Potion::LiquidMemories => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    open_discard_select_with_max_choices(combat, multiplier as usize)?;
                }
                Potion::Weak => {
                    let target = target.expect("validated weak potion target");
                    let combat = next.combat.as_mut().expect("validated combat state");
                    let monster = combat
                        .monsters
                        .iter_mut()
                        .find(|monster| monster.id == target)
                        .expect("validated potion target");
                    apply_monster_weak(&mut monster.powers, WEAK_POTION_WEAK * multiplier)?;
                }
                Potion::FruitJuice => {
                    let max_hp = FRUIT_JUICE_MAX_HP * multiplier;
                    next.gain_max_hp(max_hp)?;
                    if let Some(combat) = next.combat.as_mut() {
                        let combat_max_hp = combat.player.max_hp.checked_add(max_hp).ok_or(
                            SimError::InvalidState("Fruit Juice combat max HP gain overflows i32"),
                        )?;
                        let combat_hp =
                            combat
                                .player
                                .hp
                                .checked_add(max_hp)
                                .ok_or(SimError::InvalidState(
                                    "Fruit Juice combat HP gain overflows i32",
                                ))?;
                        combat.player.max_hp = combat_max_hp;
                        combat.player.hp = combat_hp;
                    }
                }
                Potion::GamblersBrew => {
                    let combat = next.combat.as_mut().expect("validated combat state");
                    open_gambling_chip_select(combat)?;
                }
                Potion::EntropicBrew => {
                    let mut rng = crate::rng::StsRng::with_counter(
                        next.potion_rng_seed as i64,
                        next.potion_rng_counter,
                    );
                    if next.can_gain_potions() && entropic_brew_had_open_slot {
                        let capacity = next.potion_capacity();
                        let combat_fill = next.phase == RunPhase::Combat;
                        for _ in 0..capacity {
                            let potion = if combat_fill {
                                target_random_combat_potion(&mut rng)
                            } else {
                                target_random_potion(&mut rng)
                            };
                            if next.open_potion_slots() > 0 {
                                next.gain_potion(potion)
                                    .expect("open potion slot validated");
                            }
                        }
                    }
                    next.potion_rng_counter = rng.counter();
                }
                Potion::Attack | Potion::Skill | Potion::Colorless | Potion::Power => {
                    defer_potion_use_relics = true;
                    let mut combat = next.combat.take().expect("validated combat state");
                    let next_card_id = combat.reserve_card_instance_ids(3)?;
                    let rng = &mut combat.rng.card_random_rng;
                    let (kind, content_ids) = match potion {
                        Potion::Attack => (
                            PotionCardRewardKind::Attack,
                            discovery_card_choices(rng, CardType::Attack, 3),
                        ),
                        Potion::Skill => (
                            PotionCardRewardKind::Skill,
                            discovery_card_choices(rng, CardType::Skill, 3),
                        ),
                        Potion::Colorless => (
                            PotionCardRewardKind::Colorless,
                            colorless_discovery_card_choices(rng, 3),
                        ),
                        Potion::Power => (
                            PotionCardRewardKind::Power,
                            discovery_card_choices(rng, CardType::Power, 3),
                        ),
                        _ => unreachable!("matched discovery potion"),
                    };
                    next.card_random_rng_counter = rng.counter();
                    let reward_cards = content_ids
                        .into_iter()
                        .enumerate()
                        .map(|(index, content_id)| {
                            CardInstance::new(CardId::new(next_card_id + index as u64), content_id)
                        })
                        .collect();
                    combat.queue_or_activate_decision(CombatDecisionState::PotionCardReward {
                        choices: reward_cards,
                        reward_kind: kind,
                    });
                    next.combat = Some(combat);
                }
                _ => {
                    return Err(SimError::IllegalAction(
                        "potion mechanics are not implemented",
                    ));
                }
            }
            if defer_potion_use_relics {
                if let Some(combat) = next.combat.as_ref() {
                    next.player_hp = combat.player.hp;
                }
            } else if let Some(combat) = next.combat.as_mut() {
                crate::relic::apply_potion_use_relics_to_combat(combat)?;
                next.player_hp = combat.player.hp;
            } else {
                crate::relic::apply_potion_use_relics_to_run_hp(
                    &mut next.player_hp,
                    next.player_max_hp,
                    &next.relics,
                );
            }
            let won = next
                .combat
                .as_ref()
                .map(|combat| combat.phase == CombatPhase::Won)
                .unwrap_or(false);
            if won {
                if !victory_healing_applied {
                    if let Some(combat) = next.combat.as_mut() {
                        apply_burning_blood(combat)?;
                        next.player_hp = combat.player.hp;
                        next.player_max_hp = combat.player.max_hp;
                    }
                }
                enter_combat_reward_for_current_room(&mut next)?;
            }
        }
        RunAction::DiscardPotion { slot } => {
            next.take_potion_slot(slot)?;
        }
        _ => unreachable!("validated potion action"),
    }

    Ok(next)
}

fn enter_combat_reward_for_current_room(run: &mut RunState) -> SimResult<()> {
    match current_room_kind(run) {
        Some(RoomKind::Boss) if run.current_act == 3 => enter_final_boss_victory(run),
        Some(RoomKind::Boss) => super::reward::enter_boss_combat_reward_screen(run),
        Some(RoomKind::Elite) => super::reward::enter_elite_combat_reward_screen(run),
        _ => super::reward::enter_reward_screen(run),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        apply_combat_action_on_run, apply_run_action,
        content::cards::{
            ANGER_ID, BASH_ID, BURNING_PACT_ID, BURNING_PACT_PLUS_ID, BURN_ID, CLASH_ID, CLEAVE_ID,
            DARK_EMBRACE_ID, DAZED_ID, DEFEND_R_ID, PURITY_ID, SECRET_TECHNIQUE_ID, STRIKE_R_ID,
            WARCRY_ID, WOUND_ID,
        },
        content::shop_pool::ironclad_combat_discovery_pool,
        CombatAction,
    };

    #[test]
    fn burning_pact_confirm_enters_reward_when_feel_no_pain_juggernaut_kills_last() {
        // 15ab4cc step 1102: exhaust Burn under FNP queues Juggernaut damage that
        // kills the last Darkling during CONFIRM; run must open combat reward.
        let mut run = RunState::combat_fixture();
        {
            let combat = run.combat.as_mut().expect("combat");
            combat.player.energy = 2;
            combat.player.powers.feel_no_pain = 3;
            combat.player.powers.juggernaut = 5;
            combat.piles.hand = vec![
                CardInstance::new(CardId::new(1), BURNING_PACT_ID),
                CardInstance::new(CardId::new(2), BURN_ID),
                CardInstance::new(CardId::new(3), STRIKE_R_ID),
            ];
            combat.piles.draw_pile = vec![
                CardInstance::new(CardId::new(4), DEFEND_R_ID),
                CardInstance::new(CardId::new(5), STRIKE_R_ID),
            ];
            combat.piles.discard_pile.clear();
            combat.monsters.truncate(1);
            combat.monsters[0].hp = 3;
            combat.monsters[0].alive = true;
        }

        let after_play = apply_combat_action_on_run(
            &run,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Burning Pact opens exhaust select");
        assert!(after_play
            .combat
            .as_ref()
            .and_then(|c| c.exhaust_select())
            .is_some());

        let after_choose =
            apply_run_action(&after_play, RunAction::ChooseExhaustSelect { index: 0 })
                .expect("select Burn");
        let after_confirm = apply_run_action(&after_choose, RunAction::ConfirmExhaustSelect)
            .expect("confirm Burning Pact");

        assert_eq!(
            after_confirm.phase,
            RunPhase::Reward,
            "lethal Juggernaut from FNP block on exhaust select CONFIRM must open rewards"
        );
        assert!(after_confirm.reward.is_some());
    }

    #[test]
    fn exhaust_confirm_act_three_boss_enters_victory_not_reward() {
        let mut run = RunState::combat_fixture();
        run.current_act = 3;
        run.current_floor = 50;
        run.current_room_override = Some(RoomKind::Boss);
        {
            let combat = run.combat.as_mut().expect("combat");
            combat.phase = CombatPhase::Won;
            for monster in &mut combat.monsters {
                monster.hp = 0;
                monster.alive = false;
            }
        }

        enter_combat_reward_for_current_room(&mut run).expect("Act 3 boss win from select CONFIRM");

        assert_eq!(run.phase, RunPhase::Victory);
        assert!(run.reward.is_none());
        assert!(run.combat.is_none());
    }

    #[test]
    fn burning_pact_dead_branch_precedes_dark_embrace_draw() {
        let mut run = RunState::combat_fixture_with_relics(vec![Relic::DeadBranch]);
        let initial_card_random_counter = run.card_random_rng_counter;
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.player.powers.dark_embrace = 1;
        combat.piles.hand = vec![
            CardInstance::new(CardId::new(1), BURNING_PACT_ID),
            CardInstance::new(CardId::new(2), BASH_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        // CommunicationMod exposes the bottom-first draw-pile order. Burning
        // Pact draws ids 6 and 5; Dark Embrace then draws id 4. Dead Branch's
        // MakeTempCardInHand action must settle between those draw actions.
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
            CardInstance::new(CardId::new(5), STRIKE_R_ID),
            CardInstance::new(CardId::new(6), CLASH_ID),
        ];
        combat.piles.discard_pile.clear();
        combat.piles.exhaust_pile.clear();

        let after_play = apply_combat_action_on_run(
            &run,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Burning Pact opens exhaust select");
        let after_choose =
            apply_run_action(&after_play, RunAction::ChooseExhaustSelect { index: 0 })
                .expect("select Bash");
        let after_confirm = apply_run_action(&after_choose, RunAction::ConfirmExhaustSelect)
            .expect("confirm Burning Pact");
        let combat = after_confirm.combat.expect("combat remains open");
        let hand_ids: Vec<_> = combat.piles.hand.iter().map(|card| card.id).collect();
        assert_eq!(
            hand_ids[0..3],
            [CardId::new(3), CardId::new(6), CardId::new(5)]
        );
        assert_eq!(hand_ids[4], CardId::new(4));
        assert_eq!(hand_ids[3], CardId::new(7));
        assert!(combat.piles.hand[3].combat_only);
        assert_eq!(
            after_confirm.card_random_rng_counter,
            initial_card_random_counter + 1
        );
    }

    #[test]
    fn skipped_burning_pact_retrieval_does_not_double_dead_branch() {
        // FIDL01534: skipped selected-card retrieval still exhausts Burning Pact
        // under Corruption. Dead Branch must fire once for that source, not
        // again from the exhaust-pile delta.
        let mut run = RunState::combat_fixture_with_relics(vec![Relic::DeadBranch]);
        let initial_card_random_counter = run.card_random_rng_counter;
        {
            let combat = run.combat.as_mut().expect("combat fixture");
            combat.player.powers.corruption = 1;
            combat.piles.hand = vec![
                CardInstance::new(CardId::new(1), BURNING_PACT_PLUS_ID),
                CardInstance::new(CardId::new(2), STRIKE_R_ID),
                CardInstance::new(CardId::new(3), WOUND_ID),
                CardInstance::new(CardId::new(4), WOUND_ID),
            ];
            combat.piles.draw_pile = vec![
                CardInstance::new(CardId::new(5), DEFEND_R_ID),
                CardInstance::new(CardId::new(6), CLASH_ID),
                CardInstance::new(CardId::new(7), CLEAVE_ID),
            ];
            combat.piles.discard_pile.clear();
            combat.piles.exhaust_pile.clear();
        }

        let after_play = apply_combat_action_on_run(
            &run,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Burning Pact+ opens exhaust select");
        let after_choose =
            apply_run_action(&after_play, RunAction::ChooseExhaustSelect { index: 1 })
                .expect("select a Wound");
        let (after_confirm, hidden) =
            apply_exhaust_select_confirm_skipped_burning_pact_retrieval_without_time_warp_end(
                &after_choose,
            )
            .expect("skipped Burning Pact retrieval");
        let combat = after_confirm.combat.expect("combat remains open");
        let generated = combat
            .piles
            .hand
            .iter()
            .filter(|card| card.combat_only)
            .count();
        assert_eq!(generated, 1);
        assert_eq!(hidden.content_id, WOUND_ID);
        assert!(combat
            .piles
            .exhaust_pile
            .iter()
            .all(|card| card.id != hidden.id));
        assert_eq!(
            after_confirm.card_random_rng_counter,
            initial_card_random_counter + 1
        );
    }

    #[test]
    fn warcry_hex_dead_branch_uses_combat_rng_after_dazed_insert() {
        // FIDL01442: Warcry CONFIRM puts the selected card on draw, Hex inserts
        // Dazed, then Dead Branch rolls into hand on the same cardRandomRng.
        let mut run = RunState::combat_fixture_with_relics(vec![Relic::DeadBranch]);
        {
            let combat = run.combat.as_mut().expect("combat fixture");
            combat.player.powers.hex = 1;
            combat.piles.hand = vec![
                CardInstance::new(CardId::new(1), WARCRY_ID),
                CardInstance::new(CardId::new(2), STRIKE_R_ID),
                CardInstance::new(CardId::new(3), DEFEND_R_ID),
            ];
            combat.piles.draw_pile = vec![
                CardInstance::new(CardId::new(4), BASH_ID),
                CardInstance::new(CardId::new(5), CLEAVE_ID),
                CardInstance::new(CardId::new(6), DEFEND_R_ID),
            ];
            combat.piles.discard_pile.clear();
            combat.piles.exhaust_pile.clear();
        }

        let after_play = apply_combat_action_on_run(
            &run,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Warcry should open its hand-select screen");
        let after_choose = apply_run_action(&after_play, RunAction::ChooseHandSelect { index: 0 })
            .expect("select Strike");

        let combat = after_choose.combat.as_ref().expect("combat remains open");
        let mut expected_rng = combat.rng.card_random_rng.clone();
        let mut expected_draw = combat
            .piles
            .draw_pile
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>();
        expected_draw.push(STRIKE_R_ID);
        let generated_index = expected_rng.random_int((expected_draw.len() - 1) as i32) as usize;
        expected_draw.insert(generated_index, DAZED_ID);
        let pool = ironclad_combat_discovery_pool();
        let dead_branch_id = pool[expected_rng.random_int((pool.len() - 1) as i32) as usize];

        let after_confirm = apply_run_action(&after_choose, RunAction::ConfirmHandSelect)
            .expect("confirm Warcry selection");
        let combat = after_confirm.combat.expect("combat remains open");

        assert_eq!(
            combat
                .piles
                .draw_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            expected_draw
        );
        assert_eq!(
            combat.piles.hand.last().map(|card| card.content_id),
            Some(dead_branch_id)
        );
        assert_ne!(
            dead_branch_id,
            pool[after_choose
                .combat
                .as_ref()
                .expect("pre-confirm combat")
                .rng
                .card_random_rng
                .clone()
                .random_int((pool.len() - 1) as i32) as usize],
            "Dead Branch must not reuse the pre-Hex cardRandomRng counter"
        );
    }

    #[derive(Debug, Clone, Copy)]
    enum PotionStatDestination {
        Block,
        Artifact,
        Metallicize,
        Ritual,
        Dexterity,
        PlatedArmor,
        Thorns,
        Regen,
        Strength,
        TempStrength,
        SpeedDexterity,
        SpeedTempDexterity,
        DuplicationStacks,
    }

    #[derive(Debug, Clone, Copy)]
    enum MonsterDebuffDestination {
        Weak,
        Vulnerable,
    }

    #[test]
    fn monster_debuff_potion_overflow_preserves_run_and_potion_slot() {
        let cases = [
            (
                Potion::Weak,
                MonsterDebuffDestination::Weak,
                SimError::InvalidState("monster Weak application overflows i32"),
            ),
            (
                Potion::Fear,
                MonsterDebuffDestination::Vulnerable,
                SimError::InvalidState("monster Vulnerable application overflows i32"),
            ),
        ];

        for (potion, destination, expected_error) in cases {
            let mut run = RunState::combat_fixture();
            run.potions = vec![potion];
            run.empty_potion_slots = vec![1, 2];
            let monster = run
                .combat
                .as_mut()
                .expect("combat fixture")
                .monsters
                .first_mut()
                .expect("combat fixture monster");
            let target = monster.id;
            match destination {
                MonsterDebuffDestination::Weak => monster.powers.weak = i32::MAX,
                MonsterDebuffDestination::Vulnerable => {
                    monster.powers.vulnerable = i32::MAX;
                }
            }
            let before = run.clone();

            assert_eq!(
                apply_potion_action(
                    &run,
                    RunAction::UsePotion {
                        slot: 0,
                        target: Some(target),
                    },
                ),
                Err(expected_error),
                "{potion:?} {destination:?}"
            );
            assert_eq!(run, before, "{potion:?} {destination:?}");
            assert_eq!(run.potion_at_slot(0), Some(potion));
        }
    }

    #[test]
    fn combat_potion_stat_overflow_leaves_exact_input_state() {
        let cases = [
            (Potion::Block, PotionStatDestination::Block),
            (Potion::Ancient, PotionStatDestination::Artifact),
            (Potion::HeartOfIron, PotionStatDestination::Metallicize),
            (Potion::Cultist, PotionStatDestination::Ritual),
            (Potion::Dexterity, PotionStatDestination::Dexterity),
            (Potion::EssenceOfSteel, PotionStatDestination::PlatedArmor),
            (Potion::LiquidBronze, PotionStatDestination::Thorns),
            (Potion::Regen, PotionStatDestination::Regen),
            (Potion::Strength, PotionStatDestination::Strength),
            (Potion::Flex, PotionStatDestination::TempStrength),
            (Potion::Speed, PotionStatDestination::SpeedDexterity),
            (Potion::Speed, PotionStatDestination::SpeedTempDexterity),
            (
                Potion::Duplication,
                PotionStatDestination::DuplicationStacks,
            ),
        ];

        for (potion, destination) in cases {
            let mut run = RunState::combat_fixture();
            run.potions = vec![potion];
            run.empty_potion_slots = vec![1, 2];
            let combat = run.combat.as_mut().expect("combat fixture");
            match destination {
                PotionStatDestination::Block => combat.player.block = i32::MAX,
                PotionStatDestination::Artifact => combat.player.powers.artifact = i32::MAX,
                PotionStatDestination::Metallicize => {
                    combat.player.powers.metallicize = i32::MAX;
                }
                PotionStatDestination::Ritual => combat.player.powers.ritual = i32::MAX,
                PotionStatDestination::Dexterity | PotionStatDestination::SpeedDexterity => {
                    combat.player.powers.dexterity = i32::MAX;
                }
                PotionStatDestination::PlatedArmor => {
                    combat.player.powers.plated_armor = i32::MAX;
                }
                PotionStatDestination::Thorns => combat.player.powers.thorns = i32::MAX,
                PotionStatDestination::Regen => combat.player.powers.regen = i32::MAX,
                PotionStatDestination::Strength => combat.player.powers.strength = i32::MAX,
                PotionStatDestination::TempStrength => combat.player.temp_strength = i32::MAX,
                PotionStatDestination::SpeedTempDexterity => {
                    combat.player.temp_dexterity = i32::MAX;
                }
                PotionStatDestination::DuplicationStacks => {
                    combat.duplication_potion_stacks = i32::MAX;
                }
            }
            let before = run.clone();

            assert_eq!(
                apply_potion_action(
                    &run,
                    RunAction::UsePotion {
                        slot: 0,
                        target: None,
                    },
                ),
                Err(SimError::InvalidState(
                    "combat potion stat gain overflows i32"
                )),
                "{potion:?} {destination:?}"
            );
            assert_eq!(run, before, "{potion:?} {destination:?}");
        }
    }

    #[test]
    fn sacred_bark_doubles_checked_combat_potion_stats() {
        for potion in [
            Potion::Block,
            Potion::HeartOfIron,
            Potion::Speed,
            Potion::Duplication,
        ] {
            let mut run = RunState::combat_fixture();
            run.relics.push(crate::Relic::SacredBark);
            run.potions = vec![potion];
            run.empty_potion_slots = vec![1, 2];

            let next = apply_potion_action(
                &run,
                RunAction::UsePotion {
                    slot: 0,
                    target: None,
                },
            )
            .expect("checked Sacred Bark potion effect succeeds");
            let combat = next.combat.as_ref().expect("combat remains active");
            match potion {
                Potion::Block => assert_eq!(combat.player.block, BLOCK_POTION_BLOCK * 2),
                Potion::HeartOfIron => {
                    assert_eq!(
                        combat.player.powers.metallicize,
                        HEART_OF_IRON_METALLICIZE * 2
                    );
                }
                Potion::Speed => {
                    assert_eq!(
                        combat.player.powers.dexterity,
                        SPEED_POTION_TEMP_DEXTERITY * 2
                    );
                    assert_eq!(
                        combat.player.temp_dexterity,
                        SPEED_POTION_TEMP_DEXTERITY * 2
                    );
                }
                Potion::Duplication => assert_eq!(combat.duplication_potion_stacks, 2),
                _ => unreachable!("representative checked potion list"),
            }
        }
    }

    #[test]
    fn blood_potion_uses_wide_percentage_intermediate() {
        let mut run = RunState::combat_fixture();
        run.relics.push(crate::Relic::SacredBark);
        run.potions = vec![Potion::Blood];
        run.empty_potion_slots = vec![1, 2];
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.player.hp = 1;
        combat.player.max_hp = i32::MAX;

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("wide Blood Potion heal succeeds");

        assert_eq!(
            next.combat
                .as_ref()
                .expect("combat remains active")
                .player
                .hp,
            858_993_459
        );
    }

    #[test]
    fn full_belt_entropic_brew_checks_capacity_before_consuming_itself() {
        let mut run = RunState::map_fixture();
        run.potions = vec![Potion::Swift, Potion::Elixir, Potion::EntropicBrew];
        run.empty_potion_slots.clear();
        let starting_rng_counter = run.potion_rng_counter;

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 2,
                target: None,
            },
        )
        .expect("full-belt Entropic Brew is consumed without generating a potion");

        assert_eq!(next.potions, vec![Potion::Swift, Potion::Elixir]);
        assert_eq!(next.empty_potion_slots, vec![2]);
        assert_eq!(next.potion_rng_counter, starting_rng_counter);
    }

    #[test]
    fn resource_potion_overflow_fails_before_returning_partial_state() {
        let mut energy = RunState::combat_fixture();
        energy.potions = vec![Potion::Energy];
        energy.empty_potion_slots = vec![1, 2];
        energy
            .combat
            .as_mut()
            .expect("combat fixture")
            .player
            .energy = i32::MAX;
        assert_eq!(
            apply_potion_action(
                &energy,
                RunAction::UsePotion {
                    slot: 0,
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "Energy Potion energy gain overflows i32"
            ))
        );
        assert_eq!(energy.potion_at_slot(0), Some(Potion::Energy));

        let mut fruit_juice = RunState::combat_fixture();
        fruit_juice.potions = vec![Potion::FruitJuice];
        fruit_juice.empty_potion_slots = vec![1, 2];
        fruit_juice.player_max_hp = i32::MAX;
        assert_eq!(
            apply_potion_action(
                &fruit_juice,
                RunAction::UsePotion {
                    slot: 0,
                    target: None,
                },
            ),
            Err(SimError::InvalidState("run integer addition overflows i32"))
        );
        assert_eq!(fruit_juice.potion_at_slot(0), Some(Potion::FruitJuice));

        let mut mirrored = RunState::combat_fixture();
        mirrored.potions = vec![Potion::FruitJuice];
        mirrored.empty_potion_slots = vec![1, 2];
        mirrored
            .combat
            .as_mut()
            .expect("combat fixture")
            .player
            .max_hp = i32::MAX;
        assert_eq!(
            apply_potion_action(
                &mirrored,
                RunAction::UsePotion {
                    slot: 0,
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "Fruit Juice combat max HP gain overflows i32"
            ))
        );
        assert_eq!(mirrored.potion_at_slot(0), Some(Potion::FruitJuice));
    }

    #[test]
    fn distilled_chaos_top_deck_combust_applies_its_end_turn_power() {
        use crate::content::cards::{COMBUST_ID, DEFEND_R_ID, STRIKE_R_ID};

        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::DistilledChaos];
        run.empty_potion_slots = vec![1, 2];
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(101), DEFEND_R_ID),
            CardInstance::new(CardId::new(102), STRIKE_R_ID),
            CardInstance::new(CardId::new(103), COMBUST_ID),
        ];

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("Distilled Chaos resolves top three cards");
        let mut combat = next.combat.expect("combat remains open");

        assert_eq!(combat.player.powers.combust, 1);
        assert_eq!(combat.player.powers.combust_damage, 5);
        let player_hp = combat.player.hp;
        let monster_hp = combat.monsters[0].hp;
        crate::combat::turn_powers::apply_end_of_player_turn_powers(&mut combat)
            .expect("end-turn powers resolve");
        assert_eq!(combat.player.hp, player_hp - 1);
        assert_eq!(combat.monsters[0].hp, monster_hp - 5);
    }

    #[test]
    fn distilled_chaos_killing_last_monster_applies_victory_heal_once() {
        use crate::content::cards::STRIKE_R_ID;
        use crate::content::character::BURNING_BLOOD_HEAL_AMOUNT;

        let mut run = RunState::combat_fixture_with_relics(vec![crate::relic::Relic::BurningBlood]);
        run.potions = vec![Potion::DistilledChaos];
        run.empty_potion_slots = vec![1, 2];
        let before_hp;
        {
            let combat = run.combat.as_mut().expect("combat fixture");
            combat.player.hp -= 10;
            before_hp = combat.player.hp;
            for monster in &mut combat.monsters {
                monster.alive = false;
                monster.hp = 0;
            }
            combat.monsters[0].alive = true;
            combat.monsters[0].hp = 1;
            combat.piles.draw_pile = vec![CardInstance::new(CardId::new(101), STRIKE_R_ID)];
        }
        run.player_hp = before_hp;
        let expected_hp = before_hp + BURNING_BLOOD_HEAL_AMOUNT;

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("Distilled Chaos kills the last monster");

        assert_eq!(next.phase, RunPhase::Reward);
        assert_eq!(next.player_hp, expected_hp);
        assert!(next.combat.is_none());
    }

    #[test]
    fn distilled_chaos_holds_queued_cards_authoritatively_until_hand_select() {
        use crate::content::cards::{ANGER_ID, DUAL_WIELD_ID, STRIKE_R_ID, WILD_STRIKE_ID};

        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::DistilledChaos];
        run.empty_potion_slots = vec![1, 2];
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(101), WILD_STRIKE_ID),
            CardInstance::new(CardId::new(102), DUAL_WIELD_ID),
            CardInstance::new(CardId::new(103), ANGER_ID),
            CardInstance::new(CardId::new(104), STRIKE_R_ID),
        ];

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("Distilled Chaos pauses at Dual Wield selection");
        let combat = next.combat.as_ref().expect("combat remains open");

        combat
            .validate()
            .expect("queued and generated card IDs remain unique");
        assert_eq!(
            combat
                .piles
                .limbo
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![DUAL_WIELD_ID],
            "the force-played Dual Wield remains cardInUse while its select is open"
        );
        assert!(matches!(
            &combat.decision,
            Some(CombatDecisionState::HandSelect {
                state: crate::combat::HandSelectState {
                    purpose: HandSelectPurpose::DualWieldCopy,
                    ..
                },
                ..
            })
        ));
        assert!(!combat
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == DUAL_WIELD_ID));
        assert_eq!(combat.piles.draw_pile.len(), 1);
        assert_eq!(combat.piles.draw_pile[0].content_id, WILD_STRIKE_ID);
    }

    #[test]
    fn distilled_chaos_reshuffles_to_resolve_its_third_action() {
        use crate::content::cards::DEFEND_R_ID;

        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::DistilledChaos];
        run.empty_potion_slots = vec![1, 2];
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(101), DEFEND_R_ID),
            CardInstance::new(CardId::new(102), DEFEND_R_ID),
        ];
        combat.piles.discard_pile = vec![CardInstance::new(CardId::new(103), DEFEND_R_ID)];

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("Distilled Chaos reshuffles before its third action");
        let combat = next.combat.expect("combat remains open");

        assert_eq!(combat.player.block, 15);
        assert!(combat.piles.draw_pile.is_empty());
        assert_eq!(combat.piles.discard_pile.len(), 3);
    }

    #[test]
    fn smoke_bomb_runs_end_of_combat_healing() {
        let mut run = RunState::combat_fixture();
        run.current_room_override = Some(RoomKind::Elite);
        run.relics = vec![crate::relic::Relic::BurningBlood];
        run.potions = vec![Potion::SmokeBomb];
        run.empty_potion_slots = vec![1, 2];
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.relics = run.relics.clone();
        combat.player.hp = 10;
        combat.player.max_hp = 80;

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("Smoke Bomb escapes elite combat");

        assert_eq!(next.player_hp, 16);
        assert!(next.combat.is_none());
        assert_eq!(next.phase, RunPhase::Idle);
        assert_eq!(next.potion_at_slot(0), None);
    }

    #[test]
    fn smoke_bomb_normal_combat_consumes_hidden_gold_rng_draw() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::SmokeBomb];
        run.empty_potion_slots = vec![1, 2];
        run.treasure_rng_seed = 34_961_238_615_626;
        run.treasure_rng_counter = 4;
        // The target's Java RNG reference sequence returns 25 at counter 1,
        // forcing the ordinary 40% hidden drop roll to hit. This makes the
        // regression cover the subsequent hidden potion-selection draw too.
        run.potion_rng_seed = 22_079_335_079;
        run.potion_rng_counter = 1;
        run.current_room_override = Some(RoomKind::Combat);

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("Smoke Bomb escapes normal combat");

        assert_eq!(next.treasure_rng_counter, 5);
        let mut expected_potion_rng = run.rng_for_stream(RunRngStream::Potion);
        let mut expected_potion_chance = run.potion_chance;
        let hidden_potion_offer = target_potion_reward_offer(
            &mut expected_potion_rng,
            &mut expected_potion_chance,
            1,
            run.potions.len(),
            run.potion_capacity(),
            run.relics.contains(&crate::relic::Relic::WhiteBeastStatue),
        )
        .expect("Smoke Bomb hidden potion roll");
        assert!(
            hidden_potion_offer.is_some(),
            "counter-1 reference roll must exercise hidden potion selection"
        );
        assert_eq!(next.potion_rng_counter, expected_potion_rng.counter());
        assert_eq!(next.potion_chance, expected_potion_chance);
        assert_eq!(next.phase, RunPhase::Idle);
        assert!(next.reward.is_none());
    }

    #[test]
    fn smoke_bomb_event_combat_discards_pending_event_reward() {
        let mut run = RunState::combat_fixture();
        run.current_room_override = Some(RoomKind::Event);
        run.potions = vec![Potion::SmokeBomb];
        run.empty_potion_slots = vec![1, 2];
        run.pending_event_combat_gold_offer = 27;
        run.pending_event_combat_relic_offer = Some(Relic::RedMask);

        let mut expected_potion_rng = run.rng_for_stream(RunRngStream::Potion);
        let mut expected_potion_chance = run.potion_chance;
        let _ = target_potion_reward_offer(
            &mut expected_potion_rng,
            &mut expected_potion_chance,
            1,
            run.potions.len(),
            run.potion_capacity(),
            run.relics.contains(&Relic::WhiteBeastStatue),
        )
        .expect("event Smoke Bomb hidden potion roll");

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("Smoke Bomb escapes event combat");

        assert_eq!(next.pending_event_combat_gold_offer, 0);
        assert_eq!(next.pending_event_combat_relic_offer, None);
        assert_eq!(next.potion_rng_counter, expected_potion_rng.counter());
        assert_eq!(next.potion_chance, expected_potion_chance);
        next.validate().expect("escaped event combat is valid");
    }

    #[test]
    fn explosive_potion_kill_releases_spore_cloud() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::Explosive];
        run.empty_potion_slots = vec![1, 2];
        let combat = run.combat.as_mut().unwrap();
        let mut surviving = combat.monsters[0].clone();
        surviving.id = MonsterId::new(1);
        surviving.hp = 20;
        surviving.max_hp = 20;
        surviving.alive = true;
        surviving.powers.spore_cloud = 0;
        let mut dying = surviving.clone();
        dying.id = MonsterId::new(2);
        dying.hp = EXPLOSIVE_POTION_DAMAGE;
        dying.powers.spore_cloud = 2;
        combat.monsters = vec![surviving, dying];

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(MonsterId::new(1)),
            },
        )
        .unwrap();
        let combat = next.combat.unwrap();

        assert_eq!(combat.player.powers.vulnerable, 2);
        assert_eq!(combat.monsters[0].hp, 10);
        assert!(!combat.monsters[1].alive);
    }

    #[test]
    fn explosive_potion_wakes_sleeping_lagavulin() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::Explosive];
        run.empty_potion_slots = vec![1, 2];
        run.combat = Some(CombatState::lagavulin_fixture());

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Explosive Potion damages Lagavulin");
        let monster = &next.combat.expect("combat remains open").monsters[0];

        assert_eq!(monster.sleep_turns_remaining, 0);
        assert_eq!(monster.intent, crate::MonsterIntent::Stun);
        assert_eq!(
            crate::content::monsters::target_move_byte_for_monster(monster),
            Some(4)
        );
    }

    #[test]
    fn discovery_potion_choice_is_appended_to_hand() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let original_ids = combat
            .piles
            .hand
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        let choice_content = combat.piles.hand[0].content_id;
        combat.decision = Some(CombatDecisionState::PotionCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                choice_content,
            )],
            reward_kind: PotionCardRewardKind::Colorless,
        });
        combat.rng.card_random_rng = StsRng::new(123);
        let rng_counter_before = combat.rng.card_random_rng.counter();

        let next = apply_combat_card_reward_choice(&run, 0).expect("potion card choice");
        let combat = next.combat.expect("combat remains open");
        let hand = &combat.piles.hand;

        assert_eq!(
            hand[..original_ids.len()]
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            original_ids
        );
        assert_eq!(hand.last().map(|card| card.id), Some(chosen_id));
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            rng_counter_before + 35,
            "captured colorless Discovery settlement must advance cardRandomRng"
        );
    }

    #[test]
    fn discovery_potion_queues_behind_an_active_combat_decision() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::Skill];
        run.empty_potion_slots = vec![1, 2];
        open_exhaust_select(run.combat.as_mut().expect("combat fixture"))
            .expect("open the active exhaust decision");

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("Skill Potion can be used during an active combat decision");
        let combat = next.combat.expect("combat remains open");

        assert!(matches!(
            combat.decision,
            Some(CombatDecisionState::ExhaustSelect { .. })
        ));
        assert_eq!(combat.queued_decisions.len(), 1);
        assert!(combat.potion_card_reward_choices().is_none());
    }

    #[test]
    fn snecko_oil_synchronizes_run_and_combat_card_random_rng() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::SneckoOil];
        run.empty_potion_slots = vec![1, 2];

        let next = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("Snecko Oil applies");
        let expected_rng = next.card_random_rng();
        let combat = next.combat.expect("combat remains open");

        assert_eq!(
            combat.rng.card_random_rng, expected_rng,
            "Snecko Oil's card-random draws must remain authoritative inside combat"
        );
    }

    #[test]
    fn snecko_oil_preserves_x_cost_cards_without_consuming_card_rng() {
        use crate::content::cards::{STRIKE_R_ID, WHIRLWIND_ID};

        let mut combat = CombatState::initial_fixture();
        combat.piles.hand = vec![
            CardInstance::new(CardId::new(1), WHIRLWIND_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
        ];
        let mut rng = StsRng::new(3);

        randomize_playable_hand_costs_for_snecko_oil(&mut combat, &mut rng);

        assert_eq!(combat.piles.hand[0].temp_cost, None);
        assert_eq!(combat.piles.hand[1].temp_cost, Some(0));
        assert_eq!(rng.counter(), 1);
    }

    #[test]
    fn colorless_potion_pick_matches_session_1203_card_random_rng() {
        use crate::content::cards::{FORETHOUGHT_ID, PANIC_BUTTON_ID, SADISTIC_NATURE_ID};

        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::Colorless];
        run.empty_potion_slots = vec![1, 2];
        run.combat
            .as_mut()
            .expect("combat fixture")
            .rng
            .card_random_rng = StsRng::with_counter(-571_295_464_674_976_220, 4);

        let reward = apply_potion_action(
            &run,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("Colorless Potion opens its reward");
        assert_eq!(
            reward
                .combat
                .as_ref()
                .and_then(CombatState::potion_card_reward_choices)
                .expect("potion reward")
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![FORETHOUGHT_ID, SADISTIC_NATURE_ID, PANIC_BUTTON_ID]
        );

        let next = apply_combat_card_reward_choice(&reward, 1).expect("pick Sadistic Nature");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.piles.hand.last().map(|card| card.content_id),
            Some(SADISTIC_NATURE_ID)
        );
        let mut card_random_rng = combat.rng.card_random_rng;
        assert_eq!(card_random_rng.counter(), 41);
        assert_eq!(
            card_random_rng.random_int(1),
            1,
            "session-1203 Havoc must target the second living slime"
        );
    }

    #[test]
    fn discovery_card_choice_is_appended_to_hand() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        let original_ids = combat
            .piles
            .hand
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        let choice_content = combat.piles.hand[0].content_id;
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                choice_content,
            )],
            source_card: None,
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0).expect("Discovery card choice");
        let combat = next.combat.expect("combat remains open");
        let hand = &combat.piles.hand;

        assert_eq!(
            hand[..original_ids.len()]
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            original_ids
        );
        assert_eq!(hand.last().map(|card| card.id), Some(chosen_id));
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            19,
            "Discovery retrieve burns one discarded generateCardChoices generation"
        );
    }

    #[test]
    fn havoc_play_top_discovery_retrieve_burns_six_generations() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        let choice_content = combat.piles.hand[0].content_id;
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                choice_content,
            )],
            source_card: None,
            source_card_force_exhaust: true,
            source_card_play_top: true,
            pending_actions: Default::default(),
        });

        let next =
            apply_combat_card_reward_choice(&run, 0).expect("Havoc PlayTop Discovery card choice");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            34,
            "Havoc PlayTop Discovery retrieve burns six discarded generateCardChoices generations"
        );
    }

    #[test]
    fn discovery_retrieve_while_hexed_with_two_living_enemies_burns_two_generations() {
        use crate::content::cards::STRIKE_R_ID;
        use crate::content::monsters::{monster_state, CHOSEN_A0, CULTIST_A0};
        use crate::ids::MonsterId;

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.player.powers.hex = 1;
        combat.monsters = vec![
            monster_state(&CULTIST_A0, MonsterId::new(1)),
            monster_state(&CHOSEN_A0, MonsterId::new(2)),
        ];
        combat.piles.hand = (1..=4)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: None,
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next =
            apply_combat_card_reward_choice(&run, 0).expect("Hexed two-enemy Discovery retrieve");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            22,
            "Hex with two living enemies burns two discarded generateCardChoices generations"
        );
    }

    #[test]
    fn discovery_retrieve_while_hexed_vs_solo_chosen_burns_one_generation() {
        use crate::content::cards::STRIKE_R_ID;
        use crate::content::monsters::{monster_state, CHOSEN_A0};
        use crate::ids::MonsterId;

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.player.powers.hex = 1;
        combat.monsters = vec![monster_state(&CHOSEN_A0, MonsterId::new(1))];
        combat.piles.hand = (1..=4)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: None,
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next =
            apply_combat_card_reward_choice(&run, 0).expect("solo Chosen Hexed Discovery retrieve");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            19,
            "Hex against a single living enemy burns one discarded generateCardChoices generation"
        );
    }

    #[test]
    fn discovery_retrieve_with_six_remaining_cards_vs_awakened_one_burns_two_generations() {
        use crate::content::cards::STRIKE_R_ID;
        use crate::content::monsters::{monster_state, AWAKENED_ONE_A0};
        use crate::ids::MonsterId;

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.monsters = vec![monster_state(&AWAKENED_ONE_A0, MonsterId::new(1))];
        combat.piles.hand = (1..=6)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: None,
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0).expect("full-hand Discovery retrieve");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            22,
            "Awakened One + 6 remaining cards burn two discarded generations"
        );
    }

    #[test]
    fn discovery_retrieve_with_five_remaining_cards_vs_solo_awakened_one_burns_two_generations() {
        use crate::content::cards::STRIKE_R_ID;
        use crate::content::monsters::{monster_state, AWAKENED_ONE_A0};
        use crate::ids::MonsterId;

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.monsters = vec![monster_state(&AWAKENED_ONE_A0, MonsterId::new(1))];
        combat.piles.hand = (1..=5)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: None,
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0)
            .expect("five-card remaining Discovery vs solo Awakened One");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            22,
            "solo living Awakened One + 5 remaining cards burn two discarded generations"
        );
    }

    #[test]
    fn discovery_retrieve_with_five_remaining_cards_vs_awakened_one_and_cultist_burns_one_generation(
    ) {
        use crate::content::cards::STRIKE_R_ID;
        use crate::content::monsters::{monster_state, AWAKENED_ONE_A0, CULTIST_A0};
        use crate::ids::MonsterId;

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.monsters = vec![
            monster_state(&CULTIST_A0, MonsterId::new(1)),
            monster_state(&AWAKENED_ONE_A0, MonsterId::new(2)),
        ];
        combat.piles.hand = (1..=5)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: None,
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0)
            .expect("five-card remaining Discovery vs Awakened One and Cultist");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            19,
            "Awakened One with another living enemy + 5 remaining cards stay one discarded generation"
        );
    }

    #[test]
    fn discovery_retrieve_with_magnetism_generated_source_burns_two_generations() {
        use crate::content::cards::{DISCOVERY_ID, STRIKE_R_ID};

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.player.powers.magnetism = 1;
        combat.relic_counters.cards_played_this_turn = 2;
        combat.piles.hand = (1..=4)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: Some(CardInstance {
                combat_only: true,
                magnetism_generated: true,
                ..CardInstance::new(CardId::new(99), DISCOVERY_ID)
            }),
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0)
            .expect("Magnetism-generated Discovery retrieve");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            22,
            "early-turn Magnetism-generated Discovery burns two discarded generateCardChoices generations"
        );
    }

    #[test]
    fn discovery_retrieve_lone_early_magnetism_source_from_six_card_hand_burns_one_generation() {
        use crate::content::cards::{DISCOVERY_ID, STRIKE_R_ID};

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.player.powers.magnetism = 1;
        combat.relic_counters.cards_played_this_turn = 1;
        combat.piles.hand = (1..=6)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: Some(CardInstance {
                combat_only: true,
                magnetism_generated: true,
                ..CardInstance::new(CardId::new(99), DISCOVERY_ID)
            }),
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0)
            .expect("lone early-turn Magnetism Discovery retrieve");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            19,
            "lone early-turn Magnetism-generated Discovery from a 6-card remaining hand burns one discarded generation"
        );
    }

    #[test]
    fn discovery_retrieve_early_magnetism_source_with_another_generated_card_burns_two_generations()
    {
        use crate::content::cards::{DISCOVERY_ID, FLASH_OF_STEEL_ID, STRIKE_R_ID};

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.player.powers.magnetism = 1;
        combat.relic_counters.cards_played_this_turn = 1;
        combat.piles.hand = (1..=5)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .chain(std::iter::once(CardInstance {
                combat_only: true,
                magnetism_generated: true,
                ..CardInstance::new(CardId::new(50), FLASH_OF_STEEL_ID)
            }))
            .collect();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: Some(CardInstance {
                combat_only: true,
                magnetism_generated: true,
                ..CardInstance::new(CardId::new(99), DISCOVERY_ID)
            }),
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0)
            .expect("early-turn Magnetism Discovery with leftover generated card");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            22,
            "early-turn Magnetism-generated Discovery that leaves another generated card burns two discarded generations"
        );
    }

    #[test]
    fn discovery_retrieve_mayhem_play_top_magnetism_source_burns_one_generation() {
        use crate::content::cards::{DISCOVERY_ID, FLASH_OF_STEEL_ID, STRIKE_R_ID};

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.player.powers.magnetism = 2;
        combat.player.powers.mayhem = 1;
        combat.relic_counters.cards_played_this_turn = 1;
        combat.piles.hand = (1..=5)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .chain(std::iter::once(CardInstance {
                combat_only: true,
                magnetism_generated: true,
                ..CardInstance::new(CardId::new(50), FLASH_OF_STEEL_ID)
            }))
            .collect();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: Some(CardInstance {
                combat_only: true,
                magnetism_generated: true,
                ..CardInstance::new(CardId::new(99), DISCOVERY_ID)
            }),
            source_card_force_exhaust: false,
            source_card_play_top: true,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0)
            .expect("Mayhem PlayTop Magnetism Discovery retrieve");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            19,
            "Mayhem PlayTop Magnetism-generated Discovery burns one discarded generation"
        );
    }

    #[test]
    fn discovery_retrieve_with_two_remaining_cards_burns_two_generations() {
        use crate::content::cards::{DAZED_ID, STRIKE_R_ID};

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DAZED_ID),
        ];
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: None,
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0)
            .expect("two-card remaining Discovery retrieve");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            22,
            "two remaining cards including a status burn two discarded generateCardChoices generations"
        );
    }

    #[test]
    fn discovery_retrieve_with_two_remaining_non_status_cards_burns_one_generation() {
        use crate::content::cards::STRIKE_R_ID;

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
        ];
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: None,
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0)
            .expect("two-card remaining non-status Discovery retrieve");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            19,
            "two remaining non-status cards stay one discarded generateCardChoices generation"
        );
    }

    #[test]
    fn discovery_retrieve_magnetism_generated_later_in_turn_burns_one_generation() {
        use crate::content::cards::{DISCOVERY_ID, STRIKE_R_ID};

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.player.powers.magnetism = 1;
        combat.relic_counters.cards_played_this_turn = 3;
        combat.piles.hand = (1..=4)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: Some(CardInstance {
                combat_only: true,
                magnetism_generated: true,
                ..CardInstance::new(CardId::new(99), DISCOVERY_ID)
            }),
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0)
            .expect("later-turn Magnetism Discovery retrieve");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            19,
            "Magnetism-generated Discovery later in the turn burns one discarded generation"
        );
    }

    #[test]
    fn discovery_retrieve_combat_only_with_magnetism_burns_one_generation() {
        use crate::content::cards::{DISCOVERY_ID, STRIKE_R_ID};

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.player.powers.magnetism = 1;
        combat.piles.hand = (1..=4)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: Some(CardInstance {
                combat_only: true,
                ..CardInstance::new(CardId::new(99), DISCOVERY_ID)
            }),
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0)
            .expect("Jack-of-All-Trades Discovery retrieve");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            19,
            "combat-only Discovery that Magnetism did not generate burns one discarded generation"
        );
    }

    #[test]
    fn discovery_retrieve_early_magnetism_source_from_five_card_hand_burns_one_generation() {
        use crate::content::cards::{DISCOVERY_ID, STRIKE_R_ID};

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.player.powers.magnetism = 1;
        combat.relic_counters.cards_played_this_turn = 1;
        combat.piles.hand = (1..=5)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: Some(CardInstance {
                combat_only: true,
                magnetism_generated: true,
                ..CardInstance::new(CardId::new(99), DISCOVERY_ID)
            }),
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0)
            .expect("early-turn Magnetism Discovery from a 5-card remaining hand");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            19,
            "early-turn Magnetism-generated Discovery from a 5-card remaining hand burns one discarded generation"
        );
    }

    #[test]
    fn discovery_retrieve_combat_only_without_magnetism_burns_one_generation() {
        use crate::content::cards::{DISCOVERY_ID, STRIKE_R_ID};

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.piles.hand = (1..=4)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: Some(CardInstance {
                combat_only: true,
                ..CardInstance::new(CardId::new(99), DISCOVERY_ID)
            }),
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0)
            .expect("combat-only Discovery without Magnetism");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            19,
            "combat-generated Discovery without Magnetism burns one discarded generation"
        );
    }

    #[test]
    fn discovery_retrieve_with_six_remaining_cards_without_awakened_one_burns_one_generation() {
        use crate::content::cards::STRIKE_R_ID;

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        combat.piles.hand = (1..=6)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                STRIKE_R_ID,
            )],
            source_card: None,
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0)
            .expect("full-hand Discovery retrieve without Awakened One");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            19,
            "6 remaining cards without Awakened One stay one discarded generation"
        );
    }

    #[test]
    fn played_discovery_choice_does_not_reuse_its_held_source_id() {
        use crate::content::cards::DISCOVERY_ID;

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let source_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        let choice_id = CardId::new(source_id.get() + 1);
        let chosen_id = CardId::new(choice_id.get() + 1);
        let choice_content = combat.piles.hand[0].content_id;
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(choice_id, choice_content)],
            source_card: Some(CardInstance::new(source_id, DISCOVERY_ID)),
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice(&run, 0).expect("Discovery card choice");
        let combat = next.combat.expect("combat remains open");

        combat
            .validate()
            .expect("held Discovery and generated choice IDs remain unique");
        assert_eq!(
            combat.piles.hand.last().map(|card| card.id),
            Some(chosen_id)
        );
        assert!(combat
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == source_id && card.content_id == DISCOVERY_ID));
    }

    #[test]
    fn skipped_discovery_choice_closes_source_without_creating_the_card() {
        use crate::content::cards::DISCOVERY_ID;

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.rng.card_random_rng = StsRng::with_counter(-571_295_464_674_976_203, 16);
        let original_ids = combat
            .piles
            .hand
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>();
        let source_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        let choice_content = crate::content::cards::DARK_EMBRACE_ID;
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(
                CardId::new(source_id.get() + 1),
                choice_content,
            )],
            source_card: Some(CardInstance::new(source_id, DISCOVERY_ID)),
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: Default::default(),
        });

        let next = apply_combat_card_reward_choice_skipped_discovery_retrieval(&run, 0)
            .expect("skipped Discovery retrieval");
        let combat = next.combat.expect("combat remains open");

        assert_eq!(
            combat
                .piles
                .hand
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            original_ids
        );
        assert!(combat
            .piles
            .hand
            .iter()
            .chain(combat.piles.draw_pile.iter())
            .chain(combat.piles.discard_pile.iter())
            .chain(combat.piles.exhaust_pile.iter())
            .chain(combat.piles.limbo.iter())
            .all(|card| card.content_id != choice_content || original_ids.contains(&card.id)));
        assert!(combat
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == source_id && card.content_id == DISCOVERY_ID));
        assert_eq!(
            combat.rng.card_random_rng.counter(),
            16,
            "opening-complete Discovery does not burn a discarded post-select generation"
        );
    }

    #[test]
    fn skipped_secret_technique_choice_closes_source_without_retrieving_the_skill() {
        use crate::content::cards::{DEFEND_R_ID, SECRET_TECHNIQUE_ID, STRIKE_R_ID};

        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let source_id = CardId::new(200);
        let selected_id = CardId::new(201);
        combat.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        combat.piles.draw_pile = vec![
            CardInstance::new(selected_id, DEFEND_R_ID),
            CardInstance::new(CardId::new(202), STRIKE_R_ID),
        ];
        combat.piles.limbo = vec![CardInstance::new(source_id, SECRET_TECHNIQUE_ID)];
        combat.piles.exhaust_pile.clear();
        combat.decision = Some(CombatDecisionState::DrawSelect {
            state: crate::combat::DrawSelectState {
                purpose: crate::combat::DrawSelectPurpose::SecretTechniqueSkillToHand,
                source_card_id: source_id,
                selectable_card_ids: vec![selected_id],
                selected_draw_index: None,
                pending_actions: Default::default(),
            },
        });

        let next = apply_draw_select_choice_skipped_retrieval(&run, 0)
            .expect("skipped Secret Technique retrieval");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(combat.piles.hand.len(), 1);
        assert_eq!(combat.piles.draw_pile[0].id, selected_id);
        assert!(combat
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == source_id && card.content_id == SECRET_TECHNIQUE_ID));
        assert!(combat.draw_select().is_none());
    }

    #[test]
    fn toolbox_card_choice_is_inserted_at_front_of_hand() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let original_ids = combat
            .piles
            .hand
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>();
        let chosen_id = CardId::new(
            combat
                .next_card_instance_id()
                .expect("fixture has card ID allocation headroom"),
        );
        let choice_content = combat.piles.hand[0].content_id;
        combat.decision = Some(CombatDecisionState::ToolboxCardReward {
            choices: vec![CardInstance::new(
                CardId::new(chosen_id.get() + 1),
                choice_content,
            )],
        });

        let next = apply_combat_card_reward_choice(&run, 0).expect("Toolbox card choice");
        let hand = &next.combat.expect("combat remains open").piles.hand;

        assert_eq!(hand[0].id, chosen_id);
        assert_eq!(
            hand[1..].iter().map(|card| card.id).collect::<Vec<_>>(),
            original_ids
        );
    }

    #[test]
    fn happy_flower_energy_waits_for_opening_toolbox_choice() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.relics = vec![
            crate::relic::Relic::HappyFlower,
            crate::relic::Relic::Toolbox,
        ];
        run.happy_flower_turns = 2;

        let combat = run
            .init_combat(CombatState::initial_fixture())
            .expect("combat initializes");
        assert!(combat.toolbox_card_reward_choices().is_some());
        assert_eq!(combat.relic_counters.happy_flower_turns, 0);
        assert_eq!(combat.player.energy, 3);
        assert_eq!(combat.pending_start_of_turn_relic_energy, 1);

        let json = serde_json::to_string(&combat).expect("combat serializes");
        let combat: CombatState = serde_json::from_str(&json).expect("combat deserializes");
        run.combat = Some(combat);

        let next = apply_combat_card_reward_choice(&run, 0).expect("Toolbox card choice");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(combat.player.energy, 4);
        assert_eq!(combat.pending_start_of_turn_relic_energy, 0);
    }

    #[test]
    fn toolbox_opening_queue_keeps_hand_and_anchor_block_pending_until_choice() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.relics = vec![crate::relic::Relic::Anchor, crate::relic::Relic::Toolbox];

        let combat = run
            .init_combat(CombatState::initial_fixture())
            .expect("combat initializes");
        assert!(combat.toolbox_card_reward_choices().is_some());
        assert!(combat.piles.hand.is_empty());
        assert_eq!(combat.piles.draw_pile.len(), 4);
        assert_eq!(combat.player.block, 0);
        assert_eq!(combat.pending_opening_hand_draw, 3);
        assert_eq!(combat.pending_opening_combat_block, 10);

        run.combat = Some(combat);
        let next = apply_combat_card_reward_choice(&run, 0).expect("Toolbox card choice");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(combat.piles.hand.len(), 4);
        assert_eq!(combat.player.block, 10);
        assert_eq!(combat.pending_opening_hand_draw, 0);
        assert_eq!(combat.pending_opening_combat_block, 0);
    }

    #[test]
    fn toolbox_defers_mark_of_pain_wounds_until_after_choice() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.relics = vec![
            crate::relic::Relic::MarkOfPain,
            crate::relic::Relic::Toolbox,
        ];

        let mut combat = run
            .init_combat(CombatState::initial_fixture())
            .expect("combat initializes");
        crate::run::map::add_mark_of_pain_wounds_to_draw_pile(&mut run, &mut combat)
            .expect("deferred Mark of Pain at combat entry");
        assert!(combat.toolbox_card_reward_choices().is_some());
        assert!(
            combat
                .piles
                .draw_pile
                .iter()
                .all(|card| card.content_id != crate::content::cards::WOUND_ID),
            "Wounds must not appear on the Toolbox reward screen"
        );
        run.combat = Some(combat);

        let next = apply_combat_card_reward_choice(&run, 0).expect("Toolbox card choice");
        let combat = next.combat.expect("combat remains open");
        let wound_count = combat
            .piles
            .draw_pile
            .iter()
            .filter(|card| card.content_id == crate::content::cards::WOUND_ID)
            .count();
        assert_eq!(wound_count, crate::relic::MARK_OF_PAIN_WOUNDS);
    }

    #[test]
    fn mercury_hourglass_damage_waits_for_opening_toolbox_choice() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.relics = vec![
            crate::relic::Relic::MercuryHourglass,
            crate::relic::Relic::Toolbox,
        ];

        let combat = run
            .init_combat(CombatState::initial_fixture())
            .expect("combat initializes");
        assert!(combat.toolbox_card_reward_choices().is_some());
        assert_eq!(combat.monsters[0].hp, combat.monsters[0].max_hp);
        assert_eq!(
            combat.pending_start_of_turn_relic_damage,
            crate::relic::MERCURY_HOURGLASS_DAMAGE
        );

        run.combat = Some(combat);
        let next = apply_combat_card_reward_choice(&run, 0).expect("Toolbox card choice");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(
            combat.monsters[0].hp,
            combat.monsters[0].max_hp - crate::relic::MERCURY_HOURGLASS_DAMAGE
        );
        assert_eq!(combat.pending_start_of_turn_relic_damage, 0);
    }

    #[test]
    fn skipped_toolbox_choice_settles_opening_queue_without_adding_the_card() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.relics = vec![crate::relic::Relic::Anchor, crate::relic::Relic::Toolbox];

        let combat = run
            .init_combat(CombatState::initial_fixture())
            .expect("combat initializes");
        run.combat = Some(combat);
        let next = apply_combat_card_reward_choice_skipped_toolbox_retrieval(&run, 0)
            .expect("skipped Toolbox retrieval");
        let combat = next.combat.expect("combat remains open");
        assert_eq!(combat.piles.hand.len(), 3);
        assert!(combat.decision.is_none());
        assert_eq!(combat.player.block, 10);
        assert_eq!(combat.pending_opening_hand_draw, 0);
        assert_eq!(combat.pending_opening_combat_block, 0);
    }

    #[test]
    fn pending_relic_energy_overflow_is_atomic_and_rejected_by_toolbox() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let choice_content = combat.piles.hand[0].content_id;
        combat.player.energy = i32::MAX;
        combat.pending_start_of_turn_relic_energy = 1;
        combat.decision = Some(CombatDecisionState::ToolboxCardReward {
            choices: vec![CardInstance::new(CardId::new(10_000), choice_content)],
        });
        let combat_before = combat.clone();

        assert_eq!(
            crate::relic::settle_pending_start_of_turn_relic_actions(combat),
            Err(SimError::InvalidState(
                "pending start-of-turn relic energy overflows i32"
            ))
        );
        assert_eq!(*combat, combat_before);

        let before = run.clone();

        assert_eq!(
            apply_combat_card_reward_choice(&run, 0),
            Err(SimError::InvalidState(
                "pending start-of-turn relic energy overflows i32"
            ))
        );
        assert_eq!(run, before);
    }

    #[test]
    fn toolbox_choice_activates_queued_gambling_chip_selection() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.relics = vec![
            crate::relic::Relic::GamblingChip,
            crate::relic::Relic::Toolbox,
        ];

        let combat = run
            .init_combat(CombatState::initial_fixture())
            .expect("combat initializes");
        assert!(matches!(
            combat.decision.as_ref(),
            Some(CombatDecisionState::ToolboxCardReward { .. })
        ));
        assert!(matches!(
            combat.queued_decisions.front(),
            Some(CombatDecisionState::ExhaustSelect { state })
                if state.purpose == ExhaustSelectPurpose::GamblingChip
        ));
        run.combat = Some(combat);

        let next = apply_combat_card_reward_choice(&run, 0).expect("Toolbox card choice");
        let combat = next.combat.as_ref().expect("combat remains open");
        assert!(matches!(
            combat.decision.as_ref(),
            Some(CombatDecisionState::ExhaustSelect { state })
                if state.purpose == ExhaustSelectPurpose::GamblingChip
        ));
        assert!(combat.queued_decisions.is_empty());

        let next = apply_exhaust_select_confirm(&next).expect("Gambling Chip confirmation");
        assert!(next.combat.expect("combat remains open").decision.is_none());
    }

    #[test]
    fn purity_confirm_draws_unceasing_top_when_hand_empties() {
        // FIDL01681 step 1290: Purity exhausts the rest of the hand; Unceasing
        // Top then draws the current top (Warcry).
        let mut run = RunState::combat_fixture_with_relics(vec![Relic::UnceasingTop]);
        {
            let combat = run.combat.as_mut().expect("combat");
            combat.player.energy = 0;
            combat.piles.hand = vec![
                CardInstance::new(CardId::new(1), crate::content::cards::PURITY_ID),
                CardInstance::new(CardId::new(2), BASH_ID),
                CardInstance::new(CardId::new(3), crate::content::cards::BODY_SLAM_ID),
                CardInstance::new(CardId::new(4), crate::content::cards::POMMEL_STRIKE_ID),
            ];
            combat.piles.draw_pile = vec![CardInstance::new(
                CardId::new(5),
                crate::content::cards::WARCRY_ID,
            )];
            combat.piles.discard_pile.clear();
            combat.piles.exhaust_pile.clear();
        }
        let opened = apply_combat_action_on_run(
            &run,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Purity opens exhaust select");
        let chosen = apply_run_action(&opened, RunAction::ChooseExhaustSelect { index: 0 })
            .expect("choose 0");
        let chosen = apply_run_action(&chosen, RunAction::ChooseExhaustSelect { index: 0 })
            .expect("choose 1");
        let chosen = apply_run_action(&chosen, RunAction::ChooseExhaustSelect { index: 0 })
            .expect("choose 2");
        let next = apply_exhaust_select_confirm(&chosen).expect("Purity CONFIRM");
        let combat = next.combat.as_ref().expect("combat");
        assert_eq!(
            combat
                .piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![crate::content::cards::WARCRY_ID],
            "Unceasing Top should draw Warcry after Purity empties the hand"
        );
    }

    #[test]
    fn purity_confirm_dead_branch_precedes_dark_embrace_draw() {
        // FIDL01373 step 1145: Purity CONFIRM with no selection exhausts Purity.
        // AbstractPlayer.onExhaust runs relics before powers, so Dead Branch's
        // MakeTempCardInHand is addToBot before Dark Embrace's DrawCardAction.
        let mut run = RunState::combat_fixture_with_relics(vec![Relic::DeadBranch]);
        {
            let combat = run.combat.as_mut().expect("combat");
            combat.player.energy = 0;
            combat.player.powers.dark_embrace = 1;
            combat.piles.hand = vec![
                CardInstance::new(CardId::new(1), PURITY_ID),
                CardInstance::new(CardId::new(2), CLASH_ID),
            ];
            combat.piles.draw_pile = vec![CardInstance::new(CardId::new(3), DARK_EMBRACE_ID)];
            combat.piles.discard_pile.clear();
            combat.piles.exhaust_pile.clear();
        }
        let opened = apply_combat_action_on_run(
            &run,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Purity opens exhaust select");
        let next = apply_exhaust_select_confirm(&opened).expect("Purity CONFIRM with no picks");
        let combat = next.combat.as_ref().expect("combat");
        combat.validate().expect("Purity CONFIRM piles stay unique");
        let hand: Vec<_> = combat.piles.hand.iter().collect();
        assert_eq!(
            hand.first().map(|card| card.content_id),
            Some(CLASH_ID),
            "Clash remains in hand: {:?}",
            hand.iter().map(|card| card.content_id).collect::<Vec<_>>()
        );
        assert!(
            hand.get(1).is_some_and(|card| card.combat_only),
            "Dead Branch should generate before Dark Embrace draws: {:?}",
            hand.iter().map(|card| card.content_id).collect::<Vec<_>>()
        );
        assert_eq!(
            hand.get(2).map(|card| card.content_id),
            Some(DARK_EMBRACE_ID),
            "Dark Embrace should draw after Dead Branch generates: {:?}",
            hand.iter().map(|card| card.content_id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn purity_multi_select_dead_branch_keeps_unique_card_ids() {
        // FIDL01582 step 967: three selected cards plus Purity. Purity holds the
        // highest instance id so Dead Branch cannot reuse it. Full retrieval
        // must remain a valid combat state so skipped-retrieval can still be
        // considered when the observed frame parked the selection.
        let mut run = RunState::combat_fixture_with_relics(vec![Relic::DeadBranch]);
        {
            let combat = run.combat.as_mut().expect("combat");
            combat.player.energy = 1;
            combat.piles.hand = vec![
                CardInstance::new(CardId::new(2), STRIKE_R_ID),
                CardInstance::new(CardId::new(3), STRIKE_R_ID),
                CardInstance::new(CardId::new(4), STRIKE_R_ID),
                CardInstance::new(CardId::new(5), CLASH_ID),
                CardInstance::new(CardId::new(50), PURITY_ID),
            ];
            combat.piles.draw_pile.clear();
            combat.piles.discard_pile.clear();
            combat.piles.exhaust_pile.clear();
        }
        let opened = apply_combat_action_on_run(
            &run,
            CombatAction::PlayCard {
                card_id: CardId::new(50),
                target: None,
            },
        )
        .expect("Purity opens exhaust select");
        let mut selected = opened;
        for _ in 0..3 {
            selected = apply_run_action(&selected, RunAction::ChooseExhaustSelect { index: 0 })
                .expect("select a card");
        }
        let next =
            apply_exhaust_select_confirm(&selected).expect("Purity CONFIRM with three picks");
        let combat = next.combat.as_ref().expect("combat");
        combat
            .validate()
            .expect("multi-select Purity plus Dead Branch must not duplicate card ids");
        assert_eq!(
            combat
                .piles
                .hand
                .iter()
                .filter(|card| card.content_id == CLASH_ID && !card.combat_only)
                .count(),
            1,
            "unselected Clash remains: {:?}",
            combat
                .piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            combat
                .piles
                .exhaust_pile
                .iter()
                .filter(|card| card.content_id == PURITY_ID)
                .count(),
            1
        );
    }

    #[test]
    fn secret_technique_skipped_retrieval_dead_branch_precedes_dark_embrace() {
        // FIDL01373 step 1181: CHOOSE skips retrieving Entrench. Exhausting
        // Secret Technique still runs relic onExhaust (Dead Branch) before
        // Dark Embrace draws the top two cards.
        let mut run = RunState::combat_fixture_with_relics(vec![Relic::DeadBranch]);
        {
            let combat = run.combat.as_mut().expect("combat");
            combat.player.energy = 0;
            combat.player.powers.dark_embrace = 2;
            combat.piles.hand = vec![
                CardInstance::new(CardId::new(1), SECRET_TECHNIQUE_ID),
                CardInstance::new(CardId::new(2), CLASH_ID),
            ];
            combat.piles.draw_pile = vec![
                CardInstance::new(CardId::new(3), DEFEND_R_ID),
                CardInstance::new(CardId::new(6), PURITY_ID),
                CardInstance::new(CardId::new(4), STRIKE_R_ID),
                CardInstance::new(CardId::new(5), ANGER_ID),
            ];
            combat.piles.discard_pile.clear();
            combat.piles.exhaust_pile.clear();
        }
        let opened = apply_combat_action_on_run(
            &run,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Secret Technique opens draw select");
        let next = apply_draw_select_choice_skipped_retrieval(&opened, 0)
            .expect("skipped Secret Technique retrieval");
        let combat = next.combat.as_ref().expect("combat");
        combat.validate().expect("unique card ids");
        assert!(
            combat
                .piles
                .draw_pile
                .iter()
                .any(|card| card.content_id == DEFEND_R_ID),
            "selected skill stays in draw"
        );
        let hand: Vec<_> = combat.piles.hand.iter().collect();
        assert_eq!(hand[0].content_id, CLASH_ID);
        assert!(
            hand[1].combat_only,
            "Dead Branch should land before Dark Embrace draws: {:?}",
            hand.iter().map(|card| card.content_id).collect::<Vec<_>>()
        );
        assert_eq!(hand[2].content_id, ANGER_ID);
        assert_eq!(hand[3].content_id, STRIKE_R_ID);
    }
}
