"""Diagnostics for rejected simulator steps; never policy inputs."""


class SimulatorStepError(RuntimeError):
    """A native step failed; batch advancement may be partial and must not be retried."""

    def __init__(self, message: str, step: int, root_indices: list[int], action_prefixes: list[list[int]]) -> None:
        super().__init__(message)
        self.step = step
        self.root_indices = list(root_indices)
        # Includes the attempted current action, whose acceptance is unknown.
        self.action_prefixes = [list(prefix) for prefix in action_prefixes]
