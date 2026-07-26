#!/usr/bin/env python3
"""Freeze and verify Phase 10 campaign reproducibility evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CAMPAIGN = ROOT / "phase10" / "campaign"
REPRO = CAMPAIGN / "repro"
REPORT = CAMPAIGN / "worker-reproducibility-v1.json"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def freeze() -> dict:
    evidence = {}
    for runs in (64, 256):
        records = {}
        hashes = {}
        for workers in (1, 4, 8):
            directory = REPRO / f"w{workers}-{runs}"
            archive = directory / f"ksa-g10r-{runs}.kra10"
            report = directory / f"ksa-g10r-{runs}.json"
            records[workers] = json.loads(report.read_text())
            hashes[workers] = sha256(archive)
        if len(set(hashes.values())) != 1:
            raise SystemExit(f"{runs}-run archives differ: {hashes}")
        canonical = REPRO / f"w1-{runs}"
        for suffix in ("kra10", "ksc10", "json"):
            shutil.copyfile(
                canonical / f"ksa-g10r-{runs}.{suffix}",
                CAMPAIGN / f"ksa-g10r-{runs}.{suffix}",
            )
        evidence[runs] = {
            "runs": runs,
            "archive_sha256": hashes[1],
            "one_worker_seconds": records[1]["wall_seconds"],
            "four_worker_seconds": records[4]["wall_seconds"],
            "eight_worker_seconds": records[8]["wall_seconds"],
            "byte_identical": True,
            "ground_contacts": records[1]["ground_contacts"],
            "physical_recoveries": records[1]["physical_recoveries"],
            "model_envelope_exceeded": records[1]["model_envelope_exceeded"],
            "numeric_frame_time_faults": records[1]["numeric_frame_time_faults"],
        }
    report = {
        "schema": "ksa64.phase10.worker-reproducibility-v1",
        "master_seed": "0x4b5341a0",
        "routine": evidence[64],
        "completion": evidence[256],
    }
    REPORT.write_text(json.dumps(report, indent=2) + "\n")
    return report


def check() -> bool:
    report = json.loads(REPORT.read_text())
    if report["master_seed"] != "0x4b5341a0":
        return False
    for key, runs in (("routine", 64), ("completion", 256)):
        item = report[key]
        archive = CAMPAIGN / f"ksa-g10r-{runs}.kra10"
        config = CAMPAIGN / f"ksa-g10r-{runs}.ksc10"
        summary = json.loads((CAMPAIGN / f"ksa-g10r-{runs}.json").read_text())
        if (
            item["runs"] != runs
            or not item["byte_identical"]
            or sha256(archive) != item["archive_sha256"]
            or config.stat().st_size != 512
            or summary["runs"] != runs
            or summary["ground_contacts"] != runs
            or summary["physical_recoveries"] != runs
            or summary["numeric_frame_time_faults"] != 0
            or summary["model_envelope_exceeded"] != 0
        ):
            return False
    return True


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.check:
        if not check():
            print("phase10 campaign evidence: FAIL")
            return 1
        print("phase10 campaign evidence: PASS")
        return 0
    print(json.dumps(freeze(), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
