#!/usr/bin/env python3
"""Build or verify the frozen Phase 9.5 integrated-evidence manifest."""
from __future__ import annotations
import argparse, hashlib, json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE = ROOT / "phase9_5" / "evidence" / "integrated"
MANIFEST = EVIDENCE / "manifest-v1.json"

def digest(path: Path) -> dict[str, object]:
    data = path.read_bytes()
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}

def build() -> dict[str, object]:
    files = {}
    for path in sorted(EVIDENCE.iterdir()):
        if path.is_file() and path != MANIFEST:
            files[path.name] = digest(path)
    cases = json.loads((EVIDENCE / "integrated-cases-v1.json").read_text())
    return {
        "schema": "ksa64.phase9_5-integrated-manifest-v1",
        "contract": "PHASE95_CONTRACT_ID 0x09500001",
        "case_count": len(cases),
        "accepted_seed": "0x4b534195",
        "crosswind_profile": "0 to 5 m/s east over the first 50 m; 5 m/s above",
        "files": files,
    }

def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    current = build()
    if args.check:
        expected = json.loads(MANIFEST.read_text())
        if current != expected:
            raise SystemExit("integrated evidence manifest mismatch")
    else:
        MANIFEST.write_text(json.dumps(current, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"files": len(current["files"]), "status": "pass"}))
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
