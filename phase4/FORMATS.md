# Phase 4 format allocation

- `KSC4`: 128-byte header plus sixteen 24-byte distribution slots. It binds exact KSC2/KSC3 content CRCs, master seed, and run count.
- `KSR4`: fixed raw run identity, variation checksum, outcome, terminal and cutoff states, load extrema, navigation deltas, four Phase 3 checksum chains, and record CRC.
- `KPH4`: noncanonical plotting data. Points contain a 16-bit step, scaled signed altitude, scaled unsigned downrange, and accumulated event/display flags.
- `KST4`: 96-byte run-bound header followed by unmodified 160-byte Phase 3 telemetry-frame semantics.
- `KRA4`: a 256-byte superblock and append-only typed records. Each record has length, run identity, payload CRC, and committed state. An archive footer makes a complete archive distinguishable from recoverable prefix records.
- `KXV4`: a 64-byte volume header plus a selected archive byte range. Volume identity, selection identity, index/count, logical offset, length, and CRC prevent missing, duplicate, reordered, or mixed-volume joins.

All integers are little-endian. CRC-32 uses the existing IEEE reflected implementation. Canonical records carry raw fixed-point values; decimal formatting belongs to host and C64 presentation adapters.

KST4 validates every inherited KST3 frame and binds the final checksum chains. KXV4 logical payloads are KRA4 archives; the host joiner requires exact consecutive offsets and verifies the reconstructed archive CRC before returning bytes.