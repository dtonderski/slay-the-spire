# Verification Bootstrap

This opt-in ModTheSpire mod adds a CommunicationMod command for instrumented
simulator-coverage runs:

```text
START_VERIFY IRONCLAD 0 <seed> 1000
```

The command delegates character, ascension, and seed handling to
CommunicationMod's ordinary `START` implementation. After the new dungeon is
initialized, the mod sets both `maxHealth` and `currentHealth` to the declared
value. The override consumes no gameplay RNG.

The starting HP is applied once. An ordinary `START` clears any pending or
active override and continues to start a normal run. The mod also adds
`verification_starting_hp` to CommunicationMod game-state payloads while an
instrumented run is active.

These traces are coverage evidence, not normal-HP parity evidence. The trace
action itself contains the override, so the verifier must initialize its run
from `START_VERIFY`; it must never infer or hydrate HP from a later observed
state.

## Build and install

From PowerShell:

```powershell
.\install.ps1
```

Use `-NoInstall` to build and run the focused tests without copying the jar
into the Slay the Spire `mods` directory.
