"""Current-only paired-state beam-vs-PUCT label A/B protocol.

This is not a generic treatment framework and not deleted treatment_control
compatibility. Disk plans and the production CLI accept only the frozen
production encoding. Pytest may construct a private in-memory execution
object that production disk loaders and CLI entrypoints cannot reach.
"""

from __future__ import annotations

import json
import math
import os
import stat
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, fields
from pathlib import Path
from types import MappingProxyType
from typing import Literal, cast

from ..fair import FairCombatObservation, FairRunObservation
from ..run import Action, ActionDescriptor, Decision, RunEnv
from .authorization import (
    MANDATORY_DISJOINTNESS_DIMENSIONS,
    authorization_from_bindings,
    load_authorization,
    require_held_out_evaluation,
    require_pairwise_disjoint_cohorts,
    write_authorization,
)
from .data import (
    _LOADABLE_SPLITS,
    _NATIVE_EPISODE_ERROR,
    _SOURCE_KIND,
    DatasetExclusion,
    DatasetManifest,
    DatasetRootMembership,
    RootManifest,
    _package_repository_root,
    _publish_dataset,
    _restore_labeled_root,
    generate_beam_dataset,
    generate_legal_roots,
    load_dataset_manifest,
    load_root_manifest,
)
from .experiment import (
    _ENVIRONMENT_KEYS,
    ARTIFACT_INVENTORY_NAME,
    PREDECLARATION_KIND,
    PREDECLARATION_SCHEMA_VERSION,
    ArtifactRef,
    ExperimentPredeclaration,
    _absolute_without_follow,
    _ensure_directory_nofollow,
    _iter_tree_entries,
    _lexically_normalized,
    _lstat,
    _raise_if_symlink_ancestor,
    _read_regular_file_bytes,
    _relative_to_root,
    _require_bool,
    _require_digest,
    _require_git_sha,
    _require_mapping,
    _require_string,
    _scientific_file_digests,
    load_experiment_predeclaration,
    reproduce_experiment,
    verify_artifact_integrity,
    write_artifact_inventory,
    write_scientific_artifact,
)
from .gameplay import (
    MATCHED_PUCT_REPORT_ARMS,
    canonical_public_action_descriptors,
    evaluate_matched_puct_gameplay,
    random_policy_index,
)
from .provenance import (
    RepositoryVersion,
    canonical_bytes,
    capture_repository_version,
    sha256_bytes,
)
from .puct import network_leaf_evaluator, puct_search_payload
from .puct_data import (
    AuthoritativeRootMutationError,
    _load_teacher_checkpoint,
    _puct_search_config,
)
from .records import (
    BEAM_TEACHER_NAME,
    COMBAT_PROXY_VALUE_TARGET_NAME,
    FAIR_LEAF_BATCH_SCHEMA,
    PUCT_SEARCH_ROOT_MEAN_NAME,
    PUCT_TEACHER_NAME,
    PUCT_TEACHER_VERSION,
    RECORD_VERSION,
    CombatOutcome,
    JsonValue,
    SymbolicTrainingRecord,
    action_descriptor_from_payload,
    canonical_episode_id,
    fair_observation_digest,
    first_argmax_visits,
    validate_beam_search_config,
)
from .rewards import COMBAT_PROXY_V1, CombatRewardConfig
from .tensor import Vocabularies, encoder_contract_digest
from .training import (
    TrainingConfig,
    TrainingResult,
    _digest,
    _model_state_digest,
    clone_model_state,
    create_common_initial_model_state,
    fit_union_vocabularies,
    load_training_checkpoint,
    load_training_checkpoint_bytes,
    train_beam_clone,
)

PLAN_KIND = "beam_puct_paired_label_ab_plan_v1"
PLAN_SCHEMA_VERSION = 1
PLAN_NAME = "beam-label-vs-privileged-puct-label-depth4-v1"
RESULT_KIND = "beam_puct_paired_label_ab_result_v1"
RESULT_SCHEMA_VERSION = 1
PRODUCTION_SEED_PREFIX = "BEAMPUCTAB"
PRODUCTION_BOOTSTRAP_START = 0
PRODUCTION_BOOTSTRAP_COUNT = 2400
PRODUCTION_TREATMENT_START = 2400
PRODUCTION_TREATMENT_COUNT = 2400
PRODUCTION_HELD_OUT_START = 4800
PRODUCTION_HELD_OUT_COUNT = 2400
PRODUCTION_BEHAVIOR_SEED = 20260903
PRODUCTION_EVALUATION_SEED = 20260903
PRODUCTION_BOOTSTRAP_DRAWS = 10_000
PRODUCTION_MINIMUM_HELD_OUT_ROOTS = 120
NONLEARNED_ARMS: tuple[str, ...] = (
    "random",
    "beam",
    "uniform_prior_constant_value_puct",
)
BEHAVIOR_WALK_CONTRACT: dict[str, object] = {
    "name": "sha256_public_descriptor_choice",
    "version": "behavior_seed_root_id_decision_index_v1",
    "fields": [
        "behavior_seed",
        "root_id",
        "decision_index",
        "canonical_public_action_descriptors",
    ],
}
_PLAN_KEYS = frozenset(
    {
        "kind",
        "schema_version",
        "name",
        "source_commit",
        "source_worktree_must_be_clean",
        "promotion_claim",
        "consumed_evidence_policy",
        "estimand",
        "behavior_walk",
        "cohorts",
        "root_generation",
        "bootstrap_teacher",
        "beam_query",
        "puct_query",
        "training",
        "evaluation",
        "metrics",
        "abort_rules",
        "publication",
        "plan_digest",
    }
)
_RESULT_KEYS = frozenset(
    {
        "kind",
        "schema_version",
        "plan_digest",
        "primary",
        "secondary",
        "integrity",
        "bootstrap",
        "promotion_claim",
        "result_digest",
    }
)
_PRIMARY_KEYS = frozenset({"name", "delta", "roots", "note"})
_SECONDARY_KEYS = frozenset({"paired_network_puct_win_rate_delta", "network_puct_roots"})
_INTEGRITY_KEYS = frozenset({"nonlearned_arms_identical", "nonlearned_arms", "promotion_claim"})
_BOOTSTRAP_RESULT_KEYS = frozenset(
    {"stream", "draws", "seed", "roots", "observed_delta", "percentile_ci_95"}
)
PUBLISHED_EXPERIMENT_FILES: tuple[str, ...] = (
    "plan.json",
    "result.json",
    "student-beam.pt",
    "student-puct.pt",
    "teacher.pt",
    "beam_gameplay.json",
    "puct_gameplay.json",
    "union-vocabularies.json",
    "authorization.json",
    "predeclaration.json",
    ARTIFACT_INVENTORY_NAME,
)
PUBLISHED_INPUT_DIR = "inputs"
DESIGNATED_RERUN_ARTIFACTS: tuple[str, ...] = PUBLISHED_EXPERIMENT_FILES
_PUBLISHED_INPUT_TREES: tuple[tuple[str, str, str], ...] = (
    ("bootstrap_root_manifest", "inputs/bootstrap", "root-manifest.json"),
    ("root_manifest", "inputs/treatment", "root-manifest.json"),
    ("held_out_root_manifest", "inputs/held-out", "root-manifest.json"),
    ("dataset_manifest", "inputs/beam-dataset", "dataset-manifest.json"),
    ("puct_dataset_manifest", "inputs/puct-dataset", "dataset-manifest.json"),
)
_PAIRED_ALLOWED_DIFF_FIELDS = frozenset(
    {
        "planner_name",
        "planner_version",
        "search_config",
        "teacher_visit_counts",
        "chosen_action_index",
        "chosen_action",
        "search_root_mean_value",
        "record_id",
        "episode_id",
    }
)
_RECORD_FIELD_NAMES = frozenset(item.name for item in fields(SymbolicTrainingRecord))
if not _PAIRED_ALLOWED_DIFF_FIELDS <= _RECORD_FIELD_NAMES:
    raise RuntimeError("paired allowed-diff set is not a subset of SymbolicTrainingRecord")
OutcomeStatus = Literal["won", "lost", "escaped", "truncated"]
_DIRECTORY_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_DIRECTORY | os.O_CLOEXEC


def _require_exact_keys(source: Mapping[str, object], keys: frozenset[str], label: str) -> None:
    if set(source) != keys:
        raise ValueError(f"{label} has missing or unknown fields")


def _require_int(value: object, label: str) -> int:
    if type(value) is not int:
        raise TypeError(f"{label} must be an integer")
    return value


def _require_positive_int(value: object, label: str) -> int:
    integer = _require_int(value, label)
    if integer <= 0:
        raise ValueError(f"{label} must be a positive integer")
    return integer


def _require_float(value: object, label: str) -> float:
    if type(value) is int:
        number = float(value)
    elif type(value) is float:
        number = value
    else:
        raise TypeError(f"{label} must be numeric")
    if not math.isfinite(number):
        raise ValueError(f"{label} must be finite")
    return number


def _require_string_list(value: object, label: str) -> tuple[str, ...]:
    if type(value) is not list:
        raise TypeError(f"{label} must be an array")
    names: list[str] = []
    seen: set[str] = set()
    for item in cast(list[object], value):
        name = _require_string(item, label)
        if name in seen:
            raise ValueError(f"{label} contains a duplicate entry")
        seen.add(name)
        names.append(name)
    return tuple(names)


def _deep_freeze(value: object) -> object:
    if type(value) is dict:
        frozen = {key: _deep_freeze(item) for key, item in cast(dict[str, object], value).items()}
        return MappingProxyType(frozen)
    if type(value) is list:
        return tuple(_deep_freeze(item) for item in cast(list[object], value))
    return value


def _deep_thaw(value: object) -> object:
    if isinstance(value, Mapping):
        return {key: _deep_thaw(item) for key, item in value.items()}
    if type(value) is tuple:
        return [_deep_thaw(item) for item in cast(tuple[object, ...], value)]
    return value


def _lexical_path(path: Path) -> Path:
    lexical = _lexically_normalized(_absolute_without_follow(path))
    _raise_if_symlink_ancestor(lexical)
    info = _lstat(lexical)
    if info is not None and stat.S_ISLNK(info.st_mode):
        raise ValueError(f"refusing a symlink path: {path}")
    return lexical


def _held_directory_names(path: Path) -> tuple[str, ...]:
    lexical = _lexical_path(path)
    descriptor = os.open(os.fspath(lexical), _DIRECTORY_FLAGS)
    try:
        names = tuple(sorted(os.listdir(descriptor)))
    finally:
        os.close(descriptor)
    return names


def _require_unpublished_dir(path: Path) -> Path:
    lexical = _lexically_normalized(_absolute_without_follow(path))
    info = _lstat(lexical)
    if info is not None and stat.S_ISLNK(info.st_mode):
        raise ValueError(f"directory must not be a symlink: {path}")
    _raise_if_symlink_ancestor(lexical)
    descriptor = _ensure_directory_nofollow(lexical)
    try:
        names = os.listdir(descriptor)
    finally:
        os.close(descriptor)
    if names:
        raise ValueError(f"directory must be empty: {path}")
    return lexical


def _repository_root(repository: Path | None) -> Path:
    if repository is None:
        return _package_repository_root()
    return _lexical_path(repository)


def _published_membership() -> frozenset[str]:
    return frozenset(PUBLISHED_EXPERIMENT_FILES) | {PUBLISHED_INPUT_DIR}


def _copy_tree_nofollow(source: Path, destination: Path) -> tuple[str, ...]:
    source_root = _lexical_path(source)
    dest_root = _require_unpublished_dir(destination)
    copied: list[str] = []
    for path, kind in _iter_tree_entries(source_root):
        if kind == "symlink":
            raise ValueError(f"refusing to copy a symlink tree member: {path}")
        if kind != "file":
            continue
        relative = _relative_to_root(source_root, path).as_posix()
        dest = dest_root / relative
        parent = _ensure_directory_nofollow(dest.parent)
        os.close(parent)
        write_scientific_artifact(dest, _read_regular_file_bytes(_lexical_path(path)))
        copied.append(relative)
    if not copied:
        raise ValueError(f"input tree is empty: {source}")
    return tuple(sorted(copied))


def _copied_tree_refs(
    experiment: Path,
    relative_dir: str,
    manifest_filename: str,
    manifest_role: str,
) -> list[ArtifactRef]:
    tree = experiment / relative_dir
    refs: list[ArtifactRef] = []
    for path, kind in _iter_tree_entries(tree):
        if kind == "symlink":
            raise ValueError(f"published input tree contains a symlink: {path}")
        if kind != "file":
            continue
        member = _relative_to_root(tree, path).as_posix()
        relative = f"{relative_dir}/{member}"
        role = manifest_role if member == manifest_filename else "input_tree_member"
        refs.append(_artifact_ref(role, Path(relative), relative=True, experiment_dir=experiment))
    if not refs:
        raise ValueError(f"published input tree is empty: {relative_dir}")
    refs.sort(key=lambda ref: ref.path)
    return refs


def _is_production_plan(plan: LabelAbPlan) -> bool:
    expected = production_plan_payload(plan.source_commit)
    return canonical_bytes(plan.to_dict()) == canonical_bytes(expected)


def _load_label_ab_plan_bytes(path: Path) -> tuple[LabelAbPlan, bytes]:
    content = _read_regular_file_bytes(_lexical_path(path))
    try:
        payload = json.loads(content)
    except json.JSONDecodeError as error:
        raise ValueError("label A/B plan is not JSON") from error
    plan = parse_label_ab_plan(payload)
    expected = canonical_bytes(plan.to_dict())
    if content != expected:
        raise ValueError("label A/B plan bytes are not canonical")
    return plan, expected


