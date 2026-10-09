"""HTTP-aware errors for the explorer service."""


class ExplorerError(Exception):
    status_code = 400
    code = "explorer_error"

    def __init__(self, message: str, *, code: str | None = None, status_code: int | None = None) -> None:
        super().__init__(message)
        if code is not None:
            self.code = code
        if status_code is not None:
            self.status_code = status_code


class NotFoundError(ExplorerError):
    status_code = 404
    code = "not_found"


class ConflictError(ExplorerError):
    status_code = 409
    code = "conflict"


class ForbiddenError(ExplorerError):
    status_code = 403
    code = "forbidden"


class SimulatorError(ExplorerError):
    code = "simulator_error"


class ModelError(ExplorerError):
    code = "model_error"


class InvalidSeedError(ExplorerError):
    code = "invalid_seed"
