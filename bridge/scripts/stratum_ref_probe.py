#!/usr/bin/env python3
"""
Probe a Kaspa stratum endpoint and dump the wire handshake.

Read-only diagnostic: connects, sends a handshake, prints every line in both
directions, then disconnects. Point it at a known-good pool to capture the
reference wire, then at this bridge to confirm the shapes match.

Example (NiceHash-agent subscribe against a reference pool):
  python bridge/scripts/stratum_ref_probe.py \
    --host kas.2miners.com --port 2020 \
    --agent "NiceHash/1.0.0" --user "kaspa:YOUR_ADDR.WORKER" --pass x

Example (same probe against this bridge):
  python bridge/scripts/stratum_ref_probe.py \
    --host 127.0.0.1 --port 5555 \
    --agent "NiceHash/1.0.0" --user "kaspa:YOUR_ADDR.RK1=32768" --pass x

Example (LazyPickaxe / Eth-style login):
  python bridge/scripts/stratum_ref_probe.py --host 127.0.0.1 --port 5555 --mode login
"""

from __future__ import annotations

import argparse
import json
import socket
import sys
import time


def recv_lines(sock: socket.socket, timeout: float = 3.0) -> list[str]:
    sock.settimeout(timeout)
    buf = b""
    lines: list[str] = []
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            chunk = sock.recv(65536)
            if not chunk:
                break
            buf += chunk
            while b"\n" in buf:
                raw, buf = buf.split(b"\n", 1)
                line = raw.decode("utf-8", errors="replace").rstrip("\r")
                if line:
                    lines.append(line)
                    print(f"S->C {line}")
        except socket.timeout:
            break
    return lines


def send(sock: socket.socket, obj: dict) -> None:
    line = json.dumps(obj, separators=(",", ":"))
    print(f"C->S {line}")
    sock.sendall((line + "\n").encode("utf-8"))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--host", required=True)
    ap.add_argument("--port", type=int, default=5555)
    ap.add_argument("--mode", choices=["subscribe", "login"], default="subscribe")
    ap.add_argument("--agent", default="NiceHash/1.0.0")
    ap.add_argument(
        "--user",
        default="kaspa:REPLACE_WITH_YOUR_ADDRESS.RK1=32768",
        help="authorize username, e.g. kaspa:<addr>.<worker> or kaspa:<addr>.<worker>=16384",
    )
    ap.add_argument("--pass", dest="password", default="x")
    ap.add_argument("--wait", type=float, default=4.0, help="seconds to read after auth")
    args = ap.parse_args()

    if "REPLACE_WITH_YOUR_ADDRESS" in args.user:
        print("error: pass --user with a real kaspa address", file=sys.stderr)
        return 2

    print(f"Connecting to {args.host}:{args.port} mode={args.mode} agent={args.agent}")
    sock = socket.create_connection((args.host, args.port), timeout=10)

    if args.mode == "subscribe":
        send(
            sock,
            {
                "id": 1,
                "method": "mining.subscribe",
                "params": [args.agent],
            },
        )
        recv_lines(sock, timeout=2.0)
        send(
            sock,
            {
                "id": 2,
                "method": "mining.authorize",
                "params": [args.user, args.password],
            },
        )
    else:
        send(
            sock,
            {
                "id": 1,
                "method": "login",
                "params": {
                    "login": args.user,
                    "pass": args.password,
                    "agent": "lazypickaxe.com",
                },
            },
        )

    print(f"--- waiting {args.wait}s for notifications ---")
    recv_lines(sock, timeout=args.wait)
    sock.close()
    print("DONE")
    return 0


if __name__ == "__main__":
    sys.exit(main())
