"""Read-only public transport buffers. Feature definitions stay in their encoders."""

import functools
from typing import Any

import numpy as np
import torch
from torch import Tensor

NUMERIC_VERSION = 3
CONTENT_VOCABULARY_VERSION = 1
ACTION_ROW_WIDTH = 12
ACTION_OWNER = 0
ACTION_LEGAL_INDEX = 1
ACTION_KIND = 2
ACTION_HAND = 3
ACTION_POTION = 4
ACTION_OPTION = 5
ACTION_TARGET = 6
ACTION_CARD = 7
ACTION_NODE = 8
ACTION_REWARD = 9
ACTION_SHOP = 10
ACTION_REVISION = 11
# Forward-local candidate columns. Owner here is the observation index, not the batch state index.
CANDIDATE_WIDTH = 6
CANDIDATE_OWNER = 0
CANDIDATE_KIND = 1
CANDIDATE_HAND = 2
CANDIDATE_POTION = 3
CANDIDATE_OPTION = 4
CANDIDATE_TARGET = 5


class NumericBatch:
    def __init__(self, payload: tuple) -> None:
        version, self.symbols, tables, self.model_rows = payload
        if version != NUMERIC_VERSION:
            raise ValueError(f"Unsupported numeric transport: {version}")
        self.tables = {
            name: np.frombuffer(data, dtype=np.int64).reshape(-1, width) for name, (width, data) in tables.items()
        }
        self.size = len(self.model_rows)
        self.action_rows = self.table("action_rows", ACTION_ROW_WIDTH)
        # Lookups are local to this batch's symbol table. Vocabulary identity
        # selects the catalog; symbol positions are never assumed stable across batches.
        self._code_lookups: dict[int, np.ndarray] = {}
        self._code_vocabularies: list[dict[Any, int]] = []

    def __len__(self) -> int:
        return self.size

    def table(self, name: str, width: int) -> np.ndarray:
        value = self.tables.get(name)
        if value is None:
            return np.empty((0, width), dtype=np.int64)
        if value.shape[1] != width:
            raise ValueError(f"Invalid column count for {name}")
        return value

    def codes(self, values: np.ndarray, vocabulary: dict[Any, int]) -> np.ndarray:
        """Map this batch's symbol ids through one catalog. Ids are not model features."""
        key = id(vocabulary)
        lookup = self._code_lookups.get(key)
        if lookup is None:
            lookup = np.empty(len(self.symbols) + 1, dtype=np.int64)
            lookup[0] = vocabulary.get(None, -1)
            for index, symbol in enumerate(self.symbols, start=1):
                lookup[index] = vocabulary.get(symbol, -1)
            self._code_lookups[key] = lookup
            self._code_vocabularies.append(vocabulary)
        mapped = lookup[values + 1]
        if np.any(mapped < 0):
            raise ValueError("Public categorical value is absent from encoder vocabulary")
        return mapped

    def lengths(self, table: np.ndarray) -> list[int]:
        return np.bincount(table[:, 0], minlength=self.size).tolist()

    def powers(self, name: str, count: int, vocabulary: dict[Any, int]) -> np.ndarray:
        """Scatter catalog ids. ``vocabulary`` supplies the width; ids are not symbol positions."""
        rows = self.table(name, 3)
        if len(rows) and (int(rows[:, 1].min()) < 0 or int(rows[:, 1].max()) >= len(vocabulary)):
            raise ValueError("Power id is outside content vocabulary v1")
        output = np.zeros((count, len(vocabulary)), dtype=np.float64)
        if len(rows):
            output[rows[:, 0], rows[:, 1]] = rows[:, 2]
        return output


def upload(values: np.ndarray, dtype: torch.dtype, device: torch.device) -> Tensor:
    """Copy a complete numeric array once; never expose immutable bytes as writable tensors.

    CUDA copies are staged in the caching pinned-host allocator and issued without
    blocking the host, so model inputs do not wait for queued GPU work. The allocator
    keeps each staging block until its copy has finished. Values and casts match
    ``torch.tensor(values, dtype=dtype)``.
    """
    if device.type != "cuda":
        return torch.tensor(values, dtype=dtype, device=device)
    array = np.asarray(values)
    if not _has_numpy_view(dtype):
        # e.g. bfloat16: cast on the host, then pin that copy; still a non-blocking upload.
        return torch.tensor(array, dtype=dtype).pin_memory().to(device, non_blocking=True)
    staged = torch.empty(array.shape, dtype=dtype, pin_memory=True)
    staged.numpy()[...] = array
    return staged.to(device, non_blocking=True)


@functools.cache
def _has_numpy_view(dtype: torch.dtype) -> bool:
    """Whether ``Tensor.numpy()`` works for ``dtype``; NumPy has no bfloat16, for example."""
    try:
        torch.empty(0, dtype=dtype).numpy()
    except TypeError:
        return False
    return True


def tensor(reference: Tensor, values: np.ndarray, *, integer: bool = False) -> Tensor:
    """Upload ``values`` to ``reference``'s device, as integers or in its dtype."""
    return upload(values, torch.long if integer else reference.dtype, reference.device)
