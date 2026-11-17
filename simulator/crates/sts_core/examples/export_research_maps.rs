//! Export independently generated maps for offline research, NOT trace replay.
//! JSONL input: {"seed": <signed i64>, "act": 1, "ascension": 0}.
//! No observed route, expected state, or RL dependency enters generation.

use serde::Deserialize;
use serde_json::json;
use std::io::{self, BufRead, Write};
use sts_core::map::{generate_target_map_topology, RoomKind, TargetMapAct};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    seed: i64,
    act: u8,
    ascension: u8,
}

fn export(request: Request) -> Result<serde_json::Value, String> {
    if request.ascension != 0 {
        return Err(
            "current map generator has no ascension parameter; audit supports A0 only".into(),
        );
    }
    let (act, offset) = match request.act {
        1 => (TargetMapAct::Exordium, 1),
        2 => (TargetMapAct::City, 200),
        3 => (TargetMapAct::Beyond, 600),
        _ => return Err("expected act 1, 2, or 3".into()),
    };
    request
        .seed
        .checked_add(offset)
        .ok_or("seed offset overflow")?;
    let topology = generate_target_map_topology(request.seed, act);
    let nodes: Vec<_> = topology
        .assigned_rooms
        .iter()
        .map(|node| {
            let symbol = match node.room_kind {
                RoomKind::Combat => "M",
                RoomKind::Elite => "E",
                RoomKind::Event => "?",
                RoomKind::Rest => "R",
                RoomKind::Shop => "$",
                RoomKind::Treasure => "T",
                RoomKind::Boss => "BOSS",
                RoomKind::Victory => "VICTORY",
            };
            json!({
                "row": node.row, "x": node.x, "symbol": symbol,
                "children": node.children.iter().map(|child| {
                    json!([child.row, child.x])
                }).collect::<Vec<_>>()
            })
        })
        .collect();
    Ok(
        json!({"seed": request.seed, "act": request.act, "ascension": 0,
        "starts": topology.first_row_choices, "nodes": nodes}),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut out = io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let request: Request = serde_json::from_str(&line?)?;
        let value = export(request).map_err(io::Error::other)?;
        serde_json::to_writer(&mut out, &value)?;
        writeln!(out)?;
    }
    out.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_maps_are_repeatable_and_have_no_rng_metadata() {
        let request = || Request {
            seed: -123456789,
            act: 1,
            ascension: 0,
        };
        let map = export(request()).unwrap();
        assert_eq!(map, export(request()).unwrap());
        assert!(map.get("map_rng_counter").is_none());
        assert!(!map["nodes"].as_array().unwrap().is_empty());
    }

    #[test]
    fn unsupported_generation_is_rejected() {
        assert!(export(Request {
            seed: 1,
            act: 1,
            ascension: 20
        })
        .is_err());
        assert!(export(Request {
            seed: i64::MAX,
            act: 1,
            ascension: 0
        })
        .is_err());
        assert!(export(Request {
            seed: 1,
            act: 4,
            ascension: 0
        })
        .is_err());
        assert!(
            serde_json::from_str::<Request>(r#"{"seed":1,"act":1,"ascension":0,"route":[]}"#)
                .is_err()
        );
    }
}
