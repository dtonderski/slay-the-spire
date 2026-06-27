# Fair Action Schema

## Purpose

This document defines the first fair action contract for a combat-first RL facade
over the Slay the Spire simulator.

The contract is intentionally separate from `sts_core` mechanics. The simulator
may use internal card IDs, monster IDs, exact draw piles, RNG state, and
debug-only transition details. A fair RL agent must act through visible slots and
visible choices, matching what a real player could select from the UI.

The immediate target is Ironclad combat. The schema should still leave room for
run-level screens, rewards, shops, events, and future Python/Gym wrappers.

## Goals

- Provide stable action descriptors for fair RL agents.
- Avoid leaking hidden simulator state through actions, masks, errors, or info.
- Keep action resolution inside the Rust facade, not in Python training code.
- Support both descriptor-based APIs and fixed-index action masks.
- Handle combat choice substates explicitly instead of treating them as normal
  card-play turns.
- Keep debug/omniscient planning separate from fair online policy APIs.

## Non-Goals

- Do not implement simulator mechanics here.
- Do not define tensor layouts or Gym spaces yet.
- Do not expose raw `CombatState`, `RunState`, `CardId`, `MonsterId`, RNG state,
  full snapshots, or debug logs in fair mode.
- Do not guarantee exact real-game parity beyond trace-verified simulator scope.
- Do not solve full-run action schemas before combat decisions are coherent.

## Core Invariant

Fair action generation must satisfy observational non-interference:

> If two authoritative simulator states differ only in hidden information, every
> fair public action descriptor, action mask, validation result, error, and
> `StepResult.info` field must be identical until that hidden information becomes
> visible through normal gameplay.

This is stricter than "do not expose draw pile order". Hidden information can
leak through descriptor ordering, action-mask shape, error text, target lists,
debug IDs, or branchable restore behavior.

## Public Action Descriptor

The fair facade exposes actions as visible screen operations. Descriptors use
visible slots and option indices. They do not contain simulator object identity.

```text
FairActionDescriptor
  EndTurn
  PlayHandSlot { hand_slot, target_slot? }
  UsePotionSlot { potion_slot, target_slot? }
  DiscardPotionSlot { potion_slot }
  ChooseVisibleOption { option_slot }
  ChooseMapNodeSlot { option_slot }
  ChooseRestOption { option_slot }
  ChooseShopSlot { option_slot }
  TakeRewardSlot { reward_slot }
  OpenCardReward
  OpenChest
  ToggleHandSlot { hand_slot }
  ToggleDiscardSlot { option_slot }
  ToggleExhaustSlot { option_slot }
  ToggleGridSlot { option_slot }
  ConfirmChoice
  CancelChoice
  SkipVisibleReward
  Proceed
  LeaveScreen
  ReturnToPreviousScreen
```

The exact Rust names may change, but the visibility boundary should not.

### EndTurn

Visible when the current substate is normal combat input and ending the turn is
available.

Internal resolution:

- maps to `CombatAction::EndTurn`

Fairness notes:

- It must not reveal hidden turn-ending consequences through descriptor text.
- It must remain ordered by visible UI convention, not by simulator internals.

### PlayHandSlot

```text
PlayHandSlot { hand_slot, target_slot? }
```

Uses the visible hand slot and an optional visible monster slot.

Internal resolution:

1. Resolve `hand_slot` against the current visible hand.
2. Resolve `target_slot` against the current visible target list.
3. Convert the resolved values to the internal `CardId` and `MonsterId`.
4. Apply the corresponding simulator action.

Fairness notes:

- The descriptor must not contain `CardId`, UUID, generated-card identity, or
  stable internal object identity.
- The descriptor must not reveal hidden top-deck information for cards such as
  Havoc.
- The target list must use visible monster slots, not internal monster IDs.
- Descriptor ordering must follow visible hand/monster order.

### UsePotionSlot

```text
UsePotionSlot { potion_slot, target_slot? }
```

Uses the visible potion slot and optional visible target slot.

Internal resolution:

- map to the existing potion/run action path after slot validation

Fairness notes:

- Generated potion choices are not visible until the game would show them.
- The mask must not reveal hidden generated alternatives before the choice
  screen opens.

