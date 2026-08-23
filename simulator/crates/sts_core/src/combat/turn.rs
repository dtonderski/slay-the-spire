use std::collections::VecDeque;

use crate::{
    combat::turn_powers::{
        apply_demon_form_strength_post_draw, apply_end_of_monster_turn_powers,
        apply_end_of_monster_turn_powers_without_ritual,
    },
    combat::{
        draw::{draw_cards_with_combat_rng_deferred_evolve, MAX_HAND_SIZE},
        hand::{
            discard_end_of_turn_hand, resolve_deferred_dark_embrace_draws,
            resolve_end_of_turn_hand_with_queued_autoplay,
        },
        piles::{
            add_cards_to_discard, add_cards_to_draw_random_spot, add_upgraded_burns_to_discard,
            upgrade_burns_and_add_upgraded_to_discard,
        },
        transition::resolve_deferred_draw_follow_ups,
    },
    combat::{CombatPhase, CombatState, SlimeSize},
    content::cards::{
        get_card_definition, BURN_ID, DAZED_ID, DECAY_ID, DOUBT_ID, REGRET_ID, SHAME_ID, SLIMED_ID,
        VOID_ID, WOUND_ID,
    },
    content::monsters::{
        apply_bronze_automaton_orb_spawn, apply_collector_spawn_torch_heads,
        apply_gremlin_leader_encourage, apply_gremlin_leader_rally_target, apply_heal_all_monsters,
        apply_large_acid_slime_split, apply_large_spike_slime_split,
        apply_monster_intent_with_card_rng_and_revival, apply_queued_post_attack_player_debuffs,
        apply_reptomancer_dagger_spawn, apply_slime_boss_split, apply_strength_all_monsters,
        awaken_one_after_first_death, awakened_one_is_half_dead, champ_strength_amount,
        check_slime_boss_split, clear_lagavulin_metallicize_if_awake,
        has_queued_post_attack_player_debuffs, heal_monster_to_stored_cap,
        living_monster_missing_hp, prepare_monster_intent_for_ascension,
        prepare_monster_intent_with_card_rng_and_revival, record_target_move,
        source_backed_gremlin_leader_minion_intent,
        target_book_of_stabbing_next_intent_from_roll_with_stab_count,
        target_bronze_automaton_next_intent, target_bronze_orb_next_intent_from_roll,
        target_byrd_flight_amount, target_byrd_go_airborne_intent,
        target_byrd_next_intent_from_roll, target_centurion_next_intent_from_roll,
        target_champ_next_intent_from_roll, target_chosen_next_intent_from_roll,
        target_collector_next_intent_from_roll, target_exploder_next_intent_from_roll,
        target_fungi_beast_next_intent_from_roll, target_giant_head_next_intent_from_roll,
        target_gremlin_leader_next_intent_from_roll, target_gremlin_nob_next_intent_from_roll,
        target_gremlin_wizard_direct_next_intent_after_turn, target_grounded_byrd_next_intent,
        target_healer_next_intent_from_roll, target_jaw_worm_next_intent_from_roll,
        target_lagavulin_direct_wake_attack_intent, target_large_acid_slime_next_intent_from_roll,
        target_looter_direct_next_intent_after_turn, target_louse_next_intent_from_roll,
        target_maw_next_intent_from_roll, target_medium_acid_slime_next_intent_from_roll,
        target_medium_or_large_spike_slime_next_intent_from_roll_with_profile,
        target_mugger_direct_next_intent_after_turn, target_nemesis_next_intent_from_roll,
        target_orb_walker_next_intent_from_roll, target_reptomancer_next_intent_from_roll,
        target_repulsor_next_intent_from_roll, target_sentry_next_intent,
        target_shelled_parasite_next_intent_from_roll,
        target_shelled_parasite_shell_break_roll_move, target_slaver_blue_next_intent_from_roll,
        target_slaver_red_next_intent_from_roll, target_small_acid_slime_followup_intent,
        target_snake_plant_next_intent_from_roll, target_snecko_next_intent_from_roll,
        target_spheric_guardian_next_intent_from_roll, target_spiker_next_intent_from_roll,
        target_spire_growth_next_intent_from_roll, target_taskmaster_wound_count,
        target_writhing_mass_next_intent_from_roll, ACID_SLIME_ID, ACID_SLIME_M_A7_HP_RANGE,
        ACID_SLIME_S_A7_HP_RANGE, BOOK_OF_STABBING_ID, BRONZE_AUTOMATON_ID, BRONZE_ORB_ID, BYRD_ID,
        CENTURION_ID, CHAMP_ID, CHOSEN_ID, CORRUPT_HEART_ID, DAGGER_EXPLODE_DAMAGE, DAGGER_ID,
        DARKLING_ID, DECA_ID, EXPLODER_ID, FUNGI_BEAST_ID, GIANT_HEAD_ID, GREEN_LOUSE_ID,
        GREEN_LOUSE_WEAK, GREMLIN_LEADER_ID, GREMLIN_NOB_ID, GREMLIN_THIEF_ID, GREMLIN_TSUNDERE_ID,
        GREMLIN_WARRIOR_ID, GREMLIN_WIZARD_ID, HEALER_ID, HEXAGHOST_ID, JAW_WORM_ID, LAGAVULIN_ID,
        LOOTER_ID, LOUSE_CURL_STRENGTH, MAW_ID, MUGGER_ID, NEMESIS_ID, ORB_WALKER_ID, RED_LOUSE_ID,
        REPTOMANCER_ID, REPULSOR_ID, SENTRY_ID, SHELLED_PARASITE_ID, SLAVER_BLUE_ID, SLAVER_RED_ID,
        SLIME_BOSS_ID, SNAKE_PLANT_ID, SNECKO_ID, SPHERIC_GUARDIAN_ID, SPIKER_ID, SPIKE_SLIME_ID,
        SPIKE_SLIME_L_SPIT_DAMAGE, SPIKE_SLIME_S_A7_HP_RANGE, SPIRE_GROWTH_ID, SPIRE_SHIELD_ID,
        SPIRE_SPEAR_ID, THE_COLLECTOR_ID, TORCH_HEAD_ID, TRANSIENT_ID, WRITHING_MASS_ID,
    },
    ids::MonsterId,
    relic::HpLossDrawPolicy,
    rng::StsRng,
    SimError, SimResult,
};

const HAND_SIZE: usize = 5;

/// Simplified milestone timing:
///
/// 1. Ending the player turn discards the remaining hand.
/// 2. The monster turn consumes current player block before HP.
/// 3. Player block clears after the monster turn, before the next hand is drawn.
/// 4. Monster vulnerable decrements by 1 during monster-turn cleanup.
/// 5. The next player turn refills energy and draws from the draw pile without shuffle.
pub fn end_player_turn(state: &CombatState) -> SimResult<CombatState> {
    let mut next = state.clone();
    if next.pending_end_turn_feel_no_pain_block > 0 {
        next.player.block = next.player.block.saturating_add(std::mem::take(
            &mut next.pending_end_turn_feel_no_pain_block,
        ));
    }
    let started_with_living_monster = state.monsters.iter().any(|monster| monster.alive);
    let resuming_after_nilrys = next.resume_end_turn_after_nilrys_codex;
    let resuming_time_warp_monster_action = next.time_warp_pending_monster_action;
    next.time_warp_pending_monster_action = false;
    let pre_discard_settled = next.time_warp_end_turn_pre_discard_settled;
    next.time_warp_end_turn_pre_discard_settled = false;
    // A non-duplicated forced Time Warp END can publish after monster turn
    // setup but before the captured monster action. Duplicate-queue traces use
    // the established full queue path and are deliberately excluded.
    let defer_time_warp_monster_action = next.time_warp_end_turn
        && !next.time_warp_duplicate_monster_queue
        && !next.defer_time_warp_end_turn
        && !resuming_time_warp_monster_action
        // Time Eater's Haste queue publishes its cleanup/next-roll frame
        // before the captured action; ordinary forced attacks still use the
        // established complete monster-turn path.
        && next.monsters.iter().any(|monster| {
            monster.alive
                && monster.content_id == crate::content::monsters::TIME_EATER_ID
                && matches!(
                    monster.intent,
                    crate::MonsterIntent::StrengthSelf { amount: 0 }
                )
        });
    // DiscardAction leftover-selectedCards settlement keys off whether the
    // hand was non-empty when END was clicked — not after ethereal exhaust
    // empties an only-status hand (FIDL00278 Warcry skipped Inflame + Dazed).
    let hand_nonempty_at_end_click = !next.piles.hand.is_empty();
    let mut deferred_stasis_cards;
    let mut deferred_monster_deaths = Vec::new();
    let end_of_turn_hand;

    if pre_discard_settled {
        deferred_stasis_cards = Vec::new();
        end_of_turn_hand = crate::combat::hand::exhaust_unplayed_ethereal_cards(&mut next)?;
    } else if resuming_after_nilrys {
        // Nilry's Codex already ran the pre-discard half of end-turn. Resume
        // after the card-reward decision with hand still present.
        next.resume_end_turn_after_nilrys_codex = false;
        apply_pending_nilry_end_powers(&mut next)?;
        crate::relic::nilrys_codex_flush_pending_draw_inserts(&mut next)?;
        deferred_stasis_cards = Vec::new();
        let dead_branch_cards = std::mem::take(&mut next.pending_end_turn_dead_branch_cards);
        let deferred_dark_embrace_draws =
            std::mem::take(&mut next.pending_end_turn_dark_embrace_draws);
        let mut ethereal_follow_ups = dead_branch_cards
            .iter()
            .cloned()
            .map(crate::combat::hand::EtherealEndTurnFollowUp::DeadBranch)
            .collect::<Vec<_>>();
        ethereal_follow_ups.extend(std::iter::repeat_n(
            crate::combat::hand::EtherealEndTurnFollowUp::DarkEmbraceDraw,
            deferred_dark_embrace_draws,
        ));
        end_of_turn_hand = crate::combat::hand::EndOfTurnHandResolution {
            auto_play_emptied_hand: false,
            dead_branch_cards,
            deferred_dark_embrace_draws,
            ethereal_follow_ups,
            deferred_juggernaut_damage: std::mem::take(
                &mut next.pending_end_turn_juggernaut_damage,
            ),
        };
    } else {
        let stasis_cards_before_end_powers = next
            .monsters
            .iter()
            .filter(|monster| monster.alive)
            .filter_map(|monster| monster.stasis_card.as_ref().map(|card| card.id))
            .collect::<Vec<_>>();

        // Stone Calendar's lethal trigger settles before the queued end-turn
        // self-loss powers in the target; the combat reward therefore does not
        // include Combust/Constricted HP loss (FIDL00282, FIDL00228 Spire Growth).
        // Non-lethal Calendar triggers retain the ordinary end-turn ordering.
        // Unmodified Calendar damage consumes block first (FIDL00415 Spheric
        // Guardian Barricade: 20 HP + 60 block survives 52).
        let stone_calendar_would_finish = next.relics.contains(&crate::relic::Relic::StoneCalendar)
            && next.relic_counters.player_turns_started == crate::relic::STONE_CALENDAR_TURN
            && next.monsters.iter().any(|monster| monster.alive)
            && next
                .monsters
                .iter()
                .filter(|monster| monster.alive)
                .all(|monster| {
                    let block = monster.block.max(0);
                    let hp = monster.hp.max(0);
                    match hp.checked_add(block) {
                        Some(total) => total <= crate::relic::STONE_CALENDAR_DAMAGE,
                        None => false,
                    }
                });
        if stone_calendar_would_finish {
            crate::relic::apply_end_of_player_turn_relics(&mut next)?;
            if finish_combat_if_over(&mut next, started_with_living_monster)? {
                return Ok(next);
            }
        }
        // NoBlockPower expires from its end-of-turn hook before Orichalcum's
        // end-of-turn relic hook runs. This lets the relic's block protect the
        // next monster turn when the final prevented turn has elapsed.
        if next.player.no_block_turns > 0 {
            next.player.no_block_turns -= 1;
        }
        // Slay the Spire checks Orichalcum before queued end-of-turn powers such as
        // Metallicize resolve. Both block grants therefore apply when the player
        // clicks End Turn with zero block.
        crate::relic::apply_orichalcum_end_of_player_turn(&mut next)?;
        // Orichalcum's direct block callback can trigger Juggernaut before
        // Combust/other end-turn self-loss powers. A lethal callback ends the
        // combat before those later powers are queued (FIDL01441).
        if finish_combat_if_over(&mut next, started_with_living_monster)? {
            return Ok(next);
        }
        // GameActionManager.callEndOfTurnActions queues relic onPlayerEndTurn
        // (Nilry CodexAction) before power atEndOfTurn (Combust).
        if next.relics.contains(&crate::relic::Relic::NilrysCodex)
            && next.decision.is_none()
            && next.monsters.iter().any(|monster| monster.alive)
        {
            crate::relic::open_nilrys_codex_card_reward(&mut next)?;
            next.resume_end_turn_after_nilrys_codex = true;
            next.nilrys_end_powers_pending = true;
            next.pending_nilrys_codex_draw_inserts.clear();
            return Ok(next);
        }
        let queued_autoplay = queued_end_turn_autoplay_ids(&next);
        let defer_combust_until_after_autoplay = hand_has_end_turn_autoplay_cards(&next);
        let bomb_caused_terminal = crate::combat::turn_powers::apply_end_of_player_turn_powers_before_hand_deferred_with_combust(
            &mut next,
            &mut deferred_monster_deaths,
            !defer_combust_until_after_autoplay,
        )?;
        // GameActionManager.callEndOfTurnActions queues TriggerEndOfTurnOrbsAction
        // after pre-card powers and before triggerOnEndOfTurnForPlayingCard.
        apply_end_of_turn_orb_passives(&mut next)?;
        // Metallicize.atEndOfTurnPreEndTurnCards addToBots GainBlock before
        // Regret is queued. Juggernaut can empty the field there and cancel
        // the card queue (FIDL02289 Giant Head 2 HP).
        // Only a firing Bomb that actually changed the encounter to terminal
        // uses its distinct pre-victory hand ordering. A pending timer, or a
        // timer present after another power caused lethal, follows the ordinary
        // terminal path.
        if !bomb_caused_terminal && finish_combat_if_over(&mut next, started_with_living_monster)? {
            return Ok(next);
        }
        // The Bomb's end-turn explosion can end combat before hand cleanup.
        // callEndOfTurnActions still plays Burn/Decay/Regret first (FIDL01533,
        // FIDL02376). Keep this limited to Bomb-triggered victory; other
        // end-turn powers retain the ordinary hand-before-victory ordering.
        // Constricted still resolves in that pre-hand bomb-lethal window as
        // blockable THORNS loss before Burning Blood (FIDL00403: 3876 block 5
        // / Constricted 10 → 3877 after BB +6). Combust-only lethals continue
        // to skip Constricted after hand (FIDL00443).
        if bomb_caused_terminal
            && started_with_living_monster
            && next
                .monsters
                .iter()
                .all(|monster| !monster.alive && !awakened_one_is_half_dead(monster))
        {
            // Constricted may already have run in before_hand when Combust was
            // also active (FIDL00440 ordering).
            crate::combat::hand::apply_end_of_turn_burn_and_decay_for_bomb_victory(&mut next)?;
            if !crate::combat::turn_powers::constricted_resolved_before_hand_with_combust(&next) {
                crate::combat::turn_powers::apply_end_of_turn_constricted(&mut next)?;
            }
            if finish_combat_if_over(&mut next, started_with_living_monster)? {
                return Ok(next);
            }
        }
        expire_unused_duplication_potion_stack(&mut next);
        resolve_player_temp_strength(&mut next)?;
        // Juggernaut can kill a Stasis-holding Bronze Orb from the immediate
        // Metallicize/Plated Armor block callback. `apply_monster_death_hooks`
        // has already published that card into hand (or discard when hand is
        // full), so it must stay in that pile and be processed by the ordinary
        // end-turn discard. Only cards still absent from the piles belong to
        // the deferred post-discard callback path (FIDL01447; Combust keeps
        // its existing deferred ordering).
        let stasis_cards_already_published = stasis_cards_before_end_powers
            .iter()
            .copied()
            .filter(|card_id| {
                next.piles
                    .hand
                    .iter()
                    .chain(next.piles.discard_pile.iter())
                    .any(|card| card.id == *card_id)
            })
            .collect::<std::collections::HashSet<_>>();
        let unreleased_stasis_ids = stasis_cards_before_end_powers
            .iter()
            .copied()
            .filter(|card_id| !stasis_cards_already_published.contains(card_id))
            .collect::<Vec<_>>();
        deferred_stasis_cards = if next.monsters.iter().any(|monster| monster.alive) {
            take_released_stasis_cards_from_piles(&mut next, &unreleased_stasis_ids)
        } else {
            Vec::new()
        };
        // Constricted.atEndOfTurn addToBots Damage before DiscardAtEndOfTurn.
        // RunicCube.wasHPLost addToTops DrawCardAction, so that draw lands
        // before ethereal exhaust (FIDL02191 two Apparitions).
        let constricted_before_ethereal = next.player.powers.constricted > 0
            && !crate::combat::turn_powers::constricted_resolved_before_hand_with_combust(&next);
        if constricted_before_ethereal {
            crate::combat::turn_powers::apply_end_of_turn_constricted(&mut next)?;
        }
        end_of_turn_hand =
            resolve_end_of_turn_hand_with_queued_autoplay(&mut next, Some(&queued_autoplay))?;
        // Charon's Ashes (and other on-exhaust damage) can kill a Stasis orb
        // during ethereal settlement, after the pre-hand snapshot. Those
        // cards must return after discard, not ride DiscardAtEndOfTurn into
        // the discard pile (FIDL01646 Berserk). Metallicize/Juggernaut
        // publications from before this call stay in already_published.
        if next.monsters.iter().any(|monster| monster.alive) {
            deferred_stasis_cards.extend(take_released_stasis_cards_from_piles(
                &mut next,
                &unreleased_stasis_ids,
            ));
        }
        if defer_combust_until_after_autoplay {
            // callEndOfTurnActions plays Burn/Decay/Regret first; Combust
            // LoseHPAction is queued from AbstractRoom.endTurn after that.
            crate::combat::turn_powers::apply_deferred_end_of_turn_combust(
                &mut next,
                &mut deferred_monster_deaths,
            )?;
        }
        // Combust (pre-hand) or ethereal burns can kill the last enemy during the
        // end-turn sequence. Once combat is over, skip later self-loss such as
        // Constricted (FIDL00443 Spire Growth) while still having resolved hand
        // ethereals first (FIDL00443 Hexaghost burns).
        if finish_combat_if_over(&mut next, started_with_living_monster)? {
            return Ok(next);
        }
        // Constricted is an end-of-turn power, but the target resolves it after
        // end-turn card losses when Combust is absent. That lets Metallicize
        // block Decay before the non-card Constricted loss (FIDL00415). When
        // Combust is active, Constricted already ran in before_hand (FIDL00440).
        if !crate::combat::turn_powers::constricted_resolved_before_hand_with_combust(&next)
            && !constricted_before_ethereal
        {
            crate::combat::turn_powers::apply_end_of_turn_constricted(&mut next)?;
        }
        crate::combat::turn_powers::apply_end_of_player_turn_regeneration(&mut next)?;
        if finish_combat_if_over(&mut next, started_with_living_monster)? {
            return Ok(next);
        }
        crate::relic::apply_end_of_player_turn_relics(&mut next)?;
        if finish_combat_if_over(&mut next, started_with_living_monster)? {
            return Ok(next);
        }
    }
    // Leftover HandCardSelectScreen.selectedCards re-enter discard via
    // DiscardAction only when the visible hand was non-empty at END click.
    // Empty-hand ENDs hold the card outside every pile through the next refill
    // (Burning Pact deferred exhaust), otherwise the stuck card contaminates
    // the discard→draw shuffle. Ethereal emptying the hand mid end-turn must
    // not block settlement (FIDL00278).
    let pending_hidden_waits_for_fiend_fire =
        !next.pending_hidden_hand_card_until_end_turn.is_empty()
            && (!hand_nonempty_at_end_click || end_of_turn_hand.auto_play_emptied_hand);
    if pending_hidden_waits_for_fiend_fire {
        // A skipped selection whose cards survive an empty END remains owned by
        // the closed selection screen. The next Fiend Fire can exhaust that
        // hidden batch; only a non-empty END may return it to discard.
        next.pending_hidden_hand_card_exhausts_with_fiend_fire = true;
    }
    let settle_pending_hidden_into_discard = should_settle_pending_hidden_into_discard(
        &next,
        hand_nonempty_at_end_click,
        end_of_turn_hand.auto_play_emptied_hand,
    );
    discard_end_of_turn_hand(&mut next)?;
    if settle_pending_hidden_into_discard {
        let pending = std::mem::take(&mut next.pending_hidden_hand_card_until_end_turn);
        next.piles.discard_pile.extend(pending);
        next.pending_hidden_hand_card_exhausts_with_fiend_fire = false;
    }
    // Dead Branch onExhaust and DarkEmbrace onExhaust are both addToBot per
    // ethereal. After the hand discard they resolve in that interleaved order
    // (FIDL02353: DB, draw, DB, draw — not both DBs then both draws).
    next.player.cannot_draw = false;
    let mut deferred_dark_embrace_fire_breathing = Vec::new();
    if end_of_turn_hand.ethereal_follow_ups.is_empty() {
        next.piles.hand.extend(end_of_turn_hand.dead_branch_cards);
    } else {
        for follow_up in end_of_turn_hand.ethereal_follow_ups {
            match follow_up {
                crate::combat::hand::EtherealEndTurnFollowUp::DeadBranch(card) => {
                    next.piles.hand.push(card);
                }
                crate::combat::hand::EtherealEndTurnFollowUp::DarkEmbraceDraw => {
                    deferred_dark_embrace_fire_breathing
                        .extend(resolve_deferred_dark_embrace_draws(&mut next, 1)?);
                }
            }
        }
    }
    // Runic Cube/Centennial Puzzle drew the trigger card before the bulk hand
    // discard, but Evolve/Fire Breathing callbacks were queued behind that
    // discard action in the source manager.
    let hp_loss_follow_ups = std::mem::take(&mut next.pending_hp_loss_draw_follow_ups);
    resolve_deferred_draw_follow_ups(&mut next, hp_loss_follow_ups.into_iter().collect())?;
    next.piles.hand.extend(deferred_stasis_cards);
    crate::combat::transition::resolve_deferred_end_turn_monster_deaths(
        &mut next,
        deferred_monster_deaths,
    )?;
    apply_pending_player_spikes_damage(&mut next)?;
    if next.player.hp <= 0 {
        next.player.hp = 0;
        next.player.block = 0;
        next.phase = CombatPhase::Lost;
        return Ok(next);
    }
    clear_living_monster_block(&mut next);
    resolve_deferred_draw_follow_ups(&mut next, deferred_dark_embrace_fire_breathing)?;
    for amount in end_of_turn_hand.deferred_juggernaut_damage {
        crate::combat::transition::apply_juggernaut_random_damage(&mut next, amount)?;
    }
    // Combust/bomb Mode Shift accumulates during end-of-turn powers; enter
    // defensive mode (and grant 20 block) only after monster pre-turn clear so
    // GainBlock survives into the next player turn — matching target queue order
    // ChangeState → GainBlock after MonsterStartTurn loseBlock.
    crate::content::monsters::resolve_deferred_guardian_mode_shifts(&mut next.monsters);
    next.phase = CombatPhase::MonsterTurn;
    if defer_time_warp_monster_action {
        // The source has already queued the monster roll and next-intent
        // publication, but its action remains behind the next player command.
        // Run only the turn boundary effects now; the next END re-enters this
        // function with `time_warp_pending_monster_action` and executes the
        // captured action through the ordinary path.
        let mut already_rolled = Vec::new();
        let ascension = next.ascension;
        for actor_id in next
            .monsters
            .iter()
            .filter(|monster| monster.alive)
            .map(|monster| monster.id)
            .collect::<Vec<_>>()
        {
            let Some(index) = next
                .monsters
                .iter()
                .position(|monster| monster.id == actor_id)
            else {
                continue;
            };
            if execute_state_oriented_special_intent(&mut next, actor_id, index, ascension)? {
                already_rolled.push(actor_id);
            }
        }
        finish_monster_turn_cleanup(&mut next, &[])?;
        if already_rolled.len() < next.monsters.iter().filter(|monster| monster.alive).count() {
            let remaining = next
                .monsters
                .iter()
                .filter(|monster| monster.alive && !already_rolled.contains(&monster.id))
                .map(|monster| monster.id)
                .collect::<Vec<_>>();
            prepare_next_intents_for_ids(&mut next, Some(&remaining))?;
        }
        next.time_warp_end_turn = false;
        next.time_warp_pending_monster_action = true;
        start_player_turn(&mut next)?;
        return Ok(next);
    }
    run_monster_turn(&mut next)?;
    if resuming_time_warp_monster_action && next.player.hp > 0 {
        // The queued Time Warp publication exposes the next RollMoveAction on
        // the following END after the captured attack settles. The source
        // action manager performs that roll as a distinct queue item.
        prepare_next_intents_for_ids(&mut next, None)?;
    }
    if next.player.hp <= 0 {
        next.player.hp = 0;
        next.player.block = 0;
        next.phase = CombatPhase::Lost;
        return Ok(next);
    }
    if finish_combat_if_over(&mut next, started_with_living_monster)? {
        return Ok(next);
    }

    // This EndTurn consumed Time Warp's forced end. Clearing before draw
    // prevents start_player_turn's settle_time_warp_end_turn_if_ready from
    // running a second full turn (FIDL01425: two queued Reverberates, one
    // Draw Reduction hand, not a follow-up empty END).
    next.time_warp_end_turn = false;
    start_player_turn(&mut next)?;
    Ok(next)
}

fn expire_unused_duplication_potion_stack(state: &mut CombatState) {
    // DuplicationPower.atEndOfRound reduces one unused stack, or removes the
    // power when its last stack expires.
    if state.duplication_potion_stacks > 0 {
        state.duplication_potion_stacks -= 1;
    }
    if state.duplication_potion_stacks == 0 {
        state.duplication_potion_pending = false;
    }
}

fn take_released_stasis_cards_from_piles(
    state: &mut CombatState,
    candidate_ids: &[crate::CardId],
) -> Vec<crate::CardInstance> {
    let mut released = Vec::new();
    for card_id in candidate_ids {
        if let Some(index) = state.piles.hand.iter().position(|card| card.id == *card_id) {
            released.push(state.piles.hand.remove(index));
        } else if let Some(index) = state
            .piles
            .discard_pile
            .iter()
            .position(|card| card.id == *card_id)
        {
            released.push(state.piles.discard_pile.remove(index));
        }
    }
    released
}

fn apply_pending_player_spikes_damage(state: &mut CombatState) -> SimResult<()> {
    let damage = std::mem::take(&mut state.pending_player_spikes_damage);
    if damage <= 0 {
        return Ok(());
    }
    let hp_loss =
        crate::combat::damage::reflect_spikes_to_player(&mut state.player, &state.relics, damage);
    crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss)
}

fn clear_living_monster_block(state: &mut CombatState) {
    for monster in &mut state.monsters {
        if monster.alive && monster.content_id != SPHERIC_GUARDIAN_ID {
            monster.block = 0;
        }
    }
}

/// Finish an opening END that the target already published after its draw:
/// the remaining work is the monster turn and the next hand.
pub fn settle_opening_end_turn_monster_and_draw(state: &mut CombatState) -> SimResult<()> {
    settle_leftover_end_turn_monster_and_draw_with_post_draw_relics(state, true)
}

fn settle_leftover_end_turn_monster_and_draw_with_post_draw_relics(
    state: &mut CombatState,
    apply_post_draw_relics: bool,
) -> SimResult<()> {
    if state.player.hp <= 0 {
        return Ok(());
    }
    // This leftover *is* the Time Warp forced end. Leave the flag set and
    // start_player_turn's settle_time_warp_end_turn_if_ready runs a second
    // end_player_turn (FIDL01691: Head Slam then Reverberate).
    state.time_warp_end_turn = false;
    state.time_warp_end_turn_pre_discard_settled = false;
    // Leftover EndTurn never ran player atEndOfTurn Weak (that tick lives in
    // MonsterGroup.applyEndOfTurnPowers after takeTurn). First monster apply
    // sets justApplied and must not tick (FIDL01274 Ripple). Stacking onto
    // existing Weak does not set justApplied, so cleanup decrements once
    // (FIDL01782: Weak 1 + ATTACK_DEBUFF 2 → 2).
    run_monster_turn(state)?;
    // Frail is the same atEndOfRound sibling. Tick after leftover takeTurn so
    // Face Slap can stack onto the pre-tick amount (FIDL01807: 5 + 2 → 6).
    // A lethal monster action stops the target queue before
    // MonsterGroup.applyEndOfTurnPowers, so the debuff remains unchanged.
    if state.player.hp > 0 {
        tick_player_frail_at_end_of_round(state);
        if state.monsters.iter().any(|monster| monster.alive) {
            start_player_turn_with_start_relics_and_post_draw(state, true, apply_post_draw_relics)?;
        }
    }
    Ok(())
}

fn tick_player_weak_at_end_of_round(state: &mut CombatState) {
    if state.player.powers.weak > 0 && state.player.weak_just_applied {
        state.player.weak_just_applied = false;
    } else if state.player.powers.weak > 0 {
        state.player.powers.weak -= 1;
    } else {
        state.player.weak_just_applied = false;
    }
}

fn tick_player_frail_at_end_of_round(state: &mut CombatState) {
    if state.player.powers.frail > 0 && state.player.frail_just_applied {
        state.player.frail_just_applied = false;
    } else if state.player.powers.frail > 0 {
        state.player.powers.frail -= 1;
    } else {
        state.player.frail_just_applied = false;
    }
}

pub fn apply_pending_nilry_end_powers(state: &mut CombatState) -> SimResult<()> {
    if !state.nilrys_end_powers_pending {
        return Ok(());
    }
    let mut deferred_monster_deaths = Vec::new();
    crate::combat::turn_powers::apply_end_of_player_turn_powers_before_hand_deferred(
        state,
        &mut deferred_monster_deaths,
    )?;
    apply_end_of_turn_orb_passives(state)?;
    state.nilrys_end_powers_pending = false;
    Ok(())
}

fn apply_end_of_turn_orb_passives(state: &mut CombatState) -> SimResult<()> {
    crate::combat::transition::apply_orb_end_of_turn_passives(state)
}

fn queued_end_turn_autoplay_ids(
    state: &CombatState,
) -> std::collections::HashSet<crate::ids::CardId> {
    state
        .piles
        .hand
        .iter()
        .filter(|card| {
            matches!(
                card.content_id,
                BURN_ID | DECAY_ID | REGRET_ID | DOUBT_ID | SHAME_ID
            )
        })
        .map(|card| card.id)
        .collect()
}

fn hand_has_end_turn_autoplay_cards(state: &CombatState) -> bool {
    // callEndOfTurnActions queues Burn/Decay/Regret before AbstractRoom.endTurn
    // addToBot Combust LoseHP. A 1-card draw pile is emptied by the first
    // autoplay Cube, so Combust's later Cube shuffles the settled Burn
    // (FIDL01641). Combust still runs first when the hand has no HP-loss
    // autoplay (FIDL01335 Evolve).
    if !state
        .piles
        .hand
        .iter()
        .any(|card| matches!(card.content_id, BURN_ID | DECAY_ID | REGRET_ID))
    {
        return false;
    }
    // Dark Embrace + ethereal on top of draw: Combust's addToTop Cube pulls
    // that status before Burn shuffles (FIDL01665 leftover-empty discard).
    if state.player.powers.dark_embrace > 0 && draw_pile_top_is_ethereal(state) {
        return false;
    }
    true
}

fn draw_pile_top_is_ethereal(state: &CombatState) -> bool {
    state.piles.draw_pile.last().is_some_and(|card| {
        get_card_definition(card.content_id).is_some_and(|definition| definition.keywords.ethereal)
    })
}

pub fn start_player_turn(state: &mut CombatState) -> SimResult<()> {
    start_player_turn_with_start_relics(state, true)
}

/// First-turn settle after [`crate::relic::apply_start_of_combat_relics`].
///
/// That combat-entry hook already ran `atTurnStart` relics (and incremented
/// `player_turns_started`). Calling [`start_player_turn`] again would treat
/// Pocketwatch as if the previous turn played zero cards (FIDL01563 Colosseum
/// fight two).
pub fn start_player_turn_after_opening_combat_relics(state: &mut CombatState) -> SimResult<()> {
    start_player_turn_with_start_relics(state, false)?;
    state.pending_opening_hand_draw = 0;
    state.pending_opening_combat_block = 0;
    Ok(())
}

fn start_player_turn_with_start_relics(
    state: &mut CombatState,
    apply_start_relics: bool,
) -> SimResult<()> {
    start_player_turn_with_start_relics_and_post_draw(state, apply_start_relics, true)
}

fn start_player_turn_with_start_relics_and_post_draw(
    state: &mut CombatState,
    apply_start_relics: bool,
    apply_post_draw_relics: bool,
) -> SimResult<()> {
    let mut next = state.clone();
    start_player_turn_in_place(&mut next, apply_start_relics, apply_post_draw_relics, true)?;
    *state = next;
    Ok(())
}

