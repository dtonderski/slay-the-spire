# Project Mods

Small Slay the Spire mods needed by the live simulator workflow live here.

`abandon-run-control/` adds a CommunicationMod-compatible abandon command so
the collector can reset runs safely. It also exposes the target game's
non-seeded `playtime_seconds` clock in CommunicationMod states so strict replay
can reproduce Secret Portal eligibility. It is project glue, not part of the
Rust simulator and not a fork of CommunicationMod. See its `install.ps1` for
the local build/install flow.

`verification-bootstrap/` adds an opt-in `START_VERIFY` command for coverage
runs. It starts an otherwise normal seeded run, then applies a declared
one-time starting current/max HP override before the first dungeon state is
published. The command and override remain visible in the captured trace, so
these instrumented runs cannot be confused with normal A0 parity evidence.

`superfastmode-collection/` is a local fork of SuperFastMode that multiplies
dungeon fades and monster death/escape animations. Upstream keeps those on
raw 1× delta; CommunicationMod then blocks collection on post-combat fades.
Installation replaces `mods/SuperFastMode.jar` in the game directory (same
modid). See its README.
