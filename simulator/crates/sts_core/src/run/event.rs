use crate::{
    content::cards::{
        upgrade_content_id, APPARITION_ID, BITE_ID, DECAY_ID, DEFEND_R_ID, DOUBT_ID, INJURY_ID,
        JAX_ID, REGRET_ID, RITUAL_DAGGER_ID, SHAME_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID, WRITHE_ID,
    },
    relic::{Relic, RelicKey},
    rng::{JavaRng, StsRng},
    run::{
        grid::{open_event_obtain_card_grid, open_event_remove_grid, open_event_transform_grid},
        neow::{
            apply_neow_boss_swap, apply_neow_lament_reward, apply_neow_relic_reward,
            apply_neow_simple_drawback, apply_neow_simple_reward, generate_neow_card_reward,
            generate_neow_options, generate_neow_three_potions, open_neow_reward_grid,
            GeneratedNeowOption, NeowDrawback, NeowRewardType,
        },
        reward::target_card_reward_choices_with_count,
        state::RunRngStream,
    },
    EventAction, RewardScreen, RunPhase, RunState, SimError, SimResult,
};

pub const SCRAP_OOZE_REACH_HP_LOSS: i32 = 3;
pub const SCRAP_OOZE_DEEPER_HP_LOSS: i32 = 4;
use serde::{Deserialize, Serialize};

pub const GOLDEN_SHRINE_GOLD: i32 = 100;
pub const WORLD_OF_GOOP_DAMAGE: i32 = 11;
pub const WORLD_OF_GOOP_GOLD: i32 = 75;
pub const WORLD_OF_GOOP_MIN_GOLD_LOSS: i32 = 20;
pub const WORLD_OF_GOOP_MAX_GOLD_LOSS: i32 = 50;
pub const WORLD_OF_GOOP_A15_MIN_GOLD_LOSS: i32 = 35;
pub const WORLD_OF_GOOP_A15_MAX_GOLD_LOSS: i32 = 75;
pub const GOLDEN_IDOL_HP_LOSS_PERCENT: f32 = 0.25;
pub const GOLDEN_IDOL_MAX_HP_LOSS_PERCENT: f32 = 0.08;
pub const GOLDEN_IDOL_A15_HP_LOSS_PERCENT: f32 = 0.35;
pub const GOLDEN_IDOL_A15_MAX_HP_LOSS_PERCENT: f32 = 0.10;
pub const SSSSSERPENT_GOLD: i32 = 175;
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
        let upgraded_content_id = upgrade_content_id(run.deck[index].content_id)
            .expect("upgradeable card validated before shuffle");
        run.deck[index].content_id = upgraded_content_id;
    }
}

