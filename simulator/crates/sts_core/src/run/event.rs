use crate::{
    card::{CardRarity, CardType},
    combat::initialize_combat_piles_with_relics,
    content::cards::{
        card_instance_is_upgradeable, get_card_definition, is_basic_starter_card,
        is_curse_content_id, upgrade_card_instance, APPARITION_ID, ASCENDERS_BANE_ID, BASH_ID,
        BASH_PLUS_ID, BITE_ID, CURSE_OF_THE_BELL_ID, DECAY_ID, DEFEND_R_ID, DEFEND_R_PLUS_ID,
        DOUBT_ID, INJURY_ID, JAX_ID, MADNESS_ID, NORMALITY_ID, PAIN_ID, PARASITE_ID, REGRET_ID,
        RITUAL_DAGGER_ID, SHAME_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID, WRITHE_ID,
    },
    content::{
        monsters::{
            monster_state_for_ascension, record_target_move,
            target_monster_hp_range_for_content_id, MonsterDefinition, BANDIT_BEAR_A0,
            BANDIT_LEADER_A0, BANDIT_POINTY_A0, FUNGI_BEAST_A0, GREMLIN_NOB_A0, GUARDIAN_A0,
            HEXAGHOST_A0, LAGAVULIN_A0, ORB_WALKER_A0, SENTRY_A0, SLAVER_BLUE_A0, SLAVER_RED_A0,
            SLIME_BOSS_A0, TASKMASTER_A0,
        },
        reward_pool::{random_normal_curse, IRONCLAD_REWARD_ENTRIES},
        shop_pool::{
            colorless_match_and_keep_pool, random_colorless_from_pool, shop_card_content_id,
        },
    },
    ids::ContentId,
    relic::{Relic, RelicKey, RelicTier},
    rng::{JavaRng, StsRng},
    run::{
        grid::{
            open_bonfire_elementals_grid, open_designer_remove_and_upgrade_grid,
            open_event_obtain_card_return_to_event_grid, open_event_remove_grid,
            open_event_remove_return_to_event_grid, open_event_transform_return_to_event_grid,
            open_event_upgrade_return_to_event_grid, open_falling_card_grid,
        },
        map::{apply_initial_monster_ai_rolls, enter_secret_portal_boss_combat},
        neow::{
            apply_neow_boss_swap, apply_neow_curse_drawback, apply_neow_lament_reward,
            apply_neow_relic_reward, apply_neow_simple_drawback, apply_neow_simple_reward,
            generate_neow_card_reward, generate_neow_colorless_reward_with_card_rng_counter,
            generate_neow_options, generate_neow_three_potions, open_neow_reward_grid,
            GeneratedNeowOption, NeowDrawback, NeowRewardType,
        },
        reward::{
            reward_card_choice_count, roll_event_relic_reward, roll_relic_reward,
            target_colorless_card_reward_choices_with_count, target_elite_relic_tier,
            target_library_card_choices, target_random_potion, target_uniform_random_potion,
        },
        state::RunRngStream,
    },
    CardId, CardInstance, CombatState, EventAction, MonsterId, RewardScreen, RunPhase, RunState,
    SimError, SimResult,
};

pub const SCRAP_OOZE_REACH_HP_LOSS: i32 = 3;
pub const SCRAP_OOZE_DEEPER_HP_LOSS: i32 = 4;
pub const WING_STATUE_PRAY_HP_LOSS: i32 = 7;
pub const WING_STATUE_REQUIRED_DAMAGE: i32 = 10;
pub const WING_STATUE_MIN_GOLD: i32 = 50;
pub const WING_STATUE_MAX_GOLD: i32 = 80;
pub const WHEEL_OF_CHANGE_GOLD_ACT1: i32 = 100;
pub const WHEEL_OF_CHANGE_GOLD_ACT2: i32 = 200;
pub const WHEEL_OF_CHANGE_GOLD_ACT3: i32 = 300;
pub const WHEEL_OF_CHANGE_HP_LOSS_PERCENT: f32 = 0.10;
pub const WHEEL_OF_CHANGE_A15_HP_LOSS_PERCENT: f32 = 0.15;
use serde::{Deserialize, Serialize};

pub const GOLDEN_SHRINE_GOLD: i32 = 100;
pub const GOLDEN_SHRINE_A15_GOLD: i32 = 50;
pub const GOLDEN_SHRINE_DESECRATE_GOLD: i32 = 275;
pub const WORLD_OF_GOOP_DAMAGE: i32 = 11;
pub const WORLD_OF_GOOP_GOLD: i32 = 75;
const WE_MEET_AGAIN_NO_OPTION: u8 = u8::MAX;
pub const WORLD_OF_GOOP_MIN_GOLD_LOSS: i32 = 20;
pub const WORLD_OF_GOOP_MAX_GOLD_LOSS: i32 = 50;
pub const WORLD_OF_GOOP_A15_MIN_GOLD_LOSS: i32 = 35;
pub const WORLD_OF_GOOP_A15_MAX_GOLD_LOSS: i32 = 75;
pub const GOLDEN_IDOL_HP_LOSS_PERCENT: f32 = 0.25;
pub const GOLDEN_IDOL_MAX_HP_LOSS_PERCENT: f32 = 0.08;
pub const GOLDEN_IDOL_A15_HP_LOSS_PERCENT: f32 = 0.35;
pub const GOLDEN_IDOL_A15_MAX_HP_LOSS_PERCENT: f32 = 0.10;
pub const SSSSSERPENT_GOLD: i32 = 175;
pub const FACE_TRADER_GOLD: i32 = 75;
pub const FACE_TRADER_A15_GOLD: i32 = 50;
pub const BIG_FISH_MAX_HP_GAIN: i32 = 5;
pub const SHINING_LIGHT_HP_PERCENT: f32 = 0.20;
pub const THE_LIBRARY_HEAL_PERCENT: f32 = 0.33;
pub const THE_LIBRARY_A15_HEAL_PERCENT: f32 = 0.20;
pub const THE_LIBRARY_READ_CARD_COUNT: usize = 20;
pub const MAUSOLEUM_A15_CURSE_CHANCE: i32 = 100;
pub const MAUSOLEUM_CURSE_CHANCE: i32 = 50;
pub const VAMPIRES_HP_LOSS_PERCENT: f32 = 0.30;
pub const VAMPIRES_BITE_COUNT: usize = 5;
pub const CURSED_TOME_PAGE_1_HP_LOSS: i32 = 1;
pub const CURSED_TOME_PAGE_2_HP_LOSS: i32 = 2;
pub const CURSED_TOME_PAGE_3_HP_LOSS: i32 = 3;
pub const CURSED_TOME_STOP_HP_LOSS: i32 = 3;
pub const CURSED_TOME_FINAL_HP_LOSS: i32 = 10;
pub const CURSED_TOME_A15_FINAL_HP_LOSS: i32 = 15;
pub const NEST_HP_LOSS: i32 = 6;
pub const NEST_GOLD_GAIN: i32 = 99;
pub const NEST_A15_GOLD_GAIN: i32 = 50;
pub const BEGGAR_GOLD_COST: i32 = 75;
pub const ADDICT_GOLD_COST: i32 = 85;
pub const FORGOTTEN_ALTAR_MAX_HP_GAIN: i32 = 5;
pub const FORGOTTEN_ALTAR_HP_LOSS_PERCENT: f32 = 0.25;
pub const FORGOTTEN_ALTAR_A15_HP_LOSS_PERCENT: f32 = 0.35;
pub const GHOSTS_MAX_HP_LOSS_PERCENT: f32 = 0.50;
pub const GHOSTS_APPARITION_COUNT: usize = 5;
pub const GHOSTS_A15_APPARITION_COUNT: usize = 3;
pub const DRUG_DEALER_TRANSFORM_COUNT: u8 = 2;
pub const KNOWING_SKULL_STARTING_COST: i32 = 6;
pub const KNOWING_SKULL_GOLD_REWARD: i32 = 90;
pub const SHRINE_CHANCE: f32 = 0.25;

#[must_use]
pub fn golden_shrine_gold(ascension: u8) -> i32 {
    if ascension >= 15 {
        GOLDEN_SHRINE_A15_GOLD
    } else {
        GOLDEN_SHRINE_GOLD
    }
}

#[must_use]
pub fn shining_light_hp_loss(max_hp: i32) -> i32 {
    (max_hp as f32 * SHINING_LIGHT_HP_PERCENT).round() as i32
}

#[must_use]
pub fn the_library_heal(max_hp: i32) -> i32 {
    (max_hp as f32 * THE_LIBRARY_HEAL_PERCENT).round() as i32
}

#[must_use]
pub fn the_library_heal_for_ascension(max_hp: i32, ascension: u8) -> i32 {
    let percent = if ascension >= 15 {
        THE_LIBRARY_A15_HEAL_PERCENT
    } else {
        THE_LIBRARY_HEAL_PERCENT
    };
    (max_hp as f32 * percent).round() as i32
}

#[must_use]
pub fn golden_idol_hp_loss(max_hp: i32, ascension: u8) -> i32 {
    let percent = if ascension >= 15 {
        GOLDEN_IDOL_A15_HP_LOSS_PERCENT
    } else {
        GOLDEN_IDOL_HP_LOSS_PERCENT
    };
    (max_hp as f32 * percent) as i32
}

#[must_use]
pub fn golden_idol_max_hp_loss(max_hp: i32, ascension: u8) -> i32 {
    let percent = if ascension >= 15 {
        GOLDEN_IDOL_A15_MAX_HP_LOSS_PERCENT
    } else {
        GOLDEN_IDOL_MAX_HP_LOSS_PERCENT
    };
    (max_hp as f32 * percent) as i32
}

fn open_the_library_read_grid(run: &mut RunState) {
    let mut card_rng = run.rng_for_stream(RunRngStream::CardReward);
    let next_card_id = run.next_card_instance_id();
    let choices = target_library_card_choices(
        &mut card_rng,
        run.card_rarity_factor,
        next_card_id,
        THE_LIBRARY_READ_CARD_COUNT,
    );
    run.store_rng_counter(RunRngStream::CardReward, &card_rng);
    open_event_obtain_card_return_to_event_grid(run, Event::TheLibrary, choices);
}

fn roll_mausoleum_curses_player(run: &mut RunState) -> bool {
    if run.ascension >= 15 {
        return true;
    }

    let mut misc_rng = StsRng::with_counter(run.misc_rng_seed as i64, run.misc_rng_counter);
    let cursed = misc_rng.random_bool();
    run.misc_rng_counter = misc_rng.counter();
    cursed
}

#[must_use]
pub fn vampires_max_hp_loss(max_hp: i32) -> i32 {
    let loss = (max_hp as f32 * VAMPIRES_HP_LOSS_PERCENT).ceil() as i32;
    loss.min(max_hp.saturating_sub(1))
}

fn replace_starter_strikes_with_bites(run: &mut RunState) {
    run.deck
        .retain(|card| !matches!(card.content_id, STRIKE_R_ID | STRIKE_R_PLUS_ID));
    for _ in 0..VAMPIRES_BITE_COUNT {
        run.gain_deck_card(BITE_ID);
    }
}

#[must_use]
pub fn cursed_tome_final_hp_loss(ascension: u8) -> i32 {
    if ascension >= 15 {
        CURSED_TOME_A15_FINAL_HP_LOSS
    } else {
        CURSED_TOME_FINAL_HP_LOSS
    }
}

#[must_use]
pub fn nest_gold_gain(ascension: u8) -> i32 {
    if ascension >= 15 {
        NEST_A15_GOLD_GAIN
    } else {
        NEST_GOLD_GAIN
    }
}

#[must_use]
pub fn forgotten_altar_hp_loss(max_hp: i32, ascension: u8) -> i32 {
    let percent = if ascension >= 15 {
        FORGOTTEN_ALTAR_A15_HP_LOSS_PERCENT
    } else {
        FORGOTTEN_ALTAR_HP_LOSS_PERCENT
    };
    (max_hp as f32 * percent).round() as i32
}

#[must_use]
pub fn ghosts_max_hp_loss(max_hp: i32) -> i32 {
    let loss = (max_hp as f32 * GHOSTS_MAX_HP_LOSS_PERCENT).ceil() as i32;
    loss.min(max_hp.saturating_sub(1))
}

#[must_use]
pub fn ghosts_apparition_count(ascension: u8) -> usize {
    if ascension >= 15 {
        GHOSTS_A15_APPARITION_COUNT
    } else {
        GHOSTS_APPARITION_COUNT
    }
}

fn cursed_tome_choices(stage: u8, ascension: u8) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Read", "Leave"]),
        1..=3 => labeled_choices(&["Continue"]),
        4 => vec![
            EventChoice {
                label: format!("Take (lose {} HP)", cursed_tome_final_hp_loss(ascension)),
            },
            EventChoice {
                label: "Stop".to_owned(),
            },
        ],
        5 => labeled_choices(&["Leave"]),
        _ => Vec::new(),
    }
}

fn nest_choices(stage: u8, ascension: u8) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Continue"]),
        1 => vec![
            EventChoice {
                label: format!("Smash and grab (gain {} gold)", nest_gold_gain(ascension)),
            },
            EventChoice {
                label: format!("Stay in line (lose {NEST_HP_LOSS} HP)"),
            },
        ],
        2 => labeled_choices(&["Leave"]),
        _ => Vec::new(),
    }
}

fn beggar_choices(stage: u8) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Give gold", "Leave"]),
        1 => labeled_choices(&["Choose a card"]),
        2 => labeled_choices(&["Leave"]),
        _ => Vec::new(),
    }
}

fn addict_choices(stage: u8) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Offer gold", "Rob", "Leave"]),
        1 => labeled_choices(&["Leave"]),
        _ => Vec::new(),
    }
}

fn forgotten_altar_choices(stage: u8, has_golden_idol: bool) -> Vec<EventChoice> {
    match stage {
        0 if has_golden_idol => labeled_choices(&["Offer", "Sacrifice", "Desecrate"]),
        0 => labeled_choices(&["Sacrifice", "Desecrate"]),
        1 => labeled_choices(&["Leave"]),
        _ => Vec::new(),
    }
}

fn ghosts_choices(stage: u8, max_hp: i32) -> Vec<EventChoice> {
    match stage {
        0 => vec![
            EventChoice {
                label: format!("Accept (lose {} max HP)", ghosts_max_hp_loss(max_hp)),
            },
            EventChoice {
                label: "Refuse".to_owned(),
            },
        ],
        1 => labeled_choices(&["Leave"]),
        _ => Vec::new(),
    }
}

fn masked_bandits_choices(stage: u8) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Pay", "Fight"]),
        1 | 2 => labeled_choices(&["Continue"]),
        3 => labeled_choices(&["Leave"]),
        _ => Vec::new(),
    }
}

fn colosseum_choices(stage: u8) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Continue"]),
        1 => labeled_choices(&["Fight"]),
        2 => labeled_choices(&["Flee", "Fight Nobs"]),
        _ => Vec::new(),
    }
}

fn drug_dealer_choices(stage: u8, transform_enabled: bool) -> Vec<EventChoice> {
    match stage {
        0 => vec![
            EventChoice {
                label: "Test J.A.X.".to_owned(),
            },
            EventChoice {
                label: if transform_enabled {
                    "Become test subject".to_owned()
                } else {
                    "Become test subject (requires 2 cards)".to_owned()
                },
            },
            EventChoice {
                label: "Ingest mutagens".to_owned(),
            },
        ],
        1 => labeled_choices(&["Leave"]),
        _ => Vec::new(),
    }
}

fn knowing_skull_choices(stage: u32, event_data: u32) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Continue"]),
        1 => {
            let costs = knowing_skull_costs(event_data);
            vec![
                EventChoice {
                    label: format!("A Pick Me Up? (lose {} HP)", costs.potion),
                },
                EventChoice {
                    label: format!(
                        "Riches? (gain {KNOWING_SKULL_GOLD_REWARD} gold, lose {} HP)",
                        costs.gold
                    ),
                },
                EventChoice {
                    label: format!("Success? (obtain colorless card, lose {} HP)", costs.card),
                },
                EventChoice {
                    label: format!("How do I leave? (lose {} HP)", costs.leave),
                },
            ]
        }
        2 => labeled_choices(&["Leave"]),
        _ => Vec::new(),
    }
}

fn dead_adventurer_choices(stage: u8, encounter_chance: i32) -> Vec<EventChoice> {
    match stage {
        0 => vec![
            EventChoice {
                label: format!("Search ({encounter_chance}%: monster returns)"),
            },
            EventChoice {
                label: "Leave".to_owned(),
            },
        ],
        1 => labeled_choices(&["Leave"]),
        2 => labeled_choices(&["Continue", "Leave"]),
        3 => labeled_choices(&["Fight"]),
        _ => Vec::new(),
    }
}

fn dead_adventurer_event_data(order: [u8; 3], enemy: u8, attempts: u8) -> u32 {
    u32::from(order[0])
        | (u32::from(order[1]) << 2)
        | (u32::from(order[2]) << 4)
        | (u32::from(enemy) << 6)
        | (u32::from(attempts) << 8)
}

fn dead_adventurer_order(event_data: u32) -> [u8; 3] {
    if event_data == 0 {
        [0, 1, 2]
    } else {
        [
            (event_data & 0b11) as u8,
            ((event_data >> 2) & 0b11) as u8,
            ((event_data >> 4) & 0b11) as u8,
        ]
    }
}

fn dead_adventurer_enemy(event_data: u32) -> u8 {
    ((event_data >> 6) & 0b11) as u8
}

fn dead_adventurer_attempts(event_data: u32) -> u8 {
    ((event_data >> 8) & 0b11) as u8
}

const DEAD_ADVENTURER_PENDING_ENCOUNTER: u32 = 1 << 10;

fn dead_adventurer_pending_encounter(event_data: u32) -> bool {
    event_data & DEAD_ADVENTURER_PENDING_ENCOUNTER != 0
}

fn dead_adventurer_encounter_chance(run: &RunState, attempts: u8) -> i32 {
    let starting_chance = if run.ascension >= 15 { 35 } else { 25 };
    starting_chance + i32::from(attempts) * 25
}

fn roll_dead_adventurer_event_data(run: &mut RunState) -> u32 {
    let mut rng = run.rng_for_stream(RunRngStream::Misc);
    let shuffle_seed = rng.random_long();
    let enemy = rng.random_int_range(0, 2) as u8;
    run.store_rng_counter(RunRngStream::Misc, &rng);

    let mut order = [0_u8, 1, 2];
    JavaRng::new(shuffle_seed).collections_shuffle(&mut order);
    dead_adventurer_event_data(order, enemy, 0)
}

fn dead_adventurer_screen(run: &RunState, stage: u8, event_data: u32) -> EventScreen {
    let attempts = dead_adventurer_attempts(event_data);
    let encounter_chance = dead_adventurer_encounter_chance(run, attempts);
    let mut choices = dead_adventurer_choices(stage, encounter_chance);
    if stage == 0 && attempts > 0 {
        choices[0].label = format!("Continue ({encounter_chance}%: monster returns)");
    }
    EventScreen {
        event: Event::DeadAdventurer,
        choices,
        stage: u32::from(stage),
        event_data,
    }
}

fn hypnotizing_colored_mushrooms_choices(stage: u32) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Stomp", "Eat"]),
        _ => labeled_choices(&["Leave"]),
    }
}

fn nloth_owned_relic_keys(run: &RunState) -> Vec<RelicKey> {
    run.relics
        .iter()
        .map(|relic| relic.key())
        .chain(run.relic_keys.iter().copied())
        .collect()
}

fn nloth_event_data(choice_one: usize, choice_two: usize) -> u32 {
    (choice_one as u32) | ((choice_two as u32) << 8)
}

fn nloth_choice_index(event_data: u32, choice: usize) -> usize {
    if choice == 0 {
        (event_data & 0xff) as usize
    } else {
        ((event_data >> 8) & 0xff) as usize
    }
}

fn nloth_choices(run: &RunState, stage: u32, event_data: u32) -> Vec<EventChoice> {
    if stage > 0 {
        return labeled_choices(&["Leave"]);
    }
    let owned = nloth_owned_relic_keys(run);
    let first = owned
        .get(nloth_choice_index(event_data, 0))
        .map(|key| format!("Trade {key:?}"))
        .unwrap_or_else(|| "Trade relic".to_owned());
    let second = owned
        .get(nloth_choice_index(event_data, 1))
        .map(|key| format!("Trade {key:?}"))
        .unwrap_or_else(|| "Trade relic".to_owned());
    labeled_choices(&[&first, &second, "Leave"])
}

fn roll_nloth_event_data(run: &mut RunState) -> u32 {
    let owned = nloth_owned_relic_keys(run);
    let mut indices = (0..owned.len()).collect::<Vec<_>>();
    let mut rng = run.rng_for_stream(RunRngStream::Misc);
    let shuffle_seed = rng.random_long();
    run.store_rng_counter(RunRngStream::Misc, &rng);
    JavaRng::new(shuffle_seed).collections_shuffle(&mut indices);
    nloth_event_data(indices[0], indices[1])
}

fn vampires_choices(has_blood_vial: bool) -> Vec<EventChoice> {
    if has_blood_vial {
        labeled_choices(&["Accept", "Give Blood Vial", "Refuse"])
    } else {
        labeled_choices(&["Accept", "Refuse"])
    }
}

fn lose_event_hp(run: &mut RunState, amount: i32) {
    run.player_hp = (run.player_hp - amount).max(0);
}

#[derive(Debug, Clone, Copy)]
struct KnowingSkullCosts {
    potion: i32,
    gold: i32,
    card: i32,
    leave: i32,
}

fn knowing_skull_costs(event_data: u32) -> KnowingSkullCosts {
    if event_data == 0 {
        return KnowingSkullCosts {
            potion: KNOWING_SKULL_STARTING_COST,
            gold: KNOWING_SKULL_STARTING_COST,
            card: KNOWING_SKULL_STARTING_COST,
            leave: KNOWING_SKULL_STARTING_COST,
        };
    }

    KnowingSkullCosts {
        potion: (event_data & 0xff) as i32,
        gold: ((event_data >> 8) & 0xff) as i32,
        card: ((event_data >> 16) & 0xff) as i32,
        leave: ((event_data >> 24) & 0xff) as i32,
    }
}

fn knowing_skull_event_data(costs: KnowingSkullCosts) -> u32 {
    (costs.potion as u32)
        | ((costs.gold as u32) << 8)
        | ((costs.card as u32) << 16)
        | ((costs.leave as u32) << 24)
}

fn knowing_skull_gain_random_colorless(run: &mut RunState) {
    let mut card_rng = run.rng_for_stream(RunRngStream::CardReward);
    let content_id = random_colorless_from_pool(&mut card_rng, CardRarity::Uncommon);
    run.store_rng_counter(RunRngStream::CardReward, &card_rng);
    run.gain_deck_card(content_id);
}

fn knowing_skull_gain_random_potion(run: &mut RunState) {
    if !run.can_gain_potions() {
        return;
    }
    let mut potion_rng = run.rng_for_stream(RunRngStream::Potion);
    let potion = target_random_potion(&mut potion_rng);
    run.store_rng_counter(RunRngStream::Potion, &potion_rng);
    run.gain_potion(potion)
        .expect("potion gain validated before Knowing Skull potion reward");
}

fn purgeable_event_card_count(run: &RunState) -> usize {
    run.deck.iter().filter(|card| !card.bottled).count()
}

fn cleric_purify_cost(run: &RunState) -> i32 {
    if run.ascension >= 15 {
        75
    } else {
        50
    }
}

fn has_relic_key(run: &RunState, key: RelicKey) -> bool {
    run.relics.iter().any(|relic| relic.key() == key) || run.relic_keys.contains(&key)
}

fn remove_relic_key(run: &mut RunState, key: RelicKey) -> bool {
    if let Some(index) = run.relics.iter().position(|relic| relic.key() == key) {
        run.relics.remove(index);
        return true;
    }
    if let Some(index) = run
        .relic_keys
        .iter()
        .position(|candidate| *candidate == key)
    {
        run.relic_keys.remove(index);
        return true;
    }
    false
}

fn give_forgotten_altar_idol(run: &mut RunState) -> SimResult<()> {
    if !remove_relic_key(run, RelicKey::GoldenIdol) {
        return Err(SimError::IllegalAction(
            "Forgotten Altar Give Idol requires Golden Idol",
        ));
    }
    if has_relic_key(run, RelicKey::BloodyIdol) {
        run.gain_relic_key(RelicKey::Circlet);
    } else {
        run.gain_relic_key(RelicKey::BloodyIdol);
    }
    Ok(())
}

fn choose_cursed_tome_book(run: &mut RunState) -> RelicKey {
    let mut possible_books = [
        RelicKey::Necronomicon,
        RelicKey::Enchiridion,
        RelicKey::NilrysCodex,
    ]
    .into_iter()
    .filter(|key| !has_relic_key(run, *key))
    .collect::<Vec<_>>();

    if possible_books.is_empty() {
        possible_books.push(RelicKey::Circlet);
    }

    let mut misc_rng = StsRng::with_counter(run.misc_rng_seed as i64, run.misc_rng_counter);
    let index = misc_rng.random_int(possible_books.len() as i32 - 1) as usize;
    run.misc_rng_counter = misc_rng.counter();
    possible_books[index]
}

fn open_cursed_tome_book_reward(run: &mut RunState, key: RelicKey) {
    let relic_offer = Relic::from_key(key);
    run.phase = RunPhase::Reward;
    // The target returns from the book relic screen to Cursed Tome's final
    // Leave button. Keep that event continuation while the reward is open.
    run.event = Some(EventScreen {
        event: Event::CursedTome,
        choices: cursed_tome_choices(5, run.ascension),
        stage: 5,
        event_data: 0,
    });
    run.reward = Some(RewardScreen {
        continuation: crate::RewardContinuation::None,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer,
        relic_key_offer: if relic_offer.is_some() {
            None
        } else {
            Some(key)
        },
        pending_relic_offer: None,
        pending_relic_key_offer: None,
        queued_relic_key_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_active: false,
        card_reward_pending: false,
        pending_card_reward_count: 0,
    });
}

fn upgrade_random_deck_cards(run: &mut RunState, max_count: usize) {
    let mut upgradeable: Vec<usize> = run
        .deck
        .iter()
        .enumerate()
        .filter_map(|(index, card)| card_instance_is_upgradeable(card).then_some(index))
        .collect();
    if upgradeable.is_empty() {
        return;
    }

    let mut misc_rng = StsRng::with_counter(run.misc_rng_seed as i64, run.misc_rng_counter);
    let shuffle_seed = misc_rng.random_long();
    run.misc_rng_counter = misc_rng.counter();

    JavaRng::new(shuffle_seed).collections_shuffle(&mut upgradeable);

    for index in upgradeable.into_iter().take(max_count) {
        let upgraded_card = upgrade_card_instance(run.deck[index])
            .expect("upgradeable card validated before shuffle");
        run.deck[index] = upgraded_card;
    }
}

