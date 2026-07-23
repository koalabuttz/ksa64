# Phase 4 deterministic variation engine

## Frozen behavior

The campaign sampler is allocation-free and deterministic. Every draw is keyed by the master seed, run index, parameter ID or explicit correlation group, and draw index. Catalog order and execution order therefore do not affect a run.

Run zero is the exact, unmodified Phase 3 nominal case. Sampled campaigns begin at run one.

Supported bounded distributions are:

- fixed;
- inclusive uniform;
- symmetric triangular;
- Bernoulli with probability in parts per million;
- `CltNormal3Sigma`, formed from twelve keyed uniform bytes, centered at 1,530, scaled so 768 centered units equal one standard deviation, and clamped to both plus or minus three standard deviations and the physical parameter bounds.

The reviewed catalog is capped at 16 records. Its currently defined parameter identifiers cover payload mass, independent stage thrust, atmosphere and drag scales, sensor biases and global noise scale, actuator lag, and actuator slew. Controller gains, vehicle topology, event sequencing, and probabilistic fault topology remain outside Phase 4.

## KSC4 configuration

`KSC4` is a fixed 512-byte record:

- 128-byte header;
- sixteen 24-byte distribution slots;
- exact KSC2 and KSC3 identity fields;
- record-region and header CRC-32 values;
- zeroed reserved bytes checked during parsing.

Malformed ranges, duplicate parameters, invalid shapes, excessive run counts, reserved-byte changes, identity mismatches, and CRC failures are rejected before a campaign begins.

## Independent evidence

`reference/generate_distributions.py` is intentionally independent of the Rust implementation. It freezes:

- exact vectors for runs 0, 1, 2, 17, 63, and 1,023;
- sensor seed and variation checksum for each vector;
- a 65,536-sample CLT histogram and packed-sample CRC-32;
- a SHA-256 digest of the JSON evidence.

The same vectors pass in native Rust and in the pinned rust-mos `mos-sim-none` target. The Rust generated-vector file is marked `rustfmt::skip` so formatting cannot make the evidence spuriously stale.

## Gate 2 acceptance

Gate 2 is accepted when all of the following pass:

1. Python evidence regeneration check.
2. Native exact-vector, ordering, correlation, validation, and CLT histogram tests.
3. KSC4 round-trip, binding, reserved-byte, and corruption tests.
4. Phase 3 nominal compatibility tests.
5. The finite rust-mos distribution self-test.

No long-running target campaign is part of this gate.