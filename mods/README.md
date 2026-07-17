# Project Mods

Small Slay the Spire mods needed by the live simulator workflow live here.

`abandon-run-control/` adds a CommunicationMod-compatible abandon command so
the collector can reset runs safely. It is project glue, not part of the Rust
simulator and not a fork of CommunicationMod. See its `install.ps1` for the
local build/install flow.
