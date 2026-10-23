"""Declared public-only exploration experiments, never simulator rule changes."""

from sts_sim import Observation, RelicKey

from run_training.contracts import PolicyAction
from run_training.restrictions import checked_eligibility

EXPLORATION_PROTOCOL = "public_fullhp_rest_mask_and_rest_sapphire_temperature_v1"
EXPLORATION_PROFILES = ("none", "fullhp", "rest_keys_t4", "rest_keys_t8")


def exploration_controls(
    observation: Observation,
    actions: tuple[PolicyAction, ...],
    eligibility: tuple[bool, ...],
    profile: str,
) -> tuple[tuple[bool, ...], float]:
    """Keep native rows/indices; modify only the declared policy distribution.

    Full-HP healing is excluded only with an already-eligible smith/recall choice
    and no Dream Catcher. This is an experimental restriction, not a universal
    dominance claim. Low HP and singleton decisions are never excluded here.
    Temperature flattens rest and visible optional Sapphire-choice preferences;
    it does not force a key, grant one, expose future offers, or bias its logit.
    """
    if profile not in EXPLORATION_PROFILES:
        raise ValueError("Unknown macro exploration profile")
    if profile == "none":
        return eligibility, 1.0
    allowed = checked_eligibility(len(actions), eligibility)
    context = observation.context
    full_hp_rest = (
        observation.kind == "rest"
        and context.player_hp == context.player_max_hp
        and not any(r.content_key == RelicKey.DREAM_CATCHER for r in context.relics)
        and any(
            yes and a.kind in ("rest_open_smith", "rest_recall")
            for a, yes in zip(actions, allowed, strict=True)
        )
    )
    if full_hp_rest:
        eligibility = tuple(
            yes and a.kind != "rest_heal"
            for a, yes in zip(actions, allowed, strict=True)
        )
        allowed = checked_eligibility(len(actions), eligibility)
    targeted = observation.kind == "rest" or (
        observation.kind == "reward"
        and not context.keys.sapphire
        and any(
            yes and a.kind == "take_sapphire_key"
            for a, yes in zip(actions, allowed, strict=True)
        )
    )
    temperature = (
        4.0 if profile == "rest_keys_t4" else 8.0 if profile == "rest_keys_t8" else 1.0
    )
    return eligibility, temperature if targeted and sum(allowed) > 1 else 1.0
