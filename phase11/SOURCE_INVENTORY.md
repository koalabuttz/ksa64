# Phase 11 source inventory

Status: frozen for implementation.

Phase 11 adds operational contracts, not physical source data. External
historical and current NASA material supplies design lineage only and never
becomes a runtime dependency or authority.

## Operational lineage

| Subject | Source | KSA64 use |
|---|---|---|
| State-vector and target-load uplinks | Apollo 11 Flight Plan, July 1, 1969 | Atomic navigation and bounded guidance loads |
| Autonomous burns without ground contact | NASA Apollo 8 history | Committed onboard plan survives communications loss |
| Independently developed limited backup | NASA Shuttle BFS history and technical reports | Bounded `SafeholdRecoveryV1` package concept |
| Stored versus realtime commanding | NASA/JPL Basics of Space Flight, Cruise | Prefer reviewed loads over arbitrary live commands |
| Command-loss and safe-mode behavior | NASA/JPL Basics of Space Flight, Onboard Systems | Package-declared communications-loss policy |
| Modern command and GN&C separation | NASA Orion spacecraft components | Ground command, onboard guidance/control |
| Modern onboard planning research | NASA MEXEC and autoNGC | Future-compatible package and plan boundaries |

## Primary references

- <https://www.nasa.gov/wp-content/uploads/2025/11/a11fltpln-final-reformat.pdf>
- <https://www.nasa.gov/history/50-years-ago-apollo-8-in-lunar-orbit/>
- <https://www.nasa.gov/history/sts1/pages/computer.html>
- <https://ntrs.nasa.gov/citations/20230002185>
- <https://science.nasa.gov/learn/basics-of-space-flight/chapter15-1/>
- <https://science.nasa.gov/learn/basics-of-space-flight/chapter11-1/>
- <https://www.nasa.gov/reference/spacecraft-components/>
- <https://ai.jpl.nasa.gov/public/projects/mexec/>
- <https://www.nasa.gov/communicating-with-missions/autongc/>

## Frozen project inputs

Phase 11 reuses without alteration:

- Phase 10 KEM10, KFT10, KAT10, KGV10, and KGM10 packs;
- the accepted KSA-G10R nominal world and frame sequence;
- KLR10 fast, aid, transition, command, and status cells;
- the frozen `GlobalFlightComputer`;
- Phase 10 canonical telemetry, plot, campaign, target, and validation artifacts.

Every new source project uses checked JSON with exact decimal strings.
Compiled packs, not JSON, identify target execution.

