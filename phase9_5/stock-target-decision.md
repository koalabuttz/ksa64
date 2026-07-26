# Phase 9.5 Gate 10 stock-target decision boundary

The optimized advanced flight endpoint fits the flat stock region, ending at `$B416` with 3,050 bytes remaining. Its locked realtime acceptance gate does not pass: the measured worst release is 681,123 PAL cycles against 24,631. The advanced wrapper alone measures 108,582 cycles.

The separately compiled world endpoint also does not fit. Replacing KVP8/KPE9/KPA9 parsing with byte-exact compiled fixtures reduced the final static deficit, but the linker still reports 43,058 bytes of program/data overflow and 50,604 bytes at the final static boundary. The largest generated routine is `AdvancedWorldEndpoint::release` at 35,422 bytes.

Per the accepted plan, implementation stops at this boundary. Rates were not lowered, allocation was not moved to the host, effectors were not removed, an REU was not made mandatory, and no full target mission was started. Continuing requires an explicit choice of a materially different target strategy. See [stock-target-boundary.json](evidence/stock-target-boundary.json) for the measured evidence.


## Interim target policy — host world plus externally paced C64 flight

The accepted interim stock baseline is now **host world + externally paced C64 flight**. The host advances the KSA64 world to an exact 32 Hz sensor release, sends strict KLR9 sensor cells through KLF6, waits for the stock C64 to execute the real advanced flight and allocation code, and applies the returned command only after exact shadow comparison. Simulated event timing and successor-command semantics remain authoritative; wall-clock time may pause while the C64 computes, so this is step-and-ack hardware-in-the-loop evidence rather than a realtime flight claim.

A finite eight-release VICE probe proves the clean stock endpoint, strict command/status cells, and navigation, flight, and allocator checksum chains. The accepted endpoint is 44,306 bytes, ends at `$B511`, requires no REU, and is recorded in [split-flight-v1.json](evidence/split-flight-v1.json).

Realtime stock-C64 flight and C64-world execution remain priority development tracks, but they no longer block the rest of Phase 9.5. Future investigations explicitly include a 6502-specific implementation of measured hot paths and C64 Ultimate acceleration/integration. The C64 world remains an intended execution role and long-run option; it temporarily follows the host-world baseline while physics, avionics, Mission Control, storage, and finalist workflows mature.
