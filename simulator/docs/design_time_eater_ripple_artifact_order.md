# Time Eater Ripple Artifact order

`TimeEater.takeTurn` RIPPLE queues `GainBlockAction(20)`, then
`ApplyPowerAction(VulnerablePower)`, `WeakPower`, and at A19 `FrailPower`.

One Artifact stack consumes Vulnerable. Weak still applies.

FIDL01594 END 1441: Panacea Artifact + Ripple leaves Weak, not Vulnerable.
Strike then deals 4 (Weak) not 6 (`block 16 != 14`).

Do not invert Snake Plant Spores (Frail then Weak) or Maw/Collector
Weak-then-Frail.
