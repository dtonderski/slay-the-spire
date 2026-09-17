"""Read-only public transport buffers. Feature definitions stay in their encoders."""

from typing import Any

import numpy as np
import torch
from torch import Tensor


class NumericBatch:
    def __init__(self, payload: tuple) -> None:
        version, self.symbols, tables, self.actions, self.model_rows = payload
        if version != 1:
            raise ValueError(f"Unsupported numeric transport: {version}")
        self.tables = {
            name: np.frombuffer(data, dtype=np.int64).reshape(-1, width) for name, (width, data) in tables.items()
        }
        self.size = len(self.model_rows)

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
        # Transport-local dictionary indices never enter the model as numbers.
        lookup = np.array([vocabulary.get(None, -1), *(vocabulary.get(key, -1) for key in self.symbols)])
        mapped = lookup[values + 1]
        if np.any(mapped < 0):
            raise ValueError("Public categorical value is absent from encoder vocabulary")
        return mapped

    def lengths(self, table: np.ndarray) -> list[int]:
        return np.bincount(table[:, 0], minlength=self.size).tolist()

    def powers(self, name: str, count: int, vocabulary: dict[Any, int]) -> np.ndarray:
        rows = self.table(name, 3)
        output = np.zeros((count, len(vocabulary)), dtype=np.float64)
        output[rows[:, 0], self.codes(rows[:, 1], vocabulary)] = rows[:, 2]
        return output


def tensor(reference: Tensor, values: np.ndarray, *, integer: bool = False) -> Tensor:
    """Copy a complete numeric array once; never expose immutable bytes as writable tensors."""
    return torch.tensor(values, dtype=torch.long if integer else reference.dtype, device=reference.device)
