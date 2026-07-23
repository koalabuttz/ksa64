# Phase 6 handoff: Commodore-in-the-loop

Phase 5 leaves KSA64 ready to plan, but not yet implement, a physical multi-computer split. The accepted simulator already has the architectural seam Phase 6 needs: the world owns truth, flight software sees only fixed-width sensor transports, commands return through a validated actuator transport, and mission-control products consume strict recorded evidence.

## Assets entering Phase 6

- One portable `no_std` core, shared by native Rust and David's pinned rust-mos target.
- A world/flight boundary that does not expose simulator truth to guidance code.
- Strict sensor and actuator records with sequence, reserved-field, identity, and CRC checks.
- Deterministic seeds, mission cases, fault schedules, and exact checksum chains.
- KST5 canonical telemetry and KPH5 bounded presentation history.
- Stock-C64 adapters, PAL VICE automation, SID/VIC-II replay, explicit REU DMA, and IEC export.
- Native execution for long missions plus finite target probes for exactness and timing.

No Phase 6 transport format, electrical interface, baud/handshake rate, or machine assignment is frozen yet.

## Non-negotiable constraints

1. The single-machine Phase 5 composition remains a regression oracle.
2. Splitting processes may add transport latency but may not fork or duplicate the physical models.
3. Every link is framed, length-bounded, versioned, checksummed, sequenced, and deterministic under replay.
4. Startup, timeout, duplicate, stale, corrupt, delayed, and disconnected-link behavior is explicit and fail-closed.
5. Flight software still receives transported measurements only; mission control never gains vehicle authority accidentally.
6. Optional REU capacity may buffer link traffic or evidence, but stock hardware remains a supported endpoint.
7. Wall-clock real-time operation is not assumed. The protocol must support paced, step-and-acknowledge execution.
8. Long target runs remain subject to a fresh projection and explicit user approval and are never canceled for timing evidence.

## Recommended first Phase 6 experiment

Build a transport-neutral, host-native loopback before choosing cables:

```text
world endpoint
    -> framed sensor message
    -> deterministic link emulator
    -> unchanged flight endpoint
    -> framed actuator message
    -> deterministic link emulator
    -> world endpoint
```

The emulator should inject bounded delay, loss, duplication, corruption, and disconnects while recording a canonical link transcript. With zero injected impairment and the accepted latency setting, the split run must reproduce a declared single-machine Phase 5 checksum boundary exactly. This answers the protocol and scheduling questions before electrical behavior complicates diagnosis.

The first C64 hardware spike should then move that exact framing through one local loopback adapter and one two-machine step-and-acknowledge exchange. A user-port link is the leading historical candidate, but Phase 6 planning must compare electrical safety, handshaking, cable availability, achievable throughput, and the need for level shifting before locking it in.

## Decisions Phase 6 must make

- Physical transport and safe electrical interface.
- Framing size, escaping, synchronization, and recovery.
- World/flight step ownership and acknowledgement rules.
- Whether sensor and command messages reuse existing records directly or receive an outer link envelope.
- Timeout budgets and safeing behavior at each flight phase.
- Clock synchronization and whether mission time is authoritative on one endpoint.
- Transcript format and deterministic replay rules.
- Mission-control topology: passive telemetry first, independent estimator later.
- Stock-C64 buffering budget and how REU capacity scales queues without changing behavior.

## Suggested acceptance ladder

1. Host loopback produces exact zero-impairment results and deterministic impairment transcripts.
2. A bounded rust-mos codec probe matches host framing byte for byte.
3. PAL VICE loopback validates resynchronization, timeout, corruption, and disconnect behavior.
4. Two physical or independently emulated C64 endpoints complete bounded step exchanges.
5. The split world/flight arrangement reproduces the accepted declared checksum boundary.
6. A third passive mission-control endpoint receives telemetry without affecting world or flight results.
7. An independent ground estimate is compared with onboard navigation using declared tolerances.

Phase 6 should begin with a dedicated plan that resolves the first transport and scheduling choices. This handoff deliberately does not make those choices by accident during Phase 5 completion.
