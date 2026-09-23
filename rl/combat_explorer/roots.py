"""Synthetic validation roots and on-demand generated combat specifications."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from loadout_sampling import LoadoutSampler
from scenarios import COMBAT_FLOORS, ScenarioConfig
from sts_sim import State
from synthetic_roots import sample_root
from validation_set import load_validation, native_sha256

from combat_explorer.errors import ExplorerError, NotFoundError
from combat_explorer.jsonutil import observation_sha256, parse_seed, seed_to_str, sha256_bytes
from combat_explorer.presentation import present_context


@dataclass(frozen=True)
class RootCase:
    id: str
    label: str
    act: int
    kind: str
    spec: dict[str, Any]
    spec_json: str
    initial_observation_sha256: str
    source_band: str | None
    floor: int
    encounter: str
    hp: int
    max_hp: int
    deck_size: int
    relic_count: int
    gold: int


@dataclass(frozen=True)
class PreparedRoot:
    spec_json: str
    spec: dict[str, Any]
    state: State
    provenance: dict[str, Any]


class RootService:
    def __init__(self, *, validation_manifest: Path | None, distributions: Path | None) -> None:
        self.validation_manifest = validation_manifest
        self.distributions_path = distributions
        self._document: dict[str, Any] | None = None
        self._cases: dict[str, RootCase] = {}
        self._manifest_sha256: str | None = None
        self._sampler: LoadoutSampler | None = None
        self._distributions_sha256: str | None = None
        if validation_manifest is not None:
            self._load_manifest(validation_manifest)
        if distributions is not None:
            self._sampler = LoadoutSampler.load(distributions)
            self._distributions_sha256 = sha256_bytes(distributions.read_bytes())

    def _load_manifest(self, path: Path) -> None:
        document, _groups = load_validation(path)
        cases: dict[str, RootCase] = {}
        for raw in document["cases"]:
            spec = raw["spec"]
            spec_json = json.dumps(spec)
            cases[raw["id"]] = RootCase(
                id=raw["id"],
                label=raw["label"],
                act=int(raw["act"]),
                kind=str(raw["kind"]),
                spec=spec,
                spec_json=spec_json,
                initial_observation_sha256=str(raw["initial_observation_sha256"]),
                source_band=raw.get("source_band"),
                floor=int(spec["floor"]),
                encounter=str(spec["encounter"]),
                hp=int(spec["hp"]),
                max_hp=int(spec["max_hp"]),
                deck_size=len(spec["deck"]),
                relic_count=len(spec["relics"]),
                gold=int(spec["gold"]),
            )
        self._document = document
        self._cases = cases
        self._manifest_sha256 = sha256_bytes(path.read_bytes())

    def capabilities(self) -> dict[str, Any]:
        return {
            "validation_manifest": None if self.validation_manifest is None else str(self.validation_manifest),
            "validation_protocol": None if self._document is None else self._document.get("protocol"),
            "validation_cases": len(self._cases),
            "distributions": None if self.distributions_path is None else str(self.distributions_path),
            "native_sha256": native_sha256(),
            "combat_floors": list(COMBAT_FLOORS),
            "ascension": 0,
        }

    def list_cases(self, *, query: str = "", act: int | None = None, kind: str | None = None) -> list[dict[str, Any]]:
        if self._document is None:
            return []
        needle = query.strip().lower()
        rows = []
        for case in self._cases.values():
            if act is not None and case.act != act:
                continue
            if kind is not None and case.kind != kind:
                continue
            haystack = " ".join(
                [
                    case.id,
                    case.label,
                    case.kind,
                    case.encounter,
                    str(case.floor),
                    str(case.act),
                ]
            ).lower()
            if needle and needle not in haystack:
                continue
            rows.append(self.case_summary(case))
        return rows

    def case_summary(self, case: RootCase) -> dict[str, Any]:
        return {
            "id": case.id,
            "label": case.label,
            "act": case.act,
            "kind": case.kind,
            "floor": case.floor,
            "encounter": case.encounter,
            "hp": case.hp,
            "max_hp": case.max_hp,
            "deck_size": case.deck_size,
            "relic_count": case.relic_count,
            "gold": case.gold,
            "seed": seed_to_str(case.spec["seed"]),
            "source_band": case.source_band,
        }

    def from_case(self, case_id: str) -> PreparedRoot:
        if self._document is None or self.validation_manifest is None:
            raise ExplorerError("No synthetic validation manifest is configured")
        case = self._cases.get(case_id)
        if case is None:
            raise NotFoundError(f"Unknown validation case {case_id}")
        state = State.from_synthetic_spec(case.spec_json)
        digest = observation_sha256(state.observation())
        if digest != case.initial_observation_sha256:
            raise ExplorerError(f"Validation initial observation mismatch: {case.id}")
        provenance = {
            "kind": "validation",
            "case_id": case.id,
            "label": case.label,
            "act": case.act,
            "kind_encounter": case.kind,
            "encounter": case.encounter,
            "floor": case.floor,
            "seed": seed_to_str(case.spec["seed"]),
            "manifest_path": str(self.validation_manifest),
            "manifest_sha256": self._manifest_sha256,
            "native_sha256": native_sha256(),
            "initial_observation_sha256": case.initial_observation_sha256,
            "source_band": case.source_band,
            "context": present_context(state.observation()),
        }
        return PreparedRoot(case.spec_json, case.spec, state, provenance)

    def generate(
        self,
        *,
        generation_seed: str,
        floor: int | None = None,
        min_floor: int = 1,
        max_floor: int = 55,
    ) -> PreparedRoot:
        if self._sampler is None or self.distributions_path is None:
            raise ExplorerError("No loadout distributions are configured")
        rng = __import__("random").Random(parse_seed(generation_seed, field="generation_seed"))
        config = ScenarioConfig(min_floor=min_floor, max_floor=max_floor)
        sampled = sample_root(rng, self._sampler, floor, config=config)
        spec = json.loads(sampled.spec_json)
        provenance = {
            "kind": "generated",
            "generation_seed": seed_to_str(generation_seed),
            "seed": seed_to_str(spec["seed"]),
            "floor": spec["floor"],
            "encounter": spec["encounter"],
            "kind_encounter": spec["kind"],
            "act": sampled.encounter.act,
            "distributions_path": str(self.distributions_path),
            "distributions_sha256": self._distributions_sha256,
            "native_sha256": native_sha256(),
            "initial_observation_sha256": observation_sha256(sampled.state.observation()),
            "source_band": sampled.loadout.source_band,
            "rejected_loadouts": list(sampled.rejected_loadouts),
            "context": present_context(sampled.state.observation()),
        }
        return PreparedRoot(sampled.spec_json, spec, sampled.state, provenance)
