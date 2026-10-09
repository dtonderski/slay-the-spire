"""Headed-workflow browser tests. Random-init checkpoints are labeled as fixtures.

Chromium is not installed by these tests. From `rl/`:

    uv run playwright install chromium

Optional environment inputs (never inferred from machine-specific paths):

- COMBAT_EXPLORER_CHECKPOINT: real checkpoint file; default is a temporary random-init fixture
- COMBAT_EXPLORER_DISTRIBUTIONS: real loadout fit.json; default is a temporary generated fixture
- COMBAT_EXPLORER_VALIDATION_MANIFEST: saved-roots JSON; default is a tiny generated fixture
- COMBAT_EXPLORER_SCREENSHOT_DIR: directory for optional PNG screenshots
"""

from __future__ import annotations

import json
import os
import socket
import tempfile
import threading
import time
import unittest
import urllib.request
from pathlib import Path

import torch
import uvicorn
from combat_explorer.server import AppConfig, create_app
from loadout_sampling import LoadoutSampler, band_for
from model import CombatValueModel
from scenarios import COMBAT_FLOORS
from validation_set import build_validation

OPTIONAL_CHECKPOINT = os.environ.get("COMBAT_EXPLORER_CHECKPOINT")
OPTIONAL_DISTRIBUTIONS = os.environ.get("COMBAT_EXPLORER_DISTRIBUTIONS")
OPTIONAL_VALIDATION_MANIFEST = os.environ.get("COMBAT_EXPLORER_VALIDATION_MANIFEST")
OPTIONAL_SCREENSHOT_DIR = os.environ.get("COMBAT_EXPLORER_SCREENSHOT_DIR")


def _route_already_settled(error: BaseException) -> bool:
    text = str(error).lower()
    return any(
        needle in text
        for needle in (
            "already handled",
            "route is already handled",
            "target closed",
            "has been closed",
            "page closed",
            "connection closed",
            "disposed",
            "browser has been closed",
        )
    )


class HeldRoutes:
    """Intercepted Playwright routes that must be continue/abort before unroute/close."""

    def __init__(self) -> None:
        self._routes: list = []

    def add(self, route) -> None:
        self._routes.append(route)

    def __bool__(self) -> bool:
        return bool(self._routes)

    def __len__(self) -> int:
        return len(self._routes)

    def settle(self, *, action: str = "abort") -> None:
        pending = list(self._routes)
        self._routes.clear()
        errors: list[BaseException] = []
        for route in pending:
            try:
                if action == "continue":
                    route.continue_()
                else:
                    route.abort("aborted")
            except Exception as error:
                if _route_already_settled(error):
                    continue
                errors.append(error)
        if errors:
            raise errors[0]


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
            "bands": {band_for(floor): band for floor in COMBAT_FLOORS},
        }
    )


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def wait_http(url: str, timeout: float = 8.0) -> None:
    deadline = time.time() + timeout
    last_error: Exception | None = None
    while time.time() < deadline:
        try:
            urllib.request.urlopen(url)
            return
        except Exception as error:
            last_error = error
            time.sleep(0.05)
    raise RuntimeError(f"server did not start: {last_error}")


class BrowserWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        try:
            from playwright.sync_api import sync_playwright
        except ImportError as error:
            raise unittest.SkipTest("playwright is not installed; run uv sync in rl/") from error
        cls.sync_playwright = staticmethod(sync_playwright)
        cls.tmpdir = tempfile.TemporaryDirectory()
        root = Path(cls.tmpdir.name)
        cls.sessions = root / "sessions"
        cls.sessions.mkdir()
        cls.fixture_checkpoint = root / "random-init.pt"
        torch.save({"model": CombatValueModel().state_dict(), "config": {"fixture": "random_init_not_trained"}}, cls.fixture_checkpoint)
        env_distributions = Path(OPTIONAL_DISTRIBUTIONS) if OPTIONAL_DISTRIBUTIONS else None
        if env_distributions is not None and env_distributions.is_file():
            cls.distributions = env_distributions
            cls.distributions_kind = "optional_env_distributions"
        else:
            cls.distributions = root / "fit.json"
            cls.distributions.write_text(json.dumps(sampler().distributions))
            cls.distributions_kind = "temporary_generated_fixture"
        env_manifest = Path(OPTIONAL_VALIDATION_MANIFEST) if OPTIONAL_VALIDATION_MANIFEST else None
        if env_manifest is not None and env_manifest.is_file():
            cls.manifest = env_manifest
            cls.manifest_kind = "optional_env_validation_manifest"
        else:
            cls.manifest = root / "validation.json"
            cls.manifest.write_text(json.dumps(build_validation(sampler(), 7, 1, 1, repeats=1)))
            cls.manifest_kind = "temporary_generated_fixture"
        env_checkpoint = Path(OPTIONAL_CHECKPOINT) if OPTIONAL_CHECKPOINT else None
        if env_checkpoint is not None and env_checkpoint.is_file():
            cls.checkpoint = env_checkpoint
            cls.checkpoint_kind = "optional_env_checkpoint"
        else:
            cls.checkpoint = cls.fixture_checkpoint
            cls.checkpoint_kind = "temporary_random_init_fixture"
        cls.screenshot_dir = Path(OPTIONAL_SCREENSHOT_DIR) if OPTIONAL_SCREENSHOT_DIR else None
        if cls.screenshot_dir is not None:
            cls.screenshot_dir.mkdir(parents=True, exist_ok=True)
        cls.port = free_port()
        extra_allowed = [root, cls.checkpoint, cls.distributions, cls.manifest]
        cls.app = create_app(
            AppConfig(
                validation_manifest=cls.manifest,
                distributions=cls.distributions,
                sessions_dir=cls.sessions,
                checkpoint=cls.checkpoint,
                host="127.0.0.1",
                port=cls.port,
                extra_allowed_files=extra_allowed,
            )
        )
        config = uvicorn.Config(cls.app, host="127.0.0.1", port=cls.port, log_level="warning")
        cls.server = uvicorn.Server(config)
        cls.thread = threading.Thread(target=cls.server.run, daemon=True)
        cls.thread.start()
        wait_http(f"http://127.0.0.1:{cls.port}/api/capabilities")
        cls._playwright = cls.sync_playwright().start()
        try:
            cls.browser = cls._playwright.chromium.launch(headless=True)
        except Exception as error:
            cls._playwright.stop()
            raise unittest.SkipTest(
                "Chromium is not installed for Playwright. "
                "From rl/: uv run playwright install chromium. "
                f"Launch error: {error}"
            ) from error

    @classmethod
    def tearDownClass(cls) -> None:
        browser = getattr(cls, "browser", None)
        if browser is not None:
            for context in list(browser.contexts):
                for page in list(context.pages):
                    try:
                        page.unroute_all(behavior="wait")
                    except Exception as error:
                        if not _route_already_settled(error):
                            raise
                    page.close()
            browser.close()
        playwright = getattr(cls, "_playwright", None)
        if playwright is not None:
            playwright.stop()
        server = getattr(cls, "server", None)
        thread = getattr(cls, "thread", None)
        if server is not None:
            server.should_exit = True
        if thread is not None:
            thread.join(timeout=10)
            if thread.is_alive() and server is not None:
                server.force_exit = True
                thread.join(timeout=5)
        app = getattr(cls, "app", None)
        if app is not None:
            app.state.explorer.close()
        tmpdir = getattr(cls, "tmpdir", None)
        if tmpdir is not None:
            tmpdir.cleanup()

    def setUp(self) -> None:
        self.page = self.browser.new_page()
        self.page.set_default_timeout(20000)
        self._held = HeldRoutes()

    def tearDown(self) -> None:
        self._held.settle(action="abort")
        try:
            self.page.unroute_all(behavior="wait")
        except Exception as error:
            if not _route_already_settled(error):
                raise
        self.page.close()

    def _shot(self, name: str) -> None:
        if self.screenshot_dir is None:
            return
        self.page.screenshot(path=str(self.screenshot_dir / name), full_page=True)

    def _explorer(self) -> dict:
        return self.page.evaluate(
            """() => ({
              selectedId: window.__explorer.selectedId,
              sessionId: window.__explorer.tree?.session_id,
              nodeId: window.__explorer.node?.id,
              jobId: window.__explorer.job?.id,
              jobStatus: window.__explorer.job?.status,
              jobSession: window.__explorer.job?.session_id,
              revision: window.__explorer.node?.revision,
              actions: (window.__explorer.node?.actions || []).map((item) => item.native_index),
              provenanceKind: window.__explorer.tree?.provenance?.kind,
            })"""
        )

    def _goto(self) -> None:
        self.page.goto(f"http://127.0.0.1:{self.port}/")
        self.page.wait_for_selector("#menuButton")
        self.page.wait_for_selector("#rootList option", state="attached")

    def _wait_new_session(self, old_id, timeout: float = 20000) -> None:
        self.page.wait_for_function(
            """oldId => window.__explorer.tree
                && window.__explorer.tree.session_id
                && window.__explorer.tree.session_id !== oldId
                && window.__explorer.node
                && window.__explorer.node.id === window.__explorer.selectedId
                && Array.isArray(window.__explorer.node.actions)""",
            arg=old_id,
            timeout=timeout,
        )

    def _open_start_menu(self) -> None:
        self.page.evaluate("() => document.body.classList.add('menu-open')")
        self.page.locator("#menuBackdrop").evaluate("el => { el.hidden = false }")
        self.page.locator("#startMenu").wait_for(state="visible")

    def _load_first_root(self) -> dict:
        old = self.page.evaluate("() => window.__explorer.tree?.session_id")
        self._open_start_menu()
        self.page.locator("#rootList option").first.click()
        self.page.locator("#rootForm button[type=submit]").click()
        self._wait_new_session(old)
        self.page.wait_for_function("() => window.__explorer?.node?.actions?.length > 0")
        return self._explorer()

    def _generate(self, seed: str = "42", floor: str = "1") -> dict:
        old = self.page.evaluate("() => window.__explorer.tree?.session_id")
        self._open_start_menu()
        self.page.fill("#genSeed", seed)
        self.page.fill("#genFloor", floor)
        self.page.locator("#generateForm button[type=submit]").click()
        self._wait_new_session(old)
        return self._explorer()

    def _set_temperature(self, value: str) -> None:
        self.page.evaluate(
            """value => {
                const el = document.querySelector('#temperature');
                el.value = value;
                el.dispatchEvent(new Event('change', { bubbles: true }));
            }""",
            value,
        )

    def _wait_held(self, bucket: HeldRoutes | list, timeout: float = 5.0) -> None:
        deadline = time.time() + timeout
        while time.time() < deadline and not bucket:
            self.page.wait_for_timeout(50)
        self.assertTrue(bucket, "expected an intercepted request")

    def test_root_continue_rewind_branch_save_load_and_restored_jobs(self) -> None:
        page = self.page
        self._goto()
        self.assertTrue(page.locator("#maxDecisions").is_visible())
        first = self._load_first_root()
        self._shot("01-loaded-root.png")
        self.assertEqual(first["selectedId"], first["nodeId"])
        page.fill("#maxDecisions", "24")
        source = first["selectedId"]
        previous_job = page.evaluate("() => window.__explorer.job && window.__explorer.job.id")
        page.locator("#modelFinish").click()
        page.wait_for_function(
            "id => window.__explorer.job && window.__explorer.job.id !== id && window.__explorer.job.status !== 'running'",
            arg=previous_job,
            timeout=60000,
        )
        job_status = page.evaluate("() => window.__explorer.job")
        self.assertIn(job_status["reason"], ["victory", "defeat", "limit"])
        self.assertIsNone(job_status["error"])
        after_job = self._explorer()
        self.assertEqual(after_job["selectedId"], source)
        self._shot("02-job-complete-selection-stable.png")
        page.locator("#jumpResult").click()
        page.wait_for_function(
            "source => window.__explorer.node && window.__explorer.selectedId === window.__explorer.node.id && window.__explorer.node.id !== source",
            arg=source,
        )
        self._shot("03-jumped-to-result.png")
        page.locator("#prev").click()
        page.wait_for_function("() => window.__explorer.node && window.__explorer.node.id === window.__explorer.selectedId")
        page.locator("#tree button.node").first.click()
        page.wait_for_function("() => window.__explorer.node && window.__explorer.node.id === window.__explorer.tree.root_id")
        page.wait_for_selector("#actionList button")
        # Reusing the same parent/action edge must not create a second branch.
        # Select an action whose native index is not already a root child.
        alternate = page.evaluate("""() => {
          const existing = new Set((window.__explorer.node.outgoing || []).map(edge => edge.native_index));
          return [...document.querySelectorAll('#actionList .action')]
            .map(row => Number(row.dataset.nativeIndex))
            .find(index => !existing.has(index));
        }""")
        self.assertIsNotNone(alternate, "Fixture needs a distinct legal root action")
        page.locator(f'#actionList .action[data-native-index="{alternate}"] button').click()
        page.wait_for_function("() => window.__explorer.node && window.__explorer.node.parent_id === window.__explorer.tree.root_id")
        page.fill("#maxDecisions", "16")
        page.wait_for_function("() => !document.querySelector('#modelFinish').disabled")
        previous_job = page.evaluate("() => window.__explorer.job && window.__explorer.job.id")
        page.locator("#modelFinish").click()
        page.wait_for_function(
            "id => window.__explorer.job && window.__explorer.job.id !== id && window.__explorer.job.status !== 'running'",
            arg=previous_job,
            timeout=60000,
        )
        tree_nodes = page.evaluate("() => window.__explorer.tree.nodes")
        root_id = page.evaluate("() => window.__explorer.tree.root_id")
        root = next(node for node in tree_nodes if node["id"] == root_id)
        self.assertGreaterEqual(len(root["child_ids"]), 2)
        self._shot("04-sibling-branches.png")
        saved_selected = page.evaluate("() => window.__explorer.selectedId")
        page.once("dialog", lambda dialog: dialog.accept("browser.json"))
        page.locator("#save").click()
        page.wait_for_function("() => true")
        deadline = time.time() + 5
        while time.time() < deadline and not (self.sessions / "browser.json").is_file():
            page.wait_for_timeout(50)
        self.assertTrue((self.sessions / "browser.json").is_file())
        old = page.evaluate("() => window.__explorer.tree.session_id")
        page.once("dialog", lambda dialog: dialog.accept("browser.json"))
        page.locator("#load").click()
        self._wait_new_session(old)
        loaded = page.evaluate(
            """() => ({
              nodes: window.__explorer.tree.nodes.length,
              selected: window.__explorer.selectedId,
              nodeId: window.__explorer.node?.id,
              jobs: window.__explorer.tree.jobs || [],
            })"""
        )
        self.assertGreaterEqual(loaded["nodes"], 3)
        self.assertEqual(loaded["selected"], loaded["nodeId"])
        self.assertTrue(loaded["jobs"])
        self.assertIn(loaded["jobs"][0]["reason"], ["victory", "defeat", "limit", "cancelled", "interrupted"])
        self.assertNotEqual(loaded["jobs"][0]["id"], job_status["id"])
        self.assertEqual(page.evaluate("() => window.__explorer.node.id === window.__explorer.selectedId"), True)
        self.assertEqual(saved_selected is not None, True)
        self._shot("05-after-load.png")
        generated = self._generate("42", "1")
        self.assertEqual(generated["provenanceKind"], "generated")
        self._shot("07-generated-root.png")

    def test_analyze_response_inversion_uses_live_mode_and_temperature(self) -> None:
        page = self.page
        self._goto()
        self._load_first_root()
        pending: dict[float, tuple] = {}
        held = HeldRoutes()

        def handle(route) -> None:
            if route.request.method != "POST" or "/analyze" not in route.request.url:
                route.continue_()
                return
            held.add(route)
            self._held.add(route)
            body = route.request.post_data_json or {}
            response = route.fetch()
            temp = float(body.get("temperature", 1))
            pending[temp] = (route, response)
            cool = next((item for value, item in pending.items() if abs(value - 0.25) < 1e-9), None)
            hot = next((item for value, item in pending.items() if abs(value - 0.5) < 1e-9), None)
            if cool and hot:
                hot[0].fulfill(response=hot[1])
                cool[0].fulfill(response=cool[1])

        page.route("**/api/sessions/**/analyze", handle)
        try:
            self._set_temperature("0.25")
            self._set_temperature("0.5")
            page.wait_for_function(
                """() => {
                    const el = document.querySelector('#analysisNote [data-kind=current]');
                    return el && el.dataset.mode === 'sample' && el.dataset.temperature === '0.5';
                }""",
                timeout=15000,
            )
            note = page.locator("#analysisNote").inner_text()
            self.assertIn("T=0.5", note)
            self.assertNotIn("T=0.25", note)
            self.assertNotIn("pending", note.lower())
            self._shot("06-temperature-reanalysis.png")
        finally:
            held.settle(action="abort")
            page.unroute("**/api/sessions/**/analyze")

    def test_session_switch_during_pending_tree_model_and_cancel(self) -> None:
        page = self.page
        self._goto()
        first = self._load_first_root()
        session_a = first["sessionId"]

        held_tree = HeldRoutes()

        def hold_tree(route) -> None:
            if route.request.method == "GET" and route.request.url.endswith("/tree") and session_a in route.request.url:
                held_tree.add(route)
                self._held.add(route)
                return
            route.continue_()

        page.route("**/api/sessions/**/tree", hold_tree)
        try:
            page.wait_for_selector("#actionList button")
            page.locator("#actionList button").first.click()
            self._wait_held(held_tree)
            generated = self._generate("7", "1")
            self.assertNotEqual(generated["sessionId"], session_a)
            held_tree.settle(action="continue")
            page.wait_for_timeout(400)
            after_tree = self._explorer()
            self.assertEqual(after_tree["sessionId"], generated["sessionId"])
            self.assertEqual(after_tree["selectedId"], after_tree["nodeId"])
            self.assertEqual(after_tree["jobSession"] in (None, generated["sessionId"]), True)
        finally:
            held_tree.settle(action="abort")
            page.unroute("**/api/sessions/**/tree")

        session_b = after_tree["sessionId"]
        held_model = HeldRoutes()

        def hold_model_tree(route) -> None:
            if route.request.method == "GET" and route.request.url.endswith("/tree") and session_b in route.request.url:
                held_model.add(route)
                self._held.add(route)
                return
            route.continue_()

        page.route("**/api/sessions/**/tree", hold_model_tree)
        try:
            self._open_start_menu()
            page.fill("#modelPath", str(self.checkpoint))
            page.locator("#modelForm button[type=submit]").click()
            self._wait_held(held_model)
            generated2 = self._generate("8", "1")
            self.assertNotEqual(generated2["sessionId"], session_b)
            held_model.settle(action="continue")
            page.wait_for_timeout(400)
            after_model = self._explorer()
            self.assertEqual(after_model["sessionId"], generated2["sessionId"])
            self.assertEqual(after_model["selectedId"], after_model["nodeId"])
        finally:
            held_model.settle(action="abort")
            page.unroute("**/api/sessions/**/tree")

        held_jobs = HeldRoutes()
        held_cancel = HeldRoutes()

        def hold_jobs(route) -> None:
            url = route.request.url
            if route.request.method == "POST" and "/cancel" in url:
                held_cancel.add(route)
                self._held.add(route)
                return
            if route.request.method == "GET" and "/api/jobs/" in url:
                held_jobs.add(route)
                self._held.add(route)
                return
            route.continue_()

        page.route("**/api/jobs/**", hold_jobs)
        try:
            page.fill("#maxDecisions", "64")
            previous_job = page.evaluate("() => window.__explorer.job && window.__explorer.job.id")
            page.locator("#modelFinish").click()
            page.wait_for_function("id => window.__explorer.job && window.__explorer.job.id !== id && window.__explorer.job.status === 'running'", arg=previous_job)
            session_c = page.evaluate("() => window.__explorer.tree.session_id")
            job_c = page.evaluate("() => window.__explorer.job.id")
            page.locator("#cancelJob").click()
            self._wait_held(held_cancel)
            generated3 = self._generate("9", "1")
            self.assertNotEqual(generated3["sessionId"], session_c)
            held_cancel.settle(action="continue")
            page.wait_for_timeout(400)
            after_cancel = self._explorer()
            self.assertEqual(after_cancel["sessionId"], generated3["sessionId"])
            self.assertNotEqual(after_cancel["jobId"], job_c)
            self.assertEqual(after_cancel["selectedId"], after_cancel["nodeId"])
        finally:
            held_jobs.settle(action="continue")
            held_cancel.settle(action="abort")
            page.unroute("**/api/jobs/**")

        held_poll = HeldRoutes()

        def fail_poll(route) -> None:
            if route.request.method == "GET" and "/api/jobs/" in route.request.url:
                held_poll.add(route)
                self._held.add(route)
                route.fulfill(
                    status=500,
                    content_type="application/json",
                    body=json.dumps({"error": "poll failed", "code": "boom"}),
                )
                return
            route.continue_()

        page.route("**/api/jobs/**", fail_poll)
        try:
            page.fill("#maxDecisions", "8")
            page.locator("#modelFinish").click()
            page.wait_for_function("() => document.querySelector('#error') && document.querySelector('#error').textContent.includes('poll failed')")
            polled = self._explorer()
            self.assertEqual(polled["sessionId"], generated3["sessionId"])
            self.assertEqual(polled["selectedId"], polled["nodeId"])
        finally:
            held_poll.settle(action="abort")
            page.unroute("**/api/jobs/**")

    def test_navigation_inverts_stale_root_without_waiting_for_it(self) -> None:
        page = self.page
        self._goto()
        first = self._load_first_root()
        page.fill("#maxDecisions", "12")
        previous_job = page.evaluate("() => window.__explorer.job && window.__explorer.job.id")
        page.locator("#modelFinish").click()
        page.wait_for_function(
            "id => window.__explorer.job && window.__explorer.job.id !== id && window.__explorer.job.status !== 'running'",
            arg=previous_job,
            timeout=60000,
        )
        page.locator("#jumpResult").click()
        page.wait_for_function(
            "root => window.__explorer.node && window.__explorer.selectedId === window.__explorer.node.id && window.__explorer.node.id !== root",
            arg=first["selectedId"],
        )
        root_id = page.evaluate("() => window.__explorer.tree.root_id")
        held = HeldRoutes()

        def hold_root(route) -> None:
            if route.request.method == "GET" and f"/nodes/{root_id}" in route.request.url:
                held.add(route)
                self._held.add(route)
                return
            route.continue_()

        page.route("**/api/sessions/**/nodes/**", hold_root)
        try:
            page.locator("#tree button.node").first.click()
            for _ in range(6):
                page.locator("#next").click()
            self._wait_held(held)
            selected_before_release = page.evaluate("() => window.__explorer.selectedId")
            self.assertNotEqual(selected_before_release, root_id)
            held.settle(action="continue")
            page.wait_for_function(
                """() => window.__explorer.node
                    && window.__explorer.node.id === window.__explorer.selectedId
                    && document.querySelector('#board .boardGrid')
                    && document.querySelector('#pathMeta').textContent.includes(String(window.__explorer.node.revision))"""
            )
            raced = self._explorer()
            self.assertEqual(raced["selectedId"], raced["nodeId"])
            self.assertNotEqual(raced["nodeId"], root_id)
            self.assertTrue(raced["actions"] is not None)
        finally:
            held.settle(action="abort")
            page.unroute("**/api/sessions/**/nodes/**")

    def test_historical_probabilities_are_numeric_and_survive_reanalysis(self) -> None:
        page = self.page
        self._goto()
        self._load_first_root()
        page.locator("#modelStep").click()
        page.wait_for_function("() => window.__explorer.node && window.__explorer.node.parent_id === window.__explorer.tree.root_id")
        page.locator("#prev").click()
        page.wait_for_function("() => window.__explorer.node && window.__explorer.node.id === window.__explorer.tree.root_id")
        page.wait_for_function(
            """() => {
                const row = document.querySelector('#actionList .action.historical');
                return row && row.dataset.histBase;
            }"""
        )
        hist_row = page.locator("#actionList .action.historical").first
        hist_base = hist_row.get_attribute("data-hist-base")
        hist_adj = hist_row.get_attribute("data-hist-adj")
        native = hist_row.get_attribute("data-native-index")
        self.assertRegex(hist_base or "", r"^\d+\.\d{3}$")
        note = page.locator("#analysisNote [data-kind=historical-probabilities]")
        self.assertGreater(note.count(), 0)
        self.assertIn("hist p=", note.inner_text())
        self._set_temperature("0.25")
        page.wait_for_function(
            """() => {
                const el = document.querySelector('#analysisNote [data-kind=current]');
                return el && el.dataset.temperature === '0.25';
            }"""
        )
        hist_row = page.locator(f"#actionList .action[data-native-index='{native}']")
        self.assertEqual(hist_row.get_attribute("data-hist-base"), hist_base)
        self.assertEqual(hist_row.get_attribute("data-hist-adj"), hist_adj)
        self.assertRegex(hist_row.get_attribute("data-current-base") or "", r"^\d+\.\d{3}$")
        self.assertRegex(hist_row.get_attribute("data-current-adj") or "", r"^\d+\.\d{3}$")
        current_note = page.locator("#analysisNote [data-kind=current]").inner_text()
        self.assertIn("reanalysis", current_note.lower())
        self.assertIn("T=0.25", current_note)
        page.once("dialog", lambda dialog: dialog.accept("hist.json"))
        page.locator("#save").click()
        deadline = time.time() + 5
        while time.time() < deadline and not (self.sessions / "hist.json").is_file():
            page.wait_for_timeout(50)
        old = page.evaluate("() => window.__explorer.tree.session_id")
        page.once("dialog", lambda dialog: dialog.accept("hist.json"))
        page.locator("#load").click()
        self._wait_new_session(old)
        page.locator("#tree button.node").first.click()
        page.wait_for_function("() => window.__explorer.node && window.__explorer.node.id === window.__explorer.tree.root_id")
        page.wait_for_function(
            """() => {
                const row = document.querySelector('#actionList .action.historical');
                return row && row.dataset.histBase;
            }"""
        )
        restored = page.locator("#actionList .action.historical").first
        self.assertEqual(restored.get_attribute("data-hist-base"), hist_base)
        self.assertEqual(restored.get_attribute("data-native-index"), native)


if __name__ == "__main__":
    unittest.main()
