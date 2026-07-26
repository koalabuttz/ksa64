#!/usr/bin/env python3
"""Run the finite SafeholdRecoveryV1 stock-C64 exactness probe."""

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

MAGIC = 0x3148534B


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--expected", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    vice = args.vice.resolve(strict=True)
    prg = args.prg.resolve(strict=True)
    expected = json.loads(args.expected.read_text())
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
    print(
        f"VICE Phase 11 safehold probe PID {process.pid}; one instance; warp disabled",
        flush=True,
    )
    try:
        connection = connect_forever(port, process)
        monitor = ViceMonitor(connection)
        monitor.command(0x81)
        raw = wait_memory(
            monitor,
            process,
            0xC000,
            0xC025,
            lambda data: int.from_bytes(data[:4], "little") == MAGIC,
            180,
        )
        schema, failures, releases = struct.unpack_from("<HHH", raw, 4)
        if schema != 1 or failures != 0:
            raise RuntimeError(
                f"safehold target probe failed: schema={schema} failures={failures}"
            )
        result = {
            "schema": "ksa64.phase11.safehold-probe-v1",
            "releases": releases,
            "failures": failures,
            "flight_checksum": f"{struct.unpack_from('<I', raw, 10)[0]:08x}",
            "navigation_checksum": f"{struct.unpack_from('<I', raw, 14)[0]:08x}",
            "command_checksum": f"{struct.unpack_from('<I', raw, 18)[0]:08x}",
            "journal_chain": f"{struct.unpack_from('<I', raw, 22)[0]:08x}",
            "drogue_epoch": struct.unpack_from("<H", raw, 26)[0],
            "main_epoch": struct.unpack_from("<H", raw, 28)[0],
            "transition_count": raw[30],
            "final_frame": raw[31],
            "safe": bool(raw[32]),
            "signature": f"{struct.unpack_from('<I', raw, 34)[0]:08x}",
        }
        if result != expected:
            raise RuntimeError(
                "host/C64 exactness mismatch:\n"
                + json.dumps({"expected": expected, "actual": result}, indent=2)
            )
        image = prg.read_bytes()
        load_address = struct.unpack_from("<H", image, 0)[0]
        evidence = {
            **result,
            "target": "PAL stock C64 via pinned x64sc 3.10",
            "artifact": {
                "bytes": len(image),
                "sha256": hashlib.sha256(image).hexdigest(),
                "load_address": load_address,
                "load_end_exclusive": load_address + len(image) - 2,
                "stock_fit": load_address + len(image) - 2 <= 0xC000,
            },
            "warp": False,
            "complete_mission": False,
        }
        text = json.dumps(evidence, indent=2) + "\n"
        print(text, end="")
        if args.output:
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
