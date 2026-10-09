"""Execution equivalence and failure contracts; not real-game parity evidence."""

import copy
import hashlib
import json
import math
import tempfile
import unittest
from dataclasses import fields, is_dataclass, replace
from pathlib import Path
from typing import Any, cast
from unittest.mock import Mock, patch

import torch
from model import CombatValueModel
from run_training.collector import (
    CollectionFailure,
    CollectionJob,
    CombatRequest,
    FrozenCombat,
    MacroStep,
    RunEpisode,
    collect,
    collect_many,
    sample_many,
)
from run_training.model import (
    BUCKETS,
    HealthMacroModel,
    MacroInput,
    MacroModel,
    _map_terms,
    features,
)
from run_training.trainer import update
from sts_sim import State
from sts_sim.observations.screens import MapNode, MapScreen


def public_inputs():
    return [
        MacroInput(
            tuple((i, (i % 7 - 3) / 9) for i in range(40 + j * 53)),
            tuple(
                tuple((800 + k * 11 + i, (i + 1) / 13) for i in range(2 + k))
                for k in range(j + 2)
            ),
            (0.1 + j / 5, 0.08 + j / 7, 0.8),
        )
        for j in range(4)
    ]


def learned_model(kind, device):
    torch.manual_seed(48)
    model = kind(16).to(device)
    with torch.no_grad():
        for name, parameter in model.named_parameters():
            if name.endswith(
                ("policy.2.weight", "value.weight", "health_value.2.weight")
            ):
                parameter.normal_(0, 0.03)
    return model


def combat_state(seed):
    return State.from_synthetic_spec(
        json.dumps(
            {
                "seed": seed,
                "floor": 1,
                "kind": "normal",
                "encounter": "Cultist",
                "deck": [
                    {"key": "Strike_R", "upgrades": 0},
                    {"key": "Defend_R", "upgrades": 0},
                ],
                "relics": [],
                "potions": ["smoke_bomb", None, None],
                "hp": 80,
                "max_hp": 80,
                "gold": 99,
            }
        )
    )


