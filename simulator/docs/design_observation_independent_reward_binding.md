# Observation-independent reward command binding

## Problem

The seed-start verifier previously inspected the post-observation's potion
count to decide whether a reward-screen `CHOOSE` command took or skipped the
selected potion. With a full potion belt, it could also reinterpret a selected
potion as the simulator's pending gold reward. Both behaviors let the observed
result choose the authoritative simulated transition.

## Binding rule

Reward command binding may use the pre-observation to translate a UI choice
index into its typed, visible reward kind. Once identified, the selected kind
maps to exactly one core action:

- `gold` takes the simulator's gold offer;
- `stolen_gold` takes the simulator's stolen-gold offer;
- `card` opens the simulator's card reward;
- `potion` takes the indexed simulator potion offer;
- `relic` takes the simulator's relic offer after checking visible identity.

The post-observation is projected and compared only after the core transition.
It cannot turn a potion pick into a skip or a gold pick. If the simulator says
the bound action is illegal, verification fails at that transition instead of
substituting a plausible different action.

## Regression contract

The binder no longer accepts a post-observation. Focused tests require an
available potion choice to take the simulator offer and a full-belt potion
choice to fail without consuming another reward. Corpus replay protects the
surrounding reward families and UI ordering bindings.

One historical retained trace sends `CHOOSE 1` for a full-belt potion and only
receives its next state 87 seconds later, after gold has instead been taken.
Installed CommunicationMod bytecode confirms that the command marks reward
index 1 (the potion) done; it does not select gold. That trace now declares the
exact reward-command boundary instead of claiming coverage through the later
externally divergent state. Expected-boundary assessment permits only the
single unsupported transition that exactly caused its declared boundary.
