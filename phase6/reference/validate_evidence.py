#!/usr/bin/env python3
"""Validate frozen Phase 6 full-flight and stock-packaging evidence."""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
from pathlib import Path

EXPECTED_C64 = {
    "fast_epochs": 12_692,
    "navigation_checksum": 2_195_755_368,
    "flight_checksum": 2_901_449_607,
}
BROKER_FIELDS = (
    "epochs=12692",
    "steps=3173",
    "position=[21360371, 4030786, 15731027]",
    "velocity=[-69442203, 96406364, 65655653]",
    "nav_position=[21360000, 4031445, 15731484]",
    "nav_velocity=[-68076267, 95786604, 65320561]",
    "final_flight_checksum=2901449607",
    "navigation_checksum=2195755368",
    "deadline_misses=0",
    "alarms=0",
)


def artifact(path: Path) -> dict[str, object]:
    raw = path.read_bytes()
    load = struct.unpack_from("<H", raw, 0)[0]
    end = load + len(raw) - 2
    return {
        "bytes": len(raw),
        "sha256": hashlib.sha256(raw).hexdigest(),
        "load_address": load,
        "load_end_exclusive": end,
        "stock_fit": end <= 0xC000,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--mailbox-prg", type=Path, required=True)
    parser.add_argument("--flight-prg", type=Path, required=True)
    args = parser.parse_args()

    evidence = json.loads(args.evidence.read_text())
    if evidence.get("schema") != "ksa64.phase6.vice-mailbox-v1":
        raise RuntimeError("wrong Phase 6 evidence schema")
    if evidence.get("c64") != EXPECTED_C64:
        raise RuntimeError(f"unexpected C64 terminal evidence: {evidence.get('c64')}")
    acceptance = evidence.get("acceptance", {})
    if acceptance.get("cpu_speed") != "1x PAL" or not acceptance.get("externally_paced"):
        raise RuntimeError("full flight must be identified as externally paced 1x PAL")
    if acceptance.get("command_status_cells_shadow_verified") != 12_692:
        raise RuntimeError("not every flight epoch was shadow verified")
    if acceptance.get("deadline_misses") != 0 or acceptance.get("alarms") != 0:
        raise RuntimeError("full flight reported a deadline miss or alarm")
    broker = evidence.get("broker", "")
    for field in BROKER_FIELDS:
        if field not in broker:
            raise RuntimeError(f"missing broker evidence: {field}")

    mailbox = artifact(args.mailbox_prg)
    if evidence.get("artifact") != mailbox:
        raise RuntimeError("mailbox PRG differs from the accepted full-flight artifact")
    flight = artifact(args.flight_prg)
    if not mailbox["stock_fit"] or not flight["stock_fit"]:
        raise RuntimeError("a Phase 6 endpoint exceeds the stock load window")

    print(json.dumps({
        "schema": "ksa64.phase6.evidence-validation-v1",
        "full_flight": "pass",
        "mailbox_artifact": mailbox,
        "physical_acia_artifact": flight,
    }, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
