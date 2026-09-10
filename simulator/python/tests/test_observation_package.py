from __future__ import annotations

import ast
import subprocess
import unittest
from pathlib import Path

import sts_sim
from sts_sim import Card, CombatObservation, Relic, RestSmith, decode_observation
from sts_sim.observations import CombatObservation as FacadeCombatObservation
from sts_sim.observations import Relic as FacadeRelic
from sts_sim.observations import decode_observation as facade_decode_observation
from sts_sim.observations.combat import CombatObservation as DomainCombatObservation
from sts_sim.observations.common import Relic as DomainRelic
from sts_sim.observations.screens import RestSmith as DomainRestSmith

PYTHON_ROOT = Path(__file__).resolve().parents[1]
OBSERVATIONS_ROOT = PYTHON_ROOT / "sts_sim" / "observations"
PACKAGE = "sts_sim.observations"


def _module_name(path: Path) -> str:
    if path.name == "__init__.py":
        return PACKAGE
    return f"{PACKAGE}.{path.stem}"


def _internal_dependencies(path: Path) -> set[str]:
    module = _module_name(path)
    tree = ast.parse(path.read_text(), filename=str(path))
    deps: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                if alias.name == PACKAGE or alias.name.startswith(f"{PACKAGE}."):
                    deps.add(alias.name if alias.name != PACKAGE else PACKAGE)
        elif isinstance(node, ast.ImportFrom):
            if node.level == 0:
                if node.module == PACKAGE or (
                    node.module is not None and node.module.startswith(f"{PACKAGE}.")
                ):
                    deps.add(node.module)
                continue
            if node.level == 1:
                if node.module is None:
                    for alias in node.names:
                        deps.add(f"{PACKAGE}.{alias.name}")
                else:
                    deps.add(f"{PACKAGE}.{node.module.split('.', maxsplit=1)[0]}")
            elif node.level >= 2:
                continue
    deps.discard(module)
    return deps


def _cycle(graph: dict[str, set[str]]) -> tuple[str, ...] | None:
    visiting: set[str] = set()
    seen: set[str] = set()
    stack: list[str] = []

    def visit(node: str) -> tuple[str, ...] | None:
        if node in seen:
            return None
        if node in visiting:
            start = stack.index(node)
            return tuple(stack[start:] + [node])
        visiting.add(node)
        stack.append(node)
        for child in sorted(graph.get(node, ())):
            found = visit(child)
            if found is not None:
                return found
        stack.pop()
        visiting.remove(node)
        seen.add(node)
        return None

    for node in sorted(graph):
        found = visit(node)
        if found is not None:
            return found
    return None


class ObservationPackageRuntimeTest(unittest.TestCase):
    def test_public_names_keep_class_identity_across_reexports(self) -> None:
        self.assertIs(CombatObservation, FacadeCombatObservation)
        self.assertIs(FacadeCombatObservation, DomainCombatObservation)
        self.assertIs(sts_sim.CombatObservation, DomainCombatObservation)
        self.assertIs(Relic, FacadeRelic)
        self.assertIs(FacadeRelic, DomainRelic)
        self.assertIs(sts_sim.Relic, DomainRelic)
        self.assertIs(Card, sts_sim.observations.common.Card)
        self.assertIs(RestSmith, DomainRestSmith)
        self.assertIs(decode_observation, facade_decode_observation)
        self.assertIs(sts_sim.decode_observation, facade_decode_observation)
        self.assertEqual(DomainCombatObservation.__module__, "sts_sim.observations.combat")
        self.assertEqual(DomainRelic.__module__, "sts_sim.observations.common")
        self.assertEqual(DomainRestSmith.__module__, "sts_sim.observations.screens")

    def test_observation_submodules_have_no_import_cycles(self) -> None:
        graph = {
            _module_name(path): _internal_dependencies(path)
            for path in sorted(OBSERVATIONS_ROOT.glob("*.py"))
        }
        self.assertIn(PACKAGE, graph)
        self.assertIn(f"{PACKAGE}.combat", graph)
        self.assertIn(f"{PACKAGE}.common", graph)
        self.assertIn(f"{PACKAGE}.screens", graph)
        self.assertIn(f"{PACKAGE}._decode", graph)
        self.assertEqual(graph[f"{PACKAGE}._decode"], set())
        self.assertNotIn(PACKAGE, graph[f"{PACKAGE}.combat"])
        self.assertNotIn(PACKAGE, graph[f"{PACKAGE}.common"])
        self.assertNotIn(PACKAGE, graph[f"{PACKAGE}.screens"])
        self.assertNotIn(PACKAGE, graph[f"{PACKAGE}._decode"])
        found = _cycle(graph)
        self.assertIsNone(found, f"import cycle: {' -> '.join(found or ())}")


class ObservationPackageStaticTest(unittest.TestCase):
    def test_ty_accepts_facade_and_domain_module_narrowing(self) -> None:
        result = subprocess.run(
            ["uv", "run", "ty", "check", "tests/typing/narrow_observation_modules.py"],
            cwd=PYTHON_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