### DiscardPotionSlot

```text
DiscardPotionSlot { potion_slot }
```

Uses the visible potion slot.

Fairness notes:

- Only expose when the real UI permits discarding.
- Do not expose internal potion IDs or future potion reward state.

### ChooseVisibleOption

```text
ChooseVisibleOption { option_slot }
```

Generic descriptor for visible option lists that do not yet have a more
specific descriptor family.

- potion-created card choices
- Toolbox-style start-combat choices
- discovery-style card choices
- event choices

Internal resolution:

- map the visible option slot to the active substate's internal choice action

Fairness notes:

- Only currently offered options are public.
- Do not expose unoffered alternatives or hidden generation pools.
- If a generated option has an internal ID, keep it behind the facade.

Prefer a specific descriptor when the screen type has stable semantics. For
example, use `ChooseMapNodeSlot` for map choices and `ChooseShopSlot` for shop
items rather than generic `ChooseVisibleOption`.

### ChooseMapNodeSlot

```text
ChooseMapNodeSlot { option_slot }
```

Uses the visible map-choice slot exposed by the current map screen.

Fairness notes:

- Do not expose internal `MapNodeId` or hidden future map information.
- If the full map is visible in the target UI, map topology may be part of the
  observation; future room outcomes or hidden RNG state must not be.

### ChooseRestOption

```text
ChooseRestOption { option_slot }
```

Uses the visible rest-site option slot.

Fairness notes:

- The descriptor may expose visible option labels such as rest, smith, lift, dig,
  or remove when those options are currently shown.
- Do not expose hidden relic counters beyond what the UI makes visible.

### ChooseShopSlot

```text
ChooseShopSlot { option_slot }
```

Uses the visible shop option slot for buying cards, relics, potions, opening
merchant view, removal service, or leaving/returning when those appear as
visible choices.

Fairness notes:

- Shop inventory is visible only after the player has entered the shop screen.
- Do not expose future restocks, hidden pool order, or internal shop item IDs.

### TakeRewardSlot

```text
TakeRewardSlot { reward_slot }
```

Uses the visible reward slot on reward screens.

Examples:

- take gold
- take potion
- take relic
- take boss relic

Fairness notes:

- Card rewards that open a separate card-choice screen should use
  `OpenCardReward` followed by `ChooseVisibleOption` or a future
  `ChooseCardRewardSlot`.
- Do not expose reward RNG, hidden reward queues, or internal reward IDs.

### OpenCardReward

Used when a visible combat reward includes a card reward entry that opens a card
choice screen.

Fairness notes:

- Opening the card reward is a visible UI action.
- The offered card choices become visible only after the card reward screen is
  open.

### OpenChest

Used when a visible chest screen exposes an open action.

Fairness notes:

- Do not expose chest relic identity before opening unless the target UI does.
- Do not expose treasure RNG or hidden relic pool order.

### ToggleHandSlot

```text
ToggleHandSlot { hand_slot }
```

Used for substates that ask the player to pick one or more visible cards from
hand.

Examples:

- discard hand cards
- exhaust hand cards
- put a card from hand on top of draw pile
- start-combat discard/redraw effects

The fair slot is always the absolute visible hand slot. If core logic uses a
compact selectable index, such as "all hand cards except the source card", the
facade translates from absolute visible hand slot to the compact internal index.

When a selection substate is active, the fair observation/current substate must
include a visible selection view:

```text
SelectionView
  purpose
  selected_slots
  selectable_slots
  min_count
  max_count
  can_toggle
  confirm_enabled
```

Fairness notes:

- Selection slots are visible hand slots.
- The facade resolves slots to internal IDs only when applying the action.
- The mask exposes selectable visible slots and confirmation availability, not
  hidden card identity.
- The current selected slots and selection purpose are visible because they are
  part of the active UI state.

### ToggleDiscardSlot

```text
ToggleDiscardSlot { option_slot }
```

Used only when the active UI exposes discard-pile choices.

Fairness notes:

- If the real UI exposes discard contents for the current effect, expose those
  visible option slots.
