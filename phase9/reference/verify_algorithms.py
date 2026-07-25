#!/usr/bin/env python3
"""Independent Phase 9 archive, Pareto, and worker-exactness verifier."""
from __future__ import annotations

import argparse
import binascii
import hashlib
import json
import struct
from pathlib import Path

KDV9 = 256
KOE9 = 512
KAS8 = 256
KRA_HEADER = 32
KRE_HEADER = 32
RECORD = KDV9 + KOE9


def u16(data: bytes, offset: int) -> int:
    return struct.unpack_from("<H", data, offset)[0]


def u32(data: bytes, offset: int) -> int:
    return struct.unpack_from("<I", data, offset)[0]


def i32(data: bytes, offset: int) -> int:
    return struct.unpack_from("<i", data, offset)[0]


def crc32(data: bytes) -> int:
    return binascii.crc32(data) & 0xFFFFFFFF


def checked_record(data: bytes, magic: bytes, length: int) -> None:
    assert len(data) == length
    assert data[:4] == magic
    assert u16(data, 4) in (8, 9)
    assert u32(data, length - 4) == crc32(data[:-4])


def parse_manifest(path: Path) -> dict:
    data = path.read_bytes()
    checked_record(data, b"KOM9", 2048)
    count = data[66]
    objective_count = data[67]
    constraint_count = data[68]
    variables = []
    for index in range(count):
        offset = 96 + index * 20
        minimum = i32(data, offset + 4)
        maximum = i32(data, offset + 8)
        quantum = u32(data, offset + 12)
        assert minimum <= maximum and quantum > 0
        variables.append((u16(data, offset), minimum, maximum, quantum))
    directions = [data[736 + index * 8 + 3] for index in range(objective_count)]
    assert all(value in (1, 2) for value in directions)
    return {
        "identity": u32(data, 16),
        "variables": variables,
        "objective_count": objective_count,
        "constraint_count": constraint_count,
        "directions": directions,
    }


def parse_archive(path: Path, manifest: dict) -> tuple[list[dict], list[dict]]:
    data = path.read_bytes()
    offset = 0
    generations = []
    known = set()
    while offset < len(data) and data[offset:offset + 4] == b"KRA9":
        assert len(data) - offset >= KRA_HEADER
        length = u32(data, offset + 8)
        assert u16(data, offset + 4) == 9
        assert u16(data, offset + 6) == KRA_HEADER
        assert u32(data, offset + 12) == manifest["identity"]
        assert offset + length <= len(data)
        segment = data[offset:offset + length]
        assert u32(segment, length - 4) == crc32(segment[:-4])
        index = u16(segment, 16)
        count = u16(segment, 18)
        assert index == len(generations)
        assert length == KRA_HEADER + count * RECORD + 4
        assert not any(segment[24:KRA_HEADER])
        candidates = []
        aggregates = []
        fingerprint = bytearray(struct.pack("<H", index))
        record_offset = KRA_HEADER
        for _ in range(count):
            design = segment[record_offset:record_offset + KDV9]
            checked_record(design, b"KDV9", KDV9)
            candidate = u32(design, 16)
            assert u32(design, 32) == manifest["identity"]
            value_count = design[36]
            assert value_count == len(manifest["variables"])
            for variable_index, (_, minimum, maximum, quantum) in enumerate(manifest["variables"]):
                value = i32(design, 40 + variable_index * 4)
                assert minimum <= value <= maximum
                assert (value - minimum) % quantum == 0
            record_offset += KDV9
            aggregate = segment[record_offset:record_offset + KOE9]
            checked_record(aggregate, b"KOE9", KOE9)
            assert u32(aggregate, 32) == manifest["identity"]
            assert u32(aggregate, 36) == candidate
            objective_count = aggregate[45]
            constraint_count = aggregate[46]
            assert objective_count == manifest["objective_count"]
            assert constraint_count == manifest["constraint_count"]
            objectives = [i32(aggregate, 64 + item * 4) for item in range(objective_count)]
            aggregates.append({
                "candidate": candidate,
                "identity": u32(aggregate, 16),
                "feasible": aggregate[44] == 1,
                "fatal": aggregate[42],
                "violations": aggregate[43],
                "tier": aggregate[40],
                "objectives": objectives,
            })
            record_offset += KOE9
            candidates.append(candidate)
            known.add(candidate)
            fingerprint += struct.pack("<II", candidate, aggregates[-1]["identity"])
        assert u32(segment, 20) == crc32(bytes(fingerprint))
        # Repeated population slots are canonical references to the same materialized candidate.
        generations.append({"index": index, "candidates": candidates, "aggregates": aggregates, "crc": u32(segment, 20)})
        offset += length
    evidence = []
    previous = -1
    while offset < len(data):
        assert data[offset:offset + 4] == b"KRE9"
        length = u32(data, offset + 8)
        segment = data[offset:offset + length]
        assert len(segment) == length
        assert u16(segment, 4) == 9 and u16(segment, 6) == KRE_HEADER
        assert u32(segment, 12) == manifest["identity"]
        candidate = u32(segment, 16)
        tier = segment[20]
        count = segment[21]
        assert candidate in known and candidate > previous
        assert tier == count and count in (1, 8, 64)
        assert length == KRE_HEADER + count * KAS8 + 4
        assert not any(segment[22:KRE_HEADER])
        assert u32(segment, length - 4) == crc32(segment[:-4])
        for index in range(count):
            record = segment[KRE_HEADER + index * KAS8:KRE_HEADER + (index + 1) * KAS8]
            checked_record(record, b"KAS8", KAS8)
        evidence.append({"candidate": candidate, "tier": tier, "count": count})
        previous = candidate
        offset += length
    assert offset == len(data)
    assert {item["candidate"] for item in evidence} == known
    return generations, evidence