fn upgrade_starter_strikes_and_defends(run: &mut RunState) {
    for card in &mut run.deck {
        if matches!(card.content_id, STRIKE_R_ID | DEFEND_R_ID) {
            if let Some(upgraded) = upgrade_card_instance(*card) {
                *card = upgraded;
            }
        }
    }
}

const ACT1_EVENTS: [Event; 11] = [
    Event::BigFish,
    Event::TheCleric,
    Event::DeadAdventurer,
    Event::GoldenIdol,
    Event::WingStatue,
    Event::WorldOfGoop,
    Event::TheSsssserpent,
    Event::LivingWall,
    Event::HypnotizingColoredMushrooms,
    Event::ScrapOoze,
    Event::ShiningLight,
];

const ACT1_SHRINES: [Event; 6] = [
    Event::MatchAndKeep,
    Event::GoldenShrine,
    Event::Transmorgrifier,
    Event::Purifier,
    Event::UpgradeShrine,
    Event::WheelOfChange,
];

pub const ACT2_EVENTS: [Event; 13] = [
    Event::Addict,
    Event::BackToBasics,
    Event::Beggar,
    Event::Colosseum,
    Event::CursedTome,
    Event::DrugDealer,
    Event::ForgottenAltar,
    Event::Ghosts,
    Event::MaskedBandits,
    Event::Nest,
    Event::TheLibrary,
    Event::TheMausoleum,
    Event::Vampires,
];

pub const ACT2_SHRINES: [Event; 6] = [
    Event::MatchAndKeep,
    Event::WheelOfChange,
    Event::GoldenShrine,
    Event::Transmorgrifier,
    Event::Purifier,
    Event::UpgradeShrine,
];

pub const ACT3_EVENTS: [Event; 7] = [
    Event::Falling,
    Event::MindBloom,
    Event::MoaiHead,
    Event::MysteriousSphere,
    Event::SensoryStone,
    Event::TombOfLordRedMask,
    Event::WindingHalls,
];

pub const ACT3_SHRINES: [Event; 6] = [
    Event::MatchAndKeep,
    Event::WheelOfChange,
    Event::GoldenShrine,
    Event::Transmorgrifier,
    Event::Purifier,
    Event::UpgradeShrine,
];

const SPECIAL_ONE_TIME_EVENTS_PREFIX: [Event; 9] = [
    Event::AccursedBlacksmith,
    Event::BonfireElementals,
    Event::Designer,
    Event::Duplicator,
    Event::FaceTrader,
    Event::FountainOfCleansing,
    Event::KnowingSkull,
    Event::Lab,
    Event::Nloth,
];

const SPECIAL_ONE_TIME_EVENTS_SUFFIX: [Event; 4] = [
    Event::SecretPortal,
    Event::TheJoust,
    Event::WeMeetAgain,
    Event::TheWomanInBlue,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Neow,
    SpireHeart,
    AccursedBlacksmith,
    BonfireElementals,
    Designer,
    Duplicator,
    FountainOfCleansing,
    GoldenShrine,
    BigFish,
    TheCleric,
    DeadAdventurer,
    GoldenIdol,
    WingStatue,
    WorldOfGoop,
    TheSsssserpent,
    LivingWall,
    HypnotizingColoredMushrooms,
    ScrapOoze,
    ShiningLight,
    FaceTrader,
    Nloth,
    NoteForYourself,
    SecretPortal,
    TheJoust,
    WeMeetAgain,
    TheWomanInBlue,
    Transmorgrifier,
    Purifier,
    UpgradeShrine,
    WheelOfChange,
    MatchAndKeep,
    Addict,
    BackToBasics,
    Beggar,
    Colosseum,
    CursedTome,
    DrugDealer,
    ForgottenAltar,
    Ghosts,
    KnowingSkull,
    MaskedBandits,
    Nest,
    TheLibrary,
    TheMausoleum,
    Vampires,
    Lab,
    Falling,
    MindBloom,
    MoaiHead,
    MysteriousSphere,
    SensoryStone,
    TombOfLordRedMask,
    WindingHalls,
}

fn spire_heart_choices(stage: u32) -> Vec<EventChoice> {
    let label = match stage {
        0 | 2 => "Continue",
        1 => "Attack",
        _ => "Sleep",
    };
    labeled_choices(&[label])
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventChoice {
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventScreen {
    pub event: Event,
    pub choices: Vec<EventChoice>,
    #[serde(default)]
    pub stage: u32,
    #[serde(default)]
    pub event_data: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchAndKeepState {
    pub cards: Vec<MatchAndKeepCard>,
    pub attempts_remaining: u8,
    pub first_flipped_index: Option<usize>,
    #[serde(default)]
    pub second_flipped_index: Option<usize>,
    #[serde(default)]
    pub matched_cards: Vec<ContentId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchAndKeepCard {
    pub content_id: ContentId,
    #[serde(default)]
    pub revealed: bool,
    #[serde(default)]
    pub matched: bool,
}

fn scrap_ooze_choices(stage: u32) -> Vec<EventChoice> {
    match stage {
        0 => vec![
            EventChoice {
                label: "Reach Inside".to_owned(),
            },
            EventChoice {
                label: "Leave".to_owned(),
            },
        ],
        1 => vec![
            EventChoice {
                label: "Deeper".to_owned(),
            },
            EventChoice {
                label: "Leave".to_owned(),
            },
        ],
        _ => vec![EventChoice {
            label: "Leave".to_owned(),
        }],
    }
}

fn big_fish_choices(stage: u32) -> Vec<EventChoice> {
    if stage == 0 {
        vec![
            EventChoice {
                label: "Banana".to_owned(),
            },
            EventChoice {
                label: "Donut".to_owned(),
            },
            EventChoice {
                label: "Box".to_owned(),
            },
        ]
    } else {
        vec![EventChoice {
            label: "Leave".to_owned(),
        }]
    }
}

fn sssssserpent_choices(stage: u32) -> Vec<EventChoice> {
    match stage {
        0 => vec![
            EventChoice {
                label: "Agree".to_owned(),
            },
            EventChoice {
                label: "Disagree".to_owned(),
            },
        ],
        1 => vec![EventChoice {
            label: "Continue".to_owned(),
        }],
        _ => vec![EventChoice {
            label: "Leave".to_owned(),
        }],
    }
}

fn fountain_of_cleansing_choices(stage: u32) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Drink", "Leave"]),
        _ => labeled_choices(&["Leave"]),
    }
}

fn face_trader_choices(stage: u32) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Continue"]),
        1 => labeled_choices(&["Touch", "Trade", "Leave"]),
        _ => labeled_choices(&["Leave"]),
    }
}

fn note_for_yourself_choices(stage: u32) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Continue"]),
        1 => labeled_choices(&["Take and Give", "Ignore"]),
        _ => labeled_choices(&["Leave"]),
    }
}

fn note_card_for_run(run: &RunState) -> CardInstance {
    let mut card = CardInstance::new(
        CardId::new(run.next_card_instance_id()),
        run.note_card_content_id,
    );
    for _ in 0..run.note_card_upgrades {
        if let Some(upgraded) = upgrade_card_instance(card) {
            card = upgraded;
        }
    }
    card
}

fn note_for_yourself_choices_for_run(run: &RunState, stage: u32) -> Vec<EventChoice> {
    if stage != 1 {
        return note_for_yourself_choices(stage);
    }
    let card_name = get_card_definition(note_card_for_run(run).content_id)
        .map_or("the saved card", |definition| definition.name);
    vec![
        EventChoice {
            label: format!("Take and Give ({card_name})"),
        },
        EventChoice {
            label: "Ignore".to_owned(),
        },
    ]
}

fn falling_card_types(run: &RunState) -> Vec<CardType> {
    [CardType::Skill, CardType::Power, CardType::Attack]
        .into_iter()
        .filter(|card_type| {
            run.deck.iter().any(|card| {
                !card.bottled
                    && get_card_definition(card.content_id)
                        .is_some_and(|definition| definition.card_type == *card_type)
            })
        })
        .collect()
}

fn falling_choices(run: &RunState, stage: u32) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Continue"]),
        1 => {
            let labels = falling_card_types(run)
                .into_iter()
                .map(|card_type| match card_type {
                    CardType::Skill => "Remove a Skill",
                    CardType::Power => "Remove a Power",
                    CardType::Attack => "Remove an Attack",
                    CardType::Status => "Leave",
                })
                .collect::<Vec<_>>();
            if labels.is_empty() {
                labeled_choices(&["Leave"])
            } else {
                labels
                    .into_iter()
                    .map(str::to_owned)
                    .map(|label| EventChoice { label })
                    .collect()
            }
        }
        _ => labeled_choices(&["Leave"]),
    }
}

fn moai_choices(run: &RunState, stage: u32) -> Vec<EventChoice> {
    if stage > 0 {
        return labeled_choices(&["Leave"]);
    }
    let mut choices = vec![EventChoice {
        label: format!(
            "Lose {} max HP and heal to full",
            rounded_event_percent(
                run.player_max_hp,
                if run.ascension >= 15 { 0.18 } else { 0.125 }
            )
        ),
    }];
    if has_relic_key(run, RelicKey::GoldenIdol) {
        choices.push(EventChoice {
            label: "Give Golden Idol (gain 333 gold)".to_owned(),
        });
    }
    choices.push(EventChoice {
        label: "Leave".to_owned(),
    });
    choices
}

fn rounded_event_percent(value: i32, percent: f32) -> i32 {
    ((value as f32) * percent).round() as i32
}

fn mysterious_sphere_choices(stage: u32) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Fight", "Leave"]),
        1 => labeled_choices(&["Continue"]),
        _ => Vec::new(),
    }
}

fn winding_halls_choices(run: &RunState, stage: u32) -> Vec<EventChoice> {
    if stage == 0 {
        return labeled_choices(&["..."]);
    }
    if stage > 1 {
        return labeled_choices(&["Leave"]);
    }
    let hp_loss = rounded_event_percent(
        run.player_max_hp,
        if run.ascension >= 15 { 0.18 } else { 0.125 },
    );
    let heal = rounded_event_percent(
        run.player_max_hp,
        if run.ascension >= 15 { 0.20 } else { 0.25 },
    );
    let max_hp_loss = rounded_event_percent(run.player_max_hp, 0.05);
    vec![
        EventChoice {
            label: format!("Embrace Madness (lose {hp_loss} HP, gain 2 Madness)"),
        },
        EventChoice {
            label: format!("Focus (heal {heal}, gain Writhe)"),
        },
        EventChoice {
            label: format!("Retrace Your Steps (lose {max_hp_loss} max HP)"),
        },
    ]
}

fn mind_bloom_choices(run: &RunState) -> Vec<EventChoice> {
    vec![
        EventChoice {
            label: "I am War".to_owned(),
        },
        EventChoice {
            label: "I am Awake".to_owned(),
        },
        EventChoice {
            label: if run.current_floor % 50 <= 40 {
                "I am Rich".to_owned()
            } else {
                "I am Healthy".to_owned()
            },
        },
    ]
}

fn roll_mind_bloom_boss(run: &mut RunState) -> u8 {
    let mut misc_rng = run.rng_for_stream(RunRngStream::Misc);
    let shuffle_seed = misc_rng.random_long();
    run.store_rng_counter(RunRngStream::Misc, &misc_rng);
    let mut bosses = [0_u8, 1, 2];
    JavaRng::new(shuffle_seed).collections_shuffle(&mut bosses);
    bosses[0]
}

fn sensory_stone_choices(stage: u32) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Interact"]),
        1 => labeled_choices(&["Recall", "Recall", "Recall"]),
        _ => Vec::new(),
    }
}

fn roll_sensory_memory(run: &mut RunState) -> u32 {
    let mut misc_rng = run.rng_for_stream(RunRngStream::Misc);
    let shuffle_seed = misc_rng.random_long();
    run.store_rng_counter(RunRngStream::Misc, &misc_rng);
    let mut memories = [0_u8, 1, 2, 3];
    JavaRng::new(shuffle_seed).collections_shuffle(&mut memories);
    u32::from(memories[0])
}

fn joust_choices(stage: u32) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Continue"]),
        1 => labeled_choices(&["Bet against (50 gold)", "Bet for (50 gold)"]),
        2 | 3 => labeled_choices(&["Continue"]),
        _ => labeled_choices(&["Leave"]),
    }
}

fn woman_in_blue_choices(run: &RunState) -> Vec<EventChoice> {
    let mut choices = vec![
        EventChoice {
            label: "Buy 1 potion (20 gold)".to_owned(),
        },
        EventChoice {
            label: "Buy 2 potions (30 gold)".to_owned(),
        },
        EventChoice {
            label: "Buy 3 potions (40 gold)".to_owned(),
        },
    ];
    choices.push(EventChoice {
        label: if run.ascension >= 15 {
            format!("Get punched ({} HP)", (run.player_max_hp + 19) / 20)
        } else {
            "Leave".to_owned()
        },
    });
    choices
}

fn joust_event_data(bet_for: bool, owner_wins: bool) -> u32 {
    u32::from(bet_for) | (u32::from(owner_wins) << 1)
}

fn face_trader_gold(ascension: u8) -> i32 {
    if ascension >= 15 {
        FACE_TRADER_A15_GOLD
    } else {
        FACE_TRADER_GOLD
    }
}

fn face_trader_damage(max_hp: i32) -> i32 {
    (max_hp / 10).max(1)
}

fn golden_idol_choices(stage: u32, max_hp: i32, ascension: u8) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Take", "Leave"]),
        1 => vec![
            EventChoice {
                label: "Outrun (obtain Injury)".to_owned(),
            },
            EventChoice {
                label: format!(
                    "Smash (take {} damage)",
                    golden_idol_hp_loss(max_hp, ascension)
                ),
            },
            EventChoice {
                label: format!(
                    "Hide (lose {} max HP)",
                    golden_idol_max_hp_loss(max_hp, ascension)
                ),
            },
        ],
        2 => labeled_choices(&["Leave"]),
        _ => Vec::new(),
    }
}

fn world_of_goop_choices(stage: u32, gold_loss: i32) -> Vec<EventChoice> {
    match stage {
        0 => vec![
            EventChoice {
                label: format!(
                    "Gather gold (gain {WORLD_OF_GOOP_GOLD} gold, lose {WORLD_OF_GOOP_DAMAGE} HP)"
                ),
            },
            EventChoice {
                label: format!("Leave it (lose {gold_loss} gold)"),
            },
        ],
        1 => labeled_choices(&["Leave"]),
        _ => Vec::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WeMeetAgainOptions {
    potion_slot: Option<usize>,
    gold_amount: i32,
    card_index: Option<usize>,
}

fn we_meet_again_choices(stage: u32, options: WeMeetAgainOptions) -> Vec<EventChoice> {
    if stage > 0 {
        return labeled_choices(&["Leave"]);
    }

    let mut choices = Vec::new();
    if options.potion_slot.is_some() {
        choices.push(EventChoice {
            label: "Give Potion".to_owned(),
        });
    }
    if options.gold_amount > 0 {
        choices.push(EventChoice {
            label: "Give Gold".to_owned(),
        });
    }
    if options.card_index.is_some() {
        choices.push(EventChoice {
            label: "Give Card".to_owned(),
        });
    }
    choices.push(EventChoice {
        label: "Attack".to_owned(),
    });
    choices
}

fn we_meet_again_random_potion_slot(run: &mut RunState) -> Option<usize> {
    let mut slots: Vec<_> = run
        .occupied_potion_slots()
        .into_iter()
        .map(|(slot, _)| slot)
        .collect();
    if slots.is_empty() {
        return None;
    }

    let mut misc_rng = run.rng_for_stream(RunRngStream::Misc);
    let shuffle_seed = misc_rng.random_long();
    run.store_rng_counter(RunRngStream::Misc, &misc_rng);
    JavaRng::new(shuffle_seed).collections_shuffle(&mut slots);
    slots.first().copied()
}

fn we_meet_again_gold_amount(run: &mut RunState) -> i32 {
    if run.gold < 50 {
        return 0;
    }

    let mut misc_rng = run.rng_for_stream(RunRngStream::Misc);
    let amount = if run.gold > 150 {
        misc_rng.random_int_range(50, 150)
    } else {
        misc_rng.random_int_range(50, run.gold)
    };
    run.store_rng_counter(RunRngStream::Misc, &misc_rng);
    amount
}

fn is_we_meet_again_basic_card(content_id: ContentId) -> bool {
    is_basic_starter_card(content_id)
        || content_id == STRIKE_R_PLUS_ID
        || content_id == DEFEND_R_PLUS_ID
        || content_id == BASH_PLUS_ID
}

fn we_meet_again_random_card_index(run: &mut RunState) -> Option<usize> {
    let mut candidates = run
        .deck
        .iter()
        .enumerate()
        .filter(|(_, card)| {
            !is_we_meet_again_basic_card(card.content_id) && !is_curse_content_id(card.content_id)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }

    let mut misc_rng = run.rng_for_stream(RunRngStream::Misc);
    let shuffle_seed = misc_rng.random_long();
    run.store_rng_counter(RunRngStream::Misc, &misc_rng);
    JavaRng::new(shuffle_seed).collections_shuffle(&mut candidates);
    candidates.first().copied()
}

fn we_meet_again_options_for_run(run: &mut RunState) -> WeMeetAgainOptions {
    let potion_slot = we_meet_again_random_potion_slot(run);
    let gold_amount = we_meet_again_gold_amount(run);
    let card_index = we_meet_again_random_card_index(run);
    WeMeetAgainOptions {
        potion_slot,
        gold_amount,
        card_index,
    }
}

fn we_meet_again_event_data(options: WeMeetAgainOptions) -> u32 {
    let potion = options
        .potion_slot
        .and_then(|slot| u8::try_from(slot).ok())
        .unwrap_or(WE_MEET_AGAIN_NO_OPTION);
    let gold = u8::try_from(options.gold_amount).unwrap_or(0);
    let card = options
        .card_index
        .and_then(|index| u8::try_from(index).ok())
        .unwrap_or(WE_MEET_AGAIN_NO_OPTION);
    u32::from(potion) | (u32::from(gold) << 8) | (u32::from(card) << 16)
}

fn we_meet_again_options_from_event_data(event_data: u32) -> WeMeetAgainOptions {
    let potion = (event_data & 0xff) as u8;
    let gold = ((event_data >> 8) & 0xff) as u8;
    let card = ((event_data >> 16) & 0xff) as u8;
    WeMeetAgainOptions {
        potion_slot: (potion != WE_MEET_AGAIN_NO_OPTION).then_some(usize::from(potion)),
        gold_amount: i32::from(gold),
        card_index: (card != WE_MEET_AGAIN_NO_OPTION).then_some(usize::from(card)),
    }
}

fn tomb_of_lord_red_mask_choices(run: &RunState, stage: u32) -> Vec<EventChoice> {
    if stage > 0 {
        return labeled_choices(&["Leave"]);
    }

    if has_relic_key(run, RelicKey::RedMask) {
        labeled_choices(&["Wear mask", "Leave"])
    } else {
        vec![
            EventChoice {
                label: format!("Offer: {} Gold", run.gold),
            },
            EventChoice {
                label: "Leave".to_owned(),
            },
        ]
    }
}

fn labeled_choices(labels: &[&str]) -> Vec<EventChoice> {
    labels
        .iter()
        .map(|label| EventChoice {
            label: (*label).to_owned(),
        })
        .collect()
}

fn bonfire_elementals_choices(stage: u32) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Continue"]),
        1 => labeled_choices(&["Offer"]),
        _ => labeled_choices(&["Leave"]),
    }
}

fn designer_event_data_adjustment_upgrades_one(event_data: u32) -> bool {
    event_data & 1 != 0
}

fn designer_event_data_cleanup_removes_cards(event_data: u32) -> bool {
    event_data & 2 != 0
}

fn roll_designer_event_data(run: &mut RunState) -> u32 {
    let mut rng = run.rng_for_stream(RunRngStream::Misc);
    let event_data = u32::from(rng.random_bool()) | (u32::from(rng.random_bool()) << 1);
    run.store_rng_counter(RunRngStream::Misc, &rng);
    event_data
}

fn designer_costs(run: &RunState) -> (i32, i32, i32, i32) {
    if run.ascension >= 15 {
        (50, 75, 110, 5)
    } else {
        (40, 60, 90, 3)
    }
}

fn designer_has_purgeable_card(run: &RunState) -> bool {
    run.deck.iter().any(|card| !card.bottled)
}

fn designer_purgeable_card_count(run: &RunState) -> usize {
    run.deck.iter().filter(|card| !card.bottled).count()
}

fn designer_has_upgradable_card(run: &RunState) -> bool {
    run.deck.iter().any(card_instance_is_upgradeable)
}

fn designer_choices(run: &RunState, event_data: u32) -> Vec<EventChoice> {
    let (adjust_cost, cleanup_cost, full_service_cost, hp_loss) = designer_costs(run);
    let adjustment_available = run.gold >= adjust_cost && designer_has_upgradable_card(run);
    let cleanup_available = run.gold >= cleanup_cost
        && if designer_event_data_cleanup_removes_cards(event_data) {
            designer_has_purgeable_card(run)
        } else {
            designer_purgeable_card_count(run) >= 2
        };
    let full_service_available = run.gold >= full_service_cost && designer_has_purgeable_card(run);
    vec![
        EventChoice {
            label: if adjustment_available {
                format!("Adjustments ({adjust_cost} gold)")
            } else {
                "Adjustments (Locked)".to_owned()
            },
        },
        EventChoice {
            label: if cleanup_available {
                format!("Clean Up ({cleanup_cost} gold)")
            } else {
                "Clean Up (Locked)".to_owned()
            },
        },
        EventChoice {
            label: if full_service_available {
                format!("Full Service ({full_service_cost} gold)")
            } else {
                "Full Service (Locked)".to_owned()
            },
        },
        EventChoice {
            label: format!("Get punched ({hp_loss} HP)"),
        },
    ]
}

fn designer_screen(run: &RunState, stage: u32, event_data: u32) -> EventScreen {
    let choices = match stage {
        0 => labeled_choices(&["Continue"]),
        1 => designer_choices(run, event_data),
        _ => labeled_choices(&["Leave"]),
    };
    EventScreen {
        event: Event::Designer,
        choices,
        stage,
        event_data,
    }
}

fn designer_done_screen(run: &mut RunState) {
    run.event = Some(designer_screen(run, 2, 0));
}

fn open_duplicator_card_grid(run: &mut RunState) {
    let next_card_id = run.next_card_instance_id();
    let cards = run
        .deck
        .iter()
        .copied()
        .enumerate()
        .map(|(index, mut card)| {
            card.id = CardId::new(next_card_id + index as u64);
            card.bottled = false;
            card
        })
        .collect();
    open_event_obtain_card_return_to_event_grid(run, Event::Duplicator, cards);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BonfireCardClass {
    Curse,
    Basic,
    Common,
    Special,
    Uncommon,
    Rare,
}

fn bonfire_card_class(content_id: ContentId) -> Option<BonfireCardClass> {
    if is_curse_content_id(content_id) {
        return Some(BonfireCardClass::Curse);
    }
    if is_basic_starter_card(content_id)
        || matches!(
            content_id,
            STRIKE_R_PLUS_ID | DEFEND_R_PLUS_ID | BASH_PLUS_ID
        )
    {
        return Some(BonfireCardClass::Basic);
    }
    if content_id == APPARITION_ID {
        return Some(BonfireCardClass::Special);
    }
    match crate::content::cards::card_type_and_rarity(content_id)?.1 {
        CardRarity::Common => Some(BonfireCardClass::Common),
        CardRarity::Uncommon => Some(BonfireCardClass::Uncommon),
        CardRarity::Rare => Some(BonfireCardClass::Rare),
    }
}

pub(crate) fn complete_bonfire_elementals_card(
    run: &mut RunState,
    card: CardInstance,
) -> SimResult<()> {
    let class = bonfire_card_class(card.content_id).ok_or(SimError::InvalidState(
        "Bonfire selected an unsupported card",
    ))?;
    run.remove_deck_card(card.id).ok_or(SimError::InvalidState(
        "Bonfire selected card is not in deck",
    ))?;

    match class {
        BonfireCardClass::Curse => {
            if run.relic_keys.contains(&RelicKey::SpiritPoop) {
                run.gain_relic_key(RelicKey::Circlet);
            } else {
                run.gain_relic_key(RelicKey::SpiritPoop);
            }
        }
        BonfireCardClass::Basic => {}
        BonfireCardClass::Common | BonfireCardClass::Special => {
            run.heal_player(5);
        }
        BonfireCardClass::Uncommon => {
            run.heal_player(run.player_max_hp);
        }
        BonfireCardClass::Rare => {
            run.player_max_hp += 10;
            run.heal_player(run.player_max_hp);
        }
    }

    run.card_grid = None;
    run.phase = RunPhase::Event;
    run.event = Some(make_event_screen(
        Event::BonfireElementals,
        bonfire_elementals_choices(2),
        2,
    ));
    Ok(())
}

pub(crate) fn complete_designer_remove_and_upgrade(
    run: &mut RunState,
    card: CardInstance,
) -> SimResult<()> {
    run.remove_deck_card(card.id).ok_or(SimError::InvalidState(
        "Designer selected card is not in deck",
    ))?;
    upgrade_random_deck_cards(run, 1);
    run.card_grid = None;
    run.phase = RunPhase::Event;
    designer_done_screen(run);
    Ok(())
}

fn neow_talk_choices() -> Vec<EventChoice> {
    labeled_choices(&["Talk"])
}

fn neow_leave_choices() -> Vec<EventChoice> {
    labeled_choices(&["Leave"])
}

fn neow_option_choices(run: &RunState) -> Vec<EventChoice> {
    generate_neow_options(run.event_rng_seed as i64, run.player_max_hp)
        .into_iter()
        .map(|option| EventChoice {
            label: option.label,
        })
        .collect()
}

fn roll_scrap_ooze_relic(run: &mut RunState, event_data: u32) -> bool {
    let mut rng = StsRng::with_counter(run.misc_rng_seed as i64, run.misc_rng_counter);
    let roll = rng.random_int(99);
    run.misc_rng_counter = rng.counter();
    let relic_chance = i32::try_from(event_data * 10 + 25).expect("scrap ooze relic chance");
    roll >= 99 - relic_chance
}

fn scrap_ooze_hp_loss(ascension: u8, failed_reaches: u32) -> i32 {
    let base = if ascension >= 15 {
        SCRAP_OOZE_REACH_HP_LOSS + 2
    } else {
        SCRAP_OOZE_REACH_HP_LOSS
    };
    base + i32::try_from(failed_reaches).expect("scrap ooze failed reach count")
}

fn roll_wing_statue_gold(run: &mut RunState) -> i32 {
    let mut rng = StsRng::with_counter(run.misc_rng_seed as i64, run.misc_rng_counter);
    let gold = rng.random_int_range(WING_STATUE_MIN_GOLD, WING_STATUE_MAX_GOLD);
    run.misc_rng_counter = rng.counter();
    gold
}

fn roll_face_trader_relic(run: &mut RunState) -> RelicKey {
    let mut candidates = Vec::new();
    if !run.relics.contains(&Relic::CultistMask) {
        candidates.push(RelicKey::CultistMask);
    }
    if !run.relics.contains(&Relic::FaceOfCleric) {
        candidates.push(RelicKey::FaceOfCleric);
    }
    if !run.relics.contains(&Relic::GremlinMask) {
        candidates.push(RelicKey::GremlinMask);
    }
    if !run.relics.contains(&Relic::NlothsMask) {
        candidates.push(RelicKey::NlothsMask);
    }
    if !run.relics.contains(&Relic::SsserpentHead) {
        candidates.push(RelicKey::SsserpentHead);
    }
    if candidates.is_empty() {
        candidates.push(RelicKey::Circlet);
    }

    let mut rng = StsRng::with_counter(run.misc_rng_seed as i64, run.misc_rng_counter);
    let shuffle_seed = rng.random_long();
    run.misc_rng_counter = rng.counter();
    JavaRng::new(shuffle_seed).collections_shuffle(&mut candidates);
    candidates[0]
}

fn initialize_act1_event_pools(run: &mut RunState) {
    if !run.act1_event_list.is_empty() {
        return;
    }
    run.act1_event_list = ACT1_EVENTS.to_vec();
    run.act1_shrine_list = ACT1_SHRINES.to_vec();
}

fn initialize_act2_event_pools(run: &mut RunState) {
    if !run.act2_event_list.is_empty() {
        return;
    }
    run.act2_event_list = ACT2_EVENTS.to_vec();
    run.act2_shrine_list = ACT2_SHRINES.to_vec();
}

fn initialize_act3_event_pools(run: &mut RunState) {
    if !run.act3_event_list.is_empty() {
        return;
    }
    run.act3_event_list = ACT3_EVENTS.to_vec();
    run.act3_shrine_list = ACT3_SHRINES.to_vec();
}

fn initialize_special_one_time_event_pool(run: &mut RunState) {
    if run.special_one_time_events_initialized {
        return;
    }
    run.special_one_time_event_list = SPECIAL_ONE_TIME_EVENTS_PREFIX.to_vec();
    if note_for_yourself_is_available(run) {
        run.special_one_time_event_list.push(Event::NoteForYourself);
    }
    run.special_one_time_event_list
        .extend(SPECIAL_ONE_TIME_EVENTS_SUFFIX);
    run.special_one_time_events_initialized = true;
}

fn event_lists_mut(run: &mut RunState) -> (&mut Vec<Event>, &mut Vec<Event>) {
    match run.current_act {
        2 => (&mut run.act2_event_list, &mut run.act2_shrine_list),
        3 => (&mut run.act3_event_list, &mut run.act3_shrine_list),
        _ => (&mut run.act1_event_list, &mut run.act1_shrine_list),
    }
}

fn ensure_event_lists(run: &mut RunState) {
    initialize_special_one_time_event_pool(run);
    match run.current_act {
        2 => initialize_act2_event_pools(run),
        3 => initialize_act3_event_pools(run),
        _ => initialize_act1_event_pools(run),
    }
}

fn pick_from_list(rng: &mut StsRng, list: &mut Vec<Event>) -> Event {
    let idx = rng.random_int((list.len() - 1) as i32) as usize;
    list.remove(idx)
}

fn get_shrine(run: &mut RunState, rng: &mut StsRng) -> Event {
    let mut candidates = match run.current_act {
        2 => run.act2_shrine_list.clone(),
        3 => run.act3_shrine_list.clone(),
        _ => run.act1_shrine_list.clone(),
    };
    candidates.extend(
        run.special_one_time_event_list
            .iter()
            .copied()
            .filter(|event| special_one_time_event_is_available(run, *event)),
    );
    if candidates.is_empty() {
        let (event_list, _) = event_lists_mut(run);
        return pick_from_list(rng, event_list);
    }
    let idx = rng.random_int((candidates.len() - 1) as i32) as usize;
    let event = candidates[idx];
    let (_, shrine_list) = event_lists_mut(run);
    if let Some(index) = shrine_list.iter().position(|candidate| *candidate == event) {
        shrine_list.remove(index);
    }
    if let Some(index) = run
        .special_one_time_event_list
        .iter()
        .position(|candidate| *candidate == event)
    {
        run.special_one_time_event_list.remove(index);
    }
    event
}

fn special_one_time_event_is_available(run: &RunState, event: Event) -> bool {
    match event {
        Event::Designer => run.current_act >= 2 && run.gold >= 75,
        Event::Duplicator => run.current_act >= 2,
        Event::FaceTrader => run.current_act == 1 || run.current_act == 2,
        Event::FountainOfCleansing => deck_has_curse(&run.deck),
        Event::KnowingSkull => run.current_act == 2 && run.player_hp > 12,
        Event::Nloth => run.current_act == 2 && run.relics.len() + run.relic_keys.len() >= 2,
        Event::SecretPortal => run.current_act == 3 && run.playtime_seconds >= 800,
        Event::TheJoust => run.current_act == 2 && run.gold >= 50,
        Event::TheWomanInBlue => run.gold >= 50,
        _ => true,
    }
}

fn deck_has_curse(deck: &[CardInstance]) -> bool {
    deck.iter().any(|card| is_curse_content_id(card.content_id))
}

fn fountain_removes_curse(content_id: ContentId) -> bool {
    is_curse_content_id(content_id)
        && !matches!(content_id, ASCENDERS_BANE_ID | CURSE_OF_THE_BELL_ID)
}

fn note_for_yourself_is_available(run: &RunState) -> bool {
    // Target enables NoteForYourself unconditionally at A0. A1-A14 also depends
    // on local profile unlock prefs, which seed-start replay does not model.
    run.ascension == 0
}

fn get_event(run: &mut RunState, rng: &mut StsRng) -> Event {
    let candidates: Vec<Event> = {
        let event_list = {
            let (event_list, _) = event_lists_mut(run);
            event_list.clone()
        };
        event_list
            .iter()
            .copied()
            .filter(|event| event_is_available(run, *event))
            .collect()
    };
    if candidates.is_empty() {
        get_shrine(run, rng)
    } else {
        let idx = rng.random_int((candidates.len() - 1) as i32) as usize;
        let event = candidates[idx];
        let (event_list, _) = event_lists_mut(run);
        if let Some(index) = event_list.iter().position(|candidate| *candidate == event) {
            event_list.remove(index);
        }
        event
    }
}

fn event_is_available(run: &RunState, event: Event) -> bool {
    match event {
        Event::DeadAdventurer | Event::HypnotizingColoredMushrooms => run.current_floor > 6,
        Event::MoaiHead => {
            has_relic_key(run, RelicKey::GoldenIdol)
                || (run.player_hp as f32 / run.player_max_hp as f32) <= 0.5
        }
        Event::TheCleric => run.gold >= 35,
        Event::Beggar => run.gold >= BEGGAR_GOLD_COST,
        Event::Colosseum => current_floor_in_act(run) > 8,
        _ => true,
    }
}

fn current_floor_in_act(run: &RunState) -> i32 {
    match run.current_act {
        1 => run.current_floor,
        2 => run.current_floor - 17,
        3 => run.current_floor - 34,
        _ => run.current_floor,
    }
}

fn roll_world_of_goop_gold_loss(run: &mut RunState) -> i32 {
    let (min, max) = if run.ascension >= 15 {
        (
            WORLD_OF_GOOP_A15_MIN_GOLD_LOSS,
            WORLD_OF_GOOP_A15_MAX_GOLD_LOSS,
        )
    } else {
        (WORLD_OF_GOOP_MIN_GOLD_LOSS, WORLD_OF_GOOP_MAX_GOLD_LOSS)
    };
    let mut rng = StsRng::with_counter(run.misc_rng_seed as i64, run.misc_rng_counter);
    let loss = rng.random_int_range(min, max).min(run.gold);
    run.misc_rng_counter = rng.counter();
    loss
}

fn generate_event(run: &mut RunState, rng: &mut StsRng) -> Event {
    let shrine_list_is_empty = {
        let (_, shrine_list) = event_lists_mut(run);
        shrine_list.is_empty()
    };
    if rng.random_float_range(0.0, 1.0) < SHRINE_CHANCE && !shrine_list_is_empty {
        get_shrine(run, rng)
    } else {
        get_event(run, rng)
    }
}

fn make_event_screen(event: Event, choices: Vec<EventChoice>, stage: u32) -> EventScreen {
    EventScreen {
        event,
        choices,
        stage,
        event_data: 0,
    }
}

#[must_use]
pub fn legacy_fixed_event_screen() -> EventScreen {
    make_event_screen(
        Event::GoldenShrine,
        vec![EventChoice {
            label: "Pray".to_owned(),
        }],
        0,
    )
}

fn golden_shrine_choices(stage: u32) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Pray", "Desecrate", "Leave"]),
        _ => labeled_choices(&["Leave"]),
    }
}

