# Relic Definition Audit

Date: 2026-07-01

Review note: a sub-agent review was requested after the initial audit. The
reviewer returned partial findings that supported the conservative report shape:
keep the confirmed differences, caveat off-character relics as scope-bound, and
avoid false positives for relics implemented elsewhere. The report also includes
an additional local correction for the `Neow's Lament` identity-vs-behavior
distinction below.

## Scope

This audit compares local relic identity/effect definitions against the public
Slay the Spire Fandom relic table. The local inventory is centered on
`simulator/crates/sts_core/src/relic/mod.rs` and the support matrix in
`simulator/docs/m32a_relic_potion_matrix.md`.

The online baseline was fetched from the Fandom MediaWiki API:

`https://slay-the-spire.fandom.com/api.php?action=parse&page=Relics&prop=wikitext&format=json`

The parsed table contained 179 relic rows. The local `RelicKey` enum contained
145 keys. Name normalization matched 144 local keys against the online table;
`RedCirclet` was local-only.

## Confirmed Differences

### Self-Forming Clay Timing

Online definition: "Whenever you lose HP in combat, gain 3 Block next turn."

Local behavior grants the block immediately when HP loss is observed:

- `simulator/crates/sts_core/src/relic/mod.rs`: `SELF_FORMING_CLAY_BLOCK`
- `simulator/crates/sts_core/src/relic/mod.rs`: `apply_player_hp_loss_relics`
  directly adds `SELF_FORMING_CLAY_BLOCK` to `state.player.block`.

Impact: combats with incoming multi-hit attacks, same-turn follow-up attacks, or
end-of-turn block handling can diverge because the local player receives the
block in the current turn instead of at the next turn boundary.

### Red Skull Dynamic Threshold

Online definition: "While your HP is at or below 50%, you have 3 additional
Strength."

Local behavior only applies the strength during start-of-combat relic setup if
the player begins combat at or below half HP:

- `simulator/crates/sts_core/src/relic/mod.rs`: `RED_SKULL_STRENGTH`
- `simulator/crates/sts_core/src/relic/mod.rs`: `apply_start_of_combat_relics`
  checks `combat.player.hp * 2 <= combat.player.max_hp` once.

No HP-change hook was found that adds the strength when dropping below half HP
mid-combat or removes it after healing above half HP.

Impact: any combat where Ironclad crosses the 50% HP boundary after combat
start can diverge.

### Bloody Idol Gold-Heal Hook

Online definition: "Whenever you gain Gold, heal 5 HP."

Local behavior tracks Bloody Idol ownership and the Forgotten Altar swap, but
does not heal on gold gain:

- `simulator/docs/m32a_relic_potion_matrix.md`: Bloody Idol caveat says the
  on-gain-gold healing hook is not modeled.
- `simulator/crates/sts_core/src/run/state.rs`: `gain_gold` only increments
  `self.gold` when gold gain is allowed.

Impact: all positive gold-gain surfaces can under-heal when Bloody Idol is
owned, including combat rewards, events, Ceramic Fish, Maw Bank, Old Coin, Tiny
House, and gold potions or event rewards.

### Cursed Tome Book Relic Effects

Online definitions:

- Necronomicon: first 2+ cost Attack each turn is played twice; pickup grants
  Necronomicurse.
- Enchiridion: start each combat with a random Power in hand costing 0 for the
  turn.
- Nilry's Codex: end of turn choice of one of three random cards to shuffle into
  the draw pile.

Local behavior currently supports Cursed Tome rewards as key-only ownership:

- `simulator/docs/m32a_relic_potion_matrix.md`: rows for Necronomicon,
  Enchiridion, and Nilry's Codex are `inventory_only`.
- `simulator/crates/sts_core/src/run/state.rs`: `RelicKey::Necronomicon |
  RelicKey::Enchiridion | RelicKey::NilrysCodex => None`.

Impact: Cursed Tome can grant these relic keys, but their combat effects are not
modeled.

