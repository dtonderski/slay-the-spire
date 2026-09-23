import copy
import json
import random
import tempfile
import unittest
from pathlib import Path

import torch
from combat_explorer.errors import ExplorerError
from combat_explorer.jsonutil import observation_sha256, seed_to_str
from combat_explorer.persistence import reconstruct, session_document, validate_document
from combat_explorer.policy import PolicyAdapter
from combat_explorer.presentation import action_descriptor
from combat_explorer.roots import RootService
from combat_explorer.sessions import SessionManager
from loadout_sampling import LoadoutSampler, LoadoutSpec, SampledCard, band_for
from model import CombatValueModel
from scenarios import COMBAT_FLOORS, sample_encounter
from sts_sim import PotionKey, State
from synthetic_roots import build_root
from validation_set import build_validation, native_sha256


def sampler() -> LoadoutSampler:
    band = {
        "runs": 1,
        "deck_sizes": {"10": 1},
        "cards": {"Strike_R": {"0": 5}, "Defend_R": {"0": 4}, "Bash": {"0": 1}},
        "starters": {"Burning Blood": 1},
        "other_relic_counts": {"0": 1},
        "other_relics": {},
        "potions_obtained": {"block": 1},
        "max_hp": {"80": 1},
    }
    return LoadoutSampler(
        {
            "schema": 1,
            "ascension": 0,
            "identity_namespace": "sts_sim_public_base_keys",
            "split_role": "fit",
            "bands": {band_for(f): band for f in COMBAT_FLOORS},
        }
    )


def ironclad_loadout(floor: int, potions: tuple[str | None, ...] = (None, None, None)) -> LoadoutSpec:
    return LoadoutSpec(
        floor,
        (SampledCard("Strike_R", 0),) * 5 + (SampledCard("Defend_R", 0),) * 4 + (SampledCard("Bash", 0),),
        ("Burning Blood",),
        potions,
        80,
        40,
        "test",
    )


def prepared_from_spec(spec_json: str):
    from combat_explorer.roots import PreparedRoot

    state = State.from_synthetic_spec(spec_json)
    spec = json.loads(spec_json)
    return PreparedRoot(
        spec_json,
        spec,
        state,
        {
            "kind": "generated",
            "seed": seed_to_str(spec["seed"]),
            "encounter": spec["encounter"],
            "native_sha256": native_sha256(),
            "initial_observation_sha256": observation_sha256(state.observation()),
        },
    )


class RootTests(unittest.TestCase):
    def test_manifest_round_trip_and_legacy_error(self) -> None:
        document = build_validation(sampler(), 7, 1, 1, repeats=1)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "validation.json"
            path.write_text(json.dumps(document))
            service = RootService(validation_manifest=path, distributions=None)
            cases = service.list_cases()
            self.assertTrue(any(case["id"].startswith("main-") for case in cases))
            first = cases[0]
            self.assertIsInstance(first["seed"], str)
            prepared = service.from_case(first["id"])
            self.assertEqual(prepared.provenance["case_id"], first["id"])
            original = observation_sha256(prepared.state.observation())
            service.from_case(first["id"])
            self.assertEqual(original, observation_sha256(prepared.state.observation()))
            legacy = Path(directory) / "roots.json"
            legacy.write_text(json.dumps({"val": []}))
            with self.assertRaises(ValueError):
                RootService(validation_manifest=legacy, distributions=None)

    def test_generation_is_deterministic_and_isolated_from_policy_rng(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory) / "fit.json"
            dist.write_text(json.dumps(sampler().distributions))
            service = RootService(validation_manifest=None, distributions=dist)
            first = service.generate(generation_seed="42", floor=1)
            rng_state = random.getstate()
            second = service.generate(generation_seed="42", floor=1)
            self.assertEqual(first.spec_json, second.spec_json)
            self.assertEqual(rng_state, random.getstate())
            self.assertNotEqual(service.generate(generation_seed="43", floor=1).spec_json, first.spec_json)
            self.assertEqual(service.generate(generation_seed="42", floor=1).spec_json, first.spec_json)
            huge = 2**63 + 99
            generated = service.generate(generation_seed=str(huge), min_floor=1, max_floor=1)
            self.assertEqual(generated.provenance["generation_seed"], str(huge))
            self.assertEqual(int(generated.provenance["seed"]), json.loads(generated.spec_json)["seed"])


