import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from audit_hand_select_retrieval import audit_trace  # noqa: E402


def card(card_id, upgrades=0):
    return {"id": card_id, "upgrades": upgrades, "misc": 0}


def state(
    step,
    screen,
    *,
    hand=(),
    draw=(),
    discard=(),
    exhaust=(),
    action="ExhaustAction",
    relics=(),
):
    return {
        "type": "state",
        "step": step,
        "message": {
            "current_action": action,
            "game_state": {
                "screen_type": screen,
                "relics": [{"id": relic} for relic in relics],
                "combat_state": {
                    "hand": list(hand),
                    "draw_pile": list(draw),
                    "discard_pile": list(discard),
                    "exhaust_pile": list(exhaust),
                },
            },
        },
    }


class AuditHandSelectRetrievalTest(unittest.TestCase):
    def write_trace(self, records):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        path = Path(directory.name) / "FIDLTEST-trace.jsonl"
        path.write_text("".join(json.dumps(record) + "\n" for record in records))
        return path

    def selected_confirm(
        self,
        *,
        owner="ExhaustAction",
        after_hand=(),
        after_discard=(),
        after_exhaust=(),
        relics=(),
    ):
        return self.write_trace(
            [
                state(1, "HAND_SELECT", hand=(card("Strike_R"),), action=owner, relics=relics),
                {"type": "action", "step": 2, "command": "CHOOSE 0"},
                state(2, "HAND_SELECT", action=owner, relics=relics),
                {"type": "action", "step": 3, "command": "CONFIRM"},
                state(
                    3,
                    "NONE",
                    hand=after_hand,
                    discard=after_discard,
                    exhaust=after_exhaust,
                    action=None,
                    relics=relics,
                ),
            ]
        )

    def test_reports_selected_card_absent_from_every_pile(self):
        self.assertEqual(
            audit_trace(self.selected_confirm()),
            ({"ExhaustAction": 1}, [(3, "ExhaustAction")]),
        )

    def test_accepts_normal_exhaust_retrieval(self):
        self.assertEqual(
            audit_trace(self.selected_confirm(after_exhaust=(card("Strike_R"),))),
            ({"ExhaustAction": 1}, []),
        )

    def test_accepts_strange_spoon_discard_redirect(self):
        self.assertEqual(
            audit_trace(
                self.selected_confirm(
                    after_discard=(card("Strike_R"),), relics=("Strange Spoon",)
                )
            ),
            ({"ExhaustAction": 1}, []),
        )

    def test_exhaust_requires_selected_identity_in_destination(self):
        # A later draw/generated duplicate can mask the total-card loss, but it
        # cannot replace the selected card's required ExhaustAction destination.
        path = self.selected_confirm(
            after_hand=(card("Strike_R"),), after_exhaust=(card("Defend_R"),)
        )
        self.assertEqual(audit_trace(path), ({"ExhaustAction": 1}, [(3, "ExhaustAction")]))

    def test_accepts_armaments_retrieval_after_upgrade(self):
        path = self.selected_confirm(
            owner="ArmamentsAction", after_hand=(card("Strike_R", upgrades=1),)
        )
        self.assertEqual(audit_trace(path), ({"ArmamentsAction": 1}, []))

    def test_ignores_legal_zero_card_confirm(self):
        path = self.write_trace(
            [
                state(1, "HAND_SELECT", hand=(card("Strike_R"),)),
                {"type": "action", "step": 2, "command": "CONFIRM"},
                state(2, "NONE", hand=(card("Strike_R"),), action=None),
            ]
        )
        self.assertEqual(audit_trace(path), ({}, []))


if __name__ == "__main__":
    unittest.main()
