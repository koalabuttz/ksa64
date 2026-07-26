#!/usr/bin/env python3
"""Finite, one-instance, externally paced Phase 10 stock-flight probe."""

from __future__ import annotations

import argparse
import hashlib
import json
import socket
import struct
import subprocess
import sys
import time
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "phase0" / "reference"))
from vice_timing import ViceMonitor, available_port

sys.path.insert(0, str(ROOT / "phase6" / "reference"))
from vice_mailbox_smoke import ProvenFailure, connect_forever, wait_memory

BOX_MAGIC = 0x30424D4B
RESULT_MAGIC = 0x30464C4B
SESSION = 0x4B4C523A
NONE = 0xFFFFFFFF
FAST_AT = 0xC810
AID_AT = 0xC850
TRANSITION_AT = 0xC8B0
COMMAND_AT = 0xC970
STATUS_AT = 0xC9B0
SNAPSHOT_START = 0xC806
SNAPSHOT_END = 0xCA0F


def cobs_encode(data: bytes) -> bytes:
    out = bytearray([0])
    code_at = 0
    code = 1
    for value in data:
        if value == 0:
            out[code_at] = code
            code_at = len(out)
            out.append(0)
            code = 1
        else:
            out.append(value)
            code += 1
            if code == 0xFF:
                out[code_at] = code
                code_at = len(out)
                out.append(0)
                code = 1
    out[code_at] = code
    out.append(0)
    return bytes(out)


def cobs_decode(data: bytes) -> bytes:
    if not data or data[-1] != 0:
        raise ProvenFailure("unterminated KLF6 frame")
    out = bytearray()
    index = 0
    end = len(data) - 1
    while index < end:
        code = data[index]
        index += 1
        if code == 0 or index + code - 1 > end:
            raise ProvenFailure("bad COBS frame")
        out.extend(data[index : index + code - 1])
        index += code - 1
        if code != 0xFF and index < end:
            out.append(0)
    return bytes(out)


def make_frame(
    kind: int,
    sequence: int,
    measurement: int,
    production: int,
    effective: int,
    payload: bytes,
) -> bytes:
    decoded = bytearray(36 + len(payload) + 4)
    decoded[:4] = b"KLF6"
    decoded[4] = 6
    decoded[5] = kind
    struct.pack_into(
        "<HIIIIIIH",
        decoded,
        6,
        0,
        SESSION,
        sequence,
        NONE,
        measurement,
        production,
        effective,
        len(payload),
    )
    decoded[36 : 36 + len(payload)] = payload
    struct.pack_into(
        "<I",
        decoded,
        36 + len(payload),
        zlib.crc32(decoded[: 36 + len(payload)]) & 0xFFFFFFFF,
    )
    return cobs_encode(decoded)


def receive_frame(wire: socket.socket):
    encoded = bytearray()
    while True:
        value = wire.recv(1)
        if not value:
            raise ProvenFailure("broker closed KLF6 link")
        encoded += value
        if value == b"\0":
            break
    decoded = cobs_decode(bytes(encoded))
    if len(decoded) < 40 or decoded[:4] != b"KLF6" or decoded[4] != 6:
        raise ProvenFailure("bad KLF6 frame")
    kind = decoded[5]
    session, sequence, _, measurement, production, effective = struct.unpack_from(
        "<IIIIII", decoded, 8
    )
    length = struct.unpack_from("<H", decoded, 32)[0]
    if (
        session != SESSION
        or len(decoded) != 40 + length
        or zlib.crc32(decoded[:-4]) & 0xFFFFFFFF
        != struct.unpack_from("<I", decoded, len(decoded) - 4)[0]
    ):
        raise ProvenFailure("corrupt KLF6 frame")
    return (
        kind,
        sequence,
        measurement,
        production,
        effective,
        decoded[36 : 36 + length],
    )


