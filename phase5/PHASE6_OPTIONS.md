# Phase 6 deployment and transport options

Status: exploration only. This record preserves the options discussed after
Phase 5 completion. It is not a Phase 6 plan and freezes no transport,
electrical interface, topology, scheduling rule, or acceptance contract.

## Accessibility goal

Three physical C64s remain the most theatrical Commodore-in-the-loop
demonstration, but they must not become the minimum hardware requirement. Most
users have zero or one physical C64. Phase 6 should therefore treat world,
flight, and mission control as logical endpoints whose placement is selected
independently of the simulation model and link protocol.

The preferred physical baseline is one real C64 acting as the flight computer,
with the host supplying the simulated world and initially Mission Control. The
C64 continues to receive transported measurements only and returns actuator
commands; it never gains access to simulator truth.

## Candidate deployment ladder

| Topology | Required physical C64s | Intended use |
|---|---:|---|
| Single native process | 0 | Accepted Phase 5 regression oracle |
| Split native processes | 0 | Protocol, scheduling, impairment, and transcript development |
| Two or three VICE instances | 0 | Fully emulated Commodore-in-the-loop acceptance |
| Host world plus real-C64 flight | 1 | Recommended physical baseline |
| VICE world plus real-C64 flight plus VICE Mission Control | 1 | Accessible three-station demonstration |
| Physical world and flight plus host/VICE Mission Control | 2 | Literal two-computer hardware-in-the-loop |
| Physical world, flight, and Mission Control | 3 | Optional crown-jewel demonstration |
| Self-contained single C64 | 1 | Separate feasibility track; not currently implemented |

No message format should encode a particular row of this table. The same
endpoint state machines and framed records should work through every topology.

## Transport-neutral endpoint architecture

Phase 6 should separate roles from transports:

```text
WorldEndpoint
FlightEndpoint
MissionControlEndpoint

InProcessTransport
HostSocketTransport
ViceRs232Transport
C64UserPortTransport
UltimateUciTcpTransport
TranscriptReplayTransport
```

A topology assigns one endpoint and one transport adapter to each participant.
The KSA64 link layer above those adapters owns framing, record type, bounded
length, sequence, CRC, timeout, resynchronization, and deterministic transcript
semantics.

TCP reliability must not remove the application framing or CRC. Keeping the
same link contract over every byte stream preserves identity, replay, fault
injection, reconnect behavior, and comparable failure evidence.

## VICE strategy

The normal automated multi-C64 configuration should use independently running
VICE instances joined by a deterministic host broker. VICE documents user-port
RS-232 devices, declared baud rates, flow control, and socket-backed RS-232
connections. A focused spike must confirm those facilities in the pinned
Windows VICE build before Phase 6 freezes a VICE adapter.

The broker, rather than VICE network-play, should route frames. It can impose
deterministic delay, loss, duplication, corruption, and disconnect schedules
and can record the canonical transcript. Step-and-acknowledge scheduling makes
simulation time independent of the wall-clock speeds of the individual
emulators, allowing automated VICE runs to use warp mode when appropriate.

## Recommended one-C64 physical topology

The strongest accessible physical arrangement is:

```text
native host
  world, sensors, canonical telemetry, Mission Control
             |
      framed physical link
             |
real C64
  navigation, guidance, control, sequencing
             |
      actuator commands
             |
native host
```

This makes the physical C64 the vehicle's brain while avoiding the much larger
vehicle/world target cost. Existing Phase 5 timing projects approximately 2.28
hours of raw avionics work for a nominal mission, before link overhead and
margin, compared with approximately 13.74 hours of raw vehicle work.

The current avionics timing image ends at `$BF5E`, so an endpoint build cannot
simply append a driver. Early Phase 6 packaging work must measure the real
endpoint image and recover bounded room through removal of timing harnesses,
targeted optimization, a revised memory map, ROM banking, or optional REU
buffers. This is an implementation gate, not permission to alter flight
results.

## Physical user-port transport

The user port remains the leading universal transport for an original C64. A
Phase 6 plan must still choose and validate electrical protection, voltage
levels, directionality, handshaking, baud or parallel cadence, cable design,
timeouts, reset behavior, and safe disconnect behavior. No raw C64 signal
should be connected to an incompatible host serial standard without the
required interface electronics.

