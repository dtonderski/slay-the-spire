//! Deterministic, player-visible combat observation projection.
//!
//! This module is deliberately symbolic. Tensor extraction belongs in an RL
//! wrapper, and authoritative instance IDs and hidden simulator state never
//! cross this boundary.

use crate::{
    card::CardInstance,
    combat::{
        cost::effective_card_cost_with_corruption, turn_powers::monster_damage_to_player,
        CombatDecisionState, CombatOrb, CombatPhase, CombatState, DiscardSelectPurpose,
        DrawSelectPurpose, ExhaustSelectPurpose, HandSelectPurpose, MonsterIntent, MonsterState,
        PotionCardRewardKind, SlimeSize,
    },
    content::{cards::get_card_definition, monsters::get_monster_definition},
    potion::Potion,
    power::PlayerPowers,
    relic::{Relic, MATRYOSHKA_MAX_CHESTS, OMAMORI_CHARGES},
    run::{RunPhase, RunState},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, error::Error, fmt};

/// Version of the serialized symbolic fair-combat observation contract.
pub const FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FairObservationError {
    NoActiveCombat,
    InvalidAuthoritativeState,
    UnknownPublicContent,
    /// Public pool identity for content that exists in the game but has no
    /// modeled `CardDefinition`. Never carries an internal content id.
    UnmodeledPublicContent(&'static str),
}

impl fmt::Display for FairObservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoActiveCombat => f.write_str("no active combat"),
            Self::InvalidAuthoritativeState => f.write_str("authoritative combat state is invalid"),
            Self::UnknownPublicContent => f.write_str("public combat content is unknown"),
            Self::UnmodeledPublicContent(public_key) => {
                write!(f, "public combat content is unmodeled: {public_key}")
            }
        }
    }
}

