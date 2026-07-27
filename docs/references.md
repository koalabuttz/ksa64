# Reference software and reuse policy

This document records how existing software relates to KSA64. It is a routing guide, not a claim that any one program is an authoritative implementation of the whole project.

Before code or data is reused, verify the current upstream source, version, and license.

## Reference map

| Project | Relevant strength | Intended KSA64 use | Direct core reuse |
|---|---|---|---|
| RocketPy | Atmospheric rocket flight and 3-DOF or 6-DOF simulation | Compare compatible early-flight cases | No runtime dependency planned |
| Tudat | Spaceflight dynamics and numerical propagation | Compare orbital state propagation | No runtime dependency planned |
| SatKit | Time scales, Earth orientation, frame transforms, gravity, and orbit utilities | Preferred Phase 10 specialist fixture generator | Optional offline tool; no runtime or CI dependency |
| Orekit | High-fidelity time/frame transformations and transform derivatives | Phase 10 escalation or independent comparison when SatKit coverage is insufficient | Optional offline tool; no runtime or CI dependency |
| GMAT | Mission analysis and orbit propagation | Occasional exoatmospheric and near-orbital trajectory cross-checks | Optional offline tool; no runtime or CI dependency |
| Basilisk | Spacecraft attitude dynamics and RCS effectors | Optional Phase 9.5 secondary fixtures for selected fixed-step RCS/attitude cases | Never a canard, scheduler, allocator, runtime, or CI authority |
| PREDICT | SGP4 or SDP4 satellite tracking | Host-side oracle for later tracking work | Avoid direct port initially |
| QUIKTRAK | Historical C64 satellite tracking | Historical precedent and possible pass-prediction comparison | No core foundation |
| C64 Apollo Lunar Lander | Compact BASIC physics and telemetry presentation | Study interface and interaction techniques | Avoid copying into the core |
| Apollo AGC source | Flight-software organization and historical algorithms | Architectural study | No instruction-level port |
| NASA Trick and JEOD | Professional simulation separation and environment modeling | Architectural inspiration | Far beyond target runtime |
| NASA cFS and NOS3 | Flight software and simulated-hardware boundaries | Architectural inspiration for later avionics split | No initial dependency |
| OpenMDAO and Dymos | Trajectory and design optimization | Possible host-side future workflow | Outside initial scope |
| Space Shuttle: A Journey into Space | Mission displays and operational flow | Presentation study | No known source reuse |
| Apollo 18: Mission to the Moon | Mission-phase presentation | Presentation study | No known source reuse |
| Project: Space Station | Mission-control and program UI | Presentation study | No known source reuse |

## Phase 9.5 and Phase 10 validator policy

Phase 9.5 keeps all canard, RCS, depletion, changing-mass, actuator, allocation, and handoff models native to KSA64. Analytic and independent float64 cases are primary evidence. Basilisk is optional corroboration for selected fixed-step spacecraft-attitude/RCS fixtures only.

Phase 10 keeps `GlobalEcef6DofV1` authoritative and maintains a complete independent float64 comparison. SatKit is the preferred specialist source for frozen time/frame/EOP/gravity fixtures; Orekit is used when a documented capability gap or valuable independent comparison warrants it; GMAT is reserved for occasional exoatmospheric trajectory checks.

All external results are checked in as versioned fixtures with tool and data versions, complete configuration, raw output, tolerance rationale, hashes, and regeneration instructions. Routine builds and CI do not run these programs, access the network, or download live leap-second or Earth-orientation data. No external program owns or corrects live KSA64 state.

Official project documentation:

- SatKit frame transformations and time/orientation support: <https://satkit.dev/api/frametransform/> and <https://satkit.dev/>
- Orekit frame transforms and time systems: <https://www.orekit.org/site-orekit-13.1/apidocs/org/orekit/frames/Transform.html> and <https://www.orekit.org/site-orekit-13.1/apidocs/org/orekit/time/package-summary.html>
- Basilisk simulation architecture and thruster effector: <https://avslab.github.io/basilisk/Learn/bskPrinciples/bskPrinciples-0.html> and <https://avslab.github.io/basilisk/Documentation/simulation/dynamics/Thrusters/thrusterDynamicEffector/thrusterDynamicEffector.html>
- GMAT project and coordinate-system documentation: <https://sourceforge.net/projects/gmat/> and <https://documentation.help/gmat/CoordinateSystem.html>

## Reuse policy

### Ideas

Algorithms, interface patterns, historical architecture, and presentation ideas may inform independent KSA64 designs. Record the source when an idea materially shapes a decision.

### Test oracles

Run external tools on the host and preserve their versioned outputs as validation artifacts. Match assumptions before comparing results.

### Code

Do not copy code into the KSA64 core until:

- Its exact license is verified.
- Compatibility with the eventual KSA64 license is understood.
- Attribution and source requirements are documented.
- Direct reuse is materially better than a small independent implementation.
- The copied portion is isolated and tested.