fn wing_statue_choices(stage: u32, can_attack: bool) -> Vec<EventChoice> {
    match stage {
        0 if can_attack => labeled_choices(&["Pray", "Destroy", "Leave"]),
        0 => labeled_choices(&["Pray", "Locked", "Leave"]),
        1 => labeled_choices(&["Continue"]),
        _ => labeled_choices(&["Leave"]),
    }
}

fn wheel_of_change_choices(stage: u32, _result: u32) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Play"]),
        1 => labeled_choices(&["spin"]),
        2 => labeled_choices(&["prize!"]),
        _ => labeled_choices(&["Leave"]),
    }
}

fn match_and_keep_choices(stage: u32, card_count: usize) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Continue"]),
        1 => labeled_choices(&["Play"]),
        _ => (0..card_count)
            .map(|index| EventChoice {
                label: format!("card{index}"),
            })
            .collect(),
    }
}

pub fn match_and_keep_group_index_for_label(label_index: usize, card_count: usize) -> usize {
    if card_count == 12 {
        // CommunicationMod enumerates the 4x3 card grid in hitbox order,
        // which differs from MatchAndKeep's backing CardGroup order.
        const COMMUNICATION_MOD_GROUP_ORDER: [usize; 12] = [0, 9, 6, 3, 4, 1, 10, 7, 8, 5, 2, 11];
        COMMUNICATION_MOD_GROUP_ORDER[label_index]
    } else {
        label_index
    }
}

pub fn match_and_keep_label_index_for_group(group_index: usize, card_count: usize) -> usize {
    if card_count == 12 {
        const COMMUNICATION_MOD_GROUP_ORDER: [usize; 12] = [0, 9, 6, 3, 4, 1, 10, 7, 8, 5, 2, 11];
        COMMUNICATION_MOD_GROUP_ORDER
            .iter()
            .position(|candidate| *candidate == group_index)
            .unwrap_or(group_index)
    } else {
        group_index
    }
}

fn match_and_keep_card_choices(run: &RunState) -> Vec<EventChoice> {
    run.match_and_keep
        .as_ref()
        .map(|state| {
            let card_count = state.cards.len();
            (0..card_count)
                .filter_map(|label_index| {
                    let group_index = match_and_keep_group_index_for_label(label_index, card_count);
                    let card = state.cards.get(group_index)?;
                    let currently_flipped = state.first_flipped_index == Some(group_index)
                        || state.second_flipped_index == Some(group_index);
                    (!card.matched && !currently_flipped).then_some((label_index, card))
                })
                .map(|(label_index, card)| EventChoice {
                    label: if card.revealed {
                        get_card_definition(card.content_id)
                            .map(|definition| definition.name.to_ascii_lowercase())
                            .unwrap_or_else(|| format!("card{label_index}"))
                    } else {
                        format!("card{label_index}")
                    },
                })
                .collect()
        })
        .unwrap_or_else(|| match_and_keep_choices(2, 12))
}

fn initialize_match_and_keep_state(run: &mut RunState) -> MatchAndKeepState {
    let mut card_rng = run.rng_for_stream(RunRngStream::CardReward);
    let mut shuffle_rng = run.rng_for_stream(RunRngStream::Shuffle);
    let mut contents = if run.ascension >= 15 {
        vec![
            random_ironclad_card_by_rarity(&mut card_rng, CardRarity::Rare),
            random_ironclad_card_by_rarity(&mut card_rng, CardRarity::Uncommon),
            random_ironclad_card_by_rarity(&mut card_rng, CardRarity::Common),
            random_normal_curse(&mut card_rng),
            random_normal_curse(&mut card_rng),
            BASH_ID,
        ]
    } else {
        vec![
            random_ironclad_card_by_rarity(&mut card_rng, CardRarity::Rare),
            random_ironclad_card_by_rarity(&mut card_rng, CardRarity::Uncommon),
            random_ironclad_card_by_rarity(&mut card_rng, CardRarity::Common),
            random_colorless_for_match_and_keep(&mut shuffle_rng, CardRarity::Uncommon),
            random_normal_curse(&mut card_rng),
            BASH_ID,
        ]
    };
    run.store_rng_counter(RunRngStream::CardReward, &card_rng);
    run.store_rng_counter(RunRngStream::Shuffle, &shuffle_rng);

    let mut paired = contents.clone();
    contents.append(&mut paired);

    let mut misc_rng = run.rng_for_stream(RunRngStream::Misc);
    let shuffle_seed = misc_rng.random_long();
    run.store_rng_counter(RunRngStream::Misc, &misc_rng);
    JavaRng::new(shuffle_seed).collections_shuffle(&mut contents);
    MatchAndKeepState {
        cards: contents
            .into_iter()
            .map(|content_id| MatchAndKeepCard {
                content_id,
                revealed: false,
                matched: false,
            })
            .collect(),
        attempts_remaining: 5,
        first_flipped_index: None,
        second_flipped_index: None,
        matched_cards: Vec::new(),
    }
}

fn random_colorless_for_match_and_keep(rng: &mut StsRng, rarity: CardRarity) -> ContentId {
    let mut pool = colorless_match_and_keep_pool();
    let shuffle_seed = rng.random_long();
    JavaRng::new(shuffle_seed).collections_shuffle(&mut pool);
    pool.into_iter()
        .find(|content_id| {
            get_card_definition(*content_id).is_some_and(|definition| {
                crate::content::cards::card_type_and_rarity(definition.id)
                    .is_some_and(|(_, card_rarity)| card_rarity == rarity)
            })
        })
        .unwrap_or_else(|| shop_card_content_id("SWIFT_STRIKE"))
}

fn random_ironclad_card_by_rarity(rng: &mut StsRng, rarity: CardRarity) -> ContentId {
    let candidate_indices = IRONCLAD_REWARD_ENTRIES
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.rarity == rarity)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let pick = rng.random_int((candidate_indices.len() - 1) as i32) as usize;
    IRONCLAD_REWARD_ENTRIES[candidate_indices[pick]].content_id
}

fn wheel_of_change_gold(act: i32) -> i32 {
    match act {
        1 => WHEEL_OF_CHANGE_GOLD_ACT1,
        2 => WHEEL_OF_CHANGE_GOLD_ACT2,
        _ => WHEEL_OF_CHANGE_GOLD_ACT3,
    }
}

fn wheel_of_change_hp_loss(max_hp: i32, ascension: u8) -> i32 {
    let percent = if ascension >= 15 {
        WHEEL_OF_CHANGE_A15_HP_LOSS_PERCENT
    } else {
        WHEEL_OF_CHANGE_HP_LOSS_PERCENT
    };
    ((max_hp as f32) * percent) as i32
}

fn roll_wheel_of_change_result(run: &mut RunState) -> u32 {
    let mut misc_rng = StsRng::with_counter(run.misc_rng_seed as i64, run.misc_rng_counter);
    let result = misc_rng.random_int(5) as u32;
    run.misc_rng_counter = misc_rng.counter();
    result
}

fn has_wing_statue_attack_card(run: &RunState) -> bool {
    run.deck.iter().any(|card| {
        get_card_definition(card.content_id)
            .map(|definition| {
                definition.card_type == CardType::Attack
                    && definition.values.damage.unwrap_or(0) >= WING_STATUE_REQUIRED_DAMAGE
            })
            .unwrap_or(false)
    })
}

/// Compatibility wrapper for [`legacy_fixed_event_screen`].
///
/// Fidelity: [`crate::FidelityCategory::LegacyFixed`]. This is an early
/// milestone Golden Shrine fixture, not general event RNG.
#[must_use]
pub fn fixed_event_screen() -> EventScreen {
    legacy_fixed_event_screen()
}

pub fn enter_legacy_fixed_event_screen(run: &mut RunState) {
    run.phase = RunPhase::Event;
    run.event = Some(legacy_fixed_event_screen());
}

/// Compatibility wrapper for [`enter_legacy_fixed_event_screen`].
///
/// Fidelity: [`crate::FidelityCategory::LegacyFixed`].
pub fn enter_fixed_event_screen(run: &mut RunState) {
    enter_legacy_fixed_event_screen(run);
}

pub fn enter_event_screen(run: &mut RunState) {
    run.reinit_misc_rng_for_floor();
    run.ensure_ironclad_relic_pools();
    ensure_event_lists(run);
    let mut rng = StsRng::with_counter(run.event_rng_seed as i64, run.event_rng_counter);
    let event = generate_event(run, &mut rng);
    run.phase = RunPhase::Event;
    run.match_and_keep = None;
    run.event = Some(entered_event_screen_for_run(run, event));
}

#[must_use]
pub fn event_screen(event: Event) -> EventScreen {
    match event {
        Event::Neow => make_event_screen(event, neow_talk_choices(), 0),
        Event::SpireHeart => make_event_screen(event, spire_heart_choices(0), 0),
        Event::AccursedBlacksmith => {
            make_event_screen(event, labeled_choices(&["Forge", "Rummage", "Leave"]), 0)
        }
        Event::BonfireElementals => make_event_screen(event, bonfire_elementals_choices(0), 0),
        Event::Designer => make_event_screen(event, labeled_choices(&["Continue"]), 0),
        Event::Duplicator => make_event_screen(event, labeled_choices(&["Duplicate", "Leave"]), 0),
        Event::HypnotizingColoredMushrooms => {
            make_event_screen(event, hypnotizing_colored_mushrooms_choices(0), 0)
        }
        Event::Nloth => make_event_screen(event, labeled_choices(&["Trade", "Trade", "Leave"]), 0),
        Event::GoldenShrine => legacy_fixed_event_screen(),
        Event::FountainOfCleansing => make_event_screen(event, fountain_of_cleansing_choices(0), 0),
        Event::Transmorgrifier => make_event_screen(event, labeled_choices(&["Pray", "Leave"]), 0),
        Event::Purifier => make_event_screen(
            event,
            vec![
                EventChoice {
                    label: "Pray".to_owned(),
                },
                EventChoice {
                    label: "Leave".to_owned(),
                },
            ],
            0,
        ),
        Event::UpgradeShrine => make_event_screen(
            event,
            vec![
                EventChoice {
                    label: "Pray".to_owned(),
                },
                EventChoice {
                    label: "Leave".to_owned(),
                },
            ],
            0,
        ),
        Event::TheCleric => make_event_screen(
            event,
            vec![
                EventChoice {
                    label: "Heal".to_owned(),
                },
                EventChoice {
                    label: "Purify".to_owned(),
                },
                EventChoice {
                    label: "Leave".to_owned(),
                },
            ],
            0,
        ),
        Event::ShiningLight => make_event_screen(
            event,
            vec![
                EventChoice {
                    label: "Enter the light".to_owned(),
                },
                EventChoice {
                    label: "Leave".to_owned(),
                },
            ],
            0,
        ),
        Event::ScrapOoze => make_event_screen(event, scrap_ooze_choices(0), 0),
        Event::FaceTrader => make_event_screen(event, face_trader_choices(0), 0),
        Event::BigFish => make_event_screen(event, big_fish_choices(0), 0),
        Event::GoldenIdol => make_event_screen(event, golden_idol_choices(0, 0, 0), 0),
        Event::WingStatue => make_event_screen(event, wing_statue_choices(0, false), 0),
        Event::WorldOfGoop => make_event_screen(event, world_of_goop_choices(0, 0), 0),
        Event::DeadAdventurer => make_event_screen(event, dead_adventurer_choices(0, 25), 0),
        Event::TheSsssserpent => make_event_screen(event, sssssserpent_choices(0), 0),
        Event::NoteForYourself => make_event_screen(event, note_for_yourself_choices(0), 0),
        Event::SecretPortal => {
            make_event_screen(event, labeled_choices(&["Take the portal", "Leave"]), 0)
        }
        Event::TheJoust => make_event_screen(event, joust_choices(0), 0),
        Event::TheWomanInBlue => make_event_screen(
            event,
            labeled_choices(&[
                "Buy 1 potion (20 gold)",
                "Buy 2 potions (30 gold)",
                "Buy 3 potions (40 gold)",
                "Leave",
            ]),
            0,
        ),
        Event::Falling => make_event_screen(event, labeled_choices(&["Continue"]), 0),
        Event::MoaiHead => make_event_screen(
            event,
            labeled_choices(&["Lose max HP and heal", "Leave"]),
            0,
        ),
        Event::MysteriousSphere => make_event_screen(event, mysterious_sphere_choices(0), 0),
        Event::SensoryStone => make_event_screen(event, sensory_stone_choices(0), 0),
        Event::WindingHalls => make_event_screen(event, labeled_choices(&["..."]), 0),
        Event::LivingWall => {
            make_event_screen(event, labeled_choices(&["Forget", "Change", "Grow"]), 0)
        }
        Event::BackToBasics => {
            make_event_screen(event, labeled_choices(&["Elegance", "Simplicity"]), 0)
        }
        Event::TheLibrary => make_event_screen(event, labeled_choices(&["Read", "Sleep"]), 0),
        Event::TheMausoleum => {
            make_event_screen(event, labeled_choices(&["Open coffin", "Leave"]), 0)
        }
        Event::Vampires => make_event_screen(event, vampires_choices(false), 0),
        Event::CursedTome => make_event_screen(event, cursed_tome_choices(0, 0), 0),
        Event::Nest => make_event_screen(event, nest_choices(0, 0), 0),
        Event::Beggar => make_event_screen(event, beggar_choices(0), 0),
        Event::Addict => make_event_screen(event, addict_choices(0), 0),
        Event::ForgottenAltar => make_event_screen(event, forgotten_altar_choices(0, true), 0),
        Event::Ghosts => make_event_screen(event, ghosts_choices(0, 0), 0),
        Event::KnowingSkull => make_event_screen(event, knowing_skull_choices(0, 0), 0),
        Event::MaskedBandits => make_event_screen(event, masked_bandits_choices(0), 0),
        Event::Colosseum => make_event_screen(event, colosseum_choices(0), 0),
        Event::DrugDealer => make_event_screen(event, drug_dealer_choices(0, false), 0),
        Event::Lab => make_event_screen(event, labeled_choices(&["Search"]), 0),
        Event::TombOfLordRedMask => {
            make_event_screen(event, labeled_choices(&["Offer", "Leave"]), 0)
        }
        Event::WheelOfChange => make_event_screen(event, wheel_of_change_choices(0, 0), 0),
        Event::MatchAndKeep => make_event_screen(event, match_and_keep_choices(0, 0), 0),
        Event::WeMeetAgain => make_event_screen(
            event,
            we_meet_again_choices(
                0,
                WeMeetAgainOptions {
                    potion_slot: Some(0),
                    gold_amount: 50,
                    card_index: Some(0),
                },
            ),
            0,
        ),
        Event::MindBloom => make_event_screen(
            event,
            labeled_choices(&["I am War", "I am Awake", "I am Healthy"]),
            0,
        ),
    }
}

#[must_use]
pub fn event_screen_for_run(run: &RunState, event: Event) -> EventScreen {
    match event {
        Event::Neow => make_event_screen(event, neow_option_choices(run), 1),
        Event::Designer => designer_screen(run, 0, 0),
        Event::GoldenShrine => make_event_screen(event, golden_shrine_choices(0), 0),
        Event::Vampires => make_event_screen(
            event,
            vampires_choices(run.relics.contains(&Relic::BloodVial)),
            0,
        ),
        Event::WingStatue => make_event_screen(
            event,
            wing_statue_choices(0, has_wing_statue_attack_card(run)),
            0,
        ),
        Event::TombOfLordRedMask => {
            make_event_screen(event, tomb_of_lord_red_mask_choices(run, 0), 0)
        }
        Event::TheWomanInBlue => make_event_screen(event, woman_in_blue_choices(run), 0),
        Event::ForgottenAltar => make_event_screen(
            event,
            forgotten_altar_choices(0, run.relics.contains(&Relic::GoldenIdol)),
            0,
        ),
        Event::NoteForYourself => {
            make_event_screen(event, note_for_yourself_choices_for_run(run, 0), 0)
        }
        Event::Falling => make_event_screen(event, falling_choices(run, 0), 0),
        Event::MoaiHead => make_event_screen(event, moai_choices(run, 0), 0),
        Event::MysteriousSphere => make_event_screen(event, mysterious_sphere_choices(0), 0),
        Event::SensoryStone => make_event_screen(event, sensory_stone_choices(0), 0),
        Event::WindingHalls => make_event_screen(event, winding_halls_choices(run, 0), 0),
        Event::MindBloom => make_event_screen(event, mind_bloom_choices(run), 0),
        _ => event_screen(event),
    }
}

pub(crate) fn enter_spire_heart_event(run: &mut RunState) {
    run.current_floor += 1;
    run.current_room_override = Some(crate::map::RoomKind::Victory);
    run.phase = RunPhase::Event;
    run.combat = None;
    run.reward = None;
    run.event = Some(event_screen(Event::SpireHeart));
}

