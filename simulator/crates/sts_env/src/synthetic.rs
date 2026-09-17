//! Explicit A0 training inputs. Never used to import observations or repair replay.

use serde::Deserialize;
use sts_core::adapter_internals::{
    content::cards::{public_card_definitions, upgrade_card_instance},
    run::{map::enter_synthetic_combat, state::relic_pickup_energy},
    CardId, CardInstance, CardType, Relic, RoomKind, RunPhase, RunState, ALL_RELICS,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyntheticCard {
    pub key: String,
    pub upgrades: u32,
    #[serde(default)]
    pub ritual_dagger_damage_bonus: i32,
}

/// Zero counters and an unused Lizard Tail are an explicit synthetic prior.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SyntheticCounters {
    pub incense_burner: u32,
    pub pen_nib: u32,
    pub ink_bottle: u32,
    pub happy_flower: u32,
    pub sundial: u32,
    pub nunchaku: u32,
    pub lizard_tail_used: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyntheticCombatSpec {
    pub seed: u64,
    pub floor: i32,
    pub kind: String,
    pub encounter: String,
    pub deck: Vec<SyntheticCard>,
    pub relics: Vec<String>,
    pub potions: Vec<Option<String>>,
    pub hp: i32,
    pub max_hp: i32,
    pub gold: i32,
    #[serde(default)]
    pub counters: SyntheticCounters,
}

pub(crate) fn build(spec: SyntheticCombatSpec) -> Result<RunState, String> {
    let act = match spec.floor {
        1..=16 => 1,
        18..=33 => 2,
        35..=50 => 3,
        54..=55 => 4,
        _ => return Err("not a combat-capable A0 floor".into()),
    };
    if matches!(spec.floor, 9 | 15 | 26 | 32 | 43 | 49)
        || spec.hp <= 0
        || spec.hp > spec.max_hp
        || spec.max_hp > 1000
        || spec.gold < 0
        || spec.deck.is_empty()
        || spec.deck.len() > 100
    {
        return Err("invalid synthetic floor, HP, gold, or deck size".into());
    }
    let kind = match spec.kind.as_str() {
        "normal" => RoomKind::Combat,
        "elite" => RoomKind::Elite,
        "boss" => RoomKind::Boss,
        _ => return Err("unknown combat kind".into()),
    };
    if (kind == RoomKind::Boss) != matches!(spec.floor, 16 | 33 | 50 | 55)
        || (spec.floor == 54 && kind != RoomKind::Elite)
    {
        return Err("combat kind does not match floor".into());
    }
    let mut run = RunState::try_seeded_ironclad(spec.seed, 0).map_err(|e| e.to_string())?;
    run.phase = RunPhase::Idle;
    run.event = None;
    run.map = None;
    run.emerald_key_node = None;
    run.current_act = act;
    run.current_floor = spec.floor;
    run.hp = spec.hp;
    run.max_hp = spec.max_hp;
    run.gold = spec.gold;
    run.relics.clear();
    for key in spec.relics {
        let relic = ALL_RELICS
            .iter()
            .copied()
            .find(|r| r.trace_name() == key)
            .ok_or_else(|| format!("unknown relic: {key}"))?;
        if run.relics.contains(&relic) {
            return Err("duplicate relic".into());
        }
        if relic == Relic::PrismaticShard {
            return Err(
                "incompatible loadout: Prismatic Shard is not supported by the fair API".into(),
            );
        }
        run.relics.push(relic);
    }
    if run.relics.contains(&Relic::BurningBlood) && run.relics.contains(&Relic::BlackBlood) {
        return Err("mutually exclusive starter relics".into());
    }
    // Install an already-owned loadout, not acquisition actions. In particular,
    // never transform its deck or add fruit HP a second time.
    run.energy_per_turn = 3 + run
        .relics
        .iter()
        .copied()
        .filter_map(relic_pickup_energy)
        .sum::<i32>();
    run.deck.clear();
    for (index, input) in spec.deck.iter().enumerate() {
        if input.upgrades > u32::from(u8::MAX) {
            return Err("incompatible loadout: upgrade count exceeds simulator capacity".into());
        }
        let definition = public_card_definitions()
            .find(|d| d.key == input.key)
            .ok_or_else(|| format!("unknown card: {}", input.key))?;
        if definition.key.ends_with('+') {
            return Err("expected a base card key plus upgrade count".into());
        }
        if input.ritual_dagger_damage_bonus < 0
            || input.ritual_dagger_damage_bonus > 10000
            || (input.ritual_dagger_damage_bonus != 0 && definition.key != "RitualDagger")
        {
            return Err("invalid permanent card damage bonus".into());
        }
        let mut card = CardInstance::new(CardId::new(index as u64 + 1), definition.id);
        for _ in 0..input.upgrades {
            card = upgrade_card_instance(card)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "card cannot be upgraded again".to_string())?;
        }
        card.ritual_dagger_damage_bonus = input.ritual_dagger_damage_bonus;
        run.deck.push(card);
    }
    // Deterministic first-compatible target is an explicit prior; no hidden RNG.
    for (relic, card_type) in [
        (Relic::BottledFlame, CardType::Attack),
        (Relic::BottledLightning, CardType::Skill),
        (Relic::BottledTornado, CardType::Power),
    ] {
        if run.relics.contains(&relic) {
            let card = run
                .deck
                .iter_mut()
                .find(|c| {
                    public_card_definitions()
                        .any(|d| d.id == c.content_id && d.card_type == card_type)
                })
                .ok_or_else(|| {
                    format!("incompatible loadout: no target for {}", relic.trace_name())
                })?;
            card.bottled = true;
        }
    }
    if spec.potions.len() != run.potion_capacity() {
        return Err("incorrect potion capacity".into());
    }
    run.potions.clear();
    run.empty_potion_slots.clear();
    for (slot, key) in spec.potions.iter().enumerate() {
        match key {
            None => run.empty_potion_slots.push(slot),
            Some(key) => run.potions.push(
                crate::fair_catalog::potion_from_key(key)
                    .ok_or_else(|| format!("unknown potion: {key}"))?,
            ),
        }
    }
    run.incense_burner_counter = spec.counters.incense_burner;
    run.pen_nib_attacks_played = spec.counters.pen_nib;
    run.ink_bottle_cards_played = spec.counters.ink_bottle;
    run.happy_flower_turns = spec.counters.happy_flower;
    run.sundial_shuffles = spec.counters.sundial;
    run.nunchaku_attacks_played = spec.counters.nunchaku;
    run.lizard_tail_used = spec.counters.lizard_tail_used;
    run.validate().map_err(|e| e.to_string())?;
    enter_synthetic_combat(&mut run, kind, &spec.encounter).map_err(|e| e.to_string())?;
    run.validate().map_err(|e| e.to_string())?;
    Ok(run)
}
