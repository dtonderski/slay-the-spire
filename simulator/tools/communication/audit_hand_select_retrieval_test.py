"""Synthetic read-only audit regressions; these do not establish gameplay parity."""
import json
import tempfile
import unittest
from pathlib import Path

from audit_hand_select_retrieval import audit_trace


def card(identity, uuid):
    return {"id": identity, "uuid": uuid, "upgrades": 0}


def state(step, owner, hand, *, selected=None, draw=(), discard=(), exhaust=(), spoon=False):
    screen = {} if selected is None else {"selected": selected}
    return {"type": "state", "step": step, "message": {
        "current_action": owner, "game_state": {
            "screen_type": "HAND_SELECT" if owner else "NONE", "screen_state": screen,
            "relics": [{"id": "Strange Spoon"}] if spoon else [],
            "combat_state": {"hand": hand, "draw_pile": list(draw),
                             "discard_pile": list(discard), "exhaust_pile": list(exhaust)},
        }}}


def action(step, command):
    return {"type": "action", "step": step, "command": command}


class HandRetrievalAuditTests(unittest.TestCase):
    def audit(self, records):
        with tempfile.TemporaryDirectory() as directory:
            trace = Path(directory) / "synthetic.jsonl"
            trace.write_text("".join(json.dumps(record) + "\n" for record in records))
            before = trace.read_bytes()
            result = audit_trace(trace)
            self.assertEqual(trace.read_bytes(), before)
            return result

    def adjacent_selections(self, explicit):
        warcry, strike, defend = card("Warcry", "w"), card("Strike_R", "s"), card("Defend_R", "d")
        records = [
            state(0, "PutOnDeckAction", [warcry, strike, defend], selected=[]),
            action(1, "CHOOSE 0"), state(1, "PutOnDeckAction", [strike, defend], selected=[warcry]),
            action(2, "CONFIRM"), state(2, "ExhaustAction", [strike, defend], selected=[], draw=[warcry]),
            action(3, "CHOOSE 0"), state(3, "ExhaustAction", [defend], selected=[strike], draw=[warcry]),
            action(4, "CONFIRM"), state(4, None, [defend], draw=[warcry], exhaust=[strike]),
        ]
        if not explicit:
            for record in records:
                if record["type"] == "state":
                    record["message"]["game_state"]["screen_state"].pop("selected", None)
        confirms, skipped = self.audit(records)
        self.assertEqual(confirms["ExhaustAction"], 1)
        self.assertEqual(skipped, [])

    def test_explicit_selection_does_not_include_previous_action(self):
        self.adjacent_selections(explicit=True)

    def test_legacy_fallback_stops_at_owner_change(self):
        self.adjacent_selections(explicit=False)

    def test_same_content_other_uuid_does_not_mask_missing_exhaust_retrieval(self):
        selected, replacement = card("Strike_R", "selected"), card("Strike_R", "other")
        confirms, skipped = self.audit([
            state(0, "ExhaustAction", [selected], selected=[]),
            action(1, "CHOOSE 0"), state(1, "ExhaustAction", [], selected=[selected]),
            action(2, "CONFIRM"), state(2, None, [], exhaust=[replacement]),
        ])
        self.assertEqual(confirms["ExhaustAction"], 1)
        self.assertEqual(skipped, [(2, "ExhaustAction")])

    def test_missing_selected_card_is_reported(self):
        selected = card("Strike_R", "s")
        _, skipped = self.audit([
            state(0, "ExhaustAction", [selected], selected=[]),
            action(1, "CHOOSE 0"), state(1, "ExhaustAction", [], selected=[selected]),
            action(2, "CONFIRM"), state(2, None, []),
        ])
        self.assertEqual(skipped, [(2, "ExhaustAction")])

    def test_strange_spoon_discard_keeps_the_selected_instance(self):
        selected = card("Strike_R", "s")
        _, skipped = self.audit([
            state(0, "ExhaustAction", [selected], selected=[]),
            action(1, "CHOOSE 0"), state(1, "ExhaustAction", [], selected=[selected]),
            action(2, "CONFIRM"), state(2, None, [], discard=[selected], spoon=True),
        ])
        self.assertEqual(skipped, [])


if __name__ == "__main__":
    unittest.main()
