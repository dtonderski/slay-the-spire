"""Test-only independent extraction from the ORIGINAL typed public observations."""

from collections import defaultdict

import numpy as np
from encoders.numeric import NumericBatch


def reference_batch(decisions):
    symbols = []
    tables = defaultdict(list)
    active = []

    def code(key):
        if key is None:
            return -1
        key = str(key)
        if key not in symbols:
            symbols.append(key)
        return symbols.index(key)

    def card(name, owner, value):
        d = value.dynamic
        dynamic = [
            d.rampage_damage_bonus,
            d.ritual_dagger_damage_bonus,
            d.windmill_retain_damage,
            d.steam_barrier_block_reduction,
            d.combat_cost_under_turn_override,
        ]
        tables[name].append(
            [
                owner,
                code(value.content_key),
                value.cost,
                value.upgrade_level,
                value.cost_is_modified,
                value.cost_resets_next_turn,
                value.bottled,
                value.temporary,
                *(x if x is not None else 0 for x in dynamic),
                *(x is not None for x in dynamic),
            ]
        )

    for index, decision in enumerate(decisions):
        obs = decision.observation
        tables["header"].append(
            [
                code(obs.kind),
                code(obs.phase),
                code(obs.screen.phase) if obs.kind == "combat" else -1,
                obs.context.player_hp,
                obs.context.player_max_hp,
            ]
        )
        if obs.kind != "combat" or obs.screen.phase != "waiting_for_player":
            continue
        owner = len(active)
        active.append(index)
        p = obs.screen.player
        tables["player"].append([p.hp, p.max_hp, p.block, p.energy, p.max_energy, obs.context.gold])
        for power in p.powers:
            tables["player_powers"].append([owner, code(power.key), power.amount])
        for entry in obs.screen.hand:
            card("hand", owner, entry.card)
        for name in ("draw", "discard", "exhaust"):
            for value in getattr(obs.screen, name + "_pile").cards:
                card(name, owner, value)
        for m in obs.screen.monsters:
            intent = m.intent
            damage = intent.damage if intent.visibility == "visible" else None
            hits = intent.hits if intent.visibility == "visible" else None
            enemy = len(tables["enemies"])
            tables["enemies"].append(
                [
                    owner,
                    code(m.content_key),
                    m.hp,
                    m.max_hp,
                    m.block,
                    m.alive,
                    code(m.slime_size),
                    code(intent.category if intent.visibility == "visible" else intent.visibility),
                    damage if damage is not None else 0,
                    hits if hits is not None else 0,
                    damage is not None,
                    hits is not None,
                    m.escaped,
                    m.minion,
                    m.in_defensive_mode,
                    m.stolen_gold,
                    m.stasis_card is not None,
                    m.targetable,
                ]
            )
            for power in m.powers:
                tables["enemy_powers"].append([enemy, code(power.key), power.amount])
            if m.stasis_card is not None:
                card("stasis", enemy, m.stasis_card)
        for relic in obs.context.relics:
            row = len(tables["relics"])
            tables["relics"].append([owner, code(relic.content_key)])
            for counter in relic.state:
                tables["relic_counters"].append([row, code(counter.key), counter.value])
        for potion in obs.context.potion_slots:
            tables["potions"].append([owner, code(potion.content_key), potion.slot])
        s = obs.screen.selection
        tables["selection"].append([code(s.kind if s is not None else None)])
        if s is not None:
            for option in s.options:
                card("selection_cards", owner, option.card)
                tables["selection_options"].append([owner, option.slot])
            for slot in s.selected_slots:
                tables["selected_slots"].append([owner, slot])
    packed = {name: (len(rows[0]), np.array(rows, dtype=np.int64).tobytes()) for name, rows in tables.items() if rows}
    return NumericBatch((1, symbols, packed, [d.actions for d in decisions], active))


CATEGORICAL = {
    "header": (0, 1, 2),
    "enemies": (1, 6, 7),
    "player_powers": (1,),
    "enemy_powers": (1,),
    "relics": (1,),
    "relic_counters": (1,),
    "potions": (1,),
    "selection": (0,),
    **{name: (1,) for name in ("hand", "draw", "discard", "exhaust", "stasis", "selection_cards")},
}


def semantic_tables(batch):
    output = {}
    for name, values in batch.tables.items():
        rows = values.tolist()
        for row in rows:
            for column in CATEGORICAL.get(name, ()):
                row[column] = batch.symbols[row[column]] if row[column] >= 0 else None
        output[name] = rows
    return output
