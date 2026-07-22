# KSA64

KSA64 is a proposed aerospace simulation framework for the Commodore 64: a small but technically serious system for simulating launch vehicles, flight software, sensors, guidance, telemetry, failures, and eventually hardware-in-the-loop operation across multiple physical C64s.

> Project status: Phase 1 implementation. Raw physics clears 8 Hz; canonical telemetry is captured and strictly inspected on the host, while its complete C64 validation mode is measured at 5.72 Hz. A constant-memory C64 sink now renders a VICE-verified post-run status page; high-precision accumulated-error comparison is next.

## The idea

KSA64 asks a deliberately unreasonable question:

> What would a modern aerospace simulation architecture look like if its target computer were a Commodore 64?

The answer is not a simplified arcade game and not an attempt to squeeze a modern desktop package unchanged into 64 KB. The project will use the same strategy that made early aerospace computing possible: choose the smallest model that answers the current engineering question, precompute expensive data, separate vehicle truth from flight software, validate relentlessly, and increase fidelity only when justified.

The long-term vision has three independently meaningful computers:

    C64 #1: flight computer
        navigation, guidance, control, sequencing
                         |
                         | sensor data and commands
                         |
    C64 #2: vehicle simulator
        dynamics, environment, engines, sensors
                         |
                         | telemetry
                         |
    C64 #3: mission control
        displays, tracking, independent predictions, failures

The first useful version needs only one C64 and a two-dimensional launch model.

## Project goals

- Produce a physically coherent multistage ascent and orbital-flight simulation.
- Run the same deterministic simulation core on a modern host and a C64.
- Use fixed-point arithmetic and table-driven models suitable for a 6510.
- Keep simulated vehicle truth separate from sensors and flight software.
- Validate the implementation with analytic cases and independent external tools.
- Make performance, precision, and memory tradeoffs visible and documented.
- Use the VIC-II, SID, REU, and user port as purposeful parts of the system.
- Progress from a useful one-machine simulator toward genuine Commodore-in-the-loop operation.

## Non-goals

- Reproducing NASA, RocketPy, GMAT, or Kerbal Space Program feature for feature.
- Claiming engineering-grade predictions for a real launch vehicle.
- Running computational fluid dynamics or finite-element analysis on the C64.
- Beginning with six-degree-of-freedom dynamics.
- Requiring the simulation to run in real time.
- Copying vintage or modern programs wholesale into the core.
- Optimizing before correctness and representative measurements exist.

## Current technical direction

The Phase 0 compiler experiment selected a portable Rust core:

- Native Rust builds provide rapid testing, logging, and comparison.
- rust-mos produces the C64 build.
- The portable core uses explicit two-word fixed-point operations on both targets.
- Platform-specific display, sound, REU, timing, and user-port code stays outside the core.
- Oscar64 C++ remains an independent optimization and generated-code reference.

The decision and measurements are recorded in [the Phase 0 results](phase0/RESULTS.md). The checked [Phase 1 numeric foundation](phase0/numeric/FOUNDATION.md) now has a production `no_std` Rust implementation with native, MOS-simulator, and C64 self-test paths. Validated scenarios can select the generated Earth environment, initialize private vertical truth, evaluate immutable typed forces, advance through checked semi-implicit-Euler successors, and execute to a deterministic final state and checksum. Exact interpolation and acceleration-division paths reduce checked dynamics from 160,904.64 to 114,981.59 PAL cycles per step while preserving the golden checksum and general fallbacks. That is 8.57 Hz with 8,174.41 cycles per step of raw headroom. Allocation-free telemetry writers now reproduce the independent records exactly, and the checked executor emits initial, stride, event, terminal, and fault frames through caller-provided sinks without duplicating the physics loop. The golden 257-frame stream matches an independent byte-stream CRC. Three stable PAL runs measure checksum plus canonical telemetry at 172,152.59 cycles per step (5.72 Hz); telemetry adds 7,475.28 cycles per step over checksum mode. A separate host crate captures this same stream, decodes records with the portable core, rejects framing and semantic faults, and renders a compact engineering summary. A C64 adapter retains only the latest frame plus accumulated events and renders a post-run 40x25 status page outside the accepted timing regions; VICE screen-memory inspection verifies every reported field.

## Documentation

- [Architecture](docs/architecture.md) describes the intended system boundaries and data flow.
- [Decision record](docs/decisions.md) preserves accepted and provisional choices.
- [Compiler experiment](docs/experiment.md) defines the rust-mos and Oscar64 comparison.
- [Phase 0 workspace](phase0/README.md) contains the frozen benchmark contract, independent reference generator, and golden vectors.
- [Phase 1 workspace](phase1/README.md) contains the production core and cross-target gate.
- [Host telemetry tools](host/README.md) capture, validate, and summarize canonical mission streams.
- [Phase 1 timing result](phase1/TIMING.md) records the optimization history, passing raw physics budget, and canonical telemetry cost.
- [Validation strategy](docs/validation.md) explains how numerical and physical correctness will be tested.
- [Numeric foundation](phase0/numeric/FOUNDATION.md) selects Phase 1 formats, ranges, overflow behavior, and analytic cases.
- [Data formats](docs/data-formats.md) defines deterministic scenario and telemetry records.
- [Reference software](docs/references.md) records what existing projects can and cannot contribute.
- [Toolchain setup](toolchains/README.md) pins and verifies rust-mos and Oscar64.
- [Roadmap](ROADMAP.md) divides the project into independently useful phases.

## Guiding principles

1. Use the simplest model that can answer the question.
2. Build one portable core, not parallel simulators that can drift apart.
3. Agreement between host and C64 proves consistency, not physical correctness.
4. Every fidelity increase must earn its CPU, memory, and complexity cost.
5. Data tables belong off the hot path; hot state belongs in ordinary C64 RAM.
6. Flight software should see sensors and commands, not omniscient simulator state.
7. A slower correct result is more valuable than a real-time decorative one.
8. Optimize measured kernels, and keep the rest readable.

## First milestone

The compiler and arithmetic experiment is complete. Both candidates passed the frozen workload, and the common target-visible timing result selected Rust/rust-mos:

- Rust: 223,772,332 CIA cycles, or 109,263.83 cycles per step.
- Oscar64: 235,627,088 CIA cycles, or 115,052.29 cycles per step.
- Rust used 5.03 percent fewer cycles while remaining within credible C64 memory limits.

Phase 0 is complete, and the Phase 1 exact core executes the full golden vertical mission while preserving the last valid state on faults. Two exact fast paths save 45,923.05 cycles per step, and three common-clock runs measure checked dynamics at 114,981.59 PAL cycles per step—8.57 Hz with 6.64 percent raw headroom. Canonical mission-stream emission matches the independent 257-frame schedule and byte-stream CRC across native and MOS targets. Three stable PAL measurements show telemetry serialization and discard delivery add 7,475.28 cycles per step over checksum mode; the complete recorded-validation path reaches 5.72 Hz. The host adapter now captures and strictly validates that stream through the same sink boundary. The C64 adapter retains constant-size mission status and renders a 27,866-byte post-run display whose actual screen memory is checked under PAL VICE. The next milestone is an independent high-precision mission comparison and accumulated-error report.