class MacroBatchTests(unittest.TestCase):
    def setUp(self):
        torch.set_num_threads(1)

    def compare(self, device):
        inputs = public_inputs()
        for kind in (MacroModel, HealthMacroModel):
            model = learned_model(kind, device)
            packed = model.pack(inputs)
            logits, values = model.forward_batch(packed)
            for i, row in enumerate(inputs):
                reference, value = model(row)
                torch.testing.assert_close(
                    logits[i, : len(row.candidates)], reference, rtol=2e-5, atol=2e-6
                )
                torch.testing.assert_close(values[i], value, rtol=2e-5, atol=2e-6)
                self.assertTrue(torch.isneginf(logits[i, len(row.candidates) :]).all())
            # Batching/permutation cannot leak other episodes into a prediction.
            reverse, reversed_values = model.forward_batch(model.pack(inputs[::-1]))
            torch.testing.assert_close(reverse.flip(0), logits, rtol=2e-5, atol=2e-6)
            torch.testing.assert_close(
                reversed_values.flip(0), values, rtol=2e-5, atol=2e-6
            )
            self.assertFalse(packed.ids.requires_grad)

    def test_cached_public_map_features_equal_uncached_reference(self):
        def reference(value):
            result = {}

            def add(key, weight=1.0):
                index = (
                    int.from_bytes(
                        hashlib.blake2b(key.encode(), digest_size=8).digest(), "little"
                    )
                    % BUCKETS
                )
                result[index] = result.get(index, 0.0) + weight

            def visit(item, path):
                if item is None or isinstance(item, (str, bool)):
                    add(f"{path}={item!r}")
                elif isinstance(item, (int, float)):
                    add(path + ":present")
                    add(path + ":number", math.copysign(math.log1p(abs(item)), item))
                elif is_dataclass(item) and not isinstance(item, type):
                    for f in fields(item):
                        if f.name != "schema_version":
                            visit(getattr(item, f.name), f"{path}.{f.name}")
                elif isinstance(item, tuple):
                    add(path + ":length", math.log1p(len(item)))
                    for i, child in enumerate(item):
                        visit(child, f"{path}[{i}]")
                else:
                    raise AssertionError("Unsupported test fixture")

            visit(value, "public")
            return tuple(sorted(result.items()))

        screen = MapScreen(
            act=1,
            floor=7,
            current_node=13,
            reachable_nodes=(14, 15),
            nodes=tuple(
                MapNode(
                    slot=i,
                    act=1,
                    room_kind="rest",
                    burning_elite=False,
                    children=(i + 1, i + 2),
                )
                for i in range(70)
            ),
        )
        _map_terms.cache_clear()
        rng = torch.get_rng_state()
        for hp in (1.0, 10.0, 80.0):
            value = (tuple(range(100)), hp, screen, screen)
            self.assertEqual(features(value), reference(value))
        self.assertGreater(_map_terms.cache_info().hits, 0)
        self.assertEqual(_map_terms.cache_info().maxsize, 256)
        self.assertTrue(torch.equal(rng, torch.get_rng_state()))
        with self.assertRaises(ValueError):
            features((float("nan"), screen))

    def test_cpu_predictions_and_padding(self):
        self.compare("cpu")

    @unittest.skipUnless(torch.cuda.is_available(), "CUDA unavailable")
    def test_cuda_predictions_and_padding(self):
        self.compare("cuda")

    def test_empty_bags_and_candidates(self):
        model = MacroModel(8)
        row = MacroInput((), ((), ((2, 1.0),)))
        logits, values = model.forward_batch(model.pack([row]))
        ref, value = model(row)
        torch.testing.assert_close(logits[0], ref)
        torch.testing.assert_close(values[0], value)
        with self.assertRaises(ValueError):
            model.pack([MacroInput((), ())])

    def test_loss_gradients_update_and_chunk_normalization(self):
        for kind in (MacroModel, HealthMacroModel):
            model = learned_model(kind, "cpu")
            rows = public_inputs()
            steps = []
            with torch.no_grad():
                for i, row in enumerate(rows):
                    logits, value = model(row)
                    steps.append(
                        MacroStep(
                            row,
                            i % len(row.candidates),
                            tuple(logits.tolist()),
                            float(value),
                        )
                    )
            # Unequal episode lengths and a no-macro episode exercise both denominators.
            episodes = [
                RunEpisode("death", 0.0, (steps[0],), 1, 16, objective="act1_binary"),
                RunEpisode(
                    "act1_clear", 1.0, tuple(steps[1:]), 3, 18, objective="act1_binary"
                ),
                RunEpisode("death", 0.0, (), 1, 16, objective="act1_binary"),
            ]
            reference = copy.deepcopy(model)
            expected = update(
                episodes,
                reference,
                torch.optim.SGD(reference.parameters(), lr=0.01),
                entropy_coef=0.013,
                value_coef=0.17,
                batch_steps=0,
            )
            for size in (1, 2, 99):
                actual = copy.deepcopy(model)
                result = update(
                    episodes,
                    actual,
                    torch.optim.SGD(actual.parameters(), lr=0.01),
                    entropy_coef=0.013,
                    value_coef=0.17,
                    batch_steps=size,
                )
                self.assertAlmostEqual(result["loss"], expected["loss"], places=6)
                for (name, param), (other, ref) in zip(
                    actual.named_parameters(), reference.named_parameters(), strict=True
                ):
                    self.assertEqual(name, other)
                    torch.testing.assert_close(param, ref, atol=2e-7, rtol=2e-5)
                    torch.testing.assert_close(
                        param.grad, ref.grad, atol=2e-7, rtol=2e-5
                    )
                # A mismatch in the last chunk must prevent *all* backward/updates.
                broken = copy.deepcopy(model)
                optimizer = torch.optim.Adam(broken.parameters())
                bad = replace(steps[-1], value=100.0)
                corrupt = [
                    episodes[0],
                    replace(episodes[1], steps=(steps[1], steps[2], bad)),
                ]
                with self.assertRaisesRegex(ValueError, "recomputation"):
                    update(
                        corrupt,
                        broken,
                        optimizer,
                        entropy_coef=0.01,
                        value_coef=0.1,
                        batch_steps=size,
                    )
                self.assertFalse(optimizer.state)
                self.assertTrue(all(p.grad is None for p in broken.parameters()))
                with self.assertRaises(CollectionFailure):
                    update(
                        [replace(episodes[0], reward=None, status="cutoff")],
                        broken,
                        optimizer,
                        entropy_coef=0.01,
                        value_coef=0.1,
                        batch_steps=size,
                    )

    def test_candidate_record_boundaries_are_validated_before_backward(self):
        model = MacroModel(8)
        steps = []
        with torch.no_grad():
            for row in public_inputs()[:2]:
                logits, value = model(row)
                steps.append(MacroStep(row, 0, tuple(logits.tolist()), float(value)))
        malformed = [
            # Equal total flat length must not hide different per-row boundaries.
            [
                replace(steps[0], logits=steps[0].logits + (0.0,)),
                replace(steps[1], logits=steps[1].logits[:-1]),
            ],
            [steps[0], replace(steps[1], choice=len(steps[1].inputs.candidates))],
        ]
        for records in malformed:
            optimizer = torch.optim.Adam(model.parameters())
            episode = RunEpisode(
                "death", 0.0, tuple(records), 2, 16, objective="act1_binary"
            )
            with self.assertRaisesRegex(ValueError, "candidate record"):
                update(
                    [episode],
                    model,
                    optimizer,
                    entropy_coef=0.01,
                    value_coef=0.1,
                    batch_steps=1,
                )
            self.assertFalse(optimizer.state)
            self.assertTrue(all(p.grad is None for p in model.parameters()))

    def test_sampling_owns_independent_rng_and_uses_true_lengths(self):
        for device in ["cpu"] + (["cuda"] if torch.cuda.is_available() else []):
            logits = [
                torch.tensor([0.1 * j for j in range(n)], device=device)
                for n in (3, 19, 2)
            ]
            a = [torch.Generator(device=device).manual_seed(i) for i in range(3)]
            b = [torch.Generator(device=device).manual_seed(i) for i in range(3)]
            wanted = [
                int(torch.multinomial(row.softmax(0), 1, generator=rng))
                for row, rng in zip(logits, a, strict=True)
            ]
            self.assertEqual(sample_many(logits, b), wanted)
            for x, y in zip(a, b, strict=True):
                self.assertTrue(torch.equal(x.get_state(), y.get_state()))