def _plan_for_execution(execution: _Execution, source_commit: str) -> LabelAbPlan:
    """Private in-memory plan matching ``execution``. Production loaders reject it."""

    thawed = json.loads(canonical_bytes(production_plan_payload(source_commit)).decode())
    if type(thawed) is not dict:
        raise TypeError("production plan payload must be an object")
    payload = cast(dict[str, object], thawed)
    walk = dict(_require_mapping(payload["behavior_walk"], "behavior_walk"))
    walk["behavior_seed"] = execution.behavior_seed
    payload["behavior_walk"] = walk
    cohorts = dict(_require_mapping(payload["cohorts"], "cohorts"))
    cohorts["seed_prefix"] = execution.seed_prefix
    cohorts["bootstrap"] = {
        "start": execution.bootstrap_start,
        "count": execution.bootstrap_count,
    }
    cohorts["treatment"] = {
        "start": execution.treatment_start,
        "count": execution.treatment_count,
    }
    cohorts["held_out"] = {
        "start": execution.held_out_start,
        "count": execution.held_out_count,
    }
    payload["cohorts"] = cohorts
    roots = dict(_require_mapping(payload["root_generation"], "root_generation"))
    roots["ascension"] = execution.ascension
    roots["combat_depth"] = execution.combat_depth
    roots["max_run_steps"] = execution.max_run_steps
    payload["root_generation"] = roots
    beam = dict(_require_mapping(payload["beam_query"], "beam_query"))
    beam["depth"] = execution.beam_depth
    beam["width"] = execution.beam_width
    beam["transition_budget"] = execution.beam_transition_budget
    beam["max_decisions"] = execution.max_decisions
    beam["max_player_turns"] = execution.max_player_turns
    beam["deduplicate_search_states"] = execution.deduplicate_search_states
    payload["beam_query"] = beam
    puct = dict(_require_mapping(payload["puct_query"], "puct_query"))
    puct["c_puct"] = execution.c_puct
    puct["simulation_budget"] = execution.simulation_budget
    puct["transition_budget"] = execution.puct_transition_budget
    puct["max_decisions"] = execution.max_decisions
    puct["max_player_turns"] = execution.max_player_turns
    payload["puct_query"] = puct
    training = dict(_require_mapping(payload["training"], "training"))
    config = execution.training
    training["seed"] = config.seed
    training["batch_size"] = config.batch_size
    training["total_steps"] = config.total_steps
    training["learning_rate"] = config.learning_rate
    training["weight_decay"] = config.weight_decay
    training["torch_threads"] = config.torch_threads
    training["minimum_roots"] = config.minimum_roots
    training["minimum_lineages"] = config.minimum_lineages
    training["model_width"] = config.model_width
    training["model_heads"] = config.model_heads
    training["model_layers"] = config.model_layers
    training["feedforward_width"] = config.feedforward_width
    payload["training"] = training
    evaluation = dict(_require_mapping(payload["evaluation"], "evaluation"))
    evaluation["evaluation_seed"] = execution.evaluation_seed
    evaluation["c_puct"] = execution.eval_c_puct
    evaluation["simulation_budget"] = execution.eval_simulation_budget
    evaluation["transition_budget"] = execution.eval_transition_budget
    evaluation["beam_depth"] = execution.eval_beam_depth
    evaluation["beam_width"] = execution.eval_beam_width
    evaluation["max_decisions"] = execution.eval_max_decisions
    evaluation["max_player_turns"] = execution.eval_max_player_turns
    evaluation["minimum_roots"] = execution.minimum_held_out_roots
    payload["evaluation"] = evaluation
    metrics = dict(_require_mapping(payload["metrics"], "metrics"))
    metrics["cluster_bootstrap_draws"] = execution.bootstrap_draws
    metrics["cluster_bootstrap_seed"] = execution.bootstrap_seed
    payload["metrics"] = metrics
    unsigned = {key: value for key, value in payload.items() if key != "plan_digest"}
    payload["plan_digest"] = sha256_bytes(canonical_bytes(unsigned))
    frozen = _deep_freeze(payload)
    if not isinstance(frozen, Mapping):
        raise TypeError("frozen label A/B plan must be an object")
    return LabelAbPlan(cast(Mapping[str, object], frozen))


def production_plan_payload(source_commit: str) -> dict[str, object]:
    """Return the frozen production protocol object for ``source_commit``."""

    unsigned: dict[str, object] = {
        "kind": PLAN_KIND,
        "schema_version": PLAN_SCHEMA_VERSION,
        "name": PLAN_NAME,
        "source_commit": _require_git_sha(source_commit, "source_commit"),
        "source_worktree_must_be_clean": True,
        "promotion_claim": False,
        "consumed_evidence_policy": {
            "sealed_test": False,
            "real_trace_audit": False,
            "development_only_for_assessment": True,
        },
        "estimand": {
            "name": "paired_state_policy_label_treatment",
            "unit": "combat_root_decision",
            "treatment": "beam_label_vs_privileged_puct_label",
        },
        "behavior_walk": {
            "policy": BEHAVIOR_WALK_CONTRACT,
            "behavior_seed": PRODUCTION_BEHAVIOR_SEED,
            "advance": "original_public_action_sidecar",
        },
        "cohorts": {
            "seed_prefix": PRODUCTION_SEED_PREFIX,
            "bootstrap": {
                "start": PRODUCTION_BOOTSTRAP_START,
                "count": PRODUCTION_BOOTSTRAP_COUNT,
            },
            "treatment": {
                "start": PRODUCTION_TREATMENT_START,
                "count": PRODUCTION_TREATMENT_COUNT,
            },
            "held_out": {
                "start": PRODUCTION_HELD_OUT_START,
                "count": PRODUCTION_HELD_OUT_COUNT,
            },
            "no_top_up": True,
        },
        "root_generation": {
            "character": "ironclad",
            "ascension": 0,
            "combat_depth": 4,
            "max_run_steps": 2048,
        },
        "bootstrap_teacher": {
            "labeler": "ordinary_beam_trajectories",
            "consumed": True,
        },
        "beam_query": {
            "depth": 8,
            "width": 24,
            "transition_budget": 5000,
            "max_decisions": 512,
            "max_player_turns": 100,
            "deadline": None,
            "replan": "every_public_decision",
            "deduplicate_search_states": True,
        },
        "puct_query": {
            "c_puct": 1.5,
            "simulation_budget": 64,
            "transition_budget": 64,
            "max_decisions": 512,
            "max_player_turns": 100,
            "deadline": None,
            "replan": "every_public_decision",
            "privileged": True,
            "leaf_schema": FAIR_LEAF_BATCH_SCHEMA,
            "leaf_cache": "exact_state",
            "value_target_name": COMBAT_PROXY_VALUE_TARGET_NAME,
            "search_root_mean_name": PUCT_SEARCH_ROOT_MEAN_NAME,
        },
        "training": {
            "device": "cpu",
            "seed": 7,
            "batch_size": 32,
            "total_steps": 4000,
            "learning_rate": 0.001,
            "weight_decay": 0.0001,
            "torch_threads": 1,
            "minimum_roots": 225,
            "minimum_lineages": 100,
            "model_width": 96,
            "model_heads": 4,
            "model_layers": 2,
            "feedforward_width": 192,
            "dropout": 0.0,
            "early_stopping": False,
            "selection": "final_step_only",
        },
        "evaluation": {
            "evaluation_seed": PRODUCTION_EVALUATION_SEED,
            "c_puct": 1.5,
            "simulation_budget": 64,
            "transition_budget": 64,
            "beam_depth": 8,
            "beam_width": 24,
            "max_decisions": 512,
            "max_player_turns": 100,
            "pool_loadable_splits": True,
            "authorized_evaluator_names": sorted(MATCHED_PUCT_REPORT_ARMS),
            "minimum_roots": PRODUCTION_MINIMUM_HELD_OUT_ROOTS,
        },
        "metrics": {
            "primary": "paired_official_greedy_network_win_rate_delta",
            "secondary": ["paired_network_puct_win_rate_delta"],
            "cluster_bootstrap_draws": PRODUCTION_BOOTSTRAP_DRAWS,
            "cluster_bootstrap_seed": PRODUCTION_EVALUATION_SEED,
            "cluster_bootstrap_stream": "cluster_bootstrap_v1",
            "nonlearned_arms": list(NONLEARNED_ARMS),
            "errors_in_denominator": True,
        },
        "abort_rules": {
            "no_top_up": True,
            "no_peeking": True,
            "completeness_or_abort": True,
            "consumed_bootstrap": True,
            "no_promotion": True,
        },
        "publication": {
            "immutable_nofollow": True,
            "checkpoint_work_outside_experiment": True,
            "copy_final_bytes": True,
            "reproduce_experiment_is_identity_verification": True,
            "mandatory_files": ["predeclaration.json", ARTIFACT_INVENTORY_NAME],
        },
    }
    payload = dict(unsigned)
    payload["plan_digest"] = sha256_bytes(canonical_bytes(unsigned))
    return payload


@dataclass(frozen=True, slots=True)
class LabelAbPlan:
    _payload: Mapping[str, object]

    @property
    def source_commit(self) -> str:
        return _require_string(self._payload["source_commit"], "source_commit")

    @property
    def plan_digest(self) -> str:
        return _require_digest(self._payload["plan_digest"], "plan_digest")

    def to_dict(self) -> dict[str, object]:
        thawed = _deep_thaw(self._payload)
        if type(thawed) is not dict:
            raise TypeError("label A/B plan payload must thaw to an object")
        return cast(dict[str, object], thawed)


def parse_label_ab_plan(payload: object) -> LabelAbPlan:
    source = _require_mapping(payload, "label A/B plan")
    _require_exact_keys(source, _PLAN_KEYS, "label A/B plan")
    expected = production_plan_payload(_require_git_sha(source["source_commit"], "source_commit"))
    if canonical_bytes(source) != canonical_bytes(expected):
        raise ValueError("production plan constants mismatch")
    frozen = _deep_freeze(expected)
    if not isinstance(frozen, Mapping):
        raise TypeError("frozen label A/B plan must be an object")
    return LabelAbPlan(cast(Mapping[str, object], frozen))


def require_production_plan(plan: LabelAbPlan) -> None:
    """Reject any plan that does not encode the frozen production constants."""

    expected = production_plan_payload(plan.source_commit)
    if canonical_bytes(plan.to_dict()) != canonical_bytes(expected):
        raise ValueError("production plan constants mismatch")


def load_label_ab_plan(path: Path) -> LabelAbPlan:
    """Load a canonical production plan. Tiny fixtures are never accepted."""

    plan, _content = _load_label_ab_plan_bytes(path)
    return plan


def write_label_ab_plan(path: Path, *, repository: Path | None = None) -> LabelAbPlan:
    """Freeze the production plan against the current clean source epoch."""

    version = capture_repository_version(_repository_root(repository))
    plan = parse_label_ab_plan(production_plan_payload(version.git_sha))
    write_scientific_artifact(path, canonical_bytes(plan.to_dict()))
    return plan


def _bind_live_source(plan: LabelAbPlan, *, repository: Path | None = None) -> RepositoryVersion:
    require_production_plan(plan)
    version = capture_repository_version(_repository_root(repository))
    if not version.clean:
        raise ValueError("label A/B requires a clean source worktree")
    if version.git_sha != plan.source_commit:
        raise ValueError("plan.source_commit does not match current clean HEAD")
    return version


@dataclass(frozen=True, slots=True)
class _Execution:
    """Private runtime knobs. Production disk and CLI cannot construct this."""

    behavior_seed: int
    seed_prefix: str
    bootstrap_start: int
    bootstrap_count: int
    treatment_start: int
    treatment_count: int
    held_out_start: int
    held_out_count: int
    ascension: int
    combat_depth: int
    max_run_steps: int
    beam_depth: int
    beam_width: int
    beam_transition_budget: int
    max_decisions: int
    max_player_turns: int
    deduplicate_search_states: bool
    c_puct: float
    simulation_budget: int
    puct_transition_budget: int
    training: TrainingConfig
    evaluation_seed: int
    eval_c_puct: float
    eval_simulation_budget: int
    eval_transition_budget: int
    eval_beam_depth: int
    eval_beam_width: int
    eval_max_decisions: int
    eval_max_player_turns: int
    minimum_held_out_roots: int
    bootstrap_draws: int
    bootstrap_seed: int
    reward_config: CombatRewardConfig = COMBAT_PROXY_V1


