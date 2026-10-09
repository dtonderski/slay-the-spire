"""Launch the local combat explorer.

From `rl/`:

    uv run python -m combat_explorer --help
"""

from combat_explorer.server import main

if __name__ == "__main__":
    raise SystemExit(main())
