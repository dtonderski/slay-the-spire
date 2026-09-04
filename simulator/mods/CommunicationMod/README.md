# CommunicationMod

Local CommunicationMod source used to expose Slay the Spire state and accept
external commands.

Upstream: <https://github.com/ForgottenArbiter/CommunicationMod>

## Requirements

- Slay the Spire
- [ModTheSpire](https://github.com/kiooeht/ModTheSpire)
- [BaseMod](https://github.com/daviscook477/BaseMod)

Copy the built JAR into the ModTheSpire mods directory and configure its external
command to launch `simulator/tools/communication/trace_client.js`. The child process must
print `ready` followed by a newline, then exchange one-line JSON states and
commands over stdin/stdout.

The protocol advertises currently legal command families in each state. Common
commands are `START`, `PLAY`, `POTION`, `END`, `CHOOSE`, `PROCEED`, `RETURN`,
`KEY`, `CLICK`, `WAIT`, and `STATE`. Do not construct commands from this list
alone; use the current state's advertised commands and choices.

This project also uses `PROFILE`, a one-time non-gameplay response carrying
persistent profile inputs such as the Note card and final-act availability.
Collectors copy it into trace metadata before `START`; replay never infers it
from later observed state.

Bridge operation and collection are documented in
`simulator/tools/communication/README.md`.
