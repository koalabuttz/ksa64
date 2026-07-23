#!/usr/bin/env python3
"""Validate frozen and keyboard-driven stock-C64 Phase 4 pages under VICE."""
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


def ascii_code(code: int) -> str:
    if 1 <= code <= 26:
        return chr(ord("A") + code - 1)
    if 32 <= code <= 63:
        return chr(code)
    return "?"


def rows(page: bytes) -> list[str]:
    return [
        "".join(ascii_code(code) for code in page[offset:offset + 40]).rstrip()
        for offset in range(0, 1000, 40)
    ]


def require_page(page: list[str], title: str, *markers: str) -> None:
    if not page[0].startswith(title):
        raise RuntimeError(f"page title: {page[0]!r}")
    for marker in markers:
        if not any(line.startswith(marker) for line in page):
            raise RuntimeError(f"page {title!r} missing {marker!r}")
    if page[24] != "F1 CAMPAIGN F3 HIST F5 PLOT F7 STORAGE":
        raise RuntimeError(f"page footer: {page[24]!r}")

def wait_for_screen(monitor: ViceMonitor, timeout: float, predicate) -> list[str]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        page = rows(monitor.read_memory(0x0400, 0x07E7))
        if predicate(page):
            return page
        monitor.command(COMMAND_EXIT)
        time.sleep(0.02)
    raise TimeoutError("timed out waiting for interactive stock screen")


def press(monitor: ViceMonitor, key: int) -> None:
    monitor.write_memory(0x0277, bytes((key,)))
    monitor.write_memory(0x00C6, b"\x01")
    monitor.command(COMMAND_EXIT)
    time.sleep(0.02)


def run_once(vice: Path, prg: Path, timeout: float) -> dict:
    port = available_port()
    arguments = [
        str(vice), "-default", "-pal", "-warp", "+sound", "+confirmonexit", "+saveres",
        "-minimized", "-binarymonitor", "-binarymonitoraddress",
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
        campaign = None
        while time.monotonic() < deadline:
            status = monitor.read_memory(0xCFF0, 0xCFF3)
            page = rows(monitor.read_memory(0x0400, 0x07E7))
            if status == b"KSA4" and page[0].startswith("KSA64 PHASE 4 CAMPAIGN"):
                campaign = page
                break
            if status[:1] == b"E":
                raise RuntimeError(f"stock display reported error {status[1]}")
            monitor.command(COMMAND_EXIT)
            time.sleep(0.02)
        if campaign is None:
            raise TimeoutError(f"timed out waiting for stock campaign page: status={status!r} title={page[0]!r}")
        require_page(campaign, "KSA64 PHASE 4 CAMPAIGN", "RUNS  1024   SUCCESS 0857", "SUMMARY CHAIN 813CE420")

        press(monitor, 134)  # F3
        histogram = wait_for_screen(
            monitor, timeout, lambda page: page[0].startswith("KSA64 PHASE 4 OUTCOME HISTOGRAM")
        )
        require_page(histogram, "KSA64 PHASE 4 OUTCOME HISTOGRAM", "STABLE 0857", "COMPACT CLASSIFIER - ANALYZER")

        press(monitor, 135)  # F5
        trajectory = wait_for_screen(
            monitor, timeout, lambda page: page[0].startswith("KSA64 PHASE 4 BASELINE TRAJECTORY")
        )
        require_page(trajectory, "KSA64 PHASE 4 BASELINE TRAJECTORY", "ALTITUDE VS SAMPLE", "RETAINED RUNS")
        press(monitor, 29)  # cursor right
        selected = wait_for_screen(
            monitor, timeout, lambda page: len(page[21]) > 8 and page[21][8:].startswith(">0008")
        )
        press(monitor, 13)  # Return
        detail = wait_for_screen(
            monitor, timeout,
            lambda page: page[2].startswith("RUN DETAIL") and page[4].startswith("RETAINED FOR INSERTION"),
        )
        press(monitor, 136)  # F7
        storage = wait_for_screen(
            monitor, timeout, lambda page: page[0].startswith("KSA64 PHASE 4 STORAGE")
        )
        try:
            monitor.command(COMMAND_QUIT)
        except (ConnectionError, OSError):
            pass
        require_page(storage, "KSA64 PHASE 4 STORAGE", "REU REQUIRED     NO", "ARCHIVE           COMPLETE")
        return {
            "passed": True,
            "titles": [campaign[0], histogram[0], trajectory[0], storage[0]],
            "campaign": campaign[2],
            "retained": trajectory[21],
            "storage": storage[5],
            "interactive": {
                "f5": trajectory[0],
                "selected": selected[21],
                "detail": detail[4],
                "f7": storage[0],
            },
        }
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


def run(vice: Path, prg: Path, timeout: float) -> dict:
    last_error: MonitorStartupError | None = None
    for _attempt in range(3):
        try:
            return run_once(vice, prg, timeout)
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
    args = parser.parse_args()
    result = run(args.vice.resolve(strict=True), args.prg.resolve(strict=True), args.timeout)
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())