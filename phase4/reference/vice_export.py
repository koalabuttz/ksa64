#!/usr/bin/env python3
"""Run the separate C64 IEC exporter against an attached D64 image."""
from __future__ import annotations
import argparse
import json
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "phase0/reference"))
from vice_timing import (  # noqa: E402
    COMMAND_EXIT, COMMAND_PING, COMMAND_QUIT, MonitorStartupError, ViceMonitor,
    available_port, connect_monitor,
)


def run_once(vice: Path, prg: Path, disk: Path, expect_error: bool, timeout: float) -> dict:
    port = available_port()
    arguments = [
        str(vice), "-default", "-pal", "-warp", "+sound", "+confirmonexit", "+saveres",
        "-minimized", "-binarymonitor", "-binarymonitoraddress",
        f"ip4://127.0.0.1:{port}", "-autostart", str(disk),
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
            result = monitor.read_memory(0xCFF0, 0xCFF4)
            if result[:4] in (b"X4OK", b"X4ER"):
                error = result[:4] == b"X4ER"
                code = result[4]
                if error != expect_error or (error and code == 0):
                    raise RuntimeError(
                        f"export expectation error={expect_error}, result={result!r}"
                    )
                try:
                    monitor.command(COMMAND_QUIT)
                except (ConnectionError, OSError):
                    pass
                return {"passed": True, "error": error, "code": code}
            monitor.command(COMMAND_EXIT)
            time.sleep(0.02)
        raise TimeoutError("timed out waiting for C64 IEC export result")
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


def run(vice: Path, prg: Path, disk: Path, expect_error: bool, timeout: float) -> dict:
    last_error: MonitorStartupError | None = None
    for _attempt in range(3):
        try:
            return run_once(vice, prg, disk, expect_error, timeout)
        except MonitorStartupError as error:
            last_error = error
            time.sleep(0.5)
    assert last_error is not None
    raise last_error


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--disk", type=Path, required=True)
    parser.add_argument("--expect-error", action="store_true")
    parser.add_argument("--timeout", type=float, default=180.0)
    args = parser.parse_args()
    result = run(
        args.vice.resolve(strict=True), args.prg.resolve(strict=True),
        args.disk.resolve(strict=True), args.expect_error, args.timeout,
    )
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())