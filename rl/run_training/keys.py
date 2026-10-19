"""Public key-choice opportunities, not inferred preferences from key ownership."""

from dataclasses import dataclass, replace

from sts_sim import Observation

from run_training.contracts import PolicyAction
from run_training.diagnostics import probabilities
from run_training.restrictions import checked_eligibility

KEY_CHOICE_PROTOCOL = "accepted_key_decisions_native_eligible_unique_locations_v1"
KEY_CHOICES = ("ruby", "sapphire", "emerald_pickup", "emerald_route")


@dataclass(frozen=True)
class KeyDecision:
    location: tuple[int, int]
    offered: tuple[bool, ...]
    eligible: tuple[bool, ...]
    chosen: tuple[bool, ...]
    probability: tuple[float, ...]
    forced: bool


def key_decision(
    observation: Observation,
    actions: tuple[PolicyAction, ...],
    choice: int,
    *,
    eligibility: tuple[bool, ...] = (),
    logits: tuple[float, ...] | None = None,
) -> KeyDecision | None:
    """Decision-level opportunities; route choice is NOT Emerald acquisition.

    Only public native choices count, and already-held keys are excluded. The
    caller commits this diagnostic only after the chosen transition succeeds.
    """
    if observation.kind not in ("rest", "reward", "map"):
        return None
    keys = observation.context.keys
    groups: list[list[int]] = [[], [], [], []]
    for i, action in enumerate(actions):
        if action.kind == "rest_recall" and not keys.ruby:
            groups[0].append(i)
        elif action.kind == "take_sapphire_key" and not keys.sapphire:
            groups[1].append(i)
        elif action.kind == "take_emerald_key" and not keys.emerald:
            groups[2].append(i)
        elif action.kind == "choose_map_node" and not keys.emerald:
            if (
                observation.kind != "map"
                or action.node_slot is None
                or not 0 <= action.node_slot < len(observation.screen.nodes)
            ):
                raise ValueError("Unresolved public key-route choice")
            if observation.screen.nodes[action.node_slot].burning_elite:
                groups[3].append(i)
    if not any(groups):
        return None
    allowed = checked_eligibility(len(actions), eligibility, choice=choice)
    if logits is None:
        if sum(allowed) != 1:
            raise ValueError("Optional key choice requires policy probabilities")
        p = tuple(float(v) for v in allowed)
    else:
        if len(logits) != len(actions):
            raise ValueError("Key diagnostic candidate mismatch")
        p = probabilities(logits)
        if any(
            probability != 0
            for probability, eligible in zip(p, allowed, strict=True)
            if not eligible
        ):
            raise ValueError(
                "Excluded key diagnostic candidate has positive probability"
            )
    return KeyDecision(
        (observation.context.act, observation.context.floor),
        tuple(bool(group) for group in groups),
        tuple(any(allowed[i] for i in group) for group in groups),
        tuple(choice in group for group in groups),
        tuple(sum(p[i] for i in group) for group in groups),
        sum(allowed) == 1,
    )


@dataclass(frozen=True)
class KeyChoiceStats:
    native_opportunities: tuple[int, ...] = (0, 0, 0, 0)
    eligible_opportunities: tuple[int, ...] = (0, 0, 0, 0)
    choices: tuple[int, ...] = (0, 0, 0, 0)
    forced_choices: tuple[int, ...] = (0, 0, 0, 0)
    probability_sum: tuple[float, ...] = (0.0, 0.0, 0.0, 0.0)
    seen_locations: tuple[tuple[int, int, int], ...] = ()
    chosen_locations: tuple[tuple[int, int, int], ...] = ()

    def accepted(self, decision: KeyDecision | None) -> "KeyChoiceStats":
        if decision is None:
            return self
        seen = list(self.seen_locations)
        chosen = list(self.chosen_locations)
        for i, available in enumerate(decision.eligible):
            location = (i, *decision.location)
            if available and location not in seen:
                seen.append(location)
            if decision.chosen[i] and location not in chosen:
                chosen.append(location)
        return replace(
            self,
            native_opportunities=tuple(
                n + int(v)
                for n, v in zip(
                    self.native_opportunities, decision.offered, strict=True
                )
            ),
            eligible_opportunities=tuple(
                n + int(v)
                for n, v in zip(
                    self.eligible_opportunities, decision.eligible, strict=True
                )
            ),
            choices=tuple(
                n + int(v) for n, v in zip(self.choices, decision.chosen, strict=True)
            ),
            forced_choices=tuple(
                n + int(v and decision.forced)
                for n, v in zip(self.forced_choices, decision.chosen, strict=True)
            ),
            probability_sum=tuple(
                n + p if available else n
                for n, p, available in zip(
                    self.probability_sum,
                    decision.probability,
                    decision.eligible,
                    strict=True,
                )
            ),
            seen_locations=tuple(seen),
            chosen_locations=tuple(chosen),
        )


def key_choice_scores(records: list[KeyChoiceStats]) -> dict[str, float]:
    result = {}
    for i, name in enumerate(KEY_CHOICES):
        prefix = f"keys/{name}"
        native = sum(r.native_opportunities[i] for r in records)
        eligible = sum(r.eligible_opportunities[i] for r in records)
        choices = sum(r.choices[i] for r in records)
        forced = sum(r.forced_choices[i] for r in records)
        locations = sum(
            sum(location[0] == i for location in r.seen_locations) for r in records
        )
        chosen_locations = sum(
            sum(location[0] == i for location in r.chosen_locations) for r in records
        )
        result.update(
            {
                f"{prefix}/native_opportunities": float(native),
                f"{prefix}/eligible_opportunities": float(eligible),
                f"{prefix}/choices": float(choices),
                f"{prefix}/forced_choices": float(forced),
                f"{prefix}/optional_opportunities": float(eligible - forced),
                f"{prefix}/unique_opportunity_locations": float(locations),
                f"{prefix}/unique_chosen_locations": float(chosen_locations),
            }
        )
        if eligible:
            result[f"{prefix}/choice_rate"] = choices / eligible
            result[f"{prefix}/choice_probability"] = (
                sum(r.probability_sum[i] for r in records) / eligible
            )
        if eligible > forced:
            result[f"{prefix}/optional_choice_rate"] = (choices - forced) / (
                eligible - forced
            )
        if locations:
            result[f"{prefix}/location_choice_rate"] = chosen_locations / locations
    return result
