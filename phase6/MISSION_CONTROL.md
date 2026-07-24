# Phase 6 Mission Control presentation

The Phase 6 Mission Control console is a host-native, passive presentation of the accepted KSA-6R mission. It does not command the vehicle, change the world/flight protocol, or alter canonical evidence. The full history used for plots and replay lives on the host; no C64 memory or link bandwidth was added for visualization.

## Launch

```powershell
# Live host mission, automatic plot glyphs, initial ascent view
powershell -File phase6/run.ps1 -Pace realtime -Display tui

# Start focused on the orbit view
powershell -File phase6/run.ps1 -Pace realtime -Display tui -TrajectoryView orbit

# Force terminal-safe ASCII rendering
powershell -File phase6/run.ps1 -Pace realtime -Display tui -Plot ascii -TrajectoryView ground

# Host world and Mission Control with one VICE flight computer
powershell -File phase6/run.ps1 -Flight vice -Pace realtime -Display tui
```

`-Plot auto` selects the high-resolution Braille renderer used by modern terminals. Explicit `braille` and strict `ascii` modes are also available; choose ASCII for a limited or non-Unicode terminal. `-TrajectoryView` accepts `ascent`, `orbit`, or `ground`. These settings affect presentation only.

## Console map

| Page | Purpose |
|---|---|
| F1 | Flight Director overview, phase/timeline, key trends, and GO/NO-GO matrix |
| F2 | Switchable ascent, orbit, and ground-track visualization with planned and observed paths |
| F3 | Guidance, IMU, attitude error, rates, gimbal/RCS commands, and controller trends |
| F4 | Onboard navigation, independent ground estimate, and their residuals |
| F5 | Stage stack, propellant/mass, event progress, actuator state, and model-derived load estimates |
| F6 | KLR6 link health, tracking cadence, events, checksums, bookmarks, and alarms |
| F7 | Red/orange SIM TRUTH page for simulation operations and debugging |

F2 uses `1`, `2`, and `3` to select Ascent, Orbit, and Ground Track. `Tab` and `Shift+Tab` cycle these views. `P` and `O` emphasize the planned and onboard paths. At ultra-wide sizes, F2 shows all three preview plots next to the focused view.

## Source and authority rules

Every major panel carries a source badge:

- **PLAN** — the strictly validated, frozen nominal KPH5 reference and its accepted terminal state;
- **GROUND EST** — delayed/noisy tracking and the independent ground estimator;
- **ONBOARD** — navigation and controller state transported by KLR6;
- **MODEL EST** — presentation-only calculations from observed state using accepted constants and environment tables;
- **SIM TRUTH** — omniscient world state, displayed only on F7.

The cyan planned path is not secretly generated from the current truth state. The green ground path and magenta onboard path come from their respective estimates. F1 through F6 never read SIM Director truth; an automated isolation test mutates all truth fields and requires those six rendered pages to remain byte-for-byte unchanged.

The ascent plot overlays plan, ground, and onboard trajectories with event markers and an insertion band. The orbit plot derives osculating conics, apsides, inclination, radial velocity, time to apsis, and target-orbit residuals. The ground-track plot converts inertial estimates to a rotating Earth, splits antimeridian crossings, and shows the observed ascent plus one predicted revolution. These products are operational aids, not new canonical telemetry.

## Responsive layouts

The renderer has four density tiers:

| Terminal | Behavior |
|---|---|
| 80x24 | Compact core telemetry and one readable plot |
| 100x30 to 119x39 | Standard labels and supporting metrics |
| 120x40 to 159x47 | Flagship two-column operational layout |
| 160x48 and larger | Dense histories, ribbons, matrices, and additional plot context |
| About 200x60 | Ultra-wide F2 previews for ascent, orbit, and ground track alongside the focused view |

The display uses the entire available terminal and redraws when resized. ASCII mode guarantees an ASCII-only rendered buffer for limited terminals.

## Operator controls

| Key | Action |
|---|---|
| F1-F7 | Select console page |
| 1 / 2 / 3 | Select F2 Ascent / Orbit / Ground Track |
| Tab / Shift+Tab | Cycle F2 visualization |
| P / O | Emphasize planned / onboard trajectory |
| Space | Hold/resume releases or replay |
| . | Release one epoch while held |
| [ / ] | Select 0.25x, 0.5x, 1x, 2x, or MAX |
| U | Cycle SI, dual, and US units |
| S | Cycle Off, Cues, and Cinematic sound |
| B | Add a bookmark |
| F | Freeze/unfreeze the displayed sample while recording continues |
| E | Export a completed recording |
| ? | Open the in-console help overlay |
| Q | Stop/detach/cancel during flight; exit postflight |

Replay additionally supports arrow-key seeking and Home/End jumps. Seeking rebuilds each page from the exact recorded prefix, so plots and event logs never leak future samples into an earlier replay position.

## Data and format boundary

The console retains complete `MissionControlUpdate` history for the active host session. KMR6 already records every input needed to rebuild the presentation, so replay uses the same renderer as live flight. The richer plots require no KMR6, KLR6, KLF6, or KST5 version change.

The embedded planned trajectory is parsed through the strict KPH5 reader and must match the frozen 99-point run-zero identity and CRC. If that validation ever fails, planned overlays fail visibly as **PLAN INVALID** while live telemetry remains usable. KMR6 and all visualization products remain host-only, noncanonical evidence.
