"""Generate checked-in Python StrEnums from the fair content catalog."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
OUTPUT_PATH = REPO_ROOT / "simulator" / "python" / "sts_sim" / "content_ids.py"

ENUMS = (
    ("relics", "RelicKey", "Fair relic content_key values from Relic::trace_name."),
    ("potions", "PotionKey", "Fair potion content_key values from potion_key()."),
    ("cards", "CardKey", "Fair card content_key values from public card definitions."),
    ("monsters", "MonsterKey", "Fair monster content_key values from monster definition names."),
    ("events", "EventKey", "Fair EventScreen.event values from the Event enum."),
    ("powers", "PowerKey", "Fair Power.key values emitted by combat projection."),
    ("counters", "CounterKey", "Fair Counter.key values for relics and public combat counters."),
)

HEADER = '''\
"""Generated fair content identities. Do not edit by hand.

Regenerate from the repository root:

    python simulator/python/tools/generate_content_ids.py

Values are the exact strings currently emitted by fair observations. Do not use
enum ordinals as an ML vocabulary; build an explicit versioned mapping from these
identity values to tensor indices.
"""

from __future__ import annotations

from enum import StrEnum

'''


def load_catalog() -> dict[str, list[str]]:
    result = subprocess.run(
        ["cargo", "run", "-q", "-p", "sts_env", "--bin", "export_fair_catalog"],
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        sys.stderr.write(result.stderr)
        raise SystemExit(result.returncode)
    payload = json.loads(result.stdout)
    if not isinstance(payload, dict):
        raise SystemExit("catalog export must be a JSON object")
    catalog: dict[str, list[str]] = {}
    for key, _, _ in ENUMS:
        values = payload.get(key)
        if not isinstance(values, list) or not values:
            raise SystemExit(f"catalog {key!r} must be a non-empty JSON array")
        typed: list[str] = []
        for value in values:
            if not isinstance(value, str) or not value:
                raise SystemExit(f"catalog {key!r} contains a non-string identity")
            typed.append(value)
        catalog[key] = typed
    return catalog


def member_name(value: str) -> str:
    stripped = value.replace("'", "").replace("’", "").replace("+", "_PLUS")
    chars: list[str] = []
    prev_raw = ""
    for ch in stripped:
        if ch.isalnum():
            if (
                chars
                and chars[-1] != "_"
                and (
                    (ch.isupper() and prev_raw.islower())
                    or (ch.isdigit() and prev_raw.isalpha())
                    or (ch.isalpha() and prev_raw.isdigit())
                )
            ):
                chars.append("_")
            chars.append(ch.upper())
        elif not chars or chars[-1] != "_":
            chars.append("_")
        prev_raw = ch
    name = "".join(chars).strip("_")
    while "__" in name:
        name = name.replace("__", "_")
    if not name or not name[0].isalpha():
        name = f"K_{name}" if name else "K"
    return name


def render_enum(class_name: str, doc: str, values: list[str]) -> str:
    used: dict[str, str] = {}
    lines = [f"class {class_name}(StrEnum):", f'    """{doc}"""', ""]
    for value in values:
        name = member_name(value)
        if name in used:
            raise SystemExit(
                f"{class_name}: member {name} collides for {used[name]!r} and {value!r}"
            )
        used[name] = value
        lines.append(f"    {name} = {json.dumps(value)}")
    lines.append("")
    lines.append("")
    return "\n".join(lines)


def render(catalog: dict[str, list[str]]) -> str:
    parts = [HEADER]
    exported = []
    for key, class_name, doc in ENUMS:
        parts.append(render_enum(class_name, doc, catalog[key]))
        exported.append(class_name)
    all_lines = ",\n    ".join(f'"{name}"' for name in sorted(exported))
    parts.append(f"__all__ = [\n    {all_lines},\n]\n")
    return "\n".join(parts)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="exit non-zero if the checked-in file is stale",
    )
    args = parser.parse_args()
    generated = render(load_catalog())
    if args.check:
        current = OUTPUT_PATH.read_text(encoding="utf-8") if OUTPUT_PATH.exists() else ""
        if current != generated:
            raise SystemExit(f"{OUTPUT_PATH} is stale; rerun generate_content_ids.py")
        return
    OUTPUT_PATH.write_text(generated, encoding="utf-8")


if __name__ == "__main__":
    main()
