#!/usr/bin/env python3
"""Measure the stock-C64 Phase 10 flight endpoint with warp disabled."""

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

MAGIC = 0x30544C4B
PAL_CLOCK_HZ = 985_248


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
            0xC000,
            0xC027,
            lambda data: int.from_bytes(data[:4], "little") == MAGIC,
            180,
        )
        schema, status = struct.unpack_from("<HH", raw, 4)
        if schema != 1 or status != 0:
            raise RuntimeError(
                f"global flight timing failed: schema={schema} status={status}"
            )
        overhead, fast, aided, gnss, transition, worst, budget, checksum = (
            struct.unpack_from("<IIIIIIII", raw, 8)
        )
        raw_prg = prg.read_bytes()
        load = struct.unpack_from("<H", raw_prg, 0)[0]
        result = {
            "schema": "ksa64.phase10.global-flight-timing-v1",
            "target": "PAL stock C64 via pinned x64sc 3.10",
            "cycles": {
                "timer_overhead": overhead,
                "fast_32hz": fast,
                "aided_8hz": aided,
                "gnss_1hz": gnss,
                "frame_transition": transition,
                "worst": worst,
                "pal_32hz_release": budget,
            },
            "realtime_ratios": {
                "fast_32hz": fast / budget,
                "aided_8hz": aided / budget,
                "gnss_1hz": gnss / budget,
                "frame_transition": transition / budget,
                "worst": worst / budget,
            },
            "estimated_wall_ms": {
                "fast_32hz": 1000 * fast / PAL_CLOCK_HZ,
                "aided_8hz": 1000 * aided / PAL_CLOCK_HZ,
                "gnss_1hz": 1000 * gnss / PAL_CLOCK_HZ,
                "frame_transition": 1000 * transition / PAL_CLOCK_HZ,
            },
            "flight_checksum": f"{checksum:08x}",
            "artifact": {
                "bytes": len(raw_prg),
                "sha256": hashlib.sha256(raw_prg).hexdigest(),
                "load_address": load,
                "load_end_exclusive": load + len(raw_prg) - 2,
            },
            "warp": False,
            "realtime_requirement": False,
        }
        text = json.dumps(result, indent=2) + "\n"
        print(text, end="")
        if args.check:
            if not args.output or json.loads(args.output.read_text()) != result:
                raise RuntimeError("global flight timing evidence differs")
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
