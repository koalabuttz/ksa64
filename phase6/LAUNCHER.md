# Phase 6 deployment launcher

The Phase 6 launcher runs the accepted host-owned world with either a native flight computer or one VICE C64 flight endpoint. Passive host Mission Control can now be presented as a live F1–F7 dashboard, a compact summary, or no screen output. All modes use the same world/flight seam and may record the same recoverable host session.

## Quick start

```powershell
# Fast all-host mission: summary plus automatic KMR6 recording
powershell -File phase6/run.ps1

# Live all-host mission at the 32 Hz release rate
powershell -File phase6/run.ps1 -Pace realtime -Display tui

# One VICE C64 owns the flight computer
powershell -File phase6/run.ps1 -Flight vice -Pace realtime -Display tui
```

The VICE path builds the pinned rust-mos endpoint unless `-NoBuild` is supplied. It refuses to start while another x64sc process exists. The relay owns exactly one VICE process and closes it after success or proven failure. `-Smoke` limits a VICE launch to eight shadow-verified epochs.

## Placement matrix

| World | Flight | Mission Control | Status |
|---|---|---|---|
| Host | Host | Host | Supported, live dashboard and recording available |
| Host | Host | Disabled | Supported; no TUI because no Mission Control stream exists |
| Host | VICE | Host | Supported through the externally paced mailbox relay |
| Host | VICE | Disabled | Supported through the externally paced mailbox relay |
| VICE | Any | Any | Rejected: no C64 world endpoint exists yet |
| Host | Multiple VICE endpoints | Any | Rejected: only the flight endpoint is currently packaged |

Mission Control observes validated world-to-flight and flight-to-world cells, deterministic delayed/noisy ground tracking, and an independent estimator. It cannot generate commands. Enabling the dashboard, recording, replay, sound, or export leaves terminal and avionics evidence unchanged.

## Display modes

| Display | Behavior |
|---|---|
| `adaptive` | Default. Uses summary for `fast`; uses TUI for realtime/step on an interactive terminal, otherwise summary. |
| `tui` | Forces the interactive F1–F7 Mission Control dashboard. Requires host Mission Control. |
| `summary` | Prints terminal and Mission Control evidence after the run. |
| `none` | Suppresses presentation output; validation errors still fail visibly. |

The flagship layout is 120×40 and scales down to 80×24. F1 is the Flight Director overview; F2 trajectory; F3 GNC; F4 onboard/ground navigation; F5 vehicle; F6 network/events; and F7 the clearly labelled omniscient SIM Director. Only F7 presents simulator truth.

## Live controls

| Key | Action |
|---|---|
| F1–F7 | Select console |
| Space | Hold/resume releases |
| `.` | Release exactly one epoch while held |
| `[` / `]` | Select 0.25x, 0.5x, 1x, 2x, or MAX |
| `U` | Cycle SI, dual, and US presentation units |
| `S` | Cycle Off, Cues, and Cinematic sound profiles |
| `B` | Add a session bookmark |
| `F` | Freeze/unfreeze the displayed sample; recording continues |
| `E` | Export the current completed recording to CSV and JSON |
| `Q` | Active mission: choose stop, detach/continue headless, or cancel. Postflight: exit. |

Detaching restores the terminal immediately, resumes releases, and keeps both the mission and recorder alive. Stopping is an explicit operator action. A healthy run is never canceled merely because it is taking a long time.

The default build uses terminal-bell event cues. Building `ksa64-host` with feature `mission-control-audio` enables Rodio-generated sine cues and the richer procedural Cinematic profile; no sampled or copyrighted audio is used.

## Pace modes

| Pace | Host flight | VICE flight |
|---|---|---|
| `fast` | No intentional delay; adaptive display chooses summary | VICE starts in warp; monitor traffic still adds overhead |
| `realtime` | One epoch every 31.25 ms, about 396.6 simulated seconds | VICE starts at 1x PAL; monitor transactions make wall time longer |
| `step` | Dashboard `.` key or legacy prompt releases epochs | Dashboard `.` key or legacy relay prompt releases mailbox epochs |

During a live VICE TUI run, Mission Control controls broker release pacing. If VICE was launched at 1x PAL, selecting MAX removes broker delay but does not switch the already-running emulator into warp. The mailbox remains target-exactness evidence, not proof of end-to-end physical-link timing.

## Recording, replay, and export

Recording defaults to `target/phase6/sessions/ksa6r-<timestamp>.kmr6` for TUI, summary, and no-display runs. Use `-Record off` to disable it or `-Record <path>` to select a file. KMR6 is a noncanonical host session: every update is a compact CRC-protected record and a truncated session recovers through its last valid record.

Replay on the host:

```powershell
cargo run -p ksa64-host --bin phase6_launch -- --replay target/phase6/sessions/<session>.kmr6
```

Inspect or export without the dashboard:

```powershell
cargo run -p ksa64-host --bin phase6_session -- --input <session>.kmr6 --csv <flight>.csv --json <flight>.json
```

Replay arrows seek one sample, Home/End jump, Space plays/pauses, and `E` exports. KMR6 and its CSV/JSON products are presentation evidence; KST5 and KLR6 remain authoritative.

## Direct native command

```powershell
cargo run -p ksa64-host --bin phase6_launch -- --world host --flight host --mission-control host --pace realtime --display tui --units si --sound cues --record auto
```

Unsupported values fail immediately. The launcher never silently substitutes a different endpoint placement.
