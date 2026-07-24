# Phase 7 — Multi-profile mission packs and hobby vertical evaluation

Status: complete.

Phase 7 adds a reusable evaluation boundary without rewriting the accepted
orbital stack. The same repository now evaluates three explicitly versioned
profiles:

- `LegacyKsa2PlanarV1`, adapting the frozen Phase 2 executor;
- `LegacyKsa5SpatialV1`, adapting the frozen Phase 5 executor;
- `HobbyVerticalV1`, a new SI-scaled model for model and high-power rockets.

The legacy adapters normalize results only after their existing executors
finish. Their arithmetic, mission composition, telemetry, and accepted
artifacts remain unchanged.

## Data pipeline

```text
reviewable JSON + normalized motor source
                 |
                 v
      host-only pack compiler
                 |
                 v
       KVP7 + KMP7 + KMC7
                 |
                 v
   portable allocation-free evaluator
                 |
        +--------+---------+
        |                  |
       KST7              KSR7
        |                  |
       KPH7              KRA7
```

The C64 never parses JSON. It links one selected profile and consumes bounded,
CRC-protected packs. Host tools compile sources, execute candidates and
campaigns, retain detailed evidence, and perform independent analysis; they do
not contain a second production simulator.

## Canonical reference mission

The checked configuration combines published Giant Leap Firestorm 54
specifications with a public-domain TRA-test-derived AeroTech I211W RASP curve.
Source snapshots, attribution, retrieval date, and checksums are under
`sources/` and `source-data/`.

KSA64 assumptions include a two-metre vertical rail, calm sea-level launch,
constant axial Cd 0.60, impulse-proportional propellant depletion, apogee
drogue deployment, 200 m AGL main deployment, linear canopy inflation, and
canopy Cd 1.5. This is a published-data reference configuration, not a
flight-correlated or certification-grade prediction.

The accepted exact mission completes 2,702 steps from ignition through rail
exit, burnout, apogee, drogue and main deployment, and ground contact:

| Result | Value |
|---|---:|
| Apogee | 978.066 m |
| Maximum speed | 146.477 m/s |
| Maximum acceleration | 107.691 m/s² |
| Maximum dynamic pressure | 12,933.406 Pa |
| Maximum Mach | 0.431226 |
| Impact velocity | -6.15654 m/s |
| State checksum | `a61c5720` |

The independent float64 calculation reaches 978.076 m and -6.15609 m/s. The
small differences are attributed to the declared fixed-point formats,
interpolation, and semi-implicit Euler schedule.

## Commands

Compile the human-readable sources into bounded packs:

```powershell
cargo run -p ksa64-host --bin phase7_compile -- phase7/source-data target/phase7/packs
```

Run one exact mission and produce KST7, KSR7, and KPH7:

```powershell
cargo run -p ksa64-host --bin phase7_run -- phase7/examples target/phase7/run
```

Run the routine or frozen reference campaign:

```powershell
cargo run -p ksa64-host --release --bin phase7_campaign -- phase7/examples target/phase7/campaign 64 4
cargo run -p ksa64-host --release --bin phase7_campaign -- phase7/examples target/phase7/reference 1024 4
```

Run the bounded completion audit:

```powershell
powershell -File phase7/complete.ps1
```

The routine audit rebuilds the stock binaries, reruns the finite 129-state
exactness trace and KPH7 replay, and validates the frozen full-flight result and
binary hash. It deliberately does not repeat the approximately 17.72-minute
physical-C64 mission.

## Campaign behavior

Design choices and uncertainty values use separate types. The initial catalog
varies dry mass, motor performance, body drag, drogue/main CdA, main deployment
altitude, rail length, and recovery inflation delay. Run zero is nominal.
Draws are keyed by seed, run index, and parameter, so worker scheduling cannot
change a result.

The frozen seed `0x4b534137` produces 1,024 recovered cases. One-worker and
four-worker executions are byte-identical. The apogee envelope is 860.820 to
1,097.144 m and the impact-velocity envelope is -6.565 to -5.797 m/s under the
reviewed synthetic ranges. These frequencies describe this model and catalog;
they are not real-world probability claims.

## Stock C64 result

No REU is required. The full evaluator PRG is 21,884 bytes and ends at `$5D7B`;
the replay PRG is 10,076 bytes and ends at `$2F5B`. A 129-state exactness probe
matches the native build field-for-field. The full mission consumed
1,047,635,269 net PAL cycles, or 1,063.32 seconds (17.72 minutes), and reproduced
the native checksum, state, events, outcome, and fault status exactly.

VICE is always run one instance at a time. Harnesses close it after success or
a demonstrated failure. The completed full-flight evidence is frozen in
`c64-execution-v1.json`; routine checks do not silently launch it again.

## Limits and Phase 8 handoff

`HobbyVerticalV1` is intentionally one-dimensional. It does not model wind,
weathercocking, attitude, CG/CP movement, static margin, fin aerodynamics,
recovery drift, staging, or sensor-driven deployment avionics. Phase 8 owns
geometry-derived mass properties, spatial hobby flight, stability, wind, drift,
and correlation against external tools or representative flight data.

Phase 9 remains the host-side optimization workbench. Phase 7 exposes stable
metrics, candidate packs, deterministic campaigns, and strict archives, but it
does not impose one universal score or put an optimizer into the portable core.
