import json
import tempfile
import threading
import time
import unittest
from pathlib import Path

import torch
from combat_explorer import FORMAT_NAME
from combat_explorer.server import AppConfig, create_app
from fastapi.testclient import TestClient
from loadout_sampling import LoadoutSampler, band_for
from model import CombatValueModel
from scenarios import COMBAT_FLOORS
from validation_set import build_validation


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


class ApiTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmpdir = tempfile.TemporaryDirectory()
        root = Path(self.tmpdir.name)
        self.manifest = root / "validation.json"
        self.distributions = root / "fit.json"
        self.checkpoint = root / "latest.pt"
        self.sessions = root / "sessions"
        self.distributions.write_text(json.dumps(sampler().distributions))
        self.manifest.write_text(json.dumps(build_validation(sampler(), 7, 1, 1, repeats=1)))
        torch.save({"model": CombatValueModel().state_dict(), "config": {"test": True}}, self.checkpoint)
        app = create_app(
            AppConfig(
                validation_manifest=self.manifest,
                distributions=self.distributions,
                sessions_dir=self.sessions,
                checkpoint=self.checkpoint,
                extra_allowed_files=[root],
                allowed_hosts=("testserver", "127.0.0.1", "localhost"),
            )
        )
        self.client = TestClient(app)

    def tearDown(self) -> None:
        self.client.app.state.explorer.close()  # type: ignore[attr-defined]
        self.tmpdir.cleanup()

    def wait_job(self, job_id: str, timeout: float = 30.0) -> dict:
        deadline = time.time() + timeout
        while time.time() < deadline:
            payload = self.client.get(f"/api/jobs/{job_id}").json()
            if payload["status"] != "running":
                return payload
            time.sleep(0.05)
        self.fail(f"job {job_id} did not finish")

    def test_static_and_capabilities(self) -> None:
        home = self.client.get("/")
        self.assertEqual(home.status_code, 200)
        self.assertIn(b"Combat Explorer", home.content)
        js = self.client.get("/static/app.js")
        self.assertEqual(js.status_code, 200)
        caps = self.client.get("/api/capabilities").json()
        self.assertEqual(caps["default_mode"], "sample")
        self.assertEqual(caps["default_temperature"], 1.0)
        self.assertIsNotNone(caps["checkpoint_loaded"])

    def test_rejects_foreign_origin_and_path_escape(self) -> None:
        response = self.client.post(
            "/api/sessions/generated",
            json={"generation_seed": "1", "floor": 1},
            headers={"Origin": "http://evil.example"},
        )
        self.assertEqual(response.status_code, 403)
        session = self.client.post("/api/sessions/generated", json={"generation_seed": "1", "floor": 1}).json()
        bad = self.client.post(f"/api/sessions/{session['session_id']}/save", json={"filename": "../secret.json"})
        self.assertEqual(bad.status_code, 400)

    def test_root_generate_act_branch_save_load_continue(self) -> None:
        roots = self.client.get("/api/roots").json()["cases"]
        self.assertTrue(roots)
        self.assertIsInstance(roots[0]["seed"], str)
        loaded = self.client.post("/api/sessions/from-root", json={"case_id": roots[0]["id"]}).json()
        generated = self.client.post(
            "/api/sessions/generated",
            json={"generation_seed": str(2**62 + 5), "floor": 1, "request_id": "gen-1"},
        ).json()
        self.assertEqual(generated["provenance"]["generation_seed"], str(2**62 + 5))
        session_id = generated["session_id"]
        root_id = generated["root_id"]
        node = self.client.get(f"/api/sessions/{session_id}/nodes/{root_id}").json()
        self.assertIn("board", node)
        self.assertTrue(node["actions"])
        analysis = self.client.post(
            f"/api/sessions/{session_id}/nodes/{root_id}/analyze",
            json={"mode": "sample", "temperature": 1.0},
        ).json()
        self.assertAlmostEqual(sum(analysis["base_probabilities"]), 1.0, places=5)
        end_turn = next(action for action in node["actions"] if action["descriptor"]["kind"] == "end_turn")
        first = self.client.post(
            f"/api/sessions/{session_id}/nodes/{root_id}/act",
            json={
                "native_index": end_turn["native_index"],
                "revision": node["revision"],
                "descriptor": end_turn["descriptor"],
                "request_id": "act-1",
            },
        ).json()
        duplicate = self.client.post(
            f"/api/sessions/{session_id}/nodes/{root_id}/act",
            json={
                "native_index": end_turn["native_index"],
                "revision": node["revision"],
                "descriptor": end_turn["descriptor"],
                "request_id": "act-1",
            },
        ).json()
        self.assertEqual(first["child_id"], duplicate["child_id"])
        again = self.client.post(
            f"/api/sessions/{session_id}/nodes/{root_id}/act",
            json={
                "native_index": end_turn["native_index"],
                "revision": node["revision"],
                "descriptor": end_turn["descriptor"],
                "request_id": "act-1-again",
            },
        ).json()
        self.assertEqual(first["child_id"], again["child_id"])
        play = next(action for action in node["actions"] if action["descriptor"]["kind"] == "play_hand_slot" and action["allowed"])
        second = self.client.post(
            f"/api/sessions/{session_id}/nodes/{root_id}/act",
            json={
                "native_index": play["native_index"],
                "revision": node["revision"],
                "descriptor": play["descriptor"],
                "request_id": "act-2",
            },
        ).json()
        tree = self.client.get(f"/api/sessions/{session_id}/tree").json()
        root = next(item for item in tree["nodes"] if item["id"] == root_id)
        self.assertEqual(set(root["child_ids"]), {first["child_id"], second["child_id"]})
        for item in tree["nodes"]:
            self.assertIsInstance(item["hp"], int)
            self.assertGreater(item["max_hp"], 0)
        save = self.client.post(
            f"/api/sessions/{session_id}/save",
            json={"filename": "branch.json", "selected_node_id": second["child_id"]},
        ).json()
        self.assertTrue(Path(save["path"]).is_file())
        restored = self.client.post("/api/sessions/load", json={"filename": "branch.json"}).json()
        self.assertEqual(len(restored["nodes"]), len(tree["nodes"]))
        job = self.client.post(
            f"/api/sessions/{restored['session_id']}/nodes/{restored['root_id']}/continue",
            json={"mode": "greedy", "max_decisions": 8, "request_id": "job-1"},
        ).json()
        finished = self.wait_job(job["id"])
        self.assertIn(finished["reason"], {"victory", "defeat", "limit"})
        self.assertIsNone(finished["error"])
        self.assertGreaterEqual(finished["generated"], 1)
        after = self.client.get(f"/api/sessions/{restored['session_id']}/tree").json()
        self.assertGreater(len(after["nodes"]), len(restored["nodes"]))
        self.assertTrue(any(item["id"] == job["id"] for item in after["jobs"]))

    def test_busy_rejects_mutation_and_cancel_leaves_prefix(self) -> None:
        generated = self.client.post("/api/sessions/generated", json={"generation_seed": "9", "floor": 1}).json()
        session_id = generated["session_id"]
        root_id = generated["root_id"]
        node = self.client.get(f"/api/sessions/{session_id}/nodes/{root_id}").json()
        started = threading.Event()
        original_continue = self.client.app.state.explorer.sessions.start_continue  # type: ignore[attr-defined]

        def wrapped(*args, **kwargs):
            started.set()
            return original_continue(*args, **kwargs)

        self.client.app.state.explorer.sessions.start_continue = wrapped  # type: ignore[attr-defined]
        job = self.client.post(
            f"/api/sessions/{session_id}/nodes/{root_id}/continue",
            json={"mode": "sample", "temperature": 1.0, "max_decisions": 64, "request_id": "slow"},
        ).json()
        busy = self.client.post(
            f"/api/sessions/{session_id}/nodes/{root_id}/act",
            json={
                "native_index": node["actions"][0]["native_index"],
                "revision": node["revision"],
                "descriptor": node["actions"][0]["descriptor"],
            },
        )
        self.assertEqual(busy.status_code, 409)
        cancel = self.client.post(f"/api/jobs/{job['id']}/cancel").json()
        self.assertTrue(cancel["id"])
        finished = self.wait_job(job["id"])
        self.assertIn(finished["reason"], {"cancelled", "victory", "defeat", "limit"})
        self.assertIsNone(finished["error"])
        tree = self.client.get(f"/api/sessions/{session_id}/tree").json()
        reads = self.client.get(f"/api/sessions/{session_id}/nodes/{root_id}")
        self.assertEqual(reads.status_code, 200)
        self.assertGreaterEqual(len(tree["nodes"]), 1)
        self.assertIsNone(tree["busy"])

    def test_no_model_manual_play_and_loaded_index(self) -> None:
        self.assertIn("Combat Explorer", self.client.get("/").text)
        # A second app without checkpoint still serves manual play.
        with tempfile.TemporaryDirectory() as directory:
            sessions = Path(directory) / "s"
            app = create_app(
                AppConfig(
                    validation_manifest=self.manifest,
                    distributions=self.distributions,
                    sessions_dir=sessions,
                    checkpoint=None,
                    allowed_hosts=("testserver", "127.0.0.1", "localhost"),
                )
            )
            client = TestClient(app)
            session = client.post("/api/sessions/from-root", json={"case_id": client.get("/api/roots").json()["cases"][0]["id"]}).json()
            node = client.get(f"/api/sessions/{session['session_id']}/nodes/{session['root_id']}").json()
            action = next(item for item in node["actions"] if item["allowed"])
            played = client.post(
                f"/api/sessions/{session['session_id']}/nodes/{session['root_id']}/act",
                json={"native_index": action["native_index"], "revision": node["revision"], "descriptor": action["descriptor"]},
            )
            self.assertEqual(played.status_code, 200)
            denied = client.post(
                f"/api/sessions/{session['session_id']}/nodes/{session['root_id']}/model-step",
                json={"mode": "greedy"},
            )
            self.assertEqual(denied.status_code, 400)
            app.state.explorer.close()

    def test_same_origin_wrong_port_and_malformed_origin(self) -> None:
        generated = {"generation_seed": "1", "floor": 1}
        ok = self.client.post("/api/sessions/generated", json=generated, headers={"Origin": "http://testserver"})
        self.assertEqual(ok.status_code, 200, ok.text)
        wrong_port = self.client.post(
            "/api/sessions/generated",
            json=generated,
            headers={"Host": "127.0.0.1:8765", "Origin": "http://127.0.0.1:9999"},
        )
        self.assertEqual(wrong_port.status_code, 403)
        same_host_wrong_port = self.client.post(
            "/api/sessions/generated",
            json=generated,
            headers={"Host": "127.0.0.1:8765", "Origin": "http://127.0.0.1:9999"},
        )
        self.assertEqual(same_host_wrong_port.status_code, 403)
        malformed = self.client.post(
            "/api/sessions/generated",
            json=generated,
            headers={"Origin": "not a url"},
        )
        self.assertEqual(malformed.status_code, 403)
        path_origin = self.client.post(
            "/api/sessions/generated",
            json=generated,
            headers={"Origin": "http://testserver/evil"},
        )
        self.assertEqual(path_origin.status_code, 403)

    def test_does_not_expose_sibling_artifacts(self) -> None:
        sibling = Path(self.tmpdir.name) / "sibling.pt"
        torch.save({"model": CombatValueModel().state_dict()}, sibling)
        with tempfile.TemporaryDirectory() as directory:
            app = create_app(
                AppConfig(
                    validation_manifest=self.manifest,
                    distributions=self.distributions,
                    sessions_dir=Path(directory),
                    checkpoint=self.checkpoint,
                    extra_allowed_files=[],
                    allowed_hosts=("testserver", "127.0.0.1", "localhost"),
                )
            )
            client = TestClient(app)
            session = client.post("/api/sessions/generated", json={"generation_seed": "1", "floor": 1}).json()
            denied = client.post(
                f"/api/sessions/{session['session_id']}/model",
                json={"path": str(sibling)},
            )
            self.assertEqual(denied.status_code, 400)
            allowed = client.post(
                f"/api/sessions/{session['session_id']}/model",
                json={"path": str(self.checkpoint)},
            )
            self.assertEqual(allowed.status_code, 200, allowed.text)
            app.state.explorer.close()

    def test_invalid_sampling_seed_does_not_leave_session_busy(self) -> None:
        generated = self.client.post("/api/sessions/generated", json={"generation_seed": "1", "floor": 1}).json()
        response = self.client.post(
            f"/api/sessions/{generated['session_id']}/nodes/{generated['root_id']}/continue",
            json={"mode": "sample", "sampling_seed": "not-a-number", "max_decisions": 4},
        )
        self.assertEqual(response.status_code, 400)
        self.assertEqual(response.json()["code"], "invalid_seed")
        tree = self.client.get(f"/api/sessions/{generated['session_id']}/tree").json()
        self.assertIsNone(tree["busy"])
        node = self.client.get(f"/api/sessions/{generated['session_id']}/nodes/{generated['root_id']}").json()
        action = next(item for item in node["actions"] if item["allowed"])
        played = self.client.post(
            f"/api/sessions/{generated['session_id']}/nodes/{generated['root_id']}/act",
            json={
                "native_index": action["native_index"],
                "revision": node["revision"],
                "descriptor": action["descriptor"],
            },
        )
        self.assertEqual(played.status_code, 200, played.text)

    def test_worker_failure_settles_job_and_preserves_prefix(self) -> None:
        generated = self.client.post("/api/sessions/generated", json={"generation_seed": "3", "floor": 1}).json()
        session_id = generated["session_id"]
        root_id = generated["root_id"]
        sessions = self.client.app.state.explorer.sessions  # type: ignore[attr-defined]
        original = sessions._model_step_apply

        def boom(*args, **kwargs):
            raise RuntimeError("injected worker failure")

        sessions._model_step_apply = boom  # type: ignore[method-assign]
        try:
            job = self.client.post(
                f"/api/sessions/{session_id}/nodes/{root_id}/continue",
                json={"mode": "greedy", "max_decisions": 4, "request_id": "boom"},
            ).json()
            finished = self.wait_job(job["id"])
        finally:
            sessions._model_step_apply = original  # type: ignore[method-assign]
        self.assertEqual(finished["status"], "failed")
        self.assertEqual(finished["reason"], "model_error")
        self.assertIn("injected worker failure", finished["error"])
        tree = self.client.get(f"/api/sessions/{session_id}/tree").json()
        self.assertIsNone(tree["busy"])
        self.assertEqual(len(tree["nodes"]), 1)
        node = self.client.get(f"/api/sessions/{session_id}/nodes/{root_id}").json()
        action = next(item for item in node["actions"] if item["allowed"])
        played = self.client.post(
            f"/api/sessions/{session_id}/nodes/{root_id}/act",
            json={
                "native_index": action["native_index"],
                "revision": node["revision"],
                "descriptor": action["descriptor"],
            },
        )
        self.assertEqual(played.status_code, 200, played.text)

    def test_idempotency_rejects_payload_mismatch(self) -> None:
        generated = self.client.post("/api/sessions/generated", json={"generation_seed": "4", "floor": 1}).json()
        session_id = generated["session_id"]
        root_id = generated["root_id"]
        node = self.client.get(f"/api/sessions/{session_id}/nodes/{root_id}").json()
        allowed = [item for item in node["actions"] if item["allowed"]]
        first = allowed[0]
        second = allowed[1] if len(allowed) > 1 else allowed[0]
        played = self.client.post(
            f"/api/sessions/{session_id}/nodes/{root_id}/act",
            json={
                "native_index": first["native_index"],
                "revision": node["revision"],
                "descriptor": first["descriptor"],
                "request_id": "same-key",
            },
        )
        self.assertEqual(played.status_code, 200, played.text)
        conflict = self.client.post(
            f"/api/sessions/{session_id}/nodes/{root_id}/act",
            json={
                "native_index": second["native_index"],
                "revision": node["revision"],
                "descriptor": second["descriptor"],
                "request_id": "same-key",
            },
        )
        self.assertEqual(conflict.status_code, 409)
        self.assertEqual(conflict.json()["code"], "idempotency_conflict")

    def test_analyze_uses_requested_settings_not_get_query(self) -> None:
        generated = self.client.post("/api/sessions/generated", json={"generation_seed": "5", "floor": 1}).json()
        session_id = generated["session_id"]
        root_id = generated["root_id"]
        node = self.client.get(f"/api/sessions/{session_id}/nodes/{root_id}?analyze=true").json()
        self.assertIsNone(node["current_analysis"])
        self.assertIn("outgoing", node)
        cool = self.client.post(
            f"/api/sessions/{session_id}/nodes/{root_id}/analyze",
            json={"mode": "sample", "temperature": 0.2},
        ).json()
        self.assertEqual(cool["temperature"], 0.2)
        self.assertFalse(cool["historical"])
        greedy = self.client.post(
            f"/api/sessions/{session_id}/nodes/{root_id}/analyze",
            json={"mode": "greedy", "temperature": 0.2},
        ).json()
        self.assertIsNone(greedy["temperature"])
        self.assertEqual(greedy["mode"], "greedy")
        self.assertIsNone(greedy["adjusted_probabilities"])

    def test_missing_and_malformed_session_files_are_client_errors(self) -> None:
        missing = self.client.post("/api/sessions/load", json={"filename": "no-such-session.json"})
        self.assertEqual(missing.status_code, 404)
        self.assertNotEqual(missing.status_code, 500)
        path = self.sessions / "broken.json"
        path.write_text("{not json")
        broken = self.client.post("/api/sessions/load", json={"filename": "broken.json"})
        self.assertEqual(broken.status_code, 400)
        generated = self.client.post("/api/sessions/generated", json={"generation_seed": "6", "floor": 1}).json()
        saved = self.client.post(
            f"/api/sessions/{generated['session_id']}/save",
            json={"filename": "ok.json", "selected_node_id": generated["root_id"]},
        )
        self.assertEqual(saved.status_code, 200, saved.text)
        document = json.loads((self.sessions / "ok.json").read_text())
        document["ui"]["selected_node_id"] = "nonexistent"
        (self.sessions / "bad-selected.json").write_text(json.dumps(document))
        bad_selected = self.client.post("/api/sessions/load", json={"filename": "bad-selected.json"})
        self.assertEqual(bad_selected.status_code, 400)
        ghost = json.loads((self.sessions / "ok.json").read_text())
        ghost["nodes"][0]["child_ids"] = ["child_missing"]
        (self.sessions / "bad-child.json").write_text(json.dumps(ghost))
        bad_child = self.client.post("/api/sessions/load", json={"filename": "bad-child.json"})
        self.assertEqual(bad_child.status_code, 400)
        self.assertNotEqual(bad_child.status_code, 500)

    def test_save_load_jobs_and_continue_without_model(self) -> None:
        generated = self.client.post("/api/sessions/generated", json={"generation_seed": "8", "floor": 1}).json()
        session_id = generated["session_id"]
        root_id = generated["root_id"]
        node = self.client.get(f"/api/sessions/{session_id}/nodes/{root_id}").json()
        end_turn = next(action for action in node["actions"] if action["descriptor"]["kind"] == "end_turn")
        child = self.client.post(
            f"/api/sessions/{session_id}/nodes/{root_id}/act",
            json={
                "native_index": end_turn["native_index"],
                "revision": node["revision"],
                "descriptor": end_turn["descriptor"],
            },
        ).json()
        job = self.client.post(
            f"/api/sessions/{session_id}/nodes/{child['child_id']}/continue",
            json={"mode": "greedy", "max_decisions": 6},
        ).json()
        finished = self.wait_job(job["id"])
        self.assertIn(finished["reason"], {"victory", "defeat", "limit"})
        save = self.client.post(
            f"/api/sessions/{session_id}/save",
            json={"filename": "with-jobs.json", "selected_node_id": finished["leaf_id"]},
        )
        self.assertEqual(save.status_code, 200, save.text)
        document = json.loads((self.sessions / "with-jobs.json").read_text())
        self.assertTrue(document["jobs"])
        self.assertEqual(document["jobs"][0]["reason"], finished["reason"])
        with tempfile.TemporaryDirectory() as directory:
            dest = Path(directory)
            (dest / "with-jobs.json").write_text(json.dumps(document))
            app = create_app(
                AppConfig(
                    validation_manifest=self.manifest,
                    distributions=self.distributions,
                    sessions_dir=dest,
                    checkpoint=None,
                    allowed_hosts=("testserver", "127.0.0.1", "localhost"),
                )
            )
            client = TestClient(app)
            restored = client.post("/api/sessions/load", json={"filename": "with-jobs.json"}).json()
            self.assertEqual(len(restored["nodes"]), len(document["nodes"]))
            self.assertTrue(restored["jobs"])
            self.assertIsNone(restored["model"])
            leaf = next(item for item in restored["nodes"] if item["id"] == document["ui"]["selected_node_id"])
            if not leaf["terminal"]:
                node = client.get(f"/api/sessions/{restored['session_id']}/nodes/{leaf['id']}").json()
                action = next(item for item in node["actions"] if item["allowed"])
                played = client.post(
                    f"/api/sessions/{restored['session_id']}/nodes/{leaf['id']}/act",
                    json={
                        "native_index": action["native_index"],
                        "revision": node["revision"],
                        "descriptor": action["descriptor"],
                    },
                )
                self.assertEqual(played.status_code, 200, played.text)
            denied = client.post(
                f"/api/sessions/{restored['session_id']}/nodes/{restored['root_id']}/model-step",
                json={"mode": "greedy"},
            )
            self.assertEqual(denied.status_code, 400)
            app.state.explorer.close()

    def test_malformed_session_types_are_structured_client_errors(self) -> None:
        generated = self.client.post("/api/sessions/generated", json={"generation_seed": "12", "floor": 1}).json()
        saved = self.client.post(
            f"/api/sessions/{generated['session_id']}/save",
            json={"filename": "typed.json", "selected_node_id": generated["root_id"]},
        )
        self.assertEqual(saved.status_code, 200, saved.text)
        base = json.loads((self.sessions / "typed.json").read_text())

        def load_variant(name: str, mutate) -> None:
            document = json.loads(json.dumps(base))
            mutate(document)
            (self.sessions / name).write_text(json.dumps(document))
            response = self.client.post("/api/sessions/load", json={"filename": name})
            self.assertEqual(response.status_code, 400, response.text)
            self.assertNotEqual(response.status_code, 500)
            body = response.json()
            self.assertEqual(body["code"], "invalid_session")
            self.assertIn("error", body)

        load_variant("list-selected.json", lambda d: d["ui"].__setitem__("selected_node_id", []))
        load_variant("int-selected.json", lambda d: d["ui"].__setitem__("selected_node_id", 7))
        load_variant("ui-list.json", lambda d: d.__setitem__("ui", ["selected"]))
        load_variant("pref-list.json", lambda d: d["ui"].__setitem__("preferred_children", {d["root_id"]: []}))
        load_variant("provenance-list.json", lambda d: d.__setitem__("provenance", ["bad"]))
        load_variant("spec-array.json", lambda d: d.__setitem__("spec_json", "[1, 2]"))
        load_variant("job-source-list.json", lambda d: d.__setitem__("jobs", [{
            "id": "job_x",
            "source_node_id": [],
            "status": "completed",
            "leaf_id": d["root_id"],
        }]))
        load_variant("job-leaf-list.json", lambda d: d.__setitem__("jobs", [{
            "id": "job_y",
            "source_node_id": d["root_id"],
            "status": "completed",
            "leaf_id": [],
        }]))
        nested: dict = {}
        cursor = nested
        for _ in range(80):
            cursor["x"] = {}
            cursor = cursor["x"]
        load_variant("too-deep.json", lambda d: d.__setitem__("provenance", nested))
        huge = self.sessions / "huge.json"
        huge.write_text(json.dumps({"format": FORMAT_NAME, "pad": "x" * 200}))
        import combat_explorer.persistence as persistence

        original_limit = persistence.MAX_SESSION_FILE_BYTES
        persistence.MAX_SESSION_FILE_BYTES = 64
        try:
            too_big = self.client.post("/api/sessions/load", json={"filename": "huge.json"})
        finally:
            persistence.MAX_SESSION_FILE_BYTES = original_limit
        self.assertEqual(too_big.status_code, 400, too_big.text)
        self.assertEqual(too_big.json()["code"], "invalid_session")
        self.assertIn("bytes", too_big.json()["error"])

    def test_multiple_loads_preserve_original_jobs_and_historical_probs_without_model(self) -> None:
        generated = self.client.post("/api/sessions/generated", json={"generation_seed": "13", "floor": 1}).json()
        session_id = generated["session_id"]
        root_id = generated["root_id"]
        stepped = self.client.post(
            f"/api/sessions/{session_id}/nodes/{root_id}/model-step",
            json={"mode": "sample", "temperature": 0.5, "sampling_seed": "99", "request_id": "hist-step"},
        )
        self.assertEqual(stepped.status_code, 200, stepped.text)
        parent = self.client.get(f"/api/sessions/{session_id}/nodes/{root_id}").json()
        historical = next(edge for edge in parent["outgoing"] if edge["child_id"] == stepped.json()["child_id"])
        self.assertTrue(historical["diagnostics"]["historical"])
        self.assertEqual(historical["diagnostics"]["mode"], "sample")
        self.assertEqual(historical["diagnostics"]["temperature"], 0.5)
        self.assertEqual(len(historical["diagnostics"]["native_indices"]), len(historical["diagnostics"]["base_probabilities"]))
        recorded = {
            native: prob
            for native, prob in zip(
                historical["diagnostics"]["native_indices"],
                historical["diagnostics"]["base_probabilities"],
            )
        }
        job = self.client.post(
            f"/api/sessions/{session_id}/nodes/{stepped.json()['child_id']}/continue",
            json={"mode": "greedy", "max_decisions": 4, "request_id": "hist-job"},
        ).json()
        finished = self.wait_job(job["id"])
        self.assertIn(finished["reason"], {"victory", "defeat", "limit"})
        original_job = self.client.get(f"/api/jobs/{job['id']}").json()
        save = self.client.post(
            f"/api/sessions/{session_id}/save",
            json={"filename": "hist-jobs.json", "selected_node_id": finished["leaf_id"]},
        )
        self.assertEqual(save.status_code, 200, save.text)
        document = json.loads((self.sessions / "hist-jobs.json").read_text())
        first = self.client.post("/api/sessions/load", json={"filename": "hist-jobs.json"}).json()
        second = self.client.post("/api/sessions/load", json={"filename": "hist-jobs.json"}).json()
        still_original = self.client.get(f"/api/jobs/{job['id']}").json()
        self.assertEqual(still_original["id"], original_job["id"])
        self.assertEqual(still_original["session_id"], session_id)
        self.assertEqual(still_original["reason"], original_job["reason"])
        self.assertNotEqual(first["session_id"], session_id)
        self.assertNotEqual(second["session_id"], first["session_id"])
        self.assertTrue(first["jobs"])
        self.assertTrue(second["jobs"])
        self.assertNotEqual(first["jobs"][0]["id"], job["id"])
        self.assertNotEqual(second["jobs"][0]["id"], job["id"])
        self.assertNotEqual(first["jobs"][0]["id"], second["jobs"][0]["id"])
        self.assertEqual(first["jobs"][0]["session_id"], first["session_id"])
        self.assertEqual(first["jobs"][0]["reason"], finished["reason"])
        with tempfile.TemporaryDirectory() as directory:
            dest = Path(directory)
            (dest / "hist-jobs.json").write_text(json.dumps(document))
            app = create_app(
                AppConfig(
                    validation_manifest=self.manifest,
                    distributions=self.distributions,
                    sessions_dir=dest,
                    checkpoint=None,
                    allowed_hosts=("testserver", "127.0.0.1", "localhost"),
                )
            )
            client = TestClient(app)
            restored = client.post("/api/sessions/load", json={"filename": "hist-jobs.json"}).json()
            self.assertIsNone(restored["model"])
            node = client.get(f"/api/sessions/{restored['session_id']}/nodes/{restored['root_id']}").json()
            self.assertIsNone(node["current_analysis"])
            edge = next(item for item in node["outgoing"] if item["native_index"] == historical["native_index"])
            self.assertEqual(edge["diagnostics"]["mode"], "sample")
            self.assertEqual(edge["diagnostics"]["temperature"], 0.5)
            loaded = {
                native: prob
                for native, prob in zip(
                    edge["diagnostics"]["native_indices"],
                    edge["diagnostics"]["base_probabilities"],
                )
            }
            self.assertEqual(loaded, recorded)
            app.state.explorer.close()


if __name__ == "__main__":
    unittest.main()
