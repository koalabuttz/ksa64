# Phase 4 contract

## Compatibility

Phase 4 is an additive layer over the accepted Phase 3 composition. Existing Phase 3 entry points, KSC3/KST3/KRP3 bytes, vehicle data, sensor defaults, guidance, and checksum chains remain unchanged. A zero variation must execute through the same values as the Phase 3 nominal mission.

## Determinism

Every sampled value is keyed by campaign master seed, run index, parameter identity, correlation group, and draw index. Execution order and host worker count cannot affect a run. Run zero is never sampled and is the exact baseline.

## Storage independence

Mission execution produces observations and summaries without knowing whether data is discarded, held in stock RAM, transferred to an REU, written to a host file, or exported to IEC storage. Recording failure marks evidence incomplete but cannot change physics, later seeds, or aggregate state.

## Fixed families

| Magic | Role | Fixed size |
|---|---|---:|
| KSC4 | campaign and distribution configuration | 512 bytes |
| KSR4 | one run summary | 128 bytes |
| KPH4 | compact plot header | 64 bytes |
| KPH4 point | presentation-only plot sample | 8 bytes |
| KST4 | detailed telemetry header | 96 bytes |
| KST4 frame | Phase 3-compatible detailed frame | 160 bytes |
| KRA4 | archive superblock | 256 bytes |
| KRA4 record header | append-only record framing | 32 bytes |
| KXV4 | export-volume header | 64 bytes |

KSC4 reserves a 128-byte header followed by sixteen 24-byte distribution slots. Unknown versions, nonzero reserved data, unknown enums, invalid coupled ranges, identity mismatches, and CRC failures are rejected. Incompatible meanings require new magic or version values.

## Target policy

Stock mode retains aggregate state, five interesting summaries, and a sparse plot sampled every 32 physics steps. REU mode samples plots every eight steps and allocates summary space before detailed histories. Full target campaigns remain conditional on an explicit pre-run projection; finite target probes are the completion evidence unless the projection is accepted.
