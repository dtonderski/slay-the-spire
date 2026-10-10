"""Synthetic campfire initial states reconstructed from immutable action journals.

No observation hydration: source states advance from natural seeds and accepted
commands. HP is configured only on a new independent synthetic episode via the
explicit simulator constructor. Seeds/provenance never enter policy features.
"""

import gzip
import hashlib
import json
import math
from dataclasses import asdict
from pathlib import Path

from sts_sim import Observation, State, _native

from run_training.contracts import PolicyAction
from run_training.environment import journal_environment_seed

ROOT_PROTOCOL = "natural_prefix_a0_preboss_rest_hp_only_v1"


def file_hash(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_natural_setup(setup: dict) -> None:
    # Legacy natural-run journals predate initial_state. Explicit protocols must
    # identify a natural start: never reinterpret a synthetic prefix as State.new.
    initial = setup.get("initial_state", {"protocol": "natural_start"})
    if (
        setup.get("type") != "setup"
        or setup.get("ascension") != 0
        or not isinstance(initial, dict)
        or initial.get("protocol") != "natural_start"
    ):
        raise ValueError("Root source must be a natural A0 trainer journal")


def natural_state_from_setup(setup: dict) -> State:
    validate_natural_setup(setup)
    return State.new(
        setup["seed"],
        ascension=0,
        final_act=setup["final_act"],
        training_rng_seed=journal_environment_seed(setup),
    )


def reconstruct(
    path: Path, stop_step: int
) -> tuple[State, Observation | None, PolicyAction | None]:
    if stop_step < 0:
        raise ValueError("Negative root prefix length")
    with gzip.open(path, "rt") if path.suffix == ".gz" else path.open() as source:
        rows = [json.loads(line) for line in source]
    setup = rows[0]
    state = natural_state_from_setup(setup)
    decision = state.decision()
    visible = None
    previous = None
    accepted = {row["step"] for row in rows if row["type"] == "accepted"}
    steps = 0
    for row in rows:
        if row["type"] != "attempt":
            continue
        if steps == stop_step:
            break
        if (
            row["step"] != steps
            or steps not in accepted
            or decision.revision != row["revision"]
        ):
            raise ValueError("Root source accepted prefix/revision mismatch")
        index = row["index"]
        if (
            isinstance(index, bool)
            or not isinstance(index, int)
            or not 0 <= index < len(decision.actions)
        ):
            raise ValueError(
                "Root source public action index unavailable under current rules"
            )
        action = decision.actions[index]
        descriptor = PolicyAction.from_action(action)
        if asdict(descriptor) != row["action"]:
            raise ValueError("Root source public action mismatch")
        ob = decision.observation
        if ob.kind == "map":
            visible = ob
        if visible is not None and visible.context.act != ob.context.act:
            visible = None
        decision = state.step(action)
        if ob.kind != "combat":
            previous = descriptor
        steps += 1
    if steps != stop_step:
        raise ValueError("Root prefix is truncated")
    ob = decision.observation
    if (
        ob.kind != "rest"
        or ob.context.act != 1
        or ob.context.floor != 15
        or ob.screen.complete
    ):
        raise ValueError("Root must be an unused Act-1 floor-15 campfire")
    kinds = {a.kind for a in decision.actions}
    if not {"rest_heal", "rest_smith"} <= kinds:
        raise ValueError(
            "This curriculum requires both legal healing and smith choices"
        )
    return state, visible, previous


class RootBank:
    def __init__(self, manifest: Path, *, final_act: bool) -> None:
        self.path = manifest.resolve()
        data = json.loads(self.path.read_text())
        if data["protocol"] != ROOT_PROTOCOL or data["native_sha256"] != file_hash(
            Path(_native.__file__)
        ):
            raise ValueError(
                "Root-bank protocol/native mismatch; reconstruct and validate a new bank"
            )
        if data["final_act"] != final_act:
            raise ValueError("Root-bank final-act mismatch")
        self.cases = {}
        self.training = []
        self.validation = []
        self.cache = {}
        self.environment_seeds = {}
        for case in data["cases"]:
            seed = case["seed"]
            if (
                not isinstance(seed, str)
                or not seed.isascii()
                or not seed.isdecimal()
                or str(int(seed)) != seed
            ):
                raise ValueError("Root seeds must be canonical decimal strings")
            if (
                not 0 <= int(seed) < 2**63
                or seed in self.cases
                or case["split"] not in ("train", "validation")
            ):
                raise ValueError(
                    "Duplicate/invalid root seed or split; split by original seed, never HP variant"
                )
            path = self.path.parent / case["journal"]
            if file_hash(path) != case["journal_sha256"]:
                raise ValueError("Root journal hash mismatch")
            with (
                gzip.open(path, "rt") if path.suffix == ".gz" else path.open() as source
            ):
                setup = json.loads(source.readline())
            validate_natural_setup(setup)
            self.environment_seeds[seed] = journal_environment_seed(setup)
            if (
                setup["seed"] != seed
                or setup["final_act"] != final_act
                or setup["ascension"] != 0
            ):
                raise ValueError("Root entry does not match source seed/profile")
            self.cases[seed] = case
            (self.training if case["split"] == "train" else self.validation).append(
                seed
            )
        if not self.training or not self.validation:
            raise ValueError("Both root-bank splits must be nonempty")
        self.final_act = final_act

    def initial(
        self, seed: str, fraction: float
    ) -> tuple[State, Observation | None, PolicyAction | None, dict]:
        if not math.isfinite(fraction) or not 0 < fraction <= 1:
            raise ValueError("Synthetic HP fraction must be in (0, 1]")
        case = self.cases[seed]
        if seed not in self.cache:
            self.cache[seed] = reconstruct(
                self.path.parent / case["journal"], case["stop_step"]
            )
        source, visible, previous = self.cache[seed]
        ob = source.observation()
        if ob.context.final_act_available != self.final_act:
            raise ValueError("Root source final-act mismatch")
        hp = max(1, round(fraction * ob.context.player_max_hp))
        root = source.synthetic_rest_root(hp)
        return (
            root,
            visible,
            previous,
            {
                "protocol": ROOT_PROTOCOL,
                "training_rng_seed": self.environment_seeds[seed],
                "root_manifest": str(self.path),
                "root_journal_sha256": case["journal_sha256"],
                "prefix_accepted": case["stop_step"],
                "initial_hp": hp,
                "max_hp": ob.context.player_max_hp,
                "requested_hp_fraction": fraction,
            },
        )
