#!/usr/bin/env python3
"""Replay native Phase 11 reference-operations vectors on one banked stock C64."""

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
from vice_timing import COMMAND_EXIT, COMMAND_QUIT, ViceMonitor, available_port

sys.path.insert(0, str(ROOT / "phase6" / "reference"))
from vice_mailbox_smoke import ProvenFailure, connect_forever

BOX = 0x0200
RESULT = 0x0410
BOX_MAGIC = 0x31424D4B
RESULT_MAGIC = 0x31464C4B
RECORD_LENGTH = 1056


def load_prg(path: Path) -> tuple[int, bytes]:
    image = path.resolve(strict=True).read_bytes()
    if len(image) < 3:
        raise ValueError(f"empty PRG {path}")
    return struct.unpack_from("<H", image)[0], image[2:]


def load_and_validate_manifest(
    image_dir: Path, origins: dict[str, int], payloads: dict[str, bytes]
) -> dict:
    path = image_dir / "reference-ops-banked-manifest.json"
    manifest = json.loads(path.read_text())
    if manifest.get("schema") != "ksa64.phase11.reference-ops-banked-bundle.v1":
        raise ValueError("unexpected banked manifest schema")
    if manifest.get("entry") != "0x080d":
        raise ValueError("unexpected banked manifest entry")
    source = (ROOT / manifest.get("source_bundle", "")).resolve(strict=True)
    source_bytes = source.read_bytes()
    source_sha = hashlib.sha256(source_bytes).hexdigest()
    if manifest.get("bundle_bytes") != len(source_bytes) or manifest.get("bundle_sha256") != source_sha:
        raise ValueError("banked source bundle identity mismatch")

    expected_names = ("extra", "main", "high")
    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, list) or len(artifacts) != len(expected_names):
        raise ValueError("banked manifest artifact catalogue mismatch")
    for name, artifact in zip(expected_names, artifacts):
        if artifact.get("name") != name or artifact.get("file") != f"reference-ops-{name}.prg":
            raise ValueError(f"banked manifest {name} identity mismatch")
        prg = (image_dir / artifact["file"]).resolve(strict=True).read_bytes()
        origin = origins[name]
        payload = payloads[name]
        if prg != struct.pack("<H", origin) + payload:
            raise ValueError(f"banked manifest {name} PRG mismatch")
        if artifact.get("load_address") != f"0x{origin:04x}":
            raise ValueError(f"banked manifest {name} address mismatch")
        if artifact.get("load_end_exclusive") != f"0x{origin + len(payload):04x}":
            raise ValueError(f"banked manifest {name} end mismatch")
        if artifact.get("payload_bytes") != len(payload):
            raise ValueError(f"banked manifest {name} length mismatch")
        if artifact.get("sha256") != hashlib.sha256(prg).hexdigest():
            raise ValueError(f"banked manifest {name} hash mismatch")
        capacity = artifact.get("capacity_bytes")
        margin = artifact.get("margin_bytes")
        if not isinstance(capacity, int) or capacity - len(payload) != margin:
            raise ValueError(f"banked manifest {name} capacity mismatch")
    return manifest


def load_transcript(path: Path) -> list[dict]:
    raw = path.resolve(strict=True).read_bytes()
    if len(raw) < 16 or raw[:4] != b"KOT1":
        raise ValueError("invalid KOT1 transcript")
    version, record_length, count, header_length = struct.unpack_from("<HHHH", raw, 4)
    if (version, record_length, header_length) != (1, RECORD_LENGTH, 16):
        raise ValueError("unsupported KOT1 contract")
    if len(raw) != 16 + count * record_length:
        raise ValueError("truncated or extended KOT1 transcript")
    expected_crc = struct.unpack_from("<I", raw, 12)[0]
    if zlib.crc32(raw[16:]) & 0xFFFFFFFF != expected_crc:
        raise ValueError("KOT1 transcript CRC mismatch")
    records = []
    for index in range(count):
        offset = 16 + index * record_length
        cell = raw[offset : offset + record_length]
        operation, flags, available = cell[0], cell[1], bool(cell[2])
        if cell[3] or cell[14] or cell[15]:
            raise ValueError(f"record {index} reserved bytes are nonzero")
        aux = struct.unpack_from("<I", cell, 4)[0]
        input_length, output_length, epoch = struct.unpack_from("<HHH", cell, 8)
        navigation, flight, command, output_crc = struct.unpack_from("<IIII", cell, 16)
        if input_length > 512 or output_length > 512:
            raise ValueError(f"record {index} length is out of range")
        output = cell[544:1056]
        if zlib.crc32(output[:output_length]) & 0xFFFFFFFF != output_crc:
            raise ValueError(f"record {index} output CRC mismatch")
        records.append(
            {
                "operation": operation,
                "flags": flags,
                "available": available,
                "aux": aux,
                "input_length": input_length,
                "output_length": output_length,
                "epoch": epoch,
                "navigation": navigation,
                "flight": flight,
                "command": command,
                "input": cell[32:544],
                "output": output,
            }
        )
    return records


