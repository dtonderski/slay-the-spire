//! Seeded robustness research, not training or real-game parity evidence.
//! cargo run -p sts_core --example combat_fuzz --release -- START COUNT OUTPUT_DIR
use serde_json::{json, Value};
use std::{
    fs,
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
};
use sts_core::adapter_internals::{
    apply_run_decision_action,
    content::{
        cards::{public_card_definitions, upgrade_card_instance},
        encounters::*,
    },
    legal_run_decision_actions,
    run::{map::enter_synthetic_combat, state::relic_pickup_energy},
    CardId, CardInstance, CardType, CombatPhase, Relic, RoomKind, RunPhase, RunState,
};

// Local driver RNG. Never shares a stream with gameplay.
struct Driver(u64);
impl Driver {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn index(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn generate(seed: u64) -> Result<RunState, String> {
    let mut rng = Driver(seed);
    let ascension = [0, 2, 10, 17, 18, 19, 20][rng.index(7)];
    let mut run =
        RunState::try_seeded_ironclad(rng.next(), ascension).map_err(|e| e.to_string())?;
    run.phase = RunPhase::Idle;
    run.event = None;
    run.map = None;
    run.emerald_key_node = None;
    run.current_act = 1 + rng.index(4) as i32;
    let mut encounters = Vec::new();
    type EncounterPool = &'static [(&'static str, f32)];
    let (weak, strong, elites): (EncounterPool, EncounterPool, EncounterPool) =
        match run.current_act {
            1 => (
                &EXORDIUM_WEAK_ENCOUNTERS,
                &EXORDIUM_STRONG_ENCOUNTERS,
                &EXORDIUM_ELITE_ENCOUNTERS,
            ),
            2 => (
                &CITY_WEAK_ENCOUNTERS,
                &CITY_STRONG_ENCOUNTERS,
                &CITY_ELITE_ENCOUNTERS,
            ),
            _ => (
                &BEYOND_WEAK_ENCOUNTERS,
                &BEYOND_STRONG_ENCOUNTERS,
                &BEYOND_ELITE_ENCOUNTERS,
            ),
        };
    if run.current_act < 4 {
        encounters.extend(
            weak.iter()
                .chain(strong)
                .map(|(key, _)| (RoomKind::Combat, *key)),
        );
        encounters.extend(elites.iter().map(|(key, _)| (RoomKind::Elite, *key)));
    }
    let bosses: &[&str] = match run.current_act {
        1 => &["Hexaghost", "Slime Boss", "The Guardian"],
        2 => &["Automaton", "Collector", "Champ"],
        3 => &["Awakened One", "Time Eater", "Donu and Deca"],
        _ => &["Corrupt Heart"],
    };
    encounters.extend(bosses.iter().map(|key| (RoomKind::Boss, *key)));
    if run.current_act == 4 {
        encounters.push((RoomKind::Elite, "Shield and Spear"));
    }
    let (kind, encounter) = encounters[rng.index(encounters.len())];
    run.current_floor = match (run.current_act, kind) {
        (1, RoomKind::Boss) => 16,
        (2, RoomKind::Boss) => 33,
        (3, RoomKind::Boss) => 50,
        (4, RoomKind::Boss) => 55,
        (4, _) => 54,
        (1, _) => 7,
        (2, _) => 24,
        _ => 41,
    };
    run.max_hp = 80 + rng.index(81) as i32;
    run.hp = 1 + rng.index(run.max_hp as usize) as i32;
    let relic_pool = [
        Relic::SneckoEye,
        Relic::RunicPyramid,
        Relic::DeadBranch,
        Relic::IceCream,
        Relic::Toolbox,
        Relic::GamblingChip,
        Relic::Necronomicon,
        Relic::PenNib,
        Relic::IncenseBurner,
        Relic::Vajra,
        Relic::Anchor,
        Relic::CoffeeDripper,
        Relic::InkBottle,
        Relic::OrnamentalFan,
        Relic::OddlySmoothStone,
    ];
    for _ in 0..rng.index(9) {
        let relic = relic_pool[rng.index(relic_pool.len())];
        if !run.relics.contains(&relic) {
            run.relics.push(relic);
        }
    }
    run.energy_per_turn = 3 + run
        .relics
        .iter()
        .copied()
        .filter_map(relic_pickup_energy)
        .sum::<i32>();
    let cards: Vec<_> = public_card_definitions()
        .filter(|d| !d.key.ends_with('+') && d.card_type != CardType::Status)
        .collect();
    let count = rng.index(25);
    for _ in 0..count {
        let definition = cards[rng.index(cards.len())];
        let mut card = CardInstance::new(CardId::new(run.deck.len() as u64 + 1), definition.id);
        if rng.index(2) == 0 {
            card = upgrade_card_instance(card)
                .map_err(|e| e.to_string())?
                .unwrap_or(card);
        }
        run.deck.push(card);
    }
    run.validate()
        .map_err(|e| format!("generated loadout: {e}"))?;
    enter_synthetic_combat(&mut run, kind, encounter).map_err(|e| format!("combat entry: {e}"))?;
    Ok(run)
}

fn check(run: &RunState) -> Result<(), String> {
    run.validate().map_err(|e| format!("run invariant: {e}"))?;
    if let Some(combat) = &run.combat {
        combat
            .validate()
            .map_err(|e| format!("combat invariant: {e}"))?;
    }
    Ok(())
}

