# Simulator Combat Explorer

Local, simulator-only UI for inspecting synthetic combats and trained combat policies.
There is no automatic playback: you step between player decisions, or ask the model
to compute a continuation that you then inspect.

## Launch

From `rl/`, after the simulator Python package is installed (`uv run maturin develop --uv`
in `simulator/python` as in `simulator/README.md`), prepare ignored local artifacts
once. Pass explicit source paths; nothing is inferred from a home directory:

```bash
uv run python -m combat_explorer.prepare_local \
  --distributions /path/to/loadout-a0-v3/fit.json \
  --checkpoint /path/to/checkpoint.pt \
  --output-dir combat_explorer_local
```

That copies the distributions and checkpoint byte-for-byte, generates a
current-native `explorer-roots-a0-v1.json` with `validation_set.build_validation`,
and writes `ARTIFACTS.json` / `LAUNCH.txt`. The roots file is an explorer dataset,
not the frozen `validation-a0-v1` benchmark. The checkpoint is usable inference on
the current `CombatModel` architecture, not same-native training evidence.

Then:

```bash
uv run python -m combat_explorer \
  --validation-manifest combat_explorer_local/explorer-roots-a0-v1.json \
  --distributions combat_explorer_local/fit.json \
  --checkpoint combat_explorer_local/checkpoint.pt \
  --sessions-dir combat_explorer_sessions
```

Open `http://127.0.0.1:8765`. The process binds to loopback only. Browser
`Origin` must match the `Host` header including port; `http` and `https` are
both accepted so a Tailscale Serve TLS proxy can terminate HTTPS.

This machine already uses **https://sorry.tail76d105.ts.net/** for local W&B.
Leave that proxy on port 443. For the explorer, advertise a **separate**
tailnet URL on port 8765 (HTTPS, tailnet-only):

```bash
tailscale serve --bg --https=8765 http://127.0.0.1:8765
```

Then launch with the MagicDNS hostname allowed:

```bash
uv run python -m combat_explorer \
  --validation-manifest combat_explorer_local/explorer-roots-a0-v1.json \
  --distributions combat_explorer_local/fit.json \
  --checkpoint combat_explorer_local/checkpoint.pt \
  --sessions-dir combat_explorer_sessions \
  --allowed-host sorry.tail76d105.ts.net
```

Open **https://sorry.tail76d105.ts.net:8765** from a Tailscale-connected
device. Do not use `tailscale serve reset` (that would drop W&B). Disable only
the explorer proxy with `tailscale serve --https=8765 off`.

A distinct MagicDNS name such as a Tailscale Service VIP requires a tagged
node; this host is not tagged, so the separate address is the node name on
port 8765 rather than W&B's port 443.

`--validation-manifest`, `--distributions`, and `--checkpoint` are all optional.
Without a manifest you can still generate roots if distributions are configured.
Without a checkpoint you can still play manually. Checkpoint/model paths are only
accepted if they are the configured checkpoint file or another path passed via
the server's explicit allow-list; sibling directories are not exposed.

Do not point the explorer at frozen `validation-a0-v1.json` in this worktree: its
native fingerprint is different, and `load_validation` rejects that mismatch.
Leave that frozen file unchanged.

## Workflow

1. Load a frozen synthetic validation case or generate a new A0 combat root.
2. Inspect the combat board and legal actions.
3. Click an action to play it, or **Model step** / **Model finish from here**.
4. Click any history node. Making a new move from a historical node creates a branch.
5. Save/load the tree as JSON under `--sessions-dir`.

Default model control is **sample at temperature 1**, matching
`train.play_combat`'s `Categorical(logits)` evaluation sampling. Greedy mode is
argmax over logits with earlier-native-index tie breaks. The visible **Decision
limit** bounds model finish jobs (default 512). Changing mode/temperature
reanalyzes the selected node; it does not rewrite historical choice diagnostics.

## Formats

- Validation roots: `schema=1` / `protocol=synthetic_pre_entry_hp_A0` (see `validation_set.py`).
- Generated roots: `LoadoutSampler` + `sample_root`, A0 only.
- Checkpoints: current `train_synthetic.py` dict with a `model` state, loaded
  `weights_only=True` into `CombatModel`. Incompatible architectures fail closed.
  Identity is the checkpoint file SHA-256, not the path `latest.pt`.
- Sessions: `sts_combat_explorer_session` schema 1 JSON. Reconstruction replays
  accepted actions from `spec_json`; recorded observations are never written back
  into the simulator. Job outcomes (limit/cancel/error/attempted action) are
  stored separately from combat outcomes.

## Limitations

- Combat only; the explorer stops at combat completion and does not play rewards.
- Smoke Bomb **use** is excluded by the training task filter for human and model
  actions. Inventory still shows the potion.
- Between-decision summaries are **net public-state deltas**. Individual enemy
  actions are not exposed by the fair API.
- Kill markers require an unambiguous same-identity alive→dead transition.
  Escapes are excluded. Missing/replaced/split enemies are uncertain removals.
- Hidden draw order is not shown. Draw-pile identities are unordered.
- Cross-version simulator or observation-schema replay is rejected.

## Tests

From `rl/`:

```bash
uv run python -m unittest tests.test_combat_explorer_policy tests.test_combat_explorer_core tests.test_combat_explorer_api tests.test_combat_explorer_js tests.test_combat_explorer_browser tests.test_combat_explorer_prepare tests.test_model tests.test_synthetic_roots tests.test_validation_set
```

Browser tests use Playwright/Chromium. They are a walkthrough of the UI, not a
proof of real-game parity. Random-initialized `CombatModel` weights used in unit
and browser tests are fixtures, not trained-checkpoint evidence.

Install Chromium once; tests do **not** auto-install it:

```bash
uv run playwright install chromium
```

Browser tests default to temporary deterministic fixtures, including a tiny
generated validation manifest. Optional real artifacts and screenshots must be
passed explicitly and are not a substitute for those fixture tests:

```bash
COMBAT_EXPLORER_CHECKPOINT=combat_explorer_local/checkpoint.pt \
COMBAT_EXPLORER_DISTRIBUTIONS=combat_explorer_local/fit.json \
COMBAT_EXPLORER_VALIDATION_MANIFEST=combat_explorer_local/explorer-roots-a0-v1.json \
COMBAT_EXPLORER_SCREENSHOT_DIR=combat_explorer_local/screenshots \
uv run python -m unittest tests.test_combat_explorer_browser
```