def _execution_from_payload(plan: LabelAbPlan) -> _Execution:
    payload = plan.to_dict()
    cohorts = _require_mapping(payload["cohorts"], "cohorts")
    bootstrap = _require_mapping(cohorts["bootstrap"], "bootstrap")
    treatment = _require_mapping(cohorts["treatment"], "treatment")
    held_out = _require_mapping(cohorts["held_out"], "held_out")
    roots = _require_mapping(payload["root_generation"], "root_generation")
    beam = _require_mapping(payload["beam_query"], "beam_query")
    puct = _require_mapping(payload["puct_query"], "puct_query")
    training = _require_mapping(payload["training"], "training")
    evaluation = _require_mapping(payload["evaluation"], "evaluation")
    metrics = _require_mapping(payload["metrics"], "metrics")
    walk = _require_mapping(payload["behavior_walk"], "behavior_walk")
    return _Execution(
        behavior_seed=_require_int(walk["behavior_seed"], "behavior_seed"),
        seed_prefix=_require_string(cohorts["seed_prefix"], "seed_prefix"),
        bootstrap_start=_require_int(bootstrap["start"], "bootstrap.start"),
        bootstrap_count=_require_positive_int(bootstrap["count"], "bootstrap.count"),
        treatment_start=_require_int(treatment["start"], "treatment.start"),
        treatment_count=_require_positive_int(treatment["count"], "treatment.count"),
        held_out_start=_require_int(held_out["start"], "held_out.start"),
        held_out_count=_require_positive_int(held_out["count"], "held_out.count"),
        ascension=_require_int(roots["ascension"], "ascension"),
        combat_depth=_require_positive_int(roots["combat_depth"], "combat_depth"),
        max_run_steps=_require_positive_int(roots["max_run_steps"], "max_run_steps"),
        beam_depth=_require_positive_int(beam["depth"], "beam.depth"),
        beam_width=_require_positive_int(beam["width"], "beam.width"),
        beam_transition_budget=_require_positive_int(
            beam["transition_budget"], "beam.transition_budget"
        ),
        max_decisions=_require_positive_int(beam["max_decisions"], "max_decisions"),
        max_player_turns=_require_positive_int(beam["max_player_turns"], "max_player_turns"),
        deduplicate_search_states=_require_bool(
            beam["deduplicate_search_states"], "deduplicate_search_states"
        ),
        c_puct=_require_float(puct["c_puct"], "c_puct"),
        simulation_budget=_require_positive_int(puct["simulation_budget"], "simulation_budget"),
        puct_transition_budget=_require_positive_int(
            puct["transition_budget"], "puct.transition_budget"
        ),
        training=TrainingConfig(
            seed=_require_int(training["seed"], "training.seed"),
            batch_size=_require_positive_int(training["batch_size"], "training.batch_size"),
            total_steps=_require_positive_int(training["total_steps"], "training.total_steps"),
            learning_rate=_require_float(training["learning_rate"], "training.learning_rate"),
            weight_decay=_require_float(training["weight_decay"], "training.weight_decay"),
            torch_threads=_require_positive_int(
                training["torch_threads"], "training.torch_threads"
            ),
            minimum_roots=_require_positive_int(
                training["minimum_roots"], "training.minimum_roots"
            ),
            minimum_lineages=_require_positive_int(
                training["minimum_lineages"], "training.minimum_lineages"
            ),
            model_width=_require_positive_int(training["model_width"], "training.model_width"),
            model_heads=_require_positive_int(training["model_heads"], "training.model_heads"),
            model_layers=_require_positive_int(training["model_layers"], "training.model_layers"),
            feedforward_width=_require_positive_int(
                training["feedforward_width"], "training.feedforward_width"
            ),
        ),
        evaluation_seed=_require_int(evaluation["evaluation_seed"], "evaluation_seed"),
        eval_c_puct=_require_float(evaluation["c_puct"], "evaluation.c_puct"),
        eval_simulation_budget=_require_positive_int(
            evaluation["simulation_budget"], "evaluation.simulation_budget"
        ),
        eval_transition_budget=_require_positive_int(
            evaluation["transition_budget"], "evaluation.transition_budget"
        ),
        eval_beam_depth=_require_positive_int(evaluation["beam_depth"], "evaluation.beam_depth"),
        eval_beam_width=_require_positive_int(evaluation["beam_width"], "evaluation.beam_width"),
        eval_max_decisions=_require_positive_int(
            evaluation["max_decisions"], "evaluation.max_decisions"
        ),
        eval_max_player_turns=_require_positive_int(
            evaluation["max_player_turns"], "evaluation.max_player_turns"
        ),
        minimum_held_out_roots=_require_positive_int(
            evaluation["minimum_roots"], "evaluation.minimum_roots"
        ),
        bootstrap_draws=_require_positive_int(
            metrics["cluster_bootstrap_draws"], "cluster_bootstrap_draws"
        ),
        bootstrap_seed=_require_int(metrics["cluster_bootstrap_seed"], "cluster_bootstrap_seed"),
    )


def _execution_from_plan(plan: LabelAbPlan) -> _Execution:
    require_production_plan(plan)
    return _execution_from_payload(plan)


def _test_execution(
    *,
    seed_prefix: str = "LABELABTEST",
    bootstrap_start: int = 0,
    bootstrap_count: int = 2,
    treatment_start: int = 2,
    treatment_count: int = 2,
    held_out_start: int = 4,
    held_out_count: int = 2,
    combat_depth: int = 1,
    max_run_steps: int = 128,
    beam_depth: int = 2,
    beam_width: int = 4,
    beam_transition_budget: int = 64,
    max_decisions: int = 4,
    max_player_turns: int = 3,
    simulation_budget: int = 2,
    puct_transition_budget: int = 2,
    training: TrainingConfig | None = None,
    minimum_held_out_roots: int = 1,
    bootstrap_draws: int = 8,
) -> _Execution:
    """Private in-memory fixture. Production disk and CLI never accept these knobs."""

    config = training
    if config is None:
        config = TrainingConfig(
            batch_size=1,
            total_steps=1,
            model_width=16,
            model_heads=4,
            model_layers=1,
            feedforward_width=32,
            minimum_roots=1,
            minimum_lineages=1,
        )
    return _Execution(
        behavior_seed=7,
        seed_prefix=seed_prefix,
        bootstrap_start=bootstrap_start,
        bootstrap_count=bootstrap_count,
        treatment_start=treatment_start,
        treatment_count=treatment_count,
        held_out_start=held_out_start,
        held_out_count=held_out_count,
        ascension=0,
        combat_depth=combat_depth,
        max_run_steps=max_run_steps,
        beam_depth=beam_depth,
        beam_width=beam_width,
        beam_transition_budget=beam_transition_budget,
        max_decisions=max_decisions,
        max_player_turns=max_player_turns,
        deduplicate_search_states=True,
        c_puct=1.5,
        simulation_budget=simulation_budget,
        puct_transition_budget=puct_transition_budget,
        training=config,
        evaluation_seed=20260903,
        eval_c_puct=1.5,
        eval_simulation_budget=simulation_budget,
        eval_transition_budget=puct_transition_budget,
        eval_beam_depth=beam_depth,
        eval_beam_width=beam_width,
        eval_max_decisions=max_decisions,
        eval_max_player_turns=max_player_turns,
        minimum_held_out_roots=minimum_held_out_roots,
        bootstrap_draws=bootstrap_draws,
        bootstrap_seed=20260903,
    )


def cohort_seeds(prefix: str, start: int, count: int) -> list[str]:
    if type(prefix) is not str or not prefix:
        raise TypeError("seed prefix must be a nonempty string")
    if type(start) is not int or start < 0:
        raise ValueError("cohort start must be a nonnegative integer")
    if type(count) is not int or count <= 0:
        raise ValueError("cohort count must be positive")
    return [f"{prefix}{index}" for index in range(start, start + count)]


def generate_label_ab_roots(
    output_dir: Path,
    seeds: Sequence[str],
    execution: _Execution,
) -> RootManifest:
    output = _require_unpublished_dir(output_dir)
    return generate_legal_roots(
        output,
        list(seeds),
        ascension=execution.ascension,
        max_run_steps=execution.max_run_steps,
        combat_depth=execution.combat_depth,
    )


def require_label_ab_cohorts(
    *,
    bootstrap: RootManifest,
    treatment: RootManifest,
    held_out: RootManifest,
    execution: _Execution,
) -> None:
    require_pairwise_disjoint_cohorts(
        (
            ("bootstrap", bootstrap),
            ("treatment", treatment),
            ("held_out", held_out),
        )
    )
    expected = (
        ("bootstrap", bootstrap, execution.bootstrap_start, execution.bootstrap_count),
        ("treatment", treatment, execution.treatment_start, execution.treatment_count),
        ("held_out", held_out, execution.held_out_start, execution.held_out_count),
    )
    for name, manifest, start, count in expected:
        seeds = tuple(cohort_seeds(execution.seed_prefix, start, count))
        if manifest.requested_seeds != tuple(sorted(seeds)):
            raise ValueError(f"{name} requested seeds do not match the predeclared cohort")
        if manifest.ascension != execution.ascension:
            raise ValueError(f"{name} ascension mismatch")
        if manifest.combat_depth != execution.combat_depth:
            raise ValueError(f"{name} combat depth mismatch")
        if manifest.max_run_steps != execution.max_run_steps:
            raise ValueError(f"{name} max_run_steps mismatch")
    train_roots = [root for root in bootstrap.roots if root.split == "train"]
    treatment_train = [root for root in treatment.roots if root.split == "train"]
    held_out_loadable = [root for root in held_out.roots if root.split in _LOADABLE_SPLITS]
    if len(train_roots) < execution.training.minimum_roots:
        raise ValueError(
            "bootstrap train yield is below the configured floor; abort, do not top up"
        )
    lineages = {lineage for root in train_roots for lineage in root.lineages}
    if len(lineages) < execution.training.minimum_lineages:
        raise ValueError(
            "bootstrap train lineages are below the configured floor; abort, do not top up"
        )
    if len(treatment_train) < execution.training.minimum_roots:
        raise ValueError(
            "treatment train yield is below the configured floor; abort, do not top up"
        )
    treatment_lineages = {lineage for root in treatment_train for lineage in root.lineages}
    if len(treatment_lineages) < execution.training.minimum_lineages:
        raise ValueError(
            "treatment train lineages are below the configured floor; abort, do not top up"
        )
    if len(held_out_loadable) < execution.minimum_held_out_roots:
        raise ValueError(
            "held-out loadable yield is below the configured floor; abort, do not top up"
        )


def behavior_policy_index(
    *,
    behavior_seed: int,
    root_id: str,
    decision_index: int,
    descriptors: Sequence[Mapping[str, object]],
) -> int:
    return random_policy_index(
        evaluation_seed=behavior_seed,
        root_id=root_id,
        accepted_decision_index=decision_index,
        descriptors=descriptors,
    )


def teacher_pair_id(*, root_id: str, decision_index: int, observation_digest: str) -> str:
    return sha256_bytes(
        canonical_bytes(
            {
                "kind": "paired_state_label_v1",
                "root_id": root_id,
                "decision_index": decision_index,
                "observation_digest": observation_digest,
            }
        )
    )


def _require_aligned_choice_rows(decision: Decision, raw_choices: object, label: str) -> None:
    if not isinstance(raw_choices, list) or len(raw_choices) != len(decision.actions):
        raise ValueError(f"{label} choice rows are not aligned with the public Decision")
    for action, raw_choice in zip(decision.actions, raw_choices, strict=True):
        if action.descriptor() != action_descriptor_from_payload(raw_choice):
            raise ValueError(f"{label} choice rows are not aligned with the public Decision")


def _query_beam_first_step(
    env: RunEnv,
    decision: Decision,
    execution: _Execution,
) -> tuple[tuple[str, str], int, tuple[int, ...]]:
    before = env.snapshot().hash
    payload = env.beam_clone_episode_payload(
        depth=execution.beam_depth,
        width=execution.beam_width,
        transition_budget=execution.beam_transition_budget,
        max_decisions=1,
        max_player_turns=execution.max_player_turns,
        deduplicate_search_states=execution.deduplicate_search_states,
    )
    if env.snapshot().hash != before:
        raise AuthoritativeRootMutationError("beam query mutated the behavior environment")
    if payload.get("schema_version") != 1:
        raise ValueError("unsupported native beam episode schema")
    teacher = (
        _require_string(payload.get("teacher_name"), "beam teacher_name"),
        _require_string(payload.get("teacher_version"), "beam teacher_version"),
    )
    if teacher[0] != BEAM_TEACHER_NAME:
        raise ValueError("native beam teacher identity is unsupported")
    steps = payload.get("steps")
    if type(steps) is not list or not steps:
        raise ValueError("beam query produced no public decision")
    step = _require_mapping(steps[0], "beam first step")
    _require_aligned_choice_rows(decision, step.get("choices"), "beam")
    selected = _require_int(step.get("selected_index"), "beam selected index")
    counts = tuple(
        _require_int(value, "beam visit count")
        for value in cast(list[object], step.get("teacher_visit_counts"))
    )
    if (
        len(counts) != len(decision.actions)
        or sum(counts) != 1
        or counts[selected] != 1
        or selected != first_argmax_visits(counts)
    ):
        raise ValueError("beam teacher labels must be one-hot at the chosen public row")
    return teacher, selected, counts


def _query_puct_step(
    env: RunEnv,
    decision: Decision,
    evaluator: Callable[[str], str],
    execution: _Execution,
    *,
    episode_root_max_hp: int,
    episode_root_gold: int,
) -> tuple[int, tuple[int, ...], float]:
    observation = decision.observation
    if not isinstance(observation, FairCombatObservation):
        raise TypeError("PUCT query requires a fair combat observation")
    before = env.snapshot().hash
    payload = puct_search_payload(
        env,
        evaluator,
        c_puct=execution.c_puct,
        simulation_budget=execution.simulation_budget,
        transition_budget=execution.puct_transition_budget,
        reward_config=execution.reward_config,
        episode_root_max_hp=episode_root_max_hp,
        episode_root_gold=episode_root_gold,
        leaf_cache="exact_state",
    )
    if env.snapshot().hash != before:
        raise AuthoritativeRootMutationError("PUCT query mutated the behavior environment")
    _require_aligned_choice_rows(decision, payload.get("choices"), "PUCT")
    selected = _require_int(payload.get("selected_index"), "PUCT selected index")
    counts = tuple(
        _require_int(value, "PUCT visit count")
        for value in cast(list[object], payload.get("visits"))
    )
    simulations = _require_int(payload.get("completed_simulations"), "completed simulations")
    transitions = _require_int(payload.get("transitions"), "transitions")
    if sum(counts) != simulations:
        raise ValueError("PUCT visit mass must equal completed simulations")
    if transitions > execution.puct_transition_budget or simulations > execution.simulation_budget:
        raise ValueError("PUCT query overshot its search budgets")
    if selected != first_argmax_visits(counts):
        raise ValueError("PUCT selected index is not the first visit-count argmax")
    root_mean = _require_float(payload.get("value"), "PUCT root value")
    if not -1.0 <= root_mean <= 1.0:
        raise ValueError("PUCT root value must be finite and in [-1, 1]")
    return selected, counts, root_mean


