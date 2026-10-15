"""Accepted public-history milestones, never simulator setters or policy inputs."""

from dataclasses import dataclass

from sts_sim import MonsterKey, Observation

PROGRESS_PROTOCOL = "accepted_public_run_milestones_v1"


@dataclass(frozen=True)
class RunProgress:
    furthest_floor: int = 0
    furthest_act: int = 1
    acts_cleared: int = 0
    ruby_seen: bool = False
    emerald_seen: bool = False
    sapphire_seen: bool = False
    heart_reached: bool = False

    def __post_init__(self) -> None:
        if (
            any(
                type(value) is not int
                for value in (self.furthest_floor, self.furthest_act, self.acts_cleared)
            )
            or any(
                type(value) is not bool
                for value in (
                    self.ruby_seen,
                    self.emerald_seen,
                    self.sapphire_seen,
                    self.heart_reached,
                )
            )
            or self.furthest_floor < 0
            or not 1 <= self.furthest_act <= 4
            or not 0 <= self.acts_cleared <= 3
        ):
            raise ValueError("Invalid observed run progress")

    def observed(self, observation: Observation) -> "RunProgress":
        """Call at initialization and accepted successors only, not attempted steps.

        Act entry proves earlier acts cleared; explicit final outcomes also prove
        Act 3 cleared. Floors alone never grant an act-clear bonus. Keys are ever
        observed flags, not an assertion that they remain held after Act-4 entry.
        """
        context = observation.context
        if not 1 <= context.act <= 4 or context.floor < 0:
            raise ValueError("Invalid public run milestone context")
        cleared = min(3, context.act - 1)
        if context.outcome in ("act3_clear", "heart_clear"):
            cleared = 3
        heart = context.outcome == "heart_clear" or (
            observation.kind == "combat"
            and any(
                monster.content_key == MonsterKey.CORRUPT_HEART
                for monster in observation.screen.monsters
            )
        )
        return RunProgress(
            max(self.furthest_floor, context.floor),
            max(self.furthest_act, context.act),
            max(self.acts_cleared, cleared),
            self.ruby_seen or context.keys.ruby,
            self.emerald_seen or context.keys.emerald,
            self.sapphire_seen or context.keys.sapphire,
            self.heart_reached or heart,
        )


def progress_scores(records: list[RunProgress]) -> dict[str, float]:
    result = {"progress/observed_attempts": float(len(records))}
    if not records:
        return result
    flags = {
        **{
            f"act_{act}_clears_observed": sum(r.acts_cleared >= act for r in records)
            for act in (1, 2, 3)
        },
        "act_4_entries_observed": sum(r.furthest_act == 4 for r in records),
        "heart_encounters_observed": sum(r.heart_reached for r in records),
        "ruby_seen": sum(r.ruby_seen for r in records),
        "emerald_seen": sum(r.emerald_seen for r in records),
        "sapphire_seen": sum(r.sapphire_seen for r in records),
        "all_keys_seen": sum(
            r.ruby_seen and r.emerald_seen and r.sapphire_seen for r in records
        ),
    }
    result.update({f"progress/{name}": float(value) for name, value in flags.items()})
    result["progress/furthest_floor_mean"] = sum(
        r.furthest_floor for r in records
    ) / len(records)
    return result
