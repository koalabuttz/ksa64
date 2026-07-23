#!/usr/bin/env python3
"""Run a KSA64 timing PRG under VICE and read its target-visible result."""

from __future__ import annotations

import argparse
import json
import socket
import struct
import subprocess
import sys
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Callable, TypeVar

API_VERSION = 0x02
COMMAND_MEMORY_GET = 0x01
COMMAND_MEMORY_SET = 0x02
COMMAND_PING = 0x81
COMMAND_EXIT = 0xAA
COMMAND_QUIT = 0xBB
TIMING_MAGIC = 0x5441534B
RESULT_START = 0xC000
RESULT_END = 0xC02C

ResultT = TypeVar("ResultT")


class MonitorStartupError(TimeoutError):
    pass


@dataclass(frozen=True)
class TimingResult:
    candidate: str
    elapsed_cycles: int
    boundary_overhead_cycles: int
    net_cycles: int
    cycles_per_step: float
    altitude_q12: int
    velocity_q24: int
    acceleration_q28: int
    mass_q12: int
    propellant_q12: int
    cutoff_events: int


class ViceMonitor:
    def __init__(self, connection: socket.socket) -> None:
        self.connection = connection
        self.request_id = 1

    def _read_exact(self, count: int) -> bytes:
        chunks = bytearray()
        while len(chunks) < count:
            chunk = self.connection.recv(count - len(chunks))
            if not chunk:
                raise ConnectionError("VICE binary monitor closed the connection")
            chunks.extend(chunk)
        return bytes(chunks)

    def _read_response(self) -> tuple[int, int, int, bytes]:
        header = self._read_exact(12)
        if header[0] != 0x02 or header[1] != API_VERSION:
            raise RuntimeError(f"Invalid VICE response header: {header.hex()}")
        body_length = struct.unpack_from("<I", header, 2)[0]
        response_type = header[6]
        error = header[7]
        request_id = struct.unpack_from("<I", header, 8)[0]
        return response_type, error, request_id, self._read_exact(body_length)

    def command(self, command_type: int, body: bytes = b"") -> bytes:
        request_id = self.request_id
        self.request_id += 1
        packet = (
            bytes((0x02, API_VERSION))
            + struct.pack("<I", len(body))
            + struct.pack("<I", request_id)
            + bytes((command_type,))
            + body
        )
        self.connection.sendall(packet)
        while True:
            response_type, error, response_id, response_body = self._read_response()
            if response_id != request_id:
                continue
            if error != 0:
                raise RuntimeError(
                    f"VICE command 0x{command_type:02x} failed with 0x{error:02x}"
                )
            if response_type != command_type:
                raise RuntimeError(
                    f"VICE command 0x{command_type:02x} returned "
                    f"response 0x{response_type:02x}"
                )
            return response_body

    def read_memory(self, start: int, end: int) -> bytes:
        body = struct.pack("<BHHBH", 0, start, end, 0, 0)
        response = self.command(COMMAND_MEMORY_GET, body)
        length = struct.unpack_from("<H", response, 0)[0]
        memory = response[2:]
        if length != len(memory) or length != end - start + 1:
            raise RuntimeError("VICE returned an unexpected memory segment length")
        return memory

    def write_memory(self, start: int, data: bytes) -> None:
        if not data or start + len(data) > 0x10000:
            raise ValueError("invalid VICE memory write")
        end = start + len(data) - 1
        body = struct.pack("<BHHBH", 0, start, end, 0, 0) + data
        self.command(COMMAND_MEMORY_SET, body)


def available_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.bind(("127.0.0.1", 0))
        return int(probe.getsockname()[1])


def connect_monitor(port: int, timeout: float) -> socket.socket:
    deadline = time.monotonic() + timeout
    last_error: OSError | None = None
    while time.monotonic() < deadline:
        connection = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        connection.settimeout(30.0)
        try:
            connection.connect(("127.0.0.1", port))
            return connection
        except OSError as error:
            last_error = error
            connection.close()
            time.sleep(0.05)
    raise MonitorStartupError(
        f"VICE binary monitor did not open port {port}: {last_error}"
    )