def _status_from_walk(
    *,
    combat_outcome: str | None,
    accepted_decisions: int,
    player_turns: int,
    max_decisions: int,
    max_player_turns: int,
) -> tuple[OutcomeStatus, str | None]:
    if combat_outcome is not None:
        if combat_outcome not in {"won", "lost", "escaped"}:
            raise ValueError(f"unknown combat outcome: {combat_outcome}")
        return cast(OutcomeStatus, combat_outcome), None
    if accepted_decisions >= max_decisions:
        return "truncated", "accepted_decisions"
    if player_turns > max_player_turns:
        return "truncated", "player_turns"
    raise ValueError("shared behavior walk ended without a combat outcome or truncation")


def _outcome_from_shared_walk(
    *,
    root_observation: FairCombatObservation,
    terminal_observation: FairCombatObservation | FairRunObservation,
    status: OutcomeStatus,
    accepted_decisions: int,
    player_turns: int,
    truncation_trigger: str | None,
) -> CombatOutcome:
    if isinstance(terminal_observation, FairCombatObservation):
        terminal_hp = terminal_observation.player.hp
        terminal_max_hp = terminal_observation.player.max_hp
        terminal_gold = terminal_observation.context.gold
        potion_slots = terminal_observation.potion_slots
    else:
        terminal_hp = terminal_observation.context.player_hp
        terminal_max_hp = terminal_observation.context.player_max_hp
        terminal_gold = terminal_observation.context.gold
        potion_slots = terminal_observation.context.potion_slots
    terminal = status in {"won", "lost", "escaped"}
    return CombatOutcome(
        status,
        terminal_hp,
        terminal_max_hp,
        terminal_hp - root_observation.player.hp,
        terminal_max_hp - root_observation.player.max_hp,
        terminal_gold - root_observation.context.gold,
        tuple(slot.content_key for slot in potion_slots),
        (),
        terminal,
        not terminal,
        accepted_decisions,
        player_turns,
        truncation_trigger,
    )


def _require_original_sidecar(decision: Decision, action: Action) -> None:
    if not any(candidate is action for candidate in decision.actions):
        raise ValueError("behavior policy must select an original public Action")


@dataclass(frozen=True, slots=True)
class _PairedDecision:
    observation: FairCombatObservation
    actions: tuple[ActionDescriptor, ...]
    observation_digest: str
    beam_index: int
    beam_counts: tuple[int, ...]
    beam_teacher: tuple[str, str]
    puct_index: int
    puct_counts: tuple[int, ...]
    puct_root_mean: float
    decision_index: int


def label_paired_root(
    env: RunEnv,
    *,
    root_id: str,
    split_group_id: str,
    repository: RepositoryVersion,
    root_manifest_digest: str,
    beam_search_config: dict[str, object],
    puct_search_config: dict[str, object],
    evaluator: Callable[[str], str],
    execution: _Execution,
) -> tuple[list[SymbolicTrainingRecord], list[SymbolicTrainingRecord], str]:
    """Query both teachers at shared states, then advance by hash-random sidecars."""

    before_root = env.snapshot().hash
    decision = env.decision()
    observation = decision.observation
    if not isinstance(observation, FairCombatObservation):
        raise TypeError("paired labeling requires a fair combat observation")
    if observation.phase in {"won", "lost"} or observation.player.hp <= 0:
        raise ValueError("terminal or post-combat root cannot produce training records")
    root_observation = observation
    episode_root_max_hp = observation.player.max_hp
    episode_root_gold = observation.context.gold
    steps: list[_PairedDecision] = []
    accepted_decisions = 0
    player_turns = 1
    combat_outcome: str | None = None
    beam_teacher: tuple[str, str] | None = None
    while True:
        if not isinstance(decision.observation, FairCombatObservation):
            raise TypeError("shared behavior walk left combat without a native combat outcome")
        if not decision.actions:
            raise ValueError("shared behavior walk reached an empty ongoing decision")
        query_hash = env.snapshot().hash
        teacher, beam_index, beam_counts = _query_beam_first_step(env, decision, execution)
        if beam_teacher is None:
            beam_teacher = teacher
        elif beam_teacher != teacher:
            raise ValueError("native beam teacher metadata changed within an episode")
        puct_index, puct_counts, puct_root_mean = _query_puct_step(
            env,
            decision,
            evaluator,
            execution,
            episode_root_max_hp=episode_root_max_hp,
            episode_root_gold=episode_root_gold,
        )
        if env.snapshot().hash != query_hash:
            raise AuthoritativeRootMutationError("teacher queries mutated the behavior environment")
        observation = decision.observation
        assert isinstance(observation, FairCombatObservation)
        actions = tuple(action.descriptor() for action in decision.actions)
        descriptors = canonical_public_action_descriptors(decision.actions)
        behavior_index = behavior_policy_index(
            behavior_seed=execution.behavior_seed,
            root_id=root_id,
            decision_index=accepted_decisions,
            descriptors=descriptors,
        )
        chosen = decision.actions[behavior_index]
        _require_original_sidecar(decision, chosen)
        steps.append(
            _PairedDecision(
                observation,
                actions,
                fair_observation_digest(observation),
                beam_index,
                beam_counts,
                teacher,
                puct_index,
                puct_counts,
                puct_root_mean,
                accepted_decisions,
            )
        )
        result = env.step(chosen)
        accepted_decisions += 1
        player_turns = player_turns + result.player_turn_advances
        if result.combat_outcome is not None:
            combat_outcome = result.combat_outcome
            terminal_observation = result.decision.observation
            if not isinstance(terminal_observation, (FairCombatObservation, FairRunObservation)):
                raise TypeError("shared walk terminal observation is not a fair observation")
            break
        if (
            accepted_decisions >= execution.max_decisions
            or player_turns > execution.max_player_turns
        ):
            terminal_observation = result.decision.observation
            if not isinstance(terminal_observation, FairCombatObservation):
                raise TypeError("shared walk truncated observation is not fair combat")
            break
        decision = result.decision
    if env.snapshot().hash == before_root and accepted_decisions:
        raise AuthoritativeRootMutationError("behavior walk failed to advance the environment")
    if not steps:
        raise ValueError("terminal or post-combat root cannot produce training records")
    status, trigger = _status_from_walk(
        combat_outcome=combat_outcome,
        accepted_decisions=accepted_decisions,
        player_turns=player_turns,
        max_decisions=execution.max_decisions,
        max_player_turns=execution.max_player_turns,
    )
    outcome = _outcome_from_shared_walk(
        root_observation=root_observation,
        terminal_observation=terminal_observation,
        status=status,
        accepted_decisions=accepted_decisions,
        player_turns=player_turns,
        truncation_trigger=trigger,
    )
    target = execution.reward_config.value(outcome)
    assert beam_teacher is not None
    beam_episode = canonical_episode_id(root_id, beam_search_config, execution.reward_config.digest)
    puct_episode = canonical_episode_id(root_id, puct_search_config, execution.reward_config.digest)
    beam_records: list[SymbolicTrainingRecord] = []
    puct_records: list[SymbolicTrainingRecord] = []
    for step in steps:
        pair_id = teacher_pair_id(
            root_id=root_id,
            decision_index=step.decision_index,
            observation_digest=step.observation_digest,
        )
        beam_records.append(
            SymbolicTrainingRecord.create(
                step.observation,
                step.actions,
                step.beam_index,
                step.actions[step.beam_index],
                step.beam_counts,
                target,
                COMBAT_PROXY_VALUE_TARGET_NAME,
                outcome,
                beam_teacher[0],
                beam_teacher[1],
                cast(dict[str, JsonValue], beam_search_config),
                root_id,
                split_group_id,
                pair_id,
                repository,
                step.observation_digest,
                RECORD_VERSION,
                root_manifest_digest,
                execution.reward_config.digest,
                _SOURCE_KIND,
                beam_episode,
                step.decision_index,
                target is not None,
                None,
            )
        )
        puct_records.append(
            SymbolicTrainingRecord.create(
                step.observation,
                step.actions,
                step.puct_index,
                step.actions[step.puct_index],
                step.puct_counts,
                target,
                COMBAT_PROXY_VALUE_TARGET_NAME,
                outcome,
                PUCT_TEACHER_NAME,
                PUCT_TEACHER_VERSION,
                cast(dict[str, JsonValue], puct_search_config),
                root_id,
                split_group_id,
                pair_id,
                repository,
                step.observation_digest,
                RECORD_VERSION,
                root_manifest_digest,
                execution.reward_config.digest,
                _SOURCE_KIND,
                puct_episode,
                step.decision_index,
                target is not None,
                step.puct_root_mean,
            )
        )
    return beam_records, puct_records, before_root


def _beam_search_config(execution: _Execution) -> dict[str, object]:
    payload: dict[str, object] = {
        "depth": execution.beam_depth,
        "width": execution.beam_width,
        "transition_budget": execution.beam_transition_budget,
        "max_decisions": execution.max_decisions,
        "max_player_turns": execution.max_player_turns,
        "deadline": None,
        "replan": "every_public_decision",
        "deduplicate_search_states": execution.deduplicate_search_states,
    }
    validate_beam_search_config(payload)
    return payload


def generate_paired_label_datasets(
    treatment_root_manifest_path: Path,
    beam_output_dir: Path,
    puct_output_dir: Path,
    teacher_checkpoint_path: Path,
    execution: _Execution,
    *,
    bootstrap_manifest: RootManifest,
    held_out_manifest: RootManifest,
) -> tuple[DatasetManifest, DatasetManifest]:
    beam_output = _require_unpublished_dir(beam_output_dir)
    puct_output = _require_unpublished_dir(puct_output_dir)
    treatment_path = _lexical_path(treatment_root_manifest_path)
    root_manifest = load_root_manifest(treatment_path)
    require_label_ab_cohorts(
        bootstrap=bootstrap_manifest,
        treatment=root_manifest,
        held_out=held_out_manifest,
        execution=execution,
    )
    roots = [root for root in root_manifest.roots if root.split == "train"]
    if not roots:
        raise ValueError("root manifest contains no train roots")
    repository = capture_repository_version(_package_repository_root())
    if repository != root_manifest.repository:
        raise ValueError(
            "package repository identity does not match the authenticated root manifest"
        )
    model, vocabularies, checkpoint_payload = _load_teacher_checkpoint(teacher_checkpoint_path)
    if checkpoint_payload["source_epoch_bundle_digest"] != root_manifest.source_epoch_bundle_digest:
        raise ValueError("PUCT teacher checkpoint source-epoch-bundle digest mismatch")
    if checkpoint_payload["root_manifest_digest"] != bootstrap_manifest.manifest_digest:
        raise ValueError("PUCT teacher must be trained on the consumed bootstrap cohort")
    if checkpoint_payload["root_manifest_digest"] == root_manifest.manifest_digest:
        raise ValueError("bootstrap teacher leaked treatment roots")
    if checkpoint_payload["cohort_digest"] == root_manifest.cohort_digest:
        raise ValueError("bootstrap teacher leaked the treatment cohort")
    beam_config = _beam_search_config(execution)
    puct_config = _puct_search_config(
        c_puct=execution.c_puct,
        simulation_budget=execution.simulation_budget,
        transition_budget=execution.puct_transition_budget,
        max_decisions=execution.max_decisions,
        max_player_turns=execution.max_player_turns,
        payload=checkpoint_payload,
    )
    evaluator = network_leaf_evaluator(model, vocabularies)
    beam_records: list[SymbolicTrainingRecord] = []
    puct_records: list[SymbolicTrainingRecord] = []
    used_roots: list[DatasetRootMembership] = []
    exclusions: list[DatasetExclusion] = []
    beam_teacher: tuple[str, str] | None = None
    for root in roots:
        try:
            env = _restore_labeled_root(treatment_path.parent, root)
            restored_hash = env.snapshot().hash
            root_beam, root_puct, query_hash = label_paired_root(
                env,
                root_id=root.root_id,
                split_group_id=root.split_group_id,
                repository=repository,
                root_manifest_digest=root_manifest.manifest_digest,
                beam_search_config=beam_config,
                puct_search_config=puct_config,
                evaluator=evaluator,
                execution=execution,
            )
            if query_hash != restored_hash:
                raise AuthoritativeRootMutationError(
                    f"paired labeling mutated restored root {root.root_id} before the walk"
                )
            if beam_teacher is None:
                beam_teacher = (root_beam[0].planner_name, root_beam[0].planner_version)
            elif beam_teacher != (root_beam[0].planner_name, root_beam[0].planner_version):
                raise ValueError("native teacher metadata changed within dataset")
        except AuthoritativeRootMutationError:
            raise
        except (
            AttributeError,
            IndexError,
            KeyError,
            OverflowError,
            RuntimeError,
            TypeError,
            ValueError,
        ) as error:
            detail = str(error).strip() or type(error).__name__
            exclusions.append(DatasetExclusion(root.root_id, _NATIVE_EPISODE_ERROR, detail))
            continue
        beam_records.extend(root_beam)
        puct_records.extend(root_puct)
        used_roots.append(
            DatasetRootMembership(root.root_id, root.split_group_id, root.split, root.lineages)
        )
    if exclusions:
        raise ValueError(
            "paired labeling is incomplete; abort rather than train on an intersection: "
            + ", ".join(exclusion.root_id for exclusion in exclusions)
        )
    if beam_teacher is None:
        raise RuntimeError(
            f"all {len(roots)} train roots failed native episode labeling; no dataset was published"
        )
    beam_manifest = _publish_dataset(
        beam_output,
        split="train",
        root_manifest=root_manifest,
        root_manifest_path=treatment_path,
        records=beam_records,
        used_roots=used_roots,
        exclusions=[],
        teacher=beam_teacher,
        search_config=beam_config,
        reward_config=execution.reward_config,
        repository=repository,
    )
    puct_manifest = _publish_dataset(
        puct_output,
        split="train",
        root_manifest=root_manifest,
        root_manifest_path=treatment_path,
        records=puct_records,
        used_roots=used_roots,
        exclusions=[],
        teacher=(PUCT_TEACHER_NAME, PUCT_TEACHER_VERSION),
        search_config=puct_config,
        reward_config=execution.reward_config,
        repository=repository,
    )
    require_paired_treatment_identity(
        beam_output / "dataset-manifest.json",
        puct_output / "dataset-manifest.json",
        bootstrap_manifest=bootstrap_manifest,
    )
    return beam_manifest, puct_manifest


