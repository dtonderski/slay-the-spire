# RL Visibility Matrix

## Purpose

This document defines the visibility boundary for fair reinforcement-learning agents that interact with the Slay the Spire simulator.

The simulator may hold full hidden state. A fair agent should only receive information a real player could see or infer from public gameplay history. Debug and planning tools may use richer state, but those capabilities must be explicit and separate.

## Principles

- Fair observations must not expose hidden simulator state.
- Fair actions should use visible slots and options, not internal IDs.
- Fair action masks must not reveal hidden information.
- Debug, verifier, and omniscient planning APIs may expose full state, but must be visibly separate from fair APIs.
- Belief-state systems may track hidden possibilities internally, but fair policies should not directly inspect particle full states.
- Values reconstructed from public history are fair only when the same history is available to the agent or wrapper.

## Combat Matrix

| Area | Fair Observation | Hidden | Debug / Omniscient | Belief-Latent | Notes / Open Questions |
|---|---|---|---|---|---|
| Player | Current HP, max HP, block, energy, visible powers and debuffs. Public-history counters may be included if the wrapper derives them from visible events, such as cards played this turn. | Internal counters not shown by UI and not reconstructable from the agent's observation history. | Full player state and all counters. | Public-history-derived values when the agent lacks enough history in the current observation frame. | Avoid exposing raw simulator counters just because they are convenient. |
| Hand | Ordered visible hand slots, card names, upgrades, current visible costs, visible card text, visible playability. | Internal `CardId`, UUIDs, generation provenance, hidden identity continuity. | Full card instances and mutable internal fields. | Identity continuity if needed for particle tracking. | Fair actions should target hand slots, not card IDs. |
| Draw Pile | Draw pile count and visible pile contents as an unordered/multiset-style list when the combat pile viewer exposes them. Ordered contents only when a UI effect reveals order. | Draw pile order by default. | Exact ordered draw pile. | Possible draw orders and shuffle state. | `Frozen Eye` is the main exception for ordered draw visibility. Need verify exact pile-view ordering semantics. |
| Discard Pile | Discard pile count and visible contents during combat when the pile viewer exposes them. | Internal IDs and any ordering/detail not exposed by UI. | Exact pile with card instance IDs. | Possible ordering if order matters later. | Treat visible contents as fair; still avoid internal instance IDs. Need verify exact UI ordering semantics. |
| Exhaust Pile | Exhaust pile count and visible contents during combat when the pile viewer exposes them. | Internal IDs and any ordering/detail not exposed by UI. | Exact pile with card instance IDs. | Possible ordering if order matters later. | Treat visible contents as fair; still avoid internal instance IDs. Need verify exact UI ordering semantics. |
| Monsters | Visible HP, max HP if UI-visible, block, powers/debuffs, alive/dead/minion state, current visible intent, intent damage/count when not hidden by an effect such as `Runic Dome`. | Future moves, private AI counters, hidden move history, latent random rolls, and intent while `Runic Dome` hides it. | Full monster state, move history, private flags. | AI state consistent with visible intent and public history. | Observation should carry an explicit `intent_visible` flag. |
| Relics | Owned relic list, visible counters/charges, visible effects, and values inferable from public history. | Relic pool order and simulator-only implementation counters. Per-relic counters are hidden only when not shown and not reconstructable from public history. | Full relic counters and relic pool state. | Hidden or uncertain relic state plus future relic pools. | Need a per-relic table; do not assume all `RelicCounters` fields are fair. |
| Potions | Potion slots, potion identities, visible usability, and currently visible generated choices. Newly generated potions become visible once they appear in slots. | Potion RNG and generated outcomes before the action reveals them. | Full potion state and RNG. | Hidden generated outcomes before reveal. | Entropic Brew-style refills are hidden before use, then visible as concrete filled slots afterward. |
| Powers | Visible player and monster powers/debuffs, amounts, and public durations. | Internal implementation flags not represented by UI. | Full power structs and timing internals. | Inferred hidden timing state if needed. | Need explicit exceptions for powers with confusing UI timing. |
| RNG | None. | Seeds, counters, stream state, RNG logs. | All RNG state and logs. | RNG counters/state consistent with observations. | RNG must never appear in fair `obs` or `info`. |
| Actions | Visible action descriptors and fair action mask. | Internal IDs and hidden-dependent exact legality details. | Exact simulator actions and validation reasons. | Plausible hidden outcomes behind visible actions. | Fair masks must not leak hidden top-deck or future choices. |
| Errors / Info | Coarse public phase, terminal outcome, and coarse invalid-action enums. | Detailed simulator errors, hidden validation reasons. | Full event logs, hashes, snapshots, diffs, parity metadata. | Observation history and provenance. | Avoid errors like “top draw card requires target.” |
| Snapshots | No branchable full restore in fair mode. Visible replay log only. | Full serialized state. | Full snapshot/restore. | Particle snapshots inside belief system. | Fair env should prefer seed/config plus visible action log. |
| Verification Metadata | None. | Trace waivers, source labels, resync flags, parity scaffolding. | Full verifier reports and divergence context. | Dataset provenance labels outside policy input. | Provenance is useful for training data, not fair policy observation. |

