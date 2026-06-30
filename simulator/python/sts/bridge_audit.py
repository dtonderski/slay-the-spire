"""Shared bridge readiness/audit helpers for live collection."""

from __future__ import annotations

from typing import Any

from sts.bridge import BridgeMirror


def preflight_with_client_audit(bridge: BridgeMirror) -> dict[str, Any]:
    preflight = dict(bridge.preflight())
    try:
        clients_report = bridge.clients()
    except AttributeError:
        return preflight
    except Exception as error:
        warnings = list(preflight.get("warnings") or [])
        warnings.append(f"could not inspect bridge clients: {error}")
        preflight["warnings"] = warnings
        return preflight

    clients = clients_report.get("clients") if isinstance(clients_report, dict) else None
    if not isinstance(clients, list):
        return preflight
    alive = [client for client in clients if isinstance(client, dict) and client.get("alive")]
    active_bridge_clients = [
        client
        for client in alive
        if client.get("current") or client.get("killable") is not False
    ]
    preflight["bridge_clients"] = {
        "alive_count": len(alive),
        "active_bridge_count": len(active_bridge_clients),
        "current_pid": clients_report.get("current_pid"),
        "clients": [
            {
                "pid": client.get("pid"),
                "current": bool(client.get("current")),
                "alive": bool(client.get("alive")),
                "killable": client.get("killable"),
                "trace_paths": client.get("trace_paths") or [],
            }
            for client in clients
        ],
    }
    if len(active_bridge_clients) > 1:
        problems = list(preflight.get("problems") or [])
        problems.append(
            "multiple alive bridge clients detected: "
            + ", ".join(str(client.get("pid")) for client in active_bridge_clients)
        )
        preflight["problems"] = problems
    return preflight