- If discard contents are not visible in the real UI, expose counts only and do
  not create card-selection descriptors.
- Use the visible option slot from the active discard-selection UI, not raw pile
  position unless the UI uses that same ordering.

### ToggleExhaustSlot

```text
ToggleExhaustSlot { option_slot }
```

Used only when the active UI exposes exhaust-pile choices.

Fairness notes:

- Same visibility rule as discard selections.
- Expose option slots, not internal card IDs.
- Do not use this descriptor for effects that exhaust cards from hand. Those use
  `ToggleHandSlot`.

### ToggleGridSlot

```text
ToggleGridSlot { option_slot }
```

Used for deck/grid selection screens such as transform, remove, smith, bottle,
Pandora's Box, Astrolabe, Calling Bell, and shop removal flows.

When a grid substate is active, the fair observation/current substate must
include a visible grid selection view:

```text
GridSelectView
  purpose
  visible_cards
  selected_slots
  selectable_slots
  min_count
  max_count
  confirm_enabled
  cancel_enabled
```

Fairness notes:

- Use visible grid option slots, not card UUIDs or internal card IDs.
- The grid purpose is visible when it corresponds to the current UI prompt.
- Do not expose hidden deck-transform outcomes before confirmation.

### ConfirmChoice

Used when the current substate stages selections before applying them.

Fairness notes:

- Enabled only when the staged visible selection satisfies the active
  `SelectionSpec`.
- Disabled or absent otherwise.

### CancelChoice

Used only when the real UI permits cancellation.

Fairness notes:

- Do not invent cancellation as a training convenience in the authoritative fair
  facade.
- Experimental wrappers may add no-op or cancellation behavior outside the
  authoritative environment, but those wrappers must be labeled.

### SkipVisibleReward

Used when a visible reward screen exposes a skip action, such as a card reward
skip.

Fairness notes:

- Only expose when the UI exposes skip.
- Do not expose hidden reward-pool or future reward state.

### Proceed

Used when the current visible screen exposes a proceed/continue action.

Examples:

- leaving a completed combat reward screen
- opening or closing non-choice transition screens
- advancing after a chest or reward pickup when no further visible choice is
  pending

Fairness notes:

- Do not use `Proceed` for hidden automatic transitions that are not player
  decisions.

### LeaveScreen

Used when the current visible screen exposes a leave action.

Examples:

- leaving Neow after selecting an option
- leaving a shop room after the player has finished interacting

Fairness notes:

- Only expose when the UI exposes leave.
- Do not use as a generic no-op.

### ReturnToPreviousScreen

Used when the current visible screen exposes a return/back action.

Examples:

- returning from shop merchant view to the shop room
- returning from an opened reward sub-screen when the real UI supports it

Fairness notes:

- Only expose when the UI exposes return/back.
- Do not invent return paths for training convenience.

## Decision Substates

The fair facade should expose exactly one current decision substate.

```text
DecisionSubstate
  NormalCombat
  PotionChoice
  ToolboxChoice
  CombatCardReward
  CardReward
  BossReward
  Map
  Rest
  ShopRoom
  ShopScreen
  Chest
  Event
  Grid
  ScreenNavigation
  HandSelect
  DiscardSelect
  ExhaustSelect
  Terminal
  Unsupported
```

Normal combat actions are disabled while a blocking choice substate is active.
Each substate owns its own descriptor list and action mask.

Some command families are ambient rather than primary substates. Potion use and
potion discard can coexist with several screens in the target UI. The fair facade
should model this as:

```text
CurrentDecision
  primary_substate
  ambient_command_families
```

`UsePotionSlot` and `DiscardPotionSlot` may appear alongside map, reward, grid,
rest, shop, and selection substates only when the visible potion slot and visible
screen affordances allow them. Potion masks must be derived from visible potion
slot fields and visible screen affordances, not from trial-and-error core
validation.

The first implementation may use fewer substates, but it should classify
unsupported substates explicitly instead of falling through to normal combat.

If the authoritative core state contains multiple blocking substates at once,
the fair facade must not guess silently. It should either:

- use a documented priority order that matches the target UI, or
- expose `Unsupported` with no normal combat descriptors and return
  `UnsupportedDecision` for attempted actions.