## Special Cases

### Frozen Eye

`Frozen Eye` reveals draw pile order in the real game UI. In fair mode, ordered draw pile contents may be exposed only while that visibility rule is active.

Open questions:

- Should the observation expose full ordered card features or compact content IDs plus visible card features?
- How should the tensor wrapper represent variable draw pile length?

### Runic Dome

`Runic Dome` hides monster intents. In fair mode, monster intent category, damage, hit count, and intent-derived target information must be masked.

Open questions:

- Should the observation include an explicit `intent_visible=false` flag?
- Should the action schema change at all under `Runic Dome`, or only the observation?

### Havoc And Other Top-Deck Effects

Cards like `Havoc` can depend on the hidden top card. Fair action masks must not reveal the top card by changing target availability in a way a real player could not know.

Preferred rule:

- Expose visible-shape actions.
- Resolve hidden details internally.
- Return only coarse public errors if a visible action cannot resolve.

Open questions:

- For hidden top-card target requirements, should the fair mask include all plausible targets or a single generic play action?
- How should the world-model dataset label transitions where hidden top-deck state mattered?

### Pile Viewers

During combat, the player can inspect pile contents through the UI. Fair observations may therefore include visible draw/discard/exhaust contents, but should still avoid internal instance IDs and should not treat draw pile order as known unless an effect reveals order.

Open questions:

- Does the UI ordering of each pile carry gameplay information, or should the fair tensor wrapper encode these as unordered multisets by default?
- How should pile viewer availability be represented outside combat or during blocking choice screens?

### Choice Screens

Discovery, potion-created card choices, hand/discard/exhaust selection screens, and reward choices should expose only currently visible options.

Rules:

- Offered visible choices are fair.
- Unoffered generated alternatives are hidden.
- Internal generated card IDs are hidden.
- Selection actions should use visible option slots.

Open questions:

- Which discard/exhaust pile contents are visible outside an explicit choice screen?
- Should staged selections be part of fair observation? Usually yes, if visible in UI.

### Entropic Brew And Random Potion Generation

Random potion generation is hidden before the action resolves. After `Entropic Brew` or similar effects fill slots, the resulting potion identities are visible because they appear in the potion belt.

Rules:

- Before use, do not expose future generated potion identities or RNG details.
- After use, expose the filled potion slots normally.
- Do not expose unchosen or unrealized alternatives.

### Internal IDs

Fair APIs should avoid exposing internal identity as a semantic feature.

Rules:

- Use `hand_slot`, not `CardId`, for card play.
- Use visible monster slot, not `MonsterId`, for targeting.
- Use visible option slot, not reward/card UUID, for choices.

Open questions:

- Do we need stable per-episode visible IDs for UI continuity, or are slots enough?
- How should generated summons or split monsters preserve visible slot mapping?

## Highest-Risk Leaks

- Exposing full `CombatState` or `RunState` through Python observations.
- Returning full snapshots, hashes, event logs, RNG logs, or verifier diffs in fair `step()` results.
- Using internal `CardId`, `MonsterId`, UUIDs, or stable generated object identity as action or observation features.
- Including draw pile order when `Frozen Eye` is absent.
- Showing monster intent under `Runic Dome`.
- Letting action masks reveal hidden top-deck information.
- Letting invalid-action errors reveal hidden state.
- Exposing future reward, relic, potion, event, or shop pools.
- Treating trace-resync or parity metadata as agent-observable.
- Making branchable full-state restore available in fair online policy mode.

## Review Questions

- Does Slay the Spire's pile viewer ordering have any hidden-order leakage, or should draw/discard/exhaust contents be encoded as unordered multisets by default?
- Should fair observations include public action history, or should that be left to recurrent policies/wrappers?
- Which relic counters are visibly shown, inferable, hidden, or debug-only?
- How should fair action masks handle hidden-dependent cards like `Havoc`?
- Should debug rollout/MCTS live in the same package as fair envs, or in a separate explicitly omniscient module?