def dominates(left: dict, right: dict, directions: list[int]) -> bool:
    if left["feasible"] and not right["feasible"]:
        return True
    if not left["feasible"]:
        return False
    if not right["feasible"]:
        return True
    better = False
    for a, b, direction in zip(left["objectives"], right["objectives"], directions):
        if direction == 1:
            if a > b:
                return False
            better |= a < b
        else:
            if a < b:
                return False
            better |= a > b
    return better or left["candidate"] < right["candidate"]


def verify_study(directory: Path) -> dict:
    manifest = parse_manifest(directory / "manifest.kom9")
    generations, evidence = parse_archive(directory / "search.kra9", manifest)
    report = json.loads((directory / "report.json").read_text(encoding="utf-8"))
    assert report["manifest_identity"] == f'{manifest["identity"]:08x}'
    assert report["generation_crc32"] == [f'{item["crc"]:08x}' for item in generations]
    terminal = generations[-1]["aggregates"]
    front = []
    seen = set()
    for index, value in enumerate(terminal):
        if not value["feasible"] or value["candidate"] in seen:
            continue
        seen.add(value["candidate"])
        if not any(other != index and dominates(terminal[other], value, manifest["directions"]) for other in range(len(terminal))):
            front.append(index)
    front.sort(key=lambda index: terminal[index]["candidate"])
    assert report["pareto"] == front
    finalists = report["finalists"]
    finalist_ids = [int(item["candidate"], 16) for item in finalists]
    assert len(finalist_ids) == len(set(finalist_ids))
    assert all(item["tier"] == 64 and item["feasible"] for item in finalists)
    evidence_tiers = {item["candidate"]: item["tier"] for item in evidence}
    assert all(evidence_tiers[candidate] == 64 for candidate in finalist_ids)
    return {
        "manifest": f'{manifest["identity"]:08x}',
        "generations": len(generations),
        "archived_candidates": len(evidence),
        "pareto": len(front),
        "finalists": len(finalists),
        "archive_sha256": hashlib.sha256((directory / "search.kra9").read_bytes()).hexdigest(),
    }


def verify_worker_hashes(root: Path) -> None:
    record = json.loads((root / "worker-exactness-v1.json").read_text(encoding="utf-8"))
    for group in (record["quick"], record["accepted"]):
        for study, description in group.items():
            directory = root / ({"study-a": "quick-study-a", "study-b": "quick-study-b"}.get(study, study))
            for name, expected in description["files"].items():
                actual = hashlib.sha256((directory / name).read_bytes()).hexdigest()
                assert actual == expected, (directory, name, actual, expected)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence", type=Path, default=Path(__file__).parents[1] / "evidence")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    verify_worker_hashes(args.evidence)
    studies = {}
    for directory in sorted(path for path in args.evidence.iterdir() if path.is_dir()):
        studies[directory.name] = verify_study(directory)
    assert all(value["finalists"] == 32 for name, value in studies.items() if name.startswith("accepted-study-"))
    assert studies["coupled-demo"]["finalists"] == 16
    assert studies["experimental-airframe"]["finalists"] == 0
    result = {"schema": "KSA64 Phase 9 independent audit v1", "studies": studies, "worker_hashes": "exact"}
    text = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(text, encoding="utf-8")
    print(text, end="")


if __name__ == "__main__":
    main()
