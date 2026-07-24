# Phase 6 deployment launcher

The Phase 6 launcher turns the accepted endpoint libraries into two supported everyday deployments. The world always runs on the host. The flight computer may run natively or inside one VICE C64. Mission Control may run passively on the host or be disabled.

## Quick start

The default is the fastest complete all-host mission:

```powershell
powershell -File phase6/run.ps1
```

The equivalent explicit command is:

```powershell
powershell -File phase6/run.ps1 -World host -Flight host -MissionControl host -Pace fast
```

To put the flight computer in VICE while retaining the world and Mission Control on the host:

```powershell
powershell -File phase6/run.ps1 -World host -Flight vice -MissionControl host -Pace realtime
```

The VICE path builds the pinned rust-mos endpoint unless `-NoBuild` is supplied. It refuses to start while another x64sc process exists and the mailbox harness closes its one VICE process after success or proven failure. `-Smoke` limits a VICE launch to eight shadow-verified epochs for a quick deployment check; ordinary runs always execute the complete mission.

## Placement matrix

| World | Flight | Mission Control | Status |
|---|---|---|---|
| Host | Host | Host | Supported and regression-tested |
| Host | Host | Disabled | Supported and regression-tested |
| Host | VICE | Host | Supported through the externally paced mailbox relay |
| Host | VICE | Disabled | Supported through the externally paced mailbox relay |
| VICE | Any | Any | Rejected: no C64 world endpoint exists yet |
| Host | Multiple VICE endpoints | Any | Rejected: only the flight endpoint is currently packaged |

Mission Control observes the validated world-to-flight and flight-to-world cells, runs deterministic delayed/noisy ground tracking, maintains an independent estimator, and compares its estimate with onboard navigation. Its API cannot produce flight commands. Enabling or disabling it leaves the terminal state and flight checksums identical.

## Pace modes

| Pace | Host flight | VICE flight |
|---|---|---|
| `fast` | Runs without intentional delay; normally completes in seconds | Enables VICE warp, but binary-monitor traffic still adds substantial overhead |
| `realtime` | Releases one epoch every 31.25 ms; mission duration is about 396.6 seconds | Runs a 1x PAL C64, but monitor pauses make the accepted example take about 17 minutes |
| `step` | Prompts before every one of 12,692 epochs | Prompts before every mailbox epoch |

The VICE mailbox is step-and-ack automation. Even with `-Pace realtime`, it is not proof of end-to-end live transport timing because binary-monitor access pauses the emulated machine.

## Direct native command

The native executable can also be used without PowerShell:

```powershell
cargo run -p ksa64-host --bin phase6_launch -- --world host --flight host --mission-control host --pace fast
```

Unsupported values fail immediately with an explanation; the launcher never silently substitutes a different endpoint placement.
