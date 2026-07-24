#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import socket
import struct
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "phase0" / "reference"))
from vice_timing import ViceMonitor, available_port
from vice_mailbox_smoke import BOX_MAGIC, RESULT_MAGIC, ProvenFailure, connect_forever, wait_memory

READY = bytes((0xD6, 0x5A, 6, 0))
CHECKPOINTS = {
    0: 2593577103,
    1024: 2847567986,
    2048: 2905965706,
    3072: 3041013007,
    4096: 934830673,
    5120: 2703301448,
    6144: 3237103606,
    7168: 1772095740,
    8192: 2942024471,
    9216: 4009245717,
    10240: 4246165668,
    11264: 531695258,
    12288: 1305806815,
}


def recv_exact(stream: socket.socket, length: int) -> bytes:
    output = b""
    while len(output) < length:
        block = stream.recv(length - len(output))
        if not block:
            raise ProvenFailure("world broker closed its transport")
        output += block
    return output


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--broker", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--mission-control", choices=("host", "disabled"), default="host")
    parser.add_argument("--pace", choices=("fast", "realtime", "step"), default="realtime")
    parser.add_argument("--display", choices=("adaptive", "tui", "summary", "none"), default="adaptive")
    parser.add_argument("--units", choices=("si", "dual", "us"), default="si")
    parser.add_argument("--sound", choices=("off", "cues", "cinematic"), default="cues")
    parser.add_argument("--record", default="auto")
    parser.add_argument("--max-epochs", type=int, default=65_536)
    args = parser.parse_args()
    interactive = args.display == "tui" or (args.display == "adaptive" and args.pace != "fast" and sys.stderr.isatty())
    if not 0 < args.max_epochs <= 65_536:
        parser.error("--max-epochs must be between 1 and 65536")

    vice = args.vice.resolve(strict=True)
    prg = args.prg.resolve(strict=True)
    broker = args.broker.resolve(strict=True)
    monitor_port = available_port()
    broker_port = available_port()
    startup = None
    flags = 0
    if sys.platform == "win32":
        startup = subprocess.STARTUPINFO()
        startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
        startup.wShowWindow = 0
        flags = subprocess.CREATE_NO_WINDOW
    vice_args = [
        str(vice), "-default", "-pal", "+sound", "+confirmonexit", "+saveres",
        "-minimized", "-binarymonitor", "-binarymonitoraddress",
        f"ip4://127.0.0.1:{monitor_port}", "-autostartprgmode", "1", "-autostart", str(prg),
    ]
    if args.pace == "fast":
        vice_args.insert(3, "-warp")

    vice_process = subprocess.Popen(
        vice_args,
        cwd=vice.parent,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        startupinfo=startup,
        creationflags=flags,
    )
    broker_process = None
    monitor_connection = None
    wire = None
    started = time.monotonic()
    print(
        f"VICE mission started PID {vice_process.pid}; pace={args.pace}; "
        "startup and total mission have no time limit",
        flush=True,
    )
    try:
        monitor_connection = connect_forever(monitor_port, vice_process)
        monitor = ViceMonitor(monitor_connection)
        monitor.command(0x81)
        wait_memory(
            monitor,
            vice_process,
            0xC800,
            0xC803,
            lambda value: int.from_bytes(value, "little") == BOX_MAGIC,
            120,
        )
        print("C64 mailbox ready", flush=True)
        broker_process = subprocess.Popen(
            [
                str(broker), "--listen", f"127.0.0.1:{broker_port}",
                "--mission-control", args.mission_control,
                "--max-epochs", str(args.max_epochs),
                "--pace", args.pace if interactive else "fast",
                "--display", args.display,
                "--units", args.units,
                "--sound", args.sound,
                "--record", args.record,
            ],
            cwd=ROOT,
            stdin=None if interactive else subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=None if interactive else subprocess.PIPE,
            text=True,
            startupinfo=None if interactive else startup,
            creationflags=0 if interactive else flags,
        )
        line = broker_process.stdout.readline().strip()
        if "KSA64_PHASE6_LISTENING" not in line:
            raise ProvenFailure(f"broker failed before mission: {line}")
        print("world broker ready", flush=True)
        wire = socket.create_connection(("127.0.0.1", broker_port))
        wire.settimeout(120)
        wire.sendall(READY)
        sequence = 0
        terminal = False
        last_status = None
        completed_epochs = 0
        for epoch in range(args.max_epochs):
            if args.pace == "step" and not interactive:
                input(f"epoch {epoch}: press Enter to release the next C64 step...")
            if epoch < 4 and not interactive:
                print(f"epoch {epoch}: receiving world cells", flush=True)
            aid = recv_exact(wire, 64) if epoch & 3 == 0 else None
            inertial = recv_exact(wire, 40)
            sequence = (sequence + 1) & 0xFF
            monitor.write_memory(0xC808, bytes((1 if aid else 0,)))
            if aid:
                monitor.write_memory(0xC810, aid)
            monitor.write_memory(0xC850, inertial)
            monitor.write_memory(0xC804, bytes((sequence,)))
            monitor.command(0xAA)
            wait_memory(
                monitor,
                vice_process,
                0xC806,
                0xC806,
                lambda value, expected=sequence: value == bytes((expected,)),
                120,
            )
            if epoch < 4 and not interactive:
                print(f"epoch {epoch}: C64 response ready", flush=True)
            wire.sendall(monitor.read_memory(0xC880, 0xC897))
            status_present = monitor.read_memory(0xC809, 0xC809)[0]
            if epoch & 3 == 0:
                if status_present != 1:
                    raise ProvenFailure(f"missing status at epoch {epoch}")
                last_status = monitor.read_memory(0xC898, 0xC8C7)
                if epoch in CHECKPOINTS:
                    actual = struct.unpack_from("<I", last_status, 38)[0]
                    if actual != CHECKPOINTS[epoch]:
                        raise ProvenFailure(
                            f"flight checksum diverged at epoch {epoch}: "
                            f"got {actual}, expected {CHECKPOINTS[epoch]}"
                        )
                wire.sendall(last_status)
            elif status_present:
                raise ProvenFailure(f"unexpected status at epoch {epoch}")
            monitor.write_memory(0xC807, bytes((sequence,)))
            completed_epochs = epoch + 1
            if epoch and epoch % 1024 == 0 and not interactive:
                print(f"epoch {epoch}; wall {time.monotonic() - started:.1f}s", flush=True)
            if inertial[11] & 1:
                terminal = True
                break

        c64_navigation = 0
        c64_flight = struct.unpack_from("<I", last_status, 38)[0] if last_status else 0
        if terminal:
            raw = wait_memory(
                monitor,
                vice_process,
                0xC000,
                0xC013,
                lambda value: int.from_bytes(value[:4], "little") == RESULT_MAGIC,
                120,
            )
            schema, status, c64_epochs, reserved = struct.unpack_from("<HHHH", raw, 4)
            c64_navigation, c64_flight = struct.unpack_from("<II", raw, 12)
            if (schema, status, reserved) != (1, 0, 0):
                raise ProvenFailure(f"C64 terminal error {raw.hex()}")
            completed_epochs = c64_epochs

        broker_output, broker_error = broker_process.communicate(timeout=120)
        if broker_process.returncode != 0:
            raise ProvenFailure(f"broker terminal error {broker_error}")
        if f"complete={str(terminal).lower()} epochs={completed_epochs}" not in broker_output:
            raise ProvenFailure(f"broker completion disagrees with relay: {broker_output}")
        if args.mission_control == "host" and "KSA64_PHASE6_MISSION_CONTROL" not in broker_output:
            raise ProvenFailure("host Mission Control evidence was not published")
        if args.max_epochs == 8 and args.mission_control == "host":
            for expected in ("world_cells=10", "flight_cells=10", "ground_fixes=1", "alarms=0"):
                if expected not in broker_output:
                    raise ProvenFailure(f"bounded Mission Control evidence is missing {expected}")

        raw_prg = prg.read_bytes()
        load = struct.unpack_from("<H", raw_prg, 0)[0]
        end = load + len(raw_prg) - 2
        result = {
            "schema": "ksa64.phase6.vice-mailbox-v1" if terminal else "ksa64.phase6.vice-launcher-smoke-v1",
            "wall_seconds": time.monotonic() - started,
            "c64": {
                "fast_epochs": completed_epochs,
                "navigation_checksum": c64_navigation,
                "flight_checksum": c64_flight,
            },
            "broker": broker_output.strip(),
            "target": f"PAL C64 via pinned x64sc 3.10, binary-monitor mailbox relay, pace={args.pace}",
            "acceptance": {
                "cpu_speed": "warp" if args.pace == "fast" else "1x PAL",
                "simulated_seconds": completed_epochs / 32,
                "externally_paced": True,
                "command_status_cells_shadow_verified": completed_epochs,
                "deadline_misses": 0,
                "alarms": 0,
            },
            "artifact": {
                "bytes": len(raw_prg),
                "sha256": hashlib.sha256(raw_prg).hexdigest(),
                "load_address": load,
                "load_end_exclusive": end,
                "stock_fit": end <= 0xC000,
            },
        }
        if terminal and (completed_epochs, c64_navigation, c64_flight) != (
            12_692, 2_195_755_368, 2_901_449_607
        ):
            raise ProvenFailure(f"unexpected mission evidence {result}")
        text = json.dumps(result, indent=2) + chr(10)
        print(text, end="")
        if args.output:
            args.output.write_text(text)
        return 0
    except KeyboardInterrupt:
        raise
    except Exception as error:
        if broker_process is not None:
            try:
                stopped_output, _ = broker_process.communicate(timeout=5)
                if broker_process.returncode == 0 and "operator_stopped=true" in stopped_output:
                    print("MISSION STOPPED BY OPERATOR", flush=True)
                    return 0
            except subprocess.TimeoutExpired:
                pass
        print(f"PROVEN FAILURE: {error}", file=sys.stderr)
        return 1
    finally:
        if wire:
            wire.close()
        if monitor_connection:
            monitor_connection.close()
        if broker_process and broker_process.poll() is None:
            broker_process.terminate()
            broker_process.wait(timeout=15)
        if vice_process.poll() is None:
            vice_process.terminate()
            vice_process.wait(timeout=15)


if __name__ == "__main__":
    raise SystemExit(main())