fn entered_event_screen_for_run(run: &mut RunState, event: Event) -> EventScreen {
    match event {
        Event::WorldOfGoop => {
            let gold_loss = roll_world_of_goop_gold_loss(run);
            EventScreen {
                event,
                choices: world_of_goop_choices(0, gold_loss),
                stage: 0,
                event_data: gold_loss as u32,
            }
        }
        Event::WeMeetAgain => {
            let options = we_meet_again_options_for_run(run);
            EventScreen {
                event,
                choices: we_meet_again_choices(0, options),
                stage: 0,
                event_data: we_meet_again_event_data(options),
            }
        }
        Event::MatchAndKeep => {
            run.match_and_keep = Some(initialize_match_and_keep_state(run));
            event_screen_for_run(run, event)
        }
        Event::Designer => {
            let event_data = roll_designer_event_data(run);
            designer_screen(run, 0, event_data)
        }
        Event::DeadAdventurer => {
            let event_data = roll_dead_adventurer_event_data(run);
            dead_adventurer_screen(run, 0, event_data)
        }
        Event::Nloth => {
            let event_data = roll_nloth_event_data(run);
            EventScreen {
                event,
                choices: nloth_choices(run, 0, event_data),
                stage: 0,
                event_data,
            }
        }
        _ => event_screen_for_run(run, event),
    }
}

#[must_use]
pub fn neow_talk_screen() -> EventScreen {
    make_event_screen(Event::Neow, neow_talk_choices(), 0)
}

#[must_use]
pub fn neow_screen_for_stage(run: &RunState, stage: u32) -> EventScreen {
    match stage {
        0 => make_event_screen(Event::Neow, neow_talk_choices(), 0),
        1 => make_event_screen(Event::Neow, neow_option_choices(run), 1),
        _ => make_event_screen(Event::Neow, neow_leave_choices(), 2),
    }
}

fn apply_neow_immediate_option(next: &mut RunState, option: GeneratedNeowOption) -> SimResult<()> {
    match option.drawback {
        NeowDrawback::Curse => {
            apply_neow_curse_drawback(next);
        }
        drawback => apply_neow_simple_drawback(next, drawback),
    }

    match option.reward {
        NeowRewardType::OneRandomRareCard => {
            let reward = generate_neow_card_reward(next.event_rng_seed as i64, option.reward);
            for content_id in reward.cards {
                next.gain_deck_card(content_id);
            }
        }
        NeowRewardType::ThreeSmallPotions => {
            let reward = generate_neow_three_potions(next.event_rng_seed as i64);
            for potion in reward.potions {
                if next.can_gain_potions() && next.open_potion_slots() > 0 {
                    next.gain_potion(potion)
                        .expect("open potion slot validated");
                }
            }
            next.potion_rng_counter = reward.potion_rng_counter;
            super::reward::consume_neow_three_potions_hidden_card_reward(next);
        }
        NeowRewardType::RandomCommonRelic | NeowRewardType::OneRareRelic => {
            apply_neow_relic_reward(next, option.reward);
        }
        NeowRewardType::TenPercentHpBonus
        | NeowRewardType::TwentyPercentHpBonus
        | NeowRewardType::HundredGold
        | NeowRewardType::TwoFiftyGold => apply_neow_simple_reward(next, option.reward),
        NeowRewardType::ThreeEnemyKill => apply_neow_lament_reward(next),
        NeowRewardType::BossRelic => {
            apply_neow_boss_swap(next);
        }
        NeowRewardType::RemoveCard
        | NeowRewardType::RemoveTwo
        | NeowRewardType::UpgradeCard
        | NeowRewardType::TransformCard
        | NeowRewardType::TransformTwoCards => {
            open_neow_reward_grid(next, option.reward);
            return Ok(());
        }
        NeowRewardType::ThreeCards | NeowRewardType::ThreeRareCards => {
            open_neow_card_reward(next, option.reward);
            return Ok(());
        }
        NeowRewardType::RandomColorless | NeowRewardType::RandomColorlessTwo => {
            open_neow_colorless_card_reward(next, option.reward);
            return Ok(());
        }
    }

    next.event = Some(make_event_screen(Event::Neow, neow_leave_choices(), 2));
    Ok(())
}

fn open_neow_card_reward(run: &mut RunState, reward_type: NeowRewardType) {
    let reward = generate_neow_card_reward(run.event_rng_seed as i64, reward_type);
    open_neow_card_reward_choices(run, reward.cards);
    run.event_rng_counter = reward.neow_rng_counter;
}

fn open_neow_colorless_card_reward(run: &mut RunState, reward_type: NeowRewardType) {
    let reward = generate_neow_colorless_reward_with_card_rng_counter(
        run.event_rng_seed as i64,
        reward_type,
        run.card_rng_counter,
    );
    open_neow_card_reward_choices(run, reward.cards);
    run.event_rng_counter = reward.neow_rng_counter;
    run.card_rng_counter = reward.card_rng_counter;
}

fn open_neow_card_reward_choices(run: &mut RunState, cards: Vec<ContentId>) {
    let next_card_id = run.next_card_instance_id();
    let choices = cards
        .into_iter()
        .enumerate()
        .map(|(index, content_id)| {
            CardInstance::new(CardId::new(next_card_id + index as u64), content_id)
        })
        .collect();
    run.phase = RunPhase::Reward;
    run.event = Some(make_event_screen(Event::Neow, neow_leave_choices(), 2));
    run.reward = Some(RewardScreen {
        continuation: crate::RewardContinuation::None,
        choices,
        queued_card_rewards: Vec::new(),
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer: None,
        relic_key_offer: None,
        pending_relic_offer: None,
        pending_relic_key_offer: None,
        queued_relic_key_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_active: true,
        card_reward_pending: false,
        pending_card_reward_count: 1,
    });
}

#[must_use]
pub fn legal_event_actions(run: &RunState) -> Vec<EventAction> {
    if run.phase != RunPhase::Event {
        return Vec::new();
    }

    run.event
        .as_ref()
        .map(|event| {
            event
                .choices
                .iter()
                .enumerate()
                .map(|(choice_index, _)| EventAction::Choose { choice_index })
                .collect()
        })
        .unwrap_or_default()
}

pub fn validate_event_action(run: &RunState, action: EventAction) -> SimResult<()> {
    run.validate()?;

    if run.phase != RunPhase::Event {
        return Err(SimError::IllegalAction("event actions require event phase"));
    }

    let event = run
        .event
        .as_ref()
        .ok_or(SimError::InvalidState("event screen is missing"))?;

    match action {
        EventAction::Choose { choice_index } => {
            if event.choices.get(choice_index).is_some() {
                Ok(())
            } else {
                Err(SimError::IllegalAction("event choice is not available"))
            }
        }
    }
}

fn scrap_ooze_success(next: &mut RunState) {
    let key = super::reward::roll_event_relic_reward(next, next.current_act);
    next.gain_relic_key(key);
    next.event = Some(EventScreen {
        event: Event::ScrapOoze,
        choices: scrap_ooze_choices(2),
        stage: 2,
        event_data: 0,
    });
}