The link protocol above this driver should remain identical to the VICE and
Ethernet forms.

## C64 Ultimate Ethernet

Ultimate hardware offers a promising optional transport that avoids a custom
user-port cable. The Ultimate Command Interface (UCI) exposes the Ultimate
management application's network stack to C64 software through cartridge-port
registers. Its Network Target can open and close TCP sockets and perform
bounded socket reads and writes to a named host.

The intended arrangement is:

```text
C64 or C64 Ultimate
  flight endpoint
       |
Ultimate UCI Network Target
       |
Ethernet or supported Ultimate network interface
       |
KSA64 host broker
  world and Mission Control
```

`UltimateUciTcpTransport` should implement the same bounded byte-stream
interface as the user-port driver. It must handle UCI discovery, configuration,
partial reads and writes, queue bounds, polling, socket closure, abort/reset,
timeouts, and reconnects. UCI command and response queues are documented as
896 bytes, which is sufficient for the expected bounded link records, subject
to the final Phase 6 envelope.

The UCI register interface occupies `$DF1B` through `$DF1F` when enabled.
Concurrent UCI and REU operation therefore needs an explicit register and DMA
regression even though the existing standard REU path uses a different part of
the `$DFxx` page.

VICE is expected to remain the protocol and RS-232 acceptance environment; it
must not be assumed to emulate the complete Ultimate UCI Network Target.
Ultimate-specific validation should use a native mock of the UCI state machine
plus finite read, write, close, timeout, and reconnect probes on actual
Ultimate hardware.

Official background:

- <https://1541u-documentation.readthedocs.io/en/latest/uci/core_uci_architecture.html>
- <https://1541u-documentation.readthedocs.io/en/master/uci/network_target.html>
- <https://1541u-documentation.readthedocs.io/en/latest/hardware/ethernet.html>
- <https://vice-emu.sourceforge.io/vice_6.html>

## Self-contained single-C64 feasibility track

A separate bounded investigation should ask whether the complete Phase 5
composition can execute on one physical C64. Candidate approaches are:

1. A specialized monolithic stock build using a revised memory map and only
   measured optimizations.
2. REU-backed code overlays, with a resident scheduler moving world, flight,
   and telemetry code into an execution window.
3. A banked cartridge image holding role-specific code banks.
4. A separately versioned reduced-fidelity stock mode.
5. Disk overlays, retained only as a control option because per-step loading is
   likely impractical.

An REU cannot execute code directly, but its DMA makes it the strongest current
candidate for complete sequential role overlays. A banked cartridge may be the
cleanest distributable expanded-hardware form. A reduced-fidelity build must
carry a new contract identity and must not be presented as the canonical Phase
5 model.

This investigation should report exact image sizes, resident state, transfer
cost, projected mission duration, fidelity differences, and required hardware
before any implementation path is selected.

## Provisional support priorities

The eventual Phase 6 plan should consider making these required:

1. Single-process Phase 5 regression.
2. Split native endpoints and deterministic broker.
3. Multi-instance VICE operation.
4. One physical C64 as flight computer with a host world.
5. Passive host or VICE Mission Control.
6. User-port transport and transcript replay.

These should remain optional:

1. Ultimate UCI Ethernet transport.
2. Two physical C64 endpoints.
3. Three physical C64 endpoints.
4. A self-contained single-C64 deployment.

The priority labels above are discussion defaults, not accepted decisions.

## Questions deliberately left open

- Exact link envelope and whether existing sensor/command records are nested
  directly or copied into new link payloads.
- Step ownership, acknowledgement ordering, and mission-clock authority.
- User-port electrical and signaling design.
- VICE socket syntax and capabilities in the pinned Windows build.
- Host broker topology for one, two, or three downstream endpoints.
- Flight-endpoint memory recovery below the stock target boundary.
- Whether Ultimate Ethernet is detected automatically or selected explicitly.
- Required Ultimate firmware capabilities and polling versus interrupt use.
- Whether self-contained one-C64 execution is a Phase 6 deliverable or a
  separate optional phase.
- Which topology forms the minimum Phase 6 completion requirement.

Nothing in this record authorizes a long C64 mission. Existing projection,
confirmation, and no-cancellation rules remain in force.