fn execute(
    initial: &RunState,
    rng: &mut Driver,
    journal: &mut Value,
) -> Result<&'static str, String> {
    let mut run = initial.clone();
    for step in 0..2000 {
        check(&run)?;
        if run.phase != RunPhase::Combat
            || run
                .combat
                .as_ref()
                .is_some_and(|c| matches!(c.phase, CombatPhase::Won | CombatPhase::Lost))
        {
            return Ok("terminal");
        }
        let before = serde_json::to_value(&run).map_err(|e| e.to_string())?;
        let legal = legal_run_decision_actions(&run).map_err(|e| format!("legal query: {e}"))?;
        if serde_json::to_value(&run).map_err(|e| e.to_string())? != before {
            return Err("legal query mutated state".into());
        }
        if legal.is_empty() {
            return Err("nonterminal combat has no legal actions".into());
        }
        let action = legal[rng.index(legal.len())];
        journal["attempted_action"] = serde_json::to_value(action).map_err(|e| e.to_string())?;
        journal["step"] = json!(step);
        let next = apply_run_decision_action(&run, action)
            .map_err(|e| format!("enumerated action rejected: {e}; {action:?}"))?;
        journal["accepted_actions"]
            .as_array_mut()
            .unwrap()
            .push(json!(action));
        let restored: RunState =
            serde_json::from_value(before).map_err(|e| format!("restore: {e}"))?;
        let repeated = apply_run_decision_action(&restored, action)
            .map_err(|e| format!("restored transition: {e}"))?;
        if serde_json::to_value(&next).map_err(|e| e.to_string())?
            != serde_json::to_value(&repeated).map_err(|e| e.to_string())?
        {
            return Err("restored transition differs".into());
        }
        run = next;
    }
    Ok("action_limit_candidate")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "replay") {
        let path = args.get(2).ok_or("missing artifact path")?;
        let artifact: Value = serde_json::from_slice(&fs::read(path)?)?;
        let mut run: RunState = serde_json::from_value(artifact["initial_state"].clone())?;
        check(&run)?;
        for (index, value) in artifact["accepted_actions"]
            .as_array()
            .ok_or("missing actions")?
            .iter()
            .enumerate()
        {
            let action: sts_core::adapter_internals::RunDecisionAction =
                serde_json::from_value(value.clone())?;
            let next = apply_run_decision_action(&run, action)?;
            if let Err(error) = check(&next) {
                fs::write(
                    format!("{path}.before.json"),
                    serde_json::to_vec_pretty(&run)?,
                )?;
                fs::write(
                    format!("{path}.after.json"),
                    serde_json::to_vec_pretty(&next)?,
                )?;
                println!("reproduced at accepted action {index}: {action:?}: {error}");
                return Ok(());
            }
            run = next;
        }
        fs::write(
            format!("{path}.before.json"),
            serde_json::to_vec_pretty(&run)?,
        )?;
        let legal = legal_run_decision_actions(&run)?;
        if legal.is_empty() {
            println!("reproduced empty legal list; phase={:?}", run.phase);
            return Ok(());
        }
        let action = serde_json::from_value(artifact["attempted_action"].clone())?;
        println!(
            "attempted={action:?} enumerated={}",
            legal.contains(&action)
        );
        match apply_run_decision_action(&run, action) {
            Err(error) => println!("reproduced rejected action: {error}"),
            Ok(_) => return Err("failure did not reproduce".into()),
        }
        return Ok(());
    }
    let start: u64 = args.get(1).ok_or("missing START")?.parse()?;
    let count: u64 = args.get(2).ok_or("missing COUNT")?.parse()?;
    let out = PathBuf::from(args.get(3).ok_or("missing OUTPUT_DIR")?);
    fs::create_dir_all(&out)?;
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    let dirty = std::process::Command::new("git")
        .args(["diff", "HEAD"])
        .output()?;
    fs::write(out.join("implementation.patch"), &dirty.stdout)?;
    fs::write(out.join("driver.rs"), include_str!("combat_fuzz.rs"))?;
    let mut failures = 0;
    let mut terminal = 0;
    let mut capped = 0;
    for seed in start..start.checked_add(count).ok_or("seed range overflows")? {
        let mut journal = json!({"schema": 1, "revision": String::from_utf8_lossy(&revision.stdout).trim(), "driver_seed": seed, "external_inputs": [], "accepted_actions": [], "action_limit": 2000});
        let result = catch_unwind(AssertUnwindSafe(|| {
            let initial = generate(seed)?;
            journal["initial_state"] = serde_json::to_value(&initial).map_err(|e| e.to_string())?;
            execute(
                &initial,
                &mut Driver(seed ^ 0xd1b54a32d192ed03),
                &mut journal,
            )
        }));
        let outcome = match result {
            Ok(Ok("terminal")) => {
                terminal += 1;
                continue;
            }
            Ok(Ok(other)) => {
                capped += 1;
                other.to_string()
            }
            Ok(Err(error)) => {
                failures += 1;
                error
            }
            Err(panic) => {
                failures += 1;
                format!(
                    "panic: {}",
                    panic
                        .downcast_ref::<String>()
                        .map(String::as_str)
                        .or_else(|| panic.downcast_ref::<&str>().copied())
                        .unwrap_or("non-string payload")
                )
            }
        };
        journal["outcome"] = json!(outcome);
        let name = out.join(format!("seed-{seed}.json"));
        fs::write(&name, serde_json::to_vec_pretty(&journal)?)?;
        println!("seed={seed} outcome={outcome} artifact={}", name.display());
        if (seed - start).is_multiple_of(100) {
            println!(
                "progress seed={seed} terminal={terminal} failures={failures} capped={capped}"
            );
        }
    }
    println!(
        "done start={start} count={count} terminal={terminal} failures={failures} capped={capped}"
    );
    Ok(())
}
