# Campfire smith CM lag must still apply the upgrade

## Evidence (FIDL00375)

1. Step 662–664: player smiths **Pommel Strike** (`CHOOSE 16` + `CONFIRM`).
2. CommunicationMod keeps master-deck `Pommel Strike` unupgraded through map,
   combat rewards, and the next REST screen (steps 665–700).
3. Step 701 opens the next smith grid: observed deck shows `Pommel Strike+` and
   the grid omits Pommel (already upgraded).
4. Verifier labeled 664 `rest smith effect queued` and rolled the sim deck back
   to the pre-upgrade instances for lag matching.

## Bug

When the 1.5s smith animation window elapsed without a settled deck frame, or
the capture diverged (extra reward cards, etc.), replay **released** the pending
smith while leaving the sim on the rolled-back deck. The next smith grid was
then built with base Pommel still upgradeable → choice-list shift vs real.

## Fix

1. If the pending smith’s **post-action** observed deck matches the settled
   projection, apply the upgrade before continuing (covers 701 grid open).
2. If the player opens another campfire smith while a smith is still pending,
   apply the pending upgrade first so grid membership is authoritative.
3. On stale/window release without a settled frame, **still**
   `settle_smith_simulation` rather than dropping the upgrade.
4. At trace endpoint, apply any remaining pending smith.

## Residual on FIDL00375

Rest grid at 701 now verifies. Failure moves to shop entry (~708): gold/deck
look like a **purge** (‑75 gold, ‑1 Strike) with `purge_available=false`, but no
purge `CHOOSE` appears between chest leave and merchant open. Shop card RNG also
differs. Treat as a separate shop/collector follow-up, not a reason to keep the
smith upgrade dropped.
