//! Count map-labeled combats at A0; no fights or path rollouts are required.
//! Usage: cargo run -p sts_core --example scenario_map_distribution -- 100
//! Counts all generated nodes once, not visits under a player routing policy.
use sts_core::map::{
    target::{generate_target_fixed_map, TargetMapAct},
    RoomKind,
};

fn main() {
    let seeds: u32 = std::env::args()
        .nth(1)
        .map_or(100, |s| s.parse().expect("positive seed count"));
    assert!(seeds > 0);
    let mut acts = Vec::new();
    let mut total = [0_u64; 3];
    for (index, act) in [
        TargetMapAct::Exordium,
        TargetMapAct::City,
        TargetMapAct::Beyond,
    ]
    .into_iter()
    .enumerate()
    {
        let mut counts = [0_u64; 3];
        for seed in 0..seeds {
            let map = generate_target_fixed_map(i64::from(seed), act);
            for node in &map.map.nodes {
                if node.id == map.current_node {
                    continue;
                }
                match node.room_kind {
                    RoomKind::Combat => counts[0] += 1,
                    RoomKind::Elite => counts[1] += 1,
                    RoomKind::Boss => counts[2] += 1,
                    _ => {}
                }
            }
        }
        for (sum, count) in total.iter_mut().zip(counts) {
            *sum += count;
        }
        acts.push(serde_json::json!({"act": index + 1, "normal": counts[0], "elite": counts[1], "boss": counts[2],
            "elite_fraction_nonboss": counts[1] as f64 / (counts[0] + counts[1]) as f64,
            "elite_fraction_all_combats": counts[1] as f64 / counts.iter().sum::<u64>() as f64}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "ascension": 0, "seed_start": 0, "maps_per_act": seeds,
            "scope": "all map nodes, excluding root; event fights excluded; not path-weighted",
            "acts": acts, "total": {"normal": total[0], "elite": total[1], "boss": total[2],
            "elite_fraction_nonboss": total[1] as f64 / (total[0] + total[1]) as f64,
            "elite_fraction_all_combats": total[1] as f64 / total.iter().sum::<u64>() as f64}
        }))
        .expect("serialize census")
    );
}