def require_paired_treatment_identity(
    beam_manifest_path: Path,
    puct_manifest_path: Path,
    *,
    bootstrap_manifest: RootManifest | None = None,
) -> tuple[tuple[SymbolicTrainingRecord, ...], tuple[SymbolicTrainingRecord, ...]]:
    beam_manifest, beam_root, beam_records = load_dataset_manifest(
        _lexical_path(beam_manifest_path), requested_split="train"
    )
    puct_manifest, puct_root, puct_records = load_dataset_manifest(
        _lexical_path(puct_manifest_path), requested_split="train"
    )
    if beam_root.manifest_digest != puct_root.manifest_digest:
        raise ValueError("paired datasets do not share the treatment root manifest")
    if beam_manifest.root_manifest_digest != puct_manifest.root_manifest_digest:
        raise ValueError("paired datasets do not share root_manifest_digest")
    if beam_manifest.cohort_digest != puct_manifest.cohort_digest:
        raise ValueError("paired datasets do not share cohort_digest")
    if beam_manifest.exclusions or puct_manifest.exclusions:
        raise ValueError("paired labeling is incomplete; exclusions are not allowed")
    if beam_manifest.record_count != puct_manifest.record_count:
        raise ValueError("paired datasets do not have the same record count")
    if len(beam_records) != len(puct_records):
        raise ValueError("paired datasets do not have the same sample order length")
    if {membership.root_id for membership in beam_manifest.roots} != {
        membership.root_id for membership in puct_manifest.roots
    }:
        raise ValueError("paired datasets do not share root membership")
    if bootstrap_manifest is not None:
        if beam_root.manifest_digest == bootstrap_manifest.manifest_digest:
            raise ValueError("bootstrap cohort leaked into paired treatment datasets")
        if beam_manifest.cohort_digest == bootstrap_manifest.cohort_digest:
            raise ValueError("bootstrap cohort leaked into paired treatment datasets")
    for beam_record, puct_record in zip(beam_records, puct_records, strict=True):
        for field in _RECORD_FIELD_NAMES:
            left = getattr(beam_record, field)
            right = getattr(puct_record, field)
            if field in _PAIRED_ALLOWED_DIFF_FIELDS:
                continue
            if left != right:
                raise ValueError(f"paired records differ in shared field {field}")
        if beam_record.planner_name != BEAM_TEACHER_NAME:
            raise ValueError("beam paired records have the wrong teacher")
        if puct_record.planner_name != PUCT_TEACHER_NAME:
            raise ValueError("PUCT paired records have the wrong teacher")
        if beam_record.planner_version == puct_record.planner_version:
            raise ValueError("paired teacher versions must differ")
        if beam_record.search_config == puct_record.search_config:
            raise ValueError("paired search configs must differ")
        if beam_record.search_root_mean_value is not None:
            raise ValueError("beam records must not carry a PUCT search root-mean")
        if puct_record.search_root_mean_value is None:
            raise ValueError("PUCT search root-mean diagnostic must be present")
        if beam_record.teacher_pair_id is None or puct_record.teacher_pair_id is None:
            raise ValueError("paired records must set teacher_pair_id")
        if beam_record.record_id == puct_record.record_id:
            raise ValueError("paired teacher records must not share record IDs")
        if beam_record.episode_id == puct_record.episode_id:
            raise ValueError("paired teacher records must not share episode IDs")
    return beam_records, puct_records


def train_bootstrap_teacher(
    root_manifest_path: Path,
    dataset_dir: Path,
    checkpoint_path: Path,
    execution: _Execution,
) -> TrainingResult:
    published_dataset = _require_unpublished_dir(dataset_dir)
    generate_beam_dataset(
        _lexical_path(root_manifest_path),
        published_dataset,
        split="train",
        depth=execution.beam_depth,
        width=execution.beam_width,
        transition_budget=execution.beam_transition_budget,
        max_decisions=execution.max_decisions,
        max_player_turns=execution.max_player_turns,
        deduplicate_search_states=execution.deduplicate_search_states,
        reward_config=execution.reward_config,
    )
    return train_beam_clone(
        published_dataset / "dataset-manifest.json",
        checkpoint_path,
        execution.training,
    )


def train_matched_students(
    *,
    beam_dataset_manifest_path: Path,
    puct_dataset_manifest_path: Path,
    beam_checkpoint_path: Path,
    puct_checkpoint_path: Path,
    execution: _Execution,
    bootstrap_manifest: RootManifest,
) -> tuple[Vocabularies, Mapping[str, object], TrainingResult, TrainingResult]:
    beam_records, puct_records = require_paired_treatment_identity(
        beam_dataset_manifest_path,
        puct_dataset_manifest_path,
        bootstrap_manifest=bootstrap_manifest,
    )
    vocabularies = fit_union_vocabularies((beam_records, puct_records))
    initial_state = create_common_initial_model_state(vocabularies, execution.training)
    initial_digest = _model_state_digest(initial_state)
    beam_result = train_beam_clone(
        beam_dataset_manifest_path,
        beam_checkpoint_path,
        execution.training,
        vocabularies=vocabularies,
        initial_model_state=clone_model_state(initial_state),
    )
    puct_result = train_beam_clone(
        puct_dataset_manifest_path,
        puct_checkpoint_path,
        execution.training,
        vocabularies=vocabularies,
        initial_model_state=clone_model_state(initial_state),
    )
    beam_payload, _beam_config, _beam_digest = load_training_checkpoint(beam_checkpoint_path)
    puct_payload, _puct_config, _puct_digest = load_training_checkpoint(puct_checkpoint_path)
    if beam_payload["vocabulary_fingerprint"] != vocabularies.fingerprint:
        raise ValueError("beam student vocabulary fingerprint mismatch")
    if puct_payload["vocabulary_fingerprint"] != vocabularies.fingerprint:
        raise ValueError("PUCT student vocabulary fingerprint mismatch")
    if beam_payload["encoder_contract_digest"] != encoder_contract_digest(vocabularies):
        raise ValueError("beam student encoder contract mismatch")
    if puct_payload["encoder_contract_digest"] != encoder_contract_digest(vocabularies):
        raise ValueError("PUCT student encoder contract mismatch")
    if beam_payload["dataset_manifest_digest"] == puct_payload["dataset_manifest_digest"]:
        raise ValueError("matched students must bind distinct dataset manifests")
    if (
        beam_payload["teacher_search_contract_digest"]
        == puct_payload["teacher_search_contract_digest"]
    ):
        raise ValueError("matched students must bind distinct teacher/search contracts")
    beam_init = beam_payload["common_initial_model_state_digest"]
    puct_init = puct_payload["common_initial_model_state_digest"]
    if beam_init != initial_digest or puct_init != initial_digest:
        raise ValueError("student checkpoints do not attest the common initial model state")
    if beam_payload["root_manifest_digest"] == bootstrap_manifest.manifest_digest:
        raise ValueError("bootstrap cohort leaked into student training")
    if beam_payload["cohort_digest"] == bootstrap_manifest.cohort_digest:
        raise ValueError("bootstrap cohort leaked into student training")
    if puct_payload["root_manifest_digest"] != beam_payload["root_manifest_digest"]:
        raise ValueError("matched students must bind the same treatment root manifest")
    return vocabularies, initial_state, beam_result, puct_result


def _official_win(row: Mapping[str, object], arm: str) -> int:
    policies = _require_mapping(row["policies"], "per_root.policies")
    missing = [name for name in MATCHED_PUCT_REPORT_ARMS if name not in policies]
    if missing:
        raise ValueError(f"gameplay row is missing arm {missing[0]}")
    episode = _require_mapping(policies[arm], f"policies.{arm}")
    return 1 if episode.get("status") == "won" else 0


def _bootstrap_index(seed: int, draw: int, position: int, n: int) -> int:
    digest = sha256_bytes(canonical_bytes(["cluster_bootstrap_v1", seed, draw, position, n]))
    return int.from_bytes(bytes.fromhex(digest)[:8], "big") % n


def _percentile(sorted_values: Sequence[float], p: float) -> float:
    if not 0.0 <= p <= 1.0:
        raise ValueError("percentile must be in [0, 1]")
    if not sorted_values:
        raise ValueError("percentile of an empty series")
    index = p * (len(sorted_values) - 1)
    lo = math.floor(index)
    hi = math.ceil(index)
    if lo == hi:
        return float(sorted_values[lo])
    weight = index - lo
    return float(sorted_values[lo]) * (1.0 - weight) + float(sorted_values[hi]) * weight


def cluster_bootstrap_mean_delta(
    deltas: Sequence[float],
    *,
    draws: int,
    seed: int,
) -> dict[str, object]:
    if type(draws) is not int or draws <= 0:
        raise ValueError("bootstrap draws must be positive")
    if type(seed) is not int:
        raise TypeError("bootstrap seed must be an integer")
    n = len(deltas)
    if n < 1:
        raise ValueError("bootstrap requires at least one root")
    means: list[float] = []
    for draw in range(draws):
        total = 0.0
        for position in range(n):
            total += deltas[_bootstrap_index(seed, draw, position, n)]
        means.append(total / float(n))
    ordered = tuple(sorted(means))
    observed = sum(deltas) / float(n)
    return {
        "stream": "cluster_bootstrap_v1",
        "draws": draws,
        "seed": seed,
        "roots": n,
        "observed_delta": observed,
        "percentile_ci_95": [_percentile(ordered, 0.025), _percentile(ordered, 0.975)],
    }


def _pool_gameplay_reports(reports: Sequence[Mapping[str, object]]) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    seen: set[str] = set()
    for report in reports:
        per_root = report.get("per_root")
        if type(per_root) is not list:
            raise TypeError("gameplay per_root must be an array")
        for raw in cast(list[object], per_root):
            row = _require_mapping(raw, "per_root row")
            root_id = _require_string(row["root_id"], "root_id")
            if root_id in seen:
                raise ValueError(f"pooled gameplay has a duplicate root ID {root_id}")
            seen.add(root_id)
            rows.append(dict(row))
    rows.sort(key=lambda item: cast(str, item["root_id"]))
    return rows


def require_nonlearned_arm_identity(
    beam_rows: Sequence[Mapping[str, object]],
    puct_rows: Sequence[Mapping[str, object]],
) -> None:
    if len(beam_rows) != len(puct_rows):
        raise ValueError("student gameplay reports do not cover the same roots")
    for beam_row, puct_row in zip(beam_rows, puct_rows, strict=True):
        if beam_row["root_id"] != puct_row["root_id"]:
            raise ValueError("student gameplay root order diverged")
        beam_policies = _require_mapping(beam_row["policies"], "beam policies")
        puct_policies = _require_mapping(puct_row["policies"], "puct policies")
        for arm in MATCHED_PUCT_REPORT_ARMS:
            if arm not in beam_policies or arm not in puct_policies:
                raise ValueError(f"gameplay row is missing arm {arm}")
        for arm in NONLEARNED_ARMS:
            beam_episode = _require_mapping(beam_policies[arm], f"beam {arm}")
            puct_episode = _require_mapping(puct_policies[arm], f"puct {arm}")
            if canonical_bytes(beam_episode) != canonical_bytes(puct_episode):
                raise ValueError(f"nonlearned arm {arm} diverged across students")


def _paired_win_rate_delta(
    puct_rows: Sequence[Mapping[str, object]],
    beam_rows: Sequence[Mapping[str, object]],
    arm: str,
) -> tuple[float, tuple[float, ...]]:
    deltas: list[float] = []
    for puct_row, beam_row in zip(puct_rows, beam_rows, strict=True):
        deltas.append(float(_official_win(puct_row, arm) - _official_win(beam_row, arm)))
    observed = sum(deltas) / float(len(deltas))
    return observed, tuple(deltas)


def _require_gameplay_identities(
    reports: Sequence[Mapping[str, object]],
    *,
    checkpoint_digest: str,
    held_out_digest: str,
    label: str,
) -> None:
    for report in reports:
        if report.get("checkpoint_file_digest") != checkpoint_digest:
            raise ValueError(f"{label} gameplay checkpoint digest mismatch")
        if report.get("root_manifest_digest") != held_out_digest:
            raise ValueError(f"{label} gameplay root manifest digest mismatch")


