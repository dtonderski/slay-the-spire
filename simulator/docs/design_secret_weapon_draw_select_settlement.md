# Secret Weapon draw-select settlement

Status: adopted for CommunicationMod combat replay.

`SecretWeapon.use` queues the source-backed attack-from-draw action. The action
builds its filtered candidate group by iterating the draw pile and calling
`CardGroup.addToRandomSpot`, so the simulator preserves that deterministic
`cardRandomRng` insertion order; it does not add a replacement candidate or RNG
burn. The source `UseCardAction` settles the played card after the search action.

The FIDL01264 boundary confirms the command lifecycle: step 10 `PLAY 1` opens
the GRID while the draw pile is unchanged; step 11 `CHOOSE 3` closes the GRID,
moves the chosen Strike into hand, and settles Secret Weapon into exhaust. The
run-level `ChooseDrawSelect` transition therefore performs the selected-card
settlement immediately for this one-card search, while the lower-level
selection/confirmation functions remain available for explicit simulator APIs.
Deferred actions stay behind the search and resume only after source settlement.