fn upgrade_starter_strikes_and_defends(run: &mut RunState) {
    for card in &mut run.deck {
        if matches!(card.content_id, STRIKE_R_ID | DEFEND_R_ID) {
            if let Some(upgraded_content_id) = upgrade_content_id(card.content_id) {
                card.content_id = upgraded_content_id;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Neow,
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
    MaskedBandits,
    Nest,
    TheLibrary,
    TheMausoleum,
    Vampires,
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

fn event_lists_mut(run: &mut RunState) -> (&mut Vec<Event>, &mut Vec<Event>) {
    if run.current_act == 2 {
        (&mut run.act2_event_list, &mut run.act2_shrine_list)
    } else {
        (&mut run.act1_event_list, &mut run.act1_shrine_list)
    }
}

fn ensure_event_lists(run: &mut RunState) {
    if run.current_act == 2 {
        initialize_act2_event_pools(run);
    } else {
        initialize_act1_event_pools(run);
    }
}

fn pick_from_list(rng: &mut StsRng, list: &mut Vec<Event>) -> Event {
    let idx = rng.random_int((list.len() - 1) as i32) as usize;
    list.remove(idx)
}

fn get_shrine(run: &mut RunState, rng: &mut StsRng) -> Event {
    let (event_list, shrine_list) = event_lists_mut(run);
    let mut candidates = shrine_list.clone();
    if candidates.is_empty() {
        return pick_from_list(rng, event_list);
    }
    let event = pick_from_list(rng, &mut candidates);
    *shrine_list = candidates;
    event
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
    run.store_rng_counter(RunRngStream::Event, &rng);
    run.phase = RunPhase::Event;
    run.event = Some(entered_event_screen_for_run(run, event));
}

#[must_use]
pub fn event_screen(event: Event) -> EventScreen {
    match event {
        Event::Neow => make_event_screen(event, neow_talk_choices(), 0),
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
                    label: "Remove Curse".to_owned(),
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
        Event::BigFish => make_event_screen(event, big_fish_choices(0), 0),
        Event::GoldenIdol => make_event_screen(event, golden_idol_choices(0, 0, 0), 0),
        Event::WorldOfGoop => make_event_screen(event, world_of_goop_choices(0, 0), 0),
        Event::DeadAdventurer => make_event_screen(event, dead_adventurer_choices(0), 0),
        Event::TheSsssserpent => make_event_screen(event, sssssserpent_choices(0), 0),
        Event::BackToBasics => {
            make_event_screen(event, labeled_choices(&["Elegance", "Simplicity"]), 0)
        }
        Event::TheLibrary => make_event_screen(event, labeled_choices(&["Read", "Sleep"]), 0),
        Event::TheMausoleum => {
            make_event_screen(event, labeled_choices(&["Open the coffin", "Leave"]), 0)
        }
        Event::Vampires => make_event_screen(event, vampires_choices(false), 0),
        Event::CursedTome => make_event_screen(event, cursed_tome_choices(0, 0), 0),
        Event::Nest => make_event_screen(event, nest_choices(0, 0), 0),
        Event::Beggar => make_event_screen(event, beggar_choices(0), 0),
        Event::Addict => make_event_screen(event, addict_choices(0), 0),
        Event::ForgottenAltar => make_event_screen(event, forgotten_altar_choices(0, 0, 0), 0),
        Event::Ghosts => make_event_screen(event, ghosts_choices(0, 0), 0),
        Event::MaskedBandits => make_event_screen(event, masked_bandits_choices(0), 0),
        Event::Colosseum => make_event_screen(event, colosseum_choices(0), 0),
        Event::DrugDealer => make_event_screen(event, drug_dealer_choices(0, false), 0),
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
        Event::Vampires => make_event_screen(
            event,
            vampires_choices(run.relics.contains(&Relic::BloodVial)),
            0,
        ),
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
            return Err(SimError::IllegalAction(
                "Neow curse drawback is not implemented in event replay",
            ));
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
                if next.can_gain_potions() && next.potions.len() < next.potion_capacity() {
                    next.potions.push(potion);
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
        NeowRewardType::ThreeCards
        | NeowRewardType::RandomColorless
        | NeowRewardType::RandomColorlessTwo
        | NeowRewardType::ThreeRareCards => {
            return Err(SimError::IllegalAction(
                "Neow choice-card reward is not implemented in event replay",
            ));
        }
    }

    next.event = Some(make_event_screen(Event::Neow, neow_leave_choices(), 2));
    Ok(())
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
        Event::GoldenShrine if choice_index == 0 => {
            next.gain_gold(GOLDEN_SHRINE_GOLD);
            next.phase = RunPhase::Idle;
            next.event = None;
        }
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
                next.gain_deck_card(INJURY_ID);
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
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Golden Idol",
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
        Event::TheCleric if choice_index == 0 => {
            let heal = next.player_max_hp * 25 / 100;
            next.player_hp = (next.player_hp + heal).min(next.player_max_hp);
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::ShiningLight if choice_index == 0 => {
            let loss = shining_light_hp_loss(next.player_max_hp);
            next.player_hp = (next.player_hp - loss).max(0);
            upgrade_random_deck_cards(&mut next, 2);
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::ShiningLight if choice_index == 1 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::Purifier | Event::UpgradeShrine if choice_index == 0 => {
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::ScrapOoze => match screen.stage {
            0 if choice_index == 0 => {
                next.player_hp = (next.player_hp - SCRAP_OOZE_REACH_HP_LOSS).max(0);
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
                next.player_hp = (next.player_hp - SCRAP_OOZE_DEEPER_HP_LOSS).max(0);
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
        Event::BigFish => match screen.stage {
            0 if choice_index == 2 => {
                let act = i32::from(next.current_act);
                let key = super::reward::roll_event_relic_reward(&mut next, act);
                next.gain_relic_key(key);
                next.event = Some(EventScreen {
                    event: Event::BigFish,
                    choices: big_fish_choices(1),
                    stage: 1,
                    event_data: 0,
                });
            }
            0 => {
                return Err(SimError::IllegalAction(
                    "only the Big Fish box choice is implemented",
                ));
            }
            1 if choice_index == 0 => {
                next.gain_deck_card(REGRET_ID);
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
        Event::BackToBasics if choice_index == 1 => {
            upgrade_starter_strikes_and_defends(&mut next);
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::BackToBasics if choice_index == 0 => {
            open_event_remove_grid(&mut next);
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
            next.phase = RunPhase::Idle;
            next.event = None;
        }
        Event::TheMausoleum if choice_index == 0 => {
            if roll_mausoleum_curses_player(&mut next) {
                next.gain_deck_card(WRITHE_ID);
            }
            let act = i32::from(next.current_act);
            let key = super::reward::roll_event_relic_reward(&mut next, act);
            next.gain_relic_key(key);
            next.phase = RunPhase::Idle;
            next.event = None;
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
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            1 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            _ => {
                return Err(SimError::IllegalAction(
                    "event choice is not implemented for Ghosts",
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
                return Err(SimError::IllegalAction(
                    "Masked Bandits fight branch is not implemented",
                ));
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
                return Err(SimError::IllegalAction(
                    "Colosseum Slavers combat branch is not implemented",
                ));
            }
            2 if choice_index == 0 => {
                next.phase = RunPhase::Idle;
                next.event = None;
            }
            2 if choice_index == 1 => {
                return Err(SimError::IllegalAction(
                    "Colosseum Nobs combat branch is not implemented",
                ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relic::Relic;

    #[test]
    fn fixed_event_screen_exposes_golden_shrine_choice() {
        let event = fixed_event_screen();

        assert_eq!(event.event, Event::GoldenShrine);
        assert_eq!(event.choices.len(), 1);
        assert_eq!(event.choices[0].label, "Pray");
    }

    #[test]
    fn golden_idol_take_then_injury_branch_adds_relic_and_curse_before_leave() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::GoldenIdol));

        let after_take =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("take idol");
        assert!(after_take.relics.contains(&Relic::GoldenIdol));
        assert_eq!(after_take.phase, RunPhase::Event);
        assert_eq!(after_take.event.as_ref().expect("boulder").stage, 1);
        assert_eq!(after_take.event.as_ref().expect("boulder").choices.len(), 3);

        let after_injury = apply_event_action(&after_take, EventAction::Choose { choice_index: 0 })
            .expect("take injury");
        assert!(after_injury
            .deck
            .iter()
            .any(|card| card.content_id == INJURY_ID));
        assert_eq!(after_injury.phase, RunPhase::Event);
        assert_eq!(after_injury.event.as_ref().expect("leave").stage, 2);

        let after_leave =
            apply_event_action(&after_injury, EventAction::Choose { choice_index: 0 })
                .expect("leave");
        assert_eq!(after_leave.phase, RunPhase::Idle);
        assert!(after_leave.event.is_none());
    }

    #[test]
    fn event_screen_selection_is_deterministic_for_seed() {
        let mut first = RunState::map_fixture();
        let mut second = RunState::map_fixture();
        first.event_rng_seed = 7;
        second.event_rng_seed = 7;

        enter_event_screen(&mut first);
        enter_event_screen(&mut second);

        assert_eq!(first.event, second.event);
        assert_eq!(first.event_rng_counter, second.event_rng_counter);
        assert_eq!(first.act1_event_list, second.act1_event_list);
    }

    #[test]
    fn event_screen_selection_uses_temporary_event_rng_counter() {
        let mut run = RunState::map_fixture();
        run.event_rng_seed = 7;
        run.event_rng_counter = 4;

        enter_event_screen(&mut run);

        assert_eq!(run.event_rng_counter, 4);
        assert!(
            run.act1_event_list.len() < ACT1_EVENTS.len()
                || run.act1_shrine_list.len() < ACT1_SHRINES.len()
        );
    }

    #[test]
    fn act1_event_pool_removes_selected_events() {
        let mut run = RunState::map_fixture();
        run.event_rng_seed = 22_079_335_079;

        enter_event_screen(&mut run);

        assert!(
            run.act1_event_list.len() + run.act1_shrine_list.len()
                < ACT1_EVENTS.len() + ACT1_SHRINES.len()
        );
    }

    #[test]
    fn city_event_pools_match_target_the_city_source() {
        assert_eq!(
            ACT2_EVENTS,
            [
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
            ]
        );
        assert_eq!(
            ACT2_SHRINES,
            [
                Event::MatchAndKeep,
                Event::WheelOfChange,
                Event::GoldenShrine,
                Event::Transmorgrifier,
                Event::Purifier,
                Event::UpgradeShrine,
            ]
        );
    }

    #[test]
    fn act2_event_screen_selection_uses_city_pools_without_touching_act1_pools() {
        let mut run = RunState::map_fixture();
        run.current_act = 2;
        run.event_rng_seed = 22_079_335_079;

        enter_event_screen(&mut run);

        let selected = run.event.as_ref().expect("event").event;
        let mut city_events_and_shrines = ACT2_EVENTS.to_vec();
        city_events_and_shrines.extend(ACT2_SHRINES);
        assert!(city_events_and_shrines.contains(&selected));
        assert!(run.act1_event_list.is_empty());
        assert!(run.act1_shrine_list.is_empty());
        assert!(
            run.act2_event_list.len() + run.act2_shrine_list.len()
                < ACT2_EVENTS.len() + ACT2_SHRINES.len()
        );
    }

    #[test]
    fn act2_unimplemented_shrine_bodies_are_continue_only_scaffold() {
        let screen = event_screen(Event::MatchAndKeep);

        assert_eq!(screen.event, Event::MatchAndKeep);
        assert_eq!(screen.choices.len(), 1);
        assert_eq!(screen.choices[0].label, "Continue");
    }

    #[test]
    fn act2_simple_event_screens_expose_source_backed_initial_choices() {
        assert_eq!(
            event_screen(Event::BackToBasics)
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Elegance", "Simplicity"]
        );
        assert_eq!(
            event_screen(Event::TheLibrary)
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Read", "Sleep"]
        );
        assert_eq!(
            event_screen(Event::TheMausoleum)
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Open the coffin", "Leave"]
        );
        assert_eq!(
            event_screen(Event::Vampires)
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Accept", "Leave"]
        );
        assert_eq!(
            event_screen(Event::CursedTome)
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Read", "Leave"]
        );
        assert_eq!(
            event_screen(Event::MaskedBandits)
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Pay", "Fight"]
        );
        assert_eq!(
            event_screen(Event::Colosseum)
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Continue"]
        );
    }

    #[test]
    fn dead_adventurer_leave_path_matches_trace_choice_shape() {
        let screen = event_screen(Event::DeadAdventurer);
        assert_eq!(
            screen
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Search", "Leave"]
        );

        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(screen);

        let leave_prompt = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect("leave prompt");
        assert_eq!(leave_prompt.phase, RunPhase::Event);
        assert_eq!(
            leave_prompt
                .event
                .as_ref()
                .expect("leave screen")
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Leave"]
        );

        let done = apply_event_action(&leave_prompt, EventAction::Choose { choice_index: 0 })
            .expect("leave event");
        assert_eq!(done.phase, RunPhase::Idle);
        assert!(done.event.is_none());
    }

    #[test]
    fn back_to_basics_simplicity_upgrades_starter_strikes_and_defends() {
        use crate::content::cards::{ANGER_ID, DEFEND_R_PLUS_ID, STRIKE_R_PLUS_ID};
        use crate::CardId;
        use crate::CardInstance;

        let mut run = RunState::map_fixture();
        run.deck = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), ANGER_ID),
        ];
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::BackToBasics));

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 1 }).expect("simplicity");

        assert_eq!(after.deck[0].content_id, STRIKE_R_PLUS_ID);
        assert_eq!(after.deck[1].content_id, DEFEND_R_PLUS_ID);
        assert_eq!(after.deck[2].content_id, ANGER_ID);
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
    }

    #[test]
    fn the_library_sleep_heals_one_third_max_hp_and_exits() {
        let mut run = RunState::map_fixture();
        run.player_max_hp = 80;
        run.player_hp = 40;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::TheLibrary));

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 1 }).expect("sleep");

        assert_eq!(the_library_heal(80), 26);
        assert_eq!(after.player_hp, 66);
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
    }

    #[test]
    fn the_library_sleep_heals_one_fifth_at_a15() {
        let mut run = RunState::map_fixture();
        run.ascension = 15;
        run.player_max_hp = 80;
        run.player_hp = 40;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::TheLibrary));

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 1 }).expect("sleep");

        assert_eq!(the_library_heal_for_ascension(80, 15), 16);
        assert_eq!(after.player_hp, 56);
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
    }

    #[test]
    fn the_library_read_opens_twenty_card_grid_and_obtains_selected_card() {
        use crate::run::grid::{confirm_grid, select_grid_card, GridPurpose};

        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::TheLibrary));
        let deck_before = run.deck.clone();
        let card_rng_counter_before = run.card_rng_counter;

        let grid_run =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("read");

        let grid = grid_run.card_grid.as_ref().expect("library grid");
        assert_eq!(grid.purpose, GridPurpose::EventObtainCard);
        assert_eq!(grid.cards.len(), THE_LIBRARY_READ_CARD_COUNT);
        assert!(grid
            .cards
            .iter()
            .enumerate()
            .all(|(index, card)| !grid.cards[..index]
                .iter()
                .any(|other| other.content_id == card.content_id)));
        assert!(grid_run.card_rng_counter > card_rng_counter_before);
        assert_eq!(grid_run.phase, RunPhase::Event);
        assert_eq!(grid_run.deck, deck_before);

        let chosen = grid.cards[0];
        let selected = select_grid_card(&grid_run, 0).expect("select book card");
        let after = confirm_grid(&selected).expect("confirm book card");

        assert_eq!(after.deck.len(), deck_before.len() + 1);
        assert_eq!(after.deck.last().copied(), Some(chosen));
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
        assert!(after.card_grid.is_none());
    }

    #[test]
    fn mausoleum_open_grants_event_relic_and_rolls_writhe_below_a15() {
        let mut run = RunState::map_fixture();
        run.current_act = 2;
        run.ascension = 0;
        run.misc_rng_seed = 1_957_307_888_551;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::TheMausoleum));
        let misc_counter_before = run.misc_rng_counter;
        let relic_count_before = run.relics.len() + run.relic_keys.len();

        let expected_cursed = {
            let mut rng = StsRng::with_counter(run.misc_rng_seed as i64, run.misc_rng_counter);
            rng.random_bool()
        };

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("open coffin");

        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
        assert_eq!(after.misc_rng_counter, misc_counter_before + 1);
        assert_eq!(
            after.relics.len() + after.relic_keys.len(),
            relic_count_before + 1
        );
        assert_eq!(
            after.deck.iter().any(|card| card.content_id == WRITHE_ID),
            expected_cursed
        );
    }

    #[test]
    fn mausoleum_open_always_adds_writhe_at_a15() {
        let mut run = RunState::map_fixture();
        run.current_act = 2;
        run.ascension = 15;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::TheMausoleum));
        let misc_counter_before = run.misc_rng_counter;

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("open coffin");

        assert!(after.deck.iter().any(|card| card.content_id == WRITHE_ID));
        assert_eq!(after.misc_rng_counter, misc_counter_before);
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
    }

    #[test]
    fn back_to_basics_elegance_opens_card_removal_grid_and_removes_selected_card() {
        use crate::content::cards::ANGER_ID;
        use crate::run::grid::{confirm_grid, select_grid_card, GridPurpose};
        use crate::CardId;
        use crate::CardInstance;

        let mut run = RunState::map_fixture();
        run.deck = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), ANGER_ID),
        ];
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::BackToBasics));

        let grid_run =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("elegance");

        assert_eq!(grid_run.phase, RunPhase::Event);
        assert_eq!(
            grid_run.card_grid.as_ref().expect("remove grid").purpose,
            GridPurpose::EventRemove
        );
        assert_eq!(
            grid_run.card_grid.as_ref().expect("remove grid").cards,
            run.deck
        );

        let selected = select_grid_card(&grid_run, 0).expect("select strike");
        let after = confirm_grid(&selected).expect("confirm removal");

        assert_eq!(
            after.deck,
            vec![CardInstance::new(CardId::new(2), ANGER_ID)]
        );
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
        assert!(after.card_grid.is_none());
    }

    #[test]
    fn back_to_basics_elegance_exits_when_no_purgeable_cards() {
        use crate::content::cards::ANGER_ID;
        use crate::CardId;
        use crate::CardInstance;

        let mut run = RunState::map_fixture();
        let mut bottled = CardInstance::new(CardId::new(1), ANGER_ID);
        bottled.bottled = true;
        run.deck = vec![bottled];
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::BackToBasics));

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("elegance");

        assert_eq!(after.deck, vec![bottled]);
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
        assert!(after.card_grid.is_none());
    }

    #[test]
    fn vampires_accept_loses_max_hp_replaces_starter_strikes_with_bites_and_exits() {
        use crate::content::cards::{ANGER_ID, DEFEND_R_PLUS_ID};
        use crate::CardId;
        use crate::CardInstance;

        let mut run = RunState::map_fixture();
        run.player_max_hp = 80;
        run.player_hp = 79;
        run.deck = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_PLUS_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_PLUS_ID),
            CardInstance::new(CardId::new(4), ANGER_ID),
        ];
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Vampires));

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("accept");

        assert_eq!(vampires_max_hp_loss(80), 24);
        assert_eq!(after.player_max_hp, 56);
        assert_eq!(after.player_hp, 56);
        assert!(!after
            .deck
            .iter()
            .any(|card| matches!(card.content_id, STRIKE_R_ID | STRIKE_R_PLUS_ID)));
        assert_eq!(
            after
                .deck
                .iter()
                .filter(|card| card.content_id == BITE_ID)
                .count(),
            VAMPIRES_BITE_COUNT
        );
        assert!(after
            .deck
            .iter()
            .any(|card| card.content_id == DEFEND_R_PLUS_ID));
        assert!(after.deck.iter().any(|card| card.content_id == ANGER_ID));
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
    }

    #[test]
    fn vampires_with_blood_vial_exposes_vial_choice() {
        let mut run = RunState::map_fixture();
        run.relics.push(Relic::BloodVial);

        let screen = event_screen_for_run(&run, Event::Vampires);

        assert_eq!(
            screen
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Accept", "Give Blood Vial", "Leave"]
        );
    }

    #[test]
    fn vampires_blood_vial_choice_replaces_strikes_without_max_hp_loss() {
        use crate::content::cards::{ANGER_ID, DEFEND_R_PLUS_ID};
        use crate::CardId;
        use crate::CardInstance;

        let mut run = RunState::map_fixture();
        run.player_max_hp = 80;
        run.player_hp = 79;
        run.relics.push(Relic::BloodVial);
        run.deck = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_PLUS_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_PLUS_ID),
            CardInstance::new(CardId::new(4), ANGER_ID),
        ];
        run.phase = RunPhase::Event;
        run.event = Some(event_screen_for_run(&run, Event::Vampires));

        let after = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect("give Blood Vial");

        assert_eq!(after.player_max_hp, 80);
        assert_eq!(after.player_hp, 79);
        assert!(!after.relics.contains(&Relic::BloodVial));
        assert!(!after
            .deck
            .iter()
            .any(|card| matches!(card.content_id, STRIKE_R_ID | STRIKE_R_PLUS_ID)));
        assert_eq!(
            after
                .deck
                .iter()
                .filter(|card| card.content_id == BITE_ID)
                .count(),
            VAMPIRES_BITE_COUNT
        );
        assert!(after
            .deck
            .iter()
            .any(|card| card.content_id == DEFEND_R_PLUS_ID));
        assert!(after.deck.iter().any(|card| card.content_id == ANGER_ID));
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
    }

    #[test]
    fn vampires_with_blood_vial_leave_uses_last_choice() {
        let mut run = RunState::map_fixture();
        run.relics.push(Relic::BloodVial);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen_for_run(&run, Event::Vampires));
        let hp_before = run.player_hp;
        let max_hp_before = run.player_max_hp;
        let deck_before = run.deck.clone();

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 2 }).expect("leave");

        assert_eq!(after.player_hp, hp_before);
        assert_eq!(after.player_max_hp, max_hp_before);
        assert_eq!(after.deck, deck_before);
        assert!(after.relics.contains(&Relic::BloodVial));
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
    }

    #[test]
    fn vampires_max_hp_loss_keeps_at_least_one_max_hp() {
        let mut run = RunState::map_fixture();
        run.player_max_hp = 1;
        run.player_hp = 1;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Vampires));

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("accept");

        assert_eq!(vampires_max_hp_loss(1), 0);
        assert_eq!(after.player_max_hp, 1);
        assert_eq!(after.player_hp, 1);
    }

    #[test]
    fn act2_city_event_leave_choices_exit_without_changes() {
        for event in [Event::TheMausoleum, Event::Vampires, Event::CursedTome] {
            let mut run = RunState::map_fixture();
            run.phase = RunPhase::Event;
            run.event = Some(event_screen(event));
            let hp_before = run.player_hp;
            let deck_before = run.deck.clone();

            let after =
                apply_event_action(&run, EventAction::Choose { choice_index: 1 }).expect("leave");

            assert_eq!(after.player_hp, hp_before);
            assert_eq!(after.deck, deck_before);
            assert_eq!(after.phase, RunPhase::Idle);
            assert!(after.event.is_none());
        }
    }

    #[test]
    fn cursed_tome_read_path_takes_page_damage_and_opens_book_reward() {
        let mut run = RunState::map_fixture();
        run.player_hp = 80;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::CursedTome));

        let page_1 =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("read");
        assert_eq!(page_1.player_hp, 80);
        assert_eq!(page_1.event.as_ref().expect("page 1").stage, 1);

        let page_2 =
            apply_event_action(&page_1, EventAction::Choose { choice_index: 0 }).expect("page 2");
        assert_eq!(page_2.player_hp, 79);
        assert_eq!(page_2.event.as_ref().expect("page 2").stage, 2);
        assert_eq!(page_2.event.as_ref().expect("page 2").event_data, 1);

        let page_3 =
            apply_event_action(&page_2, EventAction::Choose { choice_index: 0 }).expect("page 3");
        assert_eq!(page_3.player_hp, 77);
        assert_eq!(page_3.event.as_ref().expect("page 3").stage, 3);
        assert_eq!(page_3.event.as_ref().expect("page 3").event_data, 3);

        let last_page =
            apply_event_action(&page_3, EventAction::Choose { choice_index: 0 }).expect("last");
        assert_eq!(last_page.player_hp, 74);
        assert_eq!(last_page.event.as_ref().expect("last page").stage, 4);
        assert_eq!(last_page.event.as_ref().expect("last page").event_data, 6);
        assert_eq!(
            last_page.event.as_ref().expect("last page").choices[0].label,
            "Take the book (lose 10 HP)"
        );

        let reward =
            apply_event_action(&last_page, EventAction::Choose { choice_index: 0 }).expect("take");
        let reward_screen = reward.reward.as_ref().expect("book reward");
        let offered_key = reward_screen
            .relic_key_offer
            .or_else(|| reward_screen.relic_offer.map(|relic| relic.key()))
            .expect("offered book");

        assert_eq!(reward.player_hp, 64);
        assert_eq!(reward.phase, RunPhase::Reward);
        assert!(reward.event.is_none());
        assert!([
            RelicKey::Necronomicon,
            RelicKey::Enchiridion,
            RelicKey::NilrysCodex,
        ]
        .contains(&offered_key));
    }

    #[test]
    fn cursed_tome_final_damage_is_fifteen_at_a15() {
        let mut run = RunState::map_fixture();
        run.ascension = 15;
        run.player_hp = 80;
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::CursedTome,
            choices: cursed_tome_choices(4, run.ascension),
            stage: 4,
            event_data: 6,
        });

        let reward =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("take");

        assert_eq!(cursed_tome_final_hp_loss(15), 15);
        assert_eq!(reward.player_hp, 65);
        assert_eq!(reward.phase, RunPhase::Reward);
    }

    #[test]
    fn cursed_tome_stop_on_last_page_takes_three_and_leaves_after_end_screen() {
        let mut run = RunState::map_fixture();
        run.player_hp = 80;
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::CursedTome,
            choices: cursed_tome_choices(4, run.ascension),
            stage: 4,
            event_data: 6,
        });

        let end = apply_event_action(&run, EventAction::Choose { choice_index: 1 }).expect("stop");
        assert_eq!(end.player_hp, 77);
        assert_eq!(end.phase, RunPhase::Event);
        assert_eq!(end.event.as_ref().expect("end").stage, 5);
        assert!(end.reward.is_none());

        let after =
            apply_event_action(&end, EventAction::Choose { choice_index: 0 }).expect("leave");
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
        assert!(after.reward.is_none());
    }

    #[test]
    fn cursed_tome_book_falls_back_to_circlet_when_all_books_are_owned() {
        let mut run = RunState::map_fixture();
        run.player_hp = 80;
        run.relic_keys = vec![
            RelicKey::Necronomicon,
            RelicKey::Enchiridion,
            RelicKey::NilrysCodex,
        ];
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::CursedTome,
            choices: cursed_tome_choices(4, run.ascension),
            stage: 4,
            event_data: 6,
        });

        let reward =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("take");
        let reward_screen = reward.reward.as_ref().expect("book reward");

        assert_eq!(reward_screen.relic_offer, Some(Relic::Circlet));
        assert_eq!(reward_screen.relic_key_offer, None);
    }

    #[test]
    fn nest_continue_reveals_gold_or_ritual_dagger_choices() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Nest));

        let next =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("continue");
        let event = next.event.as_ref().expect("nest choice");

        assert_eq!(event.stage, 1);
        assert_eq!(event.choices[0].label, "Smash and grab (gain 99 gold)");
        assert_eq!(event.choices[1].label, "Stay in line (lose 6 HP)");
    }

    #[test]
    fn nest_smash_and_grab_gains_source_backed_gold_and_then_leaves() {
        let mut run = RunState::map_fixture();
        run.gold = 10;
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::Nest,
            choices: nest_choices(1, run.ascension),
            stage: 1,
            event_data: 0,
        });

        let after_gold =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("steal");
        assert_eq!(after_gold.gold, 10 + NEST_GOLD_GAIN);
        assert_eq!(after_gold.event.as_ref().expect("leave").stage, 2);

        let after_leave = apply_event_action(&after_gold, EventAction::Choose { choice_index: 0 })
            .expect("leave");
        assert_eq!(after_leave.phase, RunPhase::Idle);
        assert!(after_leave.event.is_none());
    }

    #[test]
    fn nest_smash_and_grab_gains_fifty_gold_at_a15() {
        let mut run = RunState::map_fixture();
        run.ascension = 15;
        run.gold = 10;
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::Nest,
            choices: nest_choices(1, run.ascension),
            stage: 1,
            event_data: 0,
        });

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("steal");

        assert_eq!(nest_gold_gain(15), NEST_A15_GOLD_GAIN);
        assert_eq!(after.gold, 10 + NEST_A15_GOLD_GAIN);
    }

    #[test]
    fn nest_stay_in_line_loses_hp_and_obtains_ritual_dagger() {
        let mut run = RunState::map_fixture();
        run.player_hp = 50;
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::Nest,
            choices: nest_choices(1, run.ascension),
            stage: 1,
            event_data: 0,
        });
        let deck_len = run.deck.len();

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 1 }).expect("join");

        assert_eq!(after.player_hp, 50 - NEST_HP_LOSS);
        assert_eq!(after.deck.len(), deck_len + 1);
        assert_eq!(
            after.deck.last().expect("ritual dagger").content_id,
            RITUAL_DAGGER_ID
        );
        assert_eq!(after.event.as_ref().expect("leave").stage, 2);
    }

    #[test]
    fn beggar_pay_spends_gold_then_opens_removal_grid_and_removes_selected_card() {
        use crate::content::cards::ANGER_ID;
        use crate::run::grid::{confirm_grid, select_grid_card, GridPurpose};
        use crate::{CardId, CardInstance};

        let mut run = RunState::map_fixture();
        run.gold = 100;
        run.deck = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), ANGER_ID),
        ];
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Beggar));

        let paid = apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("pay");
        assert_eq!(paid.gold, 100 - BEGGAR_GOLD_COST);
        assert_eq!(paid.phase, RunPhase::Event);
        assert_eq!(paid.event.as_ref().expect("gave money").stage, 1);

        let grid_run =
            apply_event_action(&paid, EventAction::Choose { choice_index: 0 }).expect("choose");
        assert_eq!(
            grid_run.card_grid.as_ref().expect("remove grid").purpose,
            GridPurpose::EventRemove
        );

        let selected = select_grid_card(&grid_run, 0).expect("select strike");
        let after = confirm_grid(&selected).expect("confirm removal");

        assert_eq!(
            after.deck,
            vec![CardInstance::new(CardId::new(2), ANGER_ID)]
        );
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
    }

    #[test]
    fn beggar_rejects_payment_without_enough_gold() {
        let mut run = RunState::map_fixture();
        run.gold = BEGGAR_GOLD_COST - 1;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Beggar));

        let err = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect_err("cannot afford");

        assert_eq!(err, SimError::IllegalAction("not enough gold"));
    }

    #[test]
    fn beggar_leave_exits_without_changes() {
        let mut run = RunState::map_fixture();
        run.gold = 100;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Beggar));
        let deck_before = run.deck.clone();

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 1 }).expect("leave");

        assert_eq!(after.gold, 100);
        assert_eq!(after.deck, deck_before);
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
    }

    #[test]
    fn addict_buy_spends_gold_grants_event_relic_and_then_leaves() {
        let mut run = RunState::map_fixture();
        run.current_act = 2;
        run.gold = 100;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Addict));
        let relic_count_before = run.relics.len() + run.relic_keys.len();

        let after_buy =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("buy relic");

        assert_eq!(after_buy.gold, 100 - ADDICT_GOLD_COST);
        assert_eq!(
            after_buy.relics.len() + after_buy.relic_keys.len(),
            relic_count_before + 1
        );
        assert_eq!(after_buy.phase, RunPhase::Event);
        assert_eq!(after_buy.event.as_ref().expect("leave").stage, 1);

        let after_leave =
            apply_event_action(&after_buy, EventAction::Choose { choice_index: 0 }).expect("leave");
        assert_eq!(after_leave.phase, RunPhase::Idle);
        assert!(after_leave.event.is_none());
    }

    #[test]
    fn addict_buy_rejects_insufficient_gold() {
        let mut run = RunState::map_fixture();
        run.gold = ADDICT_GOLD_COST - 1;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Addict));

        let err = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect_err("cannot afford");

        assert_eq!(err, SimError::IllegalAction("not enough gold"));
    }

    #[test]
    fn addict_steal_adds_shame_grants_event_relic_and_then_leaves() {
        let mut run = RunState::map_fixture();
        run.current_act = 2;
        run.gold = 0;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Addict));
        let relic_count_before = run.relics.len() + run.relic_keys.len();
        let deck_len_before = run.deck.len();

        let after_steal =
            apply_event_action(&run, EventAction::Choose { choice_index: 1 }).expect("steal relic");

        assert_eq!(after_steal.gold, 0);
        assert_eq!(after_steal.deck.len(), deck_len_before + 1);
        assert_eq!(after_steal.deck.last().expect("shame").content_id, SHAME_ID);
        assert_eq!(
            after_steal.relics.len() + after_steal.relic_keys.len(),
            relic_count_before + 1
        );
        assert_eq!(after_steal.event.as_ref().expect("leave").stage, 1);

        let after_leave = apply_event_action(&after_steal, EventAction::Choose { choice_index: 0 })
            .expect("leave");
        assert_eq!(after_leave.phase, RunPhase::Idle);
        assert!(after_leave.event.is_none());
    }

    #[test]
    fn addict_leave_exits_without_changes() {
        let mut run = RunState::map_fixture();
        run.gold = 100;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Addict));
        let deck_before = run.deck.clone();
        let relics_before = run.relics.clone();
        let relic_keys_before = run.relic_keys.clone();

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 2 }).expect("leave");

        assert_eq!(after.gold, 100);
        assert_eq!(after.deck, deck_before);
        assert_eq!(after.relics, relics_before);
        assert_eq!(after.relic_keys, relic_keys_before);
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
    }

    #[test]
    fn forgotten_altar_shed_blood_uses_entry_max_hp_loss_then_gains_max_hp() {
        let mut run = RunState::map_fixture();
        run.player_max_hp = 70;
        run.player_hp = 50;
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::ForgottenAltar,
            choices: forgotten_altar_choices(0, run.player_max_hp, run.ascension),
            stage: 0,
            event_data: 0,
        });

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 1 }).expect("shed blood");

        assert_eq!(forgotten_altar_hp_loss(70, 0), 18);
        assert_eq!(after.player_max_hp, 75);
        assert_eq!(after.player_hp, 32);
        assert_eq!(after.event.as_ref().expect("leave").stage, 1);
        assert_eq!(after.event.as_ref().expect("leave").event_data, 18);
    }

    #[test]
    fn forgotten_altar_shed_blood_uses_thirty_five_percent_at_a15() {
        let mut run = RunState::map_fixture();
        run.ascension = 15;
        run.player_max_hp = 70;
        run.player_hp = 50;
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::ForgottenAltar,
            choices: forgotten_altar_choices(0, run.player_max_hp, run.ascension),
            stage: 0,
            event_data: 0,
        });

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 1 }).expect("shed blood");

        assert_eq!(forgotten_altar_hp_loss(70, 15), 25);
        assert_eq!(after.player_max_hp, 75);
        assert_eq!(after.player_hp, 25);
    }

    #[test]
    fn forgotten_altar_smash_adds_decay_and_then_leaves() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::ForgottenAltar));
        let deck_len_before = run.deck.len();

        let after_smash =
            apply_event_action(&run, EventAction::Choose { choice_index: 2 }).expect("smash");

        assert_eq!(after_smash.deck.len(), deck_len_before + 1);
        assert_eq!(after_smash.deck.last().expect("decay").content_id, DECAY_ID);
        assert_eq!(after_smash.event.as_ref().expect("leave").stage, 1);

        let after_leave = apply_event_action(&after_smash, EventAction::Choose { choice_index: 0 })
            .expect("leave");
        assert_eq!(after_leave.phase, RunPhase::Idle);
        assert!(after_leave.event.is_none());
    }

    #[test]
    fn forgotten_altar_give_idol_swaps_golden_idol_for_bloody_idol() {
        let mut run = RunState::map_fixture();
        run.relics.push(Relic::GoldenIdol);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::ForgottenAltar));

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("give idol");

        assert!(!after.relics.contains(&Relic::GoldenIdol));
        assert!(after.relics.contains(&Relic::BloodyIdol));
        assert_eq!(after.event.as_ref().expect("leave").stage, 1);
    }

    #[test]
    fn forgotten_altar_give_idol_gives_circlet_when_bloody_idol_owned() {
        let mut run = RunState::map_fixture();
        run.relics.push(Relic::GoldenIdol);
        run.relics.push(Relic::BloodyIdol);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::ForgottenAltar));

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("give idol");

        assert!(!after.relics.contains(&Relic::GoldenIdol));
        assert_eq!(
            after
                .relics
                .iter()
                .filter(|relic| **relic == Relic::BloodyIdol)
                .count(),
            1
        );
        assert!(after.relics.contains(&Relic::Circlet));
        assert_eq!(after.event.as_ref().expect("leave").stage, 1);
    }

    #[test]
    fn forgotten_altar_give_idol_requires_golden_idol() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::ForgottenAltar));

        let err = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect_err("requires Golden Idol");

        assert_eq!(
            err,
            SimError::IllegalAction("Forgotten Altar Give Idol requires Golden Idol")
        );
    }

    #[test]
    fn ghosts_accept_loses_half_max_hp_and_adds_five_apparitions() {
        let mut run = RunState::map_fixture();
        run.player_max_hp = 75;
        run.player_hp = 70;
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::Ghosts,
            choices: ghosts_choices(0, run.player_max_hp),
            stage: 0,
            event_data: 0,
        });
        let deck_len_before = run.deck.len();

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("accept");

        assert_eq!(ghosts_max_hp_loss(75), 38);
        assert_eq!(after.player_max_hp, 37);
        assert_eq!(after.player_hp, 37);
        assert_eq!(after.event.as_ref().expect("leave").event_data, 38);
        assert_eq!(after.deck.len(), deck_len_before + GHOSTS_APPARITION_COUNT);
        assert_eq!(
            after
                .deck
                .iter()
                .filter(|card| card.content_id == APPARITION_ID)
                .count(),
            GHOSTS_APPARITION_COUNT
        );
    }

    #[test]
    fn ghosts_accept_adds_three_apparitions_at_a15() {
        let mut run = RunState::map_fixture();
        run.ascension = 15;
        run.player_max_hp = 75;
        run.player_hp = 70;
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::Ghosts,
            choices: ghosts_choices(0, run.player_max_hp),
            stage: 0,
            event_data: 0,
        });

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("accept");

        assert_eq!(ghosts_apparition_count(15), GHOSTS_A15_APPARITION_COUNT);
        assert_eq!(
            after
                .deck
                .iter()
                .filter(|card| card.content_id == APPARITION_ID)
                .count(),
            GHOSTS_A15_APPARITION_COUNT
        );
    }

    #[test]
    fn ghosts_max_hp_loss_keeps_at_least_one_max_hp() {
        let mut run = RunState::map_fixture();
        run.player_max_hp = 1;
        run.player_hp = 1;
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::Ghosts,
            choices: ghosts_choices(0, run.player_max_hp),
            stage: 0,
            event_data: 0,
        });

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("accept");

        assert_eq!(ghosts_max_hp_loss(1), 0);
        assert_eq!(after.player_max_hp, 1);
        assert_eq!(after.player_hp, 1);
    }

    #[test]
    fn ghosts_leave_exits_without_changes() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Ghosts));
        let max_hp_before = run.player_max_hp;
        let deck_before = run.deck.clone();

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 1 }).expect("leave");

        assert_eq!(after.player_max_hp, max_hp_before);
        assert_eq!(after.deck, deck_before);
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
    }

    #[test]
    fn masked_bandits_pay_loses_all_gold_then_exits_after_dialogue() {
        let mut run = RunState::map_fixture();
        run.gold = 123;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::MaskedBandits));

        let paid = apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("pay");
        assert_eq!(paid.gold, 0);
        assert_eq!(paid.phase, RunPhase::Event);
        assert_eq!(paid.event.as_ref().expect("paid").stage, 1);
        assert_eq!(paid.event.as_ref().expect("paid").event_data, 123);

        let paid_2 =
            apply_event_action(&paid, EventAction::Choose { choice_index: 0 }).expect("continue");
        assert_eq!(paid_2.event.as_ref().expect("paid 2").stage, 2);
        assert_eq!(paid_2.event.as_ref().expect("paid 2").event_data, 123);

        let leave =
            apply_event_action(&paid_2, EventAction::Choose { choice_index: 0 }).expect("leave");
        assert_eq!(leave.event.as_ref().expect("leave").stage, 3);
        assert_eq!(
            leave.event.as_ref().expect("leave").choices[0].label,
            "Leave"
        );

        let after =
            apply_event_action(&leave, EventAction::Choose { choice_index: 0 }).expect("exit");
        assert_eq!(after.gold, 0);
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
    }

    #[test]
    fn masked_bandits_pay_allows_zero_gold() {
        let mut run = RunState::map_fixture();
        run.gold = 0;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::MaskedBandits));

        let paid = apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("pay");

        assert_eq!(paid.gold, 0);
        assert_eq!(paid.event.as_ref().expect("paid").stage, 1);
        assert_eq!(paid.event.as_ref().expect("paid").event_data, 0);
    }

    #[test]
    fn masked_bandits_fight_branch_is_explicitly_unsupported_until_event_combat_exists() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::MaskedBandits));

        let err = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect_err("fight unsupported");

        assert_eq!(
            err,
            SimError::IllegalAction("Masked Bandits fight branch is not implemented")
        );
    }

    #[test]
    fn colosseum_continue_reveals_forced_slavers_fight_prompt() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Colosseum));

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("continue");

        let event = after.event.as_ref().expect("fight prompt");
        assert_eq!(event.stage, 1);
        assert_eq!(event.choices.len(), 1);
        assert_eq!(event.choices[0].label, "Fight");
        assert_eq!(after.phase, RunPhase::Event);
    }

    #[test]
    fn colosseum_slavers_combat_branch_is_explicitly_unsupported() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::Colosseum,
            choices: colosseum_choices(1),
            stage: 1,
            event_data: 0,
        });

        let err = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect_err("combat unsupported");

        assert_eq!(
            err,
            SimError::IllegalAction("Colosseum Slavers combat branch is not implemented")
        );
    }

    #[test]
    fn colosseum_post_combat_choices_are_staged_but_nobs_combat_is_unsupported() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::Colosseum,
            choices: colosseum_choices(2),
            stage: 2,
            event_data: 0,
        });

        let flee = apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("flee");
        assert_eq!(flee.phase, RunPhase::Idle);
        assert!(flee.event.is_none());

        let err = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect_err("nobs unsupported");
        assert_eq!(
            err,
            SimError::IllegalAction("Colosseum Nobs combat branch is not implemented")
        );
    }

    #[test]
    fn drug_dealer_take_jax_adds_card_then_leaves() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::DrugDealer));
        let deck_len_before = run.deck.len();

        let after_take =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("take JAX");

        assert_eq!(after_take.deck.len(), deck_len_before + 1);
        assert_eq!(after_take.deck.last().expect("JAX").content_id, JAX_ID);
        assert_eq!(after_take.event.as_ref().expect("leave").stage, 1);

        let after_leave = apply_event_action(&after_take, EventAction::Choose { choice_index: 0 })
            .expect("leave");
        assert_eq!(after_leave.phase, RunPhase::Idle);
        assert!(after_leave.event.is_none());
    }

    #[test]
    fn drug_dealer_inject_mutagens_grants_special_relic_or_circlet() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::DrugDealer));

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 2 }).expect("inject");

        assert!(after.relics.contains(&Relic::MutagenicStrength));
        assert_eq!(after.event.as_ref().expect("leave").stage, 1);

        let mut already_owned = run.clone();
        already_owned.relics.push(Relic::MutagenicStrength);
        let duplicate = apply_event_action(&already_owned, EventAction::Choose { choice_index: 2 })
            .expect("inject duplicate");

        assert!(duplicate.relics.contains(&Relic::Circlet));
    }

    #[test]
    fn drug_dealer_test_subject_opens_two_card_transform_grid() {
        use crate::content::cards::BASH_ID;
        use crate::run::grid::{select_grid_card, GridPurpose};
        use crate::{CardId, CardInstance};

        let mut run = RunState::map_fixture();
        run.deck = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), BASH_ID),
        ];
        run.misc_rng_seed = 40_560_393_126;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::DrugDealer));
        let misc_counter_before = run.misc_rng_counter;

        let grid_run =
            apply_event_action(&run, EventAction::Choose { choice_index: 1 }).expect("transform");

        let grid = grid_run.card_grid.as_ref().expect("transform grid");
        assert_eq!(
            grid.purpose,
            GridPurpose::EventTransform {
                count: DRUG_DEALER_TRANSFORM_COUNT
            }
        );
        assert_eq!(grid.cards, run.deck);

        let selected_one = select_grid_card(&grid_run, 0).expect("select first");
        assert!(selected_one.card_grid.is_some());
        let after = select_grid_card(&selected_one, 1).expect("select second");

        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
        assert!(after.card_grid.is_none());
        assert_eq!(after.deck.len(), run.deck.len());
        assert!(!after.deck.iter().any(|card| card.id == CardId::new(1)));
        assert!(!after.deck.iter().any(|card| card.id == CardId::new(2)));
        assert!(after.misc_rng_counter > misc_counter_before);
    }

    #[test]
    fn drug_dealer_test_subject_rejects_fewer_than_two_transformable_cards() {
        use crate::{CardId, CardInstance};

        let mut run = RunState::map_fixture();
        let mut bottled = CardInstance::new(CardId::new(1), STRIKE_R_ID);
        bottled.bottled = true;
        run.deck = vec![bottled, CardInstance::new(CardId::new(2), DEFEND_R_ID)];
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::DrugDealer));

        let err = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect_err("not enough cards");

        assert_eq!(
            err,
            SimError::IllegalAction("not enough transformable cards")
        );
    }

    #[test]
    fn golden_shrine_choice_grants_gold_and_exits_event() {
        let mut run = RunState::map_fixture();
        enter_fixed_event_screen(&mut run);
        let gold_before = run.gold;

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("pray");

        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
        assert_eq!(after.gold, gold_before + GOLDEN_SHRINE_GOLD);
    }

    #[test]
    fn world_of_goop_gather_gold_loses_hp_gains_gold_then_requires_leave() {
        let mut run = RunState::map_fixture();
        run.player_hp = 53;
        run.player_max_hp = 85;
        run.gold = 179;
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::WorldOfGoop,
            choices: world_of_goop_choices(0, 34),
            stage: 0,
            event_data: 34,
        });

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("gather");

        assert_eq!(after.phase, RunPhase::Event);
        assert_eq!(after.player_hp, 42);
        assert_eq!(after.gold, 254);
        let screen = after.event.as_ref().expect("leave screen");
        assert_eq!(screen.event, Event::WorldOfGoop);
        assert_eq!(screen.stage, 1);
        assert_eq!(screen.choices.len(), 1);

        let leave =
            apply_event_action(&after, EventAction::Choose { choice_index: 0 }).expect("leave");
        assert_eq!(leave.phase, RunPhase::Idle);
        assert!(leave.event.is_none());
    }

    #[test]
    fn world_of_goop_leave_it_loses_rolled_gold_then_requires_leave() {
        let mut run = RunState::map_fixture();
        run.gold = 30;
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::WorldOfGoop,
            choices: world_of_goop_choices(0, 34),
            stage: 0,
            event_data: 34,
        });

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 1 }).expect("leave it");

        assert_eq!(after.phase, RunPhase::Event);
        assert_eq!(after.gold, 0);
        assert_eq!(after.event.as_ref().expect("leave").stage, 1);
    }

    #[test]
    fn entering_world_of_goop_rolls_gold_loss_and_stores_event_rng_counter() {
        let mut run = RunState::map_fixture();
        run.gold = 42;
        run.ascension = 0;
        run.misc_rng_seed = 77;
        run.misc_rng_counter = 0;
        run.event_rng_seed = 13;
        run.event_rng_counter = 0;
        run.act1_event_list = vec![Event::WorldOfGoop];
        run.act1_shrine_list = Vec::new();

        enter_event_screen(&mut run);

        let screen = run.event.as_ref().expect("goop");
        assert_eq!(screen.event, Event::WorldOfGoop);
        assert!(screen.event_data >= WORLD_OF_GOOP_MIN_GOLD_LOSS as u32);
        assert!(screen.event_data <= WORLD_OF_GOOP_MAX_GOLD_LOSS as u32);
        assert_eq!(run.misc_rng_counter, 1);
        assert!(run.event_rng_counter > 0);
    }

    #[test]
    fn generated_golden_shrine_uses_same_pray_branch_as_legacy_fixture() {
        let mut run = RunState::map_fixture();
        run.event_rng_seed = 13;
        run.act1_event_list = vec![Event::BigFish];
        run.act1_shrine_list = vec![Event::GoldenShrine];
        let gold_before = run.gold;
        let mut selected_counter = None;

        for counter in 0..64 {
            let mut trial = run.clone();
            trial.event_rng_counter = counter;
            enter_event_screen(&mut trial);
            if trial.event.as_ref().unwrap().event == Event::GoldenShrine {
                run = trial;
                selected_counter = Some(counter);
                break;
            }
        }

        assert_eq!(run.event.as_ref().unwrap().event, Event::GoldenShrine);
        assert!(run.act1_shrine_list.is_empty());
        assert_eq!(run.event_rng_counter, selected_counter.expect("counter"));
        assert_eq!(run.event.as_ref().unwrap().choices[0].label, "Pray");

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("pray");

        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
        assert_eq!(after.gold, gold_before + GOLDEN_SHRINE_GOLD);
    }

    #[test]
    fn ectoplasm_blocks_event_gold_gain() {
        let mut run = RunState::map_fixture();
        run.relics.push(crate::Relic::Ectoplasm);
        enter_fixed_event_screen(&mut run);
        let gold_before = run.gold;

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("pray");

        assert_eq!(after.gold, gold_before);
    }

    #[test]
    fn cleric_heal_choice_restores_quarter_max_hp_and_exits_event() {
        let mut run = RunState::map_fixture();
        run.player_max_hp = 80;
        run.player_hp = 40;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::TheCleric));

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("heal");

        assert_eq!(after.player_hp, 60);
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
    }

    #[test]
    fn cleric_remove_curse_choice_is_explicitly_unsupported() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::TheCleric));

        assert_eq!(run.event.as_ref().unwrap().choices[1].label, "Remove Curse");

        let err = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect_err("remove curse is not implemented");

        assert_eq!(
            err,
            SimError::IllegalAction("event choice is not implemented for this event")
        );
    }

    #[test]
    fn legal_event_actions_are_available_only_during_event_phase() {
        let mut run = RunState::map_fixture();

        assert!(legal_event_actions(&run).is_empty());

        enter_fixed_event_screen(&mut run);

        assert_eq!(
            legal_event_actions(&run),
            vec![EventAction::Choose { choice_index: 0 }]
        );
    }

    #[test]
    fn event_action_rejects_missing_choice() {
        let mut run = RunState::map_fixture();
        enter_fixed_event_screen(&mut run);

        let err = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect_err("missing choice");

        assert_eq!(
            err,
            SimError::IllegalAction("event choice is not available")
        );
    }

    #[test]
    fn shining_light_hp_loss_rounds_twenty_percent_of_max_hp() {
        assert_eq!(shining_light_hp_loss(80), 16);
        assert_eq!(shining_light_hp_loss(79), 16);
    }

    #[test]
    fn shining_light_enter_costs_hp_and_upgrades_two_cards() {
        use crate::content::cards::{ANGER_ID, ANGER_PLUS_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID};
        use crate::CardId;
        use crate::CardInstance;

        let mut run = RunState::map_fixture();
        run.misc_rng_seed = 7;
        run.deck = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), ANGER_ID),
        ];
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::ShiningLight));

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("enter");

        assert_eq!(
            after.player_hp,
            run.player_hp - shining_light_hp_loss(run.player_max_hp)
        );
        assert_eq!(after.deck[0].content_id, STRIKE_R_PLUS_ID);
        assert_eq!(after.deck[1].content_id, ANGER_PLUS_ID);
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
        assert!(after.misc_rng_counter > run.misc_rng_counter);
    }

    #[test]
    fn shining_light_leave_exits_without_changes() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::ShiningLight));
        let hp_before = run.player_hp;

        let after =
            apply_event_action(&run, EventAction::Choose { choice_index: 1 }).expect("leave");

        assert_eq!(after.player_hp, hp_before);
        assert_eq!(after.deck, run.deck);
        assert_eq!(after.phase, RunPhase::Idle);
        assert!(after.event.is_none());
    }

    #[test]
    fn event_action_is_illegal_outside_event_phase() {
        let run = RunState::map_fixture();

        let err = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect_err("not in event");

        assert_eq!(
            err,
            SimError::IllegalAction("event actions require event phase")
        );
    }

    #[test]
    fn big_fish_box_grants_rng_relic_and_regret() {
        let mut run = RunState::map_fixture();
        run.reward_rng_seed = 1_218_623;
        run.relic_rng_seed = 1_218_623;
        run.current_floor = 3;
        run.ensure_ironclad_relic_pools();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::BigFish));
        let relic_count = run.relics.len() + run.relic_keys.len();
        assert_eq!(run.event.as_ref().unwrap().event, Event::BigFish);

        let after = apply_event_action(&run, EventAction::Choose { choice_index: 2 }).expect("box");

        assert_eq!(after.relics.len() + after.relic_keys.len(), relic_count + 1);
        assert!(!after.deck.iter().any(|card| card.content_id == REGRET_ID));
        assert_eq!(after.event.as_ref().unwrap().stage, 1);

        let done =
            apply_event_action(&after, EventAction::Choose { choice_index: 0 }).expect("leave");

        assert_eq!(done.phase, RunPhase::Idle);
        assert!(done.event.is_none());
        assert!(done.deck.iter().any(|card| card.content_id == REGRET_ID));
    }

    #[test]
    fn sssssserpent_agree_grants_gold_then_doubt() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::TheSsssserpent));
        let gold_before = run.gold;

        let after_agree =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("agree");
        assert_eq!(after_agree.gold, gold_before);
        assert_eq!(after_agree.event.as_ref().unwrap().stage, 1);

        let after_continue =
            apply_event_action(&after_agree, EventAction::Choose { choice_index: 0 })
                .expect("continue");
        assert_eq!(after_continue.gold, gold_before + SSSSSERPENT_GOLD);
        assert_eq!(after_continue.event.as_ref().unwrap().stage, 2);

        let after_leave =
            apply_event_action(&after_continue, EventAction::Choose { choice_index: 0 })
                .expect("leave");
        assert_eq!(after_leave.phase, RunPhase::Idle);
        assert!(after_leave.event.is_none());
        assert!(after_leave
            .deck
            .iter()
            .any(|card| card.content_id == DOUBT_ID));
    }

    #[test]
    fn test_scrap_ooze_first_reach_fails_on_test_seed_floor() {
        let mut run = RunState::map_fixture();
        run.reward_rng_seed = 1_218_623;
        run.relic_rng_seed = 1_218_623;
        run.current_floor = 3;
        run.reinit_misc_rng_for_floor();
        run.player_hp = 75;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::ScrapOoze));

        let after_reach =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("reach");
        assert_eq!(after_reach.event.as_ref().unwrap().stage, 1);
        assert!(after_reach.relic_keys.is_empty());
    }

    #[test]
    fn test_scrap_ooze_misc_counter_for_test_seed() {
        let mut run = RunState::map_fixture();
        run.reward_rng_seed = 1_218_623;
        run.relic_rng_seed = 1_218_623;
        run.current_floor = 3;
        run.reinit_misc_rng_for_floor();
        run.player_hp = 75;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::ScrapOoze));

        let after_reach =
            apply_event_action(&run, EventAction::Choose { choice_index: 0 }).expect("reach");
        assert_eq!(after_reach.event.as_ref().unwrap().stage, 1);
        let after_deeper =
            apply_event_action(&after_reach, EventAction::Choose { choice_index: 0 })
                .expect("deeper");
        assert!(after_deeper.relics.contains(&Relic::DreamCatcher));
    }

    #[test]
    fn test_seed_event_selection_removes_events_without_advancing_counter() {
        let mut run = RunState::map_fixture();
        run.event_rng_seed = 1_218_623;
        run.misc_rng_seed = 1_218_623;

        let mut first_counter = None;
        for counter in 0..64 {
            let mut trial = run.clone();
            trial.event_rng_counter = counter;
            enter_event_screen(&mut trial);
            if trial.event.as_ref().unwrap().event == Event::ScrapOoze {
                first_counter = Some(counter);
                break;
            }
        }
        let first_counter = first_counter.expect("scrap ooze counter");
        assert_eq!(first_counter, 1, "TEST seed first event counter");
        run.event_rng_counter = first_counter;

        enter_event_screen(&mut run);
        assert_eq!(run.event.as_ref().unwrap().event, Event::ScrapOoze);
        assert_eq!(run.event_rng_counter, first_counter);

        run.phase = RunPhase::Idle;
        run.event = None;
        enter_event_screen(&mut run);
        assert_ne!(run.event.as_ref().unwrap().event, Event::ScrapOoze);
        assert_eq!(run.event_rng_counter, first_counter);
    }
}
