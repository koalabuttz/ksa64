#!/usr/bin/env python3
"""Minimal deterministic client for the KSA64 Phase 9 JSONL evaluator."""
from __future__ import annotations

import argparse
import json
import struct
import subprocess
from pathlib import Path


def parse_variables(manifest: bytes) -> list[tuple[int, int, int, int]]:
    if len(manifest) != 2048 or manifest[:4] != b"KOM9":
        raise ValueError("not a KOM9 manifest")
    count = manifest[66]
    variables = []
    for index in range(count):
        offset = 96 + index * 20
        variable_id = struct.unpack_from("<H", manifest, offset)[0]
        minimum, maximum = struct.unpack_from("<ii", manifest, offset + 4)
        quantum = struct.unpack_from("<I", manifest, offset + 12)[0]
        variables.append((variable_id, minimum, maximum, quantum))
    return variables


def midpoint(variable: tuple[int, int, int, int]) -> int:
    _, minimum, maximum, quantum = variable
    steps = (maximum - minimum) // quantum
    return minimum + (steps // 2) * quantum


def exchange(process: subprocess.Popen[str], message: dict) -> dict:
    assert process.stdin is not None and process.stdout is not None
    process.stdin.write(json.dumps(message, separators=(",", ":")) + "\n")
    process.stdin.flush()
    line = process.stdout.readline()
    if not line:
        raise RuntimeError("KSA64 evaluator closed unexpectedly")
    return json.loads(line)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase9", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--transcript", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    variables = parse_variables(args.manifest.read_bytes())
    command = [str(args.phase9), "serve-kom9", str(args.manifest)]
    if args.transcript:
        command.append(str(args.transcript))
    process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    responses = []
    try:
        responses.append(exchange(process, {"kind": "hello", "request_id": 1}))
        values = {str(variable[0]): midpoint(variable) for variable in variables}
        responses.append(exchange(process, {"kind": "evaluate", "request_id": 2, "tier": 8, "values": values}))
        if variables:
            variable_id, minimum, _, _ = variables[0]
            values[str(variable_id)] = minimum
        responses.append(exchange(process, {"kind": "evaluate", "request_id": 3, "tier": 8, "values": values}))
        responses.append(exchange(process, {"kind": "checkpoint", "request_id": 4}))
        responses.append(exchange(process, {"kind": "close", "request_id": 5}))
    finally:
        if process.stdin:
            process.stdin.close()
        return_code = process.wait(timeout=30)
        if return_code != 0:
            raise RuntimeError(f"KSA64 evaluator exited {return_code}")
    text = json.dumps({"schema": "KSA64 Phase 9 external protocol example v1", "responses": responses}, indent=2) + "\n"
    if args.output:
        args.output.write_text(text, encoding="utf-8")
    print(text, end="")


if __name__ == "__main__":
    main()
