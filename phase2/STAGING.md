# Phase 2 bounded staging and mission acceptance

Status: accepted.

The Phase 2 scenario is now a fixed-capacity, checksummed `KSC2` image. Its 884 bytes contain the numeric/environment identities, mission duration, initial state, up to four constant-engine stages, sixteen pitch knots, four aerodynamic tables with sixteen knots each, and a CRC-32 footer. The portable parser rejects bad framing, identities, reserved fields, counts, event alignment, mass invariants, stage topology, guidance, or aerodynamic tables before truth state exists.

The stage executor supports pre-ignition coast, burning, pre-separation coast, and completion. Propellant is consumed only while burning. Cutoff occurs at the configured step or depletion; separation drops dry mass and residual propellant without changing position or velocity; the next stage observes its own ignition delay. All events occur on the accepted 0.125-second boundary.

The reviewed KSA-2A schedule uses 155 seconds of first-stage burn, one second to separation, a half-second upper-stage ignition delay, and 240 seconds of upper-stage burn. Its independent float64 reference reaches a 199.989 x 200.015 km orbit with eccentricity 0.000002, Max-Q 40.777 kPa, and peak proper acceleration 55.292 m/s2. The exact fixed-point path reaches a quantized 188.169 x 188.169 km orbit, Max-Q 40.779 kPa, and 55.283 m/s2; this is inside the declared 180-220 km, e <= 0.01, 60 kPa, and 60 m/s2 gates.

The failure image shortens upper-stage powered duration by five percent. The independent reference predicts a -1321 x 200 km impact orbit; the exact fixed-point classifier also reports impact. Both packed images execute for the full 900 seconds in native tests. The nominal 7,200-step mission also executes bit-identically under rust-mos. A generated cutoff fixture keeps the failure-path target check bounded at its exact 3,076-step cutoff; the redundant atmospheric tail remains a native test because it is pathologically slow under instruction-level `mos-sim`.

Generated fixture constants come from the packed mission generator rather than a separately maintained handwritten scenario. Target executables are split by responsibility so parser/contract and full mission diagnostics each fit the 64 KB MOS link region.
