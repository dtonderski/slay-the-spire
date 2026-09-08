"""Small interactive example for the native simulator API."""

from sts_sim import Action, Observation, State


def action_text(action: Action) -> str:
    slots = [
        f"{name}={value}"
        for name in (
            "hand_slot",
            "potion_slot",
            "option_slot",
            "target_slot",
            "card_slot",
            "node_slot",
            "reward_slot",
            "shop_slot",
        )
        if (value := getattr(action, name)) is not None
    ]
    suffix = f" ({', '.join(slots)})" if slots else ""
    return f"{action.family}.{action.kind}{suffix}"


def describe_observation(observation: Observation) -> str:
    summary = f"phase={observation.phase} decision={observation.kind}"
    context = observation.context
    summary += (
        f" hp={context.player_hp}/{context.player_max_hp} gold={context.gold}"
    )
    if observation.kind == "combat":
        player = observation.screen.player
        summary += f" energy={player.energy} hand={len(observation.screen.hand)}"
    elif observation.kind == "map":
        summary += f" reachable={len(observation.screen.reachable_nodes)}"
    return summary


def main() -> None:
    state = State.new("HUMAN1")
    print("Enter an action number or 'q'.")

    while True:
        actions = state.legal_actions()
        observation = state.observation()
        print(f"\n{describe_observation(observation)}")
        for index, action in enumerate(actions):
            print(f"  [{index}] {action_text(action)}")

        if not actions:
            print("No legal actions remain.")
            return

        command = input("> ").strip().lower()
        if command == "q":
            return
        try:
            action = actions[int(command)]
        except (ValueError, IndexError):
            print("Choose one of the displayed action numbers.")
            continue
        decision = state.step(action)
        print(f"revision={decision.revision}")


if __name__ == "__main__":
    main()