Until source-backed UI priority is documented, multiple simultaneous blocking
substates should be treated as unsupported in fair mode.

## Action Mask Rules

The action mask is part of the visibility boundary. It must be fair, not merely
engine-legal.

Rules:

- Generate descriptors from visible action shapes.
- Compute masks without mutating state.
- Compute masks without consuming RNG.
- Keep descriptor order deterministic and based on visible screen order.
- Do not let mask size, target options, or descriptor variants reveal hidden
  state.
- In `step`, check fair descriptor/mask membership before resolving internal
  IDs or calling simulator validation.
- Prefer coarse invalid-action outcomes when hidden-dependent validity cannot be
  known fairly.

Descriptors outside the current fair descriptor universe or masked off by the
fixed action mask must return only `NotInActionMask`. Internal simulator errors
may be mapped to public errors only after the fair descriptor was mask-legal and
only when the public error is hidden-invariant.

### Hidden-Dependent Legality

Some engine legality depends on hidden state. Havoc is the clearest early case:
the simulator knows the top draw card, but a player without Frozen Eye does not.

Fair action generation should use one of these patterns:

1. Expose the visible action shape and resolve the hidden branch internally.
2. Include every visible plausible target so the mask does not reveal which
   hidden branch is true.
3. Reject invalid hidden-dependent attempts only when the same rejection would
   occur for every hidden-equivalent state.

A coarse error is not automatically fair. If one hidden-equivalent state
succeeds and another returns even a generic `InvalidAction`, the validation
result itself has leaked hidden state.

For Havoc without Frozen Eye, the action mask must not reveal whether the top
card targets an enemy, all enemies, or no target. A conservative descriptor shape
is:

```text
PlayHandSlot { hand_slot: havoc_slot, target_slot: optional_visible_target }
```

The facade can accept only visible target shapes that have hidden-invariant
outcomes. When living visible targets exist, the safest first rule is:

- expose targeted Havoc descriptors for every visible living target;
- if the hidden top card needs a target, use the selected visible target;
- if the hidden top card ignores targets, ignore the selected visible target;
- do not also expose a no-target Havoc descriptor unless no-target would be
  accepted for every hidden-equivalent top-card mode.

If no living visible targets exist, the facade may expose a no-target Havoc
descriptor only when every hidden-equivalent top-card mode would resolve or fail
identically. Otherwise it should mask Havoc off with a hidden-invariant public
reason.

Fair Havoc extraction must not call core exact legal-action generation that
peeks at the top draw card. Exact legality belongs in debug/omniscient mode.

With Frozen Eye, top draw order is visible, so target shape may follow the
visible top card.

## Fair Action Space

There are two related but distinct action surfaces:

```text
visible_descriptors -> Vec<FairActionDescriptor>
fixed_action_mask   -> Vec<bool>
```

`visible_descriptors` is useful for inspection and descriptor-based policies. It
contains only the currently meaningful visible descriptors.

`fixed_action_mask` is useful for Gym-style discrete policies. It is a mask over
a configured public action universe with stable coordinates.

An initial combat action-space configuration can define public maxima:

```text
FairActionSpace
  max_hand_slots
  max_monster_slots
  max_potion_slots
  max_option_slots
  max_selection_slots
```

The fixed mask length is constant for a configured facade mode. Slots beyond the
current visible UI are masked off. The mask coordinate system must be documented
and derived from visible slot order, not internal IDs.

A mask over `visible_descriptors` alone is not a fixed action space; it is only a
compact list of currently legal visible choices.

## Step API Shape

The fair Rust facade should expose an environment surface like:

```text
reset(config, seed?) -> StepResult
observe() -> Observation
visible_descriptors() -> Vec<FairActionDescriptor>
fixed_action_mask() -> Vec<bool>
step(action_index | FairActionDescriptor) -> Result<StepResult, PublicError>
```

The Python/Gym wrapper can sit on top of this facade, but should not reimplement
mechanics or visibility decisions.

`fixed_action_mask()` always means the configured fixed action-space mask. A
compact mask over `visible_descriptors()` may be useful for debugging, but it is
not the primary RL mask.

