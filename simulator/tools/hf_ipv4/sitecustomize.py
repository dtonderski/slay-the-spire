"""Prefer IPv4 for Hugging Face transfers on hosts with broken IPv6 routes."""

import socket

_getaddrinfo = socket.getaddrinfo


def _getaddrinfo_ipv4(*args, **kwargs):
    return [
        address
        for address in _getaddrinfo(*args, **kwargs)
        if address[0] == socket.AF_INET
    ]


socket.getaddrinfo = _getaddrinfo_ipv4
