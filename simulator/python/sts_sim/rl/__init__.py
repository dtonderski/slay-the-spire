"""Optional PyTorch helpers for fair combat learning.

Import this subpackage explicitly; the base :mod:`sts_sim` package has no
PyTorch import or runtime dependency.
"""

from .tensor import (
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
    "FIELD_COVERAGE",
    "SCALAR_INDEX",
    "SCALAR_NAMES",
    "BatchedCombatDecision",
    "FrozenVocabulary",
    "TensorizedCombatDecision",
    "Vocabularies",
    "VocabularyBuilder",
    "collate_combat_tensors",
    "field_coverage_mismatches",
    "tensorize_combat",
]
