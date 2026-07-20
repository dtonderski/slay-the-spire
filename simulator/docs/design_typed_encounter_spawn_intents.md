# Typed Encounter Spawn Intents

## Problem

`TargetEncounterSpawn` carries its initial intent as a string plus an optional
damage value. Conversion into `MonsterState` recognizes a subset of labels with
an `if` chain. An unknown label silently retains the monster definition's
default intent, while some recognized attack labels substitute fixed damage
when the optional value is absent. Both cases turn incomplete or divergent
generated state into plausible combat.

## Decision

Replace the string label with a core-owned `TargetSpawnIntent` enum. Each variant
contains every value required to construct that intent. The ordinary variant
means that combat-entry AI will select the opening move; it is not an unknown or
debug fallback. Conversion into `MonsterState` exhaustively matches the enum
and never supplies representative damage.

Keep `rolled_attack_damage` only as the monster profile value used by later AI
rolls, such as a louse's rolled Bite damage. When a generated opening attack
uses that value, the generator copies it explicitly into the typed initial
intent. This preserves deterministic RNG behavior while separating future move
profile state from the complete opening intent.

## Scope and Verification

This slice changes only the generated-encounter boundary. It does not
change encounter selection, RNG consumption, or source-backed move rules.
Regression tests must prove that supported encounter states and RNG counters
remain identical, every fixed opening intent contains its required values, and
there is no string or missing-damage path that can retain or synthesize a
plausible intent.
