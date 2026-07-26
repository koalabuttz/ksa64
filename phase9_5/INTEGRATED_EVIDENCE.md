# Phase 9.5 Integrated Mission Evidence

Gate 8 composes the accepted advanced effectors with the frozen Phase 8 spatial world and the Phase 8.5 exact 32 Hz avionics clock. The legacy Phase 8 and 8.5 entry points remain unchanged; advanced controls enter through a new additive step path.

## Accepted results

- Firestorm-C9, Firestorm-R9, and Firestorm-M9 complete ascent, avionics-commanded dual deployment, recovery, and ground contact.
- The accepted C9 wind case uses a declared boundary-layer profile: east wind increases linearly from 0 m/s at the launch site to 5 m/s at 50 m AGL and remains 5 m/s above. A 2.5 m rail keeps local canard incidence inside the frozen 15 degree envelope. At the fixed half-second check the rail-relative error is 501 turn16 units, about 2.75 degrees, below the 3 degree gate.
- R9 recovers from the injected three-axis post-burnout disturbance with 109 turn16 units, about 0.60 degrees, at the one-second check. It uses 16 pulse quanta and retains more than the protected 20 percent reserve.
- M9 remains operational through 64 consecutive pitot-dropout epochs using the truth-blind conservative fallback.
- Canard hardover and a stuck-open RCS valve are retained as fail-closed named evidence rather than relabelled as successful flights.

## Independent evidence

The independent float64 model validates complete nominal ascent, coast, measured-state recovery sequencing, and landing for C9, R9, and M9. Canard and RCS forces, torques, hinge loads, exact pulse edges, depletion, and mass properties are separately covered by analytic and independent fixed-vector suites.

The float64 comparison is evidence, not a runtime authority. Presentation, telemetry recording, worker placement, and archive capacity cannot affect the portable exact result.

## Reproduction

After compiling the reference packs, run the integrated evidence generator, float64 analyzer, and manifest check:

```powershell
cargo run -p ksa64-host --bin phase9_5_compile -- phase8/examples/firestorm54.kvp8 phase9_5/source-data/advanced-effectors-v1.json phase9_5/examples
cargo run -p ksa64-host --bin phase9_5_integrated
python -B phase9_5/reference/analyze_integrated_float.py
python -B phase9_5/reference/build_integrated_manifest.py --check
```

KSA64 is an engineering simulation. This evidence is not launch approval, certification, regulatory acceptance, or safety authority.
