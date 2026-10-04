"""Replay this research probe's journals from initial state plus accepted actions.

Not the real-game trace verifier and not observation-driven hydration. Refuses a
native hash mismatch. A final rejected attempt is checked once without retrying
an alternative; its public pre-state is returned for diagnosis.

PYTHONPATH=rl uv run --project simulator/python python -m run_training.replay CASE.jsonl
"""

import argparse
import hashlib
import json
from dataclasses import asdict
from pathlib import Path

from sts_sim import State, _native

from .contracts import PolicyAction, outcome


def replay(path: Path) -> dict:
    manifest = json.loads((path.parent / "manifest.json").read_text())
    native_hash = hashlib.sha256(Path(_native.__file__).read_bytes()).hexdigest()
    if native_hash != manifest["native_sha256"]:
        raise ValueError(
            "Native hash mismatch; journal must use its original simulator build"
        )
    with path.open() as stream:
        setup = json.loads(next(stream))
        if setup["type"] != "setup":
            raise ValueError("Journal lacks setup")
        if setup["mode"] not in ("natural", "synthetic-act4", "profile-act4"):
            raise ValueError("Unknown journal construction mode")
        state = (
            State.new(setup["seed"], setup["ascension"])
            if setup["mode"] == "natural"
            else State.new_synthetic(
                setup["seed"], setup["ascension"], setup["hp"], final_act=True
            )
        )
        decision = state.decision()
        accepted = 0
        pending = None
        rejected = None
        failure_observation = None
        result = None
        for line in stream:
            if result is not None:
                raise ValueError("Journal contains records after its result")
            record = json.loads(line)
            if record["type"] == "attempt":
                if pending is not None or rejected is not None:
                    raise ValueError(
                        "Journal attempted another action before settlement"
                    )
                if (
                    record["step"] != accepted
                    or record["revision"] != decision.revision
                ):
                    raise ValueError("Journal step/revision mismatch")
                index = record["index"]
                if not 0 <= index < len(decision.actions):
                    raise ValueError("Journal candidate index is not currently legal")
                action = decision.actions[index]
                if record["action"] != asdict(PolicyAction.from_action(action)):
                    raise ValueError("Journal candidate descriptor mismatch")
                before = decision
                try:
                    decision = state.step(action)
                    pending = record
                except (ValueError, RuntimeError) as exception:
                    rejected = f"{type(exception).__name__}: {exception}"
                    after = state.decision()
                    if (
                        after.revision != before.revision
                        or after.observation != before.observation
                        or tuple(PolicyAction.from_action(a) for a in after.actions)
                        != tuple(PolicyAction.from_action(a) for a in before.actions)
                    ):
                        raise ValueError(
                            "Rejected action changed public state"
                        ) from exception
                    failure_observation = asdict(before.observation)
            elif record["type"] == "accepted":
                if pending is None or record["step"] != accepted:
                    raise ValueError("Journal acceptance without matching attempt")
                if record["revision"] != decision.revision:
                    raise ValueError("Journal successor revision mismatch")
                accepted += 1
                pending = None
            elif record["type"] == "result":
                if result is not None:
                    raise ValueError("Duplicate journal result")
                result = record
            else:
                raise ValueError("Unknown journal record")
        if pending is not None or result is None:
            raise ValueError("Partial journal; complete result not verified")
        context = decision.observation.context
        if accepted != result["accepted"] or (
            context.act,
            context.floor,
            context.player_hp,
        ) != (result["act"], result["floor"], result["hp"]):
            raise ValueError("Journal final public context mismatch")
        if result["outcome"] == "error":
            if rejected != result["error"]:
                raise ValueError("Journal error did not reproduce")
        elif result["outcome"] == "truncated" and outcome(decision) != "ongoing":
            raise ValueError("Journal cutoff is actually a terminal or invalid state")
        elif rejected is not None or (
            result["outcome"] != "truncated" and outcome(decision) != result["outcome"]
        ):
            raise ValueError("Journal outcome did not reproduce")
        return {
            "verified": True,
            "accepted": accepted,
            "outcome": result["outcome"],
            "error": rejected,
            "failure_public_observation": failure_observation,
        }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("journal", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = replay(args.journal)
    if args.output:
        with args.output.open("x") as stream:
            stream.write(json.dumps(result, indent=2) + "\n")
    else:
        print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