fn start_player_turn_in_place(
    state: &mut CombatState,
    apply_start_relics: bool,
    apply_post_draw_relics: bool,
    draw_hand: bool,
) -> SimResult<()> {
    // EndTurnDeathPower.atStartOfTurn (Blasphemy): LoseHP 99999 then remove power.
    if state.player.powers.end_turn_death > 0 {
        state.player.hp = 0;
        state.player.block = 0;
        state.player.powers.end_turn_death = 0;
        state.player.powers.divinity = 0;
        state.phase = CombatPhase::Lost;
        return Ok(());
    }
    state.time_warp_end_powers_applied = false;
    for monster in &mut state.monsters {
        if monster.alive && monster.powers.invincible_max > 0 {
            monster.powers.invincible = monster.powers.invincible_max;
        }
    }
    crate::relic::reset_turn_relic_counters(state);
    state.player.powers.divinity = 0;
    reset_turn_only_temp_costs(state);
    let energy_next_turn = std::mem::take(&mut state.player.energy_next_turn);
    if crate::relic::preserves_energy_between_turns(&state.relics) {
        state.player.energy = checked_turn_add(state.player.energy, state.player.max_energy)?;
    } else {
        state.player.energy = state.player.max_energy;
    }
    state.player.energy = checked_turn_add(state.player.energy, energy_next_turn)?;
    state.player.cannot_draw = false;
    if state.preserve_temp_strength_on_next_start {
        state.preserve_temp_strength_on_next_start = false;
    } else {
        state.player.temp_strength = 0;
    }
    state.player.temp_thorns = 0;
    state.player.temp_rage_block = 0;
    state.player.powers.panache_cards_played = 0;
    state.double_tap_pending = 0;
    state.pen_nib_double_active = false;
    for monster in state
        .monsters
        .iter_mut()
        .filter(|monster| monster.content_id == GIANT_HEAD_ID)
    {
        monster.powers.slow = 0;
    }
    if state.player.temp_dexterity > 0 {
        state.player.powers.dexterity = state
            .player
            .powers
            .dexterity
            .checked_sub(state.player.temp_dexterity)
            .ok_or(SimError::InvalidState(
                "combat integer subtraction overflows i32",
            ))?;
        state.player.temp_dexterity = 0;
    }
    state.player.energy = checked_turn_add(state.player.energy, state.player.powers.berserk)?;
    if state.player.powers.fasting != 0 {
        state.player.energy =
            checked_turn_add(state.player.energy, -state.player.powers.fasting)?.max(0);
    }
    let deferred_start_relic_juggernaut = if apply_start_relics {
        crate::relic::apply_start_of_player_turn_relics(state)?
    } else {
        0
    };
    if !draw_hand {
        if deferred_start_relic_juggernaut > 0 {
            crate::combat::transition::apply_juggernaut_after_direct_block_gain(
                state,
                deferred_start_relic_juggernaut,
            )?;
        }
        state.phase = CombatPhase::WaitingForPlayer;
        return Ok(());
    }
    apply_start_of_turn_magnetism(state)?;
    apply_start_of_turn_creative_ai(state)?;
    // MayhemPower.atStartOfTurn queues one anonymous action per stack before
    // DrawCardAction. That wrapper rolls getRandomMonster (cardRandomRng) and
    // only then addToBot(PlayTopCardAction). Confusion on the hand draw must
    // see those rolls first (FIDL01474).
    let mayhem_targets = collect_start_of_turn_mayhem_targets(state);
    let base_draw_follow_ups = draw_next_hand_without_shuffle_deferred(state)?;
    // Start-of-turn relic GainBlockAction is addToBot before DrawCardAction.
    // Juggernaut onGainedBlock addToBots DamageRandomEnemy behind that draw.
    if deferred_start_relic_juggernaut > 0 {
        crate::combat::transition::apply_juggernaut_after_direct_block_gain(
            state,
            deferred_start_relic_juggernaut,
        )?;
    }
    if state.player.powers.draw_reduction > 0 {
        if state.player.powers.draw_reduction_first_draw_seen {
            // DrawReductionPower stacks are a duration: each active turn draws
            // one fewer card, while an additional Head Slam extends the power.
            state.player.powers.draw_reduction =
                state.player.powers.draw_reduction.saturating_sub(1);
            state.player.powers.draw_reduction_first_draw_seen = false;
        } else {
            state.player.powers.draw_reduction_first_draw_seen = true;
        }
    }
    // BrutalityPower.atStartOfTurnPostDraw queues LoseHP + DrawCard before
    // Mayhem's forced top-play must see the post-Brutality draw pile top
    // (FIDL00381: Mayhem plays Anger+ after Brutality draws Shrug+).
    // Leftover EndTurn STATE can publish mid-DrawCardAction before
    // atTurnStartPostDraw relics (FIDL01807 Warped Tongs still queued).
    if apply_post_draw_relics {
        crate::relic::apply_start_of_player_turn_post_draw_relics(state)?;
    }
    apply_demon_form_strength_post_draw(state)?;
    let brutality_draw_follow_ups = apply_start_of_turn_brutality_post_draw(state)?;
    if state.player.hp > 0 {
        crate::combat::transition::resolve_deferred_draw_follow_ups(
            state,
            brutality_draw_follow_ups,
        )?;
    }
    // Mayhem is queued ahead of Evolve's residual DrawCardAction from the base
    // hand refill. If Mayhem is the twelfth card, Time Warp appends EndTurnAction
    // behind that pending draw; defer the forced turn until FIFO evolve draws settle.
    // PlayTop card use() still addToBot(MakeTempCardInDrawPile) behind that
    // Evolve draw, so Wild Strike's Wound insert sees the post-Evolve pile
    // (FIDL01469).
    state.defer_time_warp_end_turn = true;
    state.defer_mayhem_play_top_draw_inserts = true;
    // UseCardAction settlement waits behind Evolve residuals only when the
    // base refill actually queued those draws (FIDL02303). Unconditional
    // parking put played Powers into discard (FIDL02199 Fire Breathing+).
    state.defer_mayhem_play_top_settlement = !base_draw_follow_ups.is_empty();
    let pending_mayhem_play_tops = apply_start_of_turn_mayhem(state, &mayhem_targets)?;
    if state.player.hp <= 0 {
        state.player.hp = 0;
        state.phase = CombatPhase::Lost;
        state.clear_decisions_on_combat_end();
        state.defer_time_warp_end_turn = false;
        state.defer_mayhem_play_top_draw_inserts = false;
        state.defer_mayhem_play_top_settlement = false;
        state.deferred_mayhem_play_top_draw_inserts.clear();
        state.deferred_mayhem_play_top_settlements.clear();
        return Ok(());
    }
    if state.player.hp > 0 {
        crate::combat::transition::resolve_deferred_draw_follow_ups(state, base_draw_follow_ups)?;
    }
    // Start-of-turn Fire Breathing pulses are consecutive addToBot DamageAll
    // actions. Guardian ChangeState/GainBlock is queued behind that burst.
    crate::content::monsters::resolve_deferred_guardian_mode_shifts(&mut state.monsters);
    // PlayTopCardAction only queued the cards. Fire Breathing (and Evolve
    // residuals) sit on the action queue, so they resolve before cardQueue
    // calls use(). A dead Mayhem target then skips that use() (FIDL02199).
    if !pending_mayhem_play_tops.is_empty() && state.player.hp > 0 {
        let transition =
            crate::combat::transition::process_internal_queue(state, pending_mayhem_play_tops)?;
        *state = transition.state;
    }
    crate::combat::transition::flush_deferred_mayhem_play_top_draw_inserts(state)?;
    state.defer_time_warp_end_turn = false;
    crate::combat::transition::settle_time_warp_end_turn_if_ready(state)?;
    if state
        .monsters
        .iter()
        .all(|monster| !monster.alive && !awakened_one_is_half_dead(monster))
    {
        let was_already_won = state.phase == CombatPhase::Won;
        state.phase = CombatPhase::Won;
        // Mayhem / similar start-of-turn plays can open a hand select after the
        // fight is already over (FIDL00243 lethal prior turn + END refill).
        state.clear_decisions_on_combat_end();
        if !was_already_won {
            crate::combat::apply_burning_blood(state)?;
        }
        return Ok(());
    }
    state.phase = CombatPhase::WaitingForPlayer;
    Ok(())
}

fn checked_turn_add(value: i32, amount: i32) -> SimResult<i32> {
    value.checked_add(amount).ok_or(SimError::InvalidState(
        "combat integer addition overflows i32",
    ))
}

fn checked_turn_mul(value: i32, amount: i32) -> SimResult<i32> {
    value.checked_mul(amount).ok_or(SimError::InvalidState(
        "combat integer multiplication overflows i32",
    ))
}

fn checked_turn_increment(value: &mut u32) -> SimResult<()> {
    *value = value
        .checked_add(1)
        .ok_or(SimError::InvalidState("combat turn counter overflows u32"))?;
    Ok(())
}

fn resolve_player_temp_strength(state: &mut CombatState) -> SimResult<()> {
    let amount = std::mem::take(&mut state.player.temp_strength);
    if amount <= 0 || state.player.powers.artifact <= 0 {
        return Ok(());
    }

    // Flex's LoseStrengthPower applies negative Strength at end of turn. Artifact
    // can therefore block it even when Artifact was gained after Flex resolved.
    state.player.powers.artifact -= 1;
    state.player.powers.strength = checked_turn_add(state.player.powers.strength, amount)?;
    Ok(())
}

pub fn finish_monster_turn_after_player_revival(state: &mut CombatState) -> SimResult<()> {
    let mut next = state.clone();
    finish_monster_turn_after_player_revival_inner(&mut next)?;
    *state = next;
    Ok(())
}

fn finish_monster_turn_after_player_revival_inner(state: &mut CombatState) -> SimResult<()> {
    for monster in &mut state.monsters {
        if monster.alive {
            if monster.powers.vulnerable > 0 {
                monster.powers.vulnerable -= 1;
            }
            monster.vulnerable_just_applied = false;
            if monster.powers.weak > 0 {
                monster.powers.weak -= 1;
            }
            if monster.powers.malleable_base > 0 {
                monster.powers.malleable = monster.powers.malleable_base;
            }
            apply_end_of_monster_turn_powers(monster)?;
            if monster.content_id == BYRD_ID && monster.powers.flight > 0 {
                monster.powers.flight = target_byrd_flight_amount(state.ascension);
            }
            if monster.content_id == GIANT_HEAD_ID {
                monster.powers.slow = 0;
            }
            if monster.temp_strength_down > 0 {
                monster.powers.strength =
                    checked_turn_add(monster.powers.strength, monster.temp_strength_down)?;
                monster.temp_strength_down = 0;
            }
        }
    }

    if state.player.powers.vulnerable > 0 && state.player.vulnerable_just_applied {
        state.player.vulnerable_just_applied = false;
    } else if state.player.powers.vulnerable > 0 {
        state.player.powers.vulnerable -= 1;
    } else {
        state.player.vulnerable_just_applied = false;
    }
    tick_player_weak_at_end_of_round(state);
    tick_player_frail_at_end_of_round(state);
    if state.player.powers.intangible > 0 {
        state.player.powers.intangible -= 1;
    }

    apply_turn_transition_block_loss(state);
    Ok(())
}

fn apply_start_of_turn_brutality_post_draw(
    state: &mut CombatState,
) -> SimResult<Vec<crate::action::InternalAction>> {
    let amount = state.player.powers.brutality.max(0) as usize;
    if amount == 0 {
        return Ok(Vec::new());
    }
    let follow_ups = if state.player.cannot_draw {
        Vec::new()
    } else {
        crate::combat::transition::player_draw_cards_with_deferred_evolve(state, amount)?
    };
    let hp_loss = crate::combat::hp_loss::lose_player_hp(state, amount as i32);
    crate::combat::hp_loss::apply_player_card_hp_loss_hooks(state, hp_loss)?;
    revive_player_if_available(state)?;
    Ok(follow_ups)
}

/// Leftover `HandCardSelectScreen.selectedCards` re-enter discard only through
/// `DiscardAction`. Runic Pyramid skips that action, so a skipped-retrieval
/// residual stays on the singleton screen (FIDL01566).
fn should_settle_pending_hidden_into_discard(
    state: &CombatState,
    hand_nonempty_at_end_click: bool,
    auto_play_emptied_hand: bool,
) -> bool {
    hand_nonempty_at_end_click
        && !auto_play_emptied_hand
        && !state.relics.contains(&crate::Relic::RunicPyramid)
}

fn apply_start_of_turn_magnetism(state: &mut CombatState) -> SimResult<()> {
    if state
        .monsters
        .iter()
        .all(|monster| !monster.alive && !awakened_one_is_half_dead(monster))
    {
        return Ok(());
    }

    let count = state.player.powers.magnetism.max(0) as usize;
    if count == 0 {
        return Ok(());
    }
    let first_id = state.reserve_card_instance_ids(count)?;
    for offset in 0..count {
        let content_id = crate::combat::card_effects::magnetism_generated_colorless_card(state);
        let next_id = crate::CardId::new(first_id + offset as u64);
        let mut generated = crate::CardInstance {
            combat_only: true,
            ..crate::CardInstance::new(next_id, content_id)
        };
        if state.piles.hand.len() >= 10 {
            state.piles.discard_pile.push(generated);
        } else {
            crate::combat::transition::apply_corruption_cost_to_generated_hand_card(
                state,
                &mut generated,
            );
            state.piles.hand.push(generated);
        }
    }
    Ok(())
}

fn apply_start_of_turn_creative_ai(state: &mut CombatState) -> SimResult<()> {
    if state
        .monsters
        .iter()
        .all(|monster| !monster.alive && !awakened_one_is_half_dead(monster))
    {
        return Ok(());
    }

    let count = state.player.powers.creative_ai.max(0) as usize;
    if count == 0 {
        return Ok(());
    }
    let pool = crate::content::shop_pool::ironclad_combat_power_discovery_pool();
    let first_id = state.reserve_card_instance_ids(count)?;
    for offset in 0..count {
        let idx = state
            .rng
            .card_random_rng
            .random_int((pool.len() - 1) as i32) as usize;
        let content_id = pool[idx];
        let next_id = crate::CardId::new(first_id + offset as u64);
        let generated = crate::CardInstance {
            combat_only: true,
            ..crate::CardInstance::new(next_id, content_id)
        };
        if state.piles.hand.len() >= 10 {
            state.piles.discard_pile.push(generated);
        } else {
            state.piles.hand.push(generated);
        }
    }
    Ok(())
}

fn collect_start_of_turn_mayhem_targets(state: &mut CombatState) -> Vec<Option<MonsterId>> {
    (0..state.player.powers.mayhem.max(0))
        .map(|_| mayhem_random_living_target(state))
        .collect()
}

fn apply_start_of_turn_mayhem(
    state: &mut CombatState,
    targets: &[Option<MonsterId>],
) -> SimResult<VecDeque<crate::InternalAction>> {
    // MayhemPower.atStartOfTurn addToBots one PlayTopCardAction per stack
    // before any of them resolve. InkBottle.onUseCard addToBots Draw after
    // each played card, so a 10th-card Ink draw from the first PlayTop sits
    // behind the remaining PlayTops (FIDL02199 Intimidate then Wound).
    // A single stack keeps the sequential path so unplayable tops still
    // discard without Blue Candle / UseCardAction (FIDL02199 Havoc turn).
    // Stacked PlayTops only pop into cardQueue; GameActionManager drains the
    // action queue (Evolve residuals, Fire Breathing) before use().
    if targets.len() > 1 {
        return crate::combat::transition::pop_mayhem_play_top_cards(state, targets);
    }
    let mut remaining = targets;
    while let Some((&random_target, rest)) = remaining.split_first() {
        // PlayTop still executes after the base hand draw and Brutality's extra
        // draw (FIDL00381). Only the getRandomMonster roll is early.
        if state.piles.draw_pile.is_empty() && !state.piles.discard_pile.is_empty() {
            // PlayTopCardAction queues EmptyDeckShuffleAction when its draw
            // pile is empty, then plays the newly exposed top card.
            crate::combat::transition::player_shuffle_discard_into_draw(state)?;
        }
        let Some(top_card) = state.piles.draw_pile.last() else {
            return Ok(VecDeque::new());
        };
        let definition = crate::content::cards::get_card_definition(top_card.content_id)
            .ok_or(crate::SimError::UnknownContent(top_card.content_id))?;
        if definition.keywords.unplayable {
            // Target PlayTopCardAction removes the top card into limbo before
            // autoplay checks whether it can be used. If autoplay cannot play
            // an unplayable curse/status, the card still leaves the draw pile
            // and resolves to discard.
            if let Some(card) = state.piles.draw_pile.pop() {
                state.piles.discard_pile.push(card);
            }
            remaining = rest;
            continue;
        }
        if matches!(
            definition.id,
            crate::content::cards::DEEP_BREATH_ID | crate::content::cards::DEEP_BREATH_PLUS_ID
        ) && !rest.is_empty()
        {
            // MayhemPower queues every PlayTopCardAction before DeepBreath.use
            // addToBots ShuffleAction, so later PlayTops take the pre-shuffle
            // tops (FIDL01709 Dramatic Entrance under Deep Breath).
            crate::combat::transition::apply_mayhem_play_top_cards(state, remaining)?;
            return Ok(VecDeque::new());
        }
        let target = if definition.target == crate::TargetRequirement::Enemy {
            random_target
        } else {
            None
        };
        crate::combat::transition::apply_play_top_draw_card_to_state(state, target)?;
        if state.player.hp <= 0
            || state
                .monsters
                .iter()
                .all(|monster| !monster.alive && !awakened_one_is_half_dead(monster))
        {
            return Ok(VecDeque::new());
        }
        remaining = rest;
    }
    Ok(VecDeque::new())
}

fn mayhem_random_living_target(state: &mut CombatState) -> Option<MonsterId> {
    let living = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster.id)
        .collect::<Vec<_>>();
    if living.is_empty() {
        return None;
    }
    let index = state
        .rng
        .card_random_rng
        .random_int((living.len() - 1) as i32) as usize;
    living.get(index).copied()
}

fn finish_combat_if_over(
    state: &mut CombatState,
    started_with_living_monster: bool,
) -> SimResult<bool> {
    if state.player.hp <= 0 {
        state.player.hp = 0;
        state.player.block = 0;
        state.phase = CombatPhase::Lost;
        state.clear_decisions_on_combat_end();
        return Ok(true);
    }

    if started_with_living_monster
        && state
            .monsters
            .iter()
            .all(|monster| !monster.alive && !awakened_one_is_half_dead(monster))
    {
        state.phase = CombatPhase::Won;
        state.clear_decisions_on_combat_end();
        crate::combat::apply_burning_blood(state)?;
        return Ok(true);
    }

    Ok(false)
}

fn reset_turn_only_temp_costs(state: &mut CombatState) {
    for pile in [
        &mut state.piles.hand,
        &mut state.piles.draw_pile,
        &mut state.piles.discard_pile,
        &mut state.piles.exhaust_pile,
    ] {
        for card in pile {
            if card.temp_cost_turn_only {
                card.temp_cost = None;
                card.temp_cost_turn_only = false;
            }
        }
    }
}

/// Advance the first queued monster action without running end-of-monster-turn
/// cleanup or starting a new player turn. SuperFastMode can accept a duplicate
/// obtain END while that MonsterQueueItem is already settling (FIDL01595).
pub fn run_first_monster_action_without_cleanup(state: &mut CombatState) -> SimResult<()> {
    let Some(actor_id) = state
        .monsters
        .iter()
        .find(|monster| monster.alive)
        .map(|monster| monster.id)
    else {
        return Ok(());
    };
    let Some(index) = state
        .monsters
        .iter()
        .position(|monster| monster.id == actor_id)
    else {
        return Ok(());
    };
    let ascension = state.ascension;
    let relics = state.relics.clone();
    let mut skip_ritual_tick = Vec::new();
    clear_lagavulin_metallicize_if_awake(&mut state.monsters[index]);
    if execute_state_oriented_special_intent(state, actor_id, index, ascension)? {
        return Ok(());
    }
    if execute_spawning_or_targeted_special_intent(state, actor_id, index, ascension, false)? {
        return Ok(());
    }
    let _ = execute_generic_monster_intent(
        state,
        actor_id,
        index,
        ascension,
        &relics,
        &mut skip_ritual_tick,
    )?;
    Ok(())
}