Known cautions from the initial research:

- The modern C64 lunar-lander project was reported as CC BY-NC-SA.
- PREDICT was reported as GPL-2.0.
- The preserved C64 QUIKTRAK artifact may not include convenient editable C64 source.

These facts must be rechecked at the upstream projects before relying on them.

### Data

Atmosphere, engine, aerodynamic, and orbital data have their own provenance and licensing concerns. Every generated table should record:

- Source.
- Source version or date.
- Units and conventions.
- Transformation or interpolation method.
- License or public-domain basis.

## Why no existing program is the foundation

The identified projects cover useful pieces:

    QUIKTRAK and PREDICT
        known-orbit propagation and ground tracking

    C64 lunar lander
        local descent physics and interaction

    vintage commercial games
        mission presentation and controls

    RocketPy
        atmospheric rocket simulation

    Tudat and GMAT
        orbital propagation and mission analysis

KSA64's defining feature is the combination and separation of powered ascent, orbital dynamics, sensors, flight software, actuators, failure injection, telemetry, and eventually multiple physical computers. Building a new narrow core preserves those boundaries from the beginning.

## Research backlog

Before implementation reaches the relevant phase:

- Pin exact versions of external validation tools.
- Collect small public reference scenarios.
- Verify licenses from upstream repositories.
- Locate machine-readable constants and atmosphere data with clear provenance.
- Identify published 6-DOF check cases before beginning rigid-body work.
- Preserve screenshots or manuals from vintage software only where redistribution is allowed.
## Phase 7 published-data inputs

The canonical hobby reference uses the Giant Leap Firestorm 54 product page
for kit dimensions, dry weight, recovery sizes, and recommended motor pairing,
plus the public-domain TRA-test-derived AeroTech I211W RASP curve distributed
by ThrustCurve. Normalized snapshots, attribution, retrieval date, source URLs,
license information, and checksums are committed under `phase7/sources/` so the
pack compiler and completion audit require no network access. Model assumptions
and the non-correlation warning are frozen in `phase7/PLAN.md`.

## Phase 8 geometry and external evidence

- Giant Leap Rocketry, Firestorm 54 current product specification: <https://giantleaprocketry.com/products/firestorm-54-rocket-kit>
- Giant Leap Firestorm instructions and manufacturer CP reference: <https://device.report/m/812f6174958b8cd34507c368da687b542bb64ac1e999166f78acb37249905e0c>
- OpenRocket 24.12 release: <https://github.com/openrocket/openrocket/releases/tag/release-24.12>
- OpenRocket advanced simulation and CSV documentation: <https://openrocket.readthedocs.io/en/latest/user_guide/advanced_flight_simulation.html>
- Stanford SSI Firestorm post-flight reports: <https://wiki.stanfordssi.org/L2_Post-Flight_Analyses>

Normalized source inventory, provenance labels, hashes, OpenRocket `.ork` files, settings manifests, and exported CSV evidence are committed under `phase8/`. The completion audit consumes the checked-in evidence without network access. Stanford material is retained as a qualified contextual comparison because configuration and raw-data limitations prevent treating it as a numerical oracle.

## Phase 12 engine, editor, and visual references

The supplied `KSA64_Unreal_Codex_Windows_Guide.md` is research input rather
than an accepted implementation contract. `phase12/ENGINE_DECISION.md`,
`phase12/PLAN.md`, and the Phase 11.5 handoff control when the guide differs
from the current repository. In particular, Phase 12 uses a live session before
claiming live operation and separates the GNSS-loss operations proof from the
complete global engineering replay.

Primary Unreal references:

- Unreal Engine 5.8 announcement: <https://www.unrealengine.com/news/unreal-engine-5-8-is-now-available>
- UE 5.8 release notes and Windows toolchain: <https://dev.epicgames.com/documentation/unreal-engine/unreal-engine-5-8-release-notes>
- Hardware and software specifications: <https://dev.epicgames.com/documentation/unreal-engine/hardware-and-software-specifications-for-unreal-engine>
- Unreal MCP setup, security limitations, and tool execution: <https://dev.epicgames.com/documentation/unreal-engine/unreal-mcp-in-unreal-editor>

Unreal MCP is experimental, incomplete, local and unauthenticated by default,
and serializes editor tool calls on the game thread. It is optional supervised
development tooling, never an evidence source, CI dependency, or shipped
product requirement.

Primary NASA visual-material references:

- NASA 3D Resources: <https://science.nasa.gov/3d-resources/>
- NASA Images and Media Usage Guidelines: <https://www.nasa.gov/nasa-brand-center/images-and-media/>

NASA content may be used later as visual/reference input only after per-asset
rights, credit, third-party content, hash, and transformation review. Every
such asset declares `engineering_authority: false`; KSA64's accepted packs and
telemetry remain the source for vehicle geometry, frames, events, and physical
state.
