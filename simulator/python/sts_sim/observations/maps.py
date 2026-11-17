"""Public current-act map, available independently of the active UI screen."""

from dataclasses import dataclass
from typing import Literal, cast, get_args

from ._decode import _bool, _exact, _int, _literal, _Previous, _seq

RoomKind = Literal["combat", "elite", "event", "rest", "shop", "treasure", "boss", "victory"]


@dataclass(frozen=True, slots=True, kw_only=True)
class MapNode:
    slot: int
    act: int
    room_kind: RoomKind
    burning_elite: bool
    children: tuple[int, ...]


@dataclass(frozen=True, slots=True, kw_only=True)
class MapScreen:
    act: int
    floor: int
    current_node: int
    reachable_nodes: tuple[int, ...]
    nodes: tuple[MapNode, ...]


def decode_map_screen(value: object, path: str, previous: _Previous = None) -> MapScreen:
    """``previous`` (payload, map) lets nodes with an unchanged payload be shared."""
    data = _exact(value, path, MapScreen)
    nodes = data["nodes"]
    old_nodes: tuple[object, ...] = ()
    old_decoded: tuple[MapNode, ...] = ()
    if previous is not None and previous[1] is not None:
        old_nodes = cast(tuple[object, ...], cast(dict[str, object], previous[0])["nodes"])
        old_decoded = cast(MapScreen, previous[1]).nodes
    if old_nodes and isinstance(nodes, (list, tuple)):
        decoded = tuple(
            old_decoded[index]
            if index < len(old_nodes) and node == old_nodes[index]
            else decode_map_node(node, f"{path}.nodes[{index}]")
            for index, node in enumerate(nodes)
        )
    else:
        decoded = _seq(nodes, f"{path}.nodes", decode_map_node)
    return MapScreen(
        act=_int(data["act"], f"{path}.act"),
        floor=_int(data["floor"], f"{path}.floor"),
        current_node=_int(data["current_node"], f"{path}.current_node"),
        reachable_nodes=_seq(data["reachable_nodes"], f"{path}.reachable_nodes", _int),
        nodes=decoded,
    )


def decode_map_node(value: object, path: str) -> MapNode:
    data = _exact(value, path, MapNode)
    return MapNode(
        slot=_int(data["slot"], f"{path}.slot"),
        act=_int(data["act"], f"{path}.act"),
        room_kind=_literal(data["room_kind"], f"{path}.room_kind", get_args(RoomKind)),
        burning_elite=_bool(data["burning_elite"], f"{path}.burning_elite"),
        children=_seq(data["children"], f"{path}.children", _int),
    )
