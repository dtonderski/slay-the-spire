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
Transmorgrifier. The simulator therefore keeps deterministic transform RNG and
source removal at confirm, queues only the generated content for
`EventTransformReturnToEvent`, and flushes that queue in the owner event's
Leave handler. No observation is used as authoritative state.
