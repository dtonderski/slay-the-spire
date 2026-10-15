"""Synthetic infrastructure tests; not historical gameplay parity evidence."""

import copy
import json
import sqlite3
import tempfile
import unittest
from pathlib import Path

from pretraining.choices import content, extract, history_before_floor, slate
from pretraining.map_audit import (
    act_route,
    hypothetical_choices,
    match_route,
    numeric_seed,
    permutation_test,
    zero_failure_upper,
)
from pretraining.source import ordinary, shuffled_records, split_for


def run():
    return {
        "character_chosen": "IRONCLAD",
        "ascension_level": 0,
        "play_id": "fixture",
        "build_version": "2020-07-30",
        "is_beta": False,
        "is_daily": False,
        "is_trial": False,
        "is_endless": False,
        "chose_seed": False,
        "is_prod": False,
        "neow_bonus": "HUNDRED_GOLD",
        "neow_cost": "NONE",
        "card_choices": [
            {"floor": 1, "picked": "Disarm", "not_picked": ["Clash", "Perfected Strike"]},
            {"floor": 2, "picked": "SKIP", "not_picked": ["Anger", "Flex", "Cleave"]},
            {"floor": 3, "picked": "Bash+1", "not_picked": ["Disarm", "Clash"]},
        ],
        "event_choices": [],
        "campfire_choices": [],
        "relics_obtained": [],
        "items_purchased": [],
        "item_purchase_floors": [],
        "items_purged": [],
        "items_purged_floors": [],
        "boss_relics": [{"picked": "Astrolabe", "not_picked": ["Runic Cube", "Runic Pyramid"]}],
        "master_deck": ["Bash"],
        "relics": ["Burning Blood"],
        "path_taken": ["M"] * 15 + ["BOSS"],
    }


def graph():
    # Two starting nodes match 'M'; only one can reach '?' on the next row.
    return {
        "seed": 123,
        "act": 1,
        "ascension": 0,
        "starts": [0, 1],
        "nodes": [
            {"row": 0, "x": 0, "symbol": "M", "children": [[1, 0], [1, 1]]},
            {"row": 0, "x": 1, "symbol": "M", "children": [[1, 1]]},
            {"row": 1, "x": 0, "symbol": "?", "children": []},
            {"row": 1, "x": 1, "symbol": "E", "children": []},
        ],
    }