impl Error for FairObservationError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairCombatObservation {
    pub schema_version: u32,
    pub context: FairRunContext,
    pub phase: FairCombatPhase,
    pub player: FairPlayer,
    /// Public left-to-right orb slots. Empty slots are represented explicitly.
    pub orb_slots: Vec<FairOrbSlot>,
    pub hand: Vec<FairHandCard>,
    pub draw_pile: FairPile,
    pub discard_pile: FairPile,
    pub exhaust_pile: FairPile,
    pub monsters: Vec<FairMonster>,
    pub relics: Vec<FairRelic>,
    pub potion_slots: Vec<FairPotionSlot>,
    pub selection: Option<FairSelection>,
    pub public_counters: Vec<FairCounter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairRunContext {
    pub ascension: u8,
    pub act: i32,
    pub floor: i32,
    pub gold: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FairCombatPhase {
    WaitingForPlayer,
    MonsterTurn,
    Won,
    Lost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairPlayer {
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub energy: i32,
    pub max_energy: i32,
    pub powers: Vec<FairPower>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairOrbSlot {
    pub slot: usize,
    pub orb: Option<FairOrb>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FairOrb {
    Lightning,
    Frost,
    Dark { evoke: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FairCard {
    /// Stable public content key, never an internal numeric `ContentId`.
    pub content_key: String,
    /// Effective cost currently displayed to the player. `-1` denotes X cost.
    pub cost: i32,
    /// Whether the displayed cost differs through combat-local cost state.
    pub cost_is_modified: bool,
    /// Whether that combat-local cost expires at the next player turn.
    pub cost_resets_next_turn: bool,
    /// Instance-local upgrades not already represented by `content_key`.
    pub upgrade_level: u8,
    pub bottled: bool,
    /// Whether public history identifies this as a combat-only generated card.
    pub temporary: bool,
    pub dynamic: FairCardDynamicValues,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FairCardDynamicValues {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rampage_damage_bonus: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ritual_dagger_damage_bonus: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub windmill_retain_damage: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_barrier_block_reduction: Option<i32>,
    /// Combat-long cost that will reappear when the displayed turn-only cost
    /// expires (for example Streamline played under a zero-cost override).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combat_cost_under_turn_override: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairHandCard {
    /// Decision-local public reference; never an authoritative card ID.
    pub slot: usize,
    pub card: FairCard,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairPile {
    pub count: usize,
    /// Public contents in canonical multiset order.
    pub cards: Vec<FairCard>,
    /// Known top-to-bottom order. Empty when no order is publicly known.
    pub known_order: Vec<FairCard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairMonster {
    pub slot: usize,
    pub content_key: String,
    pub slime_size: Option<SlimeSize>,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub powers: Vec<FairPower>,
    /// Gold visibly stolen by this monster and recoverable on kill.
    pub stolen_gold: i32,
    /// Public card held by Bronze Orb's Stasis, without its instance ID.
    pub stasis_card: Option<FairCard>,
    pub intent: FairMonsterIntent,
    pub alive: bool,
    pub escaped: bool,
    pub minion: bool,
    pub targetable: bool,
    pub in_defensive_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "visibility", rename_all = "snake_case")]
pub enum FairMonsterIntent {
    Hidden,
    None,
    Visible {
        category: FairIntentCategory,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        damage: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hits: Option<i32>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FairIntentCategory {
    Unknown,
    Attack,
    AttackBuff,
    AttackDebuff,
    AttackDefend,
    Buff,
    Debuff,
    StrongDebuff,
    Defend,
    DefendBuff,
    Escape,
    Sleep,
    Stun,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FairPower {
    pub key: String,
    pub amount: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairRelic {
    pub slot: usize,
    pub content_key: String,
    pub state: Vec<FairCounter>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FairCounter {
    pub key: String,
    pub value: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairPotionSlot {
    pub slot: usize,
    pub content_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairSelection {
    pub kind: FairSelectionKind,
    pub options: Vec<FairSelectionOption>,
    pub selected_slots: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairSelectionOption {
    pub slot: usize,
    pub card: FairCard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FairSelectionKind {
    PotionAttackReward,
    PotionSkillReward,
    PotionPowerReward,
    PotionColorlessReward,
    ToolboxReward,
    DiscoveryReward,
    WarcryPutOnDraw,
    ArmamentsUpgrade,
    ForethoughtPutOnDraw,
    ForethoughtPutAnyOnDraw,
    ThinkingAheadPutOnDraw,
    PreparedDiscard,
    DualWieldCopy,
    SecretTechniqueSkillToHand,
    SecretWeaponAttackToHand,
    Scry,
    LiquidMemoriesReturnToHand,
    HeadbuttPutOnDraw,
    HologramReturnToHand,
    Exhaust,
    GamblingChip,
    ExhumeReturnToHand,
    PurityExhaustUpToThree,
    BurningPactDrawTwo,
    BurningPactDrawThree,
    TrueGritExhaustOne,
    RecycleExhaustOne,
}

/// Projects the active authoritative combat into deterministic public data.
///
/// The projection borrows state, performs no RNG draws, and does not validate
/// or mutate hidden simulator internals. Errors are intentionally coarse and
/// carry no internal identity.
pub fn fair_combat_observation(
    run: &RunState,
) -> Result<FairCombatObservation, FairObservationError> {
    let combat = match (run.phase, run.combat.as_ref()) {
        (RunPhase::Combat, Some(combat)) => combat,
        (RunPhase::Combat, None) | (_, Some(_)) => {
            return Err(FairObservationError::InvalidAuthoritativeState);
        }
        (_, None) => return Err(FairObservationError::NoActiveCombat),
    };
    let corruption_active = combat.player.powers.corruption > 0;
    let visible_gold = run
        .gold
        .checked_add(combat.combat_gold_gained)
        .ok_or(FairObservationError::InvalidAuthoritativeState)?;

    let hand = combat
        .piles
        .hand
        .iter()
        .enumerate()
        .map(|(slot, card)| {
            Ok(FairHandCard {
                slot,
                card: project_card(card, corruption_active)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let draw_cards = canonical_cards(&combat.piles.draw_pile, corruption_active)?;
    let slot_count = usize::try_from(combat.max_orbs)
        .map_err(|_| FairObservationError::InvalidAuthoritativeState)?;
    if combat.orbs.len() > slot_count {
        return Err(FairObservationError::InvalidAuthoritativeState);
    }
    let orb_slots = (0..slot_count)
        .map(|slot| {
            let orb = combat
                .orbs
                .get(slot)
                .copied()
                .map(project_orb)
                .transpose()?;
            Ok(FairOrbSlot { slot, orb })
        })
        .collect::<Result<Vec<_>, FairObservationError>>()?;
    let draw_order = if combat.relics.contains(&Relic::FrozenEye) {
        combat
            .piles
            .draw_pile
            .iter()
            .rev()
            .map(|card| project_card(card, corruption_active))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };

    Ok(FairCombatObservation {
        schema_version: FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION,
        context: FairRunContext {
            ascension: run.ascension,
            act: run.current_act,
            floor: run.current_floor,
            gold: visible_gold,
        },
        phase: project_phase(combat.phase),
        player: FairPlayer {
            hp: combat.player.hp,
            max_hp: combat.player.max_hp,
            block: combat.player.block,
            energy: combat.player.energy,
            max_energy: combat.player.max_energy,
            powers: project_player_powers(combat)?,
        },
        orb_slots,
        hand,
        draw_pile: FairPile {
            count: combat.piles.draw_pile.len(),
            cards: draw_cards,
            known_order: draw_order,
        },
        discard_pile: FairPile {
            count: combat.piles.discard_pile.len(),
            cards: canonical_cards(&combat.piles.discard_pile, corruption_active)?,
            known_order: Vec::new(),
        },
        exhaust_pile: FairPile {
            count: combat.piles.exhaust_pile.len(),
            cards: canonical_cards(&combat.piles.exhaust_pile, corruption_active)?,
            known_order: Vec::new(),
        },
        monsters: combat
            .monsters
            .iter()
            .enumerate()
            .map(|(slot, monster)| project_monster(slot, monster, combat))
            .collect::<Result<Vec<_>, _>>()?,
        relics: combat
            .relics
            .iter()
            .copied()
            .enumerate()
            .map(|(slot, relic)| FairRelic {
                slot,
                content_key: relic.trace_name().to_owned(),
                state: project_relic_state(relic, run, combat),
            })
            .collect(),
        potion_slots: (0..run.potion_capacity())
            .map(|slot| FairPotionSlot {
                slot,
                content_key: run.potion_at_slot(slot).map(potion_key).map(str::to_owned),
            })
            .collect(),
        selection: project_selection(combat, corruption_active)?,
        public_counters: project_public_counters(combat),
    })
}

fn project_phase(phase: CombatPhase) -> FairCombatPhase {
    match phase {
        CombatPhase::WaitingForPlayer => FairCombatPhase::WaitingForPlayer,
        CombatPhase::MonsterTurn => FairCombatPhase::MonsterTurn,
        CombatPhase::Won => FairCombatPhase::Won,
        CombatPhase::Lost => FairCombatPhase::Lost,
    }
}

pub(crate) fn project_card(
    card: &CardInstance,
    corruption_active: bool,
) -> Result<FairCard, FairObservationError> {
    let definition = get_card_definition(card.content_id)
        .ok_or_else(|| unmodeled_or_unknown_public_content(card.content_id))?;
    let cost = effective_card_cost_with_corruption(card, corruption_active)
        .map_err(|_| FairObservationError::InvalidAuthoritativeState)?;
    Ok(FairCard {
        content_key: definition.key.to_owned(),
        cost,
        cost_is_modified: cost != i32::from(definition.cost),
        cost_resets_next_turn: card.temp_cost_turn_only,
        upgrade_level: card.upgrades.max(card.searing_blow_upgrades),
        bottled: card.bottled,
        temporary: card.combat_only,
        dynamic: FairCardDynamicValues {
            rampage_damage_bonus: nonzero(card.rampage_damage_bonus),
            ritual_dagger_damage_bonus: nonzero(card.ritual_dagger_damage_bonus),
            windmill_retain_damage: nonzero(card.windmill_retain_damage),
            steam_barrier_block_reduction: nonzero(card.steam_barrier_block_reduction),
            combat_cost_under_turn_override: card.combat_cost_under_turn_override.map(i32::from),
        },
    })
}

fn unmodeled_or_unknown_public_content(content_id: crate::ContentId) -> FairObservationError {
    match crate::run::reward::any_color_reward_card_key(content_id) {
        Some(public_key) => FairObservationError::UnmodeledPublicContent(public_key),
        None => FairObservationError::UnknownPublicContent,
    }
}

fn project_orb(orb: CombatOrb) -> Result<FairOrb, FairObservationError> {
    match orb {
        CombatOrb::Lightning => Ok(FairOrb::Lightning),
        CombatOrb::Frost => Ok(FairOrb::Frost),
        CombatOrb::Dark { evoke } if evoke >= 0 => Ok(FairOrb::Dark { evoke }),
        CombatOrb::Dark { .. } => Err(FairObservationError::InvalidAuthoritativeState),
    }
}

fn canonical_cards(
    cards: &[CardInstance],
    corruption_active: bool,
) -> Result<Vec<FairCard>, FairObservationError> {
    let mut projected = cards
        .iter()
        .map(|card| project_card(card, corruption_active))
        .collect::<Result<Vec<_>, _>>()?;
    projected.sort();
    Ok(projected)
}

fn project_monster(
    slot: usize,
    monster: &MonsterState,
    combat: &CombatState,
) -> Result<FairMonster, FairObservationError> {
    let definition = get_monster_definition(monster.content_id)
        .ok_or(FairObservationError::UnknownPublicContent)?;
    Ok(FairMonster {
        slot,
        content_key: definition.name.to_owned(),
        slime_size: monster.slime_size,
        hp: monster.hp,
        max_hp: monster.max_hp,
        block: monster.block,
        powers: project_monster_powers(monster),
        stolen_gold: monster.stolen_gold,
        stasis_card: monster
            .stasis_card
            .as_ref()
            .map(|card| project_card(card, combat.player.powers.corruption > 0))
            .transpose()?,
        intent: project_intent(monster, combat)?,
        alive: monster.alive,
        escaped: monster.escaped,
        minion: monster.powers.minion > 0,
        targetable: monster.alive,
        in_defensive_mode: monster.in_defensive_mode,
    })
}

fn project_intent(
    monster: &MonsterState,
    combat: &CombatState,
) -> Result<FairMonsterIntent, FairObservationError> {
    if !monster.alive {
        return Ok(FairMonsterIntent::None);
    }
    if combat.relics.contains(&Relic::RunicDome) {
        return Ok(FairMonsterIntent::Hidden);
    }

    let (category, base_damage, hits) = intent_public_fields(monster);
    let damage = base_damage
        .map(|damage| monster_damage_to_player(&combat.player, monster, damage))
        .transpose()
        .map_err(|_| FairObservationError::InvalidAuthoritativeState)?;
    Ok(FairMonsterIntent::Visible {
        category,
        damage,
        hits,
    })
}

fn intent_public_fields(monster: &MonsterState) -> (FairIntentCategory, Option<i32>, Option<i32>) {
    use crate::content::monsters::{
        ACID_SLIME_ID, BANDIT_BEAR_ID, BANDIT_LEADER_ID, BRONZE_ORB_ID, BYRD_ID, CHOSEN_ID,
        GREMLIN_WIZARD_ID, GUARDIAN_ID, HEXAGHOST_ID, LAGAVULIN_ID, RED_LOUSE_ID, SLIME_BOSS_ID,
        SNECKO_ID, SPIKER_ID, SPIKE_SLIME_ID,
    };
    use FairIntentCategory as C;
    use MonsterIntent as I;

    let attack = |category, damage| (category, Some(damage), Some(1));
    match monster.intent {
        I::PendingAiRoll | I::DarklingCount | I::AwakenedOneHalfDead => (C::Unknown, None, None),
        I::Attack { damage } | I::AttackStealGold { damage, .. } => attack(C::Attack, damage),
        I::AttackAddSlimedToDiscard { damage, .. }
        | I::AttackAddWoundsToDiscard { damage, .. }
        | I::AttackAddVoidToDraw { damage, .. }
        | I::AttackApplyPlayerFrail { damage, .. }
        | I::AttackApplyPlayerFrailAndWeak { damage, .. }
        | I::AttackApplyPlayerFrailAndVulnerable { damage, .. }
        | I::AttackApplyPlayerWeak { damage, .. }
        | I::AttackApplyPlayerVulnerable { damage, .. }
        | I::AttackApplyPlayerWeakAndVulnerable { damage, .. }
        | I::AddBurnToDiscardAndDraw { damage, .. } => attack(C::AttackDebuff, damage),
        I::AttackMultiple { damage, hits } if monster.content_id == GUARDIAN_ID && hits == 2 => {
            (C::AttackBuff, Some(damage), Some(hits))
        }
        I::AttackMultiple { damage, hits } => (C::Attack, Some(damage), Some(hits)),
        I::AttackMultipleAddDazedToDiscard { damage, hits, .. }
        | I::AttackMultipleApplyPlayerWeak { damage, hits, .. }
        | I::AttackMultipleUpgradeBurns { damage, hits, .. } => {
            (C::AttackDebuff, Some(damage), Some(hits))
        }
        I::AttackAndBlock { damage, .. } => attack(C::AttackDefend, damage),
        I::AttackHealSelf { damage } => attack(C::AttackBuff, damage),
        I::AddBurnToDiscard { damage, .. } if damage > 0 => attack(C::AttackDebuff, damage),
        I::Block { .. } if monster.content_id == GREMLIN_WIZARD_ID => (C::Unknown, None, None),
        I::Block { .. } => (C::Defend, None, None),
        I::StrengthAndBlock { .. } if matches!(monster.content_id, RED_LOUSE_ID | SPIKER_ID) => {
            (C::Buff, None, None)
        }
        I::StrengthAndBlock { .. } | I::EncourageGremlins { .. } => (C::DefendBuff, None, None),
        I::StrengthSelf { .. } if monster.content_id == GREMLIN_WIZARD_ID => {
            (C::Unknown, None, None)
        }
        I::StrengthSelf { amount: 0 } if monster.content_id == BYRD_ID => (C::Unknown, None, None),
        I::Ritual { .. }
        | I::HealAllMonsters { .. }
        | I::StrengthSelf { .. }
        | I::StrengthAllMonsters { .. }
        | I::GuardianCloseUp { .. } => (C::Buff, None, None),
        I::ApplyPlayerFrailAndWeak { .. }
            if matches!(monster.content_id, ACID_SLIME_ID | SPIKE_SLIME_ID) =>
        {
            (C::Debuff, None, None)
        }
        I::SiphonPlayer { .. }
            if matches!(
                monster.content_id,
                LAGAVULIN_ID | BRONZE_ORB_ID | BANDIT_BEAR_ID
            ) =>
        {
            (C::StrongDebuff, None, None)
        }
        I::ApplyPlayerHex { .. } if monster.content_id == CHOSEN_ID => {
            (C::StrongDebuff, None, None)
        }
        I::ApplyPlayerConfusion if monster.content_id == SNECKO_ID => (C::StrongDebuff, None, None),
        I::ApplyPlayerWeak { .. }
        | I::ApplyPlayerHex { .. }
        | I::ApplyPlayerWeakStrengthSelf { .. }
        | I::ApplyPlayerConfusion
        | I::AddDazedToDiscard { .. }
        | I::AddDazedToDraw { .. }
        | I::AddBurnToDiscard { .. }
        | I::SiphonPlayer { .. } => (C::Debuff, None, None),
        I::ApplyPlayerFrailAndWeak { .. }
        | I::ApplyPlayerFrailWeakVulnerable { .. }
        | I::ApplyPlayerConstricted { .. }
        | I::ApplyPlayerEntangled { .. } => (C::StrongDebuff, None, None),
        I::AddSlimedToDiscard { .. } if monster.content_id == SLIME_BOSS_ID => {
            (C::StrongDebuff, None, None)
        }
        I::AddSlimedToDiscard { .. } => (C::Debuff, None, None),
        I::Sleep => (C::Sleep, None, None),
        I::Stun
            if matches!(
                monster.content_id,
                SLIME_BOSS_ID | HEXAGHOST_ID | BANDIT_LEADER_ID
            ) =>
        {
            (C::Unknown, None, None)
        }
        I::Stun => (C::Stun, None, None),
        I::Escape => (C::Escape, None, None),
        I::DefensiveCharge { .. }
        | I::SummonGremlins { .. }
        | I::SummonCollectorTorchHeads { .. } => (C::Unknown, None, None),
    }
}

fn project_player_powers(combat: &CombatState) -> Result<Vec<FairPower>, FairObservationError> {
    let mut powers = combat.player.powers;
    powers.strength = powers
        .strength
        .checked_add(combat.player.temp_strength)
        .ok_or(FairObservationError::InvalidAuthoritativeState)?;
    powers.thorns = powers
        .thorns
        .checked_add(combat.player.temp_thorns)
        .ok_or(FairObservationError::InvalidAuthoritativeState)?;
    let mut result = Vec::new();
    push_player_power_fields(&mut result, powers);
    push_power(&mut result, "lose_strength", combat.player.temp_strength);
    push_power(&mut result, "lose_dexterity", combat.player.temp_dexterity);
    push_power(&mut result, "temporary_thorns", combat.player.temp_thorns);
    push_power(&mut result, "rage", combat.player.temp_rage_block);
    push_power(&mut result, "no_block", combat.player.no_block_turns);
    push_power(&mut result, "no_draw", i32::from(combat.player.cannot_draw));
    push_power(&mut result, "double_tap", combat.double_tap_pending);
    let duplication = combat
        .duplication_potion_stacks
        .checked_add(i32::from(combat.duplication_potion_pending))
        .ok_or(FairObservationError::InvalidAuthoritativeState)?;
    push_power(&mut result, "duplication", duplication);
    for timer in &combat.bomb_timers {
        push_power(
            &mut result,
            &format!("bomb_{}_damage", timer.turns_remaining),
            timer.damage,
        );
    }
    result.sort();
    Ok(result)
}

fn push_player_power_fields(result: &mut Vec<FairPower>, p: PlayerPowers) {
    for (key, amount) in [
        ("strength", p.strength),
        ("focus", p.focus),
        ("mantra", p.mantra),
        ("weak", p.weak),
        ("dexterity", p.dexterity),
        ("frail", p.frail),
        ("vulnerable", p.vulnerable),
        ("ritual", p.ritual),
        ("demon_form", p.demon_form),
        ("metallicize", p.metallicize),
        ("regen", p.regen),
        ("thorns", p.thorns),
        ("plated_armor", p.plated_armor),
        ("artifact", p.artifact),
        ("feel_no_pain", p.feel_no_pain),
        ("dark_embrace", p.dark_embrace),
        ("barricade", p.barricade),
        ("evolve", p.evolve),
        ("berserk", p.berserk),
        ("fasting", p.fasting),
        ("like_water", p.like_water),
        ("nirvana", p.nirvana),
        ("rupture", p.rupture),
        ("juggernaut", p.juggernaut),
        ("brutality", p.brutality),
        ("mayhem", p.mayhem),
        ("combust", p.combust),
        ("combust_damage", p.combust_damage),
        ("fire_breathing", p.fire_breathing),
        ("corruption", p.corruption),
        ("magnetism", p.magnetism),
        ("creative_ai", p.creative_ai),
        ("storm", p.storm),
        ("after_image", p.after_image),
        ("static_discharge", p.static_discharge),
        ("wrath", p.wrath),
        ("panache", p.panache),
        ("panache_cards_played", p.panache_cards_played),
        ("buffer", p.buffer),
        ("intangible", p.intangible),
        ("sadistic_nature", p.sadistic_nature),
        ("hex", p.hex),
        ("confusion", p.confusion),
        ("entangled", p.entangled),
        ("constricted", p.constricted),
        ("vigor", p.vigor),
    ] {
        push_power(result, key, amount);
    }
}

fn project_monster_powers(monster: &MonsterState) -> Vec<FairPower> {
    let p = monster.powers;
    let mut result = Vec::new();
    for (key, amount) in [
        ("vulnerable", p.vulnerable),
        ("poison", p.poison),
        ("lock_on", p.lock_on),
        ("mark", p.mark),
        ("weak", p.weak),
        ("strength", p.strength),
        ("artifact", p.artifact),
        ("flight", p.flight),
        ("intangible", p.intangible),
        ("plated_armor", p.plated_armor),
        ("painful_stabs", p.painful_stabs),
        ("explosive", p.explosive),
        ("ritual", p.ritual),
        ("spikes", p.spikes),
        ("curl_up", p.curl_up),
        ("anger", p.anger),
        ("metallicize", p.metallicize),
        ("malleable", p.malleable),
        ("spore_cloud", p.spore_cloud),
        ("strength_up", p.strength_up),
        ("slow", p.slow),
        ("restore_strength", monster.temp_strength_down),
    ] {
        push_power(&mut result, key, amount);
    }
    if !monster.in_defensive_mode {
        push_power(&mut result, "mode_shift", monster.mode_shift);
    }
    result.sort();
    result
}

fn push_power(result: &mut Vec<FairPower>, key: &str, amount: i32) {
    if amount != 0 {
        result.push(FairPower {
            key: key.to_owned(),
            amount,
        });
    }
}

fn project_relic_state(relic: Relic, run: &RunState, combat: &CombatState) -> Vec<FairCounter> {
    let counters = &combat.relic_counters;
    let mut state = Vec::new();
    let mut push = |key: &str, value: i64| {
        state.push(FairCounter {
            key: key.to_owned(),
            value,
        });
    };
    match relic {
        Relic::LizardTail => push("available", i64::from(counters.lizard_tail_available)),
        Relic::InkBottle => push("cards", i64::from(counters.ink_bottle_cards_played)),
        Relic::OrnamentalFan => push(
            "attacks_this_turn",
            i64::from(counters.ornamental_fan_attacks_this_turn),
        ),
        Relic::Nunchaku => push("attacks", i64::from(counters.nunchaku_attacks_played)),
        Relic::PenNib => push("attacks", i64::from(counters.pen_nib_attacks_played)),
        Relic::Shuriken => push(
            "attacks_this_turn",
            i64::from(counters.shuriken_attacks_this_turn),
        ),
        Relic::Kunai => push(
            "attacks_this_turn",
            i64::from(counters.kunai_attacks_this_turn),
        ),
        Relic::LetterOpener => push(
            "skills_this_turn",
            i64::from(counters.letter_opener_skills_this_turn),
        ),
        Relic::HappyFlower => push("turns", i64::from(counters.happy_flower_turns)),
        Relic::Sundial => push("shuffles", i64::from(counters.sundial_shuffles)),
        Relic::IncenseBurner => push("turns", i64::from(counters.incense_burner_counter)),
        Relic::CentennialPuzzle => push("triggers", i64::from(counters.centennial_puzzle_triggers)),
        Relic::Akabeko => push(
            "attacks_this_combat",
            i64::from(counters.attacks_played_this_combat),
        ),
        Relic::Pocketwatch => push(
            "cards_last_turn",
            i64::from(counters.cards_played_last_turn),
        ),
        Relic::ArtOfWar => push(
            "attacks_last_turn",
            i64::from(counters.attacks_played_last_turn),
        ),
        Relic::OrangePellets => {
            push(
                "attack_played",
                i64::from(counters.orange_pellets_attack_played),
            );
            push(
                "skill_played",
                i64::from(counters.orange_pellets_skill_played),
            );
            push(
                "power_played",
                i64::from(counters.orange_pellets_power_played),
            );
        }
        Relic::Necronomicon => push(
            "used_this_turn",
            i64::from(counters.necronomicon_used_this_turn),
        ),
        Relic::SelfFormingClay => push(
            "next_turn_block",
            i64::from(counters.self_forming_clay_next_turn_block),
        ),
        Relic::RedSkull => push("active", i64::from(counters.red_skull_active)),
        Relic::VelvetChoker => push(
            "cards_this_turn",
            i64::from(counters.cards_played_this_turn),
        ),
        Relic::HornCleat | Relic::CaptainsWheel | Relic::StoneCalendar => push(
            "player_turns_started",
            i64::from(counters.player_turns_started),
        ),
        Relic::Omamori => push(
            "charges_remaining",
            i64::from(OMAMORI_CHARGES.saturating_sub(run.omamori_charges_used)),
        ),
        Relic::MawBank => push("active", i64::from(!run.maw_bank_broken)),
        Relic::AncientTeaSet => push("armed", i64::from(run.ancient_tea_set_armed)),
        Relic::Girya => push("lifts", i64::from(run.girya_lifts)),
        Relic::Matryoshka => push(
            "chests_remaining",
            i64::from(MATRYOSHKA_MAX_CHESTS.saturating_sub(run.matryoshka_chests_opened)),
        ),
        Relic::TinyChest => push("rooms", i64::from(run.tiny_chest_counter)),
        Relic::WingBoots => push("charges", i64::from(run.wing_boots_charges)),
        Relic::NeowsLament => push(
            "combats_remaining",
            i64::from(run.neow_lament_combats_remaining),
        ),
        _ => {}
    }
    state
}

fn project_public_counters(combat: &CombatState) -> Vec<FairCounter> {
    [
        (
            "cards_played_this_turn",
            i64::from(combat.relic_counters.cards_played_this_turn),
        ),
        (
            "attacks_played_this_turn",
            i64::from(combat.relic_counters.attacks_played_this_turn),
        ),
        (
            "cards_discarded_this_turn",
            i64::from(combat.total_discarded_this_turn),
        ),
    ]
    .into_iter()
    .map(|(key, value)| FairCounter {
        key: key.to_owned(),
        value,
    })
    .collect()
}

fn project_selection(
    combat: &CombatState,
    corruption_active: bool,
) -> Result<Option<FairSelection>, FairObservationError> {
    let Some(decision) = combat.decision.as_ref() else {
        return Ok(None);
    };
    let selection = match decision {
        CombatDecisionState::PotionCardReward {
            choices,
            reward_kind,
        } => FairSelection {
            kind: match reward_kind {
                PotionCardRewardKind::Attack => FairSelectionKind::PotionAttackReward,
                PotionCardRewardKind::Skill => FairSelectionKind::PotionSkillReward,
                PotionCardRewardKind::Power => FairSelectionKind::PotionPowerReward,
                PotionCardRewardKind::Colorless => FairSelectionKind::PotionColorlessReward,
            },
            options: ordered_options(choices, corruption_active)?,
            selected_slots: Vec::new(),
        },
        CombatDecisionState::ToolboxCardReward { choices } => FairSelection {
            kind: FairSelectionKind::ToolboxReward,
            options: ordered_options(choices, corruption_active)?,
            selected_slots: Vec::new(),
        },
        CombatDecisionState::NilrysCodexCardReward { choices } => FairSelection {
            // Same transport as Toolbox/Discovery card-reward overlays.
            kind: FairSelectionKind::ToolboxReward,
            options: ordered_options(choices, corruption_active)?,
            selected_slots: Vec::new(),
        },
        CombatDecisionState::DiscoveryCardReward { choices, .. } => FairSelection {
            kind: FairSelectionKind::DiscoveryReward,
            options: ordered_options(choices, corruption_active)?,
            selected_slots: Vec::new(),
        },
        CombatDecisionState::HandSelect { state, .. } => selection_from_source_indices(
            hand_selection_kind(state.purpose),
            &combat.piles.hand,
            mapped_source_indices(combat.piles.hand.len(), |ui_index| {
                crate::combat::transition::hand_select_ui_to_hand_index(combat, ui_index)
            })?,
            canonical_slot_set(
                state
                    .selected_hand_indices
                    .iter()
                    .copied()
                    .chain(state.selected_hand_index),
            ),
            false,
            corruption_active,
        )?,
        CombatDecisionState::DrawSelect { state } => selection_from_source_indices(
            draw_selection_kind(state.purpose),
            &combat.piles.draw_pile,
            mapped_source_indices(combat.piles.draw_pile.len(), |ui_index| {
                crate::combat::transition::draw_select_ui_to_draw_index(combat, ui_index)
            })?,
            canonical_slot_set(
                state
                    .selected_draw_indices
                    .iter()
                    .copied()
                    .chain(state.selected_draw_index),
            ),
            true,
            corruption_active,
        )?,
        CombatDecisionState::DiscardSelect { state } => selection_from_source_indices(
            discard_selection_kind(state.purpose),
            &combat.piles.discard_pile,
            mapped_source_indices(combat.piles.discard_pile.len(), |ui_index| {
                crate::combat::transition::discard_select_ui_to_discard_index(combat, ui_index)
            })?,
            canonical_slot_set(
                state
                    .selected_discard_indices
                    .iter()
                    .copied()
                    .chain(state.selected_discard_index),
            ),
            true,
            corruption_active,
        )?,
        CombatDecisionState::ExhaustSelect { state } => {
            let mut mapping_combat = combat.clone();
            mapping_combat
                .exhaust_select_mut()
                .expect("matched exhaust selection")
                .selected_hand_indices
                .clear();
            let cards = if state.purpose == ExhaustSelectPurpose::ExhumeReturnToHand {
                &combat.piles.exhaust_pile
            } else {
                &combat.piles.hand
            };
            selection_from_source_indices(
                exhaust_selection_kind(state.purpose),
                cards,
                mapped_source_indices(cards.len(), |ui_index| {
                    crate::combat::transition::exhaust_select_ui_to_hand_index(
                        &mapping_combat,
                        ui_index,
                    )
                })?,
                canonical_slot_set(state.selected_hand_indices.iter().copied()),
                state.purpose == ExhaustSelectPurpose::ExhumeReturnToHand,
                corruption_active,
            )?
        }
    };
    Ok(Some(selection))
}

fn ordered_options(
    cards: &[CardInstance],
    corruption_active: bool,
) -> Result<Vec<FairSelectionOption>, FairObservationError> {
    cards
        .iter()
        .enumerate()
        .map(|(slot, card)| {
            Ok(FairSelectionOption {
                slot,
                card: project_card(card, corruption_active)?,
            })
        })
        .collect()
}

fn selection_from_source_indices(
    kind: FairSelectionKind,
    cards: &[CardInstance],
    source_indices: Vec<usize>,
    selected_source_indices: Vec<usize>,
    canonicalize: bool,
    corruption_active: bool,
) -> Result<FairSelection, FairObservationError> {
    let mut indexed = source_indices
        .into_iter()
        .map(|source_index| {
            let card = cards
                .get(source_index)
                .ok_or(FairObservationError::InvalidAuthoritativeState)?;
            Ok((source_index, project_card(card, corruption_active)?))
        })
        .collect::<Result<Vec<_>, FairObservationError>>()?;
    if canonicalize {
        indexed.sort_by(|left, right| left.1.cmp(&right.1));
    }
    let selected_slots = if canonicalize {
        // Multiple source cards can have the same public representation. Match
        // selected cards by public value and multiplicity, rather than letting
        // the stable sort preserve a hidden pile index as a tie-breaker.
        let mut selected_counts = BTreeMap::<FairCard, usize>::new();
        for source_index in selected_source_indices {
            let card = indexed
                .iter()
                .find(|(candidate_index, _)| *candidate_index == source_index)
                .map(|(_, card)| card)
                .ok_or(FairObservationError::InvalidAuthoritativeState)?;
            *selected_counts.entry(card.clone()).or_default() += 1;
        }

        let mut selected_slots = Vec::new();
        for (slot, (_, card)) in indexed.iter().enumerate() {
            let Some(remaining) = selected_counts.get_mut(card) else {
                continue;
            };
            if *remaining > 0 {
                selected_slots.push(slot);
                *remaining -= 1;
            }
        }
        if selected_counts.values().any(|remaining| *remaining != 0) {
            return Err(FairObservationError::InvalidAuthoritativeState);
        }
        selected_slots
    } else {
        indexed
            .iter()
            .enumerate()
            .filter_map(|(slot, (source_index, _))| {
                selected_source_indices
                    .contains(source_index)
                    .then_some(slot)
            })
            .collect()
    };
    let options = indexed
        .into_iter()
        .enumerate()
        .map(|(slot, (_, card))| FairSelectionOption { slot, card })
        .collect();
    Ok(FairSelection {
        kind,
        options,
        selected_slots,
    })
}

fn mapped_source_indices(
    maximum_count: usize,
    mut map: impl FnMut(usize) -> crate::SimResult<usize>,
) -> Result<Vec<usize>, FairObservationError> {
    let mut source_indices = Vec::new();
    for ui_index in 0..maximum_count {
        match map(ui_index) {
            Ok(source_index) => source_indices.push(source_index),
            Err(crate::SimError::IllegalAction(message))
                if message.ends_with("index out of range") =>
            {
                break
            }
            Err(crate::SimError::UnknownContent(content_id)) => {
                return Err(unmodeled_or_unknown_public_content(content_id));
            }
            Err(_) => return Err(FairObservationError::InvalidAuthoritativeState),
        }
    }
    Ok(source_indices)
}

fn canonical_slot_set(slots: impl IntoIterator<Item = usize>) -> Vec<usize> {
    let mut slots = slots.into_iter().collect::<Vec<_>>();
    slots.sort_unstable();
    slots.dedup();
    slots
}

fn hand_selection_kind(purpose: HandSelectPurpose) -> FairSelectionKind {
    match purpose {
        HandSelectPurpose::WarcryPutOnDraw => FairSelectionKind::WarcryPutOnDraw,
        HandSelectPurpose::ArmamentsUpgrade => FairSelectionKind::ArmamentsUpgrade,
        HandSelectPurpose::ForethoughtPutOnDraw => FairSelectionKind::ForethoughtPutOnDraw,
        HandSelectPurpose::ForethoughtPutAnyOnDraw => FairSelectionKind::ForethoughtPutAnyOnDraw,
        HandSelectPurpose::ThinkingAheadPutOnDraw => FairSelectionKind::ThinkingAheadPutOnDraw,
        HandSelectPurpose::PreparedDiscard => FairSelectionKind::PreparedDiscard,
        HandSelectPurpose::DualWieldCopy => FairSelectionKind::DualWieldCopy,
    }
}

fn draw_selection_kind(purpose: DrawSelectPurpose) -> FairSelectionKind {
    match purpose {
        DrawSelectPurpose::SecretTechniqueSkillToHand => {
            FairSelectionKind::SecretTechniqueSkillToHand
        }
        DrawSelectPurpose::SecretWeaponAttackToHand => FairSelectionKind::SecretWeaponAttackToHand,
        DrawSelectPurpose::Scry => FairSelectionKind::Scry,
    }
}

fn discard_selection_kind(purpose: DiscardSelectPurpose) -> FairSelectionKind {
    match purpose {
        DiscardSelectPurpose::LiquidMemoriesReturnToHand => {
            FairSelectionKind::LiquidMemoriesReturnToHand
        }
        DiscardSelectPurpose::HeadbuttPutOnDraw => FairSelectionKind::HeadbuttPutOnDraw,
        DiscardSelectPurpose::HologramReturnToHand => FairSelectionKind::HologramReturnToHand,
    }
}

fn exhaust_selection_kind(purpose: ExhaustSelectPurpose) -> FairSelectionKind {
    match purpose {
        ExhaustSelectPurpose::Exhaust => FairSelectionKind::Exhaust,
        ExhaustSelectPurpose::GamblingChip => FairSelectionKind::GamblingChip,
        ExhaustSelectPurpose::ExhumeReturnToHand => FairSelectionKind::ExhumeReturnToHand,
        ExhaustSelectPurpose::PurityExhaustUpTo3 => FairSelectionKind::PurityExhaustUpToThree,
        ExhaustSelectPurpose::BurningPactDraw2 => FairSelectionKind::BurningPactDrawTwo,
        ExhaustSelectPurpose::BurningPactDraw3 => FairSelectionKind::BurningPactDrawThree,
        ExhaustSelectPurpose::TrueGritExhaustOne => FairSelectionKind::TrueGritExhaustOne,
        ExhaustSelectPurpose::RecycleExhaustOne => FairSelectionKind::Exhaust,
    }
}

pub fn potion_key(potion: Potion) -> &'static str {
    match potion {
        Potion::Fire => "fire",
        Potion::Block => "block",
        Potion::Fear => "fear",
        Potion::GamblersBrew => "gamblers_brew",
        Potion::Blood => "blood",
        Potion::Elixir => "elixir",
        Potion::HeartOfIron => "heart_of_iron",
        Potion::Dexterity => "dexterity",
        Potion::Energy => "energy",
        Potion::Explosive => "explosive",
        Potion::Strength => "strength",
        Potion::Swift => "swift",
        Potion::Weak => "weak",
        Potion::Attack => "attack",
        Potion::Skill => "skill",
        Potion::Power => "power",
        Potion::Colorless => "colorless",
        Potion::Flex => "flex",
        Potion::Speed => "speed",
        Potion::BlessingOfTheForge => "blessing_of_the_forge",
        Potion::Regen => "regen",
        Potion::Ancient => "ancient",
        Potion::LiquidBronze => "liquid_bronze",
        Potion::EssenceOfSteel => "essence_of_steel",
        Potion::Duplication => "duplication",
        Potion::DistilledChaos => "distilled_chaos",
        Potion::LiquidMemories => "liquid_memories",
        Potion::Cultist => "cultist",
        Potion::FruitJuice => "fruit_juice",
        Potion::SneckoOil => "snecko_oil",
        Potion::Fairy => "fairy_in_a_bottle",
        Potion::SmokeBomb => "smoke_bomb",
        Potion::EntropicBrew => "entropic_brew",
    }
}

fn nonzero(value: i32) -> Option<i32> {
    (value != 0).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        content::cards::{
            BASH_ID, DEFEND_R_ID, DUAL_WIELD_ID, INFLAME_ID, STRIKE_R_ID,
            WINDMILL_STRIKE_ANY_COLOR_ID,
        },
        content::monsters::{monster_state, BRONZE_ORB_A0, GUARDIAN_A0, LOOTER_A0},
        CardId, CardInstance, MonsterId, StsRng,
    };

    fn observation(run: &RunState) -> FairCombatObservation {
        fair_combat_observation(run).expect("fixture projects")
    }

    fn observation_bytes(run: &RunState) -> Vec<u8> {
        serde_json::to_vec(&observation(run)).expect("observation serializes")
    }

    fn assert_hidden_equivalent(label: &str, left: &RunState, right: &RunState) {
        assert_eq!(
            observation_bytes(left),
            observation_bytes(right),
            "hidden-state mutation changed fair observation: {label}"
        );
    }

    fn assert_public_change(label: &str, baseline: &RunState, changed: &RunState) {
        assert_ne!(
            observation_bytes(baseline),
            observation_bytes(changed),
            "public-state mutation did not change fair observation: {label}"
        );
    }

    fn reorder_cards(cards: &[CardInstance], order: &[usize]) -> Vec<CardInstance> {
        order.iter().map(|index| cards[*index]).collect()
    }

    #[test]
    fn hidden_draw_order_rng_and_internal_ids_do_not_change_public_bytes() {
        let mut left = RunState::combat_fixture();
        let combat = left.combat.as_mut().expect("combat");
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(101), STRIKE_R_ID),
            CardInstance::new(CardId::new(102), DEFEND_R_ID),
            CardInstance::new(CardId::new(103), BASH_ID),
        ];
        combat.piles.discard_pile = vec![
            CardInstance::new(CardId::new(104), STRIKE_R_ID),
            CardInstance::new(CardId::new(105), BASH_ID),
        ];
        combat.piles.exhaust_pile = vec![
            CardInstance::new(CardId::new(106), DEFEND_R_ID),
            CardInstance::new(CardId::new(107), STRIKE_R_ID),
        ];

        let mut right = left.clone();
        let combat = right.combat.as_mut().expect("combat");
        combat.piles.draw_pile.reverse();
        combat.piles.discard_pile.reverse();
        combat.piles.exhaust_pile.reverse();
        for (index, card) in combat
            .piles
            .hand
            .iter_mut()
            .chain(combat.piles.draw_pile.iter_mut())
            .chain(combat.piles.discard_pile.iter_mut())
            .chain(combat.piles.exhaust_pile.iter_mut())
            .enumerate()
        {
            card.id = CardId::new(8_000 + index as u64);
        }
        combat.monsters[0].id = MonsterId::new(9_000);
        combat.rng.shuffle_rng = StsRng::with_counter(123, 17);
        combat.rng.monster_rng = StsRng::with_counter(456, 18);
        combat.rng.monster_hp_rng = StsRng::with_counter(789, 19);
        combat.rng.card_random_rng = StsRng::with_counter(999, 20);

        assert_hidden_equivalent("pile order, IDs, and RNG", &left, &right);
    }

    #[test]
    fn every_hidden_pile_permutation_has_the_same_public_bytes() {
        let mut base = RunState::combat_fixture();
        let combat = base.combat.as_mut().expect("combat");
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(101), STRIKE_R_ID),
            CardInstance::new(CardId::new(102), DEFEND_R_ID),
            CardInstance::new(CardId::new(103), BASH_ID),
        ];
        combat.piles.discard_pile = vec![
            CardInstance::new(CardId::new(104), STRIKE_R_ID),
            CardInstance::new(CardId::new(105), DEFEND_R_ID),
            CardInstance::new(CardId::new(106), BASH_ID),
        ];
        combat.piles.exhaust_pile = vec![
            CardInstance::new(CardId::new(107), STRIKE_R_ID),
            CardInstance::new(CardId::new(108), DEFEND_R_ID),
            CardInstance::new(CardId::new(109), BASH_ID),
        ];
        let base_combat = base.combat.as_ref().expect("combat");
        let permutations = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];

        for order in permutations {
            let mut variant = base.clone();
            let combat = variant.combat.as_mut().expect("combat");
            combat.piles.draw_pile = reorder_cards(&base_combat.piles.draw_pile, &order);
            combat.piles.discard_pile = reorder_cards(&base_combat.piles.discard_pile, &order);
            combat.piles.exhaust_pile = reorder_cards(&base_combat.piles.exhaust_pile, &order);
            assert_hidden_equivalent("all hidden pile permutations", &base, &variant);
        }
    }

    #[test]
    fn frozen_eye_reveals_top_to_bottom_draw_order() {
        let mut hidden = RunState::combat_fixture();
        let combat = hidden.combat.as_mut().expect("combat");
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(101), STRIKE_R_ID),
            CardInstance::new(CardId::new(102), DEFEND_R_ID),
            CardInstance::new(CardId::new(103), BASH_ID),
        ];
        assert!(observation(&hidden).draw_pile.known_order.is_empty());

        let mut revealed = hidden.clone();
        revealed.relics.push(Relic::FrozenEye);
        revealed
            .combat
            .as_mut()
            .expect("combat")
            .relics
            .push(Relic::FrozenEye);
        let projected = observation(&revealed);
        let keys = projected
            .draw_pile
            .known_order
            .iter()
            .map(|card| card.content_key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(keys, vec!["Bash", "Defend_R", "Strike_R"]);

        let mut permuted = revealed.clone();
        permuted
            .combat
            .as_mut()
            .expect("combat")
            .piles
            .draw_pile
            .reverse();
        assert_ne!(observation_bytes(&revealed), observation_bytes(&permuted));
    }

    #[test]
    fn runic_dome_hides_every_intent_field() {
        let visible = RunState::combat_fixture();
        let FairMonsterIntent::Visible {
            damage: visible_damage,
            ..
        } = &observation(&visible).monsters[0].intent
        else {
            panic!("fixture intent should be visible");
        };
        assert!(visible_damage.is_some());

        let mut first = visible.clone();
        first.relics.push(Relic::RunicDome);
        first
            .combat
            .as_mut()
            .expect("combat")
            .relics
            .push(Relic::RunicDome);
        let mut second = first.clone();
        second.combat.as_mut().expect("combat").monsters[0].intent =
            MonsterIntent::AttackMultiple {
                damage: 999,
                hits: 999,
            };

        assert_hidden_equivalent("Runic Dome hidden intent", &first, &second);
        assert_eq!(
            observation(&first).monsters[0].intent,
            FairMonsterIntent::Hidden
        );
    }

    #[test]
    fn visible_intent_reports_modified_damage_and_hit_count() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat");
        combat.player.powers.vulnerable = 1;
        combat.monsters[0].powers.strength = 2;
        combat.monsters[0].powers.weak = 1;
        combat.monsters[0].intent = MonsterIntent::AttackMultiple {
            damage: 10,
            hits: 3,
        };

        assert_eq!(
            observation(&run).monsters[0].intent,
            FairMonsterIntent::Visible {
                category: FairIntentCategory::Attack,
                damage: Some(13),
                hits: Some(3),
            }
        );
    }

    #[test]
    fn visible_power_totals_keep_their_public_turn_end_components() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat");
        combat.player.powers.strength = 2;
        combat.player.temp_strength = 3;
        combat.player.powers.thorns = 1;
        combat.player.temp_thorns = 4;
        combat.monsters[0].powers.strength = -2;
        combat.monsters[0].temp_strength_down = 3;

        let projected = observation(&run);
        let player_power = |key: &str| {
            projected
                .player
                .powers
                .iter()
                .find(|power| power.key == key)
                .map(|power| power.amount)
        };
        assert_eq!(player_power("strength"), Some(5));
        assert_eq!(player_power("lose_strength"), Some(3));
        assert_eq!(player_power("thorns"), Some(5));
        assert_eq!(player_power("temporary_thorns"), Some(4));
        assert!(projected.monsters[0].powers.contains(&FairPower {
            key: "restore_strength".to_owned(),
            amount: 3,
        }));
    }

    #[test]
    fn private_monster_and_relic_counters_are_not_projected() {
        let left = RunState::combat_fixture();
        let mut right = left.clone();
        let combat = right.combat.as_mut().expect("combat");
        combat.monsters[0].move_history = vec![1, 2, 3];
        combat.monsters[0].rolled_attack_damage = Some(999);
        combat.monsters[0].powers.book_stab_count = 99;
        combat.monsters[0].powers.flight_grounding_pending = true;
        combat.relic_counters.fairy_heal_percent = 99;
        combat.relic_counters.fairy_consumed = true;
        combat.relic_counters.necronomicon_used_this_turn = true;
        combat.pending_player_spikes_damage = 99;
        combat.pending_start_of_turn_relic_energy = 99;
        combat.pending_monster_death_relic_triggers = 99;
        combat
            .piles
            .limbo
            .push(CardInstance::new(CardId::new(7_777), BASH_ID));
        combat
            .queued_decisions
            .push_back(CombatDecisionState::ToolboxCardReward {
                choices: vec![CardInstance::new(CardId::new(7_778), STRIKE_R_ID)],
            });

        assert_hidden_equivalent("private counters, queues, and limbo", &left, &right);
    }

    #[test]
    fn public_orb_slots_poison_and_windmill_damage_are_projected() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat");
        combat.max_orbs = 3;
        combat.orbs = vec![CombatOrb::Lightning, CombatOrb::Dark { evoke: 24 }];
        combat.monsters[0].powers.poison = 5;
        combat.monsters[0].powers.lock_on = 2;
        combat.piles.hand = vec![CardInstance::new(
            CardId::new(100),
            WINDMILL_STRIKE_ANY_COLOR_ID,
        )];
        combat.piles.hand[0].windmill_retain_damage = 8;

        let projected = observation(&run);
        assert_eq!(projected.schema_version, 2);
        assert_eq!(
            projected.orb_slots,
            vec![
                FairOrbSlot {
                    slot: 0,
                    orb: Some(FairOrb::Lightning),
                },
                FairOrbSlot {
                    slot: 1,
                    orb: Some(FairOrb::Dark { evoke: 24 }),
                },
                FairOrbSlot { slot: 2, orb: None },
            ]
        );
        assert!(projected.monsters[0].powers.contains(&FairPower {
            key: "poison".to_owned(),
            amount: 5,
        }));
        assert!(projected.monsters[0].powers.contains(&FairPower {
            key: "lock_on".to_owned(),
            amount: 2,
        }));
        assert_eq!(
            projected.hand[0].card.dynamic.windmill_retain_damage,
            Some(8)
        );
    }

    #[test]
    fn fair_projection_rejects_invalid_orb_contracts() {
        let mut occupied = RunState::combat_fixture();
        occupied.combat.as_mut().expect("combat").orbs = vec![CombatOrb::Frost];
        assert_eq!(
            fair_combat_observation(&occupied),
            Err(FairObservationError::InvalidAuthoritativeState)
        );

        let mut negative_dark = RunState::combat_fixture();
        let combat = negative_dark.combat.as_mut().expect("combat");
        combat.max_orbs = 1;
        combat.orbs = vec![CombatOrb::Dark { evoke: -1 }];
        assert_eq!(
            fair_combat_observation(&negative_dark),
            Err(FairObservationError::InvalidAuthoritativeState)
        );
    }

    #[test]
    fn public_cards_potions_relics_and_counters_are_projected() {
        let mut run = RunState::combat_fixture_with_relics(vec![Relic::InkBottle]);
        run.potions = vec![Potion::Fire, Potion::Block];
        run.empty_potion_slots = vec![1];
        let combat = run.combat.as_mut().expect("combat");
        combat.relic_counters.ink_bottle_cards_played = 7;
        combat.relic_counters.cards_played_this_turn = 2;
        combat.piles.hand[0].content_id = crate::content::cards::STEAM_BARRIER_ANY_COLOR_ID;
        combat.piles.hand[0].steam_barrier_block_reduction = 2;
        combat.piles.hand[0].temp_cost = Some(0);
        combat.piles.hand[0].temp_cost_turn_only = true;
        combat.piles.hand[0].combat_cost_under_turn_override = Some(1);
        combat.total_discarded_this_turn = 3;

        let projected = observation(&run);
        assert_eq!(projected.hand[0].card.cost, 0);
        assert_eq!(
            projected.hand[0].card.dynamic.steam_barrier_block_reduction,
            Some(2)
        );
        assert_eq!(
            projected.hand[0]
                .card
                .dynamic
                .combat_cost_under_turn_override,
            Some(1)
        );
        assert_eq!(projected.relics[0].content_key, "Ink Bottle");
        assert_eq!(projected.relics[0].state[0].value, 7);
        assert_eq!(
            projected
                .potion_slots
                .iter()
                .map(|slot| slot.content_key.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("fire"), None, Some("block")]
        );
        assert_eq!(projected.public_counters[0].value, 2);
        assert_eq!(
            projected
                .public_counters
                .iter()
                .find(|counter| counter.key == "cards_discarded_this_turn")
                .map(|counter| counter.value),
            Some(3)
        );
    }

    #[test]
    fn public_run_level_relic_state_is_projected() {
        let relics = vec![
            Relic::Omamori,
            Relic::MawBank,
            Relic::AncientTeaSet,
            Relic::Girya,
            Relic::Matryoshka,
            Relic::TinyChest,
            Relic::WingBoots,
            Relic::NeowsLament,
        ];
        let mut run = RunState::combat_fixture_with_relics(relics);
        run.omamori_charges_used = 1;
        run.maw_bank_broken = true;
        run.ancient_tea_set_armed = true;
        run.girya_lifts = 2;
        run.matryoshka_chests_opened = 1;
        run.tiny_chest_counter = 3;
        run.wing_boots_charges = 2;
        run.neow_lament_combats_remaining = 1;

        let projected = observation(&run);
        let value = |relic_key: &str, state_key: &str| {
            projected
                .relics
                .iter()
                .find(|relic| relic.content_key == relic_key)
                .and_then(|relic| relic.state.iter().find(|state| state.key == state_key))
                .map(|state| state.value)
                .unwrap_or_else(|| panic!("missing {relic_key}.{state_key}"))
        };
        assert_eq!(value("Omamori", "charges_remaining"), 1);
        assert_eq!(value("Maw Bank", "active"), 0);
        assert_eq!(value("Ancient Tea Set", "armed"), 1);
        assert_eq!(value("Girya", "lifts"), 2);
        assert_eq!(value("Matryoshka", "chests_remaining"), 1);
        assert_eq!(value("Tiny Chest", "rooms"), 3);
        assert_eq!(value("Wing Boots", "charges"), 2);
        assert_eq!(value("Neow's Lament", "combats_remaining"), 1);
    }

    #[test]
    fn public_monster_mode_stolen_gold_and_stasis_card_are_projected_without_ids() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat");
        let mut guardian = monster_state(&GUARDIAN_A0, MonsterId::new(41));
        guardian.mode_shift = 17;
        let mut looter = monster_state(&LOOTER_A0, MonsterId::new(42));
        looter.stolen_gold = 15;
        let mut orb = monster_state(&BRONZE_ORB_A0, MonsterId::new(43));
        orb.stasis_card = Some(CardInstance::new(CardId::new(8_888), BASH_ID));
        combat.monsters = vec![guardian, looter, orb];

        let projected = observation(&run);
        assert!(projected.monsters[0].powers.contains(&FairPower {
            key: "mode_shift".to_owned(),
            amount: 17,
        }));
        assert_eq!(projected.monsters[1].stolen_gold, 15);
        assert_eq!(
            projected.monsters[2]
                .stasis_card
                .as_ref()
                .expect("public stasis card")
                .content_key,
            "Bash"
        );

        let mut renumbered = run.clone();
        renumbered.combat.as_mut().expect("combat").monsters[2]
            .stasis_card
            .as_mut()
            .expect("stasis card")
            .id = CardId::new(9_999);
        assert_hidden_equivalent("stasis card instance ID", &run, &renumbered);
    }

    #[test]
    fn canonical_selection_does_not_reveal_source_pile_order() {
        let mut left = RunState::combat_fixture();
        let combat = left.combat.as_mut().expect("combat");
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(101), STRIKE_R_ID),
            CardInstance::new(CardId::new(102), DEFEND_R_ID),
            CardInstance::new(CardId::new(103), BASH_ID),
        ];
        combat.decision = Some(CombatDecisionState::DrawSelect {
            state: crate::combat::DrawSelectState {
                purpose: DrawSelectPurpose::SecretWeaponAttackToHand,
                source_card_id: CardId::new(777),
                selectable_card_ids: Vec::new(),
                selected_draw_index: None,
                selected_draw_indices: Vec::new(),
                pending_actions: Default::default(),
            },
        });
        let mut right = left.clone();
        right
            .combat
            .as_mut()
            .expect("combat")
            .piles
            .draw_pile
            .reverse();

        assert_hidden_equivalent("canonical selection pile order", &left, &right);
        let keys = observation(&left)
            .selection
            .expect("draw selection")
            .options
            .into_iter()
            .map(|option| option.card.content_key)
            .collect::<Vec<_>>();
        assert_eq!(keys, vec!["Bash", "Strike_R"]);
    }

    #[test]
    fn canonical_selection_normalizes_selected_duplicate_cards() {
        let mut left = RunState::combat_fixture();
        let combat = left.combat.as_mut().expect("combat");
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(101), STRIKE_R_ID),
            CardInstance::new(CardId::new(102), STRIKE_R_ID),
            CardInstance::new(CardId::new(103), DEFEND_R_ID),
        ];
        combat.decision = Some(CombatDecisionState::DrawSelect {
            state: crate::combat::DrawSelectState {
                purpose: DrawSelectPurpose::SecretWeaponAttackToHand,
                source_card_id: CardId::new(777),
                selectable_card_ids: Vec::new(),
                selected_draw_index: Some(0),
                selected_draw_indices: Vec::new(),
                pending_actions: Default::default(),
            },
        });

        let mut right = left.clone();
        right
            .combat
            .as_mut()
            .expect("combat")
            .piles
            .draw_pile
            .reverse();
        if let Some(CombatDecisionState::DrawSelect { state }) =
            right.combat.as_mut().expect("combat").decision.as_mut()
        {
            state.selected_draw_index = Some(2);
        } else {
            panic!("draw selection");
        }

        assert_hidden_equivalent("canonical duplicate selection identity", &left, &right);
        assert_eq!(
            observation(&left)
                .selection
                .expect("draw selection")
                .selected_slots,
            vec![0]
        );
    }

    #[test]
    fn invalid_selection_mapping_returns_a_coarse_error() {
        let mut run = RunState::combat_fixture();
        run.combat.as_mut().expect("combat").decision = Some(CombatDecisionState::ExhaustSelect {
            state: crate::combat::ExhaustSelectState {
                purpose: ExhaustSelectPurpose::PurityExhaustUpTo3,
                source_card_id: None,
                source_card: None,
                source_card_force_exhaust: false,
                selected_hand_indices: Vec::new(),
                interrupted_by_cultist_potion: false,
                pending_actions: Default::default(),
            },
        });

        assert_eq!(
            fair_combat_observation(&run),
            Err(FairObservationError::InvalidAuthoritativeState)
        );
    }

    #[test]
    fn hand_selection_reuses_core_candidate_filtering_without_exposing_source_id() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat");
        combat.piles.hand = vec![
            CardInstance::new(CardId::new(201), DUAL_WIELD_ID),
            CardInstance::new(CardId::new(202), STRIKE_R_ID),
            CardInstance::new(CardId::new(203), DEFEND_R_ID),
            CardInstance::new(CardId::new(204), INFLAME_ID),
        ];
        combat.decision = Some(CombatDecisionState::HandSelect {
            state: crate::combat::HandSelectState {
                purpose: HandSelectPurpose::DualWieldCopy,
                source_card_id: CardId::new(201),
                selected_hand_index: None,
                selected_hand_indices: Vec::new(),
                dual_wield_restore_on_confirm: Vec::new(),
                dual_wield_force_exhaust: false,
            },
            pending_actions: Default::default(),
        });

        let selection = observation(&run).selection.expect("hand selection");
        assert_eq!(
            selection
                .options
                .iter()
                .map(|option| (option.slot, option.card.content_key.as_str()))
                .collect::<Vec<_>>(),
            vec![(0, "Strike_R"), (1, "Inflame")]
        );

        let mut renumbered = run.clone();
        let combat = renumbered.combat.as_mut().expect("combat");
        combat.piles.hand[0].id = CardId::new(9_201);
        let Some(CombatDecisionState::HandSelect { state, .. }) = combat.decision.as_mut() else {
            panic!("hand selection");
        };
        state.source_card_id = CardId::new(9_201);
        assert_hidden_equivalent("hand source-card ID", &run, &renumbered);
    }

    #[test]
    fn projection_is_deterministic_and_side_effect_free() {
        let run = RunState::combat_fixture();
        let before = run.clone();
        let first = observation_bytes(&run);
        let second = observation_bytes(&run);
        assert_eq!(first, second);
        assert_eq!(run, before);
    }

    #[test]
    fn public_state_changes_are_observable() {
        let baseline = RunState::combat_fixture();

        let mut hp_changed = baseline.clone();
        hp_changed.combat.as_mut().expect("combat").player.hp -= 1;
        assert_public_change("player hp", &baseline, &hp_changed);

        let mut hand_reordered = baseline.clone();
        hand_reordered
            .combat
            .as_mut()
            .expect("combat")
            .piles
            .hand
            .swap(0, 1);
        assert_public_change("hand order", &baseline, &hand_reordered);

        let mut pile_membership_changed = baseline.clone();
        pile_membership_changed
            .combat
            .as_mut()
            .expect("combat")
            .piles
            .draw_pile[0]
            .content_id = DEFEND_R_ID;
        assert_public_change("draw-pile membership", &baseline, &pile_membership_changed);

        let mut intent_changed = baseline.clone();
        intent_changed.combat.as_mut().expect("combat").monsters[0].intent =
            MonsterIntent::Attack { damage: 42 };
        assert_public_change("visible monster intent", &baseline, &intent_changed);

        let mut gold_changed = baseline.clone();
        gold_changed.gold += 1;
        assert_public_change("run gold", &baseline, &gold_changed);
    }

    #[test]
    fn hidden_equivalent_errors_have_the_same_coarse_category() {
        let mut left = RunState::combat_fixture();
        left.combat.as_mut().expect("combat").piles.hand[0].content_id =
            crate::ContentId::new(9_999_999);

        let mut right = left.clone();
        let combat = right.combat.as_mut().expect("combat");
        combat.piles.draw_pile.reverse();
        combat.piles.discard_pile.reverse();
        combat.rng.shuffle_rng = StsRng::with_counter(123, 17);
        combat.rng.monster_rng = StsRng::with_counter(456, 18);
        combat.monsters[0].move_history = vec![4, 5, 6];

        assert_eq!(
            fair_combat_observation(&left),
            fair_combat_observation(&right)
        );
        assert_eq!(
            fair_combat_observation(&left),
            Err(FairObservationError::UnknownPublicContent)
        );
    }

    #[test]
    fn public_errors_do_not_embed_internal_ids() {
        let idle = RunState::map_fixture();
        assert_eq!(
            fair_combat_observation(&idle),
            Err(FairObservationError::NoActiveCombat)
        );

        let mut missing_combat = RunState::map_fixture();
        missing_combat.phase = RunPhase::Combat;
        assert_eq!(
            fair_combat_observation(&missing_combat),
            Err(FairObservationError::InvalidAuthoritativeState)
        );

        let mut unknown = RunState::combat_fixture();
        unknown.combat.as_mut().expect("combat").piles.hand[0].content_id =
            crate::ContentId::new(9_999_999);
        let error = fair_combat_observation(&unknown).expect_err("unknown content fails");
        assert_eq!(error, FairObservationError::UnknownPublicContent);
        assert!(!error.to_string().contains("9999999"));
    }

    #[test]
    fn unmodeled_prismatic_pool_cards_fail_without_fabricating_cost() {
        let synthetic = crate::content::shop_pool::shop_card_content_id("FLYING_KNEE");
        assert!(crate::content::cards::get_card_definition(synthetic).is_none());
        assert_eq!(
            crate::run::reward::any_color_reward_card_key(synthetic),
            Some("FLYING_KNEE")
        );

        let mut run = RunState::combat_fixture();
        run.combat.as_mut().expect("combat").piles.hand[0] =
            CardInstance::new(CardId::new(1), synthetic);

        let error =
            fair_combat_observation(&run).expect_err("unmodeled pool content is not projected");
        assert_eq!(
            error,
            FairObservationError::UnmodeledPublicContent("FLYING_KNEE")
        );
        assert_eq!(
            error.to_string(),
            "public combat content is unmodeled: FLYING_KNEE"
        );
    }

    #[test]
    fn serialized_schema_contains_no_internal_identity_or_rng_fields() {
        let run = RunState::combat_fixture();
        let json = serde_json::to_string(&observation(&run)).expect("observation serializes");
        for forbidden in [
            "card_id",
            "monster_id",
            "content_id",
            "source_card_id",
            "rng",
            "move_history",
            "rolled_attack_damage",
            "queued_decisions",
            "pending_actions",
        ] {
            assert!(
                !json.contains(forbidden),
                "leaked field {forbidden}: {json}"
            );
        }
    }
}