def _result_from_reports(
    beam_reports: Sequence[Mapping[str, object]],
    puct_reports: Sequence[Mapping[str, object]],
    *,
    loadable_ids: Sequence[str],
    execution: _Execution,
    plan_digest: str,
    beam_checkpoint_digest: str,
    puct_checkpoint_digest: str,
    held_out_digest: str,
) -> dict[str, object]:
    digest = _require_digest(plan_digest, "plan_digest")
    _require_gameplay_identities(
        beam_reports,
        checkpoint_digest=beam_checkpoint_digest,
        held_out_digest=held_out_digest,
        label="beam",
    )
    _require_gameplay_identities(
        puct_reports,
        checkpoint_digest=puct_checkpoint_digest,
        held_out_digest=held_out_digest,
        label="puct",
    )
    beam_rows = _pool_gameplay_reports(beam_reports)
    puct_rows = _pool_gameplay_reports(puct_reports)
    expected = list(loadable_ids)
    if [row["root_id"] for row in beam_rows] != expected:
        raise ValueError("pooled held-out accounting is incomplete")
    if [row["root_id"] for row in puct_rows] != expected:
        raise ValueError("pooled held-out accounting is incomplete")
    require_nonlearned_arm_identity(beam_rows, puct_rows)
    network_delta, network_deltas = _paired_win_rate_delta(puct_rows, beam_rows, "network")
    puct_delta, puct_deltas = _paired_win_rate_delta(puct_rows, beam_rows, "network_puct")
    bootstrap = cluster_bootstrap_mean_delta(
        network_deltas,
        draws=execution.bootstrap_draws,
        seed=execution.bootstrap_seed,
    )
    unsigned: dict[str, object] = {
        "kind": RESULT_KIND,
        "schema_version": RESULT_SCHEMA_VERSION,
        "plan_digest": digest,
        "primary": {
            "name": "paired_official_greedy_network_win_rate_delta",
            "delta": network_delta,
            "roots": len(network_deltas),
            "note": "lost/escaped/truncated/error are nonwins; errors remain in the denominator",
        },
        "secondary": {
            "paired_network_puct_win_rate_delta": puct_delta,
            "network_puct_roots": len(puct_deltas),
        },
        "integrity": {
            "nonlearned_arms_identical": True,
            "nonlearned_arms": list(NONLEARNED_ARMS),
            "promotion_claim": False,
        },
        "bootstrap": bootstrap,
        "promotion_claim": False,
        "result_digest": "0" * 64,
    }
    digest_source = {key: unsigned[key] for key in _RESULT_KEYS if key != "result_digest"}
    unsigned["result_digest"] = _digest(digest_source)
    return parse_label_ab_result(unsigned).to_dict()


def assess_label_ab_students(
    *,
    held_out_root_manifest_path: Path,
    treatment_root_manifest_path: Path,
    beam_checkpoint_path: Path,
    puct_checkpoint_path: Path,
    authorization_path: Path,
    execution: _Execution,
    bootstrap_manifest: RootManifest,
    plan_digest: str,
) -> tuple[dict[str, object], list[dict[str, object]], list[dict[str, object]]]:
    digest = _require_digest(plan_digest, "plan_digest")
    held_out_path = _lexical_path(held_out_root_manifest_path)
    treatment_path = _lexical_path(treatment_root_manifest_path)
    held_out = load_root_manifest(held_out_path)
    treatment = load_root_manifest(treatment_path)
    require_label_ab_cohorts(
        bootstrap=bootstrap_manifest,
        treatment=treatment,
        held_out=held_out,
        execution=execution,
    )
    beam_payload, _beam_config, beam_digest = load_training_checkpoint(beam_checkpoint_path)
    puct_payload, _puct_config, puct_digest = load_training_checkpoint(puct_checkpoint_path)
    if beam_payload["root_manifest_digest"] != treatment.manifest_digest:
        raise ValueError("student checkpoint is not bound to the treatment cohort")
    if puct_payload["root_manifest_digest"] != treatment.manifest_digest:
        raise ValueError("student checkpoint is not bound to the treatment cohort")
    if beam_payload["root_manifest_digest"] == bootstrap_manifest.manifest_digest:
        raise ValueError("bootstrap cohort leaked into held-out assessment")
    if (
        beam_payload["common_initial_model_state_digest"]
        != puct_payload["common_initial_model_state_digest"]
    ):
        raise ValueError("assessed students do not share a common initial model state")
    loadable = [root for root in held_out.roots if root.split in _LOADABLE_SPLITS]
    if len(loadable) < execution.minimum_held_out_roots:
        raise ValueError("held-out loadable yield is below the configured floor")
    beam_reports: list[dict[str, object]] = []
    puct_reports: list[dict[str, object]] = []
    for split in ("train", "development"):
        if not any(root.split == split for root in held_out.roots):
            continue
        require_held_out_evaluation(
            training_root_manifest_digest=treatment.manifest_digest,
            training_cohort_digest=treatment.cohort_digest,
            evaluation_manifest=held_out,
            evaluation_root_manifest_path=held_out_path,
            evaluation_split=split,
            evaluation_seed=execution.evaluation_seed,
            requested_evaluator_names=MATCHED_PUCT_REPORT_ARMS,
            authorization_path=authorization_path,
            training_root_manifest_path=treatment_path,
        )
        beam_reports.append(
            evaluate_matched_puct_gameplay(
                held_out_path,
                beam_checkpoint_path,
                split=split,
                evaluation_seed=execution.evaluation_seed,
                authorization_path=authorization_path,
                training_root_manifest_path=treatment_path,
                c_puct=execution.eval_c_puct,
                simulation_budget=execution.eval_simulation_budget,
                transition_budget=execution.eval_transition_budget,
                beam_depth=execution.eval_beam_depth,
                beam_width=execution.eval_beam_width,
                max_decisions=execution.eval_max_decisions,
                max_player_turns=execution.eval_max_player_turns,
                deduplicate_search_states=execution.deduplicate_search_states,
            )
        )
        puct_reports.append(
            evaluate_matched_puct_gameplay(
                held_out_path,
                puct_checkpoint_path,
                split=split,
                evaluation_seed=execution.evaluation_seed,
                authorization_path=authorization_path,
                training_root_manifest_path=treatment_path,
                c_puct=execution.eval_c_puct,
                simulation_budget=execution.eval_simulation_budget,
                transition_budget=execution.eval_transition_budget,
                beam_depth=execution.eval_beam_depth,
                beam_width=execution.eval_beam_width,
                max_decisions=execution.eval_max_decisions,
                max_player_turns=execution.eval_max_player_turns,
                deduplicate_search_states=execution.deduplicate_search_states,
            )
        )
    loadable_ids = sorted(root.root_id for root in loadable)
    result = _result_from_reports(
        beam_reports,
        puct_reports,
        loadable_ids=loadable_ids,
        execution=execution,
        plan_digest=digest,
        beam_checkpoint_digest=beam_digest,
        puct_checkpoint_digest=puct_digest,
        held_out_digest=held_out.manifest_digest,
    )
    return result, beam_reports, puct_reports


@dataclass(frozen=True, slots=True)
class LabelAbResult:
    kind: str
    schema_version: int
    plan_digest: str
    primary: Mapping[str, object]
    secondary: Mapping[str, object]
    integrity: Mapping[str, object]
    bootstrap: Mapping[str, object]
    promotion_claim: bool
    result_digest: str

    def to_dict(self) -> dict[str, object]:
        thawed: dict[str, object] = {
            "kind": self.kind,
            "schema_version": self.schema_version,
            "plan_digest": self.plan_digest,
            "primary": _deep_thaw(self.primary),
            "secondary": _deep_thaw(self.secondary),
            "integrity": _deep_thaw(self.integrity),
            "bootstrap": _deep_thaw(self.bootstrap),
            "promotion_claim": self.promotion_claim,
            "result_digest": self.result_digest,
        }
        return thawed


def _require_finite_number(value: object, label: str) -> float:
    number = _require_float(value, label)
    return number


def parse_label_ab_result(payload: object) -> LabelAbResult:
    source = _require_mapping(payload, "label A/B result")
    _require_exact_keys(source, _RESULT_KEYS, "label A/B result")
    if source["kind"] != RESULT_KIND:
        raise ValueError("unsupported label A/B result kind")
    if _require_int(source["schema_version"], "schema_version") != RESULT_SCHEMA_VERSION:
        raise ValueError("unsupported label A/B result schema version")
    if _require_bool(source["promotion_claim"], "promotion_claim"):
        raise ValueError("result refuses promotion claims")
    plan_digest = _require_digest(source["plan_digest"], "plan_digest")
    primary = _require_mapping(source["primary"], "primary")
    _require_exact_keys(primary, _PRIMARY_KEYS, "primary")
    if primary["name"] != "paired_official_greedy_network_win_rate_delta":
        raise ValueError("primary metric name mismatch")
    _require_finite_number(primary["delta"], "primary.delta")
    _require_positive_int(primary["roots"], "primary.roots")
    note = _require_string(primary["note"], "primary.note")
    if "errors remain in the denominator" not in note:
        raise ValueError("primary note must keep errors in the denominator")
    secondary = _require_mapping(source["secondary"], "secondary")
    _require_exact_keys(secondary, _SECONDARY_KEYS, "secondary")
    _require_finite_number(
        secondary["paired_network_puct_win_rate_delta"],
        "secondary.paired_network_puct_win_rate_delta",
    )
    _require_positive_int(secondary["network_puct_roots"], "secondary.network_puct_roots")
    integrity = _require_mapping(source["integrity"], "integrity")
    _require_exact_keys(integrity, _INTEGRITY_KEYS, "integrity")
    if integrity["nonlearned_arms_identical"] is not True:
        raise ValueError("nonlearned arms must be identical")
    if _require_string_list(integrity["nonlearned_arms"], "integrity.nonlearned_arms") != (
        NONLEARNED_ARMS
    ):
        raise ValueError("nonlearned arm integrity list mismatch")
    if _require_bool(integrity["promotion_claim"], "integrity.promotion_claim"):
        raise ValueError("result integrity refuses promotion claims")
    bootstrap = _require_mapping(source["bootstrap"], "bootstrap")
    _require_exact_keys(bootstrap, _BOOTSTRAP_RESULT_KEYS, "bootstrap")
    if bootstrap["stream"] != "cluster_bootstrap_v1":
        raise ValueError("cluster bootstrap stream mismatch")
    _require_positive_int(bootstrap["draws"], "bootstrap.draws")
    _require_int(bootstrap["seed"], "bootstrap.seed")
    _require_positive_int(bootstrap["roots"], "bootstrap.roots")
    _require_finite_number(bootstrap["observed_delta"], "bootstrap.observed_delta")
    if primary["delta"] != bootstrap["observed_delta"]:
        raise ValueError("primary delta does not match bootstrap observed_delta")
    if primary["roots"] != bootstrap["roots"]:
        raise ValueError("primary roots do not match bootstrap roots")
    if primary["roots"] != secondary["network_puct_roots"]:
        raise ValueError("primary roots do not match secondary network_puct_roots")
    interval = bootstrap["percentile_ci_95"]
    if type(interval) is not list or len(cast(list[object], interval)) != 2:
        raise TypeError("bootstrap percentile_ci_95 must be a two-number array")
    lower = _require_finite_number(cast(list[object], interval)[0], "percentile_ci_95[0]")
    upper = _require_finite_number(cast(list[object], interval)[1], "percentile_ci_95[1]")
    if lower > upper:
        raise ValueError("bootstrap percentile interval is inverted")
    unsigned = dict(source)
    digest = _require_digest(unsigned.pop("result_digest"), "result_digest")
    expected = _digest(unsigned)
    if digest != expected:
        raise ValueError("label A/B result digest is invalid")
    frozen_primary = _deep_freeze(primary)
    frozen_secondary = _deep_freeze(secondary)
    frozen_integrity = _deep_freeze(integrity)
    frozen_bootstrap = _deep_freeze(bootstrap)
    return LabelAbResult(
        RESULT_KIND,
        RESULT_SCHEMA_VERSION,
        plan_digest,
        cast(Mapping[str, object], frozen_primary),
        cast(Mapping[str, object], frozen_secondary),
        cast(Mapping[str, object], frozen_integrity),
        cast(Mapping[str, object], frozen_bootstrap),
        False,
        digest,
    )


def write_held_out_authorization(
    path: Path,
    *,
    treatment_manifest: RootManifest,
    held_out_manifest: RootManifest,
    evaluation_seed: int,
) -> str:
    authorization = authorization_from_bindings(
        training_root_manifest_digest=treatment_manifest.manifest_digest,
        training_cohort_digest=treatment_manifest.cohort_digest,
        evaluation_root_manifest_digest=held_out_manifest.manifest_digest,
        evaluation_cohort_digest=held_out_manifest.cohort_digest,
        source_epoch_bundle_digest=treatment_manifest.source_epoch_bundle_digest,
        evaluation_seed=evaluation_seed,
        authorized_evaluator_names=sorted(MATCHED_PUCT_REPORT_ARMS),
    )
    if authorization.mandatory_disjointness_dimensions != MANDATORY_DISJOINTNESS_DIMENSIONS:
        raise ValueError("authorization disjointness dimensions mismatch")
    return write_authorization(path, authorization)


def _artifact_ref(role: str, path: Path, *, relative: bool, experiment_dir: Path) -> ArtifactRef:
    if relative:
        declared = path.as_posix()
        digest = sha256_bytes(_read_regular_file_bytes(experiment_dir / path))
    else:
        lexical = _lexical_path(path)
        declared = str(lexical)
        digest = sha256_bytes(_read_regular_file_bytes(lexical))
    return ArtifactRef(role, declared, digest)


def _copy_verified_checkpoint(source: Path, destination: Path) -> tuple[dict[str, object], str]:
    content, payload, _config, digest = load_training_checkpoint_bytes(_lexical_path(source))
    write_scientific_artifact(destination, content)
    dest_content, dest_payload, _dest_config, dest_digest = load_training_checkpoint_bytes(
        destination
    )
    if dest_digest != digest or dest_content != content:
        raise ValueError("copied checkpoint bytes diverged")
    if (
        dest_payload["common_initial_model_state_digest"]
        != payload["common_initial_model_state_digest"]
    ):
        raise ValueError("copied checkpoint initial digest diverged")
    return dest_payload, dest_digest


