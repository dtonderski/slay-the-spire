"""Public current-act map, available independently of the active UI screen."""

from dataclasses import dataclass
from typing import Literal, get_args

from ._decode import _bool, _exact, _int, _literal, _seq

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


def decode_map_screen(value: object, path: str) -> MapScreen:
    data = _exact(value, path, MapScreen)
    return MapScreen(
        act=_int(data["act"], f"{path}.act"),
        floor=_int(data["floor"], f"{path}.floor"),
        current_node=_int(data["current_node"], f"{path}.current_node"),
        reachable_nodes=_seq(data["reachable_nodes"], f"{path}.reachable_nodes", _int),
        nodes=_seq(data["nodes"], f"{path}.nodes", decode_map_node),
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