pub fn apply_event_action(run: &RunState, action: EventAction) -> SimResult<RunState> {
    validate_event_action(run, action)?;

    let mut next = run.clone();
    let EventAction::Choose { choice_index } = action;
    let screen = next.event.as_ref().expect("validated event screen").clone();

    match screen.event {
        Event::Neow => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(neow_screen_for_stage(&next, 1));
            }
            1 => {
                let options = generate_neow_options(next.event_rng_seed as i64, next.player_max_hp);
                let option = options
                    .into_iter()
                    .find(|option| option.slot == choice_index)
                    .ok_or(SimError::IllegalAction("Neow option is not available"))?;
                apply_neow_immediate_option(&mut next, option)?;
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Neow",
                ));
            }
        },
        Event::SpireHeart => match screen.stage {
            0..=2 if choice_index == 0 => {
                let stage = screen.stage + 1;
                next.event = Some(make_event_screen(
                    Event::SpireHeart,
                    spire_heart_choices(stage),
                    stage,
                ));
            }
            3 if choice_index == 0 => {
                next.phase = RunPhase::Complete;
                next.event = Some(make_event_screen(Event::SpireHeart, Vec::new(), 4));
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Spire Heart",
                ));
            }
        },
        Event::BonfireElementals => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(make_event_screen(
                    Event::BonfireElementals,
                    bonfire_elementals_choices(1),
                    1,
                ));
            }
            1 if choice_index == 0 => {
                open_bonfire_elementals_grid(&mut next);
                if next.card_grid.is_none() {
                    next.event = Some(make_event_screen(
                        Event::BonfireElementals,
                        bonfire_elementals_choices(2),
                        2,
                    ));
                }
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Bonfire Elementals",
                ));
            }
        },
        Event::Designer => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(designer_screen(&next, 1, screen.event_data));
            }
            1 if choice_index == 0 => {
                let (cost, _, _, _) = designer_costs(&next);
                if next.gold < cost {
                    return Err(SimError::IllegalAction(
                        "not enough gold for Designer adjustments",
                    ));
                }
                if !designer_has_upgradable_card(&next) {
                    return Err(SimError::IllegalAction(
                        "Designer adjustments require an upgradable card",
                    ));
                }
                next.gold -= cost;
                if designer_event_data_adjustment_upgrades_one(screen.event_data) {
                    open_event_upgrade_return_to_event_grid(&mut next, Event::Designer);
                } else {
                    upgrade_random_deck_cards(&mut next, 2);
                    designer_done_screen(&mut next);
                }
            }
            1 if choice_index == 1 => {
                let (_, cost, _, _) = designer_costs(&next);
                if next.gold < cost {
                    return Err(SimError::IllegalAction(
                        "not enough gold for Designer cleanup",
                    ));
                }
                let card_count = designer_purgeable_card_count(&next);
                if designer_event_data_cleanup_removes_cards(screen.event_data) {
                    if card_count == 0 {
                        return Err(SimError::IllegalAction(
                            "Designer cleanup requires a purgeable card",
                        ));
                    }
                    next.gold -= cost;
                    open_event_remove_return_to_event_grid(&mut next, Event::Designer);
                } else {
                    if card_count < 2 {
                        return Err(SimError::IllegalAction(
                            "Designer cleanup requires two purgeable cards",
                        ));
                    }
                    next.gold -= cost;
                    open_event_transform_return_to_event_grid(&mut next, Event::Designer, 2);
                }
            }
            1 if choice_index == 2 => {
                let (_, _, cost, _) = designer_costs(&next);
                if next.gold < cost {
                    return Err(SimError::IllegalAction(
                        "not enough gold for Designer full service",
                    ));
                }
                if !designer_has_purgeable_card(&next) {
                    return Err(SimError::IllegalAction(
                        "Designer full service requires a purgeable card",
                    ));
                }
                next.gold -= cost;
                open_designer_remove_and_upgrade_grid(&mut next);
            }
            1 if choice_index == 3 => {
                let (_, _, _, hp_loss) = designer_costs(&next);
                lose_event_hp(&mut next, hp_loss);
                designer_done_screen(&mut next);
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Designer",
                ));
            }
        },
        Event::Duplicator => match screen.stage {
            0 if choice_index == 0 => {
                open_duplicator_card_grid(&mut next);
                if next.card_grid.is_none() {
                    next.event = Some(make_event_screen(
                        Event::Duplicator,
                        labeled_choices(&["Leave"]),
                        2,
                    ));
                }
            }
            0 if choice_index == 1 => {
                next.event = Some(make_event_screen(
                    Event::Duplicator,
                    labeled_choices(&["Leave"]),
                    2,
                ));
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Duplicator",
                ));
            }
        },
        Event::FountainOfCleansing => match screen.stage {
            0 if choice_index == 0 => {
                next.deck
                    .retain(|card| !fountain_removes_curse(card.content_id));
                next.event = Some(EventScreen {
                    event: Event::FountainOfCleansing,
                    choices: fountain_of_cleansing_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            1 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Fountain of Cleansing",
                ));
            }
        },
        Event::GoldenShrine => match screen.stage {
            0 if choice_index == 0 => {
                next.gain_gold(golden_shrine_gold(next.ascension));
                next.event = Some(make_event_screen(
                    Event::GoldenShrine,
                    golden_shrine_choices(1),
                    1,
                ));
            }
            0 if choice_index == 1 => {
                next.gain_gold(GOLDEN_SHRINE_DESECRATE_GOLD);
                next.queue_pending_obtain_card(REGRET_ID);
                next.event = Some(make_event_screen(
                    Event::GoldenShrine,
                    golden_shrine_choices(1),
                    1,
                ));
            }
            0 if choice_index == 2 => {
                next.event = Some(make_event_screen(
                    Event::GoldenShrine,
                    golden_shrine_choices(1),
                    1,
                ));
            }
            1 if choice_index == 0 => {
                next.flush_pending_obtain_cards();
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Golden Shrine",
                ));
            }
        },
        Event::AccursedBlacksmith => match screen.stage {
            0 if choice_index == 0 => {
                open_event_upgrade_return_to_event_grid(&mut next, Event::AccursedBlacksmith);
            }
            0 if choice_index == 1 => {
                next.gain_relic_key(RelicKey::WarpedTongs);
                next.pending_obtain_cards.push(PAIN_ID);
                next.event = Some(EventScreen {
                    event: Event::AccursedBlacksmith,
                    choices: labeled_choices(&["Leave"]),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 2 => {
                next.event = Some(EventScreen {
                    event: Event::WingStatue,
                    choices: wing_statue_choices(2, false),
                    stage: 2,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                next.flush_pending_obtain_cards();
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Accursed Blacksmith",
                ));
            }
        },
        Event::GoldenIdol => match screen.stage {
            0 if choice_index == 0 => {
                if has_relic_key(&next, RelicKey::GoldenIdol) {
                    next.gain_relic_key(RelicKey::Circlet);
                } else {
                    next.gain_relic_key(RelicKey::GoldenIdol);
                }
                next.event = Some(EventScreen {
                    event: Event::GoldenIdol,
                    choices: golden_idol_choices(1, next.player_max_hp, next.ascension),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            1 if choice_index == 0 => {
                // Target source uses ShowCardAndObtainEffect for the curse; the
                // card reaches the deck when that visual effect resolves.
                next.queue_pending_obtain_card(INJURY_ID);
                next.event = Some(EventScreen {
                    event: Event::GoldenIdol,
                    choices: golden_idol_choices(2, next.player_max_hp, next.ascension),
                    stage: 2,
                    event_data: 0,
                });
            }
            1 if choice_index == 1 => {
                let hp_loss = golden_idol_hp_loss(next.player_max_hp, next.ascension);
                lose_event_hp(&mut next, hp_loss);
                next.event = Some(EventScreen {
                    event: Event::GoldenIdol,
                    choices: golden_idol_choices(2, next.player_max_hp, next.ascension),
                    stage: 2,
                    event_data: hp_loss as u32,
                });
            }
            1 if choice_index == 2 => {
                let max_hp_loss = golden_idol_max_hp_loss(next.player_max_hp, next.ascension);
                next.player_max_hp = (next.player_max_hp - max_hp_loss).max(1);
                next.player_hp = next.player_hp.min(next.player_max_hp);
                next.event = Some(EventScreen {
                    event: Event::GoldenIdol,
                    choices: golden_idol_choices(2, next.player_max_hp, next.ascension),
                    stage: 2,
                    event_data: max_hp_loss as u32,
                });
            }
            2 if choice_index == 0 => {
                next.flush_pending_obtain_cards();
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Golden Idol",
                ));
            }
        },
        Event::WingStatue => match screen.stage {
            0 if choice_index == 0 => {
                lose_event_hp(&mut next, WING_STATUE_PRAY_HP_LOSS);
                next.event = Some(EventScreen {
                    event: Event::WingStatue,
                    choices: wing_statue_choices(1, false),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 && has_wing_statue_attack_card(&next) => {
                let gold = roll_wing_statue_gold(&mut next);
                next.gain_gold(gold);
                next.event = Some(EventScreen {
                    event: Event::WingStatue,
                    choices: wing_statue_choices(2, true),
                    stage: 2,
                    event_data: gold as u32,
                });
            }
            0 if choice_index == 2 => {
                next.event = Some(EventScreen {
                    event: Event::WingStatue,
                    choices: wing_statue_choices(2, false),
                    stage: 2,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::WingStatue,
                    choices: wing_statue_choices(2, false),
                    stage: 2,
                    event_data: 0,
                });
                open_event_remove_return_to_event_grid(&mut next, Event::WingStatue);
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Wing Statue",
                ));
            }
        },
        Event::WorldOfGoop => match screen.stage {
            0 if choice_index == 0 => {
                lose_event_hp(&mut next, WORLD_OF_GOOP_DAMAGE);
                next.gain_gold(WORLD_OF_GOOP_GOLD);
                next.event = Some(EventScreen {
                    event: Event::WorldOfGoop,
                    choices: world_of_goop_choices(1, screen.event_data as i32),
                    stage: 1,
                    event_data: screen.event_data,
                });
            }
            0 if choice_index == 1 => {
                let gold_loss = screen.event_data as i32;
                next.gold = (next.gold - gold_loss).max(0);
                next.event = Some(EventScreen {
                    event: Event::WorldOfGoop,
                    choices: world_of_goop_choices(1, gold_loss),
                    stage: 1,
                    event_data: screen.event_data,
                });
            }
            1 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for World of Goop",
                ));
            }
        },
        Event::WeMeetAgain => {
            match screen.stage {
                0 => {
                    let options = we_meet_again_options_from_event_data(screen.event_data);
                    let label = screen
                        .choices
                        .get(choice_index)
                        .ok_or(SimError::IllegalAction(
                            "We Meet Again choice is unavailable",
                        ))?
                        .label
                        .as_str();
                    match label {
                        "Give Potion" => {
                            let Some(slot) = options.potion_slot else {
                                return Err(SimError::IllegalAction(
                                    "We Meet Again potion option is unavailable",
                                ));
                            };
                            next.take_potion_slot(slot)?;
                            let act = next.current_act;
                            let key = roll_event_relic_reward(&mut next, act);
                            next.gain_relic_key(key);
                        }
                        "Give Gold" => {
                            if options.gold_amount <= 0 || next.gold < options.gold_amount {
                                return Err(SimError::IllegalAction(
                                    "We Meet Again gold option is unavailable",
                                ));
                            }
                            next.gold -= options.gold_amount;
                            let act = next.current_act;
                            let key = roll_event_relic_reward(&mut next, act);
                            next.gain_relic_key(key);
                        }
                        "Give Card" => {
                            let Some(card_index) = options.card_index else {
                                return Err(SimError::IllegalAction(
                                    "We Meet Again card option is unavailable",
                                ));
                            };
                            let card = next.deck.get(card_index).copied().ok_or(
                                SimError::InvalidState("We Meet Again card option is missing"),
                            )?;
                            next.remove_deck_card(card.id)
                                .expect("We Meet Again selected a deck card");
                            let act = next.current_act;
                            let key = roll_event_relic_reward(&mut next, act);
                            next.gain_relic_key(key);
                        }
                        "Attack" => {}
                        _ => {
                            return Err(SimError::IllegalAction(
                                "event choice is not implemented for We Meet Again",
                            ));
                        }
                    }
                    next.event = Some(EventScreen {
                        event: Event::WeMeetAgain,
                        choices: we_meet_again_choices(1, options),
                        stage: 1,
                        event_data: screen.event_data,
                    });
                }
                1 if choice_index == 0 => {
                    next.phase = RunPhase::Idle;
                    next.event = None;
                }
                _ => {
                    return Err(SimError::IllegalAction(
                        "event choice is not implemented for We Meet Again",
                    ));
                }
            }
        }
        Event::DeadAdventurer => match screen.stage {
            0 if choice_index == 0 => {
                let attempts = dead_adventurer_attempts(screen.event_data);
                let encounter_chance = dead_adventurer_encounter_chance(&next, attempts);
                let mut misc_rng = next.rng_for_stream(RunRngStream::Misc);
                let encounter = misc_rng.random_int(99) < encounter_chance;
                next.store_rng_counter(RunRngStream::Misc, &misc_rng);
                let event_data = dead_adventurer_event_data(
                    dead_adventurer_order(screen.event_data),
                    dead_adventurer_enemy(screen.event_data),
                    attempts + 1,
                );
                if encounter {
                    next.event = Some(dead_adventurer_screen(&next, 3, event_data));
                } else {
                    let reward = dead_adventurer_order(screen.event_data)[attempts as usize];
                    match reward {
                        0 => next.gain_gold(30),
                        2 => {
                            let act = next.current_act;
                            let relic = roll_event_relic_reward(&mut next, act);
                            next.gain_relic_key(relic);
                        }
                        _ => {}
                    }
                    let stage = if attempts + 1 >= 3 { 1 } else { 0 };
                    next.event = Some(dead_adventurer_screen(&next, stage, event_data));
                }
            }
            0 if choice_index == 1 => {
                next.event = Some(dead_adventurer_screen(&next, 1, screen.event_data));
            }
            1 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            2 if choice_index == 0 => {
                if dead_adventurer_pending_encounter(screen.event_data) {
                    next.event = Some(dead_adventurer_screen(&next, 3, screen.event_data));
                } else {
                    let attempts = dead_adventurer_attempts(screen.event_data);
                    let reward = dead_adventurer_order(screen.event_data)
                        [attempts.saturating_sub(1) as usize];
                    match reward {
                        0 => next.gain_gold(30),
                        2 => {
                            let act = next.current_act;
                            let relic = roll_event_relic_reward(&mut next, act);
                            next.gain_relic_key(relic);
                        }
                        _ => {}
                    }
                    let stage = if attempts >= 3 { 1 } else { 0 };
                    next.event = Some(dead_adventurer_screen(&next, stage, screen.event_data));
                }
            }
            2 if choice_index == 1 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            3 if choice_index == 0 => {
                // DeadAdventurer adds its 25-35 combat gold with miscRng when
                // the search fails. A previously found GOLD reward contributes
                // the fixed extra 30 when the player then enters combat.
                let mut misc_rng = next.rng_for_stream(RunRngStream::Misc);
                next.pending_event_combat_gold_offer = 30 + misc_rng.random_int_range(25, 35);
                next.store_rng_counter(RunRngStream::Misc, &misc_rng);
                // Dead Adventurer marks the encounter as an elite fight, so
                // the post-combat screen always contains the normal elite
                // relic reward. The shuffled search reward is unrelated.
                let mut relic_rng = next.rng_for_stream(RunRngStream::Relic);
                let relic_tier = target_elite_relic_tier(&mut relic_rng);
                next.store_rng_counter(RunRngStream::Relic, &relic_rng);
                next.pending_event_combat_relic_key_offer =
                    Some(roll_relic_reward(&mut next, relic_tier));
                match dead_adventurer_enemy(screen.event_data) {
                    0 => enter_event_combat(&mut next, &[&SENTRY_A0, &SENTRY_A0, &SENTRY_A0]),
                    1 => enter_event_combat(&mut next, &[&GREMLIN_NOB_A0]),
                    _ => enter_event_combat(&mut next, &[&LAGAVULIN_A0]),
                }
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Dead Adventurer",
                ));
            }
        },
        Event::HypnotizingColoredMushrooms => match screen.stage {
            0 if choice_index == 0 => {
                let mut misc_rng = next.rng_for_stream(RunRngStream::Misc);
                next.pending_event_combat_gold_offer = misc_rng.random_int_range(20, 30);
                next.store_rng_counter(RunRngStream::Misc, &misc_rng);
                next.pending_event_combat_relic_key_offer =
                    Some(if has_relic_key(&next, RelicKey::OddMushroom) {
                        RelicKey::Circlet
                    } else {
                        RelicKey::OddMushroom
                    });
                // Target MonsterHelper encounter "The Mushroom Lair" creates
                // three FungiBeast instances (desktop-1.0.jar case 18).
                enter_event_combat(
                    &mut next,
                    &[&FUNGI_BEAST_A0, &FUNGI_BEAST_A0, &FUNGI_BEAST_A0],
                );
            }
            0 if choice_index == 1 => {
                let heal = next.player_max_hp * 25 / 100;
                next.heal_player(heal);
                next.queue_pending_obtain_card(PARASITE_ID);
                next.event = Some(EventScreen {
                    event: Event::HypnotizingColoredMushrooms,
                    choices: hypnotizing_colored_mushrooms_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                next.flush_pending_obtain_cards();
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Hypnotizing Colored Mushrooms",
                ));
            }
        },
        Event::Nloth => match screen.stage {
            0 if choice_index <= 1 => {
                let owned = nloth_owned_relic_keys(&next);
                let offered_index = nloth_choice_index(screen.event_data, choice_index);
                let offered = *owned
                    .get(offered_index)
                    .ok_or(SimError::InvalidState("N'loth offered relic is missing"))?;
                if has_relic_key(&next, RelicKey::NlothsGift) {
                    next.gain_relic_key(RelicKey::Circlet);
                } else {
                    if !remove_relic_key(&mut next, offered) {
                        return Err(SimError::InvalidState(
                            "N'loth offered relic is no longer owned",
                        ));
                    }
                    next.gain_relic_key(RelicKey::NlothsGift);
                }
                next.event = Some(EventScreen {
                    event: Event::Nloth,
                    choices: labeled_choices(&["Leave"]),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 2 => {
                next.event = Some(EventScreen {
                    event: Event::Nloth,
                    choices: labeled_choices(&["Leave"]),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for N'loth",
                ));
            }
        },
        Event::TheCleric if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::TheJoust => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(make_event_screen(Event::TheJoust, joust_choices(1), 1));
            }
            1 if choice_index <= 1 => {
                if next.gold < 50 {
                    return Err(SimError::IllegalAction("not enough gold"));
                }
                next.gold -= 50;
                next.event = Some(EventScreen {
                    event: Event::TheJoust,
                    choices: joust_choices(2),
                    stage: 2,
                    event_data: joust_event_data(choice_index == 1, false),
                });
            }
            2 if choice_index == 0 => {
                let mut misc_rng = next.rng_for_stream(RunRngStream::Misc);
                let owner_wins = misc_rng.random_float() < 0.3;
                next.store_rng_counter(RunRngStream::Misc, &misc_rng);
                next.event = Some(EventScreen {
                    event: Event::TheJoust,
                    choices: joust_choices(3),
                    stage: 3,
                    event_data: joust_event_data(screen.event_data & 1 != 0, owner_wins),
                });
            }
            3 if choice_index == 0 => {
                let bet_for = screen.event_data & 1 != 0;
                let owner_wins = screen.event_data & 2 != 0;
                if owner_wins && bet_for {
                    next.gain_gold(250);
                } else if !owner_wins && !bet_for {
                    next.gain_gold(100);
                }
                next.event = Some(make_event_screen(Event::TheJoust, joust_choices(4), 4));
            }
            4 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for The Joust",
                ));
            }
        },
        Event::TheWomanInBlue if screen.stage == 0 && choice_index < 3 => {
            let costs = [20, 30, 40];
            let cost = costs[choice_index];
            if next.gold < cost {
                return Err(SimError::IllegalAction("not enough gold"));
            }
            next.gold -= cost;
            let mut potion_rng = next.rng_for_stream(RunRngStream::Potion);
            let potion_offers = (0..=choice_index)
                .map(|_| target_uniform_random_potion(&mut potion_rng))
                .collect();
            next.store_rng_counter(RunRngStream::Potion, &potion_rng);
            next.phase = RunPhase::Reward;
            next.event = None;
            next.reward = Some(RewardScreen {
                continuation: crate::RewardContinuation::None,
                choices: Vec::new(),
                queued_card_rewards: Vec::new(),
                gold_offer: 0,
                stolen_gold_offer: 0,
                potion_offer: None,
                potion_offers,
                relic_offer: None,
                relic_key_offer: None,
                pending_relic_offer: None,
                pending_relic_key_offer: None,
                queued_relic_key_offers: Vec::new(),
                boss_relic_choices: Vec::new(),
                card_reward_active: false,
                card_reward_pending: false,
                pending_card_reward_count: 0,
            });
        }
        Event::TheWomanInBlue if screen.stage == 0 && choice_index == 3 => {
            if next.ascension >= 15 {
                let hp_loss = (next.player_max_hp + 19) / 20;
                next.player_hp = (next.player_hp - hp_loss).max(0);
            }
            next.event = Some(make_event_screen(
                Event::TheWomanInBlue,
                labeled_choices(&["Leave"]),
                1,
            ));
        }
        Event::TheWomanInBlue if screen.stage == 1 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::TheCleric if screen.stage == 0 && choice_index == 0 => {
            if next.gold < 35 {
                return Err(SimError::IllegalAction("not enough gold"));
            }
            next.gold -= 35;
            let heal = next.player_max_hp * 25 / 100;
            next.heal_player(heal);
            next.event = Some(make_event_screen(
                Event::TheCleric,
                vec![EventChoice {
                    label: "Leave".to_owned(),
                }],
                1,
            ));
        }
        Event::TheCleric if screen.stage == 0 && choice_index == 1 => {
            if purgeable_event_card_count(&next) == 0 {
                next.event = Some(make_event_screen(
                    Event::TheCleric,
                    vec![EventChoice {
                        label: "Leave".to_owned(),
                    }],
                    1,
                ));
                return Ok(next);
            }
            let cost = cleric_purify_cost(&next);
            if next.gold < cost {
                return Err(SimError::IllegalAction("not enough gold"));
            }
            next.gold -= cost;
            open_event_remove_return_to_event_grid(&mut next, Event::TheCleric);
        }
        Event::TheCleric if screen.stage == 0 && choice_index == 2 => {
            next.event = Some(make_event_screen(
                Event::TheCleric,
                vec![EventChoice {
                    label: "Leave".to_owned(),
                }],
                1,
            ));
        }
        Event::ShiningLight if screen.stage == 0 && choice_index == 0 => {
            let loss = shining_light_hp_loss(next.player_max_hp);
            next.player_hp = (next.player_hp - loss).max(0);
            upgrade_random_deck_cards(&mut next, 2);
            next.event = Some(make_event_screen(
                Event::ShiningLight,
                labeled_choices(&["Leave"]),
                1,
            ));
        }
        Event::ShiningLight if screen.stage == 0 && choice_index == 1 => {
            next.event = Some(make_event_screen(
                Event::ShiningLight,
                labeled_choices(&["Leave"]),
                1,
            ));
        }
        Event::ShiningLight if screen.stage == 1 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::Purifier if screen.stage == 0 && choice_index == 0 => {
            open_event_remove_return_to_event_grid(&mut next, Event::Purifier);
        }
        Event::Purifier if screen.stage == 0 && choice_index == 1 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::Purifier if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::Transmorgrifier if screen.stage == 0 && choice_index == 0 => {
            open_event_transform_return_to_event_grid(&mut next, Event::Transmorgrifier, 1);
        }
        Event::Transmorgrifier if screen.stage == 0 && choice_index == 1 => {
            next.event = Some(make_event_screen(
                Event::Transmorgrifier,
                labeled_choices(&["Leave"]),
                1,
            ));
        }
        Event::Transmorgrifier if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::UpgradeShrine if screen.stage == 0 && choice_index == 0 => {
            open_event_upgrade_return_to_event_grid(&mut next, Event::UpgradeShrine);
        }
        Event::UpgradeShrine if screen.stage == 0 && choice_index == 1 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::ScrapOoze => match screen.stage {
            0 if choice_index == 0 => {
                let hp_loss = scrap_ooze_hp_loss(next.ascension, screen.event_data);
                next.player_hp = (next.player_hp - hp_loss).max(0);
                if roll_scrap_ooze_relic(&mut next, screen.event_data) {
                    scrap_ooze_success(&mut next);
                } else {
                    next.event = Some(EventScreen {
                        event: Event::ScrapOoze,
                        choices: scrap_ooze_choices(1),
                        stage: 1,
                        event_data: screen.event_data + 1,
                    });
                }
            }
            0 if choice_index == 1 => {
                next.event = Some(EventScreen {
                    event: Event::ScrapOoze,
                    choices: scrap_ooze_choices(2),
                    stage: 2,
                    event_data: screen.event_data,
                });
            }
            1 if choice_index == 0 => {
                let hp_loss = scrap_ooze_hp_loss(next.ascension, screen.event_data);
                next.player_hp = (next.player_hp - hp_loss).max(0);
                if roll_scrap_ooze_relic(&mut next, screen.event_data) {
                    scrap_ooze_success(&mut next);
                } else {
                    next.event = Some(EventScreen {
                        event: Event::ScrapOoze,
                        choices: scrap_ooze_choices(1),
                        stage: 1,
                        event_data: screen.event_data + 1,
                    });
                }
            }
            1 if choice_index == 1 => {
                next.event = Some(EventScreen {
                    event: Event::ScrapOoze,
                    choices: scrap_ooze_choices(2),
                    stage: 2,
                    event_data: screen.event_data,
                });
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Scrap Ooze",
                ));
            }
        },
        Event::FaceTrader => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::FaceTrader,
                    choices: face_trader_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                let damage = face_trader_damage(next.player_max_hp);
                next.gain_gold(face_trader_gold(next.ascension));
                lose_event_hp(&mut next, damage);
                next.event = Some(EventScreen {
                    event: Event::FaceTrader,
                    choices: face_trader_choices(2),
                    stage: 2,
                    event_data: 0,
                });
            }
            1 if choice_index == 1 => {
                let key = roll_face_trader_relic(&mut next);
                next.gain_relic_key(key);
                next.event = Some(EventScreen {
                    event: Event::FaceTrader,
                    choices: face_trader_choices(2),
                    stage: 2,
                    event_data: 0,
                });
            }
            1 if choice_index == 2 => {
                next.event = Some(EventScreen {
                    event: Event::FaceTrader,
                    choices: face_trader_choices(2),
                    stage: 2,
                    event_data: 0,
                });
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Face Trader",
                ));
            }
        },
        Event::NoteForYourself => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::NoteForYourself,
                    choices: note_for_yourself_choices_for_run(&next, 1),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                let note = note_card_for_run(&next);
                next.add_deck_card(note);
                open_event_remove_return_to_event_grid(&mut next, Event::NoteForYourself);
            }
            1 if choice_index == 1 => {
                next.event = Some(EventScreen {
                    event: Event::NoteForYourself,
                    choices: note_for_yourself_choices(2),
                    stage: 2,
                    event_data: 0,
                });
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Note For Yourself",
                ));
            }
        },
        Event::SecretPortal => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(make_event_screen(
                    Event::SecretPortal,
                    labeled_choices(&["Continue"]),
                    1,
                ));
            }
            0 if choice_index == 1 => {
                next.event = Some(make_event_screen(
                    Event::SecretPortal,
                    labeled_choices(&["Leave"]),
                    2,
                ));
            }
            1 if choice_index == 0 => {
                next.current_room_override = Some(crate::map::RoomKind::Boss);
                enter_secret_portal_boss_combat(&mut next)?;
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Secret Portal",
                ));
            }
        },
        Event::Falling => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::Falling,
                    choices: falling_choices(&next, 1),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 => {
                let card_types = falling_card_types(&next);
                if card_types.is_empty() {
                    next.event = Some(make_event_screen(
                        Event::Falling,
                        labeled_choices(&["Leave"]),
                        2,
                    ));
                } else if let Some(card_type) = card_types.get(choice_index).copied() {
                    open_falling_card_grid(&mut next, card_type);
                    if next.card_grid.is_none() {
                        next.event = Some(make_event_screen(
                            Event::Falling,
                            labeled_choices(&["Leave"]),
                            2,
                        ));
                    }
                } else {
                    return Err(SimError::IllegalAction(
                        "event choice is not implemented for Falling",
                    ));
                }
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Falling",
                ));
            }
        },
        Event::MoaiHead => match screen.stage {
            0 if choice_index == 0 => {
                let loss = rounded_event_percent(
                    next.player_max_hp,
                    if next.ascension >= 15 { 0.18 } else { 0.125 },
                );
                next.player_max_hp = (next.player_max_hp - loss).max(1);
                next.player_hp = next.player_hp.min(next.player_max_hp);
                next.heal_player(next.player_max_hp);
                next.event = Some(make_event_screen(
                    Event::MoaiHead,
                    moai_choices(&next, 1),
                    1,
                ));
            }
            0 if has_relic_key(&next, RelicKey::GoldenIdol) && choice_index == 1 => {
                remove_relic_key(&mut next, RelicKey::GoldenIdol);
                next.gain_gold(333);
                next.event = Some(make_event_screen(
                    Event::MoaiHead,
                    moai_choices(&next, 1),
                    1,
                ));
            }
            0 if choice_index == screen.choices.len().saturating_sub(1) => {
                next.event = Some(make_event_screen(
                    Event::MoaiHead,
                    moai_choices(&next, 1),
                    1,
                ));
            }
            1 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for The Moai Head",
                ));
            }
        },
        Event::MysteriousSphere => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(make_event_screen(
                    Event::MysteriousSphere,
                    mysterious_sphere_choices(1),
                    1,
                ));
            }
            0 if choice_index == 1 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            1 if choice_index == 0 => {
                let mut misc_rng = next.rng_for_stream(RunRngStream::Misc);
                next.pending_event_combat_gold_offer = misc_rng.random_int_range(45, 55);
                next.store_rng_counter(RunRngStream::Misc, &misc_rng);
                next.pending_event_combat_relic_key_offer =
                    Some(super::reward::roll_relic_reward(&mut next, RelicTier::Rare));
                enter_event_combat(&mut next, &[&ORB_WALKER_A0, &ORB_WALKER_A0]);
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Mysterious Sphere",
                ));
            }
        },
        Event::SensoryStone if screen.stage == 0 && choice_index == 0 => {
            let memory = roll_sensory_memory(&mut next);
            next.event = Some(EventScreen {
                event: Event::SensoryStone,
                choices: sensory_stone_choices(1),
                stage: 1,
                event_data: memory,
            });
        }
        Event::SensoryStone if screen.stage == 1 && choice_index < 3 => {
            let hp_loss = match choice_index {
                1 => 5,
                2 => 10,
                _ => 0,
            };
            if hp_loss > 0 {
                lose_event_hp(&mut next, hp_loss);
            }
            let reward_count = u8::try_from(choice_index + 1)
                .expect("Sensory Stone offers at most three card rewards");
            let card_choice_count = reward_card_choice_count(&next);
            let mut card_rng = next.rng_for_stream(RunRngStream::CardReward);
            let mut rarity_factor = next.card_rarity_factor;
            let mut next_card_id = next.next_card_instance_id();
            let mut queued_card_rewards = Vec::with_capacity(usize::from(reward_count));
            for _ in 0..reward_count {
                let cards = target_colorless_card_reward_choices_with_count(
                    &mut card_rng,
                    &mut rarity_factor,
                    next_card_id,
                    card_choice_count,
                );
                next_card_id += cards.len() as u64;
                queued_card_rewards.push(cards);
            }
            next.card_rarity_factor = rarity_factor;
            next.store_rng_counter(RunRngStream::CardReward, &card_rng);
            next.phase = RunPhase::Reward;
            next.event = Some(make_event_screen(
                Event::SensoryStone,
                labeled_choices(&["Leave"]),
                2,
            ));
            next.reward = Some(RewardScreen {
                continuation: crate::RewardContinuation::None,
                choices: Vec::new(),
                queued_card_rewards,
                gold_offer: 0,
                stolen_gold_offer: 0,
                potion_offer: None,
                potion_offers: Vec::new(),
                relic_offer: None,
                relic_key_offer: None,
                pending_relic_offer: None,
                pending_relic_key_offer: None,
                queued_relic_key_offers: Vec::new(),
                boss_relic_choices: Vec::new(),
                card_reward_active: false,
                card_reward_pending: true,
                pending_card_reward_count: reward_count,
            });
        }
        Event::SensoryStone if screen.stage == 2 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::WindingHalls => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::WindingHalls,
                    choices: winding_halls_choices(&next, 1),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                let loss = rounded_event_percent(
                    next.player_max_hp,
                    if next.ascension >= 15 { 0.18 } else { 0.125 },
                );
                lose_event_hp(&mut next, loss);
                next.queue_pending_obtain_card(MADNESS_ID);
                next.queue_pending_obtain_card(MADNESS_ID);
                next.event = Some(make_event_screen(
                    Event::WindingHalls,
                    winding_halls_choices(&next, 2),
                    2,
                ));
            }
            1 if choice_index == 1 => {
                let heal = rounded_event_percent(
                    next.player_max_hp,
                    if next.ascension >= 15 { 0.20 } else { 0.25 },
                );
                next.heal_player(heal);
                next.queue_pending_obtain_card(WRITHE_ID);
                next.event = Some(make_event_screen(
                    Event::WindingHalls,
                    winding_halls_choices(&next, 2),
                    2,
                ));
            }
            1 if choice_index == 2 => {
                let loss = rounded_event_percent(next.player_max_hp, 0.05);
                next.player_max_hp = (next.player_max_hp - loss).max(1);
                next.player_hp = next.player_hp.min(next.player_max_hp);
                next.event = Some(make_event_screen(
                    Event::WindingHalls,
                    winding_halls_choices(&next, 2),
                    2,
                ));
            }
            2 if choice_index == 0 => {
                next.flush_pending_obtain_cards();
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Winding Halls",
                ));
            }
        },
        Event::BigFish => match screen.stage {
            0 if choice_index == 0 => {
                let heal = next.player_max_hp / 3;
                next.heal_player(heal);
                next.event = Some(EventScreen {
                    event: Event::BigFish,
                    choices: big_fish_choices(1),
                    stage: 1,
                    event_data: heal as u32,
                });
            }
            0 if choice_index == 1 => {
                next.player_max_hp += BIG_FISH_MAX_HP_GAIN;
                next.player_hp += BIG_FISH_MAX_HP_GAIN;
                next.event = Some(EventScreen {
                    event: Event::BigFish,
                    choices: big_fish_choices(1),
                    stage: 1,
                    event_data: BIG_FISH_MAX_HP_GAIN as u32,
                });
            }
            0 if choice_index == 2 => {
                let act = next.current_act;
                let key = super::reward::roll_event_relic_reward(&mut next, act);
                next.gain_relic_key(key);
                // Target source uses ShowCardAndObtainEffect for the curse; the
                // relic is obtained immediately, but the card reaches the deck
                // when that visual effect resolves.
                next.queue_pending_obtain_card(REGRET_ID);
                next.event = Some(EventScreen {
                    event: Event::BigFish,
                    choices: big_fish_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                next.flush_pending_obtain_cards();
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Big Fish",
                ));
            }
        },
        Event::TheSsssserpent => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::TheSsssserpent,
                    choices: sssssserpent_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                next.event = Some(EventScreen {
                    event: Event::TheSsssserpent,
                    choices: sssssserpent_choices(3),
                    stage: 3,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                next.gain_gold(SSSSSERPENT_GOLD);
                next.event = Some(EventScreen {
                    event: Event::TheSsssserpent,
                    choices: sssssserpent_choices(2),
                    stage: 2,
                    event_data: 0,
                });
            }
            2 if choice_index == 0 => {
                next.gain_deck_card(DOUBT_ID);
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            3 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for The Ssssserpent",
                ));
            }
        },
        Event::BackToBasics if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::BackToBasics if choice_index == 1 => {
            upgrade_starter_strikes_and_defends(&mut next);
            next.event = Some(EventScreen {
                event: Event::BackToBasics,
                choices: labeled_choices(&["Leave"]),
                stage: 1,
                event_data: 0,
            });
        }
        Event::BackToBasics if choice_index == 0 => {
            open_event_remove_grid(&mut next);
            if next.card_grid.is_none() {
                next.event = Some(EventScreen {
                    event: Event::BackToBasics,
                    choices: labeled_choices(&["Leave"]),
                    stage: 1,
                    event_data: 0,
                });
            }
        }
        Event::LivingWall if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::LivingWall if screen.stage == 0 && choice_index == 0 => {
            open_event_remove_return_to_event_grid(&mut next, Event::LivingWall);
            if next.card_grid.is_none() {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
        }
        Event::LivingWall if screen.stage == 0 && choice_index == 1 => {
            open_event_transform_return_to_event_grid(&mut next, Event::LivingWall, 1);
            if next.card_grid.is_none() {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
        }
        Event::LivingWall if screen.stage == 0 && choice_index == 2 => {
            open_event_upgrade_return_to_event_grid(&mut next, Event::LivingWall);
            if next.card_grid.is_none() {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
        }
        Event::TheLibrary if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::TheLibrary if screen.stage == 0 && choice_index == 1 => {
            let heal = the_library_heal_for_ascension(next.player_max_hp, next.ascension);
            next.heal_player(heal);
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::TheLibrary if screen.stage == 0 && choice_index == 0 => {
            open_the_library_read_grid(&mut next);
        }
        Event::TheMausoleum | Event::Vampires
            if choice_index == screen.choices.len().saturating_sub(1) =>
        {
            if screen.event == Event::TheMausoleum {
                next.flush_pending_obtain_cards();
            }
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::TheMausoleum if screen.stage == 0 && choice_index == 0 => {
            if roll_mausoleum_curses_player(&mut next) {
                next.queue_pending_obtain_card(WRITHE_ID);
            }
            let act = next.current_act;
            let key = super::reward::roll_event_relic_reward(&mut next, act);
            next.gain_relic_key(key);
            next.event = Some(EventScreen {
                event: Event::TheMausoleum,
                choices: labeled_choices(&["Leave"]),
                stage: 1,
                event_data: 0,
            });
        }
        Event::Vampires if choice_index == 0 => {
            let loss = vampires_max_hp_loss(next.player_max_hp);
            next.player_max_hp = (next.player_max_hp - loss).max(1);
            next.player_hp = next.player_hp.min(next.player_max_hp);
            replace_starter_strikes_with_bites(&mut next);
            next.event = Some(EventScreen {
                event: Event::Vampires,
                choices: labeled_choices(&["Leave"]),
                stage: 1,
                event_data: 0,
            });
        }
        Event::Vampires if choice_index == 1 && screen.choices.len() == 3 => {
            if !next.relics.contains(&Relic::BloodVial) {
                return Err(SimError::IllegalAction(
                    "Blood Vial choice requires Blood Vial",
                ));
            }
            next.relics.retain(|relic| *relic != Relic::BloodVial);
            replace_starter_strikes_with_bites(&mut next);
            next.event = Some(EventScreen {
                event: Event::Vampires,
                choices: labeled_choices(&["Leave"]),
                stage: 1,
                event_data: 0,
            });
        }
        Event::CursedTome => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::CursedTome,
                    choices: cursed_tome_choices(1, next.ascension),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                next.event = Some(EventScreen {
                    event: Event::CursedTome,
                    choices: cursed_tome_choices(5, next.ascension),
                    stage: 5,
                    event_data: screen.event_data,
                });
            }
            1 if choice_index == 0 => {
                lose_event_hp(&mut next, CURSED_TOME_PAGE_1_HP_LOSS);
                next.event = Some(EventScreen {
                    event: Event::CursedTome,
                    choices: cursed_tome_choices(2, next.ascension),
                    stage: 2,
                    event_data: screen.event_data + CURSED_TOME_PAGE_1_HP_LOSS as u32,
                });
            }
            2 if choice_index == 0 => {
                lose_event_hp(&mut next, CURSED_TOME_PAGE_2_HP_LOSS);
                next.event = Some(EventScreen {
                    event: Event::CursedTome,
                    choices: cursed_tome_choices(3, next.ascension),
                    stage: 3,
                    event_data: screen.event_data + CURSED_TOME_PAGE_2_HP_LOSS as u32,
                });
            }
            3 if choice_index == 0 => {
                lose_event_hp(&mut next, CURSED_TOME_PAGE_3_HP_LOSS);
                next.event = Some(EventScreen {
                    event: Event::CursedTome,
                    choices: cursed_tome_choices(4, next.ascension),
                    stage: 4,
                    event_data: screen.event_data + CURSED_TOME_PAGE_3_HP_LOSS as u32,
                });
            }
            4 if choice_index == 0 => {
                let hp_loss = cursed_tome_final_hp_loss(next.ascension);
                lose_event_hp(&mut next, hp_loss);
                let key = choose_cursed_tome_book(&mut next);
                open_cursed_tome_book_reward(&mut next, key);
            }
            4 if choice_index == 1 => {
                lose_event_hp(&mut next, CURSED_TOME_STOP_HP_LOSS);
                next.event = Some(EventScreen {
                    event: Event::CursedTome,
                    choices: cursed_tome_choices(5, next.ascension),
                    stage: 5,
                    event_data: screen.event_data + CURSED_TOME_STOP_HP_LOSS as u32,
                });
            }
            5 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Cursed Tome",
                ));
            }
        },
        Event::Nest => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::Nest,
                    choices: nest_choices(1, next.ascension),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                next.gain_gold(nest_gold_gain(next.ascension));
                next.event = Some(EventScreen {
                    event: Event::Nest,
                    choices: nest_choices(2, next.ascension),
                    stage: 2,
                    event_data: 0,
                });
            }
            1 if choice_index == 1 => {
                lose_event_hp(&mut next, NEST_HP_LOSS);
                next.queue_pending_obtain_card(RITUAL_DAGGER_ID);
                next.event = Some(EventScreen {
                    event: Event::Nest,
                    choices: nest_choices(2, next.ascension),
                    stage: 2,
                    event_data: 0,
                });
            }
            2 if choice_index == 0 => {
                next.flush_pending_obtain_cards();
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Nest",
                ));
            }
        },
        Event::Beggar => match screen.stage {
            0 if choice_index == 0 => {
                if next.gold < BEGGAR_GOLD_COST {
                    return Err(SimError::IllegalAction("not enough gold"));
                }
                next.gold -= BEGGAR_GOLD_COST;
                next.event = Some(EventScreen {
                    event: Event::Beggar,
                    choices: beggar_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            1 if choice_index == 0 => {
                open_event_remove_grid(&mut next);
                next.event = Some(EventScreen {
                    event: Event::Beggar,
                    choices: beggar_choices(2),
                    stage: 2,
                    event_data: 0,
                });
                if next.card_grid.is_none() {
                    next.phase = RunPhase::Idle;
                    next.event = None;
                }
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Beggar",
                ));
            }
        },
        Event::Addict => match screen.stage {
            0 if choice_index == 0 => {
                if next.gold < ADDICT_GOLD_COST {
                    return Err(SimError::IllegalAction("not enough gold"));
                }
                next.gold -= ADDICT_GOLD_COST;
                let act = next.current_act;
                let key = super::reward::roll_event_relic_reward(&mut next, act);
                next.gain_relic_key(key);
                next.event = Some(EventScreen {
                    event: Event::Addict,
                    choices: addict_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                next.gain_deck_card(SHAME_ID);
                let act = next.current_act;
                let key = super::reward::roll_event_relic_reward(&mut next, act);
                next.gain_relic_key(key);
                next.event = Some(EventScreen {
                    event: Event::Addict,
                    choices: addict_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 2 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            1 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Addict",
                ));
            }
        },
        Event::ForgottenAltar => match screen.stage {
            0 if next.relics.contains(&Relic::GoldenIdol) && choice_index == 0 => {
                give_forgotten_altar_idol(&mut next)?;
                next.event = Some(EventScreen {
                    event: Event::ForgottenAltar,
                    choices: forgotten_altar_choices(1, false),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == usize::from(next.relics.contains(&Relic::GoldenIdol)) => {
                let hp_loss = forgotten_altar_hp_loss(next.player_max_hp, next.ascension);
                next.player_max_hp += FORGOTTEN_ALTAR_MAX_HP_GAIN;
                next.player_hp += FORGOTTEN_ALTAR_MAX_HP_GAIN;
                lose_event_hp(&mut next, hp_loss);
                next.event = Some(EventScreen {
                    event: Event::ForgottenAltar,
                    choices: forgotten_altar_choices(1, false),
                    stage: 1,
                    event_data: hp_loss as u32,
                });
            }
            0 if choice_index == usize::from(next.relics.contains(&Relic::GoldenIdol)) + 1 => {
                next.gain_deck_card(DECAY_ID);
                next.event = Some(EventScreen {
                    event: Event::ForgottenAltar,
                    choices: forgotten_altar_choices(1, false),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Forgotten Altar",
                ));
            }
        },
        Event::Ghosts => match screen.stage {
            0 if choice_index == 0 => {
                let loss = ghosts_max_hp_loss(next.player_max_hp);
                next.player_max_hp = (next.player_max_hp - loss).max(1);
                next.player_hp = next.player_hp.min(next.player_max_hp);
                for _ in 0..ghosts_apparition_count(next.ascension) {
                    next.queue_pending_obtain_card(APPARITION_ID);
                }
                next.event = Some(EventScreen {
                    event: Event::Ghosts,
                    choices: ghosts_choices(1, next.player_max_hp),
                    stage: 1,
                    event_data: loss as u32,
                });
            }
            0 if choice_index == 1 => {
                next.event = Some(EventScreen {
                    event: Event::Ghosts,
                    choices: ghosts_choices(1, next.player_max_hp),
                    stage: 2,
                    event_data: 0,
                });
            }
            1 | 2 if choice_index == 0 => {
                next.flush_pending_obtain_cards();
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Ghosts",
                ));
            }
        },
        Event::KnowingSkull => match screen.stage {
            0 if choice_index == 0 => {
                let event_data = knowing_skull_event_data(knowing_skull_costs(0));
                next.event = Some(EventScreen {
                    event: Event::KnowingSkull,
                    choices: knowing_skull_choices(1, event_data),
                    stage: 1,
                    event_data,
                });
            }
            1 if choice_index == 0 => {
                let mut costs = knowing_skull_costs(screen.event_data);
                lose_event_hp(&mut next, costs.potion);
                costs.potion += 1;
                let event_data = knowing_skull_event_data(costs);
                knowing_skull_gain_random_potion(&mut next);
                next.event = Some(EventScreen {
                    event: Event::KnowingSkull,
                    choices: knowing_skull_choices(1, event_data),
                    stage: 1,
                    event_data,
                });
            }
            1 if choice_index == 1 => {
                let mut costs = knowing_skull_costs(screen.event_data);
                lose_event_hp(&mut next, costs.gold);
                costs.gold += 1;
                let event_data = knowing_skull_event_data(costs);
                next.gain_gold(KNOWING_SKULL_GOLD_REWARD);
                next.event = Some(EventScreen {
                    event: Event::KnowingSkull,
                    choices: knowing_skull_choices(1, event_data),
                    stage: 1,
                    event_data,
                });
            }
            1 if choice_index == 2 => {
                let mut costs = knowing_skull_costs(screen.event_data);
                lose_event_hp(&mut next, costs.card);
                costs.card += 1;
                let event_data = knowing_skull_event_data(costs);
                knowing_skull_gain_random_colorless(&mut next);
                next.event = Some(EventScreen {
                    event: Event::KnowingSkull,
                    choices: knowing_skull_choices(1, event_data),
                    stage: 1,
                    event_data,
                });
            }
            1 if choice_index == 3 => {
                let costs = knowing_skull_costs(screen.event_data);
                lose_event_hp(&mut next, costs.leave);
                next.event = Some(EventScreen {
                    event: Event::KnowingSkull,
                    choices: knowing_skull_choices(2, screen.event_data),
                    stage: 2,
                    event_data: screen.event_data,
                });
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Knowing Skull",
                ));
            }
        },
        Event::MaskedBandits => match screen.stage {
            0 if choice_index == 0 => {
                let stolen_gold = next.gold.max(0) as u32;
                next.gold = 0;
                next.event = Some(EventScreen {
                    event: Event::MaskedBandits,
                    choices: masked_bandits_choices(1),
                    stage: 1,
                    event_data: stolen_gold,
                });
            }
            0 if choice_index == 1 => {
                let mut misc_rng = next.rng_for_stream(RunRngStream::Misc);
                next.pending_event_combat_gold_offer = misc_rng.random_int_range(25, 35);
                next.store_rng_counter(RunRngStream::Misc, &misc_rng);
                next.pending_event_combat_relic_key_offer =
                    Some(if has_relic_key(&next, RelicKey::RedMask) {
                        RelicKey::Circlet
                    } else {
                        RelicKey::RedMask
                    });
                enter_event_combat(
                    &mut next,
                    &[&BANDIT_POINTY_A0, &BANDIT_LEADER_A0, &BANDIT_BEAR_A0],
                );
            }
            1 | 2 if choice_index == 0 => {
                let stage = screen.stage + 1;
                next.event = Some(EventScreen {
                    event: Event::MaskedBandits,
                    choices: masked_bandits_choices(stage as u8),
                    stage,
                    event_data: screen.event_data,
                });
            }
            3 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Masked Bandits",
                ));
            }
        },
        Event::Colosseum => match screen.stage {
            0 if choice_index == 0 => {
                next.event = Some(EventScreen {
                    event: Event::Colosseum,
                    choices: colosseum_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                enter_event_combat(
                    &mut next,
                    &[&SLAVER_BLUE_A0, &TASKMASTER_A0, &SLAVER_RED_A0],
                );
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            2 if choice_index == 1 => {
                enter_event_combat(&mut next, &[&GREMLIN_NOB_A0]);
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Colosseum",
                ));
            }
        },
        Event::DrugDealer => match screen.stage {
            0 if choice_index == 0 => {
                next.gain_deck_card(JAX_ID);
                next.event = Some(EventScreen {
                    event: Event::DrugDealer,
                    choices: drug_dealer_choices(1, true),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                if purgeable_event_card_count(&next) < usize::from(DRUG_DEALER_TRANSFORM_COUNT) {
                    return Err(SimError::IllegalAction("not enough transformable cards"));
                }
                open_event_transform_return_to_event_grid(
                    &mut next,
                    Event::DrugDealer,
                    DRUG_DEALER_TRANSFORM_COUNT,
                );
                next.event = Some(EventScreen {
                    event: Event::DrugDealer,
                    choices: drug_dealer_choices(1, true),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 2 => {
                if has_relic_key(&next, RelicKey::MutagenicStrength) {
                    next.gain_relic_key(RelicKey::Circlet);
                } else {
                    next.gain_relic_key(RelicKey::MutagenicStrength);
                }
                next.event = Some(EventScreen {
                    event: Event::DrugDealer,
                    choices: drug_dealer_choices(1, true),
                    stage: 1,
                    event_data: 0,
                });
            }
            1 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Drug Dealer",
                ));
            }
        },
        Event::WheelOfChange => match screen.stage {
            0 if choice_index == 0 => {
                let result = roll_wheel_of_change_result(&mut next);
                next.event = Some(EventScreen {
                    event: Event::WheelOfChange,
                    choices: wheel_of_change_choices(1, result),
                    stage: 1,
                    event_data: result,
                });
            }
            1 if choice_index == 0 => {
                if screen.event_data == 0 {
                    next.gain_gold(wheel_of_change_gold(next.current_act));
                }
                next.event = Some(EventScreen {
                    event: Event::WheelOfChange,
                    choices: wheel_of_change_choices(2, screen.event_data),
                    stage: 2,
                    event_data: screen.event_data,
                });
            }
            2 if choice_index == 0 => match screen.event_data {
                0 => {
                    next.event = Some(EventScreen {
                        event: Event::WheelOfChange,
                        choices: wheel_of_change_choices(3, screen.event_data),
                        stage: 3,
                        event_data: screen.event_data,
                    });
                }
                1 => {
                    let act = next.current_act;
                    let key = roll_event_relic_reward(&mut next, act);
                    let relic_offer = Relic::from_key(key);
                    next.phase = RunPhase::Reward;
                    next.event = None;
                    next.reward = Some(RewardScreen {
                        continuation: crate::RewardContinuation::None,
                        choices: Vec::new(),
                        queued_card_rewards: Vec::new(),
                        gold_offer: 0,
                        stolen_gold_offer: 0,
                        potion_offer: None,
                        potion_offers: Vec::new(),
                        relic_offer,
                        relic_key_offer: if relic_offer.is_some() {
                            None
                        } else {
                            Some(key)
                        },
                        pending_relic_offer: None,
                        pending_relic_key_offer: None,
                        queued_relic_key_offers: Vec::new(),
                        boss_relic_choices: Vec::new(),
                        card_reward_active: false,
                        card_reward_pending: false,
                        pending_card_reward_count: 0,
                    });
                }
                2 => {
                    next.heal_player(next.player_max_hp);
                    next.event = Some(EventScreen {
                        event: Event::WheelOfChange,
                        choices: wheel_of_change_choices(3, screen.event_data),
                        stage: 3,
                        event_data: screen.event_data,
                    });
                }
                3 => {
                    next.gain_deck_card(DECAY_ID);
                    next.event = Some(EventScreen {
                        event: Event::WheelOfChange,
                        choices: wheel_of_change_choices(3, screen.event_data),
                        stage: 3,
                        event_data: screen.event_data,
                    });
                }
                4 => {
                    open_event_remove_return_to_event_grid(&mut next, Event::WheelOfChange);
                }
                _ => {
                    let hp_loss = wheel_of_change_hp_loss(next.player_max_hp, next.ascension);
                    lose_event_hp(&mut next, hp_loss);
                    next.event = Some(EventScreen {
                        event: Event::WheelOfChange,
                        choices: wheel_of_change_choices(3, screen.event_data),
                        stage: 3,
                        event_data: screen.event_data,
                    });
                }
            },
            3 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Wheel of Change",
                ));
            }
        },
        Event::Lab if choice_index == 0 => {
            let mut potion_rng = next.rng_for_stream(RunRngStream::Potion);
            let potion_count = if next.ascension < 15 { 3 } else { 2 };
            let potion_offers = (0..potion_count)
                .map(|_| target_uniform_random_potion(&mut potion_rng))
                .collect();
            next.store_rng_counter(RunRngStream::Potion, &potion_rng);
            next.phase = RunPhase::Reward;
            next.event = None;
            next.reward = Some(RewardScreen {
                continuation: crate::RewardContinuation::None,
                choices: Vec::new(),
                queued_card_rewards: Vec::new(),
                gold_offer: 0,
                stolen_gold_offer: 0,
                potion_offer: None,
                potion_offers,
                relic_offer: None,
                relic_key_offer: None,
                pending_relic_offer: None,
                pending_relic_key_offer: None,
                queued_relic_key_offers: Vec::new(),
                boss_relic_choices: Vec::new(),
                card_reward_active: false,
                card_reward_pending: false,
                pending_card_reward_count: 0,
            });
        }
        Event::TombOfLordRedMask if screen.stage == 0 && choice_index == 0 => {
            if has_relic_key(&next, RelicKey::RedMask) {
                next.gain_gold(222);
            } else {
                next.gold = 0;
                next.gain_relic_key(RelicKey::RedMask);
            }
            next.event = Some(make_event_screen(
                Event::TombOfLordRedMask,
                tomb_of_lord_red_mask_choices(&next, 1),
                1,
            ));
        }
        Event::TombOfLordRedMask
            if (screen.stage == 0 && choice_index == 1)
                || (screen.stage == 1 && choice_index == 0) =>
        {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::MindBloom if screen.stage == 0 && choice_index == 0 => {
            let boss = roll_mind_bloom_boss(&mut next);
            next.pending_event_combat_gold_offer = if next.ascension >= 13 { 25 } else { 50 };
            next.pending_event_combat_relic_key_offer =
                Some(super::reward::roll_relic_reward(&mut next, RelicTier::Rare));
            let event_room_override = next.current_room_override;
            next.current_room_override = Some(crate::map::RoomKind::Boss);
            match boss {
                0 => enter_event_combat(&mut next, &[&GUARDIAN_A0]),
                1 => enter_event_combat(&mut next, &[&HEXAGHOST_A0]),
                _ => enter_event_combat(&mut next, &[&SLIME_BOSS_A0]),
            }
            next.current_room_override = event_room_override;
        }
        Event::MindBloom if screen.stage == 0 && choice_index == 1 => {
            for card in &mut next.deck {
                if let Some(upgraded) = upgrade_card_instance(*card) {
                    *card = upgraded;
                }
            }
            next.gain_relic_key(RelicKey::MarkOfBloom);
            next.event = Some(make_event_screen(
                Event::MindBloom,
                labeled_choices(&["Leave"]),
                1,
            ));
        }
        Event::MindBloom if screen.stage == 0 && choice_index == 2 => {
            if next.current_floor % 50 <= 40 {
                next.gain_gold(999);
                next.gain_deck_card(NORMALITY_ID);
                next.gain_deck_card(NORMALITY_ID);
            } else {
                next.heal_player(next.player_max_hp);
                next.gain_deck_card(DOUBT_ID);
            }
            next.event = Some(make_event_screen(
                Event::MindBloom,
                labeled_choices(&["Leave"]),
                1,
            ));
        }
        Event::MindBloom if screen.stage == 1 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::MatchAndKeep if screen.stage == 0 && choice_index == 0 => {
            next.event = Some(make_event_screen(
                Event::MatchAndKeep,
                match_and_keep_choices(1, 0),
                1,
            ));
        }
        Event::MatchAndKeep if screen.stage == 1 && choice_index == 0 => {
            if next.match_and_keep.is_none() {
                next.match_and_keep = Some(initialize_match_and_keep_state(&mut next));
            }
            next.event = Some(make_event_screen(
                Event::MatchAndKeep,
                match_and_keep_card_choices(&next),
                2,
            ));
        }
        Event::MatchAndKeep if screen.stage == 2 => {
            let card_index = match_and_keep_card_index_for_choice(&next, &screen, choice_index)?;
            apply_match_and_keep_card_choice(&mut next, card_index)?;
        }
        Event::MatchAndKeep if screen.stage == 3 && choice_index == 0 => {
            next.flush_pending_obtain_cards();
            next.phase = RunPhase::Idle;
            next.event = None;
            next.match_and_keep = None;
        }
        _ if choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
            next.match_and_keep = None;
        }
        _ => {
            return Err(SimError::IllegalAction(
                "event choice is not implemented for this event",
            ));
        }
    }

    Ok(next)
}