fn run_monster_turn(state: &mut CombatState) -> SimResult<()> {
    let ascension = state.ascension;
    let relics = state.relics.clone();
    let mut skip_ritual_tick = Vec::new();
    let turn_order = state
        .monsters
        .iter()
        .map(|monster| monster.id)
        .collect::<Vec<_>>();
    // Time Warp's early-end path can leave two MonsterQueueItems ahead of their
    // RollMoveActions. Capture the second item exactly as the Java queue does:
    // it reads each monster's original intent before the first item's roll runs.
    let duplicate_monster_queue = state.time_warp_duplicate_monster_queue;
    let queued_intents = if duplicate_monster_queue {
        state
            .monsters
            .iter()
            .map(|monster| (monster.id, monster.intent))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    // Each MonsterQueueItem runs every living monster, then the next item
    // does the same with the captured intents (FIDL01597: Rally between the
    // two Mad Gremlin hits). Nested per-monster duplication would land both
    // hits before Encourage.
    let queue_count = if duplicate_monster_queue { 2 } else { 1 };
    for queued_turn in 0..queue_count {
        for actor_id in turn_order.iter().copied() {
            // The second MonsterQueueItem was already present before the first
            // item's RollMoveAction. Restore its captured intent while retaining
            // the first item's move-history/RNG effects.
            if queued_turn == 1 {
                if let Some((_, intent)) = queued_intents
                    .iter()
                    .find(|(queued_id, _)| *queued_id == actor_id)
                {
                    if let Some(monster) = state.monsters.iter_mut().find(|m| m.id == actor_id) {
                        monster.intent = *intent;
                    }
                }
            }
            // Life Link may permanently kill the pack mid-enemy-turn (e.g.
            // reactive thorns put the last living Darkling into half-dead).
            let _ = crate::combat::damage::resolve_darkling_life_link(&mut state.monsters);
            let Some(index) = state
                .monsters
                .iter()
                .position(|monster| monster.id == actor_id)
            else {
                continue;
            };
            if !state.monsters[index].alive
                && !is_half_dead_darkling(&state.monsters[index])
                && !awakened_one_is_half_dead(&state.monsters[index])
            {
                continue;
            }
            clear_lagavulin_metallicize_if_awake(&mut state.monsters[index]);
            // Awakened One's first death queues the next-turn REBIRTH with
            // move byte 3 and Intent.UNKNOWN. Preserve that source state even
            // if a prior queued action left the stale pre-death intent on the
            // half-dead monster.
            if awakened_one_is_half_dead(&state.monsters[index]) {
                state.monsters[index].intent = crate::MonsterIntent::AwakenedOneHalfDead;
            }
            if execute_state_oriented_special_intent(state, actor_id, index, ascension)? {
                continue;
            }
            if execute_spawning_or_targeted_special_intent(
                state, actor_id, index, ascension, false,
            )? {
                continue;
            }
            if matches!(
                execute_generic_monster_intent(
                    state,
                    actor_id,
                    index,
                    ascension,
                    &relics,
                    &mut skip_ritual_tick,
                )?,
                ActorTurnDisposition::StopPlayerDead
            ) {
                state.time_warp_duplicate_monster_queue = false;
                let _ = crate::combat::damage::resolve_darkling_life_link(&mut state.monsters);
                return Ok(());
            }
            let _ = crate::combat::damage::resolve_darkling_life_link(&mut state.monsters);
        }
    }
    state.time_warp_duplicate_monster_queue = false;

    finish_monster_turn_cleanup(state, &skip_ritual_tick)
}

enum ActorTurnDisposition {
    Continue,
    StopPlayerDead,
}

fn player_can_revive_after_monster_hit(state: &CombatState) -> bool {
    if state.mark_of_bloom {
        return false;
    }
    (state.relics.contains(&crate::Relic::LizardTail) && state.relic_counters.lizard_tail_available)
        || state.relic_counters.fairy_heal_percent > 0
}

fn execute_generic_monster_intent(
    state: &mut CombatState,
    actor_id: MonsterId,
    index: usize,
    ascension: u8,
    relics: &[crate::Relic],
    skip_ritual_tick: &mut Vec<MonsterId>,
) -> SimResult<ActorTurnDisposition> {
    let actor_was_alive = state.monsters[index].alive;
    let player_snapshot = state.player.clone();
    let intent = state.monsters[index].intent;
    let nemesis_had_intangible = state.monsters[index].content_id == NEMESIS_ID
        && state.monsters[index].powers.intangible > 0;
    let deferred_burn_to_discard = match intent {
        crate::MonsterIntent::AddBurnToDiscard { count, .. } => count,
        crate::MonsterIntent::AttackMultipleApplyPlayerWeak { .. }
            if state.monsters[index].content_id == SPIRE_SPEAR_ID =>
        {
            2
        }
        _ => 0,
    };
    let deferred_burn_is_upgraded = deferred_burn_to_discard > 0
        && state.monsters[index].content_id == HEXAGHOST_ID
        && state.monsters[index].burns_upgraded;
    let deferred_upgrade_burns = match intent {
        crate::MonsterIntent::AttackMultipleUpgradeBurns { count, .. } => count,
        _ => 0,
    };
    let deferred_wounds_to_discard = match intent {
        crate::MonsterIntent::AttackAddWoundsToDiscard { count, .. } => {
            if state.monsters[index].content_id == crate::content::monsters::TASKMASTER_ID {
                target_taskmaster_wound_count(ascension)
            } else {
                count
            }
        }
        _ => 0,
    };
    let piles_before_post_damage_effects =
        (deferred_burn_to_discard > 0 || deferred_upgrade_burns > 0).then(|| state.piles.clone());
    let allocated_card_id_through = state.max_authoritative_card_instance_id();
    let player_can_revive = player_can_revive_after_monster_hit(state);
    // Same-frame Time Warp queues DamageInfo before the +2 Strength action.
    // Lagged CONFIRM (FIDL01425) already published that Strength, so both
    // duplicate-queue attacks use live strength.
    let time_warp_queued_damage_snapshot = state.time_warp_duplicate_monster_queue
        && state.monsters[index].content_id == crate::content::monsters::TIME_EATER_ID
        && matches!(
            intent,
            crate::MonsterIntent::Attack { .. } | crate::MonsterIntent::AttackMultiple { .. }
        )
        && !state.time_warp_end_turn;
    // TimeWarpPower queues its +2 Strength action, while the monster queue's
    // DamageInfo objects were created from the pre-action intent. Preserve that
    // source FIFO for the duplicated queue; the monster's +2 remains in state
    // for subsequent rolls and observations.
    let strength_before_time_warp_snapshot = state.monsters[index].powers.strength;
    if time_warp_queued_damage_snapshot {
        state.monsters[index].powers.strength =
            strength_before_time_warp_snapshot.saturating_sub(2);
    }
    let damage_result = prepare_monster_intent_with_card_rng_and_revival(
        &mut state.monsters[index],
        &mut state.player,
        &mut state.piles,
        allocated_card_id_through,
        ascension,
        &player_snapshot,
        relics,
        player_can_revive,
        &mut state.rng.card_random_rng,
    );
    if time_warp_queued_damage_snapshot {
        state.monsters[index].powers.strength = strength_before_time_warp_snapshot;
    }
    let damage = damage_result?;
    if state.monsters[index].content_id == WRITHING_MASS_ID
        && matches!(intent, crate::MonsterIntent::ApplyPlayerFrailAndWeak { .. })
    {
        state.writhing_mass_mega_debuff_triggered = true;
    }
    if state.monsters[index].content_id == crate::content::monsters::TIME_EATER_ID
        && matches!(intent, crate::MonsterIntent::Attack { damage: 26 | 32 })
        && !state.time_warp_duplicate_monster_queue
    {
        // Head Slam addToBot ApplyPowerAction(DrawReductionPower). Artifact
        // consumes that DEBUFF (FIDL01762 step 1846) instead of shrinking
        // gameHandSize for the following start-of-turn draw. A leftover Time
        // Warp END still runs that same action (FIDL01566); skip only the
        // duplicate-queue replay that would stack a second Head Slam.
        crate::power::apply_player_draw_reduction(&mut state.player.powers, 1)?;
    }
    let died_during_intent =
        actor_was_alive && !state.monsters[index].alive && !state.monsters[index].escaped;
    if died_during_intent {
        crate::combat::transition::apply_monster_death_hooks(state, actor_id)?;
    }
    // Reactive thorns damage is applied inside the monster intent resolver.
    // Slime Boss checks its split threshold after that queued damage resolves.
    check_slime_boss_split(state, actor_id);
    if let Some(piles) = piles_before_post_damage_effects {
        // CommunicationMod observes Hexaghost/Nemesis status cards only
        // after attack damage resolves. In particular, a lethal Inferno
        // does not upgrade existing Burns or add its three new Burns.
        state.piles = piles;
    }
    let hits = effective_current_move_hits(intent, state.monsters[index].intent);
    if matches!(intent, crate::MonsterIntent::Ritual { .. }) {
        skip_ritual_tick.push(actor_id);
    }
    let heal_self =
        matches!(intent, crate::MonsterIntent::AttackHealSelf { .. }).then_some(actor_id);
    let burn_to_discard_and_draw = match intent {
        crate::MonsterIntent::AddBurnToDiscardAndDraw { count, .. } => count,
        _ => 0,
    };
    let dazed_to_discard = match intent {
        crate::MonsterIntent::AttackMultipleAddDazedToDiscard { count, .. } => count,
        _ => 0,
    };
    let weak = match intent {
        crate::MonsterIntent::AttackMultipleApplyPlayerWeak { weak, .. } => weak,
        _ => 0,
    };
    if damage > 0
        || has_queued_post_attack_player_debuffs(intent)
        || burn_to_discard_and_draw > 0
        || weak > 0
        || deferred_burn_to_discard > 0
        || deferred_upgrade_burns > 0
    {
        let heal_self_thorns = if heal_self.is_some() {
            checked_turn_mul(
                checked_turn_add(state.player.powers.thorns, state.player.temp_thorns)?,
                hits.max(1),
            )?
        } else {
            0
        };
        let plated_armor_before_thorns_damage = state.player.powers.plated_armor;
        apply_monster_pending_effects(
            state,
            intent,
            damage,
            hits,
            state.monsters[index].powers.painful_stabs,
            heal_self,
            heal_self_thorns,
            burn_to_discard_and_draw,
            weak,
            deferred_burn_to_discard,
            deferred_burn_is_upgraded,
            deferred_upgrade_burns,
            Some(index),
        )?;
        if matches!(intent, crate::MonsterIntent::Stun) {
            state.player.powers.plated_armor = plated_armor_before_thorns_damage;
        }
        if state.monsters[index].content_id == SPIRE_SHIELD_ID
            && matches!(
                intent,
                crate::MonsterIntent::AttackApplyPlayerWeak { weak: 0, .. }
            )
        {
            // SpireShield.takeTurn smash: Damage, then either Focus -1 or
            // Strength -1. If `player.orbs` is non-empty (EmptyOrbSlot
            // placeholders count) and aiRng.randomBoolean() is true, apply
            // Focus and skip Strength. Otherwise ApplyPower Strength -1.
            if state.max_orbs > 0 && state.rng.monster_rng.random_bool() {
                crate::power::reduce_player_focus(&mut state.player.powers, 1)?;
            } else {
                crate::power::reduce_player_strength(&mut state.player.powers, 1)?;
            }
        }
    }
    if state.monsters[index].content_id == CORRUPT_HEART_ID
        && matches!(
            intent,
            crate::MonsterIntent::ApplyPlayerFrailWeakVulnerable { .. }
        )
    {
        for content_id in [DAZED_ID, SLIMED_ID, WOUND_ID, BURN_ID, VOID_ID] {
            let allocated = state.max_authoritative_card_instance_id();
            add_cards_to_draw_random_spot(
                &mut state.piles,
                content_id,
                1,
                &mut state.rng.card_random_rng,
                allocated,
            )?;
        }
    }
    if state.monsters[index].content_id == SPIRE_SHIELD_ID
        && matches!(intent, crate::MonsterIntent::Block { block: 30 })
    {
        for (other_index, monster) in state.monsters.iter_mut().enumerate() {
            if other_index != index && monster.alive {
                monster.block = monster.block.checked_add(30).ok_or(SimError::InvalidState(
                    "Spire Shield Fortify block overflows i32",
                ))?;
            }
        }
    }
    // SnakeDagger EXPLODE queues DamageAction then LoseHPAction(self).
    // DeathScreen from a lethal hit freezes that later suicide
    // (FIDL01796: dagger stays at 22 when its 25 kills the player).
    if state.player.hp > 0
        && state.monsters[index].content_id == DAGGER_ID
        && matches!(
            intent,
            crate::MonsterIntent::Attack {
                damage: DAGGER_EXPLODE_DAMAGE
            }
        )
        && state.monsters[index].alive
    {
        state.monsters[index].hp = 0;
        state.monsters[index].alive = false;
        state.monsters[index].block = 0;
        crate::combat::transition::apply_monster_death_hooks(state, actor_id)?;
    }
    if let crate::MonsterIntent::AttackAddVoidToDraw { count, .. } = intent {
        // DamageAction is already on the queue ahead of
        // MakeTempCardInDrawPileAction. A lethal hit clears that later action.
        if state.player.hp > 0 {
            let allocated_card_id_through = state.max_authoritative_card_instance_id();
            add_cards_to_draw_random_spot(
                &mut state.piles,
                crate::content::cards::VOID_ID,
                count,
                &mut state.rng.card_random_rng,
                allocated_card_id_through,
            )?;
        }
    }
    if state.player.hp > 0
        && deferred_upgrade_burns > 0
        && state.monsters[index].content_id == HEXAGHOST_ID
    {
        state.monsters[index].burns_upgraded = true;
    }
    if state.player.hp > 0 {
        if let crate::MonsterIntent::AttackAddSlimedToDiscard { count, .. } = intent {
            let allocated_card_id_through = state.max_authoritative_card_instance_id();
            add_cards_to_discard(
                &mut state.piles,
                SLIMED_ID,
                count,
                allocated_card_id_through,
            )?;
        }
        if deferred_wounds_to_discard > 0 {
            let allocated_card_id_through = state.max_authoritative_card_instance_id();
            add_cards_to_discard(
                &mut state.piles,
                WOUND_ID,
                deferred_wounds_to_discard,
                allocated_card_id_through,
            )?;
        }
    }
    if state.player.hp > 0 && dazed_to_discard > 0 {
        let allocated_card_id_through = state.max_authoritative_card_instance_id();
        add_cards_to_discard(
            &mut state.piles,
            DAZED_ID,
            dazed_to_discard,
            allocated_card_id_through,
        )?;
    }
    if state.monsters[index].alive && state.monsters[index].content_id == NEMESIS_ID {
        if nemesis_had_intangible {
            state.monsters[index].powers.intangible =
                state.monsters[index].powers.intangible.saturating_sub(1);
        } else if state.monsters[index].powers.intangible == 0 {
            state.monsters[index].powers.intangible = 1;
        }
    }
    if state.monsters[index].alive {
        if state.monsters[index].content_id == LAGAVULIN_ID
            && matches!(intent, crate::MonsterIntent::Sleep)
            && state.monsters[index].sleep_turns_remaining == 0
        {
            state.monsters[index].intent = target_lagavulin_direct_wake_attack_intent(ascension);
            record_target_move(&mut state.monsters[index]);
            return Ok(ActorTurnDisposition::Continue);
        }
        if state.monsters[index].content_id == SHELLED_PARASITE_ID
            && matches!(intent, crate::MonsterIntent::Stun)
        {
            state.monsters[index].intent = target_shelled_parasite_shell_break_roll_move(ascension);
            record_target_move(&mut state.monsters[index]);
        }
    }
    // Every modeled target monster queues RollMoveAction at the end of its
    // takeTurn. That action still runs when reactive thorns kill the monster
    // during its own attack, as long as the combat continues for another
    // living monster. Preserve that queued AI draw even though the attacker
    // is no longer alive by the time its damage resolves.
    //
    // Fire Breathing from Runic Cube / deferred multi-hit draws runs in
    // apply_monster_pending_effects, after the early death snapshot.
    // Darkling.damage still queues SetMove(COUNT) behind that RollMoveAction
    // (FIDL01313: CHOMP + Cube Wound + FB leaves UNKNOWN/4, not STUN/5).
    let darkling_died_during_intent =
        actor_was_alive && is_half_dead_darkling(&state.monsters[index]);
    let should_roll_queued_next_intent = actor_was_alive
        && state.player.hp > 0
        && (state.monsters[index].alive
            || state
                .monsters
                .iter()
                .any(|monster| monster.id != actor_id && monster.alive));
    if should_roll_queued_next_intent {
        prepare_next_intent_for_actor(state, actor_id)?;
        if darkling_died_during_intent {
            // Darkling.damage queued SetMove(COUNT) after the current attack,
            // while takeTurn had already queued RollMoveAction. Restore that
            // later COUNT action after consuming and recording the roll.
            state.monsters[index].intent = crate::MonsterIntent::DarklingCount;
            record_target_move(&mut state.monsters[index]);
        }
        if awakened_one_is_half_dead(&state.monsters[index]) {
            // AwakenedOne.damage setMove(3, UNKNOWN) after first-form death,
            // behind the takeTurn RollMoveAction.
            state.monsters[index].intent = crate::MonsterIntent::AwakenedOneHalfDead;
            record_target_move(&mut state.monsters[index]);
        }
        if state.monsters[index].alive {
            apply_transient_fading_after_turn(&mut state.monsters, actor_id);
        }
    }
    revive_with_lizard_tail_if_available(state)?;
    if state.player.hp <= 0 {
        Ok(ActorTurnDisposition::StopPlayerDead)
    } else {
        Ok(ActorTurnDisposition::Continue)
    }
}

fn execute_state_oriented_special_intent(
    state: &mut CombatState,
    actor_id: MonsterId,
    index: usize,
    ascension: u8,
) -> SimResult<bool> {
    match state.monsters[index].intent {
        crate::MonsterIntent::StrengthSelf { amount: 0 }
            if state.monsters[index].content_id == crate::content::monsters::TIME_EATER_ID =>
        {
            // Time Eater's Haste clears its debuffs and heals to half of its
            // stored maximum HP. At A19 it also gains the source's block.
            state.monsters[index].powers.vulnerable = 0;
            state.monsters[index].powers.weak = 0;
            state.monsters[index].powers.strength = state.monsters[index].powers.strength.max(0);
            state.monsters[index].temp_strength_down = 0;
            let target_hp = state.monsters[index].max_hp / 2;
            if state.monsters[index].hp < target_hp {
                state.monsters[index].hp = target_hp;
            }
            if ascension >= 19 {
                state.monsters[index].block = checked_turn_add(state.monsters[index].block, 32)?;
            }
            checked_turn_increment(&mut state.monsters[index].moves_executed)?;
            prepare_next_intent_for_actor(state, actor_id)?;
            Ok(true)
        }
        crate::MonsterIntent::AttackAndBlock {
            damage: 0,
            block: 20,
        } if state.monsters[index].content_id == crate::content::monsters::TIME_EATER_ID => {
            state.monsters[index].block =
                checked_turn_add(state.monsters[index].block, 20)?.min(999);
            // TimeEater.takeTurn RIPPLE queues Vulnerable, then Weak, then
            // A19 Frail. Artifact consumes the first DEBUFF (FIDL01594:
            // Panacea blocks Vulnerable; Weak remains).
            let had_no_vulnerable = state.player.powers.vulnerable == 0;
            let applied = crate::power::apply_player_vulnerable(&mut state.player.powers, 1)?;
            if had_no_vulnerable && applied {
                state.player.vulnerable_just_applied = true;
            }
            let had_no_weak = state.player.powers.weak == 0;
            crate::relic::apply_player_weak_with_relics(
                &mut state.player.powers,
                &state.relics,
                1,
            )?;
            if had_no_weak && state.player.powers.weak > 0 {
                state.player.weak_just_applied = true;
            }
            if ascension >= 19 {
                let had_no_frail = state.player.powers.frail == 0;
                crate::relic::apply_player_frail_with_relics(
                    &mut state.player.powers,
                    &state.relics,
                    1,
                )?;
                if had_no_frail && state.player.powers.frail > 0 {
                    state.player.frail_just_applied = true;
                }
            }
            checked_turn_increment(&mut state.monsters[index].moves_executed)?;
            prepare_next_intent_for_actor(state, actor_id)?;
            Ok(true)
        }
        crate::MonsterIntent::AwakenedOneHalfDead => {
            // Combust (and similar end-of-turn power) first-kills defer the
            // actual REBIRTH heal to the *next* monster phase so one full player
            // turn observes the source's half-dead UNKNOWN pose (FIDL00391).
            // Clear the deferral and keep that pose without burning the
            // post-rebirth AI roll yet.
            if state.monsters[index].defer_awakened_one_rebirth {
                // Hold the half-dead UNKNOWN pose through this enemy phase;
                // REBIRTH heal runs next monster phase (FIDL00391 first-kill).
                state.monsters[index].defer_awakened_one_rebirth = false;
                state.monsters[index].intent = crate::MonsterIntent::AwakenedOneHalfDead;
                return Ok(true);
            }
            // Rebirth queues the inherited RollMoveAction before the fixed
            // phase-two Dark Echo intent (permanent FIDL00378 / FIDL00269).
            let _ = state.rng.monster_rng.random_int(99);
            awaken_one_after_first_death(&mut state.monsters[index]);
            Ok(true)
        }
        crate::MonsterIntent::DarklingCount if is_half_dead_darkling(&state.monsters[index]) => {
            checked_turn_increment(&mut state.monsters[index].moves_executed)?;
            let _ = state.rng.monster_rng.random_int(99);
            state.monsters[index].intent = crate::MonsterIntent::Stun;
            record_target_move(&mut state.monsters[index]);
            state.monsters[index].intent = crate::MonsterIntent::StrengthSelf { amount: 0 };
            Ok(true)
        }
        crate::MonsterIntent::StrengthSelf { amount: 0 }
            if is_half_dead_darkling(&state.monsters[index]) =>
        {
            state.monsters[index].alive = true;
            state.monsters[index].escaped = false;
            state.monsters[index].hp = state.monsters[index].max_hp / 2;
            if state.relics.contains(&crate::Relic::PhilosophersStone) {
                state.monsters[index].powers.strength = checked_turn_add(
                    state.monsters[index].powers.strength,
                    crate::relic::PHILOSOPHERS_STONE_MONSTER_STRENGTH,
                )?;
            }
            checked_turn_increment(&mut state.monsters[index].moves_executed)?;
            prepare_next_intent_for_actor(state, actor_id)?;
            Ok(true)
        }
        crate::MonsterIntent::Stun if is_half_dead_darkling(&state.monsters[index]) => {
            // The first Regrow turn only advances the Darkling's hidden move;
            // reincarnation happens on the following monster turn.  The
            // target still consumes AbstractMonster's common AI roll before
            // the Regrow move resolves.
            checked_turn_increment(&mut state.monsters[index].moves_executed)?;
            let _ = state.rng.monster_rng.random_int(99);
            record_target_move(&mut state.monsters[index]);
            state.monsters[index].intent = crate::MonsterIntent::StrengthSelf { amount: 0 };
            Ok(true)
        }
        crate::MonsterIntent::HealAllMonsters { amount } => {
            apply_heal_all_monsters(&mut state.monsters, amount)?;
            checked_turn_increment(&mut state.monsters[index].moves_executed)?;
            prepare_next_intent_for_actor(state, actor_id)?;
            Ok(true)
        }
        crate::MonsterIntent::StrengthAllMonsters { amount } => {
            apply_strength_all_monsters(&mut state.monsters, amount)?;
            checked_turn_increment(&mut state.monsters[index].moves_executed)?;
            prepare_next_intent_for_actor(state, actor_id)?;
            Ok(true)
        }
        crate::MonsterIntent::StrengthSelf { amount }
            if state.monsters[index].content_id == CHAMP_ID
                && amount >= champ_strength_amount(ascension) * 3 =>
        {
            state.monsters[index].powers.vulnerable = 0;
            state.monsters[index].powers.weak = 0;
            state.monsters[index].temp_strength_down = 0;
            state.monsters[index].powers.strength = state.monsters[index].powers.strength.max(0);
            state.monsters[index].powers.strength =
                checked_turn_add(state.monsters[index].powers.strength, amount)?;
            checked_turn_increment(&mut state.monsters[index].moves_executed)?;
            prepare_next_intent_for_actor(state, actor_id)?;
            Ok(true)
        }
        crate::MonsterIntent::StrengthAndBlock { strength, block }
            if state.monsters[index].content_id == THE_COLLECTOR_ID =>
        {
            apply_strength_all_monsters(&mut state.monsters, strength)?;
            if let Some(monster) = state
                .monsters
                .iter_mut()
                .find(|monster| monster.id == actor_id)
            {
                monster.block = checked_turn_add(monster.block, block)?;
                checked_turn_increment(&mut monster.moves_executed)?;
            }
            prepare_next_intent_for_actor(state, actor_id)?;
            Ok(true)
        }
        crate::MonsterIntent::StrengthAndBlock { strength, block }
            if state.monsters[index].content_id == CHAMP_ID =>
        {
            state.monsters[index].block = checked_turn_add(state.monsters[index].block, block)?;
            state.monsters[index].powers.metallicize =
                checked_turn_add(state.monsters[index].powers.metallicize, strength)?;
            checked_turn_increment(&mut state.monsters[index].moves_executed)?;
            prepare_next_intent_for_actor(state, actor_id)?;
            Ok(true)
        }
        crate::MonsterIntent::EncourageGremlins { strength, block } => {
            let leader_id = state.monsters[index].id;
            if state.monsters[index].content_id == GREMLIN_LEADER_ID {
                let _ = state.rng.monster_rng.random_int(2);
            }
            apply_gremlin_leader_encourage(&mut state.monsters, leader_id, strength, block)?;
            checked_turn_increment(&mut state.monsters[index].moves_executed)?;
            prepare_next_intent_for_actor(state, actor_id)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn execute_spawning_or_targeted_special_intent(
    state: &mut CombatState,
    actor_id: MonsterId,
    index: usize,
    ascension: u8,
    nilrys_second_queue_item: bool,
) -> SimResult<bool> {
    match state.monsters[index].intent {
        crate::MonsterIntent::Attack { damage }
            if state.monsters[index].content_id == BYRD_ID && damage == 3 =>
        {
            let player_snapshot = state.player.clone();
            let allocated_card_id_through = state.max_authoritative_card_instance_id();
            let player_can_revive = player_can_revive_after_monster_hit(state);
            let damage = apply_monster_intent_with_card_rng_and_revival(
                &mut state.monsters[index],
                &mut state.player,
                &mut state.piles,
                allocated_card_id_through,
                ascension,
                &player_snapshot,
                &state.relics,
                player_can_revive,
                &mut state.rng.card_random_rng,
            )?;
            let painful_stabs = state.monsters[index].powers.painful_stabs;
            apply_monster_pending_effects(
                state,
                crate::MonsterIntent::Attack { damage: 3 },
                damage,
                1,
                painful_stabs,
                None,
                0,
                0,
                0,
                0,
                false,
                0,
                Some(index),
            )?;
            record_target_move(&mut state.monsters[index]);
            state.monsters[index].intent = target_byrd_go_airborne_intent();
            record_target_move(&mut state.monsters[index]);
            Ok(true)
        }
        crate::MonsterIntent::SummonGremlins { count } => {
            let summoner_id = state.monsters[index].id;
            let max_existing_monster_id = state
                .monsters
                .iter()
                .map(|monster| monster.id.get())
                .max()
                .unwrap_or(0);
            if state.monsters[index].content_id == BRONZE_AUTOMATON_ID {
                apply_bronze_automaton_orb_spawn(
                    &mut state.monsters,
                    summoner_id,
                    count,
                    &mut state.rng.monster_rng,
                    &mut state.rng.monster_hp_rng,
                    ascension,
                )?;
            } else if state.monsters[index].content_id == THE_COLLECTOR_ID {
                apply_collector_spawn_torch_heads(
                    &mut state.monsters,
                    count,
                    &mut state.rng.monster_rng,
                    &mut state.rng.monster_hp_rng,
                    ascension,
                    nilrys_second_queue_item,
                )?;
            } else if state.monsters[index].content_id == ACID_SLIME_ID {
                apply_large_acid_slime_split(
                    &mut state.monsters,
                    summoner_id,
                    count,
                    &mut state.rng.monster_rng,
                    ascension,
                )?;
            } else if state.monsters[index].content_id == SPIKE_SLIME_ID {
                apply_large_spike_slime_split(
                    &mut state.monsters,
                    summoner_id,
                    count,
                    &mut state.rng.monster_rng,
                    ascension,
                )?;
            } else if state.monsters[index].content_id == SLIME_BOSS_ID {
                apply_slime_boss_split(
                    &mut state.monsters,
                    summoner_id,
                    count,
                    &mut state.rng.monster_rng,
                    ascension,
                )?;
            } else if state.monsters[index].content_id == REPTOMANCER_ID {
                apply_reptomancer_dagger_spawn(
                    &mut state.monsters,
                    summoner_id,
                    count,
                    &mut state.rng.monster_rng,
                    &mut state.rng.monster_hp_rng,
                )?;
            } else if state.monsters[index].content_id == GREMLIN_LEADER_ID {
                apply_gremlin_leader_rally_target(
                    &mut state.monsters,
                    count,
                    &mut state.rng.monster_rng,
                    &mut state.rng.monster_hp_rng,
                    ascension,
                )?;
            } else {
                return Err(SimError::InvalidState(
                    "summon intent is incompatible with monster content",
                ));
            }
            apply_spawn_relic_effects(&mut state.monsters, max_existing_monster_id, &state.relics)?;
            let mut summoner_alive = false;
            if let Some(monster) = state
                .monsters
                .iter_mut()
                .find(|monster| monster.id == summoner_id)
            {
                checked_turn_increment(&mut monster.moves_executed)?;
                summoner_alive = monster.alive;
            }
            if summoner_alive {
                prepare_next_intent_for_actor(state, actor_id)?;
            }
            Ok(true)
        }
        crate::MonsterIntent::SummonCollectorTorchHeads { count } => {
            let summoner_id = state.monsters[index].id;
            let max_existing_monster_id = state
                .monsters
                .iter()
                .map(|monster| monster.id.get())
                .max()
                .unwrap_or(0);
            apply_collector_spawn_torch_heads(
                &mut state.monsters,
                count,
                &mut state.rng.monster_rng,
                &mut state.rng.monster_hp_rng,
                ascension,
                nilrys_second_queue_item,
            )?;
            apply_spawn_relic_effects(&mut state.monsters, max_existing_monster_id, &state.relics)?;
            if let Some(monster) = state
                .monsters
                .iter_mut()
                .find(|monster| monster.id == summoner_id)
            {
                checked_turn_increment(&mut monster.moves_executed)?;
            }
            prepare_next_intent_for_actor(state, actor_id)?;
            Ok(true)
        }
        crate::MonsterIntent::Block { block }
            if state.monsters[index].content_id == BRONZE_ORB_ID =>
        {
            if let Some(automaton) = state
                .monsters
                .iter_mut()
                .find(|monster| monster.alive && monster.content_id == BRONZE_AUTOMATON_ID)
            {
                automaton.block = checked_turn_add(automaton.block, block)?;
            }
            if let Some(monster) = state
                .monsters
                .iter_mut()
                .find(|monster| monster.id == actor_id)
            {
                checked_turn_increment(&mut monster.moves_executed)?;
            }
            prepare_next_intent_for_actor(state, actor_id)?;
            Ok(true)
        }
        crate::MonsterIntent::Block { block } if state.monsters[index].content_id == DECA_ID => {
            apply_deca_square(&mut state.monsters, block, ascension)?;
            if let Some(monster) = state
                .monsters
                .iter_mut()
                .find(|monster| monster.id == actor_id)
            {
                checked_turn_increment(&mut monster.moves_executed)?;
            }
            prepare_next_intent_for_actor(state, actor_id)?;
            Ok(true)
        }
        crate::MonsterIntent::Block { block }
            if matches!(
                state.monsters[index].content_id,
                CENTURION_ID | GREMLIN_TSUNDERE_ID
            ) =>
        {
            apply_shield_gremlin_random_block(
                &mut state.monsters,
                actor_id,
                block,
                &mut state.rng.monster_rng,
            )?;
            if let Some(monster) = state
                .monsters
                .iter_mut()
                .find(|monster| monster.id == actor_id)
            {
                checked_turn_increment(&mut monster.moves_executed)?;
            }
            prepare_next_intent_for_actor(state, actor_id)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn apply_spawn_relic_effects(
    monsters: &mut [crate::MonsterState],
    max_existing_monster_id: u64,
    relics: &[crate::Relic],
) -> SimResult<()> {
    if !relics.contains(&crate::Relic::PhilosophersStone) {
        return Ok(());
    }
    for monster in monsters
        .iter_mut()
        .filter(|monster| monster.alive && monster.id.get() > max_existing_monster_id)
    {
        monster.powers.strength = checked_turn_add(
            monster.powers.strength,
            crate::relic::PHILOSOPHERS_STONE_MONSTER_STRENGTH,
        )?;
    }
    Ok(())
}

fn finish_monster_turn_cleanup(
    state: &mut CombatState,
    skip_ritual_tick: &[MonsterId],
) -> SimResult<()> {
    for monster in &mut state.monsters {
        if monster.alive {
            if monster.powers.vulnerable > 0 {
                monster.powers.vulnerable -= 1;
            }
            monster.vulnerable_just_applied = false;
            if monster.powers.weak > 0 {
                monster.powers.weak -= 1;
            }
            if monster.powers.malleable_base > 0 {
                monster.powers.malleable = monster.powers.malleable_base;
            }
            if skip_ritual_tick.contains(&monster.id) {
                apply_end_of_monster_turn_powers_without_ritual(monster)?;
            } else {
                apply_end_of_monster_turn_powers(monster)?;
            }
            // RegenerateMonsterPower.atEndOfTurn addToBot's HealAction when the
            // owner is not halfDead / isDying / isDead. This loop already skips
            // !alive monsters (Awakened One / Darkling half-dead forms).
            if monster.powers.regeneration > 0 {
                monster.hp = monster
                    .hp
                    .saturating_add(monster.powers.regeneration)
                    .min(monster.max_hp);
            }
            if monster.content_id == BYRD_ID && monster.powers.flight > 0 {
                monster.powers.flight = target_byrd_flight_amount(state.ascension);
            }
            if monster.temp_strength_down > 0 {
                monster.powers.strength =
                    checked_turn_add(monster.powers.strength, monster.temp_strength_down)?;
                monster.temp_strength_down = 0;
            }
        }
    }

    if state.player.powers.vulnerable > 0 && state.player.vulnerable_just_applied {
        state.player.vulnerable_just_applied = false;
    } else if state.player.powers.vulnerable > 0 {
        state.player.powers.vulnerable -= 1;
    } else {
        state.player.vulnerable_just_applied = false;
    }
    tick_player_weak_at_end_of_round(state);
    tick_player_frail_at_end_of_round(state);
    if state.player.powers.intangible > 0 {
        state.player.powers.intangible -= 1;
    }

    apply_turn_transition_block_loss(state);
    Ok(())
}

pub(crate) fn revival_hp(max_hp: i32, percent: i32) -> SimResult<i32> {
    if max_hp <= 0 || !(1..=100).contains(&percent) {
        return Err(SimError::InvalidState(
            "combat revival HP inputs are outside the target domain",
        ));
    }
    let healed = (i64::from(max_hp) * i64::from(percent) / 100).max(1);
    i32::try_from(healed).map_err(|_| SimError::InvalidState("combat revival HP overflows i32"))
}

pub(crate) fn revival_hp_with_relics(
    max_hp: i32,
    percent: i32,
    relics: &[crate::Relic],
) -> SimResult<i32> {
    let base_heal = i64::from(revival_hp(max_hp, percent)?);
    let heal = if relics.contains(&crate::Relic::MagicFlower) {
        (base_heal * i64::from(crate::relic::MAGIC_FLOWER_HEAL_NUMERATOR)
            + i64::from(crate::relic::MAGIC_FLOWER_HEAL_DENOMINATOR) / 2)
            / i64::from(crate::relic::MAGIC_FLOWER_HEAL_DENOMINATOR)
    } else {
        base_heal
    };
    i32::try_from(heal.min(i64::from(max_hp)).max(1))
        .map_err(|_| SimError::InvalidState("combat revival HP overflows i32"))
}

fn revive_with_lizard_tail_if_available(state: &mut CombatState) -> SimResult<()> {
    if state.player.hp > 0
        || state.mark_of_bloom
        || !state.relics.contains(&crate::Relic::LizardTail)
        || !state.relic_counters.lizard_tail_available
    {
        return Ok(());
    }

    // LizardTail.onTrigger calls player.heal(maxHealth/2, true), which runs
    // MagicFlower.onPlayerHeal in combat (FIDL02322).
    let hp = revival_hp_with_relics(
        state.player.max_hp,
        crate::relic::LIZARD_TAIL_HEAL_PERCENT,
        &state.relics,
    )?;
    state.relic_counters.lizard_tail_available = false;
    state.player.hp = hp;
    Ok(())
}

fn revive_with_fairy_if_available(state: &mut CombatState) -> SimResult<()> {
    if state.player.hp > 0 || state.mark_of_bloom || state.relic_counters.fairy_heal_percent <= 0 {
        return Ok(());
    }

    state.player.hp = revival_hp_with_relics(
        state.player.max_hp,
        state.relic_counters.fairy_heal_percent,
        &state.relics,
    )?;
    state.relic_counters.fairy_heal_percent = 0;
    state.relic_counters.fairy_consumed = true;
    Ok(())
}

pub(crate) fn revive_player_if_available(state: &mut CombatState) -> SimResult<()> {
    revive_with_lizard_tail_if_available(state)?;
    revive_with_fairy_if_available(state)
}

fn apply_nemesis_intangible_if_absent(state: &mut CombatState) {
    for monster in &mut state.monsters {
        if monster.content_id == NEMESIS_ID && monster.alive && monster.powers.intangible == 0 {
            monster.powers.intangible = 1;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_monster_pending_effects(
    state: &mut CombatState,
    intent: crate::MonsterIntent,
    damage: i32,
    hits: i32,
    painful_stabs: i32,
    heal_self: Option<MonsterId>,
    heal_self_thorns: i32,
    burn_to_discard_and_draw: i32,
    weak: i32,
    burn_to_discard: i32,
    burn_to_discard_upgraded: bool,
    upgrade_burns: i32,
    attacker_index: Option<usize>,
) -> SimResult<()> {
    fn attacker_is_dying_or_dead(state: &CombatState, attacker_index: Option<usize>) -> bool {
        attacker_index.is_some_and(|index| {
            state
                .monsters
                .get(index)
                .is_none_or(|monster| !monster.alive || awakened_one_is_half_dead(monster))
        })
    }
    // PainfulStabsPower.onInflictDamage queues MakeTempCardInDiscardAction via
    // addToBot during AbstractPlayer.damage. Those actions sit behind the rest of
    // the attack queue. A lethal hit constructs DeathScreen and switches
    // AbstractDungeon.screen to DEATH mid-DamageAction, which stops the action
    // manager before deferred status cards (and remaining multi-hits) resolve.
    // Count unblocked hits here, settle Wounds only if the player is still alive
    // after the full attack sequence, and cancel remaining multi-hits on death.
    //
    // RunicCube / CentennialPuzzle also addToBot DrawCardAction. Resolving those
    // draws mid multi-hit lets The Abacus grant block between stabs (aef32ab6:
    // 5 hits + mid-hit Abacus instead of 6 full stabs). Defer draws until after
    // the whole multi-hit DamageAction sequence, matching GameActionManager.
    let mut total_hp_damage = 0;
    let mut painful_stabs_triggers = 0;
    let hit_count = hits.max(1);
    // ThornsPower.onAttacked addToBots retaliation, so a Byrd can die in the
    // intent resolver before these DamageActions run and still deliver every
    // queued peck. StaticDischargePower.onAttacked addToTops ChannelAction, so
    // a full-slot evoke can kill Deca between beams (FIDL02358).
    let skip_after_mid_hit_death = !attacker_is_dying_or_dead(state, attacker_index);
    if damage > 0 && hit_count > 1 {
        let hit_damage = damage / hit_count;
        for _ in 0..hit_count {
            if state.player.hp <= 0 {
                break;
            }
            if skip_after_mid_hit_death && attacker_is_dying_or_dead(state, attacker_index) {
                break;
            }
            let hp_damage = deal_damage_to_player_with_draw_policy(
                state,
                hit_damage,
                HpLossDrawPolicy::DeferDraws,
            )?;
            if hp_damage > 0 && painful_stabs > 0 {
                painful_stabs_triggers = checked_turn_add(painful_stabs_triggers, painful_stabs)?;
            }
            total_hp_damage = checked_turn_add(total_hp_damage, hp_damage)?;
        }
    } else if damage > 0 {
        let hp_damage = deal_damage_to_player(state, damage)?;
        if hp_damage > 0 && painful_stabs > 0 {
            painful_stabs_triggers = painful_stabs;
        }
        total_hp_damage = checked_turn_add(total_hp_damage, hp_damage)?;
    }
    if state.player.hp <= 0 {
        // Drop the remaining queued effects — the death screen freezes the bot
        // queue after the lethal DamageAction.
        state.relic_counters.deferred_centennial_puzzle_draw = false;
        state.relic_counters.deferred_runic_cube_draws = 0;
        return Ok(());
    }
    apply_queued_post_attack_player_debuffs(intent, &mut state.player, &state.relics)?;
    // Nemesis.takeTurn queues ApplyPowerAction(Intangible) after its DamageActions
    // when it does not already have the power. RunicCube.wasHPLost addToTops a
    // DrawCardAction; FireBreathingPower.onCardDraw addToBots DamageAllEnemiesAction,
    // so that damage resolves after Intangible (FIDL01313: 42→39, not 42→34).
    apply_nemesis_intangible_if_absent(state);
    // PainfulStabsPower and the HP-loss draw relics both queue addToBot actions
    // from the same multi-hit DamageAction. The draw actions are queued before
    // PainfulStabs' MakeTempCardInDiscardAction, so settle them first. This is
    // observable when the opening draw requires a shuffle: newly generated
    // Wounds must remain in discard while the deferred draws consume the
    // pre-existing pile (FIDL01519 step 345).
    crate::relic::settle_deferred_hp_loss_draw_relics(state)?;
    settle_deferred_painful_stabs_wounds(state, painful_stabs_triggers)?;
    if weak > 0 {
        let had_no_weak = state.player.powers.weak == 0;
        crate::relic::apply_player_weak_with_relics(&mut state.player.powers, &state.relics, weak)?;
        if had_no_weak && state.player.powers.weak > 0 {
            state.player.weak_just_applied = true;
        }
    }
    apply_attack_heal_self_after_player_damage(state, heal_self, total_hp_damage)?;
    apply_attack_heal_self_thorns_after_heal(state, heal_self, heal_self_thorns);
    if burn_to_discard_and_draw > 0 {
        let allocated_card_id_through = state.max_authoritative_card_instance_id();
        add_cards_to_draw_random_spot(
            &mut state.piles,
            BURN_ID,
            burn_to_discard_and_draw,
            &mut state.rng.card_random_rng,
            allocated_card_id_through,
        )?;
        let allocated_card_id_through = state.max_authoritative_card_instance_id();
        add_cards_to_discard(
            &mut state.piles,
            BURN_ID,
            burn_to_discard_and_draw,
            allocated_card_id_through,
        )?;
    }
    if burn_to_discard > 0 {
        let allocated_card_id_through = state.max_authoritative_card_instance_id();
        if burn_to_discard_upgraded {
            add_upgraded_burns_to_discard(
                &mut state.piles,
                burn_to_discard,
                allocated_card_id_through,
            )?;
        } else {
            add_cards_to_discard(
                &mut state.piles,
                BURN_ID,
                burn_to_discard,
                allocated_card_id_through,
            )?;
        }
    }
    if upgrade_burns > 0 {
        let allocated_card_id_through = state.max_authoritative_card_instance_id();
        upgrade_burns_and_add_upgraded_to_discard(
            &mut state.piles,
            upgrade_burns,
            allocated_card_id_through,
        )?;
    }
    Ok(())
}

fn effective_current_move_hits(
    original: crate::MonsterIntent,
    after_effects: crate::MonsterIntent,
) -> i32 {
    match (original, after_effects) {
        (
            crate::MonsterIntent::AttackMultiple { .. },
            crate::MonsterIntent::AttackMultiple { hits, .. },
        )
        | (
            crate::MonsterIntent::AttackMultipleApplyPlayerWeak { .. },
            crate::MonsterIntent::AttackMultipleApplyPlayerWeak { hits, .. },
        )
        | (
            crate::MonsterIntent::AttackMultipleAddDazedToDiscard { .. },
            crate::MonsterIntent::AttackMultipleAddDazedToDiscard { hits, .. },
        )
        | (
            crate::MonsterIntent::AttackMultipleUpgradeBurns { .. },
            crate::MonsterIntent::AttackMultipleUpgradeBurns { hits, .. },
        ) => hits,
        (crate::MonsterIntent::AttackMultiple { hits, .. }, _)
        | (crate::MonsterIntent::AttackMultipleApplyPlayerWeak { hits, .. }, _)
        | (crate::MonsterIntent::AttackMultipleAddDazedToDiscard { hits, .. }, _)
        | (crate::MonsterIntent::AttackMultipleUpgradeBurns { hits, .. }, _) => hits,
        _ => 1,
    }
}

fn apply_turn_transition_block_loss(state: &mut CombatState) {
    if state.player.powers.barricade > 0 {
        return;
    }

    if state.relics.contains(&crate::Relic::Calipers) {
        state.player.block = (state.player.block - crate::relic::CALIPERS_BLOCK_LOSS).max(0);
    } else {
        state.player.block = 0;
    }
}

fn apply_transient_fading_after_turn(monsters: &mut [crate::MonsterState], actor_id: MonsterId) {
    let Some(monster) = monsters
        .iter_mut()
        .find(|monster| monster.id == actor_id && monster.content_id == TRANSIENT_ID)
    else {
        return;
    };
    if monster.moves_executed < 5 {
        return;
    }
    monster.alive = false;
    monster.escaped = true;
    monster.block = 0;
    monster.intent = crate::MonsterIntent::Attack { damage: 0 };
}

pub(crate) fn deal_damage_to_player(state: &mut CombatState, amount: i32) -> SimResult<i32> {
    deal_damage_to_player_with_draw_policy(state, amount, HpLossDrawPolicy::Immediate)
}

pub(crate) fn deal_non_attack_damage_to_player(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<i32> {
    let incoming = crate::combat::hp_loss::cap_player_damage_with_intangible(&state.player, amount);
    let blocked = state.player.block.min(incoming);
    state.player.block -= blocked;
    let hp_damage = crate::relic::apply_buffer_to_hp_loss(
        &mut state.player.powers,
        incoming.saturating_sub(blocked),
    );
    state.player.hp = (state.player.hp - hp_damage).max(0);
    crate::combat::hp_loss::apply_player_hp_loss_hooks_with_draw_policy(
        state,
        hp_damage,
        HpLossDrawPolicy::Immediate,
    )?;
    revive_player_if_available(state)?;
    if hp_damage > 0 && state.player.powers.plated_armor > 0 {
        state.player.powers.plated_armor -= 1;
    }
    Ok(hp_damage)
}

fn deal_damage_to_player_with_draw_policy(
    state: &mut CombatState,
    amount: i32,
    draw_policy: HpLossDrawPolicy,
) -> SimResult<i32> {
    let incoming = crate::combat::hp_loss::cap_player_damage_with_intangible(&state.player, amount);
    let blocked = state.player.block.min(incoming);
    state.player.block -= blocked;
    let mitigated =
        crate::relic::mitigate_unblocked_attack_damage(&state.relics, incoming - blocked);
    let hp_damage = crate::relic::apply_buffer_to_hp_loss(&mut state.player.powers, mitigated);
    state.player.hp = (state.player.hp - hp_damage).max(0);
    crate::combat::hp_loss::apply_player_hp_loss_hooks_with_draw_policy(
        state,
        hp_damage,
        draw_policy,
    )?;
    crate::combat::transition::apply_static_discharge_on_attacked(state, hp_damage)?;
    revive_player_if_available(state)?;
    if hp_damage > 0 && state.player.powers.plated_armor > 0 {
        state.player.powers.plated_armor -= 1;
    }
    Ok(hp_damage)
}

fn settle_deferred_painful_stabs_wounds(
    state: &mut CombatState,
    wound_count: i32,
) -> SimResult<()> {
    if wound_count <= 0 || state.player.hp <= 0 {
        return Ok(());
    }
    let allocated_card_id_through = state.max_authoritative_card_instance_id();
    add_cards_to_discard(
        &mut state.piles,
        WOUND_ID,
        wound_count,
        allocated_card_id_through,
    )
}

fn apply_attack_heal_self_after_player_damage(
    state: &mut CombatState,
    monster_id: Option<MonsterId>,
    hp_damage: i32,
) -> SimResult<()> {
    if hp_damage <= 0 {
        return Ok(());
    }
    let Some(monster_id) = monster_id else {
        return Ok(());
    };
    if let Some(monster) = state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == monster_id && monster.alive)
    {
        heal_monster_to_stored_cap(monster, hp_damage)?;
    }
    Ok(())
}

fn apply_attack_heal_self_thorns_after_heal(
    state: &mut CombatState,
    monster_id: Option<MonsterId>,
    thorns_damage: i32,
) {
    if thorns_damage <= 0 {
        return;
    }
    let Some(monster_id) = monster_id else {
        return;
    };
    if let Some(monster) = state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == monster_id && monster.alive)
    {
        crate::combat::damage::deal_unmodified_damage_to_monster(monster, thorns_damage);
    }
}

#[cfg(test)]
fn draw_next_hand_without_shuffle(state: &mut CombatState) -> SimResult<()> {
    let follow_ups = draw_next_hand_without_shuffle_deferred(state)?;
    crate::combat::transition::resolve_deferred_draw_follow_ups(state, follow_ups)
}

fn draw_next_hand_without_shuffle_deferred(
    state: &mut CombatState,
) -> SimResult<Vec<crate::action::InternalAction>> {
    // Target GameActionManager queues a single DrawCardAction(gameHandSize).
    // EvolvePower.addToBot follow-ups therefore run after the full base refill,
    // not interleaved between remaining base draws.
    let count = next_hand_draw_count(state);
    draw_cards_with_combat_rng_deferred_evolve(state, count)
}

pub(crate) fn target_hand_size(state: &CombatState) -> usize {
    HAND_SIZE
        + if state.relics.contains(&crate::Relic::SneckoEye) {
            crate::relic::SNECKO_EYE_DRAW
        } else {
            0
        }
}

fn next_hand_draw_count(state: &CombatState) -> usize {
    // AbstractPlayer.draw(gameHandSize) draws the complete turn batch even
    // when start-of-turn powers (for example Magnetism) have already added
    // cards to hand. Only the hard ten-card hand cap limits that batch; do not
    // subtract the pre-draw hand length from gameHandSize (FIDL00273).
    target_hand_size(state)
        .saturating_sub(usize::from(state.player.powers.draw_reduction > 0))
        .min(MAX_HAND_SIZE.saturating_sub(state.piles.hand.len()))
}

fn prepare_next_intent_for_actor(state: &mut CombatState, actor_id: MonsterId) -> SimResult<()> {
    prepare_next_intents_for_ids(state, Some(&[actor_id]))
}

fn prepare_next_intents_for_ids(
    state: &mut CombatState,
    only_ids: Option<&[MonsterId]>,
) -> SimResult<()> {
    let living_monster_count = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .count();
    let alive_gremlin_count = gremlin_leader_alive_minion_count(&state.monsters);
    let collector_minion_dead = state
        .monsters
        .iter()
        .filter(|monster| monster.powers.minion != 0 && monster.alive)
        .count()
        < 2;
    let missing_hp = living_monster_missing_hp(&state.monsters);
    let rolled_context = RolledIntentContext {
        ascension: state.ascension,
        player_hp: state.player.hp,
        player_constricted: state.player.powers.constricted > 0,
        living_monster_count,
        alive_gremlin_count,
        collector_minion_dead,
        missing_hp,
    };
    for (monster_index, monster) in state.monsters.iter_mut().enumerate() {
        if only_ids.is_some_and(|ids| !ids.contains(&monster.id)) {
            continue;
        }
        if is_half_dead_darkling(monster) {
            let _ = state.rng.monster_rng.random_int(99);
            monster.intent = crate::MonsterIntent::Stun;
            record_target_move(monster);
            continue;
        }
        if monster.content_id == BYRD_ID
            && !monster.alive
            && matches!(monster.intent, crate::MonsterIntent::Stun)
        {
            continue;
        }

        if monster.alive || only_ids.is_some() {
            if monster.initial_intent_locked {
                monster.initial_intent_locked = false;
                record_target_move(monster);
                continue;
            }
            if monster.split_triggered
                && matches!(monster.intent, crate::MonsterIntent::SummonGremlins { .. })
                && matches!(
                    monster.content_id,
                    ACID_SLIME_ID | SPIKE_SLIME_ID | SLIME_BOSS_ID
                )
            {
                // Acid/Spike non-split takeTurns (Tackle / Corrosive Spit / Lick)
                // still queue RollMoveAction. SplitPower may already have forced
                // the SPLIT intent mid-turn (e.g. reactive thorns), but
                // AbstractMonster still draws the common AI roll before getMove
                // re-asserts SPLIT. Consume that draw so post-split child spawn
                // rolls stay aligned.
                //
                // SlimeBoss never queues RollMoveAction: its cycle setMoves the
                // next intent inside takeTurn, and damage() interrupts to SPLIT
                // without an AI draw. Do not consume monster_rng for the boss.
                //
                // The SPLIT takeTurn itself does not queue RollMoveAction; when
                // the parent is already dead after spawning children, skip
                // without a draw.
                if monster.alive {
                    if matches!(monster.content_id, ACID_SLIME_ID | SPIKE_SLIME_ID) {
                        let _ = state.rng.monster_rng.random_int(99);
                    }
                    record_target_move(monster);
                }
                continue;
            }
            if monster.content_id == crate::content::monsters::AWAKENED_ONE_ID
                && monster.mode_shift == 1
                && !monster.move_history.is_empty()
            {
                // AbstractMonster.rollMove supplies the common AI roll to
                // AwakenedOne.getMove; phase two uses that roll directly.
                let roll = state.rng.monster_rng.random_int(99);
                monster.intent =
                    crate::content::monsters::target_awakened_one_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        monster.mode_shift,
                        state.ascension,
                    );
                record_target_move(monster);
                continue;
            }
            if monster.content_id == crate::content::monsters::TIME_EATER_ID {
                let roll = state.rng.monster_rng.random_int(99);
                monster.intent = crate::content::monsters::target_time_eater_next_intent_from_roll(
                    &monster.move_history,
                    roll,
                    monster.hp,
                    monster.max_hp,
                    state.ascension,
                    &mut state.rng.monster_rng,
                );
                record_target_move(monster);
                continue;
            }
            if prepare_direct_next_intent(
                monster,
                &mut state.rng.monster_rng,
                state.ascension,
                living_monster_count,
            )? {
                continue;
            }
            let roll = state.rng.monster_rng.random_int(99);
            prepare_rolled_next_intent(
                monster,
                &mut state.rng.monster_rng,
                monster_index,
                roll,
                rolled_context,
            )?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct RolledIntentContext {
    ascension: u8,
    player_hp: i32,
    player_constricted: bool,
    living_monster_count: usize,
    alive_gremlin_count: usize,
    collector_minion_dead: bool,
    missing_hp: i32,
}

fn prepare_rolled_next_intent(
    monster: &mut crate::MonsterState,
    monster_rng: &mut StsRng,
    monster_index: usize,
    roll: i32,
    context: RolledIntentContext,
) -> SimResult<()> {
    let RolledIntentContext {
        ascension,
        player_hp,
        player_constricted,
        living_monster_count,
        alive_gremlin_count,
        collector_minion_dead,
        missing_hp,
    } = context;
    monster.intent = if monster.content_id == CORRUPT_HEART_ID {
        let echo = || crate::MonsterIntent::Attack {
            damage: if ascension >= 4 { 45 } else { 40 },
        };
        let blood = || crate::MonsterIntent::AttackMultiple {
            damage: 2,
            hits: if ascension >= 4 { 15 } else { 12 },
        };
        match monster.moves_executed.saturating_sub(1) % 3 {
            0 => {
                if monster_rng.random_bool() {
                    blood()
                } else {
                    echo()
                }
            }
            1 if monster.move_history.last() == Some(&2) => blood(),
            1 => echo(),
            _ => crate::MonsterIntent::StrengthSelf { amount: 2 },
        }
    } else if monster.content_id == HEXAGHOST_ID && monster.moves_executed == 1 {
        crate::MonsterIntent::AttackMultiple {
            damage: (player_hp / 12) + 1,
            hits: 6,
        }
    } else if monster.content_id == BRONZE_AUTOMATON_ID {
        target_bronze_automaton_next_intent(
            monster.moves_executed,
            &monster.move_history,
            ascension,
        )
    } else if monster.content_id == crate::content::monsters::AWAKENED_ONE_ID {
        crate::content::monsters::target_awakened_one_next_intent_from_roll(
            &monster.move_history,
            roll,
            monster.mode_shift,
            ascension,
        )
    } else if monster.content_id == crate::content::monsters::TIME_EATER_ID {
        crate::content::monsters::target_time_eater_next_intent_from_roll(
            &monster.move_history,
            roll,
            monster.hp,
            monster.max_hp,
            ascension,
            monster_rng,
        )
    } else if monster.content_id == EXPLODER_ID {
        target_exploder_next_intent_from_roll(monster.moves_executed, ascension)
    } else if monster.content_id == SPHERIC_GUARDIAN_ID {
        target_spheric_guardian_next_intent_from_roll(
            monster.moves_executed,
            &monster.move_history,
            ascension,
        )
    } else if monster.content_id == MAW_ID {
        target_maw_next_intent_from_roll(
            monster.moves_executed,
            &monster.move_history,
            roll,
            ascension,
        )
    } else if monster.content_id == SPIRE_GROWTH_ID {
        target_spire_growth_next_intent_from_roll(
            monster.moves_executed,
            &monster.move_history,
            roll,
            player_constricted,
            ascension,
        )
    } else if monster.content_id == GIANT_HEAD_ID {
        target_giant_head_next_intent_from_roll(
            monster.moves_executed,
            &monster.move_history,
            roll,
            ascension,
        )
    } else if monster.content_id == SPIRE_SHIELD_ID {
        match monster.moves_executed % 3 {
            0 => {
                if monster_rng.random_bool() {
                    crate::MonsterIntent::Block { block: 30 }
                } else {
                    crate::MonsterIntent::AttackApplyPlayerWeak {
                        damage: if ascension >= 3 { 14 } else { 12 },
                        weak: 0,
                    }
                }
            }
            1 if monster.move_history.last() == Some(&1) => {
                crate::MonsterIntent::Block { block: 30 }
            }
            1 => crate::MonsterIntent::AttackApplyPlayerWeak {
                damage: if ascension >= 3 { 14 } else { 12 },
                weak: 0,
            },
            _ => crate::MonsterIntent::AttackAndBlock {
                damage: if ascension >= 3 { 38 } else { 34 },
                block: if ascension >= 18 {
                    99
                } else {
                    let base = if ascension >= 3 { 38 } else { 34 };
                    (base + monster.powers.strength) * 3 / 2
                },
            },
        }
    } else if monster.content_id == SPIRE_SPEAR_ID {
        let burn_strike = || crate::MonsterIntent::AttackMultipleApplyPlayerWeak {
            damage: if ascension >= 3 { 6 } else { 5 },
            hits: 2,
            weak: 0,
        };
        match monster.moves_executed % 3 {
            0 if monster.move_history.last() == Some(&1) => {
                crate::MonsterIntent::StrengthAllMonsters { amount: 2 }
            }
            0 => burn_strike(),
            1 => crate::MonsterIntent::AttackMultiple {
                damage: 10,
                hits: if ascension >= 3 { 4 } else { 3 },
            },
            _ if monster_rng.random_bool() => {
                crate::MonsterIntent::StrengthAllMonsters { amount: 2 }
            }
            _ => burn_strike(),
        }
    } else if monster.content_id == WRITHING_MASS_ID {
        target_writhing_mass_next_intent_from_roll(
            false,
            &monster.move_history,
            monster.has_siphoned,
            roll,
            monster_rng,
            ascension,
        )
    } else if monster.content_id == NEMESIS_ID {
        target_nemesis_next_intent_from_roll(
            monster.moves_executed,
            &monster.move_history,
            roll,
            monster_rng,
            ascension,
        )
    } else if monster.content_id == JAW_WORM_ID {
        target_jaw_worm_next_intent_from_roll(&monster.move_history, roll, monster_rng)
    } else if monster.content_id == RED_LOUSE_ID {
        let attack_damage = monster
            .rolled_attack_damage
            .ok_or(crate::SimError::InvalidState(
                "monster requires rolled attack damage",
            ))?;
        target_louse_next_intent_from_roll(
            &monster.move_history,
            roll,
            attack_damage,
            crate::MonsterIntent::StrengthAndBlock {
                strength: LOUSE_CURL_STRENGTH,
                block: 0,
            },
        )
    } else if monster.content_id == GREEN_LOUSE_ID {
        let attack_damage = monster
            .rolled_attack_damage
            .ok_or(crate::SimError::InvalidState(
                "monster requires rolled attack damage",
            ))?;
        target_louse_next_intent_from_roll(
            &monster.move_history,
            roll,
            attack_damage,
            crate::MonsterIntent::ApplyPlayerWeak {
                amount: GREEN_LOUSE_WEAK,
            },
        )
    } else if monster.content_id == GREMLIN_NOB_ID {
        target_gremlin_nob_next_intent_from_roll(&monster.move_history, roll, ascension)
    } else if monster.content_id == CHOSEN_ID {
        target_chosen_next_intent_from_roll(&monster.move_history, roll, ascension)
    } else if monster.content_id == CHAMP_ID {
        target_champ_next_intent_from_roll(
            &monster.move_history,
            roll,
            monster.hp,
            monster.max_hp,
            ascension,
        )
    } else if monster.content_id == BYRD_ID {
        if monster.powers.flight <= 0 {
            target_grounded_byrd_next_intent()
        } else {
            target_byrd_next_intent_from_roll(&monster.move_history, roll, monster_rng, ascension)
        }
    } else if monster.content_id == ACID_SLIME_ID && acid_slime_uses_large_move_table(monster) {
        target_large_acid_slime_next_intent_from_roll(
            &monster.move_history,
            roll,
            monster_rng,
            ascension,
        )
    } else if monster.content_id == ACID_SLIME_ID && acid_slime_uses_medium_move_table(monster) {
        target_medium_acid_slime_next_intent_from_roll(
            &monster.move_history,
            roll,
            monster_rng,
            ascension,
        )
    } else if monster.content_id == SPIKE_SLIME_ID
        && spike_slime_uses_medium_or_large_move_table(monster)
    {
        target_medium_or_large_spike_slime_next_intent_from_roll_with_profile(
            spike_slime_uses_large_move_table(monster),
            &monster.move_history,
            roll,
            ascension,
        )
    } else if monster.content_id == SENTRY_ID {
        target_sentry_next_intent(&monster.move_history, monster_index, ascension)
    } else if monster.content_id == SHELLED_PARASITE_ID {
        target_shelled_parasite_next_intent_from_roll(
            &monster.move_history,
            roll,
            monster_rng,
            ascension,
        )
    } else if monster.content_id == SNAKE_PLANT_ID {
        target_snake_plant_next_intent_from_roll(&monster.move_history, roll, ascension)
    } else if monster.content_id == SNECKO_ID {
        target_snecko_next_intent_from_roll(&monster.move_history, roll, ascension)
    } else if monster.content_id == BOOK_OF_STABBING_ID {
        let mut stab_count = monster.powers.book_stab_count.max(1);
        let intent = target_book_of_stabbing_next_intent_from_roll_with_stab_count(
            &monster.move_history,
            &mut stab_count,
            roll,
            ascension,
        );
        monster.powers.book_stab_count = stab_count;
        intent
    } else if monster.content_id == CENTURION_ID {
        target_centurion_next_intent_from_roll(
            &monster.move_history,
            roll,
            living_monster_count,
            ascension,
        )
    } else if monster.content_id == HEALER_ID {
        target_healer_next_intent_from_roll(&monster.move_history, roll, missing_hp, ascension)
    } else if monster.content_id == FUNGI_BEAST_ID {
        target_fungi_beast_next_intent_from_roll(&monster.move_history, roll, ascension)
    } else if monster.content_id == SLAVER_BLUE_ID {
        target_slaver_blue_next_intent_from_roll(&monster.move_history, roll, ascension)
    } else if monster.content_id == SLAVER_RED_ID {
        target_slaver_red_next_intent_from_roll(&monster.move_history, roll, ascension)
    } else if monster.content_id == GREMLIN_LEADER_ID {
        target_gremlin_leader_next_intent_from_roll(
            &monster.move_history,
            roll,
            monster_rng,
            alive_gremlin_count,
            ascension,
        )
    } else if monster.content_id == THE_COLLECTOR_ID {
        target_collector_next_intent_from_roll(&monster.move_history, roll, collector_minion_dead)
    } else if monster.content_id == BRONZE_ORB_ID {
        target_bronze_orb_next_intent_from_roll(&monster.move_history, roll)
    } else if monster.content_id == ORB_WALKER_ID {
        target_orb_walker_next_intent_from_roll(&monster.move_history, roll, ascension)
    } else if monster.content_id == REPTOMANCER_ID {
        target_reptomancer_next_intent_from_roll(
            &monster.move_history,
            roll,
            living_monster_count.saturating_sub(1) <= 3,
            monster_rng,
            ascension,
        )
    } else if monster.content_id == REPULSOR_ID {
        target_repulsor_next_intent_from_roll(&monster.move_history, roll, ascension)
    } else if monster.content_id == DARKLING_ID {
        let attack_damage = monster
            .rolled_attack_damage
            .ok_or(crate::SimError::InvalidState(
                "monster requires rolled attack damage",
            ))?;
        crate::content::monsters::target_darkling_next_intent_from_roll_with_rng(
            &monster.move_history,
            roll,
            monster_index,
            attack_damage,
            ascension,
            monster_rng,
        )
    } else if monster.content_id == SPIKER_ID {
        target_spiker_next_intent_from_roll(
            &monster.move_history,
            monster.powers.spiker_thorns_buffs,
            roll,
            ascension,
        )
    } else {
        prepare_monster_intent_for_ascension(monster, ascension)?
    };
    record_target_move(monster);
    Ok(())
}

fn prepare_direct_next_intent(
    monster: &mut crate::MonsterState,
    monster_rng: &mut StsRng,
    ascension: u8,
    living_monster_count: usize,
) -> SimResult<bool> {
    if monster.content_id == DAGGER_ID {
        // SnakeDagger.rollMove always consumes the AI draw, but its getMove
        // ignores that roll after the initial move and reasserts move 2
        // (EXPLODE). This remains true when the suicide move killed the
        // dagger while another monster keeps the action queue alive.
        let _ = monster_rng.random_int(99);
        monster.intent = crate::MonsterIntent::Attack {
            damage: DAGGER_EXPLODE_DAMAGE,
        };
        record_target_move(monster);
        return Ok(true);
    }
    if monster.content_id == ACID_SLIME_ID
        && monster.hp <= ACID_SLIME_S_A7_HP_RANGE.max
        && !acid_slime_uses_medium_move_table(monster)
    {
        monster.intent = target_small_acid_slime_followup_intent(monster.intent, ascension);
        record_target_move(monster);
        return Ok(true);
    }
    if monster.content_id == SPIKE_SLIME_ID
        && monster.hp <= SPIKE_SLIME_S_A7_HP_RANGE.max
        && !spike_slime_uses_medium_or_large_move_table(monster)
    {
        let _ = monster_rng.random_int(99);
        monster.intent = crate::MonsterIntent::Attack {
            damage: if ascension >= 2 { 6 } else { 5 },
        };
        record_target_move(monster);
        return Ok(true);
    }
    if monster.content_id == TORCH_HEAD_ID {
        monster.intent = crate::MonsterIntent::Attack {
            damage: crate::content::monsters::TORCH_HEAD_ATTACK_DAMAGE,
        };
        record_target_move(monster);
        return Ok(true);
    }
    if monster.content_id == TRANSIENT_ID {
        monster.intent = crate::MonsterIntent::Attack {
            damage: crate::content::monsters::transient_attack_damage(
                monster.moves_executed,
                ascension,
            )?,
        };
        record_target_move(monster);
        return Ok(true);
    }
    if monster.content_id == LOOTER_ID {
        monster.intent = target_looter_direct_next_intent_after_turn(
            &monster.move_history,
            monster.moves_executed,
            monster_rng,
            ascension,
        );
        record_target_move(monster);
        return Ok(true);
    }
    if monster.content_id == MUGGER_ID {
        monster.intent = target_mugger_direct_next_intent_after_turn(
            &monster.move_history,
            monster.moves_executed,
            monster_rng,
            ascension,
        );
        record_target_move(monster);
        return Ok(true);
    }
    if matches!(monster.content_id, GREMLIN_WARRIOR_ID | GREMLIN_THIEF_ID) {
        monster.intent = source_backed_gremlin_leader_minion_intent(
            monster.content_id,
            monster.moves_executed,
            ascension,
        )
        .ok_or(SimError::UnsupportedMechanic(monster.content_id))?;
        record_target_move(monster);
        return Ok(true);
    }
    if monster.content_id == GREMLIN_TSUNDERE_ID {
        let mut source_branch = monster.clone();
        source_branch.moves_executed = if living_monster_count > 1 { 0 } else { 1 };
        monster.intent = source_backed_gremlin_leader_minion_intent(
            source_branch.content_id,
            source_branch.moves_executed,
            ascension,
        )
        .ok_or(SimError::UnsupportedMechanic(source_branch.content_id))?;
        record_target_move(monster);
        return Ok(true);
    }
    if monster.content_id == GREMLIN_WIZARD_ID {
        monster.intent =
            target_gremlin_wizard_direct_next_intent_after_turn(monster.moves_executed, ascension);
        record_target_move(monster);
        return Ok(true);
    }
    if monster.content_id == SLIME_BOSS_ID {
        monster.intent = prepare_monster_intent_for_ascension(monster, ascension)?;
        record_target_move(monster);
        return Ok(true);
    }
    Ok(false)
}

pub(super) fn reroll_writhing_mass_after_attack(state: &mut CombatState, actor_id: MonsterId) {
    let Some(monster_index) = state
        .monsters
        .iter()
        .position(|monster| monster.id == actor_id && monster.alive)
    else {
        return;
    };
    let rng = &mut state.rng.monster_rng;
    let roll = rng.random_int(99);
    let monster = &state.monsters[monster_index];
    let target_history = monster.move_history.clone();
    let intent = target_writhing_mass_next_intent_from_roll(
        false,
        &target_history,
        monster.has_siphoned,
        roll,
        rng,
        state.ascension,
    );
    let monster = &mut state.monsters[monster_index];
    monster.intent = intent;
    record_target_move(monster);
}

fn is_half_dead_darkling(monster: &crate::MonsterState) -> bool {
    monster.content_id == DARKLING_ID && !monster.alive && monster.escaped
}

fn acid_slime_uses_medium_move_table(monster: &crate::MonsterState) -> bool {
    match monster.slime_size {
        Some(SlimeSize::Small) => return false,
        Some(SlimeSize::Medium | SlimeSize::Large) => return true,
        None => {}
    }
    acid_slime_uses_large_move_table(monster)
        || monster.max_hp > ACID_SLIME_S_A7_HP_RANGE.max
        || monster.move_history.contains(&2)
        || matches!(
            monster.intent,
            crate::MonsterIntent::AttackAddSlimedToDiscard { .. }
        )
        || matches!(
            monster.intent,
            crate::MonsterIntent::Attack { damage }
                if damage >= crate::content::monsters::ACID_SLIME_M_NORMAL_TACKLE_DAMAGE
        )
}

fn acid_slime_uses_large_move_table(monster: &crate::MonsterState) -> bool {
    match monster.slime_size {
        Some(SlimeSize::Small | SlimeSize::Medium) => return false,
        Some(SlimeSize::Large) => return true,
        None => {}
    }
    monster.max_hp > ACID_SLIME_M_A7_HP_RANGE.max
        || matches!(
            monster.rolled_attack_damage,
            Some(damage) if damage >= crate::content::monsters::ACID_SLIME_L_NORMAL_TACKLE_DAMAGE
        )
}

fn spike_slime_uses_medium_or_large_move_table(monster: &crate::MonsterState) -> bool {
    match monster.slime_size {
        Some(SlimeSize::Small) => return false,
        Some(SlimeSize::Medium | SlimeSize::Large) => return true,
        None => {}
    }
    monster.hp > SPIKE_SLIME_S_A7_HP_RANGE.max
        || matches!(
            monster.intent,
            crate::MonsterIntent::AttackAddSlimedToDiscard { .. }
                | crate::MonsterIntent::ApplyPlayerFrailAndWeak { .. }
        )
}

fn spike_slime_uses_large_move_table(monster: &crate::MonsterState) -> bool {
    match monster.slime_size {
        Some(SlimeSize::Small | SlimeSize::Medium) => return false,
        Some(SlimeSize::Large) => return true,
        None => {}
    }
    monster.max_hp > crate::content::monsters::SPIKE_SLIME_M_A7_HP_RANGE.max
        || matches!(
            monster.rolled_attack_damage,
            Some(damage) if damage >= SPIKE_SLIME_L_SPIT_DAMAGE
        )
}

fn apply_shield_gremlin_random_block(
    monsters: &mut [crate::MonsterState],
    source_id: MonsterId,
    block: i32,
    rng: &mut StsRng,
) -> SimResult<()> {
    let candidates = monsters
        .iter()
        .enumerate()
        .filter_map(|(index, monster)| {
            (monster.id != source_id
                && monster.alive
                && !matches!(monster.intent, crate::MonsterIntent::Escape))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let target_index = if candidates.is_empty() {
        monsters.iter().position(|monster| monster.id == source_id)
    } else {
        Some(candidates[rng.random_int(candidates.len() as i32 - 1) as usize])
    };
    if let Some(target_index) = target_index {
        monsters[target_index].block = checked_turn_add(monsters[target_index].block, block)?;
    }
    Ok(())
}

fn apply_deca_square(
    monsters: &mut [crate::MonsterState],
    block: i32,
    ascension: u8,
) -> SimResult<()> {
    let values = monsters
        .iter()
        .map(|monster| {
            if !monster.alive {
                return Ok((monster.block, monster.powers.plated_armor));
            }
            let next_block = checked_turn_add(monster.block, block)?;
            let next_plated_armor = if ascension >= 19 {
                checked_turn_add(monster.powers.plated_armor, 3)?
            } else {
                monster.powers.plated_armor
            };
            Ok((next_block, next_plated_armor))
        })
        .collect::<SimResult<Vec<_>>>()?;
    for (monster, (block, plated_armor)) in monsters.iter_mut().zip(values) {
        monster.block = block;
        monster.powers.plated_armor = plated_armor;
    }
    Ok(())
}

fn gremlin_leader_alive_minion_count(monsters: &[crate::MonsterState]) -> usize {
    monsters
        .iter()
        .filter(|monster| {
            monster.alive
                && crate::content::monsters::is_gremlin_leader_minion_content_id(monster.content_id)
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::hand::resolve_end_of_turn_hand;
    use crate::content::cards::{
        ANGER_ID, ARMAMENTS_ID, BASH_ID, BERSERK_ID, BLOODLETTING_ID, BURNING_PACT_ID, BURN_ID,
        DAZED_ID, DEEP_BREATH_ID, DEFEND_R_ID, DEMON_FORM_ID, DOUBT_ID, GHOSTLY_ARMOR_ID,
        INFLAME_ID, INTIMIDATE_ID, PARASITE_ID, POMMEL_STRIKE_ID, REGRET_ID, SHAME_ID,
        SHRUG_IT_OFF_PLUS_ID, SLIMED_ID, STRIKE_R_ID, THUNDERCLAP_ID, VOID_ID, WOUND_ID,
    };
    use crate::content::monsters::{
        donu_deca_boss_monsters_for_ascension, monster_state_for_ascension,
        target_giant_head_next_intent_from_roll,
        target_gremlin_wizard_direct_next_intent_after_turn,
        target_looter_direct_next_intent_after_turn, target_nemesis_next_intent_from_roll,
        target_spheric_guardian_next_intent_from_roll, target_spire_growth_next_intent_from_roll,
        transient_attack_damage, ACID_SLIME_A0, BOOK_OF_STABBING_A0, BRONZE_AUTOMATON_A0,
        BRONZE_ORB_A0, BYRD_A0, CENTURION_A0, DAGGER_A0, DAGGER_EXPLODE_DAMAGE, DAGGER_ID,
        DARKLING_A0, EXPLODER_A0, FUNGI_BEAST_A0, GIANT_HEAD_A0, GIANT_HEAD_ID, GREMLIN_NOB_A0,
        GREMLIN_THIEF_A0, GREMLIN_TSUNDERE_A0, GREMLIN_WARRIOR_A0, GREMLIN_WIZARD_A0, GUARDIAN_A0,
        GUARDIAN_DEFENSIVE_BLOCK, HEALER_A0, HEXAGHOST_A0, JAW_WORM_A0, LAGAVULIN_A0, LOOTER_A0,
        LOOTER_ID, MAW_A0, MAW_ID, MUGGER_A0, MUGGER_ID, NEMESIS_A0, NEMESIS_ID, SENTRY_A0,
        SHELLED_PARASITE_A0, SHELLED_PARASITE_ID, SLIME_BOSS_A0, SPHERIC_GUARDIAN_A0,
        SPHERIC_GUARDIAN_ID, SPIKE_SLIME_A0, SPIRE_GROWTH_A0, SPIRE_GROWTH_ID, TIME_EATER_A0,
        TRANSIENT_A0,
    };
    use crate::{CardId, CardInstance, MonsterIntent, Relic};

    #[test]
    fn explicit_medium_spike_slime_profile_wins_over_split_hp_threshold() {
        let mut monster = monster_state_for_ascension(&SPIKE_SLIME_A0, MonsterId::new(1), 0);
        monster.hp = 35;
        monster.max_hp = 35;
        monster.slime_size = Some(crate::combat::SlimeSize::Medium);
        let mut monster_rng = StsRng::new(0);

        prepare_rolled_next_intent(
            &mut monster,
            &mut monster_rng,
            0,
            0,
            RolledIntentContext {
                ascension: 0,
                player_hp: 100,
                player_constricted: false,
                living_monster_count: 1,
                alive_gremlin_count: 0,
                collector_minion_dead: false,
                missing_hp: 0,
            },
        )
        .expect("medium Spike Slime intent is supported");

        assert_eq!(
            monster.intent,
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: 8,
                count: 1,
            }
        );
    }

    #[test]
    fn regeneration_resolves_after_regret_end_turn_damage() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 90;
        state.player.max_hp = 100;
        state.player.powers.regen = 4;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), REGRET_ID)];

        crate::combat::turn_powers::apply_end_of_player_turn_powers_before_hand(&mut state)
            .expect("end-turn powers resolve");
        resolve_end_of_turn_hand(&mut state).expect("Regret resolves");
        crate::combat::turn_powers::apply_end_of_player_turn_regeneration(&mut state)
            .expect("Regeneration resolves");

        assert_eq!(state.player.hp, 93);
        assert_eq!(state.player.powers.regen, 3);
    }

    #[test]
    fn orichalcum_checks_zero_block_before_metallicize_resolves() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 80;
        state.player.block = 0;
        state.player.powers.metallicize = 3;
        state.relics = vec![Relic::Orichalcum];
        state.monsters = vec![monster_state_for_ascension(
            &GREMLIN_NOB_A0,
            MonsterId::new(1),
            state.ascension,
        )];
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 36 };

        let next = end_player_turn(&state).expect("supported monster intent");

        // Orichalcum (6) and Metallicize (3) both block the 36-damage hit.
        assert_eq!(next.player.hp, 53);
    }

    #[test]
    fn interrupted_burning_pact_card_enters_discard_after_visible_hand_cleanup() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
        ];
        state.piles.draw_pile = (3..=8)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();
        state.pending_hidden_hand_card_until_end_turn =
            vec![CardInstance::new(CardId::new(9), STRIKE_R_ID)];

        let next = end_player_turn(&state).expect("supported monster intent");

        assert_eq!(
            next.piles
                .discard_pile
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(2), CardId::new(1), CardId::new(9)]
        );
        assert!(next.pending_hidden_hand_card_until_end_turn.is_empty());
    }

    #[test]
    fn empty_hand_end_holds_pending_hidden_card_out_of_shuffle() {
        // Burning Pact deferred exhaust selection parks the stuck selected card
        // in pending_hidden. When the rest of the hand is spent before END,
        // empty-hand DiscardAction does not settle leftover selectedCards into
        // discard — the card must not join the next draw shuffle.
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(1), BURNING_PACT_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
        ];
        state.pending_hidden_hand_card_until_end_turn =
            vec![CardInstance::new(CardId::new(4), THUNDERCLAP_ID)];

        let next = end_player_turn(&state).expect("supported monster intent");

        assert_eq!(
            next.pending_hidden_hand_card_until_end_turn
                .first()
                .expect("empty-hand END holds pending card")
                .content_id,
            THUNDERCLAP_ID
        );
        assert!(next
            .piles
            .hand
            .iter()
            .chain(next.piles.draw_pile.iter())
            .chain(next.piles.discard_pile.iter())
            .all(|card| card.content_id != THUNDERCLAP_ID));
        assert_eq!(next.piles.hand.len(), 3);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == BURNING_PACT_ID));
    }

    #[test]
    fn runic_pyramid_end_holds_pending_hidden_cards() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::RunicPyramid];
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        state.piles.discard_pile.clear();
        state.pending_hidden_hand_card_until_end_turn = vec![
            CardInstance::new(CardId::new(2), INFLAME_ID),
            CardInstance::new(CardId::new(3), BURNING_PACT_ID),
        ];

        let next = end_player_turn(&state).expect("supported monster intent");

        assert_eq!(
            next.pending_hidden_hand_card_until_end_turn
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![INFLAME_ID, BURNING_PACT_ID]
        );
        assert!(next
            .piles
            .discard_pile
            .iter()
            .all(|card| card.content_id != INFLAME_ID && card.content_id != BURNING_PACT_ID));
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == STRIKE_R_ID && card.id == CardId::new(1)));
    }

    #[test]
    fn ethereal_only_hand_still_settles_pending_hidden_into_discard() {
        // Warcry put-on-deck skipped retrieval leaves the selected card in
        // limbo while END still sees a non-empty ethereal hand (FIDL00278).
        // Ethereal exhaust must not reclassify the END as empty-hand and keep
        // the limbo card out of the discard→draw shuffle.
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(1), DAZED_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
            CardInstance::new(CardId::new(5), STRIKE_R_ID),
            CardInstance::new(CardId::new(6), STRIKE_R_ID),
        ];
        state.pending_hidden_hand_card_until_end_turn =
            vec![CardInstance::new(CardId::new(7), INFLAME_ID)];

        let next = end_player_turn(&state).expect("supported monster intent");

        assert!(next.pending_hidden_hand_card_until_end_turn.is_empty());
        assert!(next
            .piles
            .hand
            .iter()
            .chain(next.piles.draw_pile.iter())
            .chain(next.piles.discard_pile.iter())
            .any(|card| card.content_id == INFLAME_ID));
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == DAZED_ID));
    }

    #[test]
    fn elixir_exhaust_cards_remain_exhausted_across_subsequent_turns() {
        use crate::combat::transition::{
            choose_exhaust_select, confirm_exhaust_select, open_exhaust_select,
        };

        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
        ];
        state.piles.draw_pile = (10..=20)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        open_exhaust_select(&mut state).expect("open elixir exhaust select");
        choose_exhaust_select(&mut state, 0).expect("select first card");
        choose_exhaust_select(&mut state, 0).expect("select second visible card");
        confirm_exhaust_select(&mut state).expect("confirm elixir exhaust");

        assert!(state.pending_elixir_exhaust_card_ids.is_empty());
        assert_eq!(state.pending_elixir_exhaust_turns_remaining, 0);
        assert_eq!(
            state
                .piles
                .exhaust_pile
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(1), CardId::new(2)]
        );

        let after_elixir_turn = end_player_turn(&state).expect("end elixir turn");
        assert!(after_elixir_turn
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(1)));
        assert!(after_elixir_turn
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(2)));
        assert!(!after_elixir_turn
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(1) || card.id == CardId::new(2)));

        let after_subsequent_turn =
            end_player_turn(&after_elixir_turn).expect("end subsequent turn");
        assert!(after_subsequent_turn
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(1)));
        assert!(after_subsequent_turn
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(2)));
        assert!(!after_subsequent_turn
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(1) || card.id == CardId::new(2)));
    }

    #[test]
    fn dead_branch_ethereal_exhaust_enters_next_hand_before_draw() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::DeadBranch];
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), DAZED_ID),
            CardInstance::new(CardId::new(2), DAZED_ID),
        ];
        state.piles.draw_pile = (3..=7)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = end_player_turn(&state).expect("Dead Branch end turn resolves");

        assert_eq!(next.piles.hand.len(), 7);
        assert!(next.piles.hand[0].combat_only);
        assert!(next.piles.hand[1].combat_only);
        assert_ne!(next.piles.hand[0].id, next.piles.hand[1].id);
        assert_eq!(
            next.piles.hand[2..]
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![STRIKE_R_ID; 5]
        );
        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .filter(|card| card.content_id == DAZED_ID)
                .count(),
            2
        );
    }

    #[test]
    fn end_player_turn_rejects_ritual_strength_overflow() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.strength = i32::MAX;
        state.player.powers.ritual = 1;
        state.validate().expect("input combat is valid");

        assert_eq!(
            end_player_turn(&state),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
    }

    #[test]
    fn evolve_does_not_draw_for_curses() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.evolve = 1;
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), SHAME_ID),
        ];
        state.piles.discard_pile.clear();

        draw_next_hand_without_shuffle(&mut state).expect("draw with Evolve");

        assert_eq!(
            state
                .piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![SHAME_ID, STRIKE_R_ID]
        );
        assert!(state.piles.draw_pile.is_empty());
    }

    #[test]
    fn turn_draw_stops_base_draws_after_evolve_fills_hand() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.evolve = 1;
        state.piles.hand.clear();
        state.piles.draw_pile = (1..=11)
            .map(|id| CardInstance::new(CardId::new(id), SLIMED_ID))
            .collect();

        draw_next_hand_without_shuffle(&mut state).expect("turn draw with Evolve");

        assert_eq!(state.piles.hand.len(), MAX_HAND_SIZE);
        assert_eq!(state.piles.draw_pile.len(), 1);
    }

    #[test]
    fn turn_draw_defers_evolve_until_after_base_hand_refill() {
        // Draw pile top is the last element. Base DrawCardAction draws the five
        // non-evolve slots first; EvolvePower.addToBot follow-ups append after.
        let mut state = CombatState::initial_fixture();
        state.player.powers.evolve = 1;
        state.piles.hand.clear();
        state.piles.discard_pile.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), DEFEND_R_ID), // bottom
            CardInstance::new(CardId::new(2), BASH_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
            CardInstance::new(CardId::new(5), DAZED_ID),
            CardInstance::new(CardId::new(6), WOUND_ID), // top: drawn first
        ];

        draw_next_hand_without_shuffle(&mut state).expect("turn draw with deferred Evolve");

        assert_eq!(
            state
                .piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![
                WOUND_ID,
                DAZED_ID,
                STRIKE_R_ID,
                STRIKE_R_ID,
                BASH_ID,
                DEFEND_R_ID, // Evolve follow-up after the five base draws
            ]
        );
        assert!(state.piles.draw_pile.is_empty());
    }

    #[test]
    fn brutality_draw_runs_after_base_draw_empty_shuffle() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.brutality = 1;
        state.piles.hand.clear();
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        state.piles.discard_pile.clear();

        start_player_turn(&mut state).expect("start turn with post-draw Brutality");

        assert_eq!(state.rng.shuffle_rng.counter(), 1);
        assert_eq!(state.piles.hand[0].content_id, STRIKE_R_ID);
    }

    #[test]
    fn evolve_follow_up_draw_respects_hand_capacity() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.evolve = 2;
        state.piles.hand = (1..=8)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(9), STRIKE_R_ID),
            CardInstance::new(CardId::new(10), STRIKE_R_ID),
            CardInstance::new(CardId::new(11), DAZED_ID),
        ];
        state.piles.discard_pile.clear();

        crate::combat::draw::draw_cards_with_combat_rng(&mut state, 2)
            .expect("draw with Evolve follow-ups");

        assert_eq!(state.piles.hand.len(), MAX_HAND_SIZE);
        assert_eq!(state.piles.draw_pile.len(), 1);
        assert_eq!(state.piles.draw_pile[0].id, CardId::new(9));
    }

    #[test]
    fn confusion_preserves_x_cost_cards_without_consuming_card_rng() {
        use crate::content::cards::{STRIKE_R_ID, WHIRLWIND_ID};

        let mut state = CombatState::initial_fixture();
        state.player.powers.confusion = 1;
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), WHIRLWIND_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
        ];
        state.rng.card_random_rng = StsRng::new(3);

        crate::combat::draw::draw_cards_with_combat_rng(&mut state, 2)
            .expect("draw under Confusion");

        assert_eq!(state.piles.hand[0].temp_cost, Some(0));
        assert_eq!(state.piles.hand[1].temp_cost, None);
        assert_eq!(state.rng.card_random_rng.counter(), 1);
    }

    #[test]
    fn end_turn_doubt_weak_overflow_fails_without_mutating_input() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.weak = i32::MAX;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(100), DOUBT_ID),
            CardInstance::new(CardId::new(101), DOUBT_ID),
        ];
        let before = state.clone();

        assert_eq!(
            end_player_turn(&state),
            Err(SimError::InvalidState(
                "player Weak application overflows i32"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn end_player_turn_rejects_temporary_strength_restore_overflow() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.strength = i32::MAX;
        state.player.powers.artifact = 1;
        state.player.temp_strength = 1;
        state.validate().expect("input combat is valid");

        assert_eq!(
            end_player_turn(&state),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
    }

    #[test]
    fn pending_effects_reject_weak_overflow_without_mutating_input() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.weak = i32::MAX;
        let before = state.clone();

        assert_eq!(
            apply_monster_pending_effects(
                &mut state,
                crate::MonsterIntent::Stun,
                0,
                1,
                0,
                None,
                0,
                0,
                1,
                0,
                false,
                0,
                None,
            ),
            Err(SimError::InvalidState(
                "player Weak application overflows i32"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn capped_monster_healing_avoids_intermediate_overflow() {
        let mut monster = CombatState::initial_fixture().monsters.remove(0);
        monster.max_hp = i32::MAX;
        monster.hp = i32::MAX - 1;

        heal_monster_to_stored_cap(&mut monster, i32::MAX)
            .expect("capped healing remains representable");

        assert_eq!(monster.hp, i32::MAX);
    }

    #[test]
    fn end_player_turn_rejects_healing_thorns_overflow_without_mutating_input() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.thorns = i32::MAX;
        state.player.temp_thorns = 1;
        state.monsters[0].intent = crate::MonsterIntent::AttackHealSelf { damage: 1 };
        state.monsters[0].initial_intent_locked = true;
        let before = state.clone();

        assert_eq!(
            end_player_turn(&state),
            Err(SimError::InvalidState("monster intent arithmetic overflow"))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn end_player_turn_reserves_stasis_ids_for_generated_statuses() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].intent = crate::MonsterIntent::AddDazedToDiscard { count: 1 };
        state.monsters[0].stasis_card = Some(CardInstance::new(
            CardId::new(i64::MAX as u64),
            POMMEL_STRIKE_ID,
        ));
        let before = state.clone();

        assert_eq!(
            end_player_turn(&state),
            Err(SimError::InvalidState(
                "generated combat card ID exceeds the target signed range"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn end_player_turn_rejects_summon_intent_for_incompatible_content() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].intent = crate::MonsterIntent::SummonGremlins { count: 2 };
        state.monsters[0].initial_intent_locked = true;
        let before = state.clone();

        assert_eq!(
            end_player_turn(&state),
            Err(SimError::InvalidState(
                "summon intent is incompatible with monster content"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn end_player_turn_rejects_special_move_counter_overflow_without_mutating_input() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].intent = crate::MonsterIntent::StrengthAllMonsters { amount: 1 };
        state.monsters[0].moves_executed = u32::MAX;
        let before = state.clone();

        assert_eq!(
            end_player_turn(&state),
            Err(SimError::InvalidState("combat turn counter overflows u32"))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn end_player_turn_rejects_monster_cleanup_overflow_without_mutating_input() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 0 };
        state.monsters[0].powers.strength = i32::MAX;
        state.monsters[0].powers.ritual = 1;
        let before = state.clone();

        assert_eq!(
            end_player_turn(&state),
            Err(SimError::InvalidState(
                "monster end-turn arithmetic overflow"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn revival_cleanup_overflow_is_transactional() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].powers.strength = i32::MAX;
        state.monsters[0].powers.ritual = 1;
        let before = state.clone();

        assert_eq!(
            finish_monster_turn_after_player_revival(&mut state),
            Err(SimError::InvalidState(
                "monster end-turn arithmetic overflow"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn self_forming_clay_juggernaut_waits_behind_confused_hand_draw() {
        // SelfFormingClay.atTurnStart addToBots GainBlock; DrawCardAction is
        // already queued, so Juggernaut's cardRandom target roll must not
        // steal the first Confusion cost (FIDL02206).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.player.powers.confusion = 1;
        state.player.powers.juggernaut = 5;
        state.relics = vec![Relic::SelfFormingClay];
        state.relic_counters.self_forming_clay_next_turn_block = 3;
        state.piles.hand.clear();
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(10), STRIKE_R_ID)];
        state.piles.discard_pile.clear();
        state.rng.card_random_rng = StsRng::new(1);

        let without_juggernaut_first = {
            let mut preview = state.clone();
            preview.player.powers.juggernaut = 0;
            preview.relic_counters.self_forming_clay_next_turn_block = 3;
            start_player_turn(&mut preview).expect("draw without Juggernaut");
            preview.piles.hand[0].temp_cost
        };

        start_player_turn(&mut state).expect("Clay then draw then Juggernaut");
        assert_eq!(state.player.block, 3);
        assert_eq!(
            state.piles.hand[0].temp_cost, without_juggernaut_first,
            "Confusion must roll before Clay Juggernaut chooses a target"
        );
    }

    #[test]
    fn juggernaut_triggers_for_orichalcum_metallicize_and_self_forming_clay() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 92;
        state.player.max_hp = 92;
        state.player.block = 0;
        state.player.powers.juggernaut = 7;
        state.player.powers.metallicize = 3;
        state.relics = vec![Relic::Orichalcum, Relic::SelfFormingClay];
        state.monsters = vec![monster_state_for_ascension(
            &GIANT_HEAD_A0,
            MonsterId::new(1),
            state.ascension,
        )];
        let monster = &mut state.monsters[0];
        monster.hp = 100;
        monster.max_hp = 100;
        monster.intent = crate::MonsterIntent::Attack { damage: 13 };
        monster.powers.strength = 1;
        monster.powers.weak = 0;

        let next = end_player_turn(&state).expect("supported monster intent");

        assert_eq!(next.player.hp, 87);
        assert_eq!(next.player.block, 3);
        assert_eq!(next.monsters[0].hp, 79);
    }

    #[test]
    fn lethal_hexaghost_inferno_does_not_upgrade_or_add_burns() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 1;
        state.player.block = 0;
        state.piles.hand.clear();
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(1), BURN_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), BURN_ID)];
        state.monsters = vec![monster_state_for_ascension(
            &HEXAGHOST_A0,
            MonsterId::new(1),
            state.ascension,
        )];
        state.monsters[0].intent = crate::MonsterIntent::AttackMultipleUpgradeBurns {
            damage: 2,
            hits: 6,
            count: 3,
        };

        run_monster_turn(&mut state).expect("supported monster intent");

        assert_eq!(state.player.hp, 0);
        assert_eq!(state.piles.discard_pile.len(), 1);
        assert_eq!(state.piles.draw_pile.len(), 1);
        assert_eq!(state.piles.discard_pile[0].upgrades, 0);
        assert_eq!(state.piles.draw_pile[0].upgrades, 0);
        assert!(!state.monsters[0].burns_upgraded);
    }

    #[test]
    fn surviving_hexaghost_inferno_upgrades_and_adds_burns() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 80;
        state.player.block = 0;
        state.piles.hand.clear();
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(1), BURN_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), BURN_ID)];
        state.monsters = vec![monster_state_for_ascension(
            &HEXAGHOST_A0,
            MonsterId::new(1),
            state.ascension,
        )];
        state.monsters[0].intent = crate::MonsterIntent::AttackMultipleUpgradeBurns {
            damage: 2,
            hits: 6,
            count: 3,
        };

        run_monster_turn(&mut state).expect("supported monster intent");

        assert_eq!(state.player.hp, 68);
        assert_eq!(state.piles.discard_pile.len(), 4);
        assert_eq!(state.piles.draw_pile.len(), 1);
        assert!(state
            .piles
            .discard_pile
            .iter()
            .chain(state.piles.draw_pile.iter())
            .all(|card| card.content_id == BURN_ID && card.upgrades == 1));
        assert!(state.monsters[0].burns_upgraded);

        let previous_discard_len = state.piles.discard_pile.len();
        state.monsters[0].intent = crate::MonsterIntent::AddBurnToDiscard {
            damage: 6,
            count: 1,
        };
        run_monster_turn(&mut state).expect("post-Inferno Sear is supported");

        assert_eq!(state.piles.discard_pile.len(), previous_discard_len + 1);
        assert_eq!(
            state.piles.discard_pile.last().map(|card| card.upgrades),
            Some(1)
        );
    }

    #[test]
    fn combust_mode_shift_block_survives_monster_pre_turn_clear() {
        // Combust depleting Mode Shift must leave Guardian with 20 block after
        // Close Up — GainBlock is after monster loseBlock in the target queue.
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state_for_ascension(
            &GUARDIAN_A0,
            MonsterId::new(1),
            state.ascension,
        )];
        state.monsters[0].mode_shift = 4;
        state.monsters[0].block = 1;
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 32 };
        state.monsters[0].moves_executed = 1;
        state.player.powers.combust = 1;
        state.player.powers.combust_damage = 5;
        state.player.hp = 200;
        state.player.block = 50;
        state.piles.hand.clear();
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();

        let next = end_player_turn(&state).expect("end turn resolves");

        assert!(next.monsters[0].in_defensive_mode);
        assert_eq!(next.monsters[0].block, GUARDIAN_DEFENSIVE_BLOCK);
        assert_eq!(next.monsters[0].powers.spikes, 3);
        // Close Up consumed one defensive turn; next intent is Roll Attack.
        assert_eq!(next.monsters[0].defensive_turns_remaining, 2);
        assert!(matches!(
            next.monsters[0].intent,
            crate::MonsterIntent::Attack { damage: 9 }
        ));
    }

    #[test]
    fn combust_kill_returns_stasis_card_after_old_hand_is_discarded() {
        let mut state = CombatState::initial_fixture();
        let mut automaton =
            monster_state_for_ascension(&BRONZE_AUTOMATON_A0, MonsterId::new(1), state.ascension);
        automaton.intent = crate::MonsterIntent::Stun;
        let mut orb =
            monster_state_for_ascension(&BRONZE_ORB_A0, MonsterId::new(2), state.ascension);
        orb.hp = 5;
        orb.max_hp = 5;
        orb.intent = crate::MonsterIntent::Stun;
        orb.stasis_card = Some(CardInstance::new(CardId::new(50), POMMEL_STRIKE_ID));
        state.monsters = vec![automaton, orb];
        state.player.powers.combust = 1;
        state.player.powers.combust_damage = 5;
        state.piles.hand = (1..=5)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.draw_pile.clear();
        state.piles.discard_pile = (10..=19)
            .map(|id| CardInstance::new(CardId::new(id), SLIMED_ID))
            .collect();
        state.rng.shuffle_rng = StsRng::new(123);

        let next = end_player_turn(&state).expect("supported monster intent");

        assert!(!next.monsters[1].alive);
        assert_eq!(next.piles.hand.len(), 6);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(50)));
    }

    #[test]
    fn charons_ashes_ethereal_kill_returns_stasis_after_hand_discard() {
        use crate::relic::Relic;

        let mut state = CombatState::initial_fixture();
        let mut automaton =
            monster_state_for_ascension(&BRONZE_AUTOMATON_A0, MonsterId::new(1), state.ascension);
        automaton.intent = crate::MonsterIntent::Stun;
        let mut orb =
            monster_state_for_ascension(&BRONZE_ORB_A0, MonsterId::new(2), state.ascension);
        orb.hp = 3;
        orb.max_hp = 5;
        orb.intent = crate::MonsterIntent::Stun;
        orb.stasis_card = Some(CardInstance::new(CardId::new(50), BERSERK_ID));
        state.monsters = vec![automaton, orb];
        state.relics = vec![Relic::CharonsAshes];
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), DAZED_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
        ];
        state.piles.draw_pile = (10..=14)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();

        let next = end_player_turn(&state).expect("end turn resolves");

        assert!(!next.monsters[1].alive);
        assert!(
            next.piles
                .hand
                .iter()
                .any(|card| card.id == CardId::new(50)),
            "Stasis Berserk must return to the next hand, not the discarded pile"
        );
        assert!(next
            .piles
            .discard_pile
            .iter()
            .all(|card| card.id != CardId::new(50)));
    }

    #[test]
    fn combust_death_callbacks_interleave_stasis_and_horn_after_hand_discard() {
        let mut state = CombatState::initial_fixture();
        let mut automaton = monster_state_for_ascension(&BRONZE_AUTOMATON_A0, MonsterId::new(1), 0);
        automaton.intent = MonsterIntent::Stun;
        let mut first_orb = monster_state_for_ascension(&BRONZE_ORB_A0, MonsterId::new(2), 0);
        first_orb.hp = 5;
        first_orb.max_hp = 5;
        first_orb.intent = MonsterIntent::Stun;
        first_orb.stasis_card = Some(CardInstance::new(CardId::new(50), POMMEL_STRIKE_ID));
        let mut second_orb = monster_state_for_ascension(&BRONZE_ORB_A0, MonsterId::new(3), 0);
        second_orb.hp = 5;
        second_orb.max_hp = 5;
        second_orb.intent = MonsterIntent::Stun;
        second_orb.stasis_card = Some(CardInstance::new(CardId::new(51), BASH_ID));
        state.monsters = vec![automaton, first_orb, second_orb];
        state.player.powers.combust = 1;
        state.player.powers.combust_damage = 5;
        state.relics = vec![Relic::GremlinHorn];
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(100), STRIKE_R_ID),
            CardInstance::new(CardId::new(101), STRIKE_R_ID),
            CardInstance::new(CardId::new(102), STRIKE_R_ID),
            CardInstance::new(CardId::new(103), STRIKE_R_ID),
            CardInstance::new(CardId::new(104), STRIKE_R_ID),
            CardInstance::new(CardId::new(10), BASH_ID),
            CardInstance::new(CardId::new(11), DEFEND_R_ID),
        ];
        state.piles.discard_pile.clear();

        let next = end_player_turn(&state).expect("end turn resolves deferred death callbacks");

        assert!(!next.monsters[1].alive);
        assert!(!next.monsters[2].alive);
        assert_eq!(next.piles.hand.len(), 9);
        assert_eq!(
            next.piles
                .hand
                .iter()
                .take(4)
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![POMMEL_STRIKE_ID, DEFEND_R_ID, BASH_ID, BASH_ID]
        );
        assert!(next.piles.draw_pile.is_empty());
    }

    #[test]
    fn combust_lethal_applies_constricted_before_combust_lose_hp() {
        // Constricted is older than Combust on the power list, so it resolves
        // before Combust LoseHP / damage even when the hit is lethal (FIDL00440
        // with Orichalcum). No Orichalcum here: full Constricted 10 + Combust 5.
        let mut state = CombatState::initial_fixture();
        state.player.hp = 9277;
        state.player.max_hp = 10000;
        state.player.powers.combust = 5;
        state.player.powers.combust_damage = 25;
        state.player.powers.constricted = 10;
        state.monsters.truncate(1);
        state.monsters[0].hp = 3;
        state.monsters[0].max_hp = 150;
        state.monsters[0].alive = true;
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 16 };
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        state.relics.clear(); // no Burning Blood

        let next = end_player_turn(&state).expect("combust lethal end turn");
        assert!(!next.monsters[0].alive);
        assert_eq!(next.phase, CombatPhase::Won);
        assert_eq!(next.player.hp, 9277 - 10 - 5, "hp={}", next.player.hp);
    }

    #[test]
    fn combust_lethal_with_orichalcum_block_absorbs_part_of_constricted() {
        // FIDL00440: Orichalcum 6 → Constricted 10 (4 HP) → Combust stacks 2 (−2).
        let mut state = CombatState::initial_fixture();
        state.player.hp = 9059;
        state.player.max_hp = 10000;
        state.player.block = 0;
        state.player.powers.combust = 2;
        state.player.powers.combust_damage = 12;
        state.player.powers.constricted = 10;
        state.relics = vec![Relic::Orichalcum];
        state.monsters.truncate(1);
        state.monsters[0].hp = 8;
        state.monsters[0].max_hp = 150;
        state.monsters[0].alive = true;
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 16 };
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];

        let next = end_player_turn(&state).expect("combust lethal with orichalcum");
        assert!(!next.monsters[0].alive);
        assert_eq!(next.phase, CombatPhase::Won);
        assert_eq!(next.player.hp, 9053, "hp={}", next.player.hp);
        assert_eq!(next.player.block, 0);
    }

    #[test]
    fn bomb_lethal_applies_constricted_before_burning_blood() {
        // FIDL00403: The Bomb's final tick kills during end-turn powers-before-hand
        // (skips Regret/hand). Constricted still consumes residual block as THORNS
        // before Burning Blood: 3876 block5 / Constricted 10 → hp 3871, BB +6 → 3877.
        let mut state = CombatState::initial_fixture();
        state.player.hp = 3876;
        state.player.max_hp = 10000;
        state.player.block = 5;
        state.player.powers.constricted = 10;
        state.bomb_timers = vec![crate::combat::state::BombTimer {
            turns_remaining: 1,
            damage: 40,
        }];
        state.monsters.truncate(1);
        state.monsters[0].hp = 13;
        state.monsters[0].max_hp = 170;
        state.monsters[0].alive = true;
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 16 };
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        state.relics = vec![
            Relic::BurningBlood,
            Relic::GoldenIdol,
            Relic::CallingBell,
            Relic::Akabeko,
            Relic::PeacePipe,
            Relic::PaperPhrog,
            Relic::BagOfPreparation,
            Relic::PotionBelt,
            Relic::FrozenEgg,
        ];
        state.player.powers.brutality = 1;
        state.player.powers.strength = 2;

        let next = end_player_turn(&state).expect("bomb lethal end turn");
        assert!(!next.monsters[0].alive);
        assert_eq!(next.phase, CombatPhase::Won);
        assert_eq!(next.player.hp, 3877, "hp={}", next.player.hp);
        assert_eq!(next.player.block, 0);
    }

    #[test]
    fn pending_bomb_timer_does_not_claim_metallicize_juggernaut_lethal() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 40;
        state.player.max_hp = 40;
        state.player.powers.metallicize = 3;
        state.player.powers.juggernaut = 7;
        state.bomb_timers = vec![crate::combat::state::BombTimer {
            turns_remaining: 2,
            damage: 40,
        }];
        state.monsters.truncate(1);
        state.monsters[0].hp = 7;
        state.monsters[0].max_hp = 7;
        state.monsters[0].alive = true;
        state.monsters[0].intent = crate::MonsterIntent::Stun;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), BURN_ID)];
        state.relics.clear();

        let next = end_player_turn(&state)
            .expect("Metallicize/Juggernaut causes lethal with a pending Bomb");

        assert_eq!(next.phase, CombatPhase::Won);
        assert_eq!(next.player.hp, 40, "noncausal Bomb must not autoplay Burn");
        assert!(next.piles.hand.iter().any(|card| card.id == CardId::new(1)));
    }

    #[test]
    fn stone_calendar_respects_block_when_predicting_finish() {
        // FIDL00415: turn-7 Calendar deals 52 unmodified; Barricade block is
        // consumed first, so 20 HP + 60 block does not end combat early.
        let mut state = CombatState::initial_fixture();
        state.player.hp = 9529;
        state.player.max_hp = 10000;
        state.player.block = 3;
        state.relics = vec![Relic::BurningBlood, Relic::StoneCalendar];
        state.relic_counters.player_turns_started = crate::relic::STONE_CALENDAR_TURN;
        state.monsters.truncate(1);
        state.monsters[0].hp = 20;
        state.monsters[0].max_hp = 20;
        state.monsters[0].block = 60;
        state.monsters[0].alive = true;
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 10 };
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];

        let next = end_player_turn(&state).expect("calendar non-lethal end turn");
        assert!(next.monsters[0].alive, "guardian survives behind block");
        assert_eq!(
            next.monsters[0].hp, 20,
            "calendar did not pierce remaining HP"
        );
        assert_eq!(next.phase, CombatPhase::WaitingForPlayer);
        // Without Barricade, end-of-round block clear may zero block after the hit.
    }

    #[test]
    fn fairy_with_magic_flower_revives_before_later_monsters_act() {
        let mut state = CombatState::initial_fixture();
        let mut automaton =
            monster_state_for_ascension(&BRONZE_AUTOMATON_A0, MonsterId::new(1), state.ascension);
        automaton.intent = crate::MonsterIntent::Attack { damage: 45 };
        automaton.powers.strength = 6;
        let mut orb =
            monster_state_for_ascension(&BRONZE_ORB_A0, MonsterId::new(2), state.ascension);
        orb.intent = crate::MonsterIntent::Block { block: 12 };
        state.monsters = vec![automaton, orb];
        state.player.hp = 16;
        state.player.max_hp = 118;
        state.relics.push(Relic::MagicFlower);
        state.relic_counters.fairy_heal_percent = 30;

        let next = end_player_turn(&state).expect("supported monster intent");

        assert_eq!(next.player.hp, 53);
        assert_eq!(next.monsters[0].block, 12);
        assert!(next.relic_counters.fairy_consumed);
        assert_eq!(next.phase, CombatPhase::WaitingForPlayer);
    }

    #[test]
    fn revival_hp_handles_the_target_hp_limit_without_overflow() {
        assert_eq!(
            revival_hp(i32::MAX, crate::relic::LIZARD_TAIL_HEAL_PERCENT),
            Ok(1_073_741_823)
        );
        assert_eq!(revival_hp(i32::MAX, 100), Ok(i32::MAX));
        assert_eq!(
            revival_hp_with_relics(i32::MAX, 100, &[Relic::MagicFlower]),
            Ok(i32::MAX)
        );
        assert_eq!(
            revival_hp_with_relics(118, 30, &[Relic::MagicFlower]),
            Ok(53)
        );
    }

    #[test]
    fn malformed_fairy_percentage_fails_without_consuming_revival() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 0;
        state.relic_counters.fairy_heal_percent = 101;
        let before = state.clone();

        assert_eq!(
            revive_player_if_available(&mut state),
            Err(SimError::InvalidState(
                "combat revival HP inputs are outside the target domain"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn lethal_combust_fairy_revival_finishes_end_turn_before_drawing() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 1;
        state.player.max_hp = 114;
        state.player.powers.combust = 1;
        state.player.powers.combust_damage = 5;
        state.relic_counters.fairy_heal_percent = 30;
        state.monsters[0].intent = crate::MonsterIntent::Stun;
        let monster_hp = state.monsters[0].hp;
        state.piles.hand = (1..=5)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.draw_pile = (6..=10)
            .map(|id| CardInstance::new(CardId::new(id), SLIMED_ID))
            .collect();
        state.piles.discard_pile.clear();

        let next = end_player_turn(&state).expect("supported monster intent");

        assert_eq!(next.player.hp, 34);
        assert_eq!(next.monsters[0].hp, monster_hp - 5);
        assert!(next.relic_counters.fairy_consumed);
        assert_eq!(next.phase, CombatPhase::WaitingForPlayer);
        assert_eq!(next.piles.hand.len(), 5);
        assert!(next
            .piles
            .hand
            .iter()
            .all(|card| card.content_id == SLIMED_ID));
        assert_eq!(next.piles.discard_pile.len(), 5);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .all(|card| card.content_id == STRIKE_R_ID));
    }

    #[test]
    fn multi_hit_thorns_keeps_first_damage_hit_when_attacker_dies() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 5;
        state.player.block = 0;
        state.player.powers.thorns = 3;
        state.monsters = vec![monster_state_for_ascension(
            &GREMLIN_NOB_A0,
            MonsterId::new(1),
            state.ascension,
        )];
        state.monsters[0].hp = 3;
        state.monsters[0].intent = crate::MonsterIntent::AttackMultiple { damage: 4, hits: 6 };

        run_monster_turn(&mut state).expect("supported monster intent");

        assert_eq!(state.player.hp, 1);
        assert!(!state.monsters[0].alive);
        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::AttackMultiple { damage: 4, hits: 1 }
        );
    }

    #[test]
    fn slime_boss_queues_split_after_reactive_thorns_damage() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.thorns = 3;
        let mut boss =
            monster_state_for_ascension(&SLIME_BOSS_A0, MonsterId::new(1), state.ascension);
        boss.hp = 72;
        boss.intent = crate::MonsterIntent::Attack { damage: 35 };
        boss.moves_executed = 5;
        state.monsters = vec![boss];
        state.rng.monster_rng = StsRng::new(99);
        let expected_counter = state.rng.monster_rng.counter();

        run_monster_turn(&mut state).expect("supported Slime Boss attack");

        assert_eq!(state.monsters[0].hp, 69);
        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::SummonGremlins { count: 2 }
        );
        assert!(state.monsters[0].split_triggered);
        // SlimeBoss never queues RollMoveAction; thorns-forced SPLIT must not
        // pull a common AI roll or child opening intents desync.
        assert_eq!(state.rng.monster_rng.counter(), expected_counter);
    }

    #[test]
    fn slime_boss_thorns_forced_split_child_rolls_skip_boss_ai_draw() {
        // After a thorns-forced SPLIT intent, the next SPLIT takeTurn rolls
        // Spike then Acid opening moves with no intervening boss AI draw.
        let boss_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.player.powers.thorns = 3;
        let mut boss = monster_state_for_ascension(&SLIME_BOSS_A0, boss_id, state.ascension);
        boss.hp = 72;
        boss.intent = crate::MonsterIntent::Attack { damage: 35 };
        boss.moves_executed = 5;
        state.monsters = vec![boss];
        state.rng.monster_rng = StsRng::new(12345);

        run_monster_turn(&mut state).expect("Slam + thorns force SPLIT intent");
        assert!(state.monsters[0].split_triggered);
        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::SummonGremlins { count: 2 }
        );

        let mut expected_rng = state.rng.monster_rng.clone();
        let spike_roll = expected_rng.random_int(99);
        let spike_intent =
            crate::content::monsters::target_medium_or_large_spike_slime_next_intent_from_roll_with_profile(
                true, &[], spike_roll, 0,
            );
        let acid_roll = expected_rng.random_int(99);
        let acid_intent = crate::content::monsters::target_large_acid_slime_next_intent_from_roll(
            &[],
            acid_roll,
            &mut expected_rng,
            0,
        );

        run_monster_turn(&mut state).expect("Slime Boss executes SPLIT");

        let living: Vec<_> = state
            .monsters
            .iter()
            .filter(|monster| monster.alive)
            .collect();
        assert_eq!(living.len(), 2);
        assert_eq!(living[0].content_id, SPIKE_SLIME_ID);
        assert_eq!(living[1].content_id, ACID_SLIME_ID);
        assert_eq!(living[0].intent, spike_intent);
        assert_eq!(living[1].intent, acid_intent);
        assert_eq!(state.rng.monster_rng.counter(), expected_rng.counter());
    }

    #[test]
    fn thorns_forced_slime_split_still_consumes_queued_ai_roll() {
        // Acid Slime (L) Tackle still queues RollMoveAction. Reactive thorns can
        // force SPLIT via SplitPower before that action runs; the common AI roll
        // must still be drawn so later child spawn rolls stay on the target stream.
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.player.powers.thorns = 3;
        let mut slime = monster_state_for_ascension(&ACID_SLIME_A0, actor_id, state.ascension);
        slime.hp = 34;
        slime.max_hp = 66;
        slime.slime_size = Some(SlimeSize::Large);
        slime.rolled_attack_damage = Some(11);
        slime.intent = crate::MonsterIntent::Attack { damage: 16 };
        slime.move_history = vec![2, 4, 4, 1, 1, 2];
        state.monsters = vec![slime];
        state.rng.monster_rng = StsRng::new(42);
        let mut expected_rng = StsRng::new(42);
        let _ = expected_rng.random_int(99);

        run_monster_turn(&mut state).expect("supported large Acid Slime attack");

        assert!(state.monsters[0].alive);
        assert!(state.monsters[0].split_triggered);
        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::SummonGremlins { count: 2 }
        );
        assert_eq!(
            state.monsters[0].move_history.last().copied(),
            Some(3),
            "RollMoveAction re-asserts SPLIT into move history"
        );
        assert_eq!(state.rng.monster_rng.counter(), expected_rng.counter());
    }

    #[test]
    fn split_take_turn_does_not_draw_extra_ai_roll_after_children_spawn() {
        // The SPLIT takeTurn path does not queue RollMoveAction. After children
        // spawn the dead parent must not consume another common AI roll.
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        let mut slime = monster_state_for_ascension(&ACID_SLIME_A0, actor_id, state.ascension);
        slime.hp = 17;
        slime.max_hp = 66;
        slime.slime_size = Some(SlimeSize::Large);
        slime.rolled_attack_damage = Some(11);
        slime.split_triggered = true;
        slime.intent = crate::MonsterIntent::SummonGremlins { count: 2 };
        slime.move_history = vec![2, 4, 4, 1, 1, 2, 3];
        state.monsters = vec![slime];
        state.rng.monster_rng = StsRng::new(7);
        let mut expected_rng = StsRng::new(7);
        // Two child opening rolls only.
        let left_roll = expected_rng.random_int(99);
        let _ =
            target_medium_acid_slime_next_intent_from_roll(&[], left_roll, &mut expected_rng, 0);
        let right_roll = expected_rng.random_int(99);
        let _ =
            target_medium_acid_slime_next_intent_from_roll(&[], right_roll, &mut expected_rng, 0);

        run_monster_turn(&mut state).expect("supported large Acid Slime split");

        let children: Vec<_> = state
            .monsters
            .iter()
            .filter(|monster| monster.alive && monster.id != actor_id)
            .collect();
        assert_eq!(children.len(), 2);
        assert_eq!(state.rng.monster_rng.counter(), expected_rng.counter());
    }

    #[test]
    fn queued_monster_roll_survives_reactive_thorns_death() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.player.powers.thorns = 3;

        let mut attacker = monster_state_for_ascension(&JAW_WORM_A0, actor_id, state.ascension);
        attacker.hp = 1;
        attacker.intent = crate::MonsterIntent::Attack { damage: 1 };
        attacker.move_history = vec![1];
        let survivor =
            monster_state_for_ascension(&JAW_WORM_A0, MonsterId::new(2), state.ascension);
        state.monsters = vec![attacker, survivor];
        state.rng.monster_rng = StsRng::new(123);

        let ascension = state.ascension;
        let relics = state.relics.clone();
        let mut skip_ritual_tick = Vec::new();
        execute_generic_monster_intent(
            &mut state,
            actor_id,
            0,
            ascension,
            &relics,
            &mut skip_ritual_tick,
        )
        .expect("supported monster intent");

        assert!(!state.monsters[0].alive);
        assert_eq!(state.rng.monster_rng.counter(), 1);
        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::StrengthAndBlock {
                strength: 3,
                block: 6,
            }
        );
    }

    #[test]
    fn awakened_one_phase_two_rolls_common_ai_value_before_move_choice() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        let mut monster = monster_state_for_ascension(
            &crate::content::monsters::AWAKENED_ONE_A0,
            actor_id,
            state.ascension,
        );
        monster.mode_shift = 1;
        monster.move_history = vec![5, 6, 8];
        monster.intent = crate::MonsterIntent::AttackMultiple {
            damage: 10,
            hits: 3,
        };
        state.monsters = vec![monster];
        state.rng.monster_rng = StsRng::new(123);

        prepare_next_intent_for_actor(&mut state, actor_id)
            .expect("Awakened One phase-two intent is supported");

        assert_eq!(state.rng.monster_rng.counter(), 1);
        assert_eq!(state.monsters[0].move_history, vec![5, 6, 8, 8]);
        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::AttackMultiple {
                damage: 10,
                hits: 3,
            }
        );
    }

    #[test]
    fn reactive_thorns_trigger_fungi_spore_cloud_death_hook() {
        let mut state = CombatState::initial_fixture();
        state.player.block = 20;
        state.player.powers.thorns = 4;
        state.player.powers.vulnerable = 1;

        let mut fungi = monster_state_for_ascension(&FUNGI_BEAST_A0, MonsterId::new(1), 0);
        fungi.hp = 3;
        fungi.intent = crate::MonsterIntent::Attack { damage: 6 };
        let mut survivor = monster_state_for_ascension(&JAW_WORM_A0, MonsterId::new(2), 0);
        survivor.intent = crate::MonsterIntent::Block { block: 0 };
        state.monsters = vec![fungi, survivor];

        run_monster_turn(&mut state).expect("supported monster intents");

        assert!(!state.monsters[0].alive);
        assert_eq!(state.player.powers.vulnerable, 2);
    }

    #[test]
    fn spore_cloud_from_persistent_thorns_survives_monster_turn_cleanup() {
        let mut state = CombatState::initial_fixture();
        state.phase = CombatPhase::MonsterTurn;
        state.player.powers.thorns = 3;

        let mut fungi = monster_state_for_ascension(&FUNGI_BEAST_A0, MonsterId::new(1), 0);
        fungi.hp = 3;
        fungi.intent = crate::MonsterIntent::Attack { damage: 6 };
        let mut survivor = monster_state_for_ascension(&JAW_WORM_A0, MonsterId::new(2), 0);
        survivor.intent = crate::MonsterIntent::Block { block: 0 };
        state.monsters = vec![fungi, survivor];

        run_monster_turn(&mut state).expect("supported monster intents");

        assert!(!state.monsters[0].alive);
        assert_eq!(
            state.player.powers.vulnerable, 2,
            "Bronze Scales death damage applies Spore Cloud after the current monster turn's duration tick"
        );
        assert!(!state.player.vulnerable_just_applied);
    }

    #[test]
    fn current_move_hits_ignore_next_intent_for_single_hit_cleanup() {
        assert_eq!(
            effective_current_move_hits(
                crate::MonsterIntent::Attack { damage: 9 },
                crate::MonsterIntent::AttackMultiple { damage: 8, hits: 2 }
            ),
            1
        );
        assert_eq!(
            effective_current_move_hits(
                crate::MonsterIntent::AttackMultiple { damage: 4, hits: 6 },
                crate::MonsterIntent::AttackMultiple { damage: 4, hits: 1 }
            ),
            1
        );
    }

    #[test]
    fn time_warp_end_player_turn_head_slam_applies_draw_reduction() {
        // Time Warp leftover PLAY/END still runs Head Slam's ApplyPowerAction.
        // Skipping that while `time_warp_end_turn` is set draws a full next
        // hand after Runic Pyramid (FIDL01566 END 1301).
        let mut state = CombatState::initial_fixture();
        state.time_warp_end_turn = true;
        state.piles.hand.clear();
        state.piles.draw_pile = (1..=6)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();
        let mut time_eater = monster_state_for_ascension(&TIME_EATER_A0, MonsterId::new(1), 0);
        time_eater.intent = MonsterIntent::Attack { damage: 26 };
        state.monsters = vec![time_eater];

        let next = end_player_turn(&state).expect("Time Warp leftover Head Slam");

        assert_eq!(next.player.powers.draw_reduction, 1);
        assert_eq!(next.piles.hand.len(), 4);
        assert!(!next.time_warp_end_turn);
    }

    #[test]
    fn end_turn_fnp_juggernaut_hits_through_panic_button_no_block() {
        let mut state = CombatState::cultist_fixture();
        state.player.powers.feel_no_pain = 4;
        state.player.powers.juggernaut = 5;
        state.player.no_block_turns = 2;
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), GHOSTLY_ARMOR_ID)];
        state.piles.draw_pile = (10..=14)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();
        let monster_hp = state.monsters[0].hp;

        let next = end_player_turn(&state).expect("ethereal Ghostly Armor exhausts");

        assert_eq!(
            next.monsters[0].hp,
            monster_hp - 5,
            "FNP block from ethereal exhaust still notifies Juggernaut"
        );
        assert_eq!(next.player.no_block_turns, 1);
    }

    #[test]
    fn magnetism_generated_card_overflows_full_hand_to_discard() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.magnetism = 1;
        state.piles.hand = (1..=10)
            .map(|id| crate::CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();
        state.monsters = vec![monster_state_for_ascension(
            &GREMLIN_NOB_A0,
            MonsterId::new(1),
            state.ascension,
        )];

        apply_start_of_turn_magnetism(&mut state).expect("Magnetism generation succeeds");

        assert_eq!(state.piles.hand.len(), 10);
        assert_eq!(state.piles.discard_pile.len(), 1);
        assert!(state.piles.discard_pile[0].combat_only);
    }

    #[test]
    fn magnetism_reserves_its_complete_id_range_before_consuming_rng() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.magnetism = 2;
        state.piles.hand[0].id = CardId::new(crate::ids::MAX_SUPPORTED_CARD_INSTANCE_ID - 1);
        let before = state.clone();

        assert_eq!(
            apply_start_of_turn_magnetism(&mut state),
            Err(SimError::InvalidState(
                "card instance ID allocation exceeds the supported domain"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn monster_added_status_can_shuffle_into_next_hand() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.rng.shuffle_rng = StsRng::new(123);
        state.monsters = vec![monster_state_for_ascension(
            &crate::content::monsters::ACID_SLIME_A0,
            MonsterId::new(1),
            state.ascension,
        )];
        state.monsters[0].intent = crate::MonsterIntent::AttackAddSlimedToDiscard {
            damage: 0,
            count: 1,
        };

        let next = end_player_turn(&state).expect("supported monster intent");

        assert_eq!(next.phase, CombatPhase::WaitingForPlayer);
        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, SLIMED_ID);
        assert!(next.piles.discard_pile.is_empty());
    }

    #[test]
    fn start_player_turn_draw_caps_at_max_hand_size() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::RunicPyramid];
        state.piles.hand = (1..=9)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), STRIKE_R_ID),
            CardInstance::new(CardId::new(11), SLIMED_ID),
        ];
        state.piles.discard_pile.clear();
        state.monsters = vec![monster_state_for_ascension(
            &GREMLIN_NOB_A0,
            MonsterId::new(1),
            state.ascension,
        )];

        start_player_turn(&mut state).expect("player turn starts");

        assert_eq!(state.piles.hand.len(), 10);
        assert_eq!(state.piles.hand[9].content_id, SLIMED_ID);
        assert_eq!(state.piles.draw_pile.len(), 1);
        assert_eq!(state.piles.draw_pile[0].content_id, STRIKE_R_ID);
    }

    #[test]
    fn time_eater_draw_reduction_expires_after_two_opening_draws() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile = (1..=6)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();
        state.player.powers.draw_reduction = 1;
        state.monsters = vec![monster_state_for_ascension(
            &GREMLIN_NOB_A0,
            MonsterId::new(1),
            state.ascension,
        )];

        start_player_turn(&mut state).expect("first reduced player turn starts");
        assert_eq!(state.piles.hand.len(), 4);
        assert_eq!(state.player.powers.draw_reduction, 1);
        assert!(state.player.powers.draw_reduction_first_draw_seen);

        state.piles.hand.clear();
        state.piles.draw_pile = (7..=12)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        start_player_turn(&mut state).expect("second reduced player turn starts");
        assert_eq!(state.piles.hand.len(), 4);
        assert_eq!(state.player.powers.draw_reduction, 0);
        assert!(!state.player.powers.draw_reduction_first_draw_seen);
    }

    #[test]
    fn time_eater_head_slam_draw_reduction_is_blocked_by_artifact() {
        // FIDL01762 step 1846: Artifact 1 eats Head Slam's DrawReductionPower,
        // so the following start-of-turn draw is still five cards.
        let mut state = CombatState::initial_fixture();
        state.player.hp = 9900;
        state.player.block = 40;
        state.player.powers.artifact = 1;
        state.relics.clear();
        state.piles.hand = (1..=5)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.draw_pile = (10..=19)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();
        let mut time_eater = monster_state_for_ascension(&TIME_EATER_A0, MonsterId::new(1), 0);
        time_eater.intent = MonsterIntent::Attack { damage: 26 };
        time_eater.powers.strength = 0;
        state.monsters = vec![time_eater];

        let next = end_player_turn(&state).expect("Head Slam with Artifact");

        assert_eq!(next.player.powers.artifact, 0);
        assert_eq!(next.player.powers.draw_reduction, 0);
        assert_eq!(
            next.piles.hand.len(),
            5,
            "Artifact-blocked Head Slam must not shrink the next hand"
        );
    }

    #[test]
    fn time_eater_ripple_artifact_blocks_vulnerable_not_weak() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.artifact = 1;
        let mut time_eater = monster_state_for_ascension(&TIME_EATER_A0, MonsterId::new(1), 0);
        time_eater.intent = MonsterIntent::AttackAndBlock {
            damage: 0,
            block: 20,
        };
        state.monsters = vec![time_eater];

        let next = end_player_turn(&state).expect("Ripple with Artifact");

        assert_eq!(next.player.powers.artifact, 0);
        assert_eq!(next.player.powers.vulnerable, 0);
        assert_eq!(next.player.powers.weak, 1);
        assert_eq!(next.monsters[0].block, 20);
    }

    #[test]
    fn awakened_one_sludge_inserts_void_after_surviving_damage() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 40;
        state.player.block = 0;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        state.piles.draw_pile = (2..=8)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();
        let mut monster = monster_state_for_ascension(
            &crate::content::monsters::AWAKENED_ONE_A0,
            MonsterId::new(1),
            0,
        );
        monster.intent = MonsterIntent::AttackAddVoidToDraw {
            damage: 18,
            count: 1,
        };
        state.monsters = vec![monster];
        let hp_before = state.player.hp;

        let next = end_player_turn(&state).expect("Sludge end turn");

        assert!(
            next.player.hp < hp_before,
            "DamageAction must resolve first"
        );
        assert!(
            next.piles
                .hand
                .iter()
                .chain(next.piles.draw_pile.iter())
                .chain(next.piles.discard_pile.iter())
                .any(|card| card.content_id == VOID_ID),
            "MakeTempCardInDrawPileAction still inserts Void after damage"
        );
    }

    #[test]
    fn start_player_turn_loses_energy_when_void_is_drawn() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), VOID_ID),
        ];
        state.piles.discard_pile.clear();

        start_player_turn(&mut state).expect("player turn starts");

        assert_eq!(state.player.energy, 2);
        assert_eq!(state.piles.hand[0].content_id, VOID_ID);
    }

    #[test]
    fn centennial_puzzle_draws_before_attack_generated_slimed_enters_discard() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::CentennialPuzzle];
        state.relic_counters.centennial_puzzle_triggers = 0;
        state.piles.hand.clear();
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        state.piles.discard_pile = (2..=9)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.rng.shuffle_rng = StsRng::new(123);
        state.monsters = vec![monster_state_for_ascension(
            &crate::content::monsters::ACID_SLIME_A0,
            MonsterId::new(1),
            state.ascension,
        )];
        state.monsters[0].intent = crate::MonsterIntent::AttackAddSlimedToDiscard {
            damage: 1,
            count: 1,
        };

        let next = end_player_turn(&state).expect("supported monster intent");

        assert_eq!(next.phase, CombatPhase::WaitingForPlayer);
        assert_eq!(next.relic_counters.centennial_puzzle_triggers, 1);
        assert_eq!(next.piles.hand.len(), 8);
        assert_eq!(next.piles.discard_pile.len(), 1);
        assert_eq!(next.piles.discard_pile[0].content_id, SLIMED_ID);
        assert!(!next
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == SLIMED_ID));
        assert!(!next
            .piles
            .draw_pile
            .iter()
            .any(|card| card.content_id == SLIMED_ID));
    }

    #[test]
    fn centennial_puzzle_draws_during_end_turn_cleanup_are_discarded() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::CentennialPuzzle];
        state.relic_counters.centennial_puzzle_triggers = 0;
        state.piles.hand = vec![CardInstance::new(
            CardId::new(1),
            crate::content::cards::REGRET_ID,
        )];
        state.piles.draw_pile = (2..=9)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();
        state.monsters = vec![monster_state_for_ascension(
            &crate::content::monsters::ACID_SLIME_A0,
            MonsterId::new(1),
            state.ascension,
        )];
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 0 };

        let next = end_player_turn(&state).expect("supported monster intent");

        assert_eq!(next.relic_counters.centennial_puzzle_triggers, 1);
        assert_eq!(next.piles.hand.len(), 5);
        assert_eq!(next.piles.draw_pile.len(), 0);
        assert_eq!(
            next.piles
                .discard_pile
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![
                CardId::new(1),
                CardId::new(7),
                CardId::new(8),
                CardId::new(9)
            ]
        );
    }

    #[test]
    fn lethal_attack_does_not_add_queued_slimed() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 5;
        state.player.block = 0;
        state.piles.hand.clear();
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();

        let mut queued_slime = monster_state_for_ascension(
            &crate::content::monsters::ACID_SLIME_A0,
            MonsterId::new(1),
            state.ascension,
        );
        queued_slime.intent = crate::MonsterIntent::AttackAddSlimedToDiscard {
            damage: 7,
            count: 1,
        };
        state.monsters = vec![queued_slime];

        let next = end_player_turn(&state).expect("supported monster intent");

        assert_eq!(next.phase, CombatPhase::Lost);
        assert_eq!(next.player.hp, 0);
        assert!(next.piles.discard_pile.is_empty());
    }

    #[test]
    fn transient_direct_set_move_does_not_consume_ai_rng_after_turn() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 4;
        state.monsters = vec![monster_state_for_ascension(
            &TRANSIENT_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![1, 1];
        state.rng.monster_rng = StsRng::new(123);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::Attack {
                damage: transient_attack_damage(2, 4).expect("Transient damage is in range")
            }
        );
        assert_eq!(state.rng.monster_rng.counter(), 0);
        assert_eq!(state.monsters[0].move_history, vec![1, 1, 1]);
    }

    #[test]
    fn start_player_turn_clears_unused_double_tap() {
        let mut state = CombatState::initial_fixture();
        state.double_tap_pending = 1;

        start_player_turn(&mut state).expect("player turn starts");

        assert_eq!(state.double_tap_pending, 0);
    }

    #[test]
    fn start_player_turn_with_ice_cream_adds_energy_to_preserved_pool() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::IceCream);
        state.player.energy = 3;
        state.player.max_energy = 3;

        start_player_turn(&mut state).expect("player turn starts");

        assert_eq!(state.player.energy, 6);
    }

    #[test]
    fn start_player_turn_rejects_ice_cream_energy_overflow_without_mutating_state() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::IceCream);
        state.player.energy = i32::MAX;
        state.player.max_energy = 1;
        state.validate().expect("input combat is valid");
        let before = state.clone();

        assert_eq!(
            start_player_turn(&mut state),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn start_player_turn_rejects_temporary_dexterity_underflow_without_mutating_state() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.dexterity = i32::MIN;
        state.player.temp_dexterity = 1;
        state.validate().expect("input combat is valid");
        let before = state.clone();

        assert_eq!(
            start_player_turn(&mut state),
            Err(SimError::InvalidState(
                "combat integer subtraction overflows i32"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn start_player_turn_rejects_berserk_energy_overflow_without_mutating_state() {
        let mut state = CombatState::initial_fixture();
        state.player.max_energy = i32::MAX;
        state.player.powers.berserk = 1;
        state.validate().expect("input combat is valid");
        let before = state.clone();

        assert_eq!(
            start_player_turn(&mut state),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn start_player_turn_rejects_brimstone_strength_overflow_without_mutating_state() {
        let mut player_overflow = CombatState::initial_fixture();
        player_overflow.relics.push(Relic::Brimstone);
        player_overflow.player.powers.strength = i32::MAX;
        player_overflow.validate().expect("input combat is valid");
        let player_before = player_overflow.clone();
        assert_eq!(
            start_player_turn(&mut player_overflow),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
        assert_eq!(player_overflow, player_before);

        let mut monster_overflow = CombatState::initial_fixture();
        monster_overflow.relics.push(Relic::Brimstone);
        monster_overflow.monsters[0].powers.strength = i32::MAX;
        monster_overflow.validate().expect("input combat is valid");
        let monster_before = monster_overflow.clone();
        assert_eq!(
            start_player_turn(&mut monster_overflow),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
        assert_eq!(monster_overflow, monster_before);
    }

    #[test]
    fn deca_square_blocks_all_living_monsters_and_adds_a19_plated_armor() {
        let deca_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 19;
        state.monsters = donu_deca_boss_monsters_for_ascension(state.ascension);
        state.monsters[0].moves_executed = 1;
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 16 };
        state.rng.monster_rng = StsRng::new(123);

        run_monster_turn(&mut state).expect("supported monster intent");

        let deca = state
            .monsters
            .iter()
            .find(|monster| monster.id == deca_id)
            .expect("Deca remains present");
        let donu = state
            .monsters
            .iter()
            .find(|monster| monster.id == MonsterId::new(2))
            .expect("Donu remains present");
        assert_eq!(deca.block, 19);
        assert_eq!(donu.block, 19);
        assert_eq!(deca.powers.plated_armor, 3);
        assert_eq!(donu.powers.plated_armor, 3);
        assert_eq!(deca.moves_executed, 2);
        assert_eq!(
            deca.intent,
            crate::MonsterIntent::AttackMultipleAddDazedToDiscard {
                damage: 12,
                hits: 2,
                count: 2
            }
        );
    }

    #[test]
    fn bronze_automaton_turn_prep_uses_source_post_beam_a19_boost() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 19;
        state.monsters = vec![monster_state_for_ascension(
            &BRONZE_AUTOMATON_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].moves_executed = 6;
        state.monsters[0].move_history = vec![4, 1, 5, 1, 5, 2];
        state.rng.monster_rng = StsRng::new(4444);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::StrengthAndBlock {
                strength: 4,
                block: 12,
            }
        );
        assert_eq!(state.monsters[0].move_history.last().copied(), Some(5));
        assert_eq!(state.rng.monster_rng.counter(), 1);
    }

    #[test]
    fn book_of_stabbing_turn_prep_uses_stored_stab_count() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 18;
        state.monsters = vec![monster_state_for_ascension(
            &BOOK_OF_STABBING_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].move_history = vec![2];
        state.monsters[0].powers.book_stab_count = 4;
        state.rng.monster_rng = StsRng::new(9);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::AttackMultiple { damage: 7, hits: 5 }
        );
        assert_eq!(state.monsters[0].powers.book_stab_count, 5);
        assert_eq!(state.monsters[0].move_history.last().copied(), Some(1));
    }

    #[test]
    fn book_of_stabbing_multi_hit_applies_all_hits_and_wounds() {
        // Painful Stabs multi-hit: N hits of 6 unblocked → 6N damage + N Wounds.
        // Regression probe for aef32ab6 (sim under-hit by one at 6 stabs).
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 0;
        state.player.hp = 9904;
        state.player.max_hp = 10000;
        state.player.block = 0;
        state.player.energy = 0;
        state.player.powers.brutality = 1;
        state.monsters = vec![monster_state_for_ascension(
            &BOOK_OF_STABBING_A0,
            actor_id,
            0,
        )];
        state.monsters[0].hp = 44;
        state.monsters[0].powers.book_stab_count = 6;
        state.monsters[0].powers.painful_stabs = 1;
        state.monsters[0].intent = crate::MonsterIntent::AttackMultiple { damage: 6, hits: 6 };
        state.monsters[0].move_history = vec![1, 1, 2, 1, 1, 2];
        state.piles.hand.clear();
        state.piles.discard_pile.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), STRIKE_R_ID),
            CardInstance::new(CardId::new(11), STRIKE_R_ID),
            CardInstance::new(CardId::new(12), STRIKE_R_ID),
            CardInstance::new(CardId::new(13), STRIKE_R_ID),
            CardInstance::new(CardId::new(14), STRIKE_R_ID),
            CardInstance::new(CardId::new(15), STRIKE_R_ID),
            CardInstance::new(CardId::new(16), STRIKE_R_ID),
            CardInstance::new(CardId::new(17), STRIKE_R_ID),
            CardInstance::new(CardId::new(18), STRIKE_R_ID),
            CardInstance::new(CardId::new(19), STRIKE_R_ID),
        ];
        state.rng.monster_rng = StsRng::new(1);

        let next = end_player_turn(&state).expect("end turn with Book of Stabbing multi-hit");

        // 6 hits × 6 + Brutality start-of-turn 1 = 37
        assert_eq!(
            next.player.hp,
            9904 - 36 - 1,
            "expected 6 full stabs + Brutality; got damage {}",
            9904 - next.player.hp
        );
        let wounds = next
            .piles
            .discard_pile
            .iter()
            .filter(|card| card.content_id == WOUND_ID)
            .count()
            + next
                .piles
                .hand
                .iter()
                .filter(|card| card.content_id == WOUND_ID)
                .count()
            + next
                .piles
                .draw_pile
                .iter()
                .filter(|card| card.content_id == WOUND_ID)
                .count();
        assert_eq!(
            wounds, 6,
            "Painful Stabs should add one Wound per unblocked hit"
        );
    }

    #[test]
    fn book_of_stabbing_multi_hit_defers_runic_cube_so_abacus_cannot_block_later_hits() {
        // Target: multi-hit DamageAction runs to completion before RunicCube's
        // addToBot DrawCardAction. Mid-hit draws + Abacus would otherwise grant
        // block between stabs (permanent aef32ab6: 6!=30+6 after mid-hit shuffle).
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 0;
        state.player.hp = 9904;
        state.player.max_hp = 10000;
        state.player.block = 0;
        state.relics = vec![Relic::RunicCube, Relic::TheAbacus];
        state.monsters = vec![monster_state_for_ascension(
            &BOOK_OF_STABBING_A0,
            actor_id,
            0,
        )];
        state.monsters[0].hp = 44;
        state.monsters[0].powers.book_stab_count = 6;
        state.monsters[0].powers.painful_stabs = 1;
        state.monsters[0].intent = crate::MonsterIntent::AttackMultiple { damage: 6, hits: 6 };
        state.piles.hand.clear();
        state.piles.draw_pile.clear();
        // Large discard so a mid-hit Runic Cube draw would reshuffle and Abacus.
        state.piles.discard_pile = (0..20)
            .map(|i| CardInstance::new(CardId::new(100 + i), STRIKE_R_ID))
            .collect();
        state.rng.monster_rng = StsRng::new(1);
        state.rng.shuffle_rng = StsRng::new(2);

        let next = end_player_turn(&state).expect("Book multi-hit with Runic Cube + Abacus");

        assert_eq!(
            next.player.hp,
            9904 - 36,
            "all six stabs must land unblocked; mid-hit Abacus must not fire, damage was {}",
            9904 - next.player.hp
        );
        let wounds = next
            .piles
            .discard_pile
            .iter()
            .chain(next.piles.hand.iter())
            .chain(next.piles.draw_pile.iter())
            .filter(|card| card.content_id == WOUND_ID)
            .count();
        assert_eq!(wounds, 6);
    }

    #[test]
    fn runic_cube_combust_and_two_burns_leave_five_in_discard() {
        // FIDL01762 step 1390: Burns autoplay before Combust LoseHP.
        // First Burn draws the lone Defend; second Burn shuffles the pre-Combust
        // discard (including Burn 1). Combust Cube then draws into the hand that
        // DiscardAtEndOfTurnAction dumps. Leftover is 5, next hand 6, draw 27.
        let mut state = CombatState::cultist_fixture();
        state.player.hp = 9435;
        state.player.max_hp = 10000;
        state.player.block = 0;
        state.player.powers.combust = 1;
        state.player.powers.combust_damage = 5;
        state.relics = vec![Relic::RunicCube];
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), WOUND_ID),
            CardInstance::new(CardId::new(2), BURN_ID),
            CardInstance::new(CardId::new(3), BURN_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(4), DEFEND_R_ID)];
        state.piles.discard_pile = (10..44)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.monsters[0].hp = 48;
        state.monsters[0].alive = true;
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 15 };

        let next = end_player_turn(&state).expect("end turn with Cube + Combust + Burns");

        assert_eq!(
            (
                next.piles.discard_pile.len(),
                next.piles.hand.len(),
                next.piles.draw_pile.len(),
                next.relic_counters.deferred_runic_cube_draws
            ),
            (5, 6, 27, 0),
            "hp={} disc={:?} hand={:?}",
            next.player.hp,
            next.piles
                .discard_pile
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>(),
            next.piles
                .hand
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn combust_cube_drawn_burn_does_not_autoplay() {
        // FIDL01762 step 1393: Combust Cube draws the top Burn after
        // callEndOfTurnActions already queued the single in-hand Burn.
        // The drawn Burn is discarded without dealing its 2 HP.
        let mut state = CombatState::cultist_fixture();
        state.player.hp = 9406;
        state.player.max_hp = 10000;
        state.player.block = 0;
        state.player.powers.combust = 1;
        state.player.powers.combust_damage = 5;
        state.relics = vec![Relic::RunicCube];
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), BURN_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
            CardInstance::new(CardId::new(4), BURN_ID),
        ];
        state.piles.discard_pile.clear();
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 10 };

        let next = end_player_turn(&state).expect("end turn Combust Cube into Burn");

        assert_eq!(
            next.player.hp,
            9406 - 1 - 2 - 10,
            "drawn Burn must not autoplay; hp leftover disc={:?}",
            next.piles
                .discard_pile
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn single_burn_shuffles_into_draw_when_it_empties_the_pile_before_combust() {
        // FIDL01641 step 798: one Burn + one-card draw. Burn Cube takes Anger;
        // Combust Cube then shuffles the discard including that Burn. Leftover
        // is the remaining hand plus Combust's draw (5), not the played Burn.
        let played_burn = CardInstance::new(CardId::new(3), BURN_ID);
        let mut state = CombatState::cultist_fixture();
        state.player.hp = 9253;
        state.player.max_hp = 10000;
        state.player.block = 0;
        state.player.powers.combust = 1;
        state.player.powers.combust_damage = 5;
        state.relics = vec![Relic::RunicCube];
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), PARASITE_ID),
            played_burn,
            CardInstance::new(CardId::new(4), BLOODLETTING_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(5), ANGER_ID)];
        state.piles.discard_pile = (10..35)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.monsters[0].hp = 243;
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 15 };

        let next = end_player_turn(&state).expect("end turn 1 Burn + 1-card draw");

        assert_eq!(next.piles.discard_pile.len(), 5);
        assert!(
            next.piles
                .discard_pile
                .iter()
                .all(|card| card.id != played_burn.id),
            "played Burn must be shuffled, leftover={:?}",
            next.piles
                .discard_pile
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn painful_stabs_queue_draws_before_wounds_enter_discard() {
        // Both effects use addToBot from the same multi-hit DamageAction. The
        // Runic Cube draws settle before Painful Stabs creates its status cards;
        // otherwise an empty draw pile would shuffle those new Wounds back into
        // the hand instead of leaving them in discard.
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::RunicCube];
        state.piles.hand.clear();
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();

        apply_monster_pending_effects(
            &mut state,
            crate::MonsterIntent::Stun,
            /*damage=*/ 4,
            /*hits=*/ 4,
            /*painful_stabs=*/ 1,
            None,
            0,
            0,
            0,
            0,
            false,
            0,
            None,
        )
        .expect("Runic Cube and Painful Stabs settle");

        assert_eq!(
            state
                .piles
                .discard_pile
                .iter()
                .filter(|card| card.content_id == WOUND_ID)
                .count(),
            4
        );
        assert!(state
            .piles
            .hand
            .iter()
            .chain(state.piles.draw_pile.iter())
            .all(|card| card.content_id != WOUND_ID));
    }

    #[test]
    fn painful_stabs_settles_wounds_only_when_player_survives_attack() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 30;
        state.player.block = 0;
        let discard_before = state.piles.discard_pile.len();

        apply_monster_pending_effects(
            &mut state,
            crate::MonsterIntent::Stun,
            /*damage=*/ 18,
            /*hits=*/ 3,
            /*painful_stabs=*/ 1,
            None,
            0,
            0,
            0,
            0,
            false,
            0,
            None,
        )
        .expect("non-lethal multi-hit");

        assert_eq!(state.player.hp, 12);
        assert_eq!(
            state
                .piles
                .discard_pile
                .iter()
                .filter(|card| card.content_id == WOUND_ID)
                .count(),
            3
        );
        assert_eq!(state.piles.discard_pile.len(), discard_before + 3);
    }

    #[test]
    fn painful_stabs_cancels_deferred_wounds_and_remaining_hits_on_lethal_multi_hit() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 10;
        state.player.block = 0;
        state.player.max_hp = 10;
        // No Lizard Tail / Fairy revive in the fixture by default.
        state.relics.clear();
        state.relic_counters.lizard_tail_available = false;
        state.relic_counters.fairy_heal_percent = 0;
        let discard_before = state.piles.discard_pile.len();

        apply_monster_pending_effects(
            &mut state,
            crate::MonsterIntent::Stun,
            /*damage=*/ 6 * 36,
            /*hits=*/ 36,
            /*painful_stabs=*/ 1,
            None,
            0,
            0,
            0,
            0,
            false,
            0,
            None,
        )
        .expect("lethal multi-hit");

        assert_eq!(state.player.hp, 0);
        // Target DeathScreen freezes the action manager before deferred
        // MakeTempCardInDiscardAction Wounds resolve, so none land.
        assert_eq!(
            state
                .piles
                .discard_pile
                .iter()
                .filter(|card| card.content_id == WOUND_ID)
                .count(),
            0
        );
        assert_eq!(state.piles.discard_pile.len(), discard_before);
    }

    #[test]
    fn static_discharge_evoke_cancels_remaining_hits_when_attacker_dies() {
        // DamageAction cancels later non-THORNS hits if info.owner.isDying.
        // SD addToTops ChannelAction, so a full-slot evoke can kill mid-beam.
        let mut state = CombatState::initial_fixture();
        state.player.hp = 80;
        state.player.block = 0;
        state.player.powers.static_discharge = 2;
        state.max_orbs = 3;
        state.orbs = vec![
            crate::combat::CombatOrb::Lightning,
            crate::combat::CombatOrb::Lightning,
            crate::combat::CombatOrb::Lightning,
        ];
        state.monsters[0].hp = 5;
        state.monsters[0].block = 0;
        state.monsters[0].alive = true;

        apply_monster_pending_effects(
            &mut state,
            crate::MonsterIntent::AttackMultiple {
                damage: 10,
                hits: 2,
            },
            /*damage=*/ 20,
            /*hits=*/ 2,
            /*painful_stabs=*/ 0,
            None,
            0,
            0,
            0,
            0,
            false,
            0,
            Some(0),
        )
        .expect("SD mid-beam");

        assert!(
            !state.monsters[0].alive,
            "lightning evokes should kill the attacker"
        );
        assert_eq!(
            state.player.hp, 70,
            "second beam hit must cancel after the attacker dies"
        );
    }

    #[test]
    fn looter_direct_set_move_consumes_speech_bool_without_roll_move() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state_for_ascension(&LOOTER_A0, actor_id, 0)];
        state.monsters[0].moves_executed = 1;
        state.monsters[0].move_history = vec![1];
        state.rng.monster_rng = StsRng::new(123);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::AttackStealGold {
                damage: 10,
                amount: 15
            }
        );
        assert_eq!(state.rng.monster_rng.counter(), 1);
        assert_eq!(state.monsters[0].move_history, vec![1, 1]);
    }

    #[test]
    fn looter_second_mug_uses_source_half_chance_without_roll_move() {
        let mut expected_rng = StsRng::new(456);
        let expected =
            target_looter_direct_next_intent_after_turn(&[1, 1], 2, &mut expected_rng, 0);
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state_for_ascension(&LOOTER_A0, actor_id, 0)];
        state.monsters[0].content_id = LOOTER_ID;
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![1, 1];
        state.rng.monster_rng = StsRng::new(456);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(state.monsters[0].intent, expected);
        assert_eq!(state.rng.monster_rng.counter(), expected_rng.counter());
        assert_eq!(
            state.monsters[0].move_history.last().copied(),
            crate::content::monsters::target_move_byte(LOOTER_ID, expected)
        );
    }

    #[test]
    fn mugger_direct_set_move_consumes_attack_voice_roll_without_roll_move() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state_for_ascension(&MUGGER_A0, actor_id, 0)];
        state.monsters[0].moves_executed = 1;
        state.monsters[0].move_history = vec![1];
        state.rng.monster_rng = StsRng::new(789);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::AttackStealGold {
                damage: 10,
                amount: 15
            }
        );
        assert_eq!(state.rng.monster_rng.counter(), 1);
        assert_eq!(state.monsters[0].move_history, vec![1, 1]);
    }

    #[test]
    fn mugger_second_mug_consumes_voice_talk_and_half_chance_without_roll_move() {
        let mut expected_rng = StsRng::new(987);
        let expected = crate::content::monsters::target_mugger_direct_next_intent_after_turn(
            &[1, 1],
            2,
            &mut expected_rng,
            17,
        );
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 17;
        state.monsters = vec![monster_state_for_ascension(
            &MUGGER_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![1, 1];
        state.rng.monster_rng = StsRng::new(987);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(state.monsters[0].intent, expected);
        assert_eq!(state.rng.monster_rng.counter(), expected_rng.counter());
        assert_eq!(
            state.monsters[0].move_history.last().copied(),
            crate::content::monsters::target_move_byte(MUGGER_ID, expected)
        );
    }

    #[test]
    fn gremlin_wizard_direct_cycle_does_not_consume_ai_rng_after_turn() {
        assert_eq!(
            target_gremlin_wizard_direct_next_intent_after_turn(1, 0),
            crate::MonsterIntent::Block { block: 0 }
        );
        assert_eq!(
            target_gremlin_wizard_direct_next_intent_after_turn(2, 0),
            crate::MonsterIntent::Block { block: 0 }
        );
        assert_eq!(
            target_gremlin_wizard_direct_next_intent_after_turn(3, 0),
            crate::MonsterIntent::Attack { damage: 25 }
        );
        assert_eq!(
            target_gremlin_wizard_direct_next_intent_after_turn(0, 0),
            crate::MonsterIntent::Block { block: 0 }
        );
        assert_eq!(
            target_gremlin_wizard_direct_next_intent_after_turn(3, 17),
            crate::MonsterIntent::Attack { damage: 30 }
        );

        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state_for_ascension(&GREMLIN_WIZARD_A0, actor_id, 0)];
        state.monsters[0].moves_executed = 3;
        state.monsters[0].move_history = vec![2, 2];
        state.rng.monster_rng = StsRng::new(246);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::Attack { damage: 25 }
        );
        assert_eq!(state.rng.monster_rng.counter(), 0);
        assert_eq!(state.monsters[0].move_history, vec![2, 2, 1]);
    }

    #[test]
    fn gremlin_wizard_attack_resets_current_charge_to_zero() {
        let mut state = CombatState::initial_fixture();
        let actor_id = MonsterId::new(1);
        let mut wizard = monster_state_for_ascension(&GREMLIN_WIZARD_A0, actor_id, 0);
        assert_eq!(wizard.moves_executed, 1, "target currentCharge starts at 1");
        wizard.intent = crate::MonsterIntent::Attack { damage: 25 };
        wizard.moves_executed = 3;
        wizard.hp = 22;
        wizard.max_hp = 22;
        state.monsters = vec![wizard];
        state.player.hp = 80;
        state.player.block = 0;

        let next = end_player_turn(&state).expect("wizard MAGIC resolves");
        assert_eq!(
            next.monsters[0].moves_executed, 0,
            "ATTACK takeTurn resets currentCharge"
        );
        assert!(
            matches!(
                next.monsters[0].intent,
                crate::MonsterIntent::Block { block: 0 }
            ),
            "next roll after reset is CHARGE, got {:?}",
            next.monsters[0].intent
        );
    }

    #[test]
    fn explicit_slime_size_wins_over_ambiguous_hp_and_move_history() {
        let mut monster = CombatState::initial_fixture().monsters.remove(0);
        monster.content_id = ACID_SLIME_ID;
        monster.max_hp = 10;
        monster.intent = crate::MonsterIntent::ApplyPlayerWeak { amount: 1 };
        monster.move_history = vec![2];

        monster.slime_size = Some(SlimeSize::Small);
        assert!(!acid_slime_uses_medium_move_table(&monster));

        monster.slime_size = Some(SlimeSize::Medium);
        assert!(acid_slime_uses_medium_move_table(&monster));
    }

    #[test]
    fn gremlin_warrior_and_thief_direct_set_next_move_after_turn_without_ai_rng() {
        for definition in [&GREMLIN_WARRIOR_A0, &GREMLIN_THIEF_A0] {
            let actor_id = MonsterId::new(1);
            let mut state = CombatState::initial_fixture();
            state.monsters = vec![monster_state_for_ascension(definition, actor_id, 0)];
            state.monsters[0].move_history = vec![1];
            state.rng.monster_rng = StsRng::new(123);

            prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

            assert_eq!(
                state.rng.monster_rng.counter(),
                0,
                "{} should use SetMoveAction after its turn",
                definition.name
            );
            assert_eq!(state.monsters[0].move_history, vec![1, 1]);
            assert!(matches!(
                state.monsters[0].intent,
                crate::MonsterIntent::Attack { .. }
            ));
        }
    }

    #[test]
    fn gremlin_tsundere_protect_uses_ai_rng_for_target_but_direct_sets_next_move() {
        let actor_id = MonsterId::new(1);
        let target_id = MonsterId::new(2);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![
            monster_state_for_ascension(&GREMLIN_TSUNDERE_A0, actor_id, 0),
            monster_state_for_ascension(&LOOTER_A0, target_id, 0),
        ];
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 7 };
        state.monsters[0].move_history = vec![1];
        state.rng.monster_rng = StsRng::new(246);

        run_monster_turn(&mut state).expect("supported monster intent");

        assert_eq!(state.monsters[1].block, 7);
        assert_eq!(state.monsters[0].moves_executed, 1);
        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::Block { block: 7 }
        );
        assert_eq!(state.monsters[0].move_history, vec![1, 1]);
        assert_eq!(state.rng.monster_rng.counter(), 1);
    }

    #[test]
    fn centurion_protect_uses_ai_rng_for_ally_block_before_roll_move() {
        let actor_id = MonsterId::new(1);
        let target_id = MonsterId::new(2);
        let mut state = CombatState::initial_fixture();
        state.ascension = 17;
        state.monsters = vec![
            monster_state_for_ascension(&CENTURION_A0, actor_id, state.ascension),
            monster_state_for_ascension(&HEALER_A0, target_id, state.ascension),
        ];
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 20 };
        state.monsters[0].move_history = vec![1, 1];
        state.rng.monster_rng = StsRng::new(2468);

        run_monster_turn(&mut state).expect("supported monster intent");

        assert_eq!(state.monsters[0].block, 0);
        assert_eq!(state.monsters[1].block, 20);
        assert_eq!(state.monsters[0].moves_executed, 1);
        assert_eq!(state.monsters[0].move_history.last().copied(), Some(2));
        assert_eq!(state.rng.monster_rng.counter(), 3);
    }

    #[test]
    fn sentry_turn_prep_ignores_roll_value_and_alternates_from_last_move() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 3;
        state.monsters = vec![monster_state_for_ascension(
            &SENTRY_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].moves_executed = 1;
        state.monsters[0].move_history = vec![4];
        state.rng.monster_rng = StsRng::new(123);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::AddDazedToDiscard { count: 2 }
        );
        assert_eq!(state.monsters[0].move_history, vec![4, 3]);
        assert_eq!(state.rng.monster_rng.counter(), 1);
    }

    #[test]
    fn grounded_byrd_turn_prep_uses_headbutt_without_replacement_draw() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 17;
        state.monsters = vec![monster_state_for_ascension(
            &BYRD_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].powers.flight = 0;
        state.monsters[0].move_history = vec![4];
        state.rng.monster_rng = StsRng::new(123);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(state.monsters[0].intent, target_grounded_byrd_next_intent());
        assert_eq!(state.monsters[0].move_history, vec![4, 5]);
        assert_eq!(state.rng.monster_rng.counter(), 1);
    }

    #[test]
    fn byrd_headbutt_direct_sets_go_airborne_without_ai_roll() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 17;
        state.monsters = vec![monster_state_for_ascension(
            &BYRD_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].powers.flight = 0;
        state.monsters[0].intent = target_grounded_byrd_next_intent();
        state.monsters[0].move_history = vec![4];
        state.rng.monster_rng = StsRng::new(456);
        let player_hp = state.player.hp;

        run_monster_turn(&mut state).expect("supported monster intent");

        assert_eq!(state.player.hp, player_hp - 3);
        assert_eq!(state.monsters[0].intent, target_byrd_go_airborne_intent());
        assert_eq!(
            crate::content::monsters::target_move_byte(BYRD_ID, state.monsters[0].intent),
            Some(2)
        );
        assert_eq!(state.monsters[0].move_history, vec![4, 5, 2]);
        assert_eq!(state.monsters[0].moves_executed, 1);
        assert_eq!(state.rng.monster_rng.counter(), 0);
    }

    fn three_darklings_with_one_half_dead(ascension: u8) -> CombatState {
        let mut state = CombatState::initial_fixture();
        state.ascension = ascension;
        // Regrow only proceeds while another Darkling is still living — Life Link
        // permanently kills the pack once every Darkling is half-dead (source
        // Darkling.damage allDead). Unit fixtures keep two living siblings.
        state.monsters = (1..=3)
            .map(|id| {
                let mut monster =
                    monster_state_for_ascension(&DARKLING_A0, MonsterId::new(id), state.ascension);
                monster.rolled_attack_damage = Some(8);
                monster
            })
            .collect();
        state.monsters[0].alive = false;
        state.monsters[0].escaped = true;
        state.monsters[0].hp = 0;
        state
    }

    #[test]
    fn half_dead_darkling_count_sets_reincarnate_after_one_roll() {
        let mut state = three_darklings_with_one_half_dead(17);
        state.monsters[0].rolled_attack_damage = Some(8);
        state.monsters[0].intent = crate::MonsterIntent::DarklingCount;
        state.monsters[0].move_history = vec![4];
        // Living siblings hold block intents so they do not attack the fixture player.
        state.monsters[1].intent = crate::MonsterIntent::Block { block: 12 };
        state.monsters[2].intent = crate::MonsterIntent::Block { block: 12 };
        state.rng.monster_rng = StsRng::new(111);

        run_monster_turn(&mut state).expect("supported monster intent");

        assert!(!state.monsters[0].alive);
        assert!(state.monsters[0].escaped);
        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::StrengthSelf { amount: 0 }
        );
        assert_eq!(
            crate::content::monsters::target_move_byte_for_monster(&state.monsters[0]),
            Some(5)
        );
        assert_eq!(state.monsters[0].move_history, vec![4, 5]);
        assert_eq!(state.monsters[0].moves_executed, 1);
        // mon0 COUNT roll + mon1/mon2 next-intent rolls after Harden.
        assert!(state.rng.monster_rng.counter() >= 1);
    }

    #[test]
    fn darkling_thorns_death_replays_roll_before_queued_count_move() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.thorns = 3;
        let actor_id = MonsterId::new(1);
        let mut actor = monster_state_for_ascension(&DARKLING_A0, actor_id, 0);
        actor.hp = 6;
        actor.intent = MonsterIntent::AttackMultiple { damage: 8, hits: 2 };
        actor.move_history = vec![1];
        let mut sibling = monster_state_for_ascension(&DARKLING_A0, MonsterId::new(2), 0);
        sibling.intent = MonsterIntent::Block { block: 12 };
        state.monsters = vec![actor, sibling];
        state.rng.monster_rng = StsRng::new(123);
        let mut skip_ritual_tick = Vec::new();

        execute_generic_monster_intent(&mut state, actor_id, 0, 0, &[], &mut skip_ritual_tick)
            .expect("Darkling's queued roll is supported");

        assert!(!state.monsters[0].alive);
        assert!(state.monsters[0].escaped);
        assert_eq!(state.monsters[0].intent, MonsterIntent::DarklingCount);
        assert_eq!(state.monsters[0].move_history, vec![1, 4, 5, 4]);
        assert_eq!(state.rng.monster_rng.counter(), 1);
    }

    #[test]
    fn darkling_fire_breathing_from_deferred_cube_draw_restores_count() {
        // Multi-hit CHOMP defers Runic Cube draws. Fire Breathing then kills
        // the attacker after the early death snapshot; SetMove(COUNT) must
        // still win over RollMoveAction's halfDead getMove(REINCARNATE).
        let mut state = CombatState::initial_fixture();
        state.player.powers.fire_breathing = 6;
        state.relics.push(Relic::RunicCube);
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(20), WOUND_ID),
            CardInstance::new(CardId::new(21), WOUND_ID),
        ];
        let actor_id = MonsterId::new(1);
        let mut actor = monster_state_for_ascension(&DARKLING_A0, actor_id, 0);
        actor.hp = 8;
        actor.intent = MonsterIntent::AttackMultiple { damage: 8, hits: 2 };
        actor.move_history = vec![1];
        let mut sibling = monster_state_for_ascension(&DARKLING_A0, MonsterId::new(2), 0);
        sibling.hp = 20;
        sibling.intent = MonsterIntent::Block { block: 12 };
        state.monsters = vec![actor, sibling];
        state.rng.monster_rng = StsRng::new(123);
        let mut skip_ritual_tick = Vec::new();

        execute_generic_monster_intent(&mut state, actor_id, 0, 0, &[], &mut skip_ritual_tick)
            .expect("Darkling CHOMP is supported");

        assert!(!state.monsters[0].alive);
        assert!(state.monsters[0].escaped);
        assert_eq!(state.monsters[0].intent, MonsterIntent::DarklingCount);
        assert_eq!(
            crate::content::monsters::target_move_byte_for_monster(&state.monsters[0]),
            Some(4)
        );
    }

    #[test]
    fn half_dead_darkling_reincarnates_then_rolls_next_move() {
        let mut state = three_darklings_with_one_half_dead(17);
        state.monsters[0].max_hp = 58;
        state.monsters[0].rolled_attack_damage = Some(11);
        state.monsters[0].intent = crate::MonsterIntent::Stun;
        state.monsters[0].move_history = vec![4, 5];
        state.monsters[1].intent = crate::MonsterIntent::Block { block: 12 };
        state.monsters[2].intent = crate::MonsterIntent::Block { block: 12 };
        state.relics.push(Relic::PhilosophersStone);
        state.rng.monster_rng = StsRng::new(222);
        let mut expected_rng = StsRng::new(222);
        // First monster turn: mon0 COUNT→REINCARNATE consumes one AI roll, then
        // mon1/mon2 each roll next intent after Harden.
        let _ = expected_rng.random_int(99);
        // Capture mon0 reincarnation roll only (same stream position as before
        // siblings were present: first draw of the *second* monster turn).
        // Run first turn to advance fixture to reincarnate intent.
        run_monster_turn(&mut state).expect("Darkling's first Regrow turn is supported");
        assert!(!state.monsters[0].alive);
        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::StrengthSelf { amount: 0 }
        );

        // Snapshot RNG after first turn so expected reincarnation roll matches.
        let mut expected_rng = state.rng.monster_rng.clone();
        let roll = expected_rng.random_int(99);
        let expected_intent =
            crate::content::monsters::target_darkling_next_intent_from_roll_with_rng(
                &[4, 5],
                roll,
                0,
                11,
                state.ascension,
                &mut expected_rng,
            );
        let expected_move =
            crate::content::monsters::target_move_byte(DARKLING_ID, expected_intent);

        // Keep siblings non-attacking for the reincarnation turn as well.
        state.monsters[1].intent = crate::MonsterIntent::Block { block: 12 };
        state.monsters[2].intent = crate::MonsterIntent::Block { block: 12 };

        run_monster_turn(&mut state).expect("Darkling reincarnation is supported");

        assert!(state.monsters[0].alive);
        assert!(!state.monsters[0].escaped);
        assert_eq!(state.monsters[0].hp, 29);
        assert_eq!(state.monsters[0].powers.strength, 1);
        assert_eq!(state.monsters[0].intent, expected_intent);
        assert_eq!(
            state.monsters[0].move_history.last().copied(),
            expected_move
        );
        assert_eq!(state.monsters[0].moves_executed, 2);
    }

    #[test]
    fn life_link_permanently_kills_all_darklings_when_last_goes_half_dead() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 100;
        state.player.block = 0;
        state.player.powers.thorns = 3;
        state.monsters = (1..=3)
            .map(|id| monster_state_for_ascension(&DARKLING_A0, MonsterId::new(id), 0))
            .collect();
        // mon0 about to chomp (2×8); thorns 3×2 lethals its 6 HP into half-dead.
        state.monsters[0].hp = 6;
        state.monsters[0].intent = crate::MonsterIntent::AttackMultiple { damage: 8, hits: 2 };
        // Siblings already half-dead from earlier player damage.
        for monster in &mut state.monsters[1..] {
            monster.hp = 0;
            monster.alive = false;
            monster.escaped = true;
            monster.intent = crate::MonsterIntent::Stun;
            monster.move_history = vec![4, 5];
        }

        run_monster_turn(&mut state).expect("darkling chomp with life link");

        assert_eq!(state.player.hp, 100 - 16);
        for monster in &state.monsters {
            assert!(!monster.alive, "all darklings permanently dead");
            assert!(!monster.escaped, "life link clears half-dead/regrow marker");
            assert_eq!(monster.hp, 0);
        }
        // mon1/mon2 must not reincarnate after the pack is linked.
        assert_eq!(state.monsters[1].hp, 0);
        assert_eq!(state.monsters[2].hp, 0);
    }

    #[test]
    fn philosophers_stone_strength_applies_to_split_slimes_on_spawn() {
        for definition in [&ACID_SLIME_A0, &SPIKE_SLIME_A0] {
            let actor_id = MonsterId::new(1);
            let mut state = CombatState::initial_fixture();
            state.monsters = vec![monster_state_for_ascension(
                definition,
                actor_id,
                state.ascension,
            )];
            state.monsters[0].hp = 20;
            state.monsters[0].max_hp = 70;
            state.monsters[0].slime_size = Some(SlimeSize::Large);
            state.monsters[0].powers.strength = 1;
            state.monsters[0].intent = crate::MonsterIntent::SummonGremlins { count: 2 };
            state.relics.push(Relic::PhilosophersStone);

            run_monster_turn(&mut state).expect("supported slime split");

            let children = state
                .monsters
                .iter()
                .filter(|monster| monster.alive && monster.id != actor_id)
                .collect::<Vec<_>>();
            assert_eq!(children.len(), 2);
            assert!(children.iter().all(|monster| monster.powers.strength == 1));
        }
    }

    #[test]
    fn lagavulin_natural_wake_direct_sets_attack_without_extra_ai_roll() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 3;
        state.monsters = vec![monster_state_for_ascension(
            &LAGAVULIN_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].sleep_turns_remaining = 1;
        state.monsters[0].intent = crate::MonsterIntent::Sleep;
        state.monsters[0].move_history = vec![5, 5];
        state.rng.monster_rng = StsRng::new(123);

        run_monster_turn(&mut state).expect("supported monster intent");

        assert_eq!(state.monsters[0].sleep_turns_remaining, 0);
        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::Attack { damage: 20 }
        );
        assert_eq!(state.monsters[0].move_history, vec![5, 5, 3]);
        assert_eq!(state.rng.monster_rng.counter(), 0);
    }

    #[test]
    fn lagavulin_damage_wake_stun_consumes_roll_move_before_attack() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 3;
        state.monsters = vec![monster_state_for_ascension(
            &LAGAVULIN_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].sleep_turns_remaining = 0;
        state.monsters[0].intent = crate::MonsterIntent::Stun;
        state.monsters[0].move_history = vec![5, 4];
        state.rng.monster_rng = StsRng::new(456);

        run_monster_turn(&mut state).expect("supported monster intent");

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::Attack { damage: 20 }
        );
        assert_eq!(state.monsters[0].move_history, vec![5, 4, 3]);
        assert_eq!(state.rng.monster_rng.counter(), 1);
    }

    #[test]
    fn gremlin_nob_turn_prep_uses_a18_history_guard_after_roll_action() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 18;
        state.monsters = vec![monster_state_for_ascension(
            &GREMLIN_NOB_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![3, 2];
        state.rng.monster_rng = StsRng::new(123);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::Attack { damage: 16 }
        );
        assert_eq!(state.monsters[0].move_history, vec![3, 2, 1]);
        assert_eq!(state.rng.monster_rng.counter(), 1);
    }

    #[test]
    fn spheric_guardian_uses_source_roll_table_and_move_bytes() {
        assert_eq!(
            target_spheric_guardian_next_intent_from_roll(0, &[], 17),
            crate::MonsterIntent::Block { block: 35 }
        );
        assert_eq!(
            target_spheric_guardian_next_intent_from_roll(1, &[2], 0),
            crate::MonsterIntent::AttackApplyPlayerFrail {
                damage: 10,
                frail: 5
            }
        );
        assert_eq!(
            target_spheric_guardian_next_intent_from_roll(2, &[2, 4], 2),
            crate::MonsterIntent::AttackMultiple {
                damage: 11,
                hits: 2
            }
        );
        assert_eq!(
            target_spheric_guardian_next_intent_from_roll(3, &[2, 4, 1], 2),
            crate::MonsterIntent::AttackAndBlock {
                damage: 11,
                block: 15
            }
        );

        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 2;
        state.monsters = vec![monster_state_for_ascension(
            &SPHERIC_GUARDIAN_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].content_id = SPHERIC_GUARDIAN_ID;
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![2, 4];
        state.rng.monster_rng = StsRng::new(246);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::AttackMultiple {
                damage: 11,
                hits: 2
            }
        );
        assert_eq!(state.rng.monster_rng.counter(), 1);
        assert_eq!(state.monsters[0].move_history, vec![2, 4, 1]);
    }

    #[test]
    fn spheric_guardian_hardens_before_attacking_into_flame_barrier() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state_for_ascension(
            &SPHERIC_GUARDIAN_A0,
            actor_id,
            0,
        )];
        state.monsters[0].content_id = SPHERIC_GUARDIAN_ID;
        state.monsters[0].hp = 20;
        state.monsters[0].max_hp = 20;
        state.monsters[0].block = 3;
        state.monsters[0].intent = crate::MonsterIntent::AttackAndBlock {
            damage: 10,
            block: 15,
        };
        state.player.hp = 64;
        state.player.block = 12;
        state.player.temp_thorns = 4;

        run_monster_turn(&mut state).expect("supported monster intent");

        assert_eq!(state.player.hp, 64);
        assert_eq!(state.monsters[0].hp, 20);
        assert_eq!(state.monsters[0].block, 14);
    }

    #[test]
    fn combust_lethal_awakened_one_rebirths_on_death_end() {
        // Permanent FIDL00368 / FIDL00395: Combust first-kill during END still
        // REBIRTHs in that END's enemy phase (Dark Echo before the redraw).
        let mut state = CombatState::initial_fixture();
        state.player.powers.combust = 1;
        state.player.powers.combust_damage = 30;
        let mut ao = monster_state_for_ascension(
            &crate::content::monsters::AWAKENED_ONE_A0,
            MonsterId::new(1),
            0,
        );
        ao.hp = 20;
        ao.max_hp = 300;
        ao.mode_shift = 0;
        ao.intent = crate::MonsterIntent::AttackMultiple { damage: 6, hits: 4 };
        ao.move_history = vec![1, 2];
        state.monsters = vec![ao];
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        state.piles.draw_pile = (2..=12)
            .map(|i| CardInstance::new(CardId::new(i), STRIKE_R_ID))
            .collect();

        let after_death_end = end_player_turn(&state).expect("death END");
        assert!(
            after_death_end.monsters[0].alive,
            "Combust first-kill REBIRTHs on the death END"
        );
        assert_eq!(after_death_end.monsters[0].hp, 300);
        assert_eq!(after_death_end.piles.hand.len(), 5);
        assert_eq!(
            after_death_end.monsters[0].intent,
            crate::MonsterIntent::Attack { damage: 40 }
        );
        assert!(!after_death_end.monsters[0].defer_awakened_one_rebirth);
    }

    #[test]
    fn frail_ticks_after_a_survived_monster_turn_but_not_after_lethal_damage() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 10;
        state.player.powers.frail = 2;
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 1 };
        state.monsters[0].initial_intent_locked = true;

        let survived = end_player_turn(&state).expect("nonlethal monster turn resolves");
        assert_eq!(survived.player.powers.frail, 1);

        state.player.hp = 1;
        let killed = end_player_turn(&state).expect("lethal monster turn resolves");
        assert_eq!(killed.phase, CombatPhase::Lost);
        assert_eq!(killed.player.powers.frail, 2);
    }

    #[test]
    fn fully_blocked_attack_still_executes_its_later_frail_action() {
        let mut state = CombatState::initial_fixture();
        state.player.block = 999;
        state.monsters[0].intent = crate::MonsterIntent::AttackApplyPlayerFrail {
            damage: 1,
            frail: 2,
        };
        state.monsters[0].initial_intent_locked = true;

        let survived = end_player_turn(&state).expect("blocked monster turn resolves");
        assert_eq!(survived.player.powers.frail, 2);
    }

    #[test]
    fn lethal_attack_cancels_its_later_frail_action_without_consuming_artifact() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 1;
        state.player.powers.artifact = 1;
        state.monsters[0].intent = crate::MonsterIntent::AttackApplyPlayerFrail {
            damage: 1,
            frail: 2,
        };
        state.monsters[0].initial_intent_locked = true;

        let killed = end_player_turn(&state).expect("lethal monster turn resolves");
        assert_eq!(killed.phase, CombatPhase::Lost);
        assert_eq!(killed.player.powers.frail, 0);
        assert_eq!(killed.player.powers.artifact, 1);
    }

    #[test]
    fn nilry_stage_three_pending_powers_do_not_tick_end_of_round_debuffs() {
        let mut state = CombatState::initial_fixture();
        state.nilrys_end_powers_pending = true;
        state.player.powers.weak = 2;
        state.player.powers.frail = 2;
        apply_pending_nilry_end_powers(&mut state).expect("pending powers");
        assert_eq!(state.player.powers.weak, 2);
        assert_eq!(state.player.powers.frail, 2);
        assert!(!state.nilrys_end_powers_pending);
    }

    #[test]
    fn shelled_parasite_suck_into_flame_barrier_with_vulnerable() {
        // FIDL00227 step 629: Suck 10 into 12 block while player is Vulnerable
        // and Flame Barrier 4 is up. 10 * 1.5 = 15 → 3 HP through block; mon
        // heals 3 then takes 4 FB → net mon -1 from pre-attack HP.
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state_for_ascension(
            &SHELLED_PARASITE_A0,
            actor_id,
            0,
        )];
        state.monsters[0].content_id = SHELLED_PARASITE_ID;
        state.monsters[0].hp = 41;
        state.monsters[0].max_hp = 72;
        state.monsters[0].block = 0;
        state.monsters[0].powers.plated_armor = 10;
        state.monsters[0].intent = crate::MonsterIntent::AttackHealSelf { damage: 10 };
        state.player.hp = 9125;
        state.player.max_hp = 10000;
        state.player.block = 12;
        state.player.powers.vulnerable = 1;
        state.player.temp_thorns = 4;

        run_monster_turn(&mut state).expect("suck resolves");

        assert_eq!(state.player.hp, 9122, "15 through 12 block = 3 HP");
        assert_eq!(state.monsters[0].hp, 40, "heal 3 then FB 4 → 41+3-4=40");
    }

    #[test]
    fn mayhem_after_combat_won_does_not_leave_stale_decision() {
        // FIDL00243: END after a lethal play refills via start_player_turn; Mayhem
        // may force-play Armaments into a hand select while every monster is
        // already dead. Combat end must clear that decision.
        let mut state = CombatState::initial_fixture();
        state.phase = CombatPhase::Won;
        state.player.powers.mayhem = 1;
        state.monsters[0].alive = false;
        state.monsters[0].hp = 0;
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(1), ARMAMENTS_ID)];
        state.piles.hand.clear();
        state.decision = None;

        start_player_turn(&mut state).expect("start turn after win");
        assert_eq!(state.phase, CombatPhase::Won);
        assert!(
            state.decision.is_none(),
            "stale decision={:?}",
            state.decision
        );
    }

    #[test]
    fn mayhem_target_roll_precedes_confusion_hand_draw() {
        // MayhemPower$1.update rolls getRandomMonster before DrawCardAction.
        // With one living monster that is still Random.random(0, 0).
        let mut state = CombatState::initial_fixture();
        state.player.powers.mayhem = 1;
        state.player.powers.confusion = 1;
        state.piles.hand.clear();
        state.piles.discard_pile.clear();
        state.piles.draw_pile = (1..=6)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.monsters = vec![monster_state_for_ascension(
            &LOOTER_A0,
            MonsterId::new(1),
            0,
        )];
        state.rng.card_random_rng = crate::rng::StsRng::new(7);

        let mut expected_rng = crate::rng::StsRng::new(7);
        let _mayhem_target = expected_rng.random_int(0);
        let expected_first_cost = expected_rng.random_int(3) as u8;

        start_player_turn(&mut state).expect("start turn");

        assert_eq!(
            state.piles.hand[0].temp_cost,
            Some(expected_first_cost),
            "first drawn cost must use the roll after Mayhem's target"
        );
        assert_ne!(
            expected_first_cost,
            crate::rng::StsRng::new(7).random_int(3) as u8,
            "fixture seed must distinguish the skipped target roll"
        );
    }

    #[test]
    fn mayhem_plays_after_brutality_draw() {
        // FIDL00381: after base draw, Brutality draws the next top card, then
        // Mayhem force-plays the following top (Anger), not the Brutality card.
        let mut state = CombatState::initial_fixture();
        state.player.powers.mayhem = 1;
        state.player.powers.brutality = 1;
        state.player.hp = 50;
        state.player.max_hp = 50;
        state.piles.hand.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        // bottom → top (last = top). Draw pops five Strikes; Shrug then Anger remain.
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), ANGER_ID),
            CardInstance::new(CardId::new(2), SHRUG_IT_OFF_PLUS_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
            CardInstance::new(CardId::new(5), STRIKE_R_ID),
            CardInstance::new(CardId::new(6), STRIKE_R_ID),
            CardInstance::new(CardId::new(7), STRIKE_R_ID),
        ];
        state.monsters = vec![monster_state_for_ascension(
            &LOOTER_A0,
            MonsterId::new(1),
            0,
        )];
        state.monsters[0].max_hp = 99;
        state.monsters[0].hp = 99;

        start_player_turn(&mut state).expect("start turn");

        assert!(
            state
                .piles
                .hand
                .iter()
                .any(|c| c.content_id == SHRUG_IT_OFF_PLUS_ID),
            "Brutality must draw Shrug into hand, hand={:?}",
            state
                .piles
                .hand
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            state.player.block, 0,
            "Mayhem must not play Shrug (would grant block)"
        );
        let anger_in_discard = state
            .piles
            .discard_pile
            .iter()
            .filter(|c| c.content_id == ANGER_ID)
            .count();
        assert!(
            anger_in_discard >= 2,
            "Mayhem plays Anger (discard + copy), discard={:?}",
            state
                .piles
                .discard_pile
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn mayhem_discards_unplayable_top_card_after_normal_draw() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.mayhem = 1;
        state.piles.hand.clear();
        state.piles.discard_pile.clear();
        state.piles.draw_pile = std::iter::once(CardInstance::new(CardId::new(1), DOUBT_ID))
            .chain((2..=6).map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID)))
            .collect();
        state.monsters = vec![monster_state_for_ascension(
            &LOOTER_A0,
            MonsterId::new(1),
            0,
        )];

        start_player_turn(&mut state).expect("player turn starts");

        assert_eq!(state.piles.hand.len(), 5);
        assert!(state.piles.draw_pile.is_empty());
        assert_eq!(state.piles.discard_pile.len(), 1);
        assert_eq!(state.piles.discard_pile[0].content_id, DOUBT_ID);
    }

    #[test]
    fn mayhem_shuffles_discard_before_playing_when_draw_is_empty() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.mayhem = 1;
        state.piles.hand.clear();
        state.piles.draw_pile = (1..=5)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(6), DEFEND_R_ID)];
        state.monsters = vec![monster_state_for_ascension(
            &LOOTER_A0,
            MonsterId::new(1),
            0,
        )];

        start_player_turn(&mut state).expect("player turn starts");

        assert_eq!(state.player.block, 5);
        assert_eq!(state.piles.hand.len(), 5);
        assert!(state.piles.draw_pile.is_empty());
        assert_eq!(state.piles.discard_pile.len(), 1);
        assert_eq!(state.piles.discard_pile[0].content_id, DEFEND_R_ID);
    }

    #[test]
    fn mayhem_play_top_power_is_empowered_not_discarded() {
        // UseCardAction.empower removes a Power; it does not moveToDiscardPile.
        let mut state = CombatState::initial_fixture();
        state.player.powers.mayhem = 1;
        state.piles.hand.clear();
        state.piles.discard_pile.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(20), INFLAME_ID),
            CardInstance::new(CardId::new(21), STRIKE_R_ID),
            CardInstance::new(CardId::new(22), STRIKE_R_ID),
            CardInstance::new(CardId::new(23), STRIKE_R_ID),
            CardInstance::new(CardId::new(24), STRIKE_R_ID),
            CardInstance::new(CardId::new(25), STRIKE_R_ID),
        ];
        state.monsters = vec![monster_state_for_ascension(
            &LOOTER_A0,
            MonsterId::new(1),
            0,
        )];

        start_player_turn(&mut state).expect("Mayhem plays Inflame");

        assert!(
            !state
                .piles
                .discard_pile
                .iter()
                .any(|card| card.content_id == INFLAME_ID),
            "played Power must not enter discard, discard={:?}",
            state
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
        );
        assert!(
            state.player.powers.strength > 0,
            "Inflame must apply Strength"
        );
    }

    #[test]
    fn mayhem_play_top_settles_after_evolve_residual_draw() {
        // UseCardAction is behind Evolve's addToBot Draw from the base refill,
        // so the forced card is not in the shuffle Evolve consumes.
        let mut state = CombatState::initial_fixture();
        state.player.powers.mayhem = 1;
        state.player.powers.evolve = 1;
        state.piles.hand.clear();
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(10), DEFEND_R_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(20), STRIKE_R_ID),
            CardInstance::new(CardId::new(21), WOUND_ID),
            CardInstance::new(CardId::new(22), DEFEND_R_ID),
            CardInstance::new(CardId::new(23), DEFEND_R_ID),
            CardInstance::new(CardId::new(24), DEFEND_R_ID),
            CardInstance::new(CardId::new(25), DEFEND_R_ID),
        ];
        state.piles.exhaust_pile.clear();
        state.monsters = vec![monster_state_for_ascension(
            &LOOTER_A0,
            MonsterId::new(1),
            0,
        )];

        start_player_turn(&mut state).expect("Mayhem + Evolve start turn");

        assert!(
            state
                .piles
                .discard_pile
                .iter()
                .any(|card| card.id == CardId::new(20)),
            "Mayhem Strike must settle after Evolve shuffle, discard={:?}",
            state
                .piles
                .discard_pile
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
        );
        assert!(
            state
                .piles
                .hand
                .iter()
                .any(|card| card.content_id == DEFEND_R_ID && card.id == CardId::new(10)),
            "Evolve must draw the pre-Mayhem discard Defend, hand={:?}",
            state
                .piles
                .hand
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn deferred_mayhem_exhaust_settlement_runs_ordinary_exhaust_hooks() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.mayhem = 1;
        state.player.powers.evolve = 1;
        state.player.powers.feel_no_pain = 3;
        state.player.block = 0;
        state.piles.hand.clear();
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(10), STRIKE_R_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(20), SLIMED_ID),
            CardInstance::new(CardId::new(21), WOUND_ID),
            CardInstance::new(CardId::new(22), DEFEND_R_ID),
            CardInstance::new(CardId::new(23), DEFEND_R_ID),
            CardInstance::new(CardId::new(24), DEFEND_R_ID),
            CardInstance::new(CardId::new(25), DEFEND_R_ID),
        ];
        state.piles.exhaust_pile.clear();
        state.monsters = vec![monster_state_for_ascension(
            &LOOTER_A0,
            MonsterId::new(1),
            0,
        )];

        start_player_turn(&mut state).expect("Mayhem settles Slimed after Evolve draw");

        assert_eq!(state.player.block, 3, "Feel No Pain must run on settlement");
        assert!(state
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(20)));
        assert!(state.deferred_mayhem_play_top_settlements.is_empty());
        assert_eq!(state.card_in_use, None);
    }

    #[test]
    fn deferred_mayhem_power_settlement_uses_played_power_removal() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.mayhem = 1;
        state.player.powers.evolve = 1;
        state.piles.hand.clear();
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(10), STRIKE_R_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(20), INFLAME_ID),
            CardInstance::new(CardId::new(21), WOUND_ID),
            CardInstance::new(CardId::new(22), DEFEND_R_ID),
            CardInstance::new(CardId::new(23), DEFEND_R_ID),
            CardInstance::new(CardId::new(24), DEFEND_R_ID),
            CardInstance::new(CardId::new(25), DEFEND_R_ID),
        ];
        state.piles.exhaust_pile.clear();
        state.monsters = vec![monster_state_for_ascension(
            &LOOTER_A0,
            MonsterId::new(1),
            0,
        )];

        start_player_turn(&mut state).expect("Mayhem settles Inflame after Evolve draw");

        assert_eq!(state.player.powers.strength, 2);
        assert!(state
            .piles
            .hand
            .iter()
            .chain(state.piles.draw_pile.iter())
            .chain(state.piles.discard_pile.iter())
            .chain(state.piles.exhaust_pile.iter())
            .chain(state.piles.limbo.iter())
            .all(|card| card.id != CardId::new(20)));
        assert!(state.deferred_mayhem_play_top_settlements.is_empty());
        assert_eq!(state.card_in_use, None);
    }

    #[test]
    fn stacked_mayhem_plays_both_tops_before_ink_bottle_draw() {
        // MayhemPower queues both PlayTops first. InkBottle.onUseCard addToBot
        // Draw sits behind the second PlayTop, so it cannot steal that card.
        let mut state = CombatState::initial_fixture();
        state.player.powers.mayhem = 2;
        state.relics = vec![crate::Relic::InkBottle];
        state.relic_counters.ink_bottle_cards_played = crate::relic::INK_BOTTLE_THRESHOLD - 1;
        state.piles.hand.clear();
        // last() is the draw-pile top. Five Pommels sit on top so the opening
        // draw leaves Defend/Bash/Strike for Mayhem + Ink.
        let mut draw_pile = vec![
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
            CardInstance::new(CardId::new(21), BASH_ID),
            CardInstance::new(CardId::new(22), STRIKE_R_ID),
        ];
        draw_pile.extend((30..35).map(|id| CardInstance::new(CardId::new(id), POMMEL_STRIKE_ID)));
        state.piles.draw_pile = draw_pile;
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.monsters = vec![monster_state_for_ascension(
            &LOOTER_A0,
            MonsterId::new(1),
            0,
        )];

        start_player_turn(&mut state).expect("Mayhem 2 then Ink draw");

        assert!(
            state
                .piles
                .discard_pile
                .iter()
                .any(|card| card.content_id == STRIKE_R_ID),
            "first PlayTop must be Strike, discard={:?}",
            state
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
        );
        assert!(
            state
                .piles
                .discard_pile
                .iter()
                .any(|card| card.content_id == BASH_ID),
            "second PlayTop must be Bash before Ink draws",
        );
        assert!(
            state
                .piles
                .hand
                .iter()
                .any(|card| card.content_id == DEFEND_R_ID),
            "Ink must draw the leftover Defend, hand={:?}",
            state
                .piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
        );
        assert!(
            !state
                .piles
                .hand
                .iter()
                .any(|card| card.content_id == BASH_ID),
            "Ink must not steal the second PlayTop into hand",
        );
    }

    #[test]
    fn stacked_mayhem_pops_second_top_before_pommel_draw() {
        // PlayTopCardAction removes its top before use() addToBots Draw.
        // Mayhem 2 must take Intimidate before Pommel draws the Wound
        // (FIDL02199).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(20), WOUND_ID),
            CardInstance::new(CardId::new(21), crate::content::cards::INTIMIDATE_ID),
            CardInstance::new(CardId::new(22), POMMEL_STRIKE_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.monsters = vec![monster_state_for_ascension(
            &LOOTER_A0,
            MonsterId::new(1),
            0,
        )];
        let target = Some(state.monsters[0].id);

        crate::combat::transition::apply_mayhem_play_top_cards(&mut state, &[target, target])
            .expect("Mayhem 2 pops both tops first");

        assert!(
            state
                .piles
                .discard_pile
                .iter()
                .any(|card| card.content_id == POMMEL_STRIKE_ID),
            "Pommel must be played, discard={:?}",
            state
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
        );
        assert!(
            state
                .piles
                .discard_pile
                .iter()
                .chain(state.piles.exhaust_pile.iter())
                .any(|card| card.content_id == crate::content::cards::INTIMIDATE_ID),
            "Intimidate must be the second PlayTop, not the Pommel draw",
        );
        assert!(
            state
                .piles
                .hand
                .iter()
                .any(|card| card.content_id == WOUND_ID),
            "Pommel should draw Wound after both PlayTops removed",
        );
    }

    #[test]
    fn fire_breathing_kills_mayhem_target_before_pommel_use() {
        // GameActionManager drains action-queue Fire Breathing before cardQueue
        // plays the Mayhem tops. A dead target skips use() but still settles.
        let mut state = CombatState::initial_fixture();
        state.player.powers.mayhem = 2;
        state.player.powers.fire_breathing = 16;
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), WOUND_ID),
            CardInstance::new(CardId::new(11), INTIMIDATE_ID),
            CardInstance::new(CardId::new(12), POMMEL_STRIKE_ID),
            CardInstance::new(CardId::new(13), BURN_ID),
            CardInstance::new(CardId::new(14), DEFEND_R_ID),
            CardInstance::new(CardId::new(15), DEFEND_R_ID),
            CardInstance::new(CardId::new(16), DEFEND_R_ID),
            CardInstance::new(CardId::new(17), BURN_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        let mut target = monster_state_for_ascension(&LOOTER_A0, MonsterId::new(1), 0);
        target.hp = 20;
        target.max_hp = 20;
        target.block = 0;
        state.monsters = vec![target];

        start_player_turn(&mut state).expect("Mayhem waits for Fire Breathing");

        assert!(
            !state.monsters[0].alive,
            "two Burns should Fire Breathe the 20 HP target down",
        );
        assert!(
            state
                .piles
                .draw_pile
                .iter()
                .any(|card| card.content_id == WOUND_ID),
            "dead-target Pommel must not draw, draw={:?}",
            state
                .piles
                .draw_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
        );
        assert!(
            state
                .piles
                .discard_pile
                .iter()
                .any(|card| card.content_id == POMMEL_STRIKE_ID),
            "skipped Pommel still settles to discard",
        );
    }

    #[test]
    fn stacked_mayhem_plays_second_top_before_deep_breath_shuffle() {
        // MayhemPower queues both PlayTopCardActions before DeepBreath.use
        // addToBots ShuffleAction. The second PlayTop is therefore the card
        // that was under Deep Breath, not a post-shuffle top.
        let mut state = CombatState::initial_fixture();
        state.player.powers.mayhem = 2;
        state.piles.hand = (1..=10)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
            CardInstance::new(CardId::new(21), BASH_ID),
            CardInstance::new(CardId::new(22), DEEP_BREATH_ID),
        ];
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(23), ANGER_ID)];
        state.piles.exhaust_pile.clear();
        state.monsters = vec![monster_state_for_ascension(
            &LOOTER_A0,
            MonsterId::new(1),
            0,
        )];
        let monster_hp = state.monsters[0].hp;

        start_player_turn(&mut state).expect("Mayhem 2 PlayTops Deep Breath then Bash");

        assert!(
            state
                .piles
                .discard_pile
                .iter()
                .any(|card| card.content_id == BASH_ID),
            "second PlayTop must be the pre-shuffle Bash, discard={:?}",
            state
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>()
        );
        assert!(
            state
                .piles
                .discard_pile
                .iter()
                .any(|card| card.content_id == DEEP_BREATH_ID),
            "Deep Breath still settles to discard after its delayed shuffle"
        );
        assert!(
            state
                .piles
                .draw_pile
                .iter()
                .any(|card| card.content_id == ANGER_ID),
            "Deep Breath shuffle still mixes the original discard after both PlayTops pop"
        );
        assert!(
            state.monsters[0].hp < monster_hp,
            "Bash must still deal its PlayTop damage"
        );
    }

    #[test]
    fn mayhem_unknown_top_card_fails_without_partial_turn_mutation() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.mayhem = 1;
        state.piles.hand = (1..=10)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        let unknown = crate::ContentId::new(u64::MAX);
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(11), unknown)];
        let before = state.clone();

        assert_eq!(
            start_player_turn(&mut state),
            Err(crate::SimError::UnknownContent(unknown))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn mayhem_uses_shared_card_effects_for_demon_form() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.mayhem = 1;
        state.piles.hand = (1..=10)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(11), DEMON_FORM_ID)];
        start_player_turn(&mut state).expect("Mayhem plays Demon Form through shared effects");

        assert_eq!(state.player.powers.demon_form, 2);
        assert!(state.piles.draw_pile.is_empty());
        assert!(!state
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == DEMON_FORM_ID));
    }

    #[test]
    fn demon_form_gains_strength_at_start_of_turn_post_draw() {
        // DemonFormPower.atStartOfTurnPostDraw, not RitualPower.atEndOfTurn.
        let mut state = CombatState::cultist_fixture();
        state.player.powers.demon_form = 2;
        state.player.powers.strength = 1;
        state.piles.hand.clear();
        state.piles.draw_pile = (10..=14)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        start_player_turn(&mut state).expect("start turn");
        assert_eq!(state.player.powers.strength, 3);
        assert_eq!(state.player.powers.demon_form, 2);
    }

    #[test]
    fn demon_form_does_not_tick_with_end_of_turn_ritual() {
        let mut state = CombatState::cultist_fixture();
        state.player.powers.demon_form = 2;
        state.player.powers.strength = 1;
        crate::combat::turn_powers::apply_end_of_player_turn_powers(&mut state)
            .expect("end-turn powers");
        assert_eq!(
            state.player.powers.strength, 1,
            "Demon Form must not share RitualPower's end-of-turn tick"
        );
    }

    #[test]
    fn maw_uses_source_turn_count_roll_table_and_move_bytes() {
        assert_eq!(
            target_maw_next_intent_from_roll(0, &[], 99, 17),
            crate::MonsterIntent::ApplyPlayerFrailAndWeak { frail: 5, weak: 5 }
        );
        assert_eq!(
            target_maw_next_intent_from_roll(1, &[2], 49, 0),
            crate::MonsterIntent::Attack { damage: 5 }
        );
        assert_eq!(
            target_maw_next_intent_from_roll(2, &[2, 5], 0, 0),
            crate::MonsterIntent::StrengthSelf { amount: 3 }
        );
        assert_eq!(
            target_maw_next_intent_from_roll(3, &[2, 5, 4], 99, 2),
            crate::MonsterIntent::Attack { damage: 30 }
        );
        assert_eq!(
            target_maw_next_intent_from_roll(4, &[2, 5, 4, 3], 0, 17),
            crate::MonsterIntent::AttackMultiple { damage: 5, hits: 3 }
        );

        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 17;
        state.monsters = vec![monster_state_for_ascension(
            &MAW_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].content_id = MAW_ID;
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![2, 5];
        state.rng.monster_rng = StsRng::new(135);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::StrengthSelf { amount: 5 }
        );
        assert_eq!(state.rng.monster_rng.counter(), 1);
        assert_eq!(state.monsters[0].move_history, vec![2, 5, 4]);
    }

    #[test]
    fn spire_growth_uses_source_constrict_roll_table_and_hp() {
        assert_eq!(
            target_spire_growth_next_intent_from_roll(0, &[], 99, false, 17),
            crate::MonsterIntent::ApplyPlayerConstricted { amount: 12 }
        );
        assert_eq!(
            target_spire_growth_next_intent_from_roll(0, &[], 49, false, 0),
            crate::MonsterIntent::Attack { damage: 16 }
        );
        assert_eq!(
            target_spire_growth_next_intent_from_roll(1, &[1], 99, false, 0),
            crate::MonsterIntent::ApplyPlayerConstricted { amount: 10 }
        );
        assert_eq!(
            target_spire_growth_next_intent_from_roll(2, &[1, 2], 99, true, 2),
            crate::MonsterIntent::Attack { damage: 25 }
        );
        assert_eq!(
            target_spire_growth_next_intent_from_roll(4, &[1, 2, 3, 3], 99, true, 2),
            crate::MonsterIntent::Attack { damage: 18 }
        );

        let mut source_monster =
            monster_state_for_ascension(&SPIRE_GROWTH_A0, MonsterId::new(1), 17);
        assert_eq!((source_monster.hp, source_monster.max_hp), (190, 190));
        source_monster.intent = crate::MonsterIntent::ApplyPlayerConstricted { amount: 12 };
        let fixture = CombatState::initial_fixture();
        let allocated_card_id_through = fixture.max_authoritative_card_instance_id();
        let mut player = fixture.player;
        let before = player.clone();
        let mut piles = fixture.piles;
        let mut card_random_rng = StsRng::new(0);
        let damage = crate::content::monsters::apply_monster_intent_with_card_rng(
            &mut source_monster,
            &mut player,
            &mut piles,
            allocated_card_id_through,
            17,
            &before,
            &[],
            &mut card_random_rng,
        );
        assert_eq!(damage, Ok(0));
        assert_eq!(player.powers.constricted, 12);

        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 17;
        state.monsters = vec![monster_state_for_ascension(
            &SPIRE_GROWTH_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].content_id = SPIRE_GROWTH_ID;
        state.monsters[0].moves_executed = 1;
        state.monsters[0].move_history = vec![1];
        state.player.powers.constricted = 0;
        state.rng.monster_rng = StsRng::new(2468);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::ApplyPlayerConstricted { amount: 12 }
        );
        assert_eq!(state.rng.monster_rng.counter(), 1);
        assert_eq!(state.monsters[0].move_history, vec![1, 2]);
    }

    #[test]
    fn giant_head_uses_source_countdown_roll_table_hp_and_slow_setup() {
        assert_eq!(
            target_giant_head_next_intent_from_roll(0, &[], 49, 0),
            crate::MonsterIntent::ApplyPlayerWeak { amount: 1 }
        );
        assert_eq!(
            target_giant_head_next_intent_from_roll(0, &[], 50, 0),
            crate::MonsterIntent::Attack { damage: 13 }
        );
        assert_eq!(
            target_giant_head_next_intent_from_roll(2, &[1, 1], 0, 0),
            crate::MonsterIntent::Attack { damage: 13 }
        );
        assert_eq!(
            target_giant_head_next_intent_from_roll(4, &[1, 3, 1, 3], 0, 0),
            crate::MonsterIntent::Attack { damage: 30 }
        );
        assert_eq!(
            target_giant_head_next_intent_from_roll(3, &[1, 3, 1], 0, 18),
            crate::MonsterIntent::Attack { damage: 40 }
        );
        assert_eq!(
            target_giant_head_next_intent_from_roll(10, &[2, 2, 2], 0, 18),
            crate::MonsterIntent::Attack { damage: 70 }
        );
        assert_eq!(
            target_giant_head_next_intent_from_roll(12, &[2; 12], 0, 0),
            crate::MonsterIntent::Attack { damage: 60 }
        );

        let source_monster = monster_state_for_ascension(&GIANT_HEAD_A0, MonsterId::new(1), 18);
        assert_eq!((source_monster.hp, source_monster.max_hp), (520, 520));
        assert_eq!(source_monster.powers.slow, 1);

        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 18;
        state.monsters = vec![monster_state_for_ascension(
            &GIANT_HEAD_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].content_id = GIANT_HEAD_ID;
        state.monsters[0].moves_executed = 3;
        state.monsters[0].move_history = vec![1, 3, 1];
        state.rng.monster_rng = StsRng::new(97531);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::Attack { damage: 40 }
        );
        assert_eq!(state.rng.monster_rng.counter(), 1);
        assert_eq!(state.monsters[0].move_history, vec![1, 3, 1, 2]);
    }

    #[test]
    fn nemesis_uses_source_replacement_booleans_burns_hp_and_intangible() {
        let mut no_reroll_rng = StsRng::new(0);
        assert_eq!(
            target_nemesis_next_intent_from_roll(0, &[], 49, &mut no_reroll_rng, 3),
            crate::MonsterIntent::AttackMultiple { damage: 7, hits: 3 }
        );
        assert_eq!(
            target_nemesis_next_intent_from_roll(0, &[], 50, &mut no_reroll_rng, 18),
            crate::MonsterIntent::AddBurnToDiscard {
                count: 5,
                damage: 0
            }
        );
        assert_eq!(
            target_nemesis_next_intent_from_roll(1, &[2], 29, &mut no_reroll_rng, 0),
            crate::MonsterIntent::Attack { damage: 45 }
        );
        assert_eq!(no_reroll_rng.counter(), 0);

        let mut expected_rng = StsRng::new(4242);
        let expected = target_nemesis_next_intent_from_roll(2, &[2, 3], 20, &mut expected_rng, 18);
        assert_eq!(expected_rng.counter(), 1);
        assert!(matches!(
            expected,
            crate::MonsterIntent::AttackMultiple { .. }
                | crate::MonsterIntent::AddBurnToDiscard { .. }
        ));

        let source_monster = monster_state_for_ascension(&NEMESIS_A0, MonsterId::new(1), 18);
        assert_eq!((source_monster.hp, source_monster.max_hp), (200, 200));

        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 18;
        state.monsters = vec![monster_state_for_ascension(
            &NEMESIS_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].content_id = NEMESIS_ID;
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![2, 3];
        state.rng.monster_rng = StsRng::new(4242);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("supported monster intent");

        assert_eq!(state.monsters[0].intent, expected);
        assert_eq!(state.rng.monster_rng.counter(), expected_rng.counter() + 1);
        assert_eq!(
            state.monsters[0].move_history.last().copied(),
            crate::content::monsters::target_move_byte(NEMESIS_ID, expected)
        );

        state.monsters[0].intent = crate::MonsterIntent::AddBurnToDiscard {
            count: 5,
            damage: 0,
        };
        state.monsters[0].moves_executed = 0;
        state.monsters[0].move_history.clear();
        state.monsters[0].powers.strength = 1;
        state.player.temp_thorns = 4;
        let monster_hp_before_burn = state.monsters[0].hp;
        let player_hp_before_burn = state.player.hp;
        run_monster_turn(&mut state).expect("supported monster intent");

        assert_eq!(state.player.hp, player_hp_before_burn);
        assert_eq!(state.monsters[0].powers.intangible, 1);
        assert_eq!(state.monsters[0].hp, monster_hp_before_burn);
        assert_eq!(
            state
                .piles
                .discard_pile
                .iter()
                .filter(|card| card.content_id == BURN_ID)
                .count(),
            5
        );
        let hp_before = state.monsters[0].hp;
        let hp_damage =
            crate::combat::damage::deal_unmodified_damage_to_monster(&mut state.monsters[0], 99);
        assert_eq!(hp_damage, 1);
        assert_eq!(state.monsters[0].hp, hp_before - 1);

        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 0 };
        run_monster_turn(&mut state).expect("supported monster intent");
        assert_eq!(state.monsters[0].powers.intangible, 0);
        let hp_before = state.monsters[0].hp;
        let hp_damage =
            crate::combat::damage::deal_unmodified_damage_to_monster(&mut state.monsters[0], 99);
        assert_eq!(hp_damage, 99);
        assert_eq!(state.monsters[0].hp, hp_before - 99);
    }

    #[test]
    fn nemesis_gains_intangible_before_runic_cube_fire_breathing() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 50;
        state.player.block = 11;
        state.player.powers.fire_breathing = 6;
        state.relics.push(Relic::RunicCube);
        state.piles.hand.clear();
        state.piles.discard_pile.clear();
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(20), WOUND_ID)];
        let actor_id = MonsterId::new(1);
        let mut actor = monster_state_for_ascension(&NEMESIS_A0, actor_id, 0);
        actor.powers.intangible = 0;
        actor.intent = MonsterIntent::AttackMultiple { damage: 6, hits: 3 };
        state.monsters = vec![actor];
        let hp_before = state.monsters[0].hp;
        let mut skip_ritual_tick = Vec::new();

        execute_generic_monster_intent(&mut state, actor_id, 0, 0, &[], &mut skip_ritual_tick)
            .expect("Nemesis tri-attack");

        assert_eq!(state.monsters[0].powers.intangible, 1);
        assert_eq!(state.monsters[0].hp, hp_before - 1);
        assert!(state
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == WOUND_ID));
    }

    #[test]
    fn lethal_combust_clears_block_without_damaging_the_monster() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 1;
        state.player.block = 10;
        state.player.powers.combust = 2;
        state.player.powers.combust_damage = 10;
        let monster_hp = state.monsters[0].hp;

        let next = end_player_turn(&state).expect("supported monster intent");

        assert_eq!(next.phase, CombatPhase::Lost);
        assert_eq!(next.player.hp, 0);
        assert_eq!(next.player.block, 0);
        assert_eq!(next.monsters[0].hp, monster_hp);
    }

    #[test]
    fn dagger_explode_attacks_then_loses_all_hp_without_next_roll() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state_for_ascension(&DAGGER_A0, actor_id, 0)];
        state.monsters[0].content_id = DAGGER_ID;
        state.monsters[0].hp = 20;
        state.monsters[0].max_hp = 20;
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 25 };
        state.monsters[0].move_history = vec![1, 2];
        state.rng.monster_rng = StsRng::new(11);
        let player_hp = state.player.hp;

        run_monster_turn(&mut state).expect("supported monster intent");

        assert_eq!(state.player.hp, player_hp - 25);
        assert_eq!(state.monsters[0].hp, 0);
        assert!(!state.monsters[0].alive);
        assert_eq!(state.monsters[0].block, 0);
        assert_eq!(state.monsters[0].move_history, vec![1, 2]);
        assert_eq!(state.rng.monster_rng.counter(), 0);
    }

    #[test]
    fn dagger_explode_skips_suicide_when_the_hit_kills_the_player() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.player.hp = 15;
        state.player.block = 0;
        state.monsters = vec![monster_state_for_ascension(&DAGGER_A0, actor_id, 0)];
        state.monsters[0].content_id = DAGGER_ID;
        state.monsters[0].hp = 22;
        state.monsters[0].max_hp = 22;
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 25 };
        let mut skip_ritual_tick = Vec::new();

        let disposition =
            execute_generic_monster_intent(&mut state, actor_id, 0, 0, &[], &mut skip_ritual_tick)
                .expect("lethal dagger explode");

        assert!(matches!(disposition, ActorTurnDisposition::StopPlayerDead));
        assert_eq!(state.player.hp, 0);
        assert_eq!(state.monsters[0].hp, 22);
        assert!(state.monsters[0].alive);
    }

    #[test]
    fn exploder_unknown_move_deals_explosive_damage_and_dies_without_next_roll() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state_for_ascension(&EXPLODER_A0, actor_id, 0)];
        state.monsters[0].intent = crate::MonsterIntent::Stun;
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![1, 1, 2];
        state.rng.monster_rng = StsRng::new(12);
        let player_hp = state.player.hp;

        run_monster_turn(&mut state).expect("supported monster intent");

        assert_eq!(state.player.hp, player_hp - 30);
        assert_eq!(state.monsters[0].hp, 0);
        assert!(!state.monsters[0].alive);
        assert_eq!(state.monsters[0].block, 0);
        assert_eq!(state.monsters[0].powers.explosive, 0);
        assert_eq!(state.monsters[0].move_history, vec![1, 1, 2]);
        assert_eq!(state.rng.monster_rng.counter(), 0);
    }

    #[test]
    fn dagger_suicide_roll_keeps_explode_intent_when_other_monster_lives() {
        let actor_id = MonsterId::new(1);
        let other_id = MonsterId::new(2);
        let mut state = CombatState::initial_fixture();
        let mut dagger = monster_state_for_ascension(&DAGGER_A0, actor_id, 0);
        dagger.alive = false;
        dagger.hp = 0;
        dagger.intent = MonsterIntent::Attack {
            damage: DAGGER_EXPLODE_DAMAGE,
        };
        dagger.moves_executed = 2;
        dagger.move_history = vec![1, 2, 1];
        let other = monster_state_for_ascension(&DAGGER_A0, other_id, 0);
        state.monsters = vec![dagger, other];
        state.rng.monster_rng = StsRng::new(13);

        prepare_next_intent_for_actor(&mut state, actor_id).expect("Dagger roll is supported");

        assert_eq!(
            state.monsters[0].intent,
            MonsterIntent::Attack {
                damage: DAGGER_EXPLODE_DAMAGE
            }
        );
        assert_eq!(state.monsters[0].move_history, vec![1, 2, 1, 2]);
        assert_eq!(state.rng.monster_rng.counter(), 1);
    }
}
