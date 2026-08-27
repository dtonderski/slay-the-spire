"""Optional PyTorch helpers for fair combat learning.

Import this subpackage explicitly; the base :mod:`sts_sim` package has no
PyTorch import or runtime dependency.
"""

from .diagnostics import TeacherConflictGroup, teacher_conflict_report
from .model import (
    CheckpointSourceMismatchWarning,
    CombatModelConfig,
    FairCombatPolicyValueNet,
    LoadedCheckpoint,
    PolicyValueOutput,
    load_checkpoint,
    policy_value_loss,
    save_checkpoint,
)
from .provenance import RepositoryVersion, capture_repository_version, file_digest
from .records import (
    BatchedTrainingExamples,
    CombatOutcome,
    CounterChange,
    SymbolicCombatDataset,
    SymbolicTrainingRecord,
    TensorizedTrainingExample,
    collate_training_examples,
    fair_observation_digest,
    read_jsonl,
    write_jsonl,
)
from .rollout import (
    CombatRolloutResult,
    RolloutDistribution,
    rollout_model_combat,
    summarize_rollouts,
)
from .tensor import (
    CATEGORY_NAMESPACES,
    FIELD_COVERAGE,
    SCALAR_INDEX,
    SCALAR_NAMES,
    BatchedCombatDecision,
    FrozenVocabulary,
    TensorizedCombatDecision,
    Vocabularies,
    VocabularyBuilder,
    collate_combat_tensors,
    field_coverage_mismatches,
    tensorize_combat,
)

__all__ = [
    "CATEGORY_NAMESPACES",
    "FIELD_COVERAGE",
    "SCALAR_INDEX",
    "SCALAR_NAMES",
    "BatchedCombatDecision",
    "BatchedTrainingExamples",
    "CheckpointSourceMismatchWarning",
    "CombatModelConfig",
    "CombatOutcome",
    "CombatRolloutResult",
    "CounterChange",
    "FairCombatPolicyValueNet",
    "FrozenVocabulary",
    "LoadedCheckpoint",
    "PolicyValueOutput",
    "RepositoryVersion",
    "RolloutDistribution",
    "SymbolicCombatDataset",
    "SymbolicTrainingRecord",
    "TeacherConflictGroup",
    "TensorizedCombatDecision",
    "TensorizedTrainingExample",
    "Vocabularies",
    "VocabularyBuilder",
    "capture_repository_version",
    "collate_combat_tensors",
    "collate_training_examples",
    "fair_observation_digest",
    "field_coverage_mismatches",
    "file_digest",
    "load_checkpoint",
    "policy_value_loss",
    "read_jsonl",
    "rollout_model_combat",
    "save_checkpoint",
    "summarize_rollouts",
    "teacher_conflict_report",
    "tensorize_combat",
    "write_jsonl",
]