fn match_and_keep_card_index_for_choice(
    run: &RunState,
    screen: &EventScreen,
    choice_index: usize,
) -> SimResult<usize> {
    let card_count = run
        .match_and_keep
        .as_ref()
        .map(|state| state.cards.len())
        .unwrap_or(screen.choices.len());
    let label = screen
        .choices
        .get(choice_index)
        .ok_or(SimError::IllegalAction(
            "Match and Keep card choice is out of range",
        ))?
        .label
        .as_str();
    label
        .strip_prefix("card")
        .and_then(|index| index.parse::<usize>().ok())
        .map(|label_index| match_and_keep_group_index_for_label(label_index, card_count))
        .or_else(|| match_and_keep_group_index_for_visible_choice(run, choice_index))
        .ok_or(SimError::InvalidState(
            "Match and Keep card label is invalid",
        ))
}

fn match_and_keep_group_index_for_visible_choice(
    run: &RunState,
    choice_index: usize,
) -> Option<usize> {
    let state = run.match_and_keep.as_ref()?;
    let card_count = state.cards.len();
    (0..card_count)
        .filter_map(|label_index| {
            let group_index = match_and_keep_group_index_for_label(label_index, card_count);
            let card = state.cards.get(group_index)?;
            let currently_flipped = state.first_flipped_index == Some(group_index)
                || state.second_flipped_index == Some(group_index);
            (!card.matched && !currently_flipped).then_some(group_index)
        })
        .nth(choice_index)
}

fn apply_match_and_keep_card_choice(run: &mut RunState, choice_index: usize) -> SimResult<()> {
    // A matched card's obtain effect settles on the following update.
    run.flush_pending_obtain_cards();

    {
        let state = run
            .match_and_keep
            .as_mut()
            .ok_or(SimError::InvalidState("Match and Keep state is missing"))?;
        if choice_index >= state.cards.len() {
            return Err(SimError::IllegalAction(
                "Match and Keep card choice is out of range",
            ));
        }
        if state.cards[choice_index].matched {
            return Err(SimError::IllegalAction(
                "Match and Keep card choice is not available",
            ));
        }
        state.cards[choice_index].revealed = true;
        if let Some(first_index) = state.first_flipped_index.take() {
            if first_index == choice_index {
                return Err(SimError::IllegalAction(
                    "Match and Keep cannot choose the same card twice",
                ));
            }
            state.first_flipped_index = Some(first_index);
            state.second_flipped_index = Some(choice_index);
        } else {
            state.first_flipped_index = Some(choice_index);
        }
    };

    if run
        .match_and_keep
        .as_ref()
        .is_some_and(|state| state.second_flipped_index.is_some())
    {
        resolve_match_and_keep_pending_pair(run)?;
    }

    let attempts_remaining = run
        .match_and_keep
        .as_ref()
        .ok_or(SimError::InvalidState("Match and Keep state is missing"))?
        .attempts_remaining;
    if attempts_remaining == 0 {
        run.flush_pending_obtain_cards();
        run.event = Some(make_event_screen(
            Event::MatchAndKeep,
            labeled_choices(&["Leave"]),
            3,
        ));
    } else {
        run.event = Some(make_event_screen(
            Event::MatchAndKeep,
            match_and_keep_card_choices(run),
            2,
        ));
    }
    Ok(())
}

fn resolve_match_and_keep_pending_pair(run: &mut RunState) -> SimResult<bool> {
    let mut matched_content = None;
    let Some(state) = run.match_and_keep.as_mut() else {
        return Ok(false);
    };
    let Some(first_index) = state.first_flipped_index.take() else {
        return Ok(false);
    };
    let Some(second_index) = state.second_flipped_index.take() else {
        state.first_flipped_index = Some(first_index);
        return Ok(false);
    };
    if first_index >= state.cards.len() || second_index >= state.cards.len() {
        return Err(SimError::InvalidState(
            "Match and Keep pending pair is out of range",
        ));
    }

    let first_content = state.cards[first_index].content_id;
    let second_content = state.cards[second_index].content_id;
    if first_content == second_content {
        state.matched_cards.push(first_content);
        matched_content = Some(first_content);
        state.cards[first_index].matched = true;
        state.cards[second_index].matched = true;
    } else {
        state.cards[first_index].revealed = true;
        state.cards[second_index].revealed = true;
    }
    state.attempts_remaining = state.attempts_remaining.saturating_sub(1);

    if let Some(content_id) = matched_content {
        run.queue_pending_obtain_card(content_id);
    }
    Ok(true)
}

