# Combust + Constricted end-turn order (FIDL00440)

## Observation

FIDL00440 end-turn kills with Combust while Constricted 10 and **Orichalcum** are
active. Real reward HP is **9053** from **9059**:

1. Orichalcum +6 block (block was 0)
2. Constricted 10 THORNS consumes block then HP → block 0, **−4 HP**
3. Combust stacks 2 LoseHP → **−2 HP**
4. Combust damage kills last enemy

Simulator previously ran Combust in `before_hand` and deferred Constricted until
after hand, then skipped Constricted on combust lethality → only **−2 HP** (9057).

## Rule

When `combust > 0`, apply Constricted in `apply_end_of_player_turn_powers_before_hand`
**before** Combust (matches older Constricted before later Combust on the power
list, and full `apply_end_of_player_turn_powers` order).

When `combust == 0`, keep Constricted **after hand** so Metallicize can absorb
Decay (FIDL00415).

Do not double-apply Constricted on the bomb-lethal pre-hand path if Combust
already triggered the pre-hand Constricted window.

## Tests

- `combust_lethal_with_orichalcum_block_absorbs_part_of_constricted` → 9053
- `combust_lethal_applies_constricted_before_combust_lose_hp` → full Constricted+Combust
