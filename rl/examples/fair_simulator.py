"""A fair API walkthrough, not a trainer or a search implementation.

Run from the repository root:
    uv run --project simulator/python python rl/examples/fair_simulator.py
"""

import argparse
import random

from sts_sim import Action, Observation, State


def describe(action: Action) -> str:
    """Slots refer to visible entries in THIS decision, not persistent IDs."""
    slots = (
        "hand_slot", "target_slot", "potion_slot", "option_slot",
        "card_slot", "node_slot", "reward_slot", "shop_slot",
    )
    details = ", ".join(
        f"{name}={value}" for name in slots
        if (value := getattr(action, name)) is not None
    )
    return f"{action.kind}({details})"


def choose_action(
    observation: Observation,
    actions: tuple[Action, ...],
    rng: random.Random,
) -> int:
    """Your policy goes here. Return an index into the current candidate list.

    Only public observation + legal candidates enter this function; no State,
    game seed, revision, snapshots, or rollout access. Public history could be
    added explicitly later. This random baseline deliberately ignores features.
    Action.revision is transport metadata, NOT a policy feature.
    """
    return rng.randrange(len(actions))


def inspect_combat(observation: Observation) -> None:
    """Explicit combat fields after `observation.kind == "combat"` narrowing."""
    if observation.kind != "combat":
        return
    player = observation.screen.player
    print(f"  energy={player.energy} block={player.block}")
    for entry in observation.screen.hand:
        print(f"  hand[{entry.slot}]: {entry.card.content_key}, cost={entry.card.cost}")
    for monster in observation.screen.monsters:
        print(f"  target[{monster.slot}]: {monster.content_key}, hp={monster.hp}")
    # Inspect screen.draw_pile.cards as a multiset, NOT top-to-bottom order.
    # Only draw_pile.known_positions records publicly known next-draw slots.


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seed", default="HUMAN1", help="environment seed; not policy input")
    parser.add_argument("--ascension", type=int, choices=range(21), default=0)
    parser.add_argument("--policy-seed", type=int, default=123, help="independent policy RNG")
    parser.add_argument("--max-decisions", type=int, default=30)
    parser.add_argument("--inspect", action="store_true", help="print public field names, combat sample, and candidates")
    args = parser.parse_args()
    if args.max_decisions < 1:
        parser.error("--max-decisions must be positive")

    # Environment ownership stays here, outside the policy. This is a coding
    # convention, not a security sandbox for untrusted Python code.
    state = State.new(args.seed, ascension=args.ascension)
    policy_rng = random.Random(args.policy_seed)
    decision = state.decision()

    for step in range(args.max_decisions):
        observation = decision.observation
        actions = decision.actions
        context = observation.context
        print(
            f"{step:04d} {observation.kind:>8} "
            f"act={context.act} floor={context.floor} "
            f"hp={context.player_hp}/{context.player_max_hp} gold={context.gold}"
        )
        if args.inspect:
            print("  public context:", context)
            print("  public screen: ", observation.screen)
            inspect_combat(observation)
            for index, action in enumerate(actions):
                print(f"  [{index}] {describe(action)}")

        if observation.kind == "complete":
            print("Environment complete (not automatically a win).")
            return
        if not actions:
            print("Stopped: no legal actions; do not silently label this a win or terminal loss.")
            return

        index = choose_action(observation, actions, policy_rng)
        if not 0 <= index < len(actions):
            raise ValueError("Policy returned an invalid candidate index")
        print("  ->", describe(actions[index]))

        # One accepted action, one transition. Use the returned atomic decision;
        # never reuse previous candidates or test alternatives on the live state.
        # Simulator errors intentionally propagate: don't turn them into rewards
        # or let the policy probe errors and retry a different action.
        decision = state.step(actions[index])

    print(
        f"Demo budget reached; next screen={decision.observation.kind}. "
        "This cutoff is not a game outcome."
    )


if __name__ == "__main__":
    main()