### Off-Character Starter And Replacement Relics

The local code includes identity mappings for several non-Ironclad starter or
starter-replacement relics, but their active effects are inventory-only or
no-op in the Ironclad-focused simulator:

- Cracked Core: online channels 1 Lightning at combat start.
- Frozen Core: online replaces Cracked Core and channels Frost when ending turn
  with empty orb slots.
- Pure Water: online adds a Miracle at combat start.
- Holy Water: online replaces Pure Water and adds 3 Miracles at combat start.
- Ring of the Snake: online draws 2 additional cards at combat start.
- Ring of the Serpent: online draws 1 additional card at the start of each turn.

Local evidence:

- `simulator/docs/m32a_relic_potion_matrix.md`: these rows are marked
  `inventory_only`.
- `simulator/crates/sts_core/src/run/state.rs`: keys map to `Relic` variants,
  but no character-specific orb, Miracle, or Silent draw behavior was found.

Impact: this is probably intentional for current Ironclad scope, but these local
definitions differ from the online relic effects if used outside that scope.

### Online Relics Missing From Local RelicKey

The following 35 online relic table entries were not present in local
`RelicKey`. Most are non-Ironclad class relics or event relics. Of those, 34
are listed here as absent local relic definitions:

- Boss: Hovering Kite, Wrist Blade, Inserter, Nuclear Battery, Violet Lotus.
- Common: Snecko Skull, Data Disk, Damaru.
- Uncommon: Ninja Scroll, Paper Krane, Gold-Plated Cables, Symbiotic Virus,
  Duality, Teardrop Locket.
- Rare: The Specimen, Tingsha, Tough Bandages, Emotion Chip, Cloak Clasp,
  Golden Eye.
- Shop: Twisted Funnel, Runic Capacitor, Melange.
- Event: Cultist Headpiece, Face of Cleric, Gremlin Visage, Mark of the Bloom,
  N'loth's Gift, N'loth's Hungry Face, Odd Mushroom, Red Mask, Spirit Poop,
  Ssserpent Head, Warped Tongs.

Impact: these are absent local definitions relative to the full online relic
catalog. Some may be intentionally out of scope because the simulator currently
focuses on Ironclad-reachable behavior.

`Neow's Lament` also appears in the online event relic list and is absent from
`RelicKey`, but its core reward behavior is modeled outside the relic inventory
as `neow_lament_combats_remaining` plus combat-entry HP reduction. It is
therefore not counted above as an absent gameplay mechanic, only as an absent
relic identity.

### Local-Only Red Circlet

`RedCirclet` exists locally as a boss-pool exhaustion fallback:

- `simulator/crates/sts_core/src/relic/mod.rs`: empty boss pool returns
  `RelicKey::RedCirclet`.
- `simulator/docs/m32a_relic_potion_matrix.md`: Red Circlet is marked
  `inventory_only`.

The Fandom relic table used for this audit includes `Circlet` but not
`RedCirclet`.

Impact: this is a local fallback/reference definition not represented in the
online table baseline.

## Checked And Not Reported As Differences

The following initially suspicious relics do have local behavior or an explicit
local rationale, so they should not be reported as missing effects in this
audit:

- Golden Idol: combat gold reward multiplier is modeled in
  `combat_gold_offer_with_relics`.
- Juzu Bracelet: `?` room monster outcomes are converted to events in map room
  handling.
- Tiny Chest: every fourth `?` room can be forced to Treasure by
  `apply_tiny_chest`.
- Gambling Chip: combat entry opens a discard/redraw selection.
- Toolbox: combat entry opens a three-card colorless choice.
- Prismatic Shard: card reward generation switches to any-color pool.
- Frozen Eye: intentionally modeled as information-only because the simulator
  already exposes ordered draw-pile state.

## Notes

This audit is a definition/effect comparison, not a full action-manager timing
proof. Rows marked implemented in the matrix can still have unresolved ordering,
RNG, UI, or trace-parity caveats that are outside this report.
