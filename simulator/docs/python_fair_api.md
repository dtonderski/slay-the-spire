# Typed Python Fair Combat API

The public Python package is `sts_sim`. Its fair surface is implemented as a
typed Python facade over the private PyO3 extension `sts_sim._native`.

```python
from sts_sim import FairCombatEnv, PlayerChoiceRequest

env = FairCombatEnv.combat_fixture()
decision = env.decision()
choice = decision.choices[0]
result = env.step(PlayerChoiceRequest(decision.decision_revision, choice))
```

`decision()` returns one atomic `FairDecision` containing the symbolic
`FairCombatObservation`, the public `PlayerChoice` tuple, and the monotonic
decision revision. `step()` resolves through the existing Rust legality engine
and returns the next atomic decision.

The fair facade does not expose state JSON, snapshots, hashes, RNG state,
authoritative IDs, or exact actions. The explicitly privileged `OmniCombatEnv`
and `OmniRunEnv` classes remain available for replay, diagnostics, and search.

Every Python source file and test is checked by `ty` and Ruff's annotation
rules. The native extension has a checked `_native.pyi` contract, and the
package includes `py.typed` for downstream type checkers.

From `simulator/`, run the Python checks with:

```bash
uv sync --project python --reinstall-package sts-sim
uv run --project python ty check
uv run --project python ruff check
uv run --project python pytest -q python/tests
```

The explicit reinstall step builds the private PyO3 extension before pytest
imports `sts_sim._native`, and also makes the clean-checkout CI path explicit.