## StepResult

```text
StepResult
  observation
  visible_descriptors
  fixed_action_mask
  reward_signal
  done
  terminal_reason
  public_info
```

Fair-mode `public_info` may include:

- current decision substate
- turn number if visible or public by convention
- coarse terminal reason
- coarse invalid-action reason when applicable
- reward components derived from visible/current transition signals

Fair-mode `public_info` must not include:

- full state hash
- full snapshot
- raw `CombatState` or `RunState`
- RNG seeds, counters, or logs
- internal action queue
- event log
- exact hidden validation reason
- trace verifier metadata
- captured-branch or resync scaffolding

Debug/omniscient mode may expose these fields, but it must be a separate API
surface or explicit capability that fair training code cannot accidentally use.

## Public Errors

Public errors should be coarse and non-leaking.

```text
PublicError
  NotInActionMask
  WrongSubstate
  InvalidSlot
  TargetRequired
  TargetNotAllowed
  InsufficientEnergy
  CardUnplayable
  EpisodeDone
  UnsupportedDecision
  InternalError
```

Detailed simulator errors belong in debug/omniscient mode.

`TargetRequired` and `TargetNotAllowed` may only be returned when the target
requirement is visible to the player. They must not be used to expose
hidden-dependent target rules such as an unseen Havoc top card.

Unsafe public errors:

```text
Havoc top card requires a target
Havoc top card cannot have a target
RNG counter mismatch
reward pool exhausted
unknown hidden card id
monster move history forbids this move
```

## Fair vs Debug Capabilities

Fair mode:

- no raw state objects
- no branchable full restore
- no full snapshots
- no RNG state
- no hidden IDs
- no full event logs
- reproducibility via seed/config plus visible action log

Debug/omniscient mode:

- may expose full snapshots and restore
- may expose state hashes and event logs
- may expose exact legal actions and internal IDs
- may support tree search over exact simulator states
- must be opt-in and visibly separate

Belief-state planning should bridge these worlds by keeping full particles in
the planner while exposing only fair observations to the policy.

## Mapping To Existing Core Actions

The fair facade resolves descriptors to existing simulator actions internally.

| Fair descriptor | Internal target |
| --- | --- |
| `EndTurn` | `CombatAction::EndTurn` |
| `PlayHandSlot` | `CombatAction::PlayCard { card_id, target }` |
| `UsePotionSlot` | `RunAction::UsePotion { slot, target }` or equivalent potion path |
| `DiscardPotionSlot` | `RunAction::DiscardPotion { slot }` or equivalent potion path |
| `ChooseVisibleOption` | active substate choice action |
| `ChooseMapNodeSlot` | active map-choice action |
| `ChooseRestOption` | active rest-site action |
| `ChooseShopSlot` | active shop action |
| `TakeRewardSlot` | active reward pickup action |
| `OpenCardReward` | active card-reward opening action |
| `OpenChest` | active chest-opening action |
| `ToggleHandSlot` | active hand-selection action(s), including hand cards selected for exhaust/discard effects |
| `ToggleDiscardSlot` | active discard-select action(s) |
| `ToggleExhaustSlot` | active exhaust-pile UI choice action(s), not hand cards selected for exhaust effects |
| `ToggleGridSlot` | active grid/deck-selection action(s) |
| `ConfirmChoice` | active confirm action |
| `CancelChoice` | active cancel action |
| `SkipVisibleReward` | active visible skip action |
| `Proceed` | active visible proceed/continue action |
| `LeaveScreen` | active visible leave action |
| `ReturnToPreviousScreen` | active visible return/back action |

The mapping layer owns all internal ID lookup. Python and fair RL clients should
never need to know those IDs.

## Ordering Rules

Descriptor order should be stable and screen-derived:

1. hand slots left to right
2. targets in visible monster slot order
3. potion slots left to right
4. visible choice options in UI order
5. map/rest/shop/reward/grid option slots in UI order
6. confirm/cancel controls in UI order
7. skip/proceed/leave/return controls in UI order
8. end turn in a documented fixed position

The exact order is less important than stability and visibility. Do not sort by
internal IDs, content IDs, memory addresses, hashes, or generated UUIDs.

