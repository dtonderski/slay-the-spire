# Event transform obtain lifecycle

The target event source routes Living Wall `Change`, Transmorgrifier `Pray`,
Drug Dealer `Become test subject`, and Designer `Clean Up` through a card
transform followed by `ShowCardAndObtainEffect`. The selected source leaves the
master deck at grid confirmation; the generated card is committed when the
returned event `Leave` action settles the effect. This is the same observable
lifecycle used by the existing Library/Duplicator and J.A.X./Shame obtains,
while direct event transforms without a returned Leave screen remain eager.

FIDL01359 is the boundary witness: Living Wall `CHOOSE 1` (Change), grid
`CHOOSE 3` (the fourth Strike), then `CONFIRM` produces a Leave screen with 18
cards and no Double Tap. The next Leave response (step 205) has 19 cards with
Double Tap. FIDL01248 shows the same confirm-before-Leave absence for
Transmorgrifier. FIDL01250 shows the same lifecycle for single-card Neow
transform: `CONFIRM` returns to Leave with nine cards, and Battle Trance enters
the deck only on the next Leave. Immediate Neow card obtains follow the same
rule when the next published screen is still Neow Leave: FIDL01317 queues the
curse while applying +250 gold on the option, and FIDL01337 queues the rare
card until Leave. Overlay rewards are different. FIDL01258 / FIDL01469 publish
the curse into the master deck on the first CARD_REWARD frame, matching the
existing colorless curse overlay. The simulator therefore keeps deterministic
transform/card RNG and source removal at confirm, queues only the generated
content, and flushes that queue either on the overlay-open frame or in the
owner event's Leave handler.
CommunicationMod can expose either side of the same Leave publication race: a
returned Leave frame may already show the generated card, or the first MAP
frame may still show the pre-obtain deck before the next poll settles it
(FIDL01284, FIDL01718, and the corresponding owner controls). The verifier may
compare these two source-backed projections only when the authoritative event
owner, pending content, and all other projected fields match; the simulator
state remains canonical and is never hydrated from the observation.
