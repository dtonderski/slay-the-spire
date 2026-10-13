"""Stable public card IDs: frozen v1 prefix, then lexical additions."""

import json
from pathlib import Path

from sts_sim import CardKey

CARD_KEYS_V1: tuple[str, ...] = tuple(json.loads(Path(__file__).with_name("legacy_cards_v1.json").read_text()))
if len(CARD_KEYS_V1) != 322 or len(set(CARD_KEYS_V1)) != 322:
    raise ValueError("Malformed legacy card vocabulary")
current = {str(key) for key in CardKey}
if not set(CARD_KEYS_V1) <= current:
    raise ValueError("A legacy public card identity was removed")
CARD_KEYS = CARD_KEYS_V1 + tuple(sorted(current - set(CARD_KEYS_V1)))
CARD_TO_INDEX = {CardKey(key): index for index, key in enumerate(CARD_KEYS)}