def parse_result(memory: bytes, expected_candidate: int, name: str) -> TimingResult | None:
    magic = struct.unpack_from("<I", memory, 0)[0]
    if magic != TIMING_MAGIC:
        return None
    schema, candidate, status = struct.unpack_from("<HHH", memory, 4)
    if schema != 1:
        raise RuntimeError(f"Unsupported timing schema {schema}")
    if candidate != expected_candidate:
        raise RuntimeError(f"Expected candidate {expected_candidate}, received {candidate}")
    if status != 0:
        raise RuntimeError(f"{name} reported final-state status {status}")
    elapsed, overhead, net = struct.unpack_from("<III", memory, 12)
    altitude, velocity, acceleration, mass, propellant = struct.unpack_from(
        "<iiiii", memory, 24
    )
    cutoff_events = memory[44]
    if net != (elapsed - overhead) & 0xFFFFFFFF:
        raise RuntimeError(f"{name} reported an inconsistent net cycle count")
    return TimingResult(
        candidate=name,
        elapsed_cycles=elapsed,
        boundary_overhead_cycles=overhead,
        net_cycles=net,
        cycles_per_step=net / 2048.0,
        altitude_q12=altitude,
        velocity_q24=velocity,
        acceleration_q28=acceleration,
        mass_q12=mass,
        propellant_q12=propellant,
        cutoff_events=cutoff_events,
    )


def _run_prg_until_result_once(
    vice: Path,
    prg: Path,
    timeout: float,
    result_start: int,
    result_end: int,
    parser: Callable[[bytes], ResultT | None],
) -> ResultT:
    port = available_port()
    arguments = [
        str(vice),
        "-default",
        "-pal",
        "-warp",
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
    startup = None
    creation_flags = 0
    if sys.platform == "win32":
        startup = subprocess.STARTUPINFO()
        startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
        startup.wShowWindow = 0
        creation_flags = subprocess.CREATE_NO_WINDOW
    process = subprocess.Popen(
        arguments,
        cwd=vice.parent,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        startupinfo=startup,
        creationflags=creation_flags,
    )
    connection: socket.socket | None = None
    try:
        connection = connect_monitor(port, min(timeout, 15.0))
        monitor = ViceMonitor(connection)
        monitor.command(COMMAND_PING)
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            memory = monitor.read_memory(result_start, result_end)
            result = parser(memory)
            if result is not None:
                try:
                    monitor.command(COMMAND_QUIT)
                except (ConnectionError, OSError):
                    pass
                return result
            monitor.command(COMMAND_EXIT)
            time.sleep(0.02)
        raise TimeoutError(f"Timed out waiting for result from {prg.name}")
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


def run_prg_until_result(
    vice: Path,
    prg: Path,
    timeout: float,
    result_start: int,
    result_end: int,
    parser: Callable[[bytes], ResultT | None],
) -> ResultT:
    last_error: MonitorStartupError | None = None
    for _attempt in range(3):
        try:
            return _run_prg_until_result_once(
                vice, prg, timeout, result_start, result_end, parser
            )
        except MonitorStartupError as error:
            last_error = error
            time.sleep(0.5)
    assert last_error is not None
    raise last_error


def run_once(
    vice: Path,
    prg: Path,
    candidate_id: int,
    candidate_name: str,
    timeout: float,
) -> TimingResult:
    return run_prg_until_result(
        vice,
        prg,
        timeout,
        RESULT_START,
        RESULT_END,
        lambda memory: parse_result(memory, candidate_id, candidate_name),
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--candidate-id", type=int, choices=(1, 2), required=True)
    parser.add_argument("--candidate-name", required=True)
    parser.add_argument("--runs", type=int, default=1)
    parser.add_argument("--timeout", type=float, default=120.0)
    args = parser.parse_args()

    vice = args.vice.resolve(strict=True)
    prg = args.prg.resolve(strict=True)
    results = [
        run_once(
            vice,
            prg,
            args.candidate_id,
            args.candidate_name,
            args.timeout,
        )
        for _ in range(args.runs)
    ]
    payload = {
        "vice": str(vice),
        "prg": str(prg),
        "runs": [asdict(result) for result in results],
        "stable": len({result.net_cycles for result in results}) == 1,
    }
    print(json.dumps(payload, indent=2))
    return 0 if payload["stable"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
