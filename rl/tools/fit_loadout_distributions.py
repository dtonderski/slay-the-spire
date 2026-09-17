"""Fit independent A0 marginals from recorded FINAL loadouts, never inferred combat states."""

import argparse
import hashlib
import json
import re
import sqlite3
import sys
import time
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from loadout_sampling import FLOOR_BANDS, STARTERS, band_for
from tools.loadout_identities import CATALOG_SHA256, resolve


def empty_band() -> dict:
    return {
        "runs": 0,
        "wins": 0,
        "deck_sizes": Counter(),
        "cards": {},
        "starters": Counter(),
        "other_relic_counts": Counter(),
        "other_relics": Counter(),
        "potions_obtained": Counter(),
        "max_hp": Counter(),
    }


def split_for(play_id: str) -> str:
    # Stable across source order/restarts. Duplicate play IDs always share a split.
    return "held_out" if int.from_bytes(hashlib.sha256(play_id.encode()).digest()[:8], "big") % 10 == 0 else "fit"


def parse_card(value: str) -> tuple[str, int]:
    if not isinstance(value, str) or not value:
        raise ValueError("invalid_card_identity")
    match = re.fullmatch(r"(.+?)\+(\d+)", value)
    name, upgrades = (match[1], int(match[2])) if match else (value, 0)
    if "+" in name or (upgrades > 1 and name != "Searing Blow"):
        raise ValueError("unsupported_upgrade_notation")
    return resolve(name, "card"), upgrades


def validated_event(event: dict) -> tuple[list[tuple[str, int]], list[str], int]:
    for flag in ("is_daily", "is_trial", "is_endless", "is_beta", "chose_seed"):
        if event.get(flag) not in (False, 0):
            raise ValueError(f"excluded_or_missing_{flag}")
    cards = [parse_card(value) for value in event["master_deck"]]
    if not 1 <= len(cards) <= 100:
        raise ValueError("deck_size_outside_1_100")
    relics = event["relics"]
    if not isinstance(relics, list) or any(not isinstance(x, str) or not x for x in relics):
        raise ValueError("invalid_relics")
    relics = [resolve(value, "relic") for value in relics]
    if len(relics) != len(set(relics)):
        raise ValueError("duplicate_relic_identity")
    if all(key in relics for key in STARTERS):
        raise ValueError("conflicting_starter_relics")
    values = event.get("max_hp_per_floor", [])
    if (
        not values
        or not isinstance(values[-1], (int, float))
        or not 1 <= values[-1] <= 1000
        or int(values[-1]) != values[-1]
    ):
        raise ValueError("missing_or_out_of_range_final_max_hp")
    for potion in event.get("potions_obtained", []):
        if not isinstance(potion, dict) or not isinstance(potion.get("key"), str) or not potion["key"]:
            raise ValueError("invalid_potion_acquisition")
        resolve(potion["key"], "potion")
    return cards, relics, int(values[-1])


def accumulate(data: dict, event: dict, parsed: tuple) -> None:
    cards, relics, maximum = parsed
    data["runs"] += 1
    data["wins"] += bool(event.get("victory"))
    data["deck_sizes"][len(cards)] += 1
    for name, upgrades in cards:
        data["cards"].setdefault(name, Counter())[upgrades] += 1
    starter = next((key for key in STARTERS if key in relics), "none")
    data["starters"][starter] += 1
    others = [key for key in relics if key not in STARTERS]
    data["other_relic_counts"][len(others)] += 1
    data["other_relics"].update(others)
    data["potions_obtained"].update(resolve(potion["key"], "potion") for potion in event.get("potions_obtained", []))
    data["max_hp"][maximum] += 1


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--database", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--version", default="2020-07-30")
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=False)
    splits = {split: {f"{lo}-{hi}": empty_band() for lo, hi in FLOOR_BANDS} for split in ("fit", "held_out")}
    counts = Counter()
    seen = set()
    start = time.time()
    with sqlite3.connect(f"file:{args.database.resolve()}?mode=ro", uri=True) as connection:
        connection.execute("PRAGMA cache_size=-65536")
        records = connection.execute(
            """SELECT id,play_id,floor_reached,raw_json FROM runs
            WHERE character='IRONCLAD' AND ascension=0 AND build_version=? AND floor_reached BETWEEN 1 AND 56
            ORDER BY floor_reached,id""",
            (args.version,),
        )
        with (args.output_dir / "membership.jsonl").open("w") as membership:
            for row_id, play_id, floor, raw in records:
                counts["candidate_records"] += 1
                if not isinstance(play_id, str) or not play_id:
                    counts["missing_play_id"] += 1
                    continue
                if play_id in seen:
                    counts["duplicate_play_id_first_record_kept"] += 1
                    continue
                seen.add(play_id)
                try:
                    event = json.loads(raw)["event"]
                    counts[f"observed_is_prod_{event.get('is_prod')}"] += 1
                    parsed = validated_event(event)
                    band = band_for(floor)
                except (KeyError, ValueError, TypeError) as error:
                    counts[str(error)] += 1
                    continue
                split = split_for(play_id)
                accumulate(splits[split][band], event, parsed)
                counts[split] += 1
                membership.write(
                    json.dumps({"row_id": row_id, "play_id": play_id, "split": split, "band": band}) + "\n"
                )
                if counts["candidate_records"] % 10000 == 0:
                    print(dict(counts), flush=True)
    if not counts["fit"]:
        raise RuntimeError(f"No fit records accepted; inspect filters: {dict(counts)}")
    metadata = {
        "schema": 1,
        "character": "IRONCLAD",
        "ascension": 0,
        "version": args.version,
        "identity_namespace": "sts_sim_public_base_keys",
        "catalog_sha256": CATALOG_SHA256,
        "database": str(args.database.resolve()),
        "database_bytes": args.database.stat().st_size,
        "counts": counts,
        "seconds": time.time() - start,
        "split": "SHA256(play_id) first 8 bytes modulo 10: 0 held out, 1–9 fit; first candidate per ID in (ending floor, row ID) order",
        "limitations": [
            "Final loadouts grouped by ending floor, NOT exact precombat snapshots; survival and player-selection bias remain.",
            "Independent card draws lose deck synergies and within-deck count correlations; duplicates are allowed.",
            "Other relics use weighted draws without replacement; their inclusion rates are not an exact marginal match.",
            "Potion identity weights count logged acquisitions across each cohort's entire runs, not end inventory or all offers.",
            "Potion occupancy is a uniform 0..capacity prior, not measured; Sozu does not force empty inventory.",
            "HP fraction is a configurable uniform-positive prior, not fitted; max HP uses last logged max_hp_per_floor value.",
            "IDs are translated to the current public catalog; unmapped content excludes the entire record. This can exclude legitimate unsupported off-color decks, not just mods. Relic counters/bottled targets are not supplied.",
            "No simulator state construction, gameplay rule changes, or replay hydration. Held-out marginals are diagnostic only.",
            "No card/relic balance compatibility with the simulator's newer target build is assumed.",
            "is_prod is counted but NOT filtered: it is false throughout the inspected ordinary-run sample; its meaning is unverified.",
        ],
    }
    for split, bands in splits.items():
        (args.output_dir / f"{split}.json").write_text(
            json.dumps({**metadata, "split_role": split, "bands": bands}, indent=2)
        )
    (args.output_dir / "summary.json").write_text(json.dumps(metadata, indent=2))
    print(json.dumps(metadata, indent=2), flush=True)


if __name__ == "__main__":
    main()
