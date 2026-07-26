# Phase 10: global Earth flight

Status: complete and accepted.

Phase 10 adds the separately versioned `GlobalEcef6DofV1` profile. It connects
a local launch rail to rotating-Earth atmospheric flight, an inertial
exoatmospheric coast, entry, and local recovery while preserving every
Phase 0-9.5 artifact.

## What it can do

The accepted KSA-G10R research mission passes through five explicit owners:

```text
LocalLaunch -> EcefAscent -> EciCoast -> EcefEntry -> LocalRecovery
```

- WGS 84 geodesy and ellipsoidal height.
- Central gravity plus J2.
- Compiled IERS/IAU Earth-orientation transforms.
- Continuous elapsed TAI with UTC, TAI, TT, and UT1 host conversions.
- Compiled U.S. Standard Atmosphere 1976 profiles and rotating air.
- Exact frame changes at 32 Hz avionics releases.
- Truth-blind global navigation, guidance, gimbal/RCS control, and recovery.
- Strict global telemetry, summaries, plot histories, campaign archives, and
  passive Mission Control reports.
- A frozen KSA-5A insertion handoff and one-orbit global coast check.

The fictional KSA-G10R nominal controlled mission reaches 210.897 km apogee,
travels 336.169 km downrange, crosses all four frame boundaries, and lands
after 687.938 seconds.

## Validation model

KSA64 remains the sole production authority. Validation is deliberately split:

1. Analytic and fixed-vector tests validate time, frames, geodesy, gravity,
   atmosphere, integration, and transitions.
2. A separate Python/float64 implementation propagates the complete
   uninstrumented physical mission.
3. The controlled flight is validated separately through truth-blind avionics
   tests, exact cell/checksum chains, named faults, and campaigns.
4. Frozen SatKit-derived frame fixtures and the KSA-5A coast evidence provide
   specialist corroboration without becoming runtime dependencies.

The independent physical comparison differs by 0.0017% in apogee, 0.0141% in
downrange, and 48.9 m in landing position. All flight events agree within one
32 Hz step. Terminal ground contact is allowed four recovery steps (0.125 s)
for accumulated fixed-point descent error; the observed difference is
0.09375 s.

See [VALIDATION.md](VALIDATION.md) and [COMPLETION.md](COMPLETION.md).

## Campaign evidence

The accepted seed is `0x4b5341a0`.

| Campaign | Recoveries | Numeric/frame/time faults | Archive SHA-256 |
|---|---:|---:|---|
| Routine, 64 cases | 64 | 0 | `2cc8e089ecfbc6f470ef61cd2aca684e53dc1b4a9bcdb1f2dd821c85714c05d1` |
| Completion, 256 cases | 256 | 0 | `18c56e7537a8393376e0444033170319f74449b51826a4916cce74c5bc2f4daf` |

Both archives are byte-identical with one, four, and eight workers.

## Host usage

Run the global mission with the passive Mission Control presentation:

```powershell
cargo run -p ksa64-host --bin phase10_launch -- --display tui
```

Run a deterministic campaign:

```powershell
cargo run -p ksa64-host --release --bin phase10_campaign -- target/phase10/campaign 64 8
```

Regenerate the production uninstrumented reference:

```powershell
cargo run -p ksa64-host --bin phase10_world_reference
```

The checked-in HTML, CSV, KMR10, KPH10, KSR10, and KTT10 nominal evidence is
under [evidence](evidence/).

## Stock C64

Phase 10 supplies:

- a 37,403-byte externally paced global flight endpoint;
- a 35,247-byte release-timing probe;
- a 17,002-byte stock trajectory replay.

All fit below `$C000`, require no REU, and passed finite one-instance VICE
probes with warp disabled. The endpoint is exact step-and-ack hardware in the
loop, not realtime: the measured fast release costs 54.9 PAL release slots and
the transition release costs 114.1 slots.

The host advances the world to a release, sends only transported KLR10
measurements, waits for the C64 flight result, verifies it against the native
shadow, and then advances. A portable C64 global world remains a priority
follow-on, but it does not block this phase.

No complete C64 Phase 10 mission was started. The bounded completion audit
uses stored evidence by default and launches VICE only with `-RunVice`.

## Audit

Run the complete bounded audit:

```powershell
powershell -File phase10/complete.ps1
```

Useful development variants:

```powershell
powershell -File phase10/complete.ps1 -SkipLegacy
powershell -File phase10/complete.ps1 -SkipMos
powershell -File phase10/complete.ps1 -TargetOnly
powershell -File phase10/complete.ps1 -SkipLegacy -RunVice
```

`-RunVice` uses one emulator at a time, disables warp, closes every completed
or proven-failed process, and waits 20 seconds between processes. It still does
not start a complete target mission.

KSA64 is engineering-simulation software. Its numerical validation is not
real-vehicle validation, launch approval, certification, regulatory evidence,
or safety authority.
