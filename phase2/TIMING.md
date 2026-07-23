# Phase 2 C64 timing

Status: accepted.

The final linked size-optimized C64 timing program measures the same eight powered atmospheric steps twice under the PAL CIA common clock: once through the raw mission wrapper and once through rolling checksums plus canonical `KST2` scheduling and serialization. Three sequential runs under pinned cycle-accurate VICE 3.10 are identical.

| Path | Net cycles | Cycles/step | PAL steps/s |
|---|---:|---:|---:|
| Raw mission | 9,861,605 | 1,232,700.625 | 0.7993 |
| Checksummed KST2 | 10,950,388 | 1,368,798.500 | 0.7198 |

The recorded path adds 1,088,783 cycles across eight steps (136,097.875 cycles/step). It emits the initial and step-eight frames, 168 bytes total, ending at checksum `0x92dc28f3` and frame CRC `0x2a1875e0`. The timing boundary costs 24 cycles. The linked timing PRG is 34,208 bytes.

This is a powered sea-level fixture, not an assertion that every mission step has identical cost. There is deliberately no real-time acceptance floor. At this measured rate, a complete 7,200-step run is on the order of 2.5 hours raw or 2.8 hours with full validation/recording on a real PAL C64. Slow execution is an accepted Phase 2 result; deterministic post-run replay avoids paying that cost twice merely to draw the mission.