class BranchingTests(unittest.TestCase):
    def setUp(self) -> None:
        spec = sample_encounter(random.Random(0), 1)
        root = build_root(spec, ironclad_loadout(1), 123)
        self.manager = SessionManager(PolicyAdapter())
        self.session = self.manager.create(prepared_from_spec(root.spec_json), model=None)

    def tearDown(self) -> None:
        self.manager.close()

    def _end_turn_index(self, node_id: str) -> tuple[int, int, dict]:
        node = self.session.nodes[node_id]
        decision = node.state.decision()
        index = next(i for i, action in enumerate(decision.actions) if action.kind == "end_turn")
        return index, decision.revision, action_descriptor(decision.actions[index])

    def test_siblings_do_not_mutate_parent(self) -> None:
        root_id = self.session.root_id
        parent_hash = self.session.nodes[root_id].observation_sha256
        parent_obs = self.session.nodes[root_id].observation
        first_index, revision, descriptor = self._end_turn_index(root_id)
        child_a = self.manager.act(self.session, root_id, native_index=first_index, revision=revision, descriptor=descriptor, request_id="a")
        play = None
        node = self.session.nodes[root_id]
        decision = node.state.decision()
        for index, action in enumerate(decision.actions):
            if action.kind == "play_hand_slot":
                play = (index, action_descriptor(action))
                break
        self.assertIsNotNone(play)
        child_b = self.manager.act(
            self.session,
            root_id,
            native_index=play[0],
            revision=revision,
            descriptor=play[1],
            request_id="b",
        )
        self.assertNotEqual(child_a["child_id"], child_b["child_id"])
        self.assertEqual(self.session.nodes[root_id].observation_sha256, parent_hash)
        self.assertEqual(self.session.nodes[root_id].observation, parent_obs)
        self.assertEqual(set(self.session.nodes[root_id].child_ids), {child_a["child_id"], child_b["child_id"]})
        self.assertEqual(self.session.nodes[child_a["child_id"]].observation_sha256, child_a["node"]["incoming"] and self.session.nodes[child_a["child_id"]].observation_sha256)

    def test_delete_node_removes_subtree_not_parent(self) -> None:
        root_id = self.session.root_id
        index, revision, descriptor = self._end_turn_index(root_id)
        child = self.manager.act(
            self.session, root_id, native_index=index, revision=revision, descriptor=descriptor, request_id="del-a"
        )
        child_id = child["child_id"]
        result = self.manager.delete_node(self.session, child_id)
        self.assertEqual(result["parent_id"], root_id)
        self.assertIn(child_id, result["deleted"])
        self.assertNotIn(child_id, self.session.nodes)
        self.assertEqual(self.session.nodes[root_id].child_ids, [])
        with self.assertRaises(ExplorerError):
            self.manager.delete_node(self.session, root_id)

    def test_same_parent_same_action_reuses_child(self) -> None:
        root_id = self.session.root_id
        index, revision, descriptor = self._end_turn_index(root_id)
        first = self.manager.act(
            self.session, root_id, native_index=index, revision=revision, descriptor=descriptor, request_id="reuse-a"
        )
        second = self.manager.act(
            self.session, root_id, native_index=index, revision=revision, descriptor=descriptor, request_id="reuse-b"
        )
        self.assertEqual(first["child_id"], second["child_id"])
        self.assertEqual(self.session.nodes[root_id].child_ids, [first["child_id"]])
        incoming = self.session.nodes[first["child_id"]].incoming
        self.assertEqual(incoming["native_index"], index)
        self.assertEqual(incoming["actor"], "human")

    def test_stale_and_descriptor_mismatch_are_atomic(self) -> None:
        root_id = self.session.root_id
        index, revision, descriptor = self._end_turn_index(root_id)
        with self.assertRaises(ExplorerError):
            self.manager.act(self.session, root_id, native_index=index, revision=revision + 1, descriptor=descriptor, request_id=None)
        with self.assertRaises(ExplorerError):
            self.manager.act(self.session, root_id, native_index=index, revision=revision, descriptor={**descriptor, "kind": "play_hand_slot"}, request_id=None)
        self.assertEqual(self.session.nodes[root_id].child_ids, [])

    def test_smoke_bomb_use_is_filtered(self) -> None:
        spec = sample_encounter(random.Random(0), 1)
        root = build_root(spec, ironclad_loadout(1, (PotionKey.SMOKE_BOMB, None, None)), 5)
        session = self.manager.create(prepared_from_spec(root.spec_json), model=None)
        node = session.nodes[session.root_id]
        decision = node.state.decision()
        bomb = [i for i, action in enumerate(decision.actions) if action.kind == "use_potion_slot" and action.potion_slot == 0]
        self.assertTrue(bomb)
        self.assertNotIn(bomb[0], node.allowed_native_indices)
        with self.assertRaises(ExplorerError):
            self.manager.act(session, node.id, native_index=bomb[0], revision=node.revision, descriptor=action_descriptor(decision.actions[bomb[0]]), request_id=None)

    def test_nested_selection_is_not_skipped(self) -> None:
        spec = sample_encounter(random.Random(1), 1)
        cards = LoadoutSpec(
            1,
            (SampledCard("True Grit", 1),) * 5 + (SampledCard("Defend_R", 0),) * 5,
            ("Burning Blood",),
            (None, None, None),
            80,
            80,
            "test",
        )
        found = False
        for seed in range(80):
            root = build_root(spec, cards, seed)
            session = self.manager.create(prepared_from_spec(root.spec_json), model=None)
            node = session.nodes[session.root_id]
            decision = node.state.decision()
            def hand_key(slot):
                for entry in decision.observation.screen.hand:
                    if entry.slot == slot:
                        return str(entry.card.content_key)
                return ""

            grit = [
                (i, action)
                for i, action in enumerate(decision.actions)
                if action.kind == "play_hand_slot" and action.hand_slot is not None
                and hand_key(action.hand_slot) in ("True Grit", "True Grit+")
            ]
            if not grit:
                continue
            index, action = grit[0]
            result = self.manager.act(
                session,
                node.id,
                native_index=index,
                revision=node.revision,
                descriptor=action_descriptor(action),
                request_id=None,
            )
            child = session.nodes[result["child_id"]]
            kinds = {item["kind"] for item in child.action_manifest}
            if child.observation.kind == "combat" and child.observation.screen.selection is not None:
                found = True
                self.assertTrue(kinds & {"toggle_visible_card", "choose_visible_option", "confirm_selection", "skip_selection"})
                self.assertIsNone(child.outcome)
                break
        self.assertTrue(found, "expected a True Grit selection decision")

    def test_terminal_cannot_advance_and_replay_matches(self) -> None:
        session = self.session
        node_id = session.root_id
        for step in range(80):
            node = session.nodes[node_id]
            if node.outcome is not None:
                with self.assertRaises(ExplorerError):
                    self.manager.act(session, node_id, native_index=0, revision=node.revision, descriptor=None, request_id=None)
                break
            decision = node.state.decision()
            index = next((i for i, action in enumerate(decision.actions) if action.kind == "end_turn"), 0)
            if index not in node.allowed_native_indices:
                index = node.allowed_native_indices[0]
            result = self.manager.act(
                session,
                node_id,
                native_index=index,
                revision=node.revision,
                descriptor=action_descriptor(decision.actions[index]),
                request_id=f"s{step}",
            )
            node_id = result["child_id"]
        document = session_document(session)
        validate_document(document)
        replayed = reconstruct(self.manager, copy.deepcopy(document))
        self.assertEqual({node.observation_sha256 for node in session.nodes.values()}, {node.observation_sha256 for node in replayed.nodes.values()})
        self.assertEqual(len(session.nodes), len(replayed.nodes))


