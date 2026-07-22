# KSA64

KSA64 is a proposed aerospace simulation framework for the Commodore 64: a small but technically serious system for simulating launch vehicles, flight software, sensors, guidance, telemetry, failures, and eventually hardware-in-the-loop operation across multiple physical C64s.

> Project status: Phase 1 implementation. Numeric, scenario, environment, truth-state, and pure force-evaluation gates exist; state integration has not begun.

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

The decision and measurements are recorded in [the Phase 0 results](phase0/RESULTS.md). The checked [Phase 1 numeric foundation](phase0/numeric/FOUNDATION.md) now has a production `no_std` Rust implementation with native, MOS-simulator, and C64 self-test paths. Validated scenarios can select the generated Earth environment, initialize private vertical truth, and evaluate an immutable typed force snapshot. Truth-state transitions have not been implemented yet.

## Documentation

- [Architecture](docs/architecture.md) describes the intended system boundaries and data flow.
- [Decision record](docs/decisions.md) preserves accepted and provisional choices.
- [Compiler experiment](docs/experiment.md) defines the rust-mos and Oscar64 comparison.
- [Phase 0 workspace](phase0/README.md) contains the frozen benchmark contract, independent reference generator, and golden vectors.
- [Phase 1 workspace](phase1/README.md) contains the production numeric core and cross-target gate.
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

Phase 0 is complete, and four Phase 1 production gates pass: deterministic numerics, validated scenario ingestion, generated environment sampling plus immutable truth initialization, and pure vertical force evaluation. The next milestone is one checked semi-implicit-Euler state transition; a mission loop comes afterward.
