#!/usr/bin/env python3
"""Exercise preserving REU detection and DMA timing across the VICE matrix."""
from __future__ import annotations
import argparse
import json
import socket
import struct
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "phase0/reference"))
from vice_timing import (  # noqa: E402
    COMMAND_EXIT, COMMAND_PING, COMMAND_QUIT, MonitorStartupError, ViceMonitor, available_port,
    connect_monitor,
)

SIZES = [128, 256, 512, 1024, 2048, 4096, 8192, 16384]


def parse(memory: bytes, expected: int) -> dict | None:
    if memory[:4] != b"R4P0":
        return None
    version, status = struct.unpack_from("<HH", memory, 4)
    capacity, second = struct.unpack_from("<II", memory, 8)
    preserved = bool(memory[16])
    summaries, full, compact = struct.unpack_from("<HHH", memory, 18)
    used, free = struct.unpack_from("<II", memory, 24)
    cycles = list(struct.unpack_from("<HHH", memory, 32))
    repetitions = struct.unpack_from("<H", memory, 38)[0]
    if version != 1 or status != 0:
        raise RuntimeError(f"REU probe expected={expected} version={version} status={status} capacity={capacity} second={second} preserved={preserved}")
    if capacity != expected or second != expected or not preserved:
        raise RuntimeError(
            f"expected {expected} KiB, detected {capacity}/{second}, preserved={preserved}"
        )
    if expected == 0:
        if summaries != 5 or full != 0 or compact != 1 or cycles != [0, 0, 0]:
            raise RuntimeError("stock fallback plan or timing result differs")
    else:
        if summaries == 0 or used + free != expected * 1024:
            raise RuntimeError("REU storage plan does not account for capacity")
        if repetitions != 32 or not (0 < cycles[0] < cycles[1] < cycles[2]):
            raise RuntimeError(f"DMA timing is not ordered: {cycles}")
    return {
        "capacity_kib": capacity,
        "preserved": preserved,
        "summary_slots": summaries,
        "full_histories": full,
        "compact_histories": compact,
        "used_bytes": used,
        "free_bytes": free,
        "dma_total_cycles": {"64": cycles[0], "160": cycles[1], "256": cycles[2]},
        "dma_repetitions": repetitions,
    }


def _run_case_once(vice: Path, prg: Path, expected: int, timeout: float) -> dict:
    port = available_port()
    reu_args = ["+reu"] if expected == 0 else ["-reusize", str(expected), "-reu"]
    arguments = [
        str(vice), "-default", "-pal", "-warp", "+sound", "+confirmonexit", "+saveres",
        "-minimized", *reu_args, "-binarymonitor", "-binarymonitoraddress",
        f"ip4://127.0.0.1:{port}", "-autostartprgmode", "1", "-autostart", str(prg),
    ]
    startup = None
    creation_flags = 0
    if sys.platform == "win32":
        startup = subprocess.STARTUPINFO()
        startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
        startup.wShowWindow = 0
        creation_flags = subprocess.CREATE_NO_WINDOW
    process = subprocess.Popen(
        arguments, cwd=vice.parent, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL, startupinfo=startup, creationflags=creation_flags,
    )
    connection: socket.socket | None = None
    try:
        connection = connect_monitor(port, min(timeout, 15.0))
        monitor = ViceMonitor(connection)
        monitor.command(COMMAND_PING)
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            result = parse(monitor.read_memory(0xC000, 0xC03F), expected)
            if result is not None:
                try:
                    monitor.command(COMMAND_QUIT)
                except (ConnectionError, OSError):
                    pass
                return result
            monitor.command(COMMAND_EXIT)
            time.sleep(0.02)
        raise TimeoutError(f"timed out waiting for {expected} KiB REU probe")
    finally:
        if connection is not None:
            connection.close()
        try:
            process.wait(timeout=5.0)
        except subprocess.TimeoutExpired:
            process.terminate()
            try:
                process.wait(timeout=5.0)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5.0)


def run_case(vice: Path, prg: Path, expected: int, timeout: float) -> dict:
    last_error: MonitorStartupError | None = None
    for _attempt in range(3):
        try:
            return _run_case_once(vice, prg, expected, timeout)
        except MonitorStartupError as error:
            last_error = error
            time.sleep(0.5)
    assert last_error is not None
    raise last_error

def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=180.0)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    vice = args.vice.resolve(strict=True)
    prg = args.prg.resolve(strict=True)
    cases = [run_case(vice, prg, size, args.timeout) for size in [0, *SIZES]]
    payload = {"schema": "KSA64 phase4 REU matrix v1", "cases": cases}
    encoded = json.dumps(payload, indent=2) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())