"""Small interactive example for the native simulator API."""

from sts_sim import Action, State


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


def main() -> None:
    state = State.new("HUMAN1")
    print("Enter an action number or 'q'.")

    while True:
        actions = state.legal_actions()
        observation = state.observation()
        print(f"\nphase={observation.phase} decision={observation.kind}")
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