def capabilities() -> bytes:
    output = bytearray(28)
    output[0] = 2
    output[1] = 1
    struct.pack_into("<III", output, 4, 13, 0x06010001, 0x47561001)
    struct.pack_into("<I", output, 16, 0x10520001)
    struct.pack_into("<HBBB", output, 20, 512, 32, 8, 1)
    return bytes(output)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--broker", type=Path, required=True)
    parser.add_argument("--transition-probe", action="store_true")
    parser.add_argument("--max-releases", type=int, default=33)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--vice-log", type=Path)
    args = parser.parse_args()
    if not 1 <= args.max_releases <= 65535:
        parser.error("max releases must be 1..65535")

    vice = args.vice.resolve(strict=True)
    prg = args.prg.resolve(strict=True)
    broker = args.broker.resolve(strict=True)
    monitor_port = available_port()
    broker_port = available_port()
    startup = None
    creation_flags = 0
    if sys.platform == "win32":
        startup = subprocess.STARTUPINFO()
        startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
        startup.wShowWindow = 0
        creation_flags = subprocess.CREATE_NO_WINDOW
    vice_command = [
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
        f"ip4://127.0.0.1:{monitor_port}",
        "-autostartprgmode",
        "1",
        "-autostart",
        str(prg),
    ]
    if args.vice_log:
        vice_command.extend(("-logfile", str(args.vice_log.resolve())))
    vice_process = subprocess.Popen(
        vice_command,
        cwd=vice.parent,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        startupinfo=startup,
        creationflags=creation_flags,
    )
    broker_process = None
    monitor_connection = None
    wire = None
    started = time.monotonic()
    stage = "launch"
    completed = 0
    print(
        f"VICE Phase 10 split probe PID {vice_process.pid}; one instance; "
        "warp disabled; externally paced",
        flush=True,
    )
    try:
        stage = "connect-monitor"
        monitor_connection = connect_forever(monitor_port, vice_process)
        monitor = ViceMonitor(monitor_connection)
        monitor.command(0x81)
        wait_memory(
            monitor,
            vice_process,
            0xC800,
            0xC803,
            lambda data: int.from_bytes(data, "little") == BOX_MAGIC,
            120,
        )
        print("C64 KLR10 mailbox ready", flush=True)

        stage = "launch-broker"
        broker_command = [
            str(broker),
            "--listen",
            f"127.0.0.1:{broker_port}",
            "--pace",
            "externally-paced",
            "--max-releases",
            str(args.max_releases),
        ]
        if args.transition_probe:
            broker_command.append("--transition-probe")
        broker_process = subprocess.Popen(
            broker_command,
            cwd=ROOT,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            startupinfo=startup,
            creationflags=creation_flags,
        )
        deadline = time.monotonic() + 120
        while True:
            try:
                wire = socket.create_connection(("127.0.0.1", broker_port))
                break
            except OSError:
                if broker_process.poll() is not None:
                    raise ProvenFailure(
                        f"broker exited before accepting link: {broker_process.returncode}"
                    )
                if time.monotonic() >= deadline:
                    raise ProvenFailure("broker did not listen within 120 seconds")
                time.sleep(0.05)
        wire.settimeout(180)
        wire.sendall(make_frame(2, 0, 0, 0, 0, capabilities()))
        if receive_frame(wire)[0] != 3:
            raise ProvenFailure("missing Start frame")

        sequence = 0
        while True:
            aid = None
            transition = None
            while True:
                kind, frame_sequence, measurement, _, _, payload = receive_frame(wire)
                stage = f"world-cell-{completed}"
                if kind == 10:
                    monitor.write_memory(0xC80A, b"\x01")
                    monitor.command(0xAA)
                    payload = None
                    break
                if kind != 4 or len(payload) not in (64, 96, 192):
                    raise ProvenFailure(f"unexpected world record {kind}/{len(payload)}")
                cell_kind = payload[3]
                if cell_kind == 1:
                    fast = payload
                    fast_sequence = frame_sequence
                    epoch = struct.unpack_from("<H", fast, 6)[0]
                    break
                if cell_kind == 2:
                    aid = payload
                elif cell_kind == 3:
                    transition = payload
                else:
                    raise ProvenFailure(f"unexpected KLR10 cell kind {cell_kind}")
            if payload is None:
                break

            sequence = (sequence + 1) & 0xFF
            monitor.write_memory(0xC808, bytes((1 if aid else 0,)))
            monitor.write_memory(0xC80B, bytes((1 if transition else 0,)))
            if aid:
                monitor.write_memory(AID_AT, aid)
            if transition:
                monitor.write_memory(TRANSITION_AT, transition)
            monitor.write_memory(FAST_AT, fast)
            monitor.write_memory(0xC804, bytes((sequence,)))
            monitor.command(0xAA)
            monitor_connection.close()
            monitor_connection = None
            time.sleep(4.0)

            stage = f"reconnect-c64-{completed}"
            monitor_connection = connect_forever(monitor_port, vice_process)
            monitor = ViceMonitor(monitor_connection)
            monitor.command(0x81)
            stage = f"wait-c64-{completed}"
            snapshot = wait_memory(
                monitor,
                vice_process,
                SNAPSHOT_START,
                SNAPSHOT_END,
                lambda data, expected=sequence: data[:1] == bytes((expected,)),
                120,
            )
            command_offset = COMMAND_AT - SNAPSHOT_START
            status_offset = STATUS_AT - SNAPSHOT_START
            command = snapshot[command_offset : command_offset + 64]
            wire.sendall(
                make_frame(
                    5,
                    fast_sequence + 1,
                    epoch,
                    epoch,
                    (epoch + 1) & 0xFFFF,
                    command,
                )
            )
            status_present = snapshot[0xC809 - SNAPSHOT_START]
            if epoch & 3 == 0:
                if status_present != 1:
                    raise ProvenFailure(f"missing status at epoch {epoch}")
                status = snapshot[status_offset : status_offset + 96]
                wire.sendall(
                    make_frame(7, fast_sequence + 2, epoch, epoch, NONE, status)
                )
            elif status_present:
                raise ProvenFailure(f"unexpected status at epoch {epoch}")
            completed += 1

        result_raw = wait_memory(
            monitor,
            vice_process,
            0xC000,
            0xC017,
            lambda data: int.from_bytes(data[:4], "little") == RESULT_MAGIC,
            120,
        )
        broker_output, broker_error = broker_process.communicate(timeout=180)
        expected = f"KSA64_PHASE10_BOUNDED releases={completed}"
        if broker_process.returncode != 0 or expected not in broker_output:
            raise ProvenFailure(f"broker mismatch: {broker_output} {broker_error}")
        raw_prg = prg.read_bytes()
        load = struct.unpack_from("<H", raw_prg, 0)[0]
        end = load + len(raw_prg) - 2
        result = {
            "schema": "ksa64.phase10.vice-global-flight-v1",
            "releases": completed,
            "transition_probe": args.transition_probe,
            "wall_seconds": time.monotonic() - started,
            "target": (
                "PAL stock C64 flight endpoint via one pinned VICE instance; "
                "externally paced KLF6/KLR10 step-and-ack, not realtime"
            ),
            "artifact": {
                "bytes": len(raw_prg),
                "sha256": hashlib.sha256(raw_prg).hexdigest(),
                "load_address": load,
                "load_end_exclusive": end,
                "stock_fit": end <= 0xC000,
            },
            "broker": broker_output.strip(),
            "result_raw": result_raw.hex(),
        }
        text = json.dumps(result, indent=2) + "\n"
        print(text, end="")
        if args.output:
            args.output.write_text(text)
        return 0
    except KeyboardInterrupt:
        raise
    except Exception as error:
        if broker_process:
            if broker_process.poll() is None:
                time.sleep(0.2)
            if broker_process.poll() is not None:
                broker_output, broker_error = broker_process.communicate()
                print(
                    f"BROKER OUTPUT: {broker_output} {broker_error}", file=sys.stderr
                )
        print(
            f"PROVEN FAILURE at {stage}: {error}; "
            f"vice_poll={vice_process.poll()}",
            file=sys.stderr,
        )
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