def _require_student_dataset_binding(
    payload: Mapping[str, object],
    dataset: DatasetManifest,
    treatment: RootManifest,
    label: str,
) -> None:
    if payload["dataset_manifest_digest"] != dataset.manifest_digest:
        raise ValueError(f"{label} checkpoint is not bound to its dataset manifest")
    if payload["dataset_shard_digest"] != dataset.shard_digest:
        raise ValueError(f"{label} checkpoint is not bound to its dataset shard")
    if payload["cohort_digest"] != treatment.cohort_digest:
        raise ValueError(f"{label} checkpoint is not bound to the treatment cohort")
    if payload["root_manifest_digest"] != treatment.manifest_digest:
        raise ValueError(f"{label} checkpoint is not bound to the treatment root manifest")
    if payload["source_epoch_bundle_digest"] != treatment.source_epoch_bundle_digest:
        raise ValueError(f"{label} checkpoint is not bound to the source-epoch bundle")


def _require_teacher_dataset_binding(
    payload: Mapping[str, object],
    file_digest: str,
    puct_dataset: DatasetManifest,
) -> None:
    search = _require_mapping(puct_dataset.search_config, "PUCT dataset search_config")
    bindings = {
        "checkpoint_file_digest": file_digest,
        "checkpoint_model_state_digest": _model_state_digest(payload["model_state"]),
        "checkpoint_config_digest": payload["config_digest"],
        "source_digest": payload["source_digest"],
        "runtime_identity_digest": payload["runtime_identity_digest"],
        "vocabulary_fingerprint": payload["vocabulary_fingerprint"],
        "encoder_contract_digest": payload["encoder_contract_digest"],
    }
    for field, expected in bindings.items():
        if search.get(field) != expected:
            raise ValueError(f"published teacher does not match PUCT label-time {field}")


def _require_authorization_bindings(
    path: Path,
    *,
    treatment: RootManifest,
    held_out: RootManifest,
    execution: _Execution,
) -> None:
    authorization = load_authorization(path)
    if authorization.training_root_manifest_digest != treatment.manifest_digest:
        raise ValueError("authorization is not bound to the treatment root manifest")
    if authorization.training_cohort_digest != treatment.cohort_digest:
        raise ValueError("authorization is not bound to the treatment cohort")
    if authorization.evaluation_root_manifest_digest != held_out.manifest_digest:
        raise ValueError("authorization is not bound to the held-out root manifest")
    if authorization.evaluation_cohort_digest != held_out.cohort_digest:
        raise ValueError("authorization is not bound to the held-out cohort")
    if authorization.source_epoch_bundle_digest != treatment.source_epoch_bundle_digest:
        raise ValueError("authorization is not bound to the source-epoch bundle")
    if authorization.evaluation_seed != execution.evaluation_seed:
        raise ValueError("authorization evaluation seed mismatch")
    if tuple(authorization.authorized_evaluator_names) != tuple(sorted(MATCHED_PUCT_REPORT_ARMS)):
        raise ValueError("authorization evaluator names mismatch")


def _verify_published_input_trees(
    experiment: Path,
) -> tuple[RootManifest, RootManifest, RootManifest, DatasetManifest, DatasetManifest]:
    bootstrap = load_root_manifest(experiment / "inputs/bootstrap/root-manifest.json")
    treatment = load_root_manifest(experiment / "inputs/treatment/root-manifest.json")
    held_out = load_root_manifest(experiment / "inputs/held-out/root-manifest.json")
    beam_dataset, beam_root, _beam_records = load_dataset_manifest(
        experiment / "inputs/beam-dataset/dataset-manifest.json", requested_split="train"
    )
    puct_dataset, puct_root, _puct_records = load_dataset_manifest(
        experiment / "inputs/puct-dataset/dataset-manifest.json", requested_split="train"
    )
    if beam_root.manifest_digest != treatment.manifest_digest:
        raise ValueError("published beam dataset root copy does not match treatment copy")
    if puct_root.manifest_digest != treatment.manifest_digest:
        raise ValueError("published PUCT dataset root copy does not match treatment copy")
    return bootstrap, treatment, held_out, beam_dataset, puct_dataset


def _publish_label_ab_experiment(
    experiment_dir: Path,
    *,
    plan: LabelAbPlan,
    plan_bytes: bytes,
    teacher_checkpoint: Path,
    beam_student_checkpoint: Path,
    puct_student_checkpoint: Path,
    union_vocabularies: Vocabularies,
    authorization_path: Path,
    result: Mapping[str, object],
    beam_gameplay_reports: Sequence[Mapping[str, object]],
    puct_gameplay_reports: Sequence[Mapping[str, object]],
    bootstrap_root_manifest: Path,
    treatment_root_manifest: Path,
    held_out_root_manifest: Path,
    beam_dataset_manifest: Path,
    puct_dataset_manifest: Path,
    execution: _Execution,
    version: RepositoryVersion,
) -> ExperimentPredeclaration:
    experiment = _require_unpublished_dir(experiment_dir)
    if plan_bytes != canonical_bytes(plan.to_dict()):
        raise ValueError("plan bytes do not match the loaded plan")
    parsed_result = parse_label_ab_result(result)
    if parsed_result.plan_digest != plan.plan_digest:
        raise ValueError("result plan_digest does not match the frozen plan")
    metrics = _require_mapping(plan.to_dict()["metrics"], "metrics")
    if parsed_result.bootstrap["draws"] != metrics["cluster_bootstrap_draws"]:
        raise ValueError("published bootstrap draws do not match the frozen plan")
    if parsed_result.bootstrap["seed"] != metrics["cluster_bootstrap_seed"]:
        raise ValueError("published bootstrap seed does not match the frozen plan")
    if parsed_result.bootstrap["stream"] != metrics["cluster_bootstrap_stream"]:
        raise ValueError("published bootstrap stream does not match the frozen plan")
    bootstrap_path = _lexical_path(bootstrap_root_manifest)
    treatment_path = _lexical_path(treatment_root_manifest)
    held_out_path = _lexical_path(held_out_root_manifest)
    beam_dataset_path = _lexical_path(beam_dataset_manifest)
    puct_dataset_path = _lexical_path(puct_dataset_manifest)
    bootstrap = load_root_manifest(bootstrap_path)
    treatment = load_root_manifest(treatment_path)
    held_out = load_root_manifest(held_out_path)
    require_label_ab_cohorts(
        bootstrap=bootstrap,
        treatment=treatment,
        held_out=held_out,
        execution=execution,
    )
    beam_records, puct_records = require_paired_treatment_identity(
        beam_dataset_path,
        puct_dataset_path,
        bootstrap_manifest=bootstrap,
    )
    recomputed_union = fit_union_vocabularies((beam_records, puct_records))
    if recomputed_union.fingerprint != union_vocabularies.fingerprint:
        raise ValueError("union vocabulary does not match paired treatment records")
    beam_dataset, beam_root, _loaded_beam = load_dataset_manifest(
        beam_dataset_path, requested_split="train"
    )
    puct_dataset, puct_root, _loaded_puct = load_dataset_manifest(
        puct_dataset_path, requested_split="train"
    )
    if beam_root.manifest_digest != treatment.manifest_digest:
        raise ValueError("beam dataset is not bound to the treatment root manifest")
    if puct_root.manifest_digest != treatment.manifest_digest:
        raise ValueError("PUCT dataset is not bound to the treatment root manifest")
    write_scientific_artifact(experiment / "plan.json", plan_bytes)
    teacher_payload, teacher_digest = _copy_verified_checkpoint(
        teacher_checkpoint, experiment / "teacher.pt"
    )
    beam_payload, beam_digest = _copy_verified_checkpoint(
        beam_student_checkpoint, experiment / "student-beam.pt"
    )
    puct_payload, puct_digest = _copy_verified_checkpoint(
        puct_student_checkpoint, experiment / "student-puct.pt"
    )
    if teacher_payload["root_manifest_digest"] != bootstrap.manifest_digest:
        raise ValueError("published teacher is not bound to the bootstrap cohort")
    _require_teacher_dataset_binding(teacher_payload, teacher_digest, puct_dataset)
    _require_student_dataset_binding(beam_payload, beam_dataset, treatment, "beam student")
    _require_student_dataset_binding(puct_payload, puct_dataset, treatment, "PUCT student")
    if beam_payload["config_digest"] != execution.training.digest:
        raise ValueError("published student training config does not match the frozen plan")
    if puct_payload["config_digest"] != execution.training.digest:
        raise ValueError("published student training config does not match the frozen plan")
    if beam_payload["vocabulary_fingerprint"] != recomputed_union.fingerprint:
        raise ValueError("published beam student vocabulary mismatch")
    if puct_payload["vocabulary_fingerprint"] != recomputed_union.fingerprint:
        raise ValueError("published PUCT student vocabulary mismatch")
    if (
        beam_payload["common_initial_model_state_digest"]
        != puct_payload["common_initial_model_state_digest"]
    ):
        raise ValueError("published students do not attest a shared common initial state")
    write_scientific_artifact(
        experiment / "authorization.json",
        _read_regular_file_bytes(_lexical_path(authorization_path)),
    )
    _require_authorization_bindings(
        experiment / "authorization.json",
        treatment=treatment,
        held_out=held_out,
        execution=execution,
    )
    vocab_payload = {
        "vocabularies": recomputed_union.to_dict(),
        "vocabulary_fingerprint": recomputed_union.fingerprint,
        "encoder_contract_digest": encoder_contract_digest(recomputed_union),
        "checkpoint_config_digest": beam_payload["config_digest"],
        "source_digest": beam_payload["source_digest"],
        "runtime_identity_digest": beam_payload["runtime_identity_digest"],
        "cohort_digest": treatment.cohort_digest,
        "root_manifest_digest": treatment.manifest_digest,
        "dataset_manifest_digest": beam_dataset.manifest_digest,
        "source_epoch_bundle_digest": treatment.source_epoch_bundle_digest,
    }
    write_scientific_artifact(
        experiment / "union-vocabularies.json", canonical_bytes(vocab_payload)
    )
    write_scientific_artifact(
        experiment / "beam_gameplay.json", canonical_bytes(list(beam_gameplay_reports))
    )
    write_scientific_artifact(
        experiment / "puct_gameplay.json", canonical_bytes(list(puct_gameplay_reports))
    )
    loadable_ids = sorted(root.root_id for root in held_out.roots if root.split in _LOADABLE_SPLITS)
    recomputed_result = _result_from_reports(
        beam_gameplay_reports,
        puct_gameplay_reports,
        loadable_ids=loadable_ids,
        execution=execution,
        plan_digest=plan.plan_digest,
        beam_checkpoint_digest=beam_digest,
        puct_checkpoint_digest=puct_digest,
        held_out_digest=held_out.manifest_digest,
    )
    if canonical_bytes(recomputed_result) != canonical_bytes(parsed_result.to_dict()):
        raise ValueError("result does not match recomputed gameplay statistics")
    write_scientific_artifact(experiment / "result.json", canonical_bytes(recomputed_result))
    tree_sources = (
        bootstrap_path.parent,
        treatment_path.parent,
        held_out_path.parent,
        beam_dataset_path.parent,
        puct_dataset_path.parent,
    )
    inputs = [
        _artifact_ref("plan", Path("plan.json"), relative=True, experiment_dir=experiment),
        _artifact_ref(
            "teacher_checkpoint", Path("teacher.pt"), relative=True, experiment_dir=experiment
        ),
    ]
    for (role, relative_dir, manifest_name), source in zip(
        _PUBLISHED_INPUT_TREES, tree_sources, strict=True
    ):
        _copy_tree_nofollow(source, experiment / relative_dir)
        inputs.extend(_copied_tree_refs(experiment, relative_dir, manifest_name, role))
    copied_inputs = _verify_published_input_trees(experiment)
    original_inputs = (bootstrap, treatment, held_out, beam_dataset, puct_dataset)
    labels = ("bootstrap", "treatment", "held-out", "beam dataset", "PUCT dataset")
    for label, original, copied in zip(labels, original_inputs, copied_inputs, strict=True):
        if original.manifest_digest != copied.manifest_digest:
            raise ValueError(f"published {label} input tree changed while being copied")
    outputs = [
        _artifact_ref(
            "checkpoint", Path("student-beam.pt"), relative=True, experiment_dir=experiment
        ),
        _artifact_ref(
            "student_puct_checkpoint",
            Path("student-puct.pt"),
            relative=True,
            experiment_dir=experiment,
        ),
        _artifact_ref(
            "authorization", Path("authorization.json"), relative=True, experiment_dir=experiment
        ),
        _artifact_ref(
            "union_vocabularies",
            Path("union-vocabularies.json"),
            relative=True,
            experiment_dir=experiment,
        ),
        _artifact_ref(
            "beam_gameplay", Path("beam_gameplay.json"), relative=True, experiment_dir=experiment
        ),
        _artifact_ref(
            "puct_gameplay", Path("puct_gameplay.json"), relative=True, experiment_dir=experiment
        ),
        _artifact_ref("result", Path("result.json"), relative=True, experiment_dir=experiment),
    ]
    environment: dict[str, str | None] = {
        "runtime_identity_digest": _require_digest(
            beam_payload["runtime_identity_digest"], "runtime_identity_digest"
        ),
        "encoder_contract_digest": encoder_contract_digest(recomputed_union),
        "vocabulary_fingerprint": recomputed_union.fingerprint,
        "source_digest": _require_digest(beam_payload["source_digest"], "source_digest"),
        "cohort_digest": treatment.cohort_digest,
        "root_manifest_digest": treatment.manifest_digest,
        "dataset_manifest_digest": beam_dataset.manifest_digest,
        "checkpoint_file_digest": beam_digest,
        "checkpoint_config_digest": _require_digest(
            beam_payload["config_digest"], "checkpoint_config_digest"
        ),
    }
    if set(environment) != _ENVIRONMENT_KEYS:
        raise ValueError("predeclaration environment keys mismatch")
    declared = ExperimentPredeclaration(
        PREDECLARATION_KIND,
        PREDECLARATION_SCHEMA_VERSION,
        PLAN_NAME,
        version.git_sha,
        True,
        False,
        MappingProxyType(
            {
                "sealed_test": False,
                "real_trace_audit": False,
                "development_only_for_assessment": True,
            }
        ),
        tuple(inputs),
        tuple(outputs),
        MappingProxyType(environment),
    )
    write_scientific_artifact(
        experiment / "predeclaration.json",
        canonical_bytes(declared.to_dict()),
    )
    write_artifact_inventory(experiment)
    if frozenset(_held_directory_names(experiment)) != _published_membership():
        raise ValueError("published experiment membership is not exact")
    verify_artifact_integrity(experiment)
    return load_experiment_predeclaration(experiment / "predeclaration.json")


