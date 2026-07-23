# Phase 4 host campaign and independent analysis

## Ordered native execution

`ksa64-host` executes campaign runs on one or more native worker threads. Each worker receives run indices determined solely by its worker number. Worker results are collected, sorted by run index, validated for a complete zero-to-N sequence, and only then folded into the canonical aggregate and KSR4 byte stream.

The frozen 1,024-run campaign was executed with 12, 1, and 5 workers. All three KSC4 and KSR4 artifacts were byte-identical.

| Workers | Native release time |
|---:|---:|
| 1 | 30.580 s |
| 5 | 7.719 s |
| 12 | 4.330 s |

The preceding 64-run single-worker timing was 2.015 s, projecting approximately 32.24 s for 1,024 runs before the serial acceptance run was started.

## Frozen artifacts

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `ksa4-reference.ksc4` | 512 | `c7b2124c4b4b41e578bff3c4b55d9bc2c6b289dd2fae558c38d0809effd90506` |
| `ksa4-reference.ksr4` | 131,072 | `5823615849f636adeb67a6dae17f384caff81fd5ace8c4ba9e7f8ee1f727bf46` |
| `reference-campaign-analysis.json` | generated report | `87362f1c9fe8d9a81f320649566636a11a5beb0a9b5357817182788bfedd1b42` |

Campaign identity is `0xa2e9e9d5`; its ordered KSR4 summary chain is `0x813ce420`.

## Independent analyzer

`reference/analyze_campaign.py` does not import or execute Rust. It independently:

1. validates KSC4 framing, reserved bytes, record CRCs, header CRC, and canonical campaign identity;
2. reconstructs every keyed draw, sensor seed, and variation checksum for runs 0–1,023;
3. validates every KSR4 record and its run/campaign/scenario identities;
4. verifies run zero’s four Phase 3 checksum chains;
5. computes float64 orbital energy, eccentricity, perigee, and apogee from cutoff radius, radial velocity, and specific angular momentum;
6. computes float64 load and navigation ranges from raw KSR4 values.

The KSR4 fixed-point classifier is retained for deterministic selection and UI only. It reported 857 stable, 166 suborbital, and one impact outcome. The independent float64 calculation is authoritative for physical analysis and found:

- 934 stable insertions;
- 89 suborbital insertions;
- one impact trajectory;
- zero escape trajectories;
- perigee altitude from -4.843 to 193.631 km;
- apogee altitude from 188.065 to 256.840 km;
- maximum dynamic pressure from 39.906 to 43.896 kPa;
- maximum proper acceleration from 54.611 to 55.634 m/s²;
- cutoff navigation position error from 0.488 to 62.744 m.

The difference between the compact fixed-point classifier and float64 counts is expected and demonstrates why the C64 classification is never physical acceptance evidence.

## Reproduction

Generate artifacts with the release host runner:

```text
cargo run --release -p ksa64-host --bin phase4_campaign -- 1024 12 phase4/examples
```

Regenerate or verify independent analysis:

```text
python -B phase4/reference/analyze_campaign.py --ksc phase4/examples/ksa4-reference.ksc4 --ksr phase4/examples/ksa4-reference.ksr4 --output phase4/reference-campaign-analysis.json
python -B phase4/reference/analyze_campaign.py --ksc phase4/examples/ksa4-reference.ksc4 --ksr phase4/examples/ksa4-reference.ksr4 --output phase4/reference-campaign-analysis.json --check
```