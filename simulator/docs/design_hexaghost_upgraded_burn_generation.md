# Hexaghost upgraded Burn generation

## Evidence

The retained `trace-session-15` Hexaghost combat captures Inferno followed by
later Sear moves. Before Inferno, existing Burns report `upgrades = 0`. After a
survived Inferno, all existing Burns and its three generated Burns report
`upgrades = 1`. A later Sear creates a new UUID that also immediately reports
`upgrades = 1`.

This was previously invisible because verifier card projections erased upgrade
metadata. Once upgrade identity became observable, the permanent corpus showed
the later generated Burn as the sole mismatch.

## Contract

Hexaghost records whether a survived Inferno has activated upgraded Burn
generation. Inferno still applies its post-damage upgrade/add effect only when
the player survives the damage sequence. Once activated, later Hexaghost Sear
moves add upgraded Burns directly. Other monsters that use the same generic
`AddBurnToDiscard` intent, including Nemesis, continue to add ordinary Burns.

The flag is authoritative combat state, serialized with snapshots, and valid
only on Hexaghost. Older snapshots default it to false. This explicit state is
necessary because inferring it from move count would be wrong when Inferno is
lethal and post-damage effects do not resolve.

## Failure behavior

Upgraded Burn generation uses the same checked card-instance allocation and
capacity handling as other generated cards. Invalid allocation fails the
combat transition without substituting a normal Burn.
