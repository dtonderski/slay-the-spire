"""Explicit private environmental inputs for simulator-only training.

Allocation is independent of scheduling, policy samplers and gameplay streams.
The master seed is an experiment input; it is not inferred from observations or
run outcomes. Fixed validation episodes receive the same environmental seed at
every evaluation. Neither seed enters observations, features or policy RNGs.
"""

import hashlib

ENVIRONMENT_PROTOCOL = "libgdx_training_environment_sha256_episode_v1"


def episode_environment_seed(master_seed: int, run_seed: str) -> int:
    if (
        isinstance(master_seed, bool)
        or not isinstance(master_seed, int)
        or not 0 <= master_seed < 2**64
    ):
        raise ValueError("Training environment seed must be an unsigned 64-bit integer")
    if (
        not isinstance(run_seed, str)
        or not run_seed.isascii()
        or not run_seed.isdecimal()
        or str(int(run_seed)) != run_seed
        or not 0 <= int(run_seed) < 2**63
    ):
        raise ValueError("Run seed must be a canonical decimal string in [0, 2**63)")
    payload = (
        ENVIRONMENT_PROTOCOL.encode("ascii")
        + b"\0"
        + master_seed.to_bytes(8, "big")
        + int(run_seed).to_bytes(8, "big")
    )
    return int.from_bytes(hashlib.sha256(payload).digest()[:8], "big")


def journal_environment_seed(setup: dict) -> int | None:
    """Validate immutable journal inputs; missing means the legacy strict profile."""
    seed = setup.get("training_rng_seed")
    if seed is not None and (
        isinstance(seed, bool) or not isinstance(seed, int) or not 0 <= seed < 2**64
    ):
        raise ValueError(
            "Journal training_rng_seed must be null or an unsigned 64-bit integer"
        )
    return seed