class CooperativeCollectorTests(unittest.TestCase):
    def setUp(self):
        torch.set_num_threads(1)
        state = State.new("1", final_act=True)
        self.initial = state.step(state.decision().actions[0])
        self.terminal = replace(
            self.initial,
            observation=replace(
                self.initial.observation,
                context=replace(self.initial.observation.context, act=2, floor=18),
            ),
        )
        self.model = learned_model(MacroModel, "cpu")
        self.combat: Any = Mock(spec=FrozenCombat)
        self.combat.choose_many.side_effect = AssertionError("No combat expected")

    def jobs(self, path, specifications, stop=None):
        initial, terminal = self.initial, self.terminal
        states = []
        jobs = []
        for i, (length, error) in enumerate(specifications):

            class ScriptedState:
                def __init__(self, length, error):
                    self.length, self.error, self.calls = length, error, 0

                def decision(self):
                    return initial

                def step(self, action):
                    self.calls += 1
                    if self.error:
                        raise ValueError("injected unsupported action")
                    return terminal if self.calls >= self.length else initial

            state = ScriptedState(length, error)
            states.append(state)
            jobs.append(
                CollectionJob(
                    str(i),
                    {
                        "objective": "act1_binary",
                        "final_act": True,
                        "max_actions": 4,
                        "macro_rng": torch.Generator().manual_seed(400 + i),
                        "combat_rng": torch.Generator().manual_seed(500 + i),
                        "journal": path / f"{i}.jsonl",
                        "stop_requested": stop,
                        "state_factory": lambda *args, state=state, **kwargs: cast(
                            Any, state
                        ),
                    },
                )
            )
        return jobs, states

    def test_serial_batched_actions_rng_coverage_and_input_order(self):
        spec = [(3, False), (1, False), (2, False), (99, False), (1, False)]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            serial, _ = self.jobs(path / "serial", spec)
            batched, _ = self.jobs(path / "batch", spec)
            expected = [
                collect(j.seed, self.model, self.combat, **j.options) for j in serial
            ]
            actual = collect_many(batched, self.model, self.combat, width=2)
            self.assertEqual([e.status for e in actual], [e.status for e in expected])
            self.assertEqual([e.accepted for e in actual], [3, 1, 2, 4, 1])
            for i, (a, b) in enumerate(zip(actual, expected, strict=True)):
                self.assertEqual(a.reward, b.reward)
                self.assertEqual(
                    [s.choice for s in a.steps], [s.choice for s in b.steps]
                )
                self.assertEqual(
                    (path / "serial" / f"{i}.jsonl").read_text(),
                    (path / "batch" / f"{i}.jsonl").read_text(),
                )
                self.assertTrue(
                    torch.equal(
                        serial[i].options["macro_rng"].get_state(),
                        batched[i].options["macro_rng"].get_state(),
                    )
                )

    def test_failure_is_not_retried_or_a_death_and_cannot_train(self):
        with tempfile.TemporaryDirectory() as directory:
            jobs, states = self.jobs(
                Path(directory), [(1, True), (2, False), (1, False)]
            )
            observed = []
            episodes = collect_many(
                jobs,
                self.model,
                self.combat,
                width=2,
                continue_on_failure=True,
                on_failure=lambda job, error: observed.append(job.seed),
            )
            self.assertEqual(observed, ["0"])
            self.assertEqual(states[0].calls, 1)
            self.assertEqual(
                [e.status for e in episodes], ["error", "act1_clear", "act1_clear"]
            )
            with self.assertRaises(CollectionFailure):
                update(
                    episodes,
                    self.model,
                    torch.optim.Adam(self.model.parameters()),
                    entropy_coef=0.01,
                    value_coef=0.1,
                    batch_steps=8,
                )
            self.assertEqual(
                json.loads(jobs[0].options["journal"].read_text().splitlines()[-1])[
                    "type"
                ],
                "error",
            )

    def test_fatal_model_error_closes_every_open_journal(self):
        with tempfile.TemporaryDirectory() as directory:
            jobs, states = self.jobs(Path(directory), [(2, False)] * 5)
            with (
                patch.object(
                    self.model, "forward_batch", side_effect=ValueError("bad model")
                ),
                self.assertRaisesRegex(ValueError, "bad model"),
            ):
                collect_many(
                    jobs, self.model, self.combat, width=3, continue_on_failure=True
                )
            self.assertEqual([s.calls for s in states], [0] * 5)
            for job in jobs[:3]:
                self.assertEqual(
                    json.loads(job.options["journal"].read_text().splitlines()[-1])[
                        "type"
                    ],
                    "error",
                )
            self.assertFalse(jobs[3].options["journal"].exists())

    def test_stop_does_not_launch_jobs(self):
        with tempfile.TemporaryDirectory() as directory:
            jobs, _ = self.jobs(Path(directory), [(1, False)] * 4)
            self.assertEqual(
                collect_many(
                    jobs, self.model, self.combat, width=2, stop_requested=lambda: True
                ),
                [],
            )
            self.assertFalse(any(Path(directory).iterdir()))

    def test_stop_during_wave_keeps_outcomes_and_quarantines_partial_batch(self):
        with tempfile.TemporaryDirectory() as directory:
            jobs, states = self.jobs(Path(directory), [(1, False)] + [(10, False)] * 4)
            episodes = collect_many(
                jobs,
                self.model,
                self.combat,
                width=3,
                stop_requested=lambda: sum(s.calls for s in states) >= 2,
            )
            self.assertEqual(
                [e.status for e in episodes], ["act1_clear", "cutoff", "cutoff"]
            )
            self.assertEqual([e.reward for e in episodes], [1.0, None, None])
            self.assertEqual([s.calls for s in states[3:]], [0, 0])
            self.assertFalse(jobs[3].options["journal"].exists())
            for job in jobs[:3]:
                self.assertEqual(
                    json.loads(job.options["journal"].read_text().splitlines()[-1])[
                        "type"
                    ],
                    "result",
                )
            with self.assertRaises(CollectionFailure):
                update(
                    episodes,
                    self.model,
                    torch.optim.Adam(self.model.parameters()),
                    entropy_coef=0.01,
                    value_coef=0.1,
                    batch_steps=8,
                )

    def test_real_combat_batch_covers_smoke_and_matches_single_state(self):
        torch.manual_seed(91)
        frozen: Any = FrozenCombat.__new__(FrozenCombat)
        frozen.model = CombatValueModel(
            d_model=16, action_dim=16, n_layers=1, precision="fp32"
        ).eval()
        states = [combat_state(1), combat_state(2)]
        a = [torch.Generator().manual_seed(10 + i) for i in range(2)]
        b = [torch.Generator().manual_seed(10 + i) for i in range(2)]
        expected = [
            frozen.choose(s, s.decision(), rng)
            for s, rng in zip(states, a, strict=True)
        ]
        requests = [
            CombatRequest(s, s.decision(), rng)
            for s, rng in zip(states, b, strict=True)
        ]
        with patch.object(
            frozen.model, "forward", wraps=frozen.model.forward
        ) as forward:
            self.assertEqual(frozen.choose_many(requests), expected)
            candidates = forward.call_args.args[1]
            self.assertEqual(
                len(candidates), sum(len(s.decision().actions) for s in states)
            )
        for s in states:
            self.assertTrue(
                any(
                    x.kind == "use_potion_slot" and x.potion_slot == 0
                    for x in s.decision().actions
                )
            )
        for x, y in zip(a, b, strict=True):
            self.assertTrue(torch.equal(x.get_state(), y.get_state()))
        with patch.object(
            State, "numeric_decisions", side_effect=ValueError("bad export")
        ) as export:
            with self.assertRaisesRegex(RuntimeError, "unlocalized"):
                frozen.choose_many(requests)
            export.assert_called_once()


if __name__ == "__main__":
    unittest.main()