class PersistenceTests(unittest.TestCase):
    def test_corrupt_documents_fail_closed(self) -> None:
        spec = sample_encounter(random.Random(0), 1)
        root = build_root(spec, ironclad_loadout(1), 9)
        manager = SessionManager(PolicyAdapter())
        session = manager.create(prepared_from_spec(root.spec_json), model=None)
        document = session_document(session)
        document["nodes"].append({"id": "ghost", "parent_id": "missing", "child_ids": []})
        with self.assertRaises(ExplorerError):
            validate_document(document)
        missing_child = session_document(session)
        missing_child["nodes"][0]["child_ids"] = ["child_does_not_exist"]
        with self.assertRaises(ExplorerError):
            validate_document(missing_child)
        bad_selected = session_document(session)
        bad_selected["ui"]["selected_node_id"] = "nonexistent"
        with self.assertRaises(ExplorerError):
            validate_document(bad_selected)
        list_selected = session_document(session)
        list_selected["ui"]["selected_node_id"] = []
        with self.assertRaises(ExplorerError):
            validate_document(list_selected)
        int_selected = session_document(session)
        int_selected["ui"]["selected_node_id"] = 1
        with self.assertRaises(ExplorerError):
            validate_document(int_selected)
        list_ui = session_document(session)
        list_ui["ui"] = []
        with self.assertRaises(ExplorerError):
            validate_document(list_ui)
        list_pref = session_document(session)
        list_pref["ui"]["preferred_children"] = {session.root_id: []}
        with self.assertRaises(ExplorerError):
            validate_document(list_pref)
        list_prov = session_document(session)
        list_prov["provenance"] = []
        with self.assertRaises(ExplorerError):
            validate_document(list_prov)
        nested: dict = {}
        cursor = nested
        for _ in range(80):
            cursor["x"] = {}
            cursor = cursor["x"]
        deep = session_document(session)
        deep["provenance"] = nested
        with self.assertRaises(ExplorerError):
            validate_document(deep)
        broken = session_document(session)
        broken["native_sha256"] = "nope"
        with self.assertRaises(ExplorerError):
            reconstruct(manager, broken)
        manager.close()

    def test_multibranch_roundtrip_and_continue_without_model(self) -> None:
        spec = sample_encounter(random.Random(0), 1)
        root = build_root(spec, ironclad_loadout(1), 11)
        manager = SessionManager(PolicyAdapter())
        session = manager.create(prepared_from_spec(root.spec_json), model=None)
        root_id = session.root_id
        node = session.nodes[root_id]
        decision = node.state.decision()
        end = next(i for i, action in enumerate(decision.actions) if action.kind == "end_turn")
        play = next(i for i, action in enumerate(decision.actions) if action.kind == "play_hand_slot")
        child_a = manager.act(
            session,
            root_id,
            native_index=end,
            revision=node.revision,
            descriptor=action_descriptor(decision.actions[end]),
            request_id="a",
        )
        child_b = manager.act(
            session,
            root_id,
            native_index=play,
            revision=node.revision,
            descriptor=action_descriptor(decision.actions[play]),
            request_id="b",
        )
        document = session_document(
            session,
            selected_node_id=child_b["child_id"],
            preferred_children={root_id: child_b["child_id"]},
        )
        replayed = reconstruct(manager, copy.deepcopy(document))
        self.assertEqual(set(session.nodes), set(replayed.nodes))
        self.assertEqual(
            {node_id: session.nodes[node_id].observation_sha256 for node_id in session.nodes},
            {node_id: replayed.nodes[node_id].observation_sha256 for node_id in replayed.nodes},
        )
        self.assertIsNone(replayed.model)
        leaf = replayed.nodes[child_a["child_id"]]
        if leaf.outcome is None:
            decision = leaf.state.decision()
            index = leaf.allowed_native_indices[0]
            continued = manager.act(
                replayed,
                leaf.id,
                native_index=index,
                revision=leaf.revision,
                descriptor=action_descriptor(decision.actions[index]),
                request_id="loaded-manual",
            )
            self.assertIn(continued["child_id"], replayed.nodes)
            self.assertEqual(session.nodes[child_a["child_id"]].observation_sha256, leaf.observation_sha256)
        tampered = copy.deepcopy(document)
        child_raw = next(node for node in tampered["nodes"] if node["id"] == child_b["child_id"])
        child_raw["summary"] = {"action": "tampered"}
        child_raw["incoming"]["summary"] = {"action": "tampered"}
        child_raw["markers"] = {"turn_end": False, "kills": [{"content_key": "fake"}]}
        child_raw["incoming"]["markers"] = child_raw["markers"]
        recomputed = reconstruct(manager, tampered)
        restored_child = recomputed.nodes[child_b["child_id"]]
        self.assertNotEqual(restored_child.summary.get("action"), "tampered")
        self.assertEqual(restored_child.summary.get("kind"), session.nodes[child_b["child_id"]].summary.get("kind"))
        self.assertEqual(restored_child.markers.get("kills"), session.nodes[child_b["child_id"]].markers.get("kills"))
        manager.close()


