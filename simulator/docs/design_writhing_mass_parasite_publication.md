# Writhing Mass Parasite publication

Writhing Mass queues an `AddCardToDeckAction` for Parasite when its Mega Debuff
triggers. The authoritative simulator keeps that obtain in the typed
`pending_combat_obtain_cards` queue and settles it on the next combat-owned
transition. CommunicationMod exposes mixed command-ready frames: FIDL01252,
FIDL01257, and FIDL01271 publish Parasite on the triggering END, while FIDL01349
publishes it on the following END.

Strict replay compares a source-backed eager-publication projection only when
the simulator has a pending combat obtain and every other combat field matches.
The authoritative state remains queued; no observed deck card is inserted into
it. The next END therefore settles the queue normally and does not duplicate
the card. This keeps the mixed bridge timing separate from gameplay mechanics.

A later PLAY can also be the publication boundary: SuperFastMode records a
command Java rejects while `AddCardToDeckAction` is still settling. The frame
shows Ceramic Fish gold and the new Parasite, and the combat hand is unchanged.
Flush the pending obtain and skip that PLAY only when the complete observed
combat subset matches (FIDL01726 PLAY 1230, FIDL01782 PLAY 1090, FIDL01572
PLAY 1091). The rejected play stays on Java's action queue; the next captured
command may resolve that leftover action instead of the recorded END or PLAY
(FIDL01782 END 1091 Pommel Strike; FIDL01572 PLAY 1092 Defend). Apply the
parked combat action only when the observed subset matches. If it matches,
park the recorded command as the next leftover so a following END can settle
the rejected play's original click (FIDL01782 discarded hand; FIDL01572
Warcry select).