## Test Plan

Fair-action tests should exist before Python bindings become habitual.

### Non-Mutation And RNG Tests

- `visible_descriptors()` does not mutate state.
- `fixed_action_mask()` does not mutate state.
- `observe()` does not mutate state.
- none of the above consumes RNG.

### Observational Equivalence Tests

Construct pairs of states with identical visible observations but different:

- draw pile order
- shuffle RNG counter
- monster move history
- generated internal card IDs
- reward/relic/shop pools

Assert identical:

- observations
- descriptors
- masks
- public errors
- fair `StepResult.info`

### Visibility Exception Tests

- Frozen Eye exposes draw pile order.
- Without Frozen Eye, draw pile order does not affect descriptors or masks.
- Runic Dome hides monster intent and intent damage/count.
- Havoc action masks do not reveal the hidden top card's target mode.

### Error Redaction Tests

- Hidden-dependent simulator errors are mapped to coarse public errors.
- Fair exceptions and `repr` strings do not include internal IDs, RNG data, or
  hidden validation messages.

### Descriptor Mapping Tests

- `PlayHandSlot` resolves to the current hand card at execution time.
- Dead monsters and changing target slots are handled by visible slot mapping.
- Choice substates disable normal combat descriptors.
- Confirm is enabled only when staged visible selections satisfy constraints.
- Core compact selection indices are translated from visible slots inside the
  facade.
- Hidden-equivalent states produce identical descriptor lists and fixed masks.

### Trace Command Coverage Tests

The current CommunicationMod corpus contains these command verbs:

- combat: `PLAY`, `END`, `POTION`
- visible choices: `CHOOSE`, `CONFIRM`, `CANCEL`
- screen navigation: `SKIP`, `PROCEED`, `LEAVE`, `RETURN`
- run bootstrap/polling: `START`, `STATE`

The fair combat/run facade should map the first three groups to fair
descriptors. `START` belongs to environment reset/configuration, not an ordinary
agent action. `STATE`, `WAIT`, `CLICK`, and `KEY` are bridge/UI utility commands
and should not become fair RL actions unless a future UI-level agent explicitly
targets them.

The current corpus also exercises these screen shapes:

- `NONE` for ordinary combat and some non-screen states
- `HAND_SELECT`
- `GRID`
- `COMBAT_REWARD`
- `CARD_REWARD`
- `BOSS_REWARD`
- `MAP`
- `REST`
- `SHOP_ROOM`
- `SHOP_SCREEN`
- `CHEST`
- `EVENT`
- `GAME_OVER`

Trace payloads are command/screen-shape evidence, not fair observation
templates. They can guide visible command verbs, screen types, option ordering,
and UI affordances. They must not be copied wholesale into fair observations or
`public_info`: trace fields such as seed, UUIDs, raw IDs, draw pile order without
Frozen Eye, verifier metadata, and internal/scaffolded state must pass a separate
redaction allowlist before becoming fair API output.

## Open Questions

- Should `EndTurn` appear first or last in the descriptor list?
- Should fair mode accept descriptor payloads directly, or only action indices?
- How should invalid actions be represented in Gym wrappers: error, no-op, or
  wrapper-level penalty?
- Which discard/exhaust pile contents are visible in the target UI at each
  supported choice screen?
- How much public action history should be included in observation versus
  left to recurrent policies?
- What is the first exact combat fixture for the fair action MVP?

## First Implementation Slice

The first non-mechanics slice should be an `sts_rl` facade with types and tests
only:

- define fair descriptors
- define decision substates
- define public errors
- define `StepResult`
- implement normal-combat descriptor extraction for a tiny existing fixture
- prove extraction is deterministic, non-mutating, and RNG-free
- prove at least one hidden-equivalence non-interference case before adding
  Python bindings
- keep dependency direction one-way: `sts_rl` may depend on `sts_core`, but
  `sts_core` must not depend on `sts_rl`
- do not modify simulator mechanics, core action enums, or milestone parity code
- do not reuse hidden-sensitive exact legality helpers for fair masks unless
  wrapped by visibility-specific non-interference tests
- do not add Python bindings yet