def command_accept(
    monitor: ViceMonitor, command_type: int, body: bytes, response_types: tuple[int, ...]
) -> bytes:
    request_id = monitor.request_id
    monitor.request_id += 1
    packet = (
        bytes((0x02, 0x02))
        + struct.pack("<I", len(body))
        + struct.pack("<I", request_id)
        + bytes((command_type,))
        + body
    )
    monitor.connection.sendall(packet)
    while True:
        response_type, error, response_id, response_body = monitor._read_response()
        if response_id != request_id:
            continue
        if error != 0:
            raise RuntimeError(
                f"VICE command 0x{command_type:02x} failed with 0x{error:02x}"
            )
        if response_type not in response_types:
            raise RuntimeError(
                f"VICE command 0x{command_type:02x} returned 0x{response_type:02x}"
            )
        return response_body


def register_id(monitor: ViceMonitor, name: str) -> int:
    response = monitor.command(0x83, b"\x00")
    count = struct.unpack_from("<H", response)[0]
    offset = 2
    for _ in range(count):
        item_length = response[offset]
        item = response[offset + 1 : offset + 1 + item_length]
        if len(item) >= 3:
            label_length = item[2]
            label = item[3 : 3 + label_length].decode("ascii")
            if label.upper() == name.upper():
                return item[0]
        offset += 1 + item_length
    raise RuntimeError(f"VICE did not expose register {name}")


def register_value(monitor: ViceMonitor, register: int) -> int:
    response = monitor.command(0x31, b"\x00")
    count = struct.unpack_from("<H", response)[0]
    offset = 2
    for _ in range(count):
        item_length = response[offset]
        item = response[offset + 1 : offset + 1 + item_length]
        if item and item[0] == register and len(item) >= 3:
            return struct.unpack_from("<H", item, 1)[0]
        offset += 1 + item_length
    raise RuntimeError(f"VICE omitted register {register}")


def set_register(monitor: ViceMonitor, register: int, value: int) -> None:
    body = b"\x00" + struct.pack("<H", 1) + bytes((3, register)) + struct.pack("<H", value)
    command_accept(monitor, 0x32, body, (0x31, 0x32))


def current_pc(monitor: ViceMonitor) -> int:
    return register_value(monitor, register_id(monitor, "PC"))


def wait_ready(monitor: ViceMonitor, process: subprocess.Popen) -> None:
    last_notice = time.monotonic()
    while True:
        if process.poll() is not None:
            raise ProvenFailure(f"VICE exited during endpoint initialization with {process.returncode}")
        result = monitor.read_memory(RESULT, RESULT + 23)
        if struct.unpack_from("<I", result)[0] == RESULT_MAGIC:
            status, code = struct.unpack_from("<HH", result, 4)
            raise ProvenFailure(f"endpoint stopped during initialization: status={status} code={code}")
        box = monitor.read_memory(BOX, BOX + 15)
        if struct.unpack_from("<I", box)[0] == BOX_MAGIC and box[6] == 1:
            return
        monitor.command(COMMAND_EXIT)
        if time.monotonic() - last_notice >= 60:
            pc = current_pc(monitor)
            print(
                f"still initializing banked endpoint; PC=${pc:04x}; "
                f"mailbox={box.hex()}; run remains active",
                flush=True,
            )
            last_notice = time.monotonic()
        time.sleep(0.02)