class ChoiceExtractionTests(unittest.TestCase):
    def test_candidates_not_chosen_first_and_duplicate_equivalence(self):
        candidates, label = slate({"picked": "Disarm", "not_picked": ["Clash", "Disarm"]}, "card")
        self.assertEqual(candidates, sorted(set(candidates)))
        self.assertEqual(len(candidates), 2)
        self.assertEqual(label, content("Disarm", "card"))
        with self.assertRaises(ValueError):
            content("Definitely a mod", "card")
        self.assertEqual(content("Ghostly", "card"), content("Apparition", "card"))

    def test_skip_bowl_and_missing_boss_pick_are_not_fake_negatives(self):
        e = run()
        e["card_choices"].append({"floor": 4, "picked": "Singing Bowl", "not_picked": ["Clash"]})
        e["boss_relics"].append({"not_picked": ["Sozu"]})
        examples, audit, counts = extract(e)
        self.assertEqual(len(examples), 3)
        self.assertEqual(counts["card_choices_rejected:special_action_requires_eligibility"], 2)
        self.assertEqual(counts["boss_relics_rejected:missing_pick"], 1)
        self.assertEqual(len(audit), 3)

    def test_causal_prefix_future_fields_and_same_floor_are_excluded(self):
        e = run()
        baseline = history_before_floor(e, 3)
        self.assertIn(content("Disarm", "card"), baseline["card_picks"])
        self.assertNotIn(content("Bash+1", "card"), baseline["card_picks"])
        e.update(master_deck=["Secret future card"], victory=True, seed_played="999", score=999999)
        e["card_choices"].append({"floor": 4, "picked": "Unmapped future card", "not_picked": []})
        e["relics_obtained"].append({"floor": 3, "key": "War Paint"})
        self.assertEqual(history_before_floor(e, 3), baseline)

    def test_event_fields_are_edits_not_deck_reconstruction(self):
        e = run()
        e["event_choices"] = [
            {"floor": 2, "cards_transformed": ["Bash"], "cards_obtained": ["Armaments"], "relics_lost": ["Golden Idol"]}
        ]
        history = history_before_floor(e, 3)
        self.assertEqual(history["event_transforms"], {content("Bash", "card"): 1})
        self.assertEqual(history["event_gains"], {content("Armaments", "card"): 1})
        self.assertNotIn("deck", history)
        self.assertEqual(history["relic_losses"], {content("Golden Idol", "relic"): 1})

    def test_purchases_and_invalid_alignment(self):
        e = run()
        e.update(items_purchased=["Bash", "Black Star", "Fire Potion"], item_purchase_floors=[1, 1, 1])
        self.assertEqual(len(history_before_floor(e, 2)["purchases"]), 3)
        e["item_purchase_floors"].pop()
        with self.assertRaisesRegex(ValueError, "unaligned"):
            history_before_floor(e, 2)

    def test_relic_acquisition_is_not_a_choice_and_boss_has_no_invented_floor(self):
        e = run()
        e["relics_obtained"] = [{"floor": 7, "key": "War Paint"}]
        examples, audit, counts = extract(e)
        boss = [x for x in examples if x["kind"] == "relic"][0]
        self.assertNotIn("floor", boss["context"])
        self.assertNotIn("deck", boss["context"])
        self.assertEqual(counts["relic_acquisitions_not_demonstrations"], 1)
        self.assertTrue(any(a["reason"] == "acquisition_only_no_choice_slate" for a in audit))

    def test_required_flags_and_fractional_floors_fail_closed(self):
        e = run()
        ordinary(e)  # is_prod=False is intentionally accepted.
        del e["is_beta"]
        with self.assertRaises(ValueError):
            ordinary(e)
        e = run()
        e["card_choices"][0]["floor"] = 1.25
        _, _, counts = extract(e)
        self.assertGreater(counts["card_choices_rejected:invalid_floor"], 0)

    def test_split_is_stable_and_existing_holdout_is_preserved(self):
        import hashlib

        for i in range(1000):
            key = str(i)
            value = int.from_bytes(hashlib.sha256(key.encode()).digest()[:8], "big")
            self.assertEqual(split_for(key) == "test", value % 10 == 0)
            self.assertEqual(split_for(key), split_for(key))

    def test_readonly_sampling_repeats_without_mutation(self):
        with tempfile.TemporaryDirectory() as temp:
            db = Path(temp) / "runs.sqlite"
            with sqlite3.connect(db) as c:
                c.execute(
                    "CREATE TABLE runs(id INTEGER PRIMARY KEY,character,ascension,build_version,floor_reached,file_id,record_index,raw_json)"
                )
                for i in range(8):
                    c.execute(
                        "INSERT INTO runs VALUES(?,?,?,?,?,?,?,?)",
                        (i, "IRONCLAD", 0, "2020-07-30", 16, 1, i, json.dumps({"event": run()})),
                    )
            before = db.read_bytes()
            a = list(shuffled_records(db, ascension=0, build="2020-07-30", seed=7))
            b = list(shuffled_records(db, ascension=0, build="2020-07-30", seed=7))
            self.assertEqual(a, b)
            self.assertEqual(len(a), 8)
            self.assertEqual(before, db.read_bytes())


class MapAuditTests(unittest.TestCase):
    def test_signed_decimal_seed_not_base35_or_float(self):
        self.assertEqual(numeric_seed("-7637825359074474900"), -7637825359074474900)
        for seed in (True, 1.0, "VERIFY01", str(2**63), str(2**63 - 1)):
            with self.assertRaises(ValueError):
                numeric_seed(seed)

    def test_route_uses_map_symbols_not_realized_rooms(self):
        e = run()
        e["path_taken"][1] = "?"
        e["path_per_floor"] = ["M"] * 16
        self.assertEqual(act_route(e)[1], "?")
        e["ascension_level"] = 20
        with self.assertRaisesRegex(ValueError, "a0"):
            act_route(e)

    def test_acts_and_unmodeled_movement(self):
        e = run()
        e["path_taken"] += ["?"] * 15 + ["BOSS"] + ["E"] * 15 + ["BOSS"]
        self.assertEqual(act_route(e, 2), ["?"] * 15)
        self.assertEqual(act_route(e, 3), ["E"] * 15)
        e["relics"].append("WingedGreaves")
        with self.assertRaisesRegex(ValueError, "wing_boots"):
            act_route(e)

    def test_exact_symbol_matching_and_ambiguity(self):
        m = graph()
        unique = match_route(m, ["M", "?"])
        self.assertEqual(unique["compatible_paths"], 1)
        self.assertEqual(unique["possible_xs_by_row"], [[0], [0]])
        self.assertEqual(match_route(m, ["M", "E"])["compatible_paths"], 2)
        self.assertEqual(match_route(m, ["M", "M"])["compatible_paths"], 0)
        self.assertEqual(match_route(m, ["?", "E"])["compatible_paths"], 0)
        self.assertEqual(match_route(m, ["M", "M"])["first_incompatible_row"], 1)

    def test_hypothetical_labels_refuse_ambiguity_and_never_expose_seed(self):
        m = graph()
        self.assertEqual(hypothetical_choices(m, match_route(m, ["M", "E"])), [])
        examples = hypothetical_choices(m, match_route(m, ["M", "?"]))
        self.assertEqual(len(examples), 2)
        self.assertNotIn("seed", examples[0]["context"])
        self.assertNotIn("remaining_route", examples[0]["context"])
        self.assertEqual(examples[0]["candidates"], [[0, 0], [0, 1]])
        self.assertEqual(examples[0]["label"], [0, 0])

    def test_unvisited_map_difference_can_pass_same_route(self):
        original = graph()
        changed = copy.deepcopy(original)
        changed["nodes"][-1]["symbol"] = "$"
        self.assertNotEqual(original, changed)
        self.assertEqual(match_route(original, ["M", "?"]), match_route(changed, ["M", "?"]))

    def test_zero_failure_bound_is_not_99_9_percent_equality(self):
        self.assertAlmostEqual(zero_failure_upper(200), 0.03394912101018662)
        self.assertGreater(zero_failure_upper(6904), 0.001)
        self.assertLess(zero_failure_upper(6905), 0.001)
        self.assertEqual(permutation_test([[True, True], [True, True]], 99, 1)["monte_carlo_p"], 1)
        matrix = [[i == j for j in range(8)] for i in range(8)]
        self.assertEqual(permutation_test(matrix, 999, 1), permutation_test(matrix, 999, 1))
        self.assertLess(permutation_test(matrix, 999, 1)["monte_carlo_p"], 0.01)