fn enter_event_combat(run: &mut RunState, definitions: &[&MonsterDefinition]) {
    let mut shuffle_rng = StsRng::new(run.event_rng_seed as i64 + i64::from(run.current_floor));
    let mut monster_hp_rng = StsRng::new(run.event_rng_seed as i64 + i64::from(run.current_floor));
    let mut monster_rng = StsRng::new(run.monster_rng_seed as i64 + i64::from(run.current_floor));
    let mut card_random_rng = Some(run.card_random_rng());
    let mut combat = CombatState::initial_fixture();
    combat.ascension = run.ascension;
    combat.monsters = definitions
        .iter()
        .enumerate()
        .map(|(index, definition)| {
            let mut monster = monster_state_for_ascension(
                definition,
                MonsterId::new(index as u64 + 1),
                run.ascension,
            );
            if let Some(range) =
                target_monster_hp_range_for_content_id(definition.content_id, run.ascension)
            {
                let max_hp = range.roll(&mut monster_hp_rng);
                monster.hp = max_hp;
                monster.max_hp = max_hp;
            }
            monster
        })
        .collect();
    apply_initial_monster_ai_rolls(&mut combat, &mut monster_rng);
    for monster in &mut combat.monsters {
        record_target_move(monster);
    }
    combat.piles = initialize_combat_piles_with_relics(
        &run.deck,
        &mut shuffle_rng,
        &mut card_random_rng,
        &run.relics,
    );
    combat.shuffle_rng = Some(shuffle_rng);
    combat.monster_hp_rng = Some(monster_hp_rng);
    combat.monster_rng = Some(monster_rng);
    combat.card_random_rng = card_random_rng;
    run.phase = RunPhase::Combat;
    run.event = None;
    run.combat = Some(run.init_combat_consuming_relics(combat));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        map::RoomKind,
        potion::Potion,
        relic::CERAMIC_FISH_GOLD,
        run::{apply_run_action, RunAction},
        MonsterIntent,
    };

    fn bonfire_run(card: ContentId, player_hp: i32, player_max_hp: i32) -> RunState {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.deck = vec![CardInstance::new(CardId::new(1), card)];
        run.player_hp = player_hp;
        run.player_max_hp = player_max_hp;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::BonfireElementals));
        run
    }

    #[test]
    fn event_screen_selection_does_not_advance_persistent_event_rng() {
        let mut run = RunState::placeholder_seeded_ironclad(7_141_693_325_691_831_207, 0);
        run.current_act = 2;
        run.current_floor = 25;
        run.event_rng_counter = 9;

        enter_event_screen(&mut run);

        assert_eq!(run.event_rng_counter, 9);
    }

    #[test]
    fn colosseum_requires_a_map_row_past_the_act_midpoint() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.current_act = 2;
        run.current_floor = 25;

        assert!(!event_is_available(&run, Event::Colosseum));

        run.current_floor = 26;
        assert!(event_is_available(&run, Event::Colosseum));
    }

    #[test]
    fn wheel_of_change_preserves_real_spin_and_prize_reveal_stages() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.player_hp = 70;
        run.player_max_hp = 80;
        run.event = Some(EventScreen {
            event: Event::WheelOfChange,
            choices: wheel_of_change_choices(1, 5),
            stage: 1,
            event_data: 5,
        });

        let after_spin = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("spin reveals prize button");
        assert_eq!(after_spin.player_hp, 70);
        assert_eq!(after_spin.event.as_ref().unwrap().stage, 2);
        assert_eq!(
            after_spin.event.as_ref().unwrap().choices[0].label,
            "prize!"
        );

        let after_prize = apply_event_action(&after_spin, EventAction::Choose { choice_index: 0 })
            .expect("prize button applies the hidden result");
        assert!(after_prize.player_hp < 70);
        assert_eq!(after_prize.event.as_ref().unwrap().stage, 3);
        assert_eq!(
            after_prize.event.as_ref().unwrap().choices[0].label,
            "Leave"
        );
    }

    #[test]
    fn wheel_of_change_applies_gold_before_prize_reveal() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.gold = 115;
        run.event = Some(EventScreen {
            event: Event::WheelOfChange,
            choices: wheel_of_change_choices(1, 0),
            stage: 1,
            event_data: 0,
        });

        let after_spin = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("spin completion applies gold and reveals prize");
        assert_eq!(after_spin.gold, 215);
        assert_eq!(after_spin.event.as_ref().unwrap().stage, 2);
        assert_eq!(
            after_spin.event.as_ref().unwrap().choices[0].label,
            "prize!"
        );

        let after_prize = apply_event_action(&after_spin, EventAction::Choose { choice_index: 0 })
            .expect("prize reveal advances without applying gold twice");
        assert_eq!(after_prize.gold, 215);
        assert_eq!(after_prize.event.as_ref().unwrap().stage, 3);
    }

    #[test]
    fn drug_dealer_choice_labels_match_the_live_augmenter_screen() {
        let choices = drug_dealer_choices(0, true);
        assert_eq!(
            choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            ["Test J.A.X.", "Become test subject", "Ingest mutagens"]
        );
    }

    #[test]
    fn cursed_tome_final_choice_labels_match_communication_mod() {
        assert_eq!(
            cursed_tome_choices(4, 0)
                .iter()
                .map(|choice| choice.label.split(" (").next().unwrap_or_default())
                .collect::<Vec<_>>(),
            ["Take", "Stop"]
        );
    }

    #[test]
    fn cursed_tome_book_reward_proceeds_back_to_leave() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::CursedTome,
            choices: cursed_tome_choices(4, run.ascension),
            stage: 4,
            event_data: 6,
        });

        let reward = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("taking the book opens its relic reward");
        assert_eq!(reward.phase, RunPhase::Reward);
        assert_eq!(reward.event.as_ref().expect("event continuation").stage, 5);

        let claimed = crate::apply_run_action(&reward, crate::RunAction::TakeRelicReward)
            .expect("book relic can be claimed");
        let leave = crate::apply_run_action(&claimed, crate::RunAction::Proceed)
            .expect("reward proceeds back to Cursed Tome");

        assert_eq!(leave.phase, RunPhase::Event);
        assert_eq!(
            leave
                .event
                .as_ref()
                .expect("Cursed Tome leave screen")
                .choices[0]
                .label,
            "Leave"
        );
    }

    #[test]
    fn drug_dealer_transform_returns_to_leave_after_two_cards() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::DrugDealer,
            choices: drug_dealer_choices(0, true),
            stage: 0,
            event_data: 0,
        });
        let original_deck_len = run.deck.len();

        let after_choice = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect("Become test subject opens the transform grid");
        assert!(matches!(
            after_choice
                .card_grid
                .as_ref()
                .expect("transform grid")
                .purpose,
            crate::GridPurpose::EventTransformReturnToEvent {
                event: Event::DrugDealer,
                count: DRUG_DEALER_TRANSFORM_COUNT
            }
        ));

        let after_first = crate::run::grid::select_grid_card(&after_choice, 0)
            .expect("first transform source can be selected");
        let after_second = crate::run::grid::select_grid_card(&after_first, 1)
            .expect("second transform source can be selected");
        let completed = crate::run::grid::confirm_grid(&after_second).expect("transform confirms");

        assert!(completed.card_grid.is_none());
        assert_eq!(completed.deck.len(), original_deck_len);
        assert_eq!(completed.phase, RunPhase::Event);
        assert_eq!(
            completed
                .event
                .as_ref()
                .expect("Drug Dealer leave screen")
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            ["Leave"]
        );
    }

    #[test]
    fn dead_adventurer_legacy_continue_returns_to_next_search_after_a_safe_attempt() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        let event_data = dead_adventurer_event_data([0, 1, 2], 0, 1);
        run.event = Some(dead_adventurer_screen(&run, 2, event_data));
        assert_eq!(run.event.as_ref().unwrap().choices[0].label, "Continue");
        assert_eq!(run.event.as_ref().unwrap().choices[1].label, "Leave");

        let next = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("continue should return to the next search prompt");

        assert_eq!(next.event.as_ref().unwrap().stage, 0);
        assert!(next.event.as_ref().unwrap().choices[0]
            .label
            .starts_with("Continue"));
    }

    #[test]
    fn dead_adventurer_continue_reveals_fight_before_entering_combat() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        let event_data =
            dead_adventurer_event_data([0, 1, 2], 1, 2) | DEAD_ADVENTURER_PENDING_ENCOUNTER;
        run.event = Some(dead_adventurer_screen(&run, 2, event_data));

        let reveal = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("continue should reveal the pending encounter");
        assert_eq!(reveal.phase, RunPhase::Event);
        assert_eq!(reveal.event.as_ref().unwrap().stage, 3);
        assert_eq!(reveal.event.as_ref().unwrap().choices[0].label, "Fight");

        let mut expected = reveal.clone();
        let misc_counter_before_fight = reveal.misc_rng_counter;
        let treasure_counter_before_fight = reveal.treasure_rng_counter;
        let mut relic_rng = expected.rng_for_stream(RunRngStream::Relic);
        let relic_tier = target_elite_relic_tier(&mut relic_rng);
        expected.store_rng_counter(RunRngStream::Relic, &relic_rng);
        let expected_relic = roll_relic_reward(&mut expected, relic_tier);

        let fight = apply_event_action(&reveal, EventAction::Choose { choice_index: 0 })
            .expect("fight should enter combat");
        assert_eq!(fight.phase, RunPhase::Combat);
        assert!((55..=65).contains(&fight.pending_event_combat_gold_offer));
        assert_eq!(fight.misc_rng_counter, misc_counter_before_fight + 1);
        assert_eq!(fight.treasure_rng_counter, treasure_counter_before_fight);
        assert_eq!(
            fight.pending_event_combat_relic_key_offer,
            Some(expected_relic)
        );
    }

    #[test]
    fn dead_adventurer_triple_sentries_open_beam_attack_beam() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);

        enter_event_combat(&mut run, &[&SENTRY_A0, &SENTRY_A0, &SENTRY_A0]);

        let monsters = &run.combat.as_ref().unwrap().monsters;
        assert!(matches!(
            monsters[0].intent,
            MonsterIntent::AddDazedToDiscard { count: 2 }
        ));
        assert!(matches!(monsters[1].intent, MonsterIntent::Attack { .. }));
        assert!(matches!(
            monsters[2].intent,
            MonsterIntent::AddDazedToDiscard { count: 2 }
        ));
    }

    #[test]
    fn bonfire_elementals_removes_rare_card_and_applies_reward() {
        let run = bonfire_run(RITUAL_DAGGER_ID, 40, 70);
        let after_intro = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Bonfire intro applies");
        assert_eq!(after_intro.event.as_ref().expect("choice screen").stage, 1);

        let after_offer = apply_event_action(&after_intro, EventAction::Choose { choice_index: 0 })
            .expect("Bonfire offer opens card grid");
        assert!(matches!(
            after_offer
                .card_grid
                .as_ref()
                .expect("Bonfire card grid")
                .purpose,
            crate::GridPurpose::BonfireElementals
        ));

        let selected = crate::run::grid::select_grid_card(&after_offer, 0)
            .expect("Bonfire card can be selected");
        let after_confirm =
            crate::run::grid::confirm_grid(&selected).expect("Bonfire card selection resolves");

        assert!(after_confirm.deck.is_empty());
        assert_eq!(after_confirm.player_max_hp, 80);
        assert_eq!(after_confirm.player_hp, 80);
        assert_eq!(after_confirm.phase, RunPhase::Event);
        assert_eq!(after_confirm.event.as_ref().expect("leave screen").stage, 2);

        let after_leave =
            apply_event_action(&after_confirm, EventAction::Choose { choice_index: 0 })
                .expect("Bonfire leave applies");
        assert_eq!(after_leave.phase, RunPhase::Idle);
        assert!(after_leave.event.is_none());
    }

    #[test]
    fn bonfire_elementals_offering_curse_awards_spirit_poop() {
        let run = bonfire_run(REGRET_ID, 50, 80);
        let after_intro = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Bonfire intro applies");
        let after_offer = apply_event_action(&after_intro, EventAction::Choose { choice_index: 0 })
            .expect("Bonfire offer opens card grid");
        let selected =
            crate::run::grid::select_grid_card(&after_offer, 0).expect("curse can be selected");
        let after_confirm =
            crate::run::grid::confirm_grid(&selected).expect("curse offering resolves");

        assert!(after_confirm.deck.is_empty());
        assert!(after_confirm.relic_keys.contains(&RelicKey::SpiritPoop));
    }

    #[test]
    fn designer_adjustments_can_upgrade_one_selected_card() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.current_act = 2;
        run.gold = 100;
        run.deck = vec![CardInstance::new(CardId::new(1), BASH_ID)];
        run.phase = RunPhase::Event;
        run.event = Some(designer_screen(&run, 0, 1));

        let main = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Designer intro applies");
        let selected = apply_event_action(&main, EventAction::Choose { choice_index: 0 })
            .expect("Designer adjustments open upgrade grid");
        assert!(matches!(
            selected
                .card_grid
                .as_ref()
                .expect("Designer upgrade grid")
                .purpose,
            crate::GridPurpose::EventUpgradeReturnToEvent {
                event: Event::Designer
            }
        ));

        let selected_card = crate::run::grid::select_grid_card(&selected, 0)
            .expect("Designer card can be selected");
        let completed =
            crate::run::grid::confirm_grid(&selected_card).expect("Designer upgrade confirms");

        assert_eq!(completed.gold, 60);
        assert_eq!(completed.deck[0].content_id, BASH_PLUS_ID);
        assert_eq!(
            completed
                .event
                .as_ref()
                .expect("Designer leave screen")
                .stage,
            2
        );
    }

    #[test]
    fn duplicator_adds_an_unbottled_copy_and_returns_to_leave() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.current_act = 2;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Duplicator));
        let original_len = run.deck.len();
        let original_content = run.deck[0].content_id;

        let after_duplicate = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Duplicator opens card grid");
        assert!(matches!(
            after_duplicate
                .card_grid
                .as_ref()
                .expect("Duplicator card grid")
                .purpose,
            crate::GridPurpose::EventObtainCardReturnToEvent {
                event: Event::Duplicator
            }
        ));

        let completed = crate::run::grid::select_grid_card(&after_duplicate, 0)
            .expect("Duplicator selected card resolves");
        assert_eq!(completed.deck.len(), original_len + 1);
        assert_eq!(
            completed.deck.last().expect("copied card").content_id,
            original_content
        );
        assert!(!completed.deck.last().expect("copied card").bottled);
        assert_eq!(
            completed
                .event
                .as_ref()
                .expect("Duplicator leave screen")
                .stage,
            2
        );
    }

    #[test]
    fn match_and_keep_continue_opens_play_choice() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::MatchAndKeep));

        let after_continue = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("continue choice applies");
        let choices = after_continue
            .event
            .as_ref()
            .expect("Match and Keep remains open")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(after_continue.phase, RunPhase::Event);
        assert_eq!(choices, vec!["Play"]);
    }

    #[test]
    fn match_and_keep_play_opens_twelve_card_choices() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(make_event_screen(
            Event::MatchAndKeep,
            match_and_keep_choices(1, 0),
            1,
        ));

        let after_play = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("play choice applies");
        let choices = after_play
            .event
            .as_ref()
            .expect("Match and Keep card grid remains open")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(after_play.phase, RunPhase::Event);
        assert_eq!(
            choices,
            vec![
                "card0", "card1", "card2", "card3", "card4", "card5", "card6", "card7", "card8",
                "card9", "card10", "card11"
            ]
        );
    }

    #[test]
    fn match_and_keep_uses_ironclad_event_starter_card() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        let state = initialize_match_and_keep_state(&mut run);
        let bash_count = state
            .cards
            .iter()
            .filter(|card| card.content_id == BASH_ID)
            .count();
        let strike_count = state
            .cards
            .iter()
            .filter(|card| card.content_id == STRIKE_R_ID)
            .count();

        assert_eq!(bash_count, 2);
        assert_eq!(strike_count, 0);
    }

    #[test]
    fn match_and_keep_card_labels_use_communication_mod_grid_order() {
        let expected = [0, 9, 6, 3, 4, 1, 10, 7, 8, 5, 2, 11];
        for (label, group) in expected.into_iter().enumerate() {
            assert_eq!(match_and_keep_group_index_for_label(label, 12), group);
            assert_eq!(match_and_keep_label_index_for_group(group, 12), label);
        }
    }

    #[test]
    fn match_and_keep_matched_cards_keep_stable_labels_but_leave_live_choices() {
        use crate::content::cards::LIMIT_BREAK_ID;

        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(make_event_screen(
            Event::MatchAndKeep,
            match_and_keep_choices(2, 12),
            2,
        ));
        run.match_and_keep = Some(MatchAndKeepState {
            cards: (0..12)
                .map(|index| MatchAndKeepCard {
                    content_id: if index == 6 || index == 9 {
                        LIMIT_BREAK_ID
                    } else if index == 1 {
                        DEFEND_R_ID
                    } else {
                        STRIKE_R_ID
                    },
                    revealed: false,
                    matched: false,
                })
                .collect(),
            attempts_remaining: 5,
            first_flipped_index: None,
            second_flipped_index: None,
            matched_cards: Vec::new(),
        });

        let after_first = apply_event_action(&run, EventAction::Choose { choice_index: 2 })
            .expect("card2 maps to group slot 6");
        let after_second =
            apply_event_action(&after_first, EventAction::Choose { choice_index: 1 })
                .expect("card1 maps to group slot 9 after card2 is omitted");

        let state = after_second
            .match_and_keep
            .as_ref()
            .expect("Match and Keep remains open");
        assert_eq!(state.cards.len(), 12);
        assert_eq!(state.first_flipped_index, None);
        assert_eq!(state.second_flipped_index, None);
        assert!(state.cards[6].matched);
        assert!(state.cards[9].matched);
        assert_eq!(state.matched_cards, vec![LIMIT_BREAK_ID]);
        assert_eq!(
            after_second
                .event
                .as_ref()
                .expect("resolved match screen")
                .choices
                .iter()
                .map(|choice| choice.label.clone())
                .collect::<Vec<_>>(),
            vec![
                "card0", "card3", "card4", "card5", "card6", "card7", "card8", "card9", "card10",
                "card11"
            ]
        );

        let after_next = apply_event_action(&after_second, EventAction::Choose { choice_index: 0 })
            .expect("the next flip flushes the matched-card obtain effect");
        let state = after_next.match_and_keep.as_ref().unwrap();
        assert_eq!(state.cards.len(), 12);
        assert!(state.cards[6].matched);
        assert!(state.cards[9].matched);
        assert_eq!(state.matched_cards, vec![LIMIT_BREAK_ID]);
        let labels = after_next
            .event
            .as_ref()
            .unwrap()
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();
        assert!(!labels.contains(&"card2"));
        assert!(!labels.contains(&"card1"));
    }

    #[test]
    fn match_and_keep_second_visible_index_preserves_both_pending_labels() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(make_event_screen(
            Event::MatchAndKeep,
            match_and_keep_choices(2, 12),
            2,
        ));
        run.match_and_keep = Some(MatchAndKeepState {
            cards: (0..12)
                .map(|_| MatchAndKeepCard {
                    content_id: STRIKE_R_ID,
                    revealed: false,
                    matched: false,
                })
                .collect(),
            attempts_remaining: 2,
            first_flipped_index: None,
            second_flipped_index: None,
            matched_cards: Vec::new(),
        });

        let after_first = apply_event_action(&run, EventAction::Choose { choice_index: 3 })
            .expect("card3 first flip");
        let after_second =
            apply_event_action(&after_first, EventAction::Choose { choice_index: 6 })
                .expect("visible index 6 selects card7");
        assert_eq!(
            after_second
                .event
                .as_ref()
                .unwrap()
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec![
                "card0", "card1", "card2", "card4", "card5", "card6", "card8", "card9", "card10",
                "card11"
            ]
        );
    }

    #[test]
    fn match_and_keep_third_click_maps_after_resolving_prior_pair() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(make_event_screen(
            Event::MatchAndKeep,
            match_and_keep_choices(2, 12),
            2,
        ));
        run.match_and_keep = Some(MatchAndKeepState {
            cards: (0..12)
                .map(|index| MatchAndKeepCard {
                    content_id: if index == 3 { DEFEND_R_ID } else { STRIKE_R_ID },
                    revealed: false,
                    matched: false,
                })
                .collect(),
            attempts_remaining: 5,
            first_flipped_index: None,
            second_flipped_index: None,
            matched_cards: Vec::new(),
        });
        let first = apply_event_action(&run, EventAction::Choose { choice_index: 11 }).unwrap();
        let second = apply_event_action(&first, EventAction::Choose { choice_index: 3 }).unwrap();
        let third = apply_event_action(&second, EventAction::Choose { choice_index: 6 }).unwrap();
        let choices = third
            .event
            .as_ref()
            .unwrap()
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();
        assert!(!choices.contains(&"card6"));
        assert!(choices.contains(&"card7"));
    }

    #[test]
    fn match_and_keep_matching_pair_obtains_card() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(make_event_screen(
            Event::MatchAndKeep,
            match_and_keep_choices(2, 3),
            2,
        ));
        run.match_and_keep = Some(MatchAndKeepState {
            cards: vec![
                MatchAndKeepCard {
                    content_id: STRIKE_R_ID,
                    revealed: false,
                    matched: false,
                },
                MatchAndKeepCard {
                    content_id: DEFEND_R_ID,
                    revealed: false,
                    matched: false,
                },
                MatchAndKeepCard {
                    content_id: STRIKE_R_ID,
                    revealed: false,
                    matched: false,
                },
            ],
            attempts_remaining: 5,
            first_flipped_index: None,
            second_flipped_index: None,
            matched_cards: Vec::new(),
        });

        let after_first = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("first flip applies");
        assert_eq!(
            after_first
                .event
                .as_ref()
                .expect("Match and Keep remains open after first flip")
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["card1", "card2"]
        );
        let after_second =
            apply_event_action(&after_first, EventAction::Choose { choice_index: 1 })
                .expect("second flip applies");

        let state = after_second
            .match_and_keep
            .as_ref()
            .expect("state remains while attempts remain");
        assert_eq!(state.attempts_remaining, 4);
        assert_eq!(state.matched_cards, vec![STRIKE_R_ID]);
        assert_eq!(state.cards.len(), 3);
        assert_eq!(state.first_flipped_index, None);
        assert_eq!(state.second_flipped_index, None);
        assert!(state.cards[0].matched);
        assert!(state.cards[2].matched);
        assert_eq!(after_second.pending_obtain_cards, vec![STRIKE_R_ID]);
        assert_eq!(
            after_second
                .event
                .as_ref()
                .expect("Match and Keep remains open")
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["card1"]
        );
        assert_ne!(
            after_second.deck.last().map(|card| card.content_id),
            Some(STRIKE_R_ID)
        );

        let after_next_flip =
            apply_event_action(&after_second, EventAction::Choose { choice_index: 0 })
                .expect("next flip resolves the pair and flushes the obtain effect");
        assert!(after_next_flip.pending_obtain_cards.is_empty());
        let state = after_next_flip.match_and_keep.as_ref().unwrap();
        assert_eq!(state.attempts_remaining, 4);
        assert_eq!(state.matched_cards, vec![STRIKE_R_ID]);
        assert_eq!(state.cards.len(), 3);
        assert!(state.cards[0].matched);
        assert_eq!(state.cards[1].content_id, DEFEND_R_ID);
        assert_eq!(state.first_flipped_index, Some(1));
        assert!(state.cards[2].matched);
        assert_eq!(
            after_next_flip.deck.last().map(|card| card.content_id),
            Some(STRIKE_R_ID)
        );
    }

    #[test]
    fn match_and_keep_non_matching_pair_spends_attempt_without_obtain() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(make_event_screen(
            Event::MatchAndKeep,
            match_and_keep_choices(2, 2),
            2,
        ));
        let deck_len = run.deck.len();
        run.match_and_keep = Some(MatchAndKeepState {
            cards: vec![
                MatchAndKeepCard {
                    content_id: STRIKE_R_ID,
                    revealed: false,
                    matched: false,
                },
                MatchAndKeepCard {
                    content_id: DEFEND_R_ID,
                    revealed: false,
                    matched: false,
                },
            ],
            attempts_remaining: 1,
            first_flipped_index: None,
            second_flipped_index: None,
            matched_cards: Vec::new(),
        });

        let after_first = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("first flip applies");
        let after_second =
            apply_event_action(&after_first, EventAction::Choose { choice_index: 0 })
                .expect("second flip applies");

        assert_eq!(after_second.deck.len(), deck_len);
        let state = after_second
            .match_and_keep
            .as_ref()
            .expect("final mismatch keeps event state until Leave");
        assert_eq!(state.attempts_remaining, 0);
        assert_eq!(state.first_flipped_index, None);
        assert_eq!(state.second_flipped_index, None);
        assert_eq!(
            after_second
                .event
                .as_ref()
                .expect("complete screen")
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Leave"]
        );

        let after_leave =
            apply_event_action(&after_second, EventAction::Choose { choice_index: 0 })
                .expect("leave closes the completed game");
        assert_eq!(after_leave.deck.len(), deck_len);
        assert_eq!(after_leave.phase, RunPhase::Idle);
        assert!(after_leave.event.is_none());
        assert!(after_leave.match_and_keep.is_none());
    }

    #[test]
    fn upgrade_shrine_pray_returns_to_leave_after_upgrade() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::UpgradeShrine));

        let choices = run
            .event
            .as_ref()
            .expect("Upgrade Shrine is open")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(choices, vec!["Pray", "Leave"]);

        let after_pray = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("pray choice applies");
        assert!(matches!(
            after_pray
                .card_grid
                .as_ref()
                .expect("Pray opens the upgrade grid")
                .purpose,
            crate::GridPurpose::EventUpgradeReturnToEvent {
                event: Event::UpgradeShrine
            }
        ));

        let bash_index = after_pray
            .card_grid
            .as_ref()
            .expect("Pray opens the upgrade grid")
            .cards
            .iter()
            .position(|card| card.content_id == crate::content::cards::BASH_ID)
            .expect("starter Bash is upgradeable");
        let after_select = crate::run::grid::select_grid_card(&after_pray, bash_index)
            .expect("Bash can be selected");
        let after_confirm =
            crate::run::grid::confirm_grid(&after_select).expect("upgrade confirms");
        let leave_choices = after_confirm
            .event
            .as_ref()
            .expect("Upgrade Shrine returns to event")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(after_confirm.phase, RunPhase::Event);
        assert_eq!(leave_choices, vec!["Leave"]);
    }

    #[test]
    fn purifier_pray_removes_selected_card_and_returns_to_leave() {
        use crate::content::cards::BASH_ID;

        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Purifier));

        let choices = run
            .event
            .as_ref()
            .expect("Purifier is open")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(choices, vec!["Pray", "Leave"]);

        let after_pray = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("pray choice applies");
        let bash_index = after_pray
            .card_grid
            .as_ref()
            .expect("Pray opens the removal grid")
            .cards
            .iter()
            .position(|card| card.content_id == BASH_ID)
            .expect("starter Bash is upgradeable");
        let after_select = crate::run::grid::select_grid_card(&after_pray, bash_index)
            .expect("Bash can be selected");
        let after_confirm =
            crate::run::grid::confirm_grid(&after_select).expect("removal confirms");
        let leave_choices = after_confirm
            .event
            .as_ref()
            .expect("Purifier returns to event")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(after_confirm.phase, RunPhase::Event);
        assert_eq!(leave_choices, vec!["Leave"]);
        assert!(!after_confirm
            .deck
            .iter()
            .any(|card| card.content_id == BASH_ID));
    }

    #[test]
    fn the_cleric_leave_requires_second_leave_click() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::TheCleric));

        let after_first_leave = apply_event_action(&run, EventAction::Choose { choice_index: 2 })
            .expect("first leave choice applies");
        let leave_choices = after_first_leave
            .event
            .as_ref()
            .expect("The Cleric remains open after first leave")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(after_first_leave.phase, RunPhase::Event);
        assert_eq!(leave_choices, vec!["Leave"]);

        let after_second_leave =
            apply_event_action(&after_first_leave, EventAction::Choose { choice_index: 0 })
                .expect("second leave choice closes the event");

        assert_eq!(after_second_leave.phase, RunPhase::Idle);
        assert!(after_second_leave.event.is_none());
    }

    #[test]
    fn the_cleric_uses_a15_purify_cost_and_does_not_charge_empty_deck() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 15);
        run.phase = RunPhase::Event;
        run.gold = 75;
        run.event = Some(event_screen(Event::TheCleric));

        let after_purify = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect("A15 Purify applies");
        assert_eq!(after_purify.gold, 0);
        assert!(after_purify.card_grid.is_some());

        let mut empty = RunState::placeholder_seeded_ironclad(1, 15);
        empty.phase = RunPhase::Event;
        empty.gold = 75;
        empty.deck.clear();
        empty.event = Some(event_screen(Event::TheCleric));
        let after_empty = apply_event_action(&empty, EventAction::Choose { choice_index: 1 })
            .expect("empty Purify proceeds to leave");
        assert_eq!(after_empty.gold, 75);
        assert_eq!(after_empty.event.as_ref().expect("leave").stage, 1);
    }

    #[test]
    fn secret_portal_requires_beyond_and_eight_hundred_seconds() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.current_act = 3;
        run.playtime_seconds = 799;
        assert!(!special_one_time_event_is_available(
            &run,
            Event::SecretPortal
        ));
        run.playtime_seconds = 800;
        assert!(special_one_time_event_is_available(
            &run,
            Event::SecretPortal
        ));

        let json = serde_json::to_string(&run).expect("Secret Portal state serializes");
        let restored: RunState = serde_json::from_str(&json).expect("Secret Portal state loads");
        assert_eq!(restored.playtime_seconds, 800);
    }

    #[test]
    fn secret_portal_enters_act_three_boss_combat_after_acceptance() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.current_act = 3;
        run.playtime_seconds = 800;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::SecretPortal));

        let after_accept = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Secret Portal accept applies");
        let after_continue =
            apply_event_action(&after_accept, EventAction::Choose { choice_index: 0 })
                .expect("Secret Portal transition applies");
        assert_eq!(after_continue.phase, RunPhase::Combat);
        assert_eq!(
            after_continue.current_room_kind(),
            Some(crate::map::RoomKind::Boss)
        );
        assert!(after_continue.combat.is_some());
    }

    #[test]
    fn transmogrifier_pray_returns_to_leave_after_transform() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Transmorgrifier));

        let choices = run
            .event
            .as_ref()
            .expect("Transmogrifier is open")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(choices, vec!["Pray", "Leave"]);

        let after_pray = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("pray choice applies");
        assert!(matches!(
            after_pray
                .card_grid
                .as_ref()
                .expect("Pray opens the transform grid")
                .purpose,
            crate::GridPurpose::EventTransformReturnToEvent {
                event: Event::Transmorgrifier,
                count: 1
            }
        ));

        let after_select = crate::run::grid::select_grid_card(&after_pray, 0)
            .expect("a starter card can be selected");
        let after_confirm =
            crate::run::grid::confirm_grid(&after_select).expect("transform confirms");
        let leave_choices = after_confirm
            .event
            .as_ref()
            .expect("Transmogrifier returns to event")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(after_confirm.phase, RunPhase::Event);
        assert_eq!(leave_choices, vec!["Leave"]);
    }

    #[test]
    fn we_meet_again_omits_locked_card_option_for_basic_only_deck() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.gold = 132;
        run.gain_potion(Potion::Swift).expect("potion slot is open");
        run.gain_potion(Potion::Elixir)
            .expect("potion slot is open");
        run.phase = RunPhase::Event;
        let screen = entered_event_screen_for_run(&mut run, Event::WeMeetAgain);
        run.event = Some(screen);

        let choices = run
            .event
            .as_ref()
            .expect("We Meet Again is open")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(choices, vec!["Give Potion", "Give Gold", "Attack"]);

        let after_attack = apply_event_action(&run, EventAction::Choose { choice_index: 2 })
            .expect("shifted Attack choice applies");
        let leave_choices = after_attack
            .event
            .as_ref()
            .expect("We Meet Again returns to event")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(after_attack.gold, 132);
        assert_eq!(after_attack.potions, run.potions);
        assert_eq!(leave_choices, vec!["Leave"]);
    }

    #[test]
    fn note_for_yourself_ignore_shows_final_leave_screen() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::NoteForYourself));

        let initial_choices = run
            .event
            .as_ref()
            .expect("Note For Yourself is open")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(initial_choices, vec!["Continue"]);

        let after_continue = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("intro continue choice applies");
        let guided_choices = after_continue
            .event
            .as_ref()
            .expect("Note For Yourself remains open")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(after_continue.phase, RunPhase::Event);
        assert_eq!(guided_choices, vec!["Take and Give (Iron Wave)", "Ignore"]);

        let after_ignore =
            apply_event_action(&after_continue, EventAction::Choose { choice_index: 1 })
                .expect("ignore choice shows leave screen");
        assert_eq!(after_ignore.phase, RunPhase::Event);
        assert_eq!(after_ignore.event.as_ref().expect("leave screen").stage, 2);

        let after_leave =
            apply_event_action(&after_ignore, EventAction::Choose { choice_index: 0 })
                .expect("leave choice closes the event");
        assert_eq!(after_leave.phase, RunPhase::Idle);
        assert!(after_leave.event.is_none());
    }

    #[test]
    fn note_for_yourself_take_and_give_adds_note_then_removes_a_card() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::NoteForYourself));
        let original_len = run.deck.len();

        let after_continue = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("intro continue choice applies");
        let after_take =
            apply_event_action(&after_continue, EventAction::Choose { choice_index: 0 })
                .expect("take choice opens removal grid");
        assert!(matches!(
            after_take.card_grid.as_ref().expect("removal grid").purpose,
            crate::GridPurpose::EventRemoveReturnToEvent {
                event: Event::NoteForYourself
            }
        ));
        assert_eq!(after_take.deck.len(), original_len + 1);

        let selected = crate::run::grid::select_grid_card(&after_take, 0)
            .expect("card can be selected for removal");
        let completed = crate::run::grid::confirm_grid(&selected).expect("note removal confirms");
        assert_eq!(completed.deck.len(), original_len);
        assert_eq!(completed.event.as_ref().expect("leave screen").stage, 2);
    }

    #[test]
    fn note_for_yourself_uses_profile_card_and_upgrade() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.note_card_content_id = BASH_ID;
        run.note_card_upgrades = 1;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen_for_run(&run, Event::NoteForYourself));

        let after_continue = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("intro continue choice applies");
        assert!(after_continue
            .event
            .as_ref()
            .expect("choice screen")
            .choices[0]
            .label
            .contains("Bash+"));
        let after_take =
            apply_event_action(&after_continue, EventAction::Choose { choice_index: 0 })
                .expect("profile note card is obtained");
        assert!(after_take
            .deck
            .iter()
            .any(|card| card.content_id == BASH_PLUS_ID));
    }

    #[test]
    fn shining_light_leave_shows_final_leave_screen_before_map() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::ShiningLight));

        let after_first_leave = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect("initial leave choice applies");
        let choices = after_first_leave
            .event
            .as_ref()
            .expect("Shining Light remains open for final leave")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(after_first_leave.phase, RunPhase::Event);
        assert_eq!(choices, vec!["Leave"]);

        let after_final_leave =
            apply_event_action(&after_first_leave, EventAction::Choose { choice_index: 0 })
                .expect("final leave choice returns to map");
        assert_eq!(after_final_leave.phase, RunPhase::Idle);
        assert!(after_final_leave.event.is_none());
    }

    #[test]
    fn wing_statue_pray_shows_continue_before_remove_grid() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::WingStatue,
            choices: wing_statue_choices(0, false),
            stage: 0,
            event_data: 0,
        });

        let after_pray = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("pray choice applies");
        let choices = after_pray
            .event
            .as_ref()
            .expect("Wing Statue remains open after Pray")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(choices, vec!["Continue"]);

        let after_continue =
            apply_event_action(&after_pray, EventAction::Choose { choice_index: 0 })
                .expect("continue choice applies");
        assert!(
            after_continue.card_grid.is_some(),
            "Continue after Pray should open the remove-card grid"
        );
    }

    #[test]
    fn scrap_ooze_leave_after_failed_reach_shows_final_leave_screen() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.player_hp = 77;
        run.event = Some(EventScreen {
            event: Event::ScrapOoze,
            choices: scrap_ooze_choices(1),
            stage: 1,
            event_data: 1,
        });

        let after_leave = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect("leave choice applies");
        let choices = after_leave
            .event
            .as_ref()
            .expect("Scrap Ooze remains open for final leave")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(after_leave.phase, RunPhase::Event);
        assert_eq!(choices, vec!["Leave"]);
    }

    #[test]
    fn big_fish_box_queues_regret_until_obtain_effect_resolves() {
        let mut run = RunState::placeholder_seeded_ironclad(1_260_350_191_924, 0);
        run.phase = RunPhase::Event;
        run.current_act = 1;
        run.current_floor = 5;
        run.event = Some(EventScreen {
            event: Event::BigFish,
            choices: big_fish_choices(0),
            stage: 0,
            event_data: 0,
        });

        let after_box = apply_event_action(&run, EventAction::Choose { choice_index: 2 })
            .expect("Big Fish box choice applies");
        let regret_count = after_box
            .deck
            .iter()
            .filter(|card| card.content_id == REGRET_ID)
            .count();
        assert_eq!(regret_count, 0);
        assert_eq!(after_box.pending_obtain_cards, vec![REGRET_ID]);
        assert!(after_box.relics.len() + after_box.relic_keys.len() > run.relics.len());

        let after_leave = apply_event_action(&after_box, EventAction::Choose { choice_index: 0 })
            .expect("Big Fish leave applies");
        let final_regret_count = after_leave
            .deck
            .iter()
            .filter(|card| card.content_id == REGRET_ID)
            .count();
        assert_eq!(final_regret_count, 1);
        assert!(after_leave.pending_obtain_cards.is_empty());
    }

    #[test]
    fn accursed_blacksmith_rummage_queues_pain_until_leave_screen() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.current_act = 1;
        run.event = Some(EventScreen {
            event: Event::AccursedBlacksmith,
            choices: labeled_choices(&["Forge", "Rummage", "Leave"]),
            stage: 0,
            event_data: 0,
        });

        let after_rummage = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect("Accursed Blacksmith rummage applies");

        assert!(has_relic_key(&after_rummage, RelicKey::WarpedTongs));
        assert!(!after_rummage
            .deck
            .iter()
            .any(|card| card.content_id == PAIN_ID));
        assert_eq!(after_rummage.pending_obtain_cards, vec![PAIN_ID]);

        let after_leave =
            apply_event_action(&after_rummage, EventAction::Choose { choice_index: 0 })
                .expect("Accursed Blacksmith leave applies");
        assert_eq!(
            after_leave
                .deck
                .iter()
                .filter(|card| card.content_id == PAIN_ID)
                .count(),
            1
        );
        assert!(after_leave.pending_obtain_cards.is_empty());
    }

    #[test]
    fn golden_shrine_desecrate_queues_regret_until_leave_screen() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen_for_run(&run, Event::GoldenShrine));

        let after_desecrate = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect("Golden Shrine desecrate applies");

        assert_eq!(
            after_desecrate.gold,
            run.gold + GOLDEN_SHRINE_DESECRATE_GOLD
        );
        assert!(!after_desecrate
            .deck
            .iter()
            .any(|card| card.content_id == REGRET_ID));
        assert_eq!(after_desecrate.pending_obtain_cards, vec![REGRET_ID]);

        let after_leave =
            apply_event_action(&after_desecrate, EventAction::Choose { choice_index: 0 })
                .expect("Golden Shrine leave applies");
        assert_eq!(
            after_leave
                .deck
                .iter()
                .filter(|card| card.content_id == REGRET_ID)
                .count(),
            1
        );
        assert!(after_leave.pending_obtain_cards.is_empty());
    }

    #[test]
    fn golden_shrine_pray_uses_a15_gold_amount() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 15);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen_for_run(&run, Event::GoldenShrine));

        let after_pray = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("A15 Golden Shrine pray applies");
        assert_eq!(after_pray.gold, run.gold + GOLDEN_SHRINE_A15_GOLD);
    }

    #[test]
    fn golden_shrine_leave_does_not_change_event_identity() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen_for_run(&run, Event::GoldenShrine));

        let after_leave = apply_event_action(&run, EventAction::Choose { choice_index: 2 })
            .expect("Golden Shrine leave applies");
        assert_eq!(
            after_leave.event.as_ref().expect("leave screen").event,
            Event::GoldenShrine
        );
        assert_eq!(after_leave.event.as_ref().expect("leave screen").stage, 1);
    }

    #[test]
    fn dead_adventurer_search_awards_progress_and_keeps_search_state() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.current_floor = 7;
        run.phase = RunPhase::Event;
        let gold_before = run.gold;
        let mut search_counter = None;
        for counter in 0..64 {
            let mut rng = StsRng::with_counter(run.misc_rng_seed as i64, counter);
            if rng.random_int(99) >= 25 {
                search_counter = Some(counter);
                break;
            }
        }
        run.misc_rng_counter = search_counter.expect("a non-encounter RNG draw exists");
        run.event = Some(dead_adventurer_screen(&run, 0, 0));

        let after_search = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Dead Adventurer search applies");
        assert_eq!(after_search.phase, RunPhase::Event);
        assert!(after_search.combat.is_none());
        assert_eq!(after_search.gold, gold_before + 30);
        assert_eq!(after_search.event.as_ref().expect("event").stage, 0);
        assert!(after_search.event.as_ref().expect("event").choices[0]
            .label
            .starts_with("Continue"));
        assert_eq!(
            dead_adventurer_attempts(after_search.event.as_ref().expect("event").event_data),
            1
        );
    }

    #[test]
    fn hypnotizing_mushrooms_heal_path_adds_parasite_on_leave() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.current_floor = 7;
        run.player_hp = 40;
        run.player_max_hp = 80;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::HypnotizingColoredMushrooms));
        assert_eq!(
            run.event
                .as_ref()
                .expect("Mushrooms event")
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Stomp", "Eat"]
        );

        let after_heal = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect("Mushroom heal choice applies");
        assert_eq!(after_heal.player_hp, 60);
        assert_eq!(after_heal.pending_obtain_cards, vec![PARASITE_ID]);

        let after_leave = apply_event_action(&after_heal, EventAction::Choose { choice_index: 0 })
            .expect("Mushroom leave applies");
        assert!(after_leave.event.is_none());
        assert!(after_leave
            .deck
            .iter()
            .any(|card| card.content_id == PARASITE_ID));
    }

    #[test]
    fn hypnotizing_mushrooms_fight_spawns_three_fungi_beasts() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.current_floor = 8;
        run.monster_rng_seed = 10_634_058_411_488_052_108;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::HypnotizingColoredMushrooms));

        let after_stomp = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Mushroom fight choice applies");
        let monsters = &after_stomp.combat.as_ref().expect("event combat").monsters;

        assert_eq!(monsters.len(), 3);
        assert!(monsters
            .iter()
            .all(|monster| monster.content_id == FUNGI_BEAST_A0.content_id));
        assert!(matches!(
            monsters[0].intent,
            MonsterIntent::Attack { damage: 6 }
        ));
        assert!(matches!(
            monsters[1].intent,
            MonsterIntent::Attack { damage: 6 }
        ));
        assert!(matches!(
            monsters[2].intent,
            MonsterIntent::StrengthSelf { amount: 3 }
        ));
        assert_eq!(
            after_stomp
                .combat
                .as_ref()
                .and_then(|combat| combat.monster_rng.as_ref())
                .map(StsRng::counter),
            Some(3)
        );
    }

    #[test]
    fn the_library_read_uses_twenty_unique_cards_without_changing_rarity_factor() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.current_act = 2;
        run.current_floor = 29;
        run.card_rarity_factor = -7;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::TheLibrary));

        let after_read = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Library read choice opens a grid");
        let grid = after_read.card_grid.as_ref().expect("Library card grid");
        let unique = grid
            .cards
            .iter()
            .map(|card| card.content_id)
            .collect::<std::collections::HashSet<_>>();

        assert_eq!(grid.cards.len(), THE_LIBRARY_READ_CARD_COUNT);
        assert_eq!(unique.len(), THE_LIBRARY_READ_CARD_COUNT);
        assert_eq!(after_read.card_rarity_factor, -7);
    }

    #[test]
    fn vampires_accept_stages_leave_before_returning_to_map() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.current_act = 2;
        run.current_floor = 31;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Vampires));
        assert_eq!(
            run.event
                .as_ref()
                .expect("Vampires event")
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Accept", "Refuse"]
        );

        let accepted = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Vampires accept applies");
        assert_eq!(accepted.phase, RunPhase::Event);
        assert_eq!(
            accepted.event.as_ref().expect("leave stage").choices[0].label,
            "Leave"
        );
        assert!(!accepted
            .deck
            .iter()
            .any(|card| matches!(card.content_id, STRIKE_R_ID | STRIKE_R_PLUS_ID)));
        assert_eq!(
            accepted
                .deck
                .iter()
                .filter(|card| card.content_id == BITE_ID)
                .count(),
            VAMPIRES_BITE_COUNT
        );

        let left = apply_event_action(&accepted, EventAction::Choose { choice_index: 0 })
            .expect("Vampires leave returns to map");
        assert_eq!(left.phase, RunPhase::Idle);
        assert!(left.event.is_none());
    }

    #[test]
    fn nloth_trades_one_owned_relic_for_nloths_gift() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.current_act = 2;
        run.gain_relic(Relic::Vajra);
        run.gain_relic(Relic::Strawberry);
        run.phase = RunPhase::Event;
        let event_data = nloth_event_data(0, 1);
        run.event = Some(EventScreen {
            event: Event::Nloth,
            choices: nloth_choices(&run, 0, event_data),
            stage: 0,
            event_data,
        });

        let after_trade = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("N'loth trade applies");
        assert!(!after_trade.relics.contains(&Relic::BurningBlood));
        assert!(has_relic_key(&after_trade, RelicKey::NlothsGift));
        assert_eq!(after_trade.event.as_ref().expect("leave screen").stage, 1);
    }

    #[test]
    fn lab_search_offers_source_backed_potion_choices() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen_for_run(&run, Event::Lab));

        let after_search = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Lab search applies");
        let offers = after_search
            .reward
            .as_ref()
            .expect("Lab opens reward screen")
            .potion_offers
            .clone();

        assert_eq!(after_search.phase, RunPhase::Reward);
        assert_eq!(offers.len(), 3);
        assert_eq!(after_search.potion_rng_counter, 3);
        assert!(after_search.reward.as_ref().unwrap().potion_offer.is_none());

        let after_pick = apply_run_action(&after_search, RunAction::TakePotionReward { index: 1 })
            .expect("Lab potion pick applies");
        assert_eq!(after_pick.potions, vec![offers[1]]);
        assert_eq!(
            after_pick.reward.as_ref().unwrap().potion_offers,
            vec![offers[0], offers[2]]
        );

        let mut ascension_fifteen = RunState::placeholder_seeded_ironclad(1, 15);
        ascension_fifteen.phase = RunPhase::Event;
        ascension_fifteen.event = Some(event_screen_for_run(&ascension_fifteen, Event::Lab));
        let after_search_a15 =
            apply_event_action(&ascension_fifteen, EventAction::Choose { choice_index: 0 })
                .expect("Ascension 15 Lab search applies");
        assert_eq!(
            after_search_a15
                .reward
                .as_ref()
                .unwrap()
                .potion_offers
                .len(),
            2
        );
        assert_eq!(after_search_a15.potion_rng_counter, 2);
    }

    #[test]
    fn golden_idol_queues_injury_until_obtain_effect_resolves() {
        let mut run = RunState::placeholder_seeded_ironclad(1_435_099_163_226, 0);
        run.phase = RunPhase::Event;
        run.current_act = 1;
        run.event = Some(EventScreen {
            event: Event::GoldenIdol,
            choices: golden_idol_choices(0, run.player_max_hp, run.ascension),
            stage: 0,
            event_data: 0,
        });

        let after_take = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Golden Idol take choice applies");
        assert!(has_relic_key(&after_take, RelicKey::GoldenIdol));

        let after_outrun = apply_event_action(&after_take, EventAction::Choose { choice_index: 0 })
            .expect("Golden Idol outrun choice applies");
        assert!(!after_outrun
            .deck
            .iter()
            .any(|card| card.content_id == INJURY_ID));
        assert_eq!(after_outrun.pending_obtain_cards, vec![INJURY_ID]);

        let after_leave =
            apply_event_action(&after_outrun, EventAction::Choose { choice_index: 0 })
                .expect("Golden Idol leave applies");
        assert_eq!(
            after_leave
                .deck
                .iter()
                .filter(|card| card.content_id == INJURY_ID)
                .count(),
            1
        );
        assert!(after_leave.pending_obtain_cards.is_empty());
    }

    #[test]
    fn fountain_of_cleansing_drink_removes_curses_then_shows_leave() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.current_act = 1;
        run.gain_deck_card(INJURY_ID);
        run.gain_deck_card(ASCENDERS_BANE_ID);
        run.gain_deck_card(CURSE_OF_THE_BELL_ID);
        run.event = Some(event_screen(Event::FountainOfCleansing));

        let initial_choices = run
            .event
            .as_ref()
            .expect("event screen")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(initial_choices, vec!["Drink", "Leave"]);

        let after_drink = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Fountain drink choice applies");

        assert!(!after_drink
            .deck
            .iter()
            .any(|card| card.content_id == INJURY_ID));
        assert!(after_drink
            .deck
            .iter()
            .any(|card| card.content_id == ASCENDERS_BANE_ID));
        assert!(after_drink
            .deck
            .iter()
            .any(|card| card.content_id == CURSE_OF_THE_BELL_ID));
        assert_eq!(
            after_drink
                .event
                .as_ref()
                .expect("leave screen")
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Leave"]
        );

        let after_leave = apply_event_action(&after_drink, EventAction::Choose { choice_index: 0 })
            .expect("Fountain leave applies");
        assert_eq!(after_leave.phase, RunPhase::Idle);
        assert!(after_leave.event.is_none());
    }

    #[test]
    fn ghosts_accept_queues_apparitions_until_leave_screen() {
        let mut run = RunState::placeholder_seeded_ironclad(772_776_727_775, 0);
        run.phase = RunPhase::Event;
        run.current_act = 2;
        run.event = Some(EventScreen {
            event: Event::Ghosts,
            choices: ghosts_choices(0, run.player_max_hp),
            stage: 0,
            event_data: 0,
        });

        let initial_choices = run
            .event
            .as_ref()
            .expect("Ghosts event is open")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(initial_choices[1], "Refuse");

        let initial_deck_len = run.deck.len();
        let after_accept = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Ghosts accept applies");
        assert_eq!(
            after_accept
                .event
                .as_ref()
                .expect("Ghosts leave screen is open")
                .choices[0]
                .label,
            "Leave"
        );
        assert_eq!(after_accept.deck.len(), initial_deck_len);
        assert_eq!(
            after_accept.pending_obtain_cards.len(),
            ghosts_apparition_count(after_accept.ascension)
        );

        let after_leave =
            apply_event_action(&after_accept, EventAction::Choose { choice_index: 0 })
                .expect("Ghosts leave applies");
        assert_eq!(
            after_leave
                .deck
                .iter()
                .filter(|card| card.content_id == APPARITION_ID)
                .count(),
            ghosts_apparition_count(after_leave.ascension)
        );
        assert!(after_leave.pending_obtain_cards.is_empty());
    }

    #[test]
    fn the_joust_resolves_bet_against_when_owner_loses() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.current_act = 2;
        run.gold = 50;
        run.event = Some(event_screen(Event::TheJoust));

        let after_halt = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Joust explanation applies");
        let after_bet = apply_event_action(&after_halt, EventAction::Choose { choice_index: 0 })
            .expect("Joust bet applies");
        assert_eq!(after_bet.gold, 0);
        assert_eq!(after_bet.event.as_ref().expect("pre-joust").stage, 2);

        let mut forced_joust = after_bet;
        forced_joust.event = Some(EventScreen {
            event: Event::TheJoust,
            choices: joust_choices(3),
            stage: 3,
            event_data: joust_event_data(false, false),
        });
        let after_resolve =
            apply_event_action(&forced_joust, EventAction::Choose { choice_index: 0 })
                .expect("Joust resolution applies");
        assert_eq!(after_resolve.gold, 100);
        assert_eq!(after_resolve.event.as_ref().expect("complete").stage, 4);
    }

    #[test]
    fn the_woman_in_blue_offers_potions_and_a15_punch() {
        let mut buyer = RunState::placeholder_seeded_ironclad(1, 0);
        buyer.phase = RunPhase::Event;
        buyer.gold = 40;
        buyer.event = Some(event_screen_for_run(&buyer, Event::TheWomanInBlue));

        let after_buy = apply_event_action(&buyer, EventAction::Choose { choice_index: 2 })
            .expect("Woman in Blue purchase applies");
        assert_eq!(after_buy.gold, 0);
        assert_eq!(after_buy.phase, RunPhase::Reward);
        assert_eq!(
            after_buy
                .reward
                .as_ref()
                .expect("potion reward")
                .potion_offers
                .len(),
            3
        );

        let mut punched = RunState::placeholder_seeded_ironclad(1, 15);
        punched.phase = RunPhase::Event;
        punched.event = Some(event_screen_for_run(&punched, Event::TheWomanInBlue));
        let initial_hp = punched.player_hp;
        let after_punch = apply_event_action(&punched, EventAction::Choose { choice_index: 3 })
            .expect("Woman in Blue punch applies");
        assert_eq!(
            after_punch.player_hp,
            initial_hp - (punched.player_max_hp + 19) / 20
        );
        assert_eq!(after_punch.event.as_ref().expect("result").stage, 1);
    }

    #[test]
    fn falling_removes_a_random_card_of_the_selected_type() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen_for_run(&run, Event::Falling));
        let after_intro = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Falling intro applies");
        let skill_index = after_intro
            .event
            .as_ref()
            .expect("Falling choices")
            .choices
            .iter()
            .position(|choice| choice.label.contains("Skill"))
            .expect("starter deck has a skill");
        let after_choice = apply_event_action(
            &after_intro,
            EventAction::Choose {
                choice_index: skill_index,
            },
        )
        .expect("Falling card choice applies");
        assert_eq!(
            after_choice
                .card_grid
                .as_ref()
                .expect("card grid")
                .cards
                .len(),
            1
        );
        let selected = crate::run::grid::select_grid_card(&after_choice, 0)
            .expect("Falling selects its displayed card");
        let completed = crate::run::grid::confirm_grid(&selected).expect("Falling removes card");
        assert_eq!(completed.event.as_ref().expect("leave screen").stage, 2);
    }

    #[test]
    fn moai_head_accept_heals_after_max_hp_loss() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.player_hp = 10;
        run.player_max_hp = 80;
        run.event = Some(event_screen_for_run(&run, Event::MoaiHead));
        let after_accept = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Moai accept applies");
        assert_eq!(after_accept.player_max_hp, 70);
        assert_eq!(after_accept.player_hp, 70);
        assert_eq!(after_accept.event.as_ref().expect("leave screen").stage, 1);
    }

    #[test]
    fn mysterious_sphere_fight_enters_two_orb_walker_combat() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.current_act = 3;
        run.event = Some(event_screen(Event::MysteriousSphere));
        let after_fight = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Sphere fight applies");
        let after_continue =
            apply_event_action(&after_fight, EventAction::Choose { choice_index: 0 })
                .expect("Sphere combat entry applies");
        assert_eq!(after_continue.phase, RunPhase::Combat);
        assert_eq!(
            after_continue
                .combat
                .as_ref()
                .expect("combat")
                .monsters
                .len(),
            2
        );
    }

    #[test]
    fn winding_halls_card_obtains_wait_for_the_leave_stage() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.player_hp = 20;
        run.relics.push(Relic::CeramicFish);
        run.event = Some(event_screen_for_run(&run, Event::WindingHalls));
        assert_eq!(run.event.as_ref().expect("intro").choices[0].label, "...");
        let after_intro = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Winding Halls intro applies");
        let labels = after_intro
            .event
            .as_ref()
            .expect("choices")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(labels.len(), 3);
        assert!(labels[0].starts_with("Embrace Madness"));
        assert!(labels[1].starts_with("Focus"));
        assert!(labels[2].starts_with("Retrace Your Steps"));

        let after_madness =
            apply_event_action(&after_intro, EventAction::Choose { choice_index: 0 })
                .expect("Madness path applies");
        assert_eq!(
            after_madness.pending_obtain_cards,
            vec![MADNESS_ID, MADNESS_ID]
        );

        let gold_before = after_intro.gold;
        let after_choice =
            apply_event_action(&after_intro, EventAction::Choose { choice_index: 1 })
                .expect("Writhe path applies");
        assert_eq!(after_choice.player_hp, 40);
        assert!(!after_choice
            .deck
            .iter()
            .any(|card| card.content_id == WRITHE_ID));
        assert_eq!(after_choice.pending_obtain_cards, vec![WRITHE_ID]);
        assert_eq!(after_choice.gold, gold_before);
        assert_eq!(after_choice.event.as_ref().expect("leave screen").stage, 2);

        let after_leave =
            apply_event_action(&after_choice, EventAction::Choose { choice_index: 0 })
                .expect("Winding Halls leave applies");
        assert!(after_leave.pending_obtain_cards.is_empty());
        assert!(after_leave
            .deck
            .iter()
            .any(|card| card.content_id == WRITHE_ID));
        assert_eq!(after_leave.gold, gold_before + CERAMIC_FISH_GOLD);
    }

    #[test]
    fn mind_bloom_third_choice_changes_after_floor_forty() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.current_floor = 39;
        assert_eq!(mind_bloom_choices(&run)[2].label, "I am Rich");

        run.current_floor = 41;
        assert_eq!(mind_bloom_choices(&run)[2].label, "I am Healthy");
    }

    #[test]
    fn mind_bloom_boss_fight_activates_slavers_collar_without_becoming_a_boss_room() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.current_floor = 39;
        run.current_room_override = Some(RoomKind::Event);
        run.relics.push(Relic::SlaversCollar);
        run.event = Some(event_screen_for_run(&run, Event::MindBloom));

        let fight = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Mind Bloom boss fight starts");

        let combat = fight.combat.as_ref().expect("Mind Bloom combat");
        assert_eq!(combat.player.max_energy, run.energy_per_turn + 1);
        assert_eq!(combat.player.energy, run.energy_per_turn + 1);
        assert_eq!(fight.current_room_kind(), Some(RoomKind::Event));
    }

    #[test]
    fn mind_bloom_gold_path_awards_gold_and_normality() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.current_floor = 10;
        run.event = Some(event_screen_for_run(&run, Event::MindBloom));
        let after_gold = apply_event_action(&run, EventAction::Choose { choice_index: 2 })
            .expect("Mind Bloom gold path applies");
        assert_eq!(after_gold.gold, run.gold + 999);
        assert_eq!(
            after_gold
                .deck
                .iter()
                .filter(|card| card.content_id == NORMALITY_ID)
                .count(),
            2
        );
        assert_eq!(after_gold.event.as_ref().expect("leave screen").stage, 1);
    }

    #[test]
    fn mark_of_bloom_blocks_run_and_combat_healing() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.gain_relic_key(RelicKey::MarkOfBloom);
        run.player_hp = 20;
        run.heal_player(30);
        assert_eq!(run.player_hp, 20);

        let mut combat = run.init_combat(CombatState::initial_fixture());
        assert!(combat.mark_of_bloom);
        let before = combat.player.hp;
        crate::relic::heal_combat_player_with_relics(&mut combat, 30);
        assert_eq!(combat.player.hp, before);
    }

    #[test]
    fn sensory_stone_opens_colorless_reward_with_hp_cost() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.player_hp = 50;
        run.event = Some(event_screen_for_run(&run, Event::SensoryStone));
        assert_eq!(
            run.event.as_ref().expect("event").choices[0].label,
            "Interact"
        );
        let after_intro = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Sensory Stone intro applies");
        assert_eq!(
            after_intro
                .event
                .as_ref()
                .expect("memory choices")
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Recall", "Recall", "Recall"]
        );
        let after_memory =
            apply_event_action(&after_intro, EventAction::Choose { choice_index: 1 })
                .expect("Sensory Stone memory applies");
        assert_eq!(after_memory.player_hp, 45);
        assert_eq!(after_memory.phase, RunPhase::Reward);
        let reward = after_memory.reward.as_ref().expect("reward");
        assert!(!reward.card_reward_active);
        assert_eq!(reward.pending_card_reward_count(), 2);
        assert_eq!(reward.queued_card_rewards.len(), 2);
        assert!(reward.choices.is_empty());
        assert!(reward
            .queued_card_rewards
            .iter()
            .all(|choices| choices.len() == 3));
        assert!(reward
            .queued_card_rewards
            .iter()
            .flatten()
            .all(|card| crate::content::shop_pool::shop_card_is_colorless(card.content_id)));

        let opened =
            crate::run::reward::apply_run_action(&after_memory, crate::RunAction::OpenCardReward)
                .expect("first colorless reward opens");
        assert!(opened.reward.as_ref().expect("reward").card_reward_active);
        assert_eq!(opened.reward.as_ref().expect("reward").choices.len(), 3);
        assert_eq!(
            opened
                .event
                .as_ref()
                .expect("Sensory Stone leave screen")
                .choices[0]
                .label,
            "Leave"
        );

        let skipped =
            crate::run::reward::apply_run_action(&after_memory, crate::RunAction::SkipReward)
                .expect("pending Sensory Stone rewards can be skipped");
        assert_eq!(skipped.phase, RunPhase::Event);
        assert!(skipped.reward.is_none());
        assert_eq!(
            skipped
                .event
                .as_ref()
                .expect("Sensory Stone leave screen")
                .choices[0]
                .label,
            "Leave"
        );
    }
}
