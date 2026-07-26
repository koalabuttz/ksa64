# Post-Phase-10 handoff

Phase 10 closes the local/global coordinate overlap. Future work should begin
from a concrete mission question and add only the fidelity needed to answer
it.

## Highest-priority engineering tracks

1. **Stock-C64 execution**
   - profile the global and advanced flight hot kernels;
   - evaluate a deliberate 6502-specific rewrite;
   - evaluate C64 Ultimate acceleration without changing canonical behavior;
   - restore a portable C64-world long-run role;
   - accept physical user-port, ACIA, or Ethernet transports.

2. **Environment fidelity**
   - add a versioned runtime empirical atmosphere with pinned space weather;
   - add higher-order gravity only for missions whose error budget requires it;
   - retain the existing compiled profile as the smaller deterministic option.

3. **Orbital operations**
   - sustained propagation and deorbit guidance;
   - `SixAxisWrenchV1`;
   - rendezvous, docking, and station keeping;
   - ground tracking and pass prediction.

4. **Entry and landing**
   - thermal state and ablation;
   - lifting entry and precision landing;
   - propulsive landing and deliberate translational RCS guidance.

5. **Beyond Earth**
   - multi-body and lunar/interplanetary frame/time contracts;
   - only after a specific mission defines the required ephemeris,
     environment, navigation, and validation envelope.

## Preserved rules

- One model owns state in an interval.
- The portable deterministic evaluator remains authoritative.
- External tools produce frozen validation fixtures, never runtime corrections.
- No profile is selected merely from the vehicle's informal category.
- Stock, REU, host, VICE, and physical-C64 placement cannot change physics.
- Long target runs require a fresh projection and explicit confirmation.
- Added fidelity receives a new identity and never mutates frozen artifacts.
