# Unified command and compatibility guide

Phase 11.5 makes `ksa64` the primary host entrypoint. Phase-numbered programs remain compatibility and engineering surfaces through at least Phase 13.

## Start here

From the repository root:

```powershell
cargo run -p ksa64-host --bin ksa64 --
cargo run -p ksa64-host --bin ksa64 -- catalog list
cargo run -p ksa64-host --bin ksa64 -- catalog show ksa-g10r.operations
```

The flagship guided experience is:

```powershell
cargo run -p ksa64-host --bin ksa64 -- mission control ksa-g10r.operations --scenario gnss-loss
```

Running `ksa64` with no arguments is read-only. It prints discovery and quick-start commands; it does not start VICE, hardware, a target build, or a long mission.

## Primary command surface

| Domain | Commands |
|---|---|
| Discovery | `catalog list`, `catalog show`, `catalog export` |
| Authoring | `project lint`, `compile`, `run`, `script` |
| Missions | `mission run`, `mission control` |
| Campaigns | `campaign run` |
| Optimization | `optimize compile`, `run`, `run-manifest`, `serve` |
| Evidence | `evidence inspect`, `verify`, `replay`, `debrief` |
| C64 targets | `target list`, `show`, `build`, `verify`, `probe --live` |
| Historical validation | `audit list`, `audit run` |

Use `--json` before or after a subcommand for deterministic structured discovery, diagnostics, and outcomes. Execution-only settings such as display, pace, worker count, and output paths do not become experiment identities.

## Migration table

| Historical invocation | Unified equivalent | Compatibility status |
|---|---|---|
| `ksa64-host capture OUT.kst` | `ksa64 capture OUT.kst` | Hidden exact alias retained |
| `ksa64-host inspect IN.kst` | `ksa64 inspect IN.kst` | Hidden exact alias retained |
| `ksa64-host phase2-capture OUT.kst2` | `ksa64 phase2-capture OUT.kst2` | Hidden exact alias retained |
| `ksa64-host phase2-inspect IN.kst2` | `ksa64 phase2-inspect IN.kst2` | Hidden exact alias retained |
| `phase11 lint SOURCE` | `ksa64 project lint SOURCE` | Thin wrapper retained |
| `phase11 compile SOURCE OUT` | `ksa64 project compile SOURCE --output OUT` | Thin wrapper retained |
| `phase11 run SOURCE OUT` | `ksa64 project run SOURCE --output OUT` | Thin wrapper retained |
| `phase11 script SOURCE OUT` | `ksa64 project script SOURCE --output OUT` | Thin wrapper retained; output bytes exact |
| `phase11 replay SESSION` | `ksa64 evidence replay SESSION` | Thin wrapper retained |
| `phase11 verify SESSION` | `ksa64 evidence verify SESSION` | Thin wrapper retained |
| `phase11 debrief SESSION DIR` | `ksa64 evidence debrief SESSION --output DIR` | Thin wrapper retained |
| `phase11_mission_control gnss-loss guided fast` | `ksa64 mission control ksa-g10r.operations --scenario gnss-loss --role guided-operator --pace fast` | Thin wrapper retained |
| `phase9 ...` | `ksa64 optimize ...` with a catalog workbench ID | Historical optimizer retained for exact specialist workflows |
| `phaseN/complete.ps1` | `ksa64 audit run N` | Original audit remains authority |

The specialized Phase 6–10 launchers, bridges, reference generators, and target probes remain available. The catalog links to them without pretending every historical switch belongs in the primary product surface.

## Safety boundaries

- `target verify` reads stored evidence only.
- `target probe` refuses to run without `--live`.
- `audit run` does not request live VICE unless `--live-vice` is present.
- Commands use explicit argument arrays and validated workspace paths; the shell never assembles command strings.
- Existing one-instance, warp-disabled, cooldown, cleanup, and long-run confirmation rules remain in force.
