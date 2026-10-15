"""Extract observed card/boss-relic slates, never invent historical legality.

Run from rl/: uv run python -m pretraining.choices --help
The first training task is explicitly conditional on taking a card/relic.
Skip/Bowl and non-choice relic acquisitions remain audit records, not negatives.
"""

import argparse
import json
import re
from collections import Counter
from pathlib import Path

from tools.loadout_identities import CATALOG_SHA256, resolve

from .source import digest, ordinary, shuffled_records, split_for, write_json

SCHEMA = 1
SPECIAL = {"SKIP", "Singing Bowl"}


def floor_number(value) -> int:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not 0 <= value <= 60 or int(value) != value:
        raise ValueError("invalid_floor")
    return int(value)


def content(value: str, kind: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError("invalid_identity")
    if kind == "relic":
        return "relic:" + resolve(value, kind)
    match = re.fullmatch(r"(.+?)\+(\d+)", value)
    name, upgrades = (match[1], int(match[2])) if match else (value, 0)
    if "+" in name or (upgrades > 1 and name != "Searing Blow"):
        raise ValueError("unsupported_upgrade_notation")
    return f"card:{resolve(name, 'card')}+{upgrades}"


def slate(choice: dict, kind: str) -> tuple[list[str], str]:
    if not isinstance(choice, dict) or not isinstance(choice.get("picked"), str):
        raise ValueError("missing_pick")
    picked = choice["picked"]
    if picked in SPECIAL:
        raise ValueError("special_action_requires_eligibility")
    rejected = choice.get("not_picked")
    if not isinstance(rejected, list):
        raise ValueError("missing_rejected_options")
    label = content(picked, kind)
    # Equivalent duplicate identities form one choice class; original UI slots
    # are absent. Sorted identities prevent picked-first positional leakage.
    candidates = sorted({label, *(content(value, kind) for value in rejected)})
    return candidates, label


def _array(event: dict, key: str) -> list:
    values = event.get(key)
    if not isinstance(values, list):
        raise ValueError(f"missing_or_invalid_{key}")
    return values


def history_before_floor(event: dict, floor: int) -> dict:
    """Explicit earlier logged edits, NOT an asserted reconstructed owned deck.

    No final deck, final relics, future consistency flags, outcome, seed, exact
    HP/inventory or current-floor ordering is consumed. Missing arrays reject.
    """
    history: dict[str, list[str]] = {
        key: []
        for key in (
            "card_picks",
            "card_skips",
            "card_bowls",
            "event_gains",
            "event_removals",
            "event_transforms",
            "event_upgrades",
            "smith_targets",
            "purchases",
            "purges",
            "relic_gains",
            "relic_losses",
        )
    }
    for entry in _array(event, "card_choices"):
        if floor_number(entry.get("floor")) >= floor:
            continue
        picked = entry.get("picked")
        if picked == "SKIP":
            history["card_skips"].append("skip")
        elif picked == "Singing Bowl":
            history["card_bowls"].append("bowl")
        else:
            history["card_picks"].append(content(picked, "card"))
    for entry in _array(event, "event_choices"):
        if floor_number(entry.get("floor")) >= floor:
            continue
        for source, target, kind in (
            ("cards_obtained", "event_gains", "card"),
            ("cards_removed", "event_removals", "card"),
            ("cards_transformed", "event_transforms", "card"),
            ("cards_upgraded", "event_upgrades", "card"),
            ("relics_obtained", "relic_gains", "relic"),
            ("relics_lost", "relic_losses", "relic"),
        ):
            values = entry.get(source, [])
            if not isinstance(values, list):
                raise ValueError(f"invalid_event_{source}")
            history[target].extend(content(value, kind) for value in values)
    for entry in _array(event, "campfire_choices"):
        if floor_number(entry.get("floor")) < floor and entry.get("key") == "SMITH":
            history["smith_targets"].append(content(entry.get("data"), "card"))
    for entry in _array(event, "relics_obtained"):
        if floor_number(entry.get("floor")) < floor:
            history["relic_gains"].append(content(entry.get("key"), "relic"))
    for items, floors, target in (
        ("items_purchased", "item_purchase_floors", "purchases"),
        ("items_purged", "items_purged_floors", "purges"),
    ):
        values, locations = _array(event, items), _array(event, floors)
        if len(values) != len(locations):
            raise ValueError("unaligned_purchase_or_purge_arrays")
        for value, location in zip(values, locations, strict=True):
            if floor_number(location) >= floor:
                continue
            if target == "purges":
                history[target].append(content(value, "card"))
            else:
                # Purchases mix cards, relics and potions in a single array.
                matches = []
                for kind in ("card", "relic", "potion"):
                    try:
                        matches.append(content(value, kind) if kind != "potion" else "potion:" + resolve(value, kind))
                    except ValueError:
                        pass
                if len(matches) != 1:
                    raise ValueError("unknown_or_ambiguous_purchase")
                history[target].append(matches[0])
    # Deliberately do not merge parallel log sources into a fictitious inventory.
    return {key: dict(sorted(Counter(values).items())) for key, values in history.items()}


def extract(event: dict) -> tuple[list[dict], list[dict], Counter]:
    ordinary(event)
    examples, audits, counts = [], [], Counter()
    ascension = event["ascension_level"]
    for kind, source in (("card", "card_choices"), ("relic", "boss_relics")):
        for ordinal, choice in enumerate(_array(event, source)):
            counts[f"{source}_observed"] += 1
            try:
                candidates, label = slate(choice, kind)
                if len(candidates) < 2:
                    raise ValueError("singleton_no_ranking_signal")
                if kind == "card":
                    floor = floor_number(choice.get("floor"))
                    context = {
                        "ascension": ascension,
                        "floor": floor,
                        "history": history_before_floor(event, floor),
                        "neow_bonus": event.get("neow_bonus", "unknown"),
                        "neow_cost": event.get("neow_cost", "unknown"),
                    }
                    scope = "earlier_floor_logged_history_not_reconstructed_state"
                else:
                    # Boss entries have no floors. Do not manufacture a floor
                    # alignment from ordinal or use the final deck as context.
                    prior = []
                    for previous in event[source][:ordinal]:
                        picked = previous.get("picked")
                        if isinstance(picked, str) and picked not in SPECIAL:
                            prior.append(content(picked, "relic"))
                    context = {
                        "ascension": ascension,
                        "boss_ordinal": ordinal,
                        "history": {"prior_boss_picks": dict(Counter(prior))},
                    }
                    scope = "prior_boss_picks_only_no_floor_alignment"
                examples.append(
                    {
                        "schema": SCHEMA,
                        "kind": kind,
                        "source_array": source,
                        "ordinal": ordinal,
                        "context_scope": scope,
                        "context": context,
                        "candidates": candidates,
                        "label": label,
                        "task": "conditional_on_taking_item",
                    }
                )
                counts[f"{source}_accepted"] += 1
            except (ValueError, TypeError, KeyError, AttributeError) as error:
                reason = str(error) if isinstance(error, ValueError) else f"malformed_{type(error).__name__}"
                counts[f"{source}_rejected:{reason}"] += 1
                audits.append({"source_array": source, "ordinal": ordinal, "reason": reason, "logged": choice})
    # Acquisitions do not establish rejected offers, optional skips, or a choice
    # between elite relics. Preserve them separately; never train fake negatives.
    for ordinal, entry in enumerate(_array(event, "relics_obtained")):
        audits.append(
            {
                "source_array": "relics_obtained",
                "ordinal": ordinal,
                "reason": "acquisition_only_no_choice_slate",
                "logged": entry,
            }
        )
        counts["relic_acquisitions_not_demonstrations"] += 1
    return examples, audits, counts


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--database", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--runs-per-ascension", type=int, default=1000)
    parser.add_argument("--ascensions", type=int, nargs="+", default=[0, 20])
    parser.add_argument("--build", default="2020-07-30")
    parser.add_argument("--seed", type=int, default=20261015)
    args = parser.parse_args()
    if args.runs_per_ascension <= 0 or len(set(args.ascensions)) != len(args.ascensions):
        parser.error("positive count and distinct ascensions required")
    args.output_dir.mkdir(parents=True, exist_ok=False)
    counts, seen = Counter(), set()
    with (
        (args.output_dir / "examples.jsonl").open("w") as out,
        (args.output_dir / "audit.jsonl").open("w") as audit,
        (args.output_dir / "membership.jsonl").open("w") as membership,
    ):
        for ascension in args.ascensions:
            accepted = 0
            for event, provenance in shuffled_records(
                args.database, ascension=ascension, build=args.build, seed=args.seed
            ):
                counts["candidate_records"] += 1
                try:
                    ordinary(event)
                    if event["build_version"] != args.build or event["ascension_level"] != ascension:
                        raise ValueError("database_cohort_disagrees_with_payload")
                    if event["play_id"] in seen:
                        raise ValueError("duplicate_play_id")
                    examples, audits, report = extract(event)
                except (ValueError, TypeError, KeyError, AttributeError) as error:
                    counts[f"run_rejected:{error}"] += 1
                    audit.write(json.dumps({"source": provenance, "run_rejection": str(error)}) + "\n")
                    continue
                seen.add(event["play_id"])
                group = digest(event["play_id"].encode())
                split = split_for(event["play_id"])
                metadata = {"source": provenance, "run_group": group, "split": split, "build": args.build}
                membership.write(json.dumps(metadata) + "\n")
                for example in examples:
                    out.write(json.dumps({**metadata, **example}, sort_keys=True, allow_nan=False) + "\n")
                for record in audits:
                    audit.write(json.dumps({**metadata, **record}, sort_keys=True, allow_nan=False) + "\n")
                counts.update(report)
                counts[f"runs_a{ascension}"] += 1
                counts[f"examples_{split}"] += len(examples)
                accepted += 1
                if accepted == args.runs_per_ascension:
                    break
            if accepted != args.runs_per_ascension:
                raise RuntimeError(f"Cohort exhausted: A{ascension}: {accepted}; output remains incomplete")
    write_json(
        args.output_dir / "summary.json",
        {
            "schema": SCHEMA,
            "status": "complete",
            "counts": dict(counts),
            "catalog_sha256": CATALOG_SHA256,
            "seed": args.seed,
            "build": args.build,
            "ascensions": args.ascensions,
            "database": str(args.database.resolve()),
            "database_bytes": args.database.stat().st_size,
            "examples_sha256": digest((args.output_dir / "examples.jsonl").read_bytes()),
            "limitations": [
                "Conditional item ranking; not a complete legal-action policy.",
                "Partial logged history, not exact deck, relic inventory, HP, keys or map.",
                "Boss context has no floor alignment; acquisitions are not elite choice labels.",
                "Play-ID split is not player-disjoint and does not eliminate different-ID semantic duplicates.",
            ],
        },
    )
    print(json.dumps(dict(counts), indent=2))


if __name__ == "__main__":
    main()