class ModelStepTests(unittest.TestCase):
    def test_greedy_and_sampled_steps_create_history(self) -> None:
        spec = sample_encounter(random.Random(0), 1)
        root = build_root(spec, ironclad_loadout(1), 3)
        adapter = PolicyAdapter()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "m.pt"
            torch.save({"model": CombatValueModel().state_dict()}, path)
            loaded = adapter.load(path)
            manager = SessionManager(adapter)
            session = manager.create(prepared_from_spec(root.spec_json), model=loaded)
            analysis = manager.analyze(session, session.root_id, mode="sample", temperature=1.0)
            self.assertAlmostEqual(sum(analysis["base_probabilities"]), 1.0, places=5)
            self.assertEqual(len(analysis["native_indices"]), len(analysis["base_probabilities"]))
            first = manager.model_step(session, session.root_id, mode="greedy", temperature=1.0, sampling_seed="1", request_id="g")
            second = manager.model_step(session, session.root_id, mode="sample", temperature=1.0, sampling_seed="7", request_id="s")
            distinct = (session.nodes[first["child_id"]].incoming["native_index"] !=
                        session.nodes[second["child_id"]].incoming["native_index"])
            self.assertEqual(len(session.nodes[session.root_id].child_ids), 2 if distinct else 1)
            self.assertEqual(first["child_id"] == second["child_id"], not distinct)
            repeated = manager.model_step(session, session.root_id, mode="sample", temperature=1.0, sampling_seed="7", request_id="s")
            self.assertEqual(repeated["child_id"], second["child_id"])
            import random as py_random

            before = py_random.getstate()
            manager.analyze(session, session.root_id, mode="sample", temperature=1.2)
            self.assertEqual(py_random.getstate(), before)
            child = session.nodes[first["child_id"]]
            self.assertTrue(child.incoming["diagnostics"]["historical"])
            self.assertEqual(child.incoming["diagnostics"]["mode"], "greedy")
            payload = manager.node_payload(session, session.root_id)
            self.assertIsNone(payload["current_analysis"])
            outgoing = next(edge for edge in payload["outgoing"] if edge["child_id"] == first["child_id"])
            self.assertEqual(outgoing["native_index"], child.incoming["native_index"])
            self.assertTrue(outgoing["diagnostics"]["historical"])
            manager.close()


if __name__ == "__main__":
    unittest.main()