def wait_ack(
    monitor: ViceMonitor,
    process: subprocess.Popen,
    sequence: int,
    operation: int,
) -> float:
    started = time.monotonic()
    last_notice = started
    while True:
        if process.poll() is not None:
            raise ProvenFailure(f"VICE exited during operation {operation} with {process.returncode}")
        result = monitor.read_memory(RESULT, RESULT + 23)
        if struct.unpack_from("<I", result)[0] == RESULT_MAGIC:
            status, code = struct.unpack_from("<HH", result, 4)
            raise ProvenFailure(f"endpoint stopped in operation {operation}: status={status} code={code}")
        if monitor.read_memory(BOX + 5, BOX + 5)[0] == sequence:
            return time.monotonic() - started
        monitor.command(COMMAND_EXIT)
        elapsed = time.monotonic() - started
        if elapsed - (last_notice - started) >= 60:
            print(
                f"operation {operation} still running after {elapsed:.0f}s; run remains active",
                flush=True,
            )
            last_notice = time.monotonic()
        if elapsed >= 300:
            state = monitor.read_memory(BOX, BOX + 15).hex()
            raise ProvenFailure(
                f"bounded ready endpoint made no acknowledgment for 300s in operation "
                f"{operation}; mailbox={state}"
            )
        time.sleep(0.02)


