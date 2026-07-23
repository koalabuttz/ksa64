#!/usr/bin/env python3
"""Independent Phase 4 keyed-distribution vectors and CLT evidence."""
from __future__ import annotations
import argparse
import hashlib
import json
import struct
import sys
import zlib
from pathlib import Path

MASTER = 0x4B534134
PARAMETERS = 15
RUNS = [0, 1, 2, 17, 63, 1023]
SPECS = [
    (0, "fixed", 0, 123, 123, 123, 0),
    (1, "uniform", 0, -10_000, 0, 10_000, 0),
    (2, "triangular", 0, -20_000, 0, 20_000, 0),
    (3, "bernoulli", 0, 0, 0, 1, 333_333),
    (4, "clt", 0, -300_000, 0, 300_000, 0),
    (5, "clt", 7, -100_000, 0, 100_000, 0),
    (6, "clt", 7, -100_000, 0, 100_000, 0),
]

def u32(x: int) -> int: return x & 0xFFFFFFFF

def mix32(x: int) -> int:
    x = u32(x ^ (x >> 16)); x = u32(x * 0x7FEB352D)
    x = u32(x ^ (x >> 15)); x = u32(x * 0x846CA68B)
    return u32(x ^ (x >> 16))

def word(run: int, parameter: int, group: int, draw: int) -> int:
    source = parameter if group == 0 else 0x100 + group
    return mix32(MASTER ^ u32(run * 0x9E3779B9) ^ u32(source * 0x85EBCA6B) ^ u32(draw * 0xC2B2AE35))

def mul_hi(a: int, b: int) -> int: return ((a * b) >> 32) & 0xFFFFFFFF

def uniform(w: int, lo: int, hi: int) -> int:
    return lo if lo == hi else lo + mul_hi(w, hi - lo + 1)

def sample(spec, run: int) -> int:
    parameter, kind, group, lo, baseline, hi, shape = spec
    if run == 0 or kind == "fixed": return baseline
    w = lambda draw: word(run, parameter, group, draw)
    if kind == "uniform": return uniform(w(0), lo, hi)
    if kind == "triangular": return int((uniform(w(0), lo, hi) + uniform(w(1), lo, hi)) / 2)
    if kind == "bernoulli": return hi if mul_hi(w(0), 1_000_000) < shape else lo
    total = sum(w(draw) & 0xFF for draw in range(12))
    centered = total - 1530
    span = hi - baseline if centered >= 0 else baseline - lo
    # Rust integer division truncates toward zero.
    delta = abs(centered * span) // 768
    if centered < 0: delta = -delta
    return max(lo, min(hi, baseline + delta))

def sensor_seed(run: int) -> int:
    if run == 0: return 0x4B534133
    value = mix32(MASTER ^ u32(run * 0xD1B54A35) ^ 0x53454544)
    return value or 0x6D2B79F5

def vector(run: int):
    values = [0] * PARAMETERS
    for spec in SPECS: values[spec[0]] = sample(spec, run) - spec[4]
    seed = sensor_seed(run)
    raw = struct.pack("<II", run, seed) + b"".join(struct.pack("<i", v) for v in values)
    return {"run": run, "sensor_seed": seed, "variation_crc32": zlib.crc32(raw), "values": values}

def build():
    vectors = [vector(run) for run in RUNS]
    clt_spec = SPECS[4]
    samples = [sample(clt_spec, run) for run in range(1, 65_537)]
    edges = list(range(-300_000, 300_001, 50_000))
    bins = [0] * (len(edges) - 1)
    for value in samples:
        index = min((value - edges[0]) // 50_000, len(bins) - 1)
        bins[max(index, 0)] += 1
    packed = b"".join(struct.pack("<i", value) for value in samples)
    return {
        "schema": "ksa64.phase4.distribution-vectors-v1",
        "master_seed": MASTER,
        "vectors": vectors,
        "clt_65536": {
            "minimum": min(samples), "maximum": max(samples),
            "sum": sum(samples), "sum_squares": sum(x*x for x in samples),
            "edges": edges, "bins": bins, "crc32": f"0x{zlib.crc32(packed):08x}"
        }
    }

def rust_source(data):
    lines = ["// Generated independently by phase4/reference/generate_distributions.py.",
             "#[rustfmt::skip]",
             "pub const EXPECTED: &[(u32, u32, u32, [i32; 15])] = &["]
    for v in data["vectors"]:
        values = ", ".join(str(x) for x in v["values"])
        lines.append(f"    ({v['run']}, 0x{v['sensor_seed']:08x}, 0x{v['variation_crc32']:08x}, [{values}]),")
    lines += ["];", f"pub const CLT_CRC32: u32 = {data['clt_65536']['crc32']};", ""]
    return "\n".join(lines).encode()

def main() -> int:
    parser=argparse.ArgumentParser(); parser.add_argument("--check", action="store_true"); args=parser.parse_args()
    root=Path(__file__).resolve().parents[2]
    data=build(); payload=(json.dumps(data, indent=2)+"\n").encode()
    out=root/"phase4/distribution-vectors-v1.json"; sha=out.with_name(out.name+".sha256")
    digest=(hashlib.sha256(payload).hexdigest()+"  distribution-vectors-v1.json\n").encode()
    rust=rust_source(data); rust_out=root/"sim/src/phase4/generated_distribution_vectors.rs"
    if args.check:
        if not out.exists() or out.read_bytes()!=payload or not sha.exists() or sha.read_bytes()!=digest or not rust_out.exists() or rust_out.read_bytes()!=rust:
            print("Phase 4 distribution evidence is stale", file=sys.stderr); return 1
        print("Phase 4 distribution evidence is current"); return 0
    out.write_bytes(payload); sha.write_bytes(digest); rust_out.write_bytes(rust)
    print(out.relative_to(root)); return 0
if __name__=="__main__": raise SystemExit(main())
