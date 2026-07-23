# Phase 3 navigation

The flight crate propagates time, radius, Earth-fixed downrange, radial and
tangential velocity, and steering pitch using only `SensorFrame`. It has no
dependency on the simulator or truth state.

Altitude updates use a fixed alpha-beta correction. GPS-like updates are
loosely coupled PVT corrections. The frozen shift gains were selected by
`reference/tune_navigation.py` from a bounded 180-candidate grid and 16
repeatable noise cases. The objective is lexicographic: minimize worst cutoff
position error, then worst cutoff velocity error.

Selected shifts are recorded in `navigation-gains-v1.json` and compiled into
`ksa64-flight`:

- altitude alpha: 1/8
- altitude beta correction: shift 1 plus the fixed rate conversion
- GPS position: 1/8
- GPS velocity: 1/32

A 60-second GPS outage is propagated inertially; reacquired GPS resumes aiding.
Non-monotonic sequence numbers, missing inertial fields, and invalid time deltas
fail closed.
