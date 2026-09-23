"""Simulator-only combat exploration UI and session service."""

FORMAT_NAME = "sts_combat_explorer_session"
SESSION_SCHEMA_VERSION = 1
TASK_PROTOCOL = "combat_task_smoke_bomb_excluded_v1"
POLICY_ADAPTER_VERSION = "combat_model_typed_batch1_v1"
SAMPLING_GENERATOR = "python.random.Random"
SAMPLING_GENERATOR_VERSION = "cpython-random-3.12"
DEFAULT_MAX_DECISIONS = 512
MAX_DECISIONS_LIMIT = 512
MIN_TEMPERATURE = 0.05
MAX_TEMPERATURE = 5.0
MAX_SESSION_NODES = 20_000
MAX_NODE_CHILDREN = 1_024
MAX_ID_LENGTH = 200
MAX_SESSION_FILE_BYTES = 32 * 1024 * 1024
MAX_JSON_DEPTH = 64