class ChoiceTrainingTests(unittest.TestCase):
    def setUp(self):
        import torch

        torch.set_num_threads(1)
        torch.manual_seed(123)

    def examples(self):
        examples, _, _ = extract(run())
        return [{**e, "run_group": "synthetic", "split": "train"} for e in examples]

    def test_incomplete_or_changed_dataset_is_refused(self):
        from pretraining.source import digest
        from pretraining.train_choices import load_dataset

        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "examples.jsonl"
            path.write_text("".join(json.dumps(e) + "\n" for e in self.examples()))
            with self.assertRaisesRegex(ValueError, "summary"):
                load_dataset(path)
            (path.parent / "summary.json").write_text(
                json.dumps({"schema": 1, "status": "complete", "examples_sha256": digest(path.read_bytes())})
            )
            _, examples = load_dataset(path)
            self.assertEqual(len(examples), 3)
            path.write_text(path.read_text() + "\n")
            with self.assertRaisesRegex(ValueError, "hash"):
                load_dataset(path)

    def test_feature_whitelist_and_training_only_vocab(self):
        from pretraining.train_choices import feature_counts, vocabularies

        examples = self.examples()
        before = feature_counts(examples[0], True)
        examples[0].update(seed=999, victory=True, source={"row_id": 45}, debug_state="secret")
        self.assertEqual(feature_counts(examples[0], True), before)
        features, items = vocabularies(examples, True)
        self.assertNotIn("seed", features)
        self.assertNotIn("unseen_validation_card", items)

    def test_permutation_equivariant_logits_and_finite_gradients(self):
        import torch
        from pretraining.train_choices import ChoiceModel, vocabularies

        examples = self.examples()
        model = ChoiceModel(*vocabularies(examples, True))
        logits, labels = model(examples)
        permuted = copy.deepcopy(examples)
        for e in permuted:
            e["candidates"].reverse()
        new_logits, _ = model(permuted)
        for i, e in enumerate(examples):
            n = len(e["candidates"])
            torch.testing.assert_close(logits[i, :n], new_logits[i, :n].flip(0))
        loss = torch.nn.functional.cross_entropy(logits, labels)
        loss.backward()
        self.assertTrue(torch.isfinite(loss))
        self.assertTrue(all(p.grad is None or torch.isfinite(p.grad).all() for p in model.parameters()))

    def test_optimizer_learns_synthetic_choice(self):
        import torch
        from pretraining.train_choices import ChoiceModel, vocabularies

        examples = self.examples()[:1]
        model = ChoiceModel(*vocabularies(examples, True))
        opt = torch.optim.Adam(model.parameters(), lr=0.01)

        def loss():
            logits, labels = model(examples)
            return torch.nn.functional.cross_entropy(logits, labels)

        initial = float(loss().detach())
        for _ in range(15):
            opt.zero_grad()
            error = loss()
            error.backward()
            opt.step()
        self.assertLess(float(loss().detach()), initial)

    def test_group_leakage_and_hypothetical_routes_rejected(self):
        from pretraining.train_choices import validate_examples

        examples = self.examples()
        examples[-1]["split"] = "test"
        with self.assertRaisesRegex(ValueError, "crosses"):
            validate_examples(examples)
        with self.assertRaises(ValueError):
            validate_examples([{"schema": "hypothetical_route_v1"}])


if __name__ == "__main__":
    unittest.main()
