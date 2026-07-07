use crate::{
    card::{CardRarity, CardType},
    combat::initialize_combat_piles_with_relics,
    content::cards::{
        get_card_definition, is_curse_content_id, upgrade_card_instance, upgrade_content_id,
        APPARITION_ID, BITE_ID, DECAY_ID, DEFEND_R_ID, DOUBT_ID, INJURY_ID, JAX_ID, PAIN_ID,
        REGRET_ID, RITUAL_DAGGER_ID, SHAME_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID, WRITHE_ID,
    },
    content::{
        monsters::{
            monster_state_for_ascension, record_target_move,
            target_monster_hp_range_for_content_id, MonsterDefinition, BANDIT_BEAR_A0,
            BANDIT_LEADER_A0, BANDIT_POINTY_A0, GREMLIN_NOB_A0, SLAVER_BLUE_A0, SLAVER_RED_A0,
            SLIME_BOSS_A0, TASKMASTER_A0,
        },
        shop_pool::random_colorless_from_pool,
    },
    ids::ContentId,
    potion::Potion,
    relic::{Relic, RelicKey, RelicTier},
    rng::{JavaRng, StsRng},
    run::{
        grid::{
            open_event_obtain_card_grid, open_event_remove_grid,
            open_event_remove_return_to_event_grid, open_event_transform_grid,
            open_event_transform_return_to_event_grid, open_event_upgrade_return_to_event_grid,
        },
        neow::{
            apply_neow_boss_swap, apply_neow_curse_drawback, apply_neow_lament_reward,
            apply_neow_relic_reward, apply_neow_simple_drawback, apply_neow_simple_reward,
            generate_neow_card_reward, generate_neow_colorless_reward_with_card_rng_counter,
            generate_neow_options, generate_neow_three_potions, open_neow_reward_grid,
            GeneratedNeowOption, NeowDrawback, NeowRewardType,
        },
        reward::{
            roll_event_relic_reward, target_card_reward_choices_with_count, target_random_potion,
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
pub const GOLDEN_SHRINE_DESECRATE_GOLD: i32 = 275;
pub const WORLD_OF_GOOP_DAMAGE: i32 = 11;
pub const WORLD_OF_GOOP_GOLD: i32 = 75;
const WE_MEET_AGAIN_NO_POTION_SLOT: u32 = u32::MAX;
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
    let mut rarity_factor = run.card_rarity_factor;
    let next_card_id = run.next_card_instance_id();
    let choices = target_card_reward_choices_with_count(
        &mut card_rng,
        &mut rarity_factor,
        next_card_id,
        THE_LIBRARY_READ_CARD_COUNT,
    );
    run.card_rarity_factor = rarity_factor;
    run.store_rng_counter(RunRngStream::CardReward, &card_rng);
    open_event_obtain_card_grid(run, choices);
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
                label: format!(
                    "Take the book (lose {} HP)",
                    cursed_tome_final_hp_loss(ascension)
                ),
            },
            EventChoice {
                label: "Stop reading".to_owned(),
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
        0 => labeled_choices(&["Buy relic", "Steal relic", "Leave"]),
        1 => labeled_choices(&["Leave"]),
        _ => Vec::new(),
    }
}

fn forgotten_altar_choices(stage: u8, max_hp: i32, ascension: u8) -> Vec<EventChoice> {
    match stage {
        0 => vec![
            EventChoice {
                label: "Give Idol".to_owned(),
            },
            EventChoice {
                label: format!(
                    "Shed blood (gain {FORGOTTEN_ALTAR_MAX_HP_GAIN} max HP, lose {} HP)",
                    forgotten_altar_hp_loss(max_hp, ascension)
                ),
            },
            EventChoice {
                label: "Smash altar (obtain Decay)".to_owned(),
            },
        ],
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
                label: "Leave".to_owned(),
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
                label: "Take J.A.X.".to_owned(),
            },
            EventChoice {
                label: if transform_enabled {
                    "Become test subject".to_owned()
                } else {
                    "Become test subject (requires 2 cards)".to_owned()
                },
            },
            EventChoice {
                label: "Inject mutagens".to_owned(),
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

fn dead_adventurer_choices(stage: u8) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Search", "Leave"]),
        1 => labeled_choices(&["Leave"]),
        _ => Vec::new(),
    }
}

fn vampires_choices(has_blood_vial: bool) -> Vec<EventChoice> {
    if has_blood_vial {
        labeled_choices(&["Accept", "Give Blood Vial", "Leave"])
    } else {
        labeled_choices(&["Accept", "Leave"])
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
    run.event = None;
    run.reward = Some(RewardScreen {
        choices: Vec::new(),
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
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
        .filter_map(|(index, card)| {
            upgrade_content_id(card.content_id)
                .is_some()
                .then_some(index)
        })
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

fn face_trader_choices(stage: u32) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Continue"]),
        1 => labeled_choices(&["Touch", "Trade", "Leave"]),
        _ => labeled_choices(&["Leave"]),
    }
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

fn we_meet_again_choices(stage: u32, potion_slot: Option<usize>) -> Vec<EventChoice> {
    if stage > 0 {
        return labeled_choices(&["Leave"]);
    }

    let potion_label = if potion_slot.is_some() {
        "Give Potion"
    } else {
        "No Potion"
    };
    labeled_choices(&[potion_label, "Give Gold", "Give Card", "Attack"])
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

fn we_meet_again_event_data_for_potion_slot(slot: Option<usize>) -> u32 {
    slot.and_then(|slot| u32::try_from(slot).ok())
        .unwrap_or(WE_MEET_AGAIN_NO_POTION_SLOT)
}

fn we_meet_again_potion_slot_from_event_data(event_data: u32) -> Option<usize> {
    (event_data != WE_MEET_AGAIN_NO_POTION_SLOT).then_some(event_data as usize)
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
        Event::SecretPortal => false,
        Event::TheJoust => run.current_act == 2 && run.gold >= 50,
        Event::TheWomanInBlue => run.gold >= 50,
        _ => true,
    }
}

fn deck_has_curse(deck: &[CardInstance]) -> bool {
    deck.iter().any(|card| is_curse_content_id(card.content_id))
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
        Event::Colosseum => current_floor_in_act(run) > 7,
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

fn wheel_of_change_choices(stage: u32, result: u32) -> Vec<EventChoice> {
    match stage {
        0 => labeled_choices(&["Play"]),
        1 => labeled_choices(&["spin"]),
        2 => match result {
            0 => labeled_choices(&["Claim Gold"]),
            1 => labeled_choices(&["Claim Relic"]),
            2 => labeled_choices(&["Heal"]),
            3 => labeled_choices(&["Take Curse"]),
            4 => labeled_choices(&["Remove Card"]),
            _ => labeled_choices(&["Take Damage"]),
        },
        _ => labeled_choices(&["Leave"]),
    }
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
    run.event = Some(entered_event_screen_for_run(run, event));
}

#[must_use]
pub fn event_screen(event: Event) -> EventScreen {
    match event {
        Event::Neow => make_event_screen(event, neow_talk_choices(), 0),
        Event::AccursedBlacksmith => {
            make_event_screen(event, labeled_choices(&["Forge", "Rummage", "Leave"]), 0)
        }
        Event::GoldenShrine => legacy_fixed_event_screen(),
        Event::Purifier => make_event_screen(
            event,
            vec![EventChoice {
                label: "Purify".to_owned(),
            }],
            0,
        ),
        Event::UpgradeShrine => make_event_screen(
            event,
            vec![EventChoice {
                label: "Upgrade".to_owned(),
            }],
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
        Event::DeadAdventurer => make_event_screen(event, dead_adventurer_choices(0), 0),
        Event::TheSsssserpent => make_event_screen(event, sssssserpent_choices(0), 0),
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
        Event::ForgottenAltar => make_event_screen(event, forgotten_altar_choices(0, 0, 0), 0),
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
        Event::WeMeetAgain => make_event_screen(event, we_meet_again_choices(0, None), 0),
        Event::MindBloom => make_event_screen(
            event,
            labeled_choices(&["I am War", "I am Awake", "I am Healthy"]),
            0,
        ),
        _ => make_event_screen(
            event,
            vec![EventChoice {
                label: "Continue".to_owned(),
            }],
            0,
        ),
    }
}

#[must_use]
pub fn event_screen_for_run(run: &RunState, event: Event) -> EventScreen {
    match event {
        Event::Neow => make_event_screen(event, neow_option_choices(run), 1),
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
        _ => event_screen(event),
    }
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
            let potion_slot = we_meet_again_random_potion_slot(run);
            EventScreen {
                event,
                choices: we_meet_again_choices(0, potion_slot),
                stage: 0,
                event_data: we_meet_again_event_data_for_potion_slot(potion_slot),
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
        choices,
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
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
    let key = super::reward::roll_event_relic_reward(next, i32::from(next.current_act));
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
    let screen = next.event.as_ref().expect("validated event screen").clone();
    let EventAction::Choose { choice_index } = action;

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
        Event::GoldenShrine => match screen.stage {
            0 if choice_index == 0 => {
                next.gain_gold(GOLDEN_SHRINE_GOLD);
                next.event = Some(make_event_screen(
                    Event::GoldenShrine,
                    golden_shrine_choices(1),
                    1,
                ));
            }
            0 if choice_index == 1 => {
                next.gain_gold(GOLDEN_SHRINE_DESECRATE_GOLD);
                next.gain_deck_card(REGRET_ID);
                next.event = Some(make_event_screen(
                    Event::GoldenShrine,
                    golden_shrine_choices(1),
                    1,
                ));
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
                next.gain_deck_card(PAIN_ID);
                next.event = Some(make_event_screen(
                    Event::AccursedBlacksmith,
                    labeled_choices(&["Leave"]),
                    1,
                ));
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
        Event::WeMeetAgain => match screen.stage {
            0 if choice_index == 0 => {
                let Some(slot) = we_meet_again_potion_slot_from_event_data(screen.event_data)
                else {
                    return Err(SimError::IllegalAction(
                        "We Meet Again potion option is unavailable",
                    ));
                };
                next.take_potion_slot(slot)?;
                let act = i32::from(next.current_act);
                let key = roll_event_relic_reward(&mut next, act);
                next.gain_relic_key(key);
                next.event = Some(EventScreen {
                    event: Event::WeMeetAgain,
                    choices: we_meet_again_choices(1, None),
                    stage: 1,
                    event_data: screen.event_data,
                });
            }
            0 if choice_index == 3 => {
                next.event = Some(EventScreen {
                    event: Event::WeMeetAgain,
                    choices: we_meet_again_choices(1, None),
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
        },
        Event::DeadAdventurer => match screen.stage {
            0 if choice_index == 1 => {
                next.event = Some(EventScreen {
                    event: Event::DeadAdventurer,
                    choices: dead_adventurer_choices(1),
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
                    "event choice is not implemented for Dead Adventurer",
                ));
            }
        },
        Event::TheCleric if screen.stage > 0 && choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::TheCleric if screen.stage == 0 && choice_index == 0 => {
            if next.gold < 35 {
                return Err(SimError::IllegalAction("not enough gold"));
            }
            next.gold -= 35;
            let heal = next.player_max_hp * 25 / 100;
            next.player_hp = (next.player_hp + heal).min(next.player_max_hp);
            next.event = Some(make_event_screen(
                Event::TheCleric,
                vec![EventChoice {
                    label: "Leave".to_owned(),
                }],
                1,
            ));
        }
        Event::TheCleric if screen.stage == 0 && choice_index == 1 => {
            if next.gold < 50 {
                return Err(SimError::IllegalAction("not enough gold"));
            }
            next.gold -= 50;
            open_event_remove_return_to_event_grid(&mut next, Event::TheCleric);
        }
        Event::TheCleric if screen.stage == 0 && choice_index == 2 => {
            next.phase = RunPhase::Idle;
            next.event = None;
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
        Event::ShiningLight if choice_index == 1 || screen.stage == 1 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::Purifier | Event::UpgradeShrine if choice_index == 0 => {
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
                next.phase = RunPhase::Idle;
                next.event = None;
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
                next.phase = RunPhase::Idle;
                next.event = None;
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
        Event::BigFish => match screen.stage {
            0 if choice_index == 0 => {
                let heal = next.player_max_hp / 3;
                next.player_hp = (next.player_hp + heal).min(next.player_max_hp);
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
                let act = i32::from(next.current_act);
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
                next.phase = RunPhase::Idle;
                next.event = None;
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
        Event::TheLibrary if choice_index == 1 => {
            let heal = the_library_heal_for_ascension(next.player_max_hp, next.ascension);
            next.player_hp = (next.player_hp + heal).min(next.player_max_hp);
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::TheLibrary if choice_index == 0 => {
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
            let act = i32::from(next.current_act);
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
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::Vampires if choice_index == 1 && screen.choices.len() == 3 => {
            if !next.relics.contains(&Relic::BloodVial) {
                return Err(SimError::IllegalAction(
                    "Blood Vial choice requires Blood Vial",
                ));
            }
            next.relics.retain(|relic| *relic != Relic::BloodVial);
            replace_starter_strikes_with_bites(&mut next);
            next.phase = RunPhase::Idle;
            next.event = None;
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
                next.phase = RunPhase::Idle;
                next.event = None;
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
                next.gain_deck_card(RITUAL_DAGGER_ID);
                next.event = Some(EventScreen {
                    event: Event::Nest,
                    choices: nest_choices(2, next.ascension),
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
                let act = i32::from(next.current_act);
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
                let act = i32::from(next.current_act);
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
            0 if choice_index == 0 => {
                give_forgotten_altar_idol(&mut next)?;
                next.event = Some(EventScreen {
                    event: Event::ForgottenAltar,
                    choices: forgotten_altar_choices(1, next.player_max_hp, next.ascension),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 if choice_index == 1 => {
                let hp_loss = forgotten_altar_hp_loss(next.player_max_hp, next.ascension);
                next.player_max_hp += FORGOTTEN_ALTAR_MAX_HP_GAIN;
                lose_event_hp(&mut next, hp_loss);
                next.event = Some(EventScreen {
                    event: Event::ForgottenAltar,
                    choices: forgotten_altar_choices(1, next.player_max_hp, next.ascension),
                    stage: 1,
                    event_data: hp_loss as u32,
                });
            }
            0 if choice_index == 2 => {
                next.gain_deck_card(DECAY_ID);
                next.event = Some(EventScreen {
                    event: Event::ForgottenAltar,
                    choices: forgotten_altar_choices(1, next.player_max_hp, next.ascension),
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
                    next.gain_deck_card(APPARITION_ID);
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
                open_event_transform_grid(&mut next, DRUG_DEALER_TRANSFORM_COUNT);
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
                        choices: Vec::new(),
                        gold_offer: 0,
                        stolen_gold_offer: 0,
                        potion_offer: None,
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
                    next.player_hp = next.player_max_hp;
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
            next.phase = RunPhase::Reward;
            next.event = None;
            next.reward = Some(RewardScreen {
                choices: Vec::new(),
                gold_offer: 0,
                stolen_gold_offer: 0,
                potion_offer: Some(Potion::Power),
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
            next.pending_event_combat_gold_offer = 50;
            next.pending_event_combat_relic_key_offer =
                Some(super::reward::roll_relic_reward(&mut next, RelicTier::Rare));
            enter_event_combat(&mut next, &[&SLIME_BOSS_A0]);
        }
        _ if choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        _ => {
            return Err(SimError::IllegalAction(
                "event choice is not implemented for this event",
            ));
        }
    }

    Ok(next)
}

fn enter_event_combat(run: &mut RunState, definitions: &[&MonsterDefinition]) {
    let mut shuffle_rng = StsRng::new(run.event_rng_seed as i64 + i64::from(run.current_floor));
    let mut monster_hp_rng = StsRng::new(run.event_rng_seed as i64 + i64::from(run.current_floor));
    let monster_rng = StsRng::new(run.monster_rng_seed as i64 + i64::from(run.current_floor));
    let mut card_random_rng = Some(run.card_random_rng());
    let mut combat = CombatState::initial_fixture();
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
}
