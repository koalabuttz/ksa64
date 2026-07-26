#!/usr/bin/env python3
"""Finite one-instance stock-C64 Phase 10 replay acceptance probe."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "phase0" / "reference"))
from vice_timing import ViceMonitor, available_port

sys.path.insert(0, str(ROOT / "phase6" / "reference"))
from vice_mailbox_smoke import connect_forever, wait_memory

MAGIC = 0x3042503A


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    vice = args.vice.resolve(strict=True)
    prg = args.prg.resolve(strict=True)
    port = available_port()
    startup = None
    flags = 0
    if sys.platform == "win32":
        startup = subprocess.STARTUPINFO()
        startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
        startup.wShowWindow = 0
        flags = subprocess.CREATE_NO_WINDOW
    command = [
        str(vice),
        "-default",
        "-pal",
        "+warp",
        "+sound",
        "+confirmonexit",
        "+saveres",
        "-minimized",
        "-binarymonitor",
        "-binarymonitoraddress",
        f"ip4://127.0.0.1:{port}",
        "-autostartprgmode",
        "1",
        "-autostart",
        str(prg),
    ]
    process = subprocess.Popen(
        command,
        cwd=vice.parent,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        startupinfo=startup,
        creationflags=flags,
    )
    connection = None
    try:
        connection = connect_forever(port, process)
        monitor = ViceMonitor(connection)
        monitor.command(0x81)
        raw = wait_memory(
            monitor,
            process,
            0xC800,
            0xC813,
            lambda data: int.from_bytes(data[:4], "little") == MAGIC,
            120,
        )
        status, code, points = struct.unpack_from("<HHH", raw, 4)
        if status != 0:
            raise RuntimeError(f"stock replay failed with code {code}")
        outcome = raw[10]
        transition_mask = raw[11]
        evaluation, cue_hash = struct.unpack_from("<II", raw, 12)
        screen = monitor.read_memory(0x0400, 0x07E7)
        if transition_mask != 0x0F or points != 128:
            raise RuntimeError("stock replay did not retain all transitions")
        raw_prg = prg.read_bytes()
        load = struct.unpack_from("<H", raw_prg, 0)[0]
        end = load + len(raw_prg) - 2
        result = {
            "schema": "ksa64.phase10.stock-replay-vice-v1",
            "status": status,
            "code": code,
            "points": points,
            "outcome": outcome,
            "transition_mask": f"{transition_mask:02x}",
            "evaluation_identity": f"{evaluation:08x}",
            "cue_hash": f"{cue_hash:08x}",
            "screen_sha256": hashlib.sha256(screen).hexdigest(),
            "artifact": {
                "bytes": len(raw_prg),
                "sha256": hashlib.sha256(raw_prg).hexdigest(),
                "load_address": load,
                "load_end_exclusive": end,
                "stock_fit": end <= 0xC000,
            },
            "reu_required": False,
            "warp": False,
        }
        text = json.dumps(result, indent=2) + "\n"
        print(text, end="")
        if args.check:
            if not args.output or json.loads(args.output.read_text()) != result:
                raise RuntimeError("stock replay evidence differs")
        elif args.output:
            args.output.write_text(text)
        return 0
    finally:
        if connection:
            connection.close()
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=15)


if __name__ == "__main__":
    raise SystemExit(main())
