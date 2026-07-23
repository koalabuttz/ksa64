# Phase 2 telemetry and host validation

Status: accepted.

`KST2` is the canonical Phase 2 stream. Its 40-byte header binds the stream to the Phase 2 numeric contract, rotating-Earth environment, packed scenario identity, timestep, telemetry stride, and mission length. Each 64-byte frame carries raw planar truth, pitch command, stage/phase, Mach, dynamic pressure, event bits, the rolling exact-state checksum, and a per-record CRC-32.

The mission executor owns physics and staging exactly once. An immutable observer receives the initial truth and each accepted successor. Telemetry accumulates ignition, cutoff, separation, impact, and end events until the next accepted frame; it emits at step zero, every configured stride, and the terminal step. The raw mission wrapper compiles the same observer path away and omits rolling checksums, preserving a separately measurable production path.

The nominal stream contains 901 frames and 57,704 bytes. Its stream CRC-32 is `0x7d13b2bf` and its final exact-state checksum is `0xcc57612b`. The host inspector validates record CRCs, header/scenario binding, initial truth, strict step order, stride, exact step-derived time, numeric ranges, terminal placement, and event counts before formatting physical values. The public host command can capture or inspect `.kst2` files; the committed golden stream round-trips through that command.

Generated target fixtures freeze the header, initial frame, and final frame. Portable self-tests decode and re-encode those records exactly. Complete-stream storage and formatting remain host concerns; the core sink contract stays allocation-free for later C64 retained-state, REU, disk, or user-port transports.

The external-validation handoff under `external/gmat` converts the independent float64 cutoff state to an equatorial EarthMJ2000Eq Cartesian state and aligns GMAT to a spherical point-mass coast. GMAT is not a repository dependency and the fixture is not claimed as executed automatically; its report comparator makes a future R2026a run repeatable while the independent float64/RK4 evidence remains automated.
