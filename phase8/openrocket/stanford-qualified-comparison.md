# Qualified Stanford Firestorm comparison

Source: [Stanford SSI, L2 Post-Flight Analyses](https://wiki.stanfordssi.org/L2_Post-Flight_Analyses) (page snapshot observed 2026-07-24).

This source establishes useful historical configuration context, but it is not a numeric Phase 8 oracle. The page describes several Giant Leap Firestorm 54 vehicles flown in 2016 with Featherweight Raven 3 avionics. Reported examples used AeroTech J425 or J270 motors, 18-inch drogue and 36-inch main parachutes, and main deployment near 600 ft. The page links Raven data externally, but a complete raw data set with a trustworthy vehicle reconstruction was not available in this audit.

The mismatch with the accepted KSA64 reference is material:

- KSA64 uses an AeroTech I211W, not a J425 or J270.
- KSA64 uses the current 1.8923 m / published-mass reference and an explicitly reconstructed geometry; Stanford lists shorter/lighter historical builds in some entries.
- KSA64 recovery CdA and 200 m main trigger do not exactly match the listed 18/36-inch, 600 ft configurations.
- Stanford records recovery anomalies including non-deployment, inverted avionics installation, and inertial main deployment. Those flights cannot validate nominal descent or drift.

The page is retained as evidence that the vehicle family, dual-deployment architecture, and Raven-era flight practice are historically real. It contributes no numeric pass/fail threshold. OpenRocket 24.12 and the independent float64 model remain the Phase 8 comparison evidence.
