# Status Definition Audit

Date: 2026-07-01

This report compares the simulator's stored combat status definitions against Slay the Spire behavior described by online wiki references and the behavior implied by the local implementation.

## Scope

Primary local definitions reviewed:

- `simulator/crates/sts_core/src/power.rs`
- `simulator/crates/sts_core/src/combat/state.rs`
- `simulator/crates/sts_core/src/combat/turn.rs`
- `simulator/crates/sts_core/src/combat/turn_powers.rs`
- `simulator/crates/sts_core/src/combat/damage.rs`
- `simulator/crates/sts_core/src/combat/transition.rs`
- `simulator/crates/sts_core/src/combat/draw.rs`
- `simulator/crates/sts_core/src/combat/legal.rs`
- `simulator/crates/sts_core/src/combat/hp_loss.rs`
- `simulator/crates/sts_core/src/run/map.rs`
- `simulator/crates/sts_core/src/content/monsters.rs`

Online references used:

- https://slay-the-spire.fandom.com/wiki/Weak
- https://slay-the-spire.fandom.com/wiki/Vulnerable
- https://slay-the-spire.fandom.com/wiki/Frail
- https://slay-the-spire.fandom.com/wiki/Intangible
- https://slay-the-spire.fandom.com/wiki/Barricade
- https://slay-the-spire.fandom.com/wiki/Malleable
- https://slay-the-spire.fandom.com/wiki/Byrd
- https://slay-the-spire.fandom.com/wiki/Fungi_Beast

## Local Status Coverage

Player powers from `PlayerPowers`:

- `strength`
- `weak`
- `dexterity`
- `frail`
- `vulnerable`
- `ritual`
- `metallicize`
- `regen`
- `thorns`
- `plated_armor`
- `artifact`
- `feel_no_pain`
- `dark_embrace`
- `barricade`
- `evolve`
- `berserk`
- `rupture`
- `juggernaut`
- `brutality`
- `mayhem`
- `combust`
- `combust_damage`
- `fire_breathing`
- `corruption`
- `magnetism`
- `panache`
- `panache_cards_played`
- `buffer`
- `intangible`
- `sadistic_nature`
- `hex`
- `confusion`
- `entangled`

Monster powers from `MonsterPowers`:

- `vulnerable`
- `weak`
- `strength`
- `artifact`
- `flight`
- `plated_armor`
- `painful_stabs`
- `ritual`
- `spikes`
- `curl_up`
- `anger`
- `metallicize`
- `malleable`
- `malleable_base`
- `spore_cloud`
- `minion`
- `strength_up`

Status-like transient combat fields reviewed:

- `cannot_draw`
- `temp_strength`
- `temp_dexterity`
- `temp_thorns`
- `temp_rage_block`
- `no_block_turns`
- `vulnerable_just_applied`
- `temp_strength_down`
- `double_tap_pending`
- `duplication_potion_pending`
- `bomb_timers`

## Findings

### Confirmed Difference: Intangible Is Applied Too Narrowly

Online behavior describes Intangible as reducing damage and HP loss to 1. Locally, the Intangible cap is applied in the monster attack path in `deal_damage_to_player`, where incoming monster attack damage is reduced to 1 before block and relic mitigation.

Several other HP-loss paths do not consult `player.powers.intangible`:

- `InternalAction::LoseHp` in `combat/transition.rs`
- Brutality and Combust HP loss in `combat/turn.rs` and `combat/turn_powers.rs`
- Spikes reflection in `combat/damage.rs`
- End-of-turn hand/status damage in `combat/hand.rs`

Impact: Intangible protects against monster attacks, but not against all damage/HP-loss sources that the online definition implies it should cap.

Recommended follow-up: introduce one shared player HP-loss/damage helper that applies Intangible consistently before Buffer and HP-loss hooks, then migrate the direct call sites.

### Confirmed Difference: Monster Plated Armor Decrements On Unmodified Damage

Online behavior describes Plated Armor as losing 1 stack after unblocked attack damage. Locally, both attack damage and unmodified damage can call `reduce_monster_plated_armor_after_hp_damage`.

The attack path in `deal_attack_damage_to_monster` is expected to decrement monster Plated Armor. The unmodified path in `deal_unmodified_damage_to_monster` also calls the same reducer, which means non-attack/unmodified damage can decrement monster Plated Armor.

Impact: Effects modeled as unmodified damage may incorrectly reduce monster Plated Armor and may trigger the local stun behavior when the stack reaches zero.

Recommended follow-up: remove the Plated Armor decrement from `deal_unmodified_damage_to_monster`, or split the reducer into attack-only and shared portions if a specific monster requires special handling.

### Suspicious/Needs Verification: `strength_up` Is Parsed But Not Consumed

`run/map.rs` maps `"Generic Strength Up Power"` into `MonsterPowers::strength_up`, but no later behavior was found that reads `strength_up`.

Impact: If this power appears in imported encounter data and is expected to add strength at a later point, the simulator currently preserves the value but does not enact it. If it is display-only or unused in supported content, this is harmless.

Recommended follow-up: verify which monsters/imported traces emit `"Generic Strength Up Power"` and whether the real game expects a future strength gain. If so, add the corresponding turn hook.

### Softened Finding: Core Scalar Status Math Appears Correct

The following local behaviors matched the checked online definitions and expected game behavior:

- Weak reduces outgoing attack damage by 25%, rounded down.
- Vulnerable increases incoming attack damage by 50%, rounded down.
- Frail reduces block gained from cards by 25%, rounded down.
- Strength and Dexterity modify attack damage and block respectively.
- Artifact blocks one debuff application.
- Barricade retains block across turns.
- Evolve draws when Status cards are drawn.
- Fire Breathing damages enemies when Status or Curse cards are drawn.
- Malleable grants block after unblocked attack damage and increments until reset.
- Flight halves attack damage and decrements after nonlethal unblocked attack damage.
- Spore Cloud applies Vulnerable on Fungi Beast death while combat continues.

## Notes

This audit focused on statuses represented directly in combat state and status-like transient fields. It did not attempt to audit every card, relic, potion, or monster move that can create those statuses except where needed to understand status behavior.