def publish_label_ab_experiment(
    experiment_dir: Path,
    *,
    plan_path: Path,
    teacher_checkpoint: Path,
    beam_student_checkpoint: Path,
    puct_student_checkpoint: Path,
    union_vocabularies: Vocabularies,
    authorization_path: Path,
    result: Mapping[str, object],
    beam_gameplay_reports: Sequence[Mapping[str, object]],
    puct_gameplay_reports: Sequence[Mapping[str, object]],
    bootstrap_root_manifest: Path,
    treatment_root_manifest: Path,
    held_out_root_manifest: Path,
    beam_dataset_manifest: Path,
    puct_dataset_manifest: Path,
    repository: Path | None = None,
) -> ExperimentPredeclaration:
    plan, plan_bytes = _load_label_ab_plan_bytes(plan_path)
    version = _bind_live_source(plan, repository=repository)
    return _publish_label_ab_experiment(
        experiment_dir,
        plan=plan,
        plan_bytes=plan_bytes,
        teacher_checkpoint=teacher_checkpoint,
        beam_student_checkpoint=beam_student_checkpoint,
        puct_student_checkpoint=puct_student_checkpoint,
        union_vocabularies=union_vocabularies,
        authorization_path=authorization_path,
        result=result,
        beam_gameplay_reports=beam_gameplay_reports,
        puct_gameplay_reports=puct_gameplay_reports,
        bootstrap_root_manifest=bootstrap_root_manifest,
        treatment_root_manifest=treatment_root_manifest,
        held_out_root_manifest=held_out_root_manifest,
        beam_dataset_manifest=beam_dataset_manifest,
        puct_dataset_manifest=puct_dataset_manifest,
        execution=_execution_from_plan(plan),
        version=version,
    )


def _execute_label_ab(
    plan: LabelAbPlan,
    plan_bytes: bytes,
    work_dir: Path,
    experiment_dir: Path,
    *,
    repository: Path | None = None,
) -> dict[str, object]:
    if plan_bytes != canonical_bytes(plan.to_dict()):
        raise ValueError("plan bytes do not match the loaded plan")
    if _is_production_plan(plan):
        version = _bind_live_source(plan, repository=repository)
        execution = _execution_from_plan(plan)
    else:
        version = capture_repository_version(_repository_root(repository))
        if not version.clean:
            raise ValueError("label A/B requires a clean source worktree")
        if version.git_sha != plan.source_commit:
            raise ValueError("plan.source_commit does not match current clean HEAD")
        execution = _execution_from_payload(plan)
    work = _require_unpublished_dir(work_dir)
    experiment = experiment_dir
    bootstrap_dir = work / "bootstrap-roots"
    treatment_dir = work / "treatment-roots"
    held_out_dir = work / "held-out-roots"
    bootstrap = generate_label_ab_roots(
        bootstrap_dir,
        cohort_seeds(execution.seed_prefix, execution.bootstrap_start, execution.bootstrap_count),
        execution,
    )
    treatment = generate_label_ab_roots(
        treatment_dir,
        cohort_seeds(execution.seed_prefix, execution.treatment_start, execution.treatment_count),
        execution,
    )
    held_out = generate_label_ab_roots(
        held_out_dir,
        cohort_seeds(execution.seed_prefix, execution.held_out_start, execution.held_out_count),
        execution,
    )
    require_label_ab_cohorts(
        bootstrap=bootstrap,
        treatment=treatment,
        held_out=held_out,
        execution=execution,
    )
    teacher_dataset = work / "bootstrap-beam"
    teacher_checkpoint = work / "teacher.pt"
    train_bootstrap_teacher(
        bootstrap_dir / "root-manifest.json",
        teacher_dataset,
        teacher_checkpoint,
        execution,
    )
    beam_dir = work / "treatment-beam"
    puct_dir = work / "treatment-puct"
    generate_paired_label_datasets(
        treatment_dir / "root-manifest.json",
        beam_dir,
        puct_dir,
        teacher_checkpoint,
        execution,
        bootstrap_manifest=bootstrap,
        held_out_manifest=held_out,
    )
    beam_ckpt = work / "student-beam.pt"
    puct_ckpt = work / "student-puct.pt"
    union, _initial, _beam_result, _puct_result = train_matched_students(
        beam_dataset_manifest_path=beam_dir / "dataset-manifest.json",
        puct_dataset_manifest_path=puct_dir / "dataset-manifest.json",
        beam_checkpoint_path=beam_ckpt,
        puct_checkpoint_path=puct_ckpt,
        execution=execution,
        bootstrap_manifest=bootstrap,
    )
    auth = work / "authorization.json"
    write_held_out_authorization(
        auth,
        treatment_manifest=treatment,
        held_out_manifest=held_out,
        evaluation_seed=execution.evaluation_seed,
    )
    result, beam_reports, puct_reports = assess_label_ab_students(
        held_out_root_manifest_path=held_out_dir / "root-manifest.json",
        treatment_root_manifest_path=treatment_dir / "root-manifest.json",
        beam_checkpoint_path=beam_ckpt,
        puct_checkpoint_path=puct_ckpt,
        authorization_path=auth,
        execution=execution,
        bootstrap_manifest=bootstrap,
        plan_digest=plan.plan_digest,
    )
    _publish_label_ab_experiment(
        experiment,
        plan=plan,
        plan_bytes=plan_bytes,
        teacher_checkpoint=teacher_checkpoint,
        beam_student_checkpoint=beam_ckpt,
        puct_student_checkpoint=puct_ckpt,
        union_vocabularies=union,
        authorization_path=auth,
        result=result,
        beam_gameplay_reports=beam_reports,
        puct_gameplay_reports=puct_reports,
        bootstrap_root_manifest=bootstrap_dir / "root-manifest.json",
        treatment_root_manifest=treatment_dir / "root-manifest.json",
        held_out_root_manifest=held_out_dir / "root-manifest.json",
        beam_dataset_manifest=beam_dir / "dataset-manifest.json",
        puct_dataset_manifest=puct_dir / "dataset-manifest.json",
        execution=execution,
        version=version,
    )
    return {
        "plan_digest": plan.plan_digest,
        "result_digest": result["result_digest"],
        "experiment_dir": str(experiment),
        "promotion_claim": False,
    }


def run_label_ab_experiment(
    plan_path: Path,
    work_dir: Path,
    experiment_dir: Path,
    *,
    repository: Path | None = None,
) -> dict[str, object]:
    """Create cohorts, train, label, assess, and publish one production experiment."""

    plan, plan_bytes = _load_label_ab_plan_bytes(plan_path)
    _bind_live_source(plan, repository=repository)
    return _execute_label_ab(
        plan,
        plan_bytes,
        work_dir,
        experiment_dir,
        repository=repository,
    )


def rerun_label_ab_experiment(
    plan_path: Path,
    work_dir: Path,
    experiment_dir: Path,
    *,
    reference: Path,
    repository: Path | None = None,
) -> dict[str, object]:
    """Re-execute into fresh unpublished dirs, then compare designated published bytes."""

    run_label_ab_experiment(
        plan_path,
        work_dir,
        experiment_dir,
        repository=repository,
    )
    return verify_label_ab_rerun(reference=reference, candidate=experiment_dir)


def verify_label_ab_rerun(*, reference: Path, candidate: Path) -> dict[str, object]:
    """Compare designated published bytes. This retrains nothing.

    ``reproduce_experiment`` remains identity verification of a published
    directory against the current checkout and must not be described as a
    retraining driver.
    """

    left_root = _lexical_path(reference)
    right_root = _lexical_path(candidate)
    left_names = frozenset(_held_directory_names(left_root))
    right_names = frozenset(_held_directory_names(right_root))
    expected = _published_membership()
    mismatches: list[str] = []
    compared: list[dict[str, object]] = []
    extras = sorted((left_names | right_names) - expected)
    missing = sorted(expected - left_names) + sorted(expected - right_names)
    if extras or missing:
        mismatches.extend(f"membership:{name}" for name in extras + missing)
    for relative in DESIGNATED_RERUN_ARTIFACTS:
        if relative not in left_names or relative not in right_names:
            continue
        left = _read_regular_file_bytes(left_root / relative)
        right = _read_regular_file_bytes(right_root / relative)
        left_digest = sha256_bytes(left)
        right_digest = sha256_bytes(right)
        compared.append({"path": relative, "reference": left_digest, "candidate": right_digest})
        if left != right:
            mismatches.append(relative)
    if PUBLISHED_INPUT_DIR in left_names and PUBLISHED_INPUT_DIR in right_names:
        left_inputs = _scientific_file_digests(left_root / PUBLISHED_INPUT_DIR)
        right_inputs = _scientific_file_digests(right_root / PUBLISHED_INPUT_DIR)
        if set(left_inputs) != set(right_inputs):
            mismatches.append("membership:inputs")
        for relative in sorted(set(left_inputs) | set(right_inputs)):
            left_digest = left_inputs.get(relative)
            right_digest = right_inputs.get(relative)
            compared.append(
                {
                    "path": f"{PUBLISHED_INPUT_DIR}/{relative}",
                    "reference": left_digest,
                    "candidate": right_digest,
                }
            )
            if left_digest != right_digest:
                mismatches.append(f"{PUBLISHED_INPUT_DIR}/{relative}")
    report = {
        "kind": "beam_puct_paired_label_ab_rerun_report_v1",
        "ok": not mismatches,
        "compared": compared,
        "mismatches": mismatches,
        "reproduce_experiment_is_identity_verification": True,
    }
    report["report_digest"] = _digest(report)
    return report


def identity_verify_experiment(experiment_dir: Path, *, repository: Path) -> dict[str, object]:
    """Identity-verify a published experiment. This retrains nothing."""

    experiment = _lexical_path(experiment_dir)
    report = reproduce_experiment(experiment, repository=_repository_root(repository))
    plan = load_label_ab_plan(experiment / "plan.json")
    _bind_live_source(plan, repository=repository)
    result = parse_label_ab_result(json.loads(_read_regular_file_bytes(experiment / "result.json")))
    if result.plan_digest != plan.plan_digest:
        raise ValueError("published result plan_digest does not match the frozen plan")
    predeclaration = load_experiment_predeclaration(experiment / "predeclaration.json")
    if predeclaration.source_commit != plan.source_commit:
        raise ValueError("predeclaration source_commit does not match the frozen plan")
    for ref in predeclaration.inputs:
        if Path(ref.path).is_absolute():
            raise ValueError("published input paths must be experiment-relative")
    bootstrap, treatment, held_out, beam_dataset, puct_dataset = _verify_published_input_trees(
        experiment
    )
    beam_payload, _beam_config, beam_digest = load_training_checkpoint(
        experiment / "student-beam.pt"
    )
    puct_payload, _puct_config, puct_digest = load_training_checkpoint(
        experiment / "student-puct.pt"
    )
    teacher_payload, _teacher_config, teacher_digest = load_training_checkpoint(
        experiment / "teacher.pt"
    )
    require_paired_treatment_identity(
        experiment / "inputs/beam-dataset/dataset-manifest.json",
        experiment / "inputs/puct-dataset/dataset-manifest.json",
        bootstrap_manifest=bootstrap,
    )
    execution = _execution_from_plan(plan)
    _require_student_dataset_binding(beam_payload, beam_dataset, treatment, "beam student")
    _require_student_dataset_binding(puct_payload, puct_dataset, treatment, "PUCT student")
    if teacher_payload["root_manifest_digest"] != bootstrap.manifest_digest:
        raise ValueError("published teacher is not bound to the bootstrap cohort")
    _require_teacher_dataset_binding(teacher_payload, teacher_digest, puct_dataset)
    _require_authorization_bindings(
        experiment / "authorization.json",
        treatment=treatment,
        held_out=held_out,
        execution=execution,
    )
    beam_gameplay = json.loads(_read_regular_file_bytes(experiment / "beam_gameplay.json"))
    puct_gameplay = json.loads(_read_regular_file_bytes(experiment / "puct_gameplay.json"))
    if type(beam_gameplay) is not list or type(puct_gameplay) is not list:
        raise TypeError("published gameplay reports must be arrays")
    loadable_ids = sorted(root.root_id for root in held_out.roots if root.split in _LOADABLE_SPLITS)
    recomputed = _result_from_reports(
        cast(list[Mapping[str, object]], beam_gameplay),
        cast(list[Mapping[str, object]], puct_gameplay),
        loadable_ids=loadable_ids,
        execution=execution,
        plan_digest=plan.plan_digest,
        beam_checkpoint_digest=beam_digest,
        puct_checkpoint_digest=puct_digest,
        held_out_digest=held_out.manifest_digest,
    )
    if canonical_bytes(recomputed) != canonical_bytes(result.to_dict()):
        raise ValueError("published result does not match recomputed gameplay statistics")
    if frozenset(_held_directory_names(experiment)) != _published_membership():
        raise ValueError("published experiment membership is not exact")
    full = {
        **report,
        "plan_digest": plan.plan_digest,
        "result_digest": result.result_digest,
        "reproduce_experiment_is_identity_verification": True,
        "retrained": False,
    }
    full.pop("report_digest", None)
    full["report_digest"] = sha256_bytes(canonical_bytes(full))
    return full
