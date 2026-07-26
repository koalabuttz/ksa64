# Phase 9.5 Gate 10 stock-target decision boundary

The optimized advanced flight endpoint fits the flat stock region, ending at `$B416` with 3,050 bytes remaining. Its locked realtime acceptance gate does not pass: the measured worst release is 681,123 PAL cycles against 24,631. The advanced wrapper alone measures 108,582 cycles.

The separately compiled world endpoint also does not fit. Replacing KVP8/KPE9/KPA9 parsing with byte-exact compiled fixtures reduced the final static deficit, but the linker still reports 43,058 bytes of program/data overflow and 50,604 bytes at the final static boundary. The largest generated routine is `AdvancedWorldEndpoint::release` at 35,422 bytes.

Per the accepted plan, implementation stops at this boundary. Rates were not lowered, allocation was not moved to the host, effectors were not removed, an REU was not made mandatory, and no full target mission was started. Continuing requires an explicit choice of a materially different target strategy. See [stock-target-boundary.json](evidence/stock-target-boundary.json) for the measured evidence.