def changed_stack_bytes(raw: bytes, fill: int = 0xA5) -> int:
    changed = [index for index, value in enumerate(raw) if value != fill]
    return 0 if not changed else len(raw) - min(changed)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--image-dir", type=Path, required=True)
    parser.add_argument("--transcript", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    vice = args.vice.resolve(strict=True)
    image_dir = args.image_dir.resolve(strict=True)
    records = load_transcript(args.transcript)
    origins = {}
    payloads = {}
    for name in ("extra", "main", "high"):
        origins[name], payloads[name] = load_prg(image_dir / f"reference-ops-{name}.prg")
    if origins != {"extra": 0x053F, "main": 0x0801, "high": 0xE1FE}:
        raise ValueError(f"unexpected segment origins {origins}")
    manifest = load_and_validate_manifest(image_dir, origins, payloads)

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
        "-initbreak",
        "ready",
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
    connection: socket.socket | None = None
    clean = False
    print(
        f"VICE Phase 11 banked endpoint PID {process.pid}; one instance; warp disabled",
        flush=True,
    )
    try:
        # Let VICE reach the BASIC ready breakpoint before direct bank injection.
        time.sleep(2.0)
        if process.poll() is not None:
            raise ProvenFailure(f"VICE exited before the entry breakpoint with {process.returncode}")
        connection = connect_forever(port, process)
        monitor = ViceMonitor(connection)
        monitor.command(0x81)
        # At the BASIC ready stop, inject the visible banks, map all RAM,
        # verify the hidden bank, and set PC=$080d directly.
        monitor.write_memory(origins["extra"], payloads["extra"])
        monitor.write_memory(origins["main"], payloads["main"])
        # A side-effecting CPU-port write makes the high bank visible before load.
        port_body = struct.pack("<BHHBH", 1, 0x0001, 0x0001, 0, 0) + b"\x34"
        monitor.command(0x02, port_body)
        if monitor.read_memory(0x0001, 0x0001) != b"\x34":
            raise ProvenFailure("VICE did not apply the all-RAM CPU-port mapping")
        monitor.write_memory(origins["high"], payloads["high"])
        if monitor.read_memory(origins["high"], origins["high"] + len(payloads["high"]) - 1) != payloads["high"]:
            raise ProvenFailure("hidden high bank did not load exactly")
        monitor.write_memory(0x0428, bytes([0xA5]) * (0x053F - 0x0428))
        main_end = origins["main"] + len(payloads["main"])
        high_end = origins["high"] + len(payloads["high"])
        monitor.write_memory(main_end, bytes([0x5A]) * (0xC000 - main_end))
        monitor.write_memory(high_end, bytes([0x3C]) * (0x10000 - high_end))
        set_register(monitor, register_id(monitor, "PC"), 0x080D)
        monitor.command(COMMAND_EXIT)
        wait_ready(monitor, process)
        if monitor.read_memory(0x0001, 0x0001) != b"\x34":
            raise ProvenFailure("endpoint did not retain all-RAM banking")

        timings = []
        for index, record in enumerate(records, 1):
            monitor.write_memory(BOX + 16, record["input"])
            monitor.write_memory(BOX + 12, struct.pack("<I", record["aux"]))
            monitor.write_memory(BOX + 8, bytes((record["flags"],)))
            monitor.write_memory(BOX + 7, bytes((record["operation"],)))
            monitor.write_memory(BOX + 10, b"\x00")
            monitor.write_memory(BOX + 4, bytes((index,)))
            monitor.command(COMMAND_EXIT)
            elapsed = wait_ack(monitor, process, index, record["operation"])
            timings.append({"record": index, "operation": record["operation"], "wall_seconds": round(elapsed, 3)})
            box = monitor.read_memory(BOX, BOX + 15)
            result = monitor.read_memory(RESULT, RESULT + 23)
            actual_available = bool(box[9])
            actual_result = struct.unpack_from("<HxxIII", result, 8)
            expected_result = (
                record["epoch"],
                record["navigation"],
                record["flight"],
                record["command"],
            )
            if actual_available != record["available"]:
                raise ProvenFailure(
                    f"record {index} availability mismatch: {actual_available} != {record['available']}"
                )
            if actual_result != expected_result:
                raise ProvenFailure(
                    f"record {index} result mismatch: {actual_result} != {expected_result}"
                )
            length = record["output_length"]
            if length:
                actual = monitor.read_memory(BOX + 16, BOX + 15 + length)
                expected = record["output"][:length]
                if actual != expected:
                    raise ProvenFailure(
                        f"record {index} payload mismatch: "
                        f"actual={hashlib.sha256(actual).hexdigest()} "
                        f"expected={hashlib.sha256(expected).hexdigest()}"
                    )

        stack = monitor.read_memory(0x0428, 0x053E)
        main_margin = monitor.read_memory(main_end, 0xBFFF)
        high_margin = monitor.read_memory(high_end, 0xFFFF)
        if main_margin != bytes([0x5A]) * len(main_margin):
            raise ProvenFailure("normal-RAM segment overran its guard")
        if high_margin != bytes([0x3C]) * len(high_margin):
            raise ProvenFailure("high segment overran its guard")
        for name in ("extra", "main", "high"):
            actual = monitor.read_memory(origins[name], origins[name] + len(payloads[name]) - 1)
            if actual != payloads[name]:
                raise ProvenFailure(f"{name} code segment changed during execution")

        transcript_bytes = args.transcript.resolve(strict=True).read_bytes()
        evidence = {
            "schema": "ksa64.phase11.reference-ops-banked-vice.v1",
            "target": "PAL stock C64 via pinned x64sc 3.10",
            "warp": False,
            "reu_required": False,
            "records": len(records),
            "transcript_sha256": hashlib.sha256(transcript_bytes).hexdigest(),
            "bundle_sha256": manifest["bundle_sha256"],
            "entry": manifest["entry"],
            "banking": manifest["banking"],
            "emergency_stack_capacity_bytes": len(stack),
            "emergency_stack_high_water_bytes": changed_stack_bytes(stack),
            "segment_guards_preserved": True,
            "code_segments_preserved": True,
            "final_epoch": records[-1]["epoch"],
            "navigation_checksum": f"{records[-1]['navigation']:08x}",
            "flight_checksum": f"{records[-1]['flight']:08x}",
            "command_checksum": f"{records[-1]['command']:08x}",
            "operation_wall_seconds": timings,
            "timing_claim": "diagnostic host wall time only; no PAL realtime claim",
            "complete_mission": False,
        }
        text = json.dumps(evidence, indent=2) + "\n"
        print(text, end="")
        if args.output:
            args.output.write_text(text)
        clean = True
        try:
            monitor.command(COMMAND_QUIT)
        except (ConnectionError, OSError):
            pass
        return 0
    except KeyboardInterrupt:
        clean = True
        raise
    except Exception as error:
        print(f"PROVEN FAILURE: {error}", file=sys.stderr)
        clean = True
        return 1
    finally:
        if connection:
            connection.close()
        if clean and process.poll() is None:
            process.terminate()
            process.wait(timeout=15)
        if not clean and process.poll() is None:
            print(
                f"unclassified interruption; VICE PID {process.pid} left running",
                file=sys.stderr,
            )


if __name__ == "__main__":
    raise SystemExit(main())
