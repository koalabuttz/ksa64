#!/usr/bin/env python3
"""Validate all four frozen stock-C64 Phase 4 pages under VICE."""
from __future__ import annotations
import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "phase0/reference"))
from vice_timing import run_prg_until_result  # noqa: E402


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


def parse(memory: bytes) -> dict | None:
    if memory[0xFF0:0xFF4] != b"KSA4":
        if memory[0xFF0] == ord("E"):
            raise RuntimeError(f"stock display reported error {memory[0xFF1]}")
        return None
    pages = [rows(memory[offset:offset + 1000]) for offset in (0, 0x400, 0x800, 0xC00)]
    expected = [
        (0, "KSA64 PHASE 4 CAMPAIGN", "RUNS  1024   SUCCESS 0857", "SUMMARY CHAIN 813CE420"),
        (1, "KSA64 PHASE 4 OUTCOME HISTOGRAM", "STABLE 0857", "COMPACT CLASSIFIER - ANALYZER"),
        (2, "KSA64 PHASE 4 BASELINE TRAJECTORY", "ALTITUDE VS SAMPLE", "RETAINED RUNS"),
        (3, "KSA64 PHASE 4 STORAGE", "REU REQUIRED     NO", "ARCHIVE           COMPLETE"),
    ]
    for index, title, first, second in expected:
        if not pages[index][0].startswith(title):
            raise RuntimeError(f"page {index} title: {pages[index][0]!r}")
        if not any(line.startswith(first) for line in pages[index]):
            raise RuntimeError(f"page {index} missing {first!r}")
        if not any(line.startswith(second) for line in pages[index]):
            raise RuntimeError(f"page {index} missing {second!r}")
        if pages[index][24] != "F1 CAMPAIGN F3 HIST F5 PLOT F7 STORAGE":
            raise RuntimeError(f"page {index} footer: {pages[index][24]!r}")
    return {
        "passed": True,
        "titles": [page[0] for page in pages],
        "campaign": pages[0][2],
        "retained": pages[2][21],
        "storage": pages[3][5],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=180.0)
    args = parser.parse_args()
    result = run_prg_until_result(
        args.vice.resolve(strict=True), args.prg.resolve(strict=True), args.timeout,
        0xC000, 0xCFF3, parse,
    )
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())