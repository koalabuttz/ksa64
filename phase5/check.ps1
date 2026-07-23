[CmdletBinding()]
param([switch]$SkipMos)

$ErrorActionPreference = "Stop"

function Invoke-Gate([scriptblock]$Command) {
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "Gate command failed with exit code $LASTEXITCODE"
    }
}

Invoke-Gate { python -B phase5/reference/generate_contract.py --check }
Invoke-Gate { python -B phase5/reference/generate_spatial_vectors.py --check }
Invoke-Gate { python -B phase5/reference/generate_rigid_body_vectors.py --check }
Invoke-Gate { python -B phase5/reference/generate_flexible_vectors.py --check }
Invoke-Gate { python -B phase5/reference/generate_spatial_world.py --check }
Invoke-Gate { python -B phase5/reference/generate_vehicle_vectors.py --check }
Invoke-Gate { python -B phase5/reference/generate_avionics_vectors.py --check }
Invoke-Gate { python -B phase5/reference/generate_guidance.py --check }
Invoke-Gate { python -B phase5/reference/verify_missions.py --check }
Invoke-Gate { cargo run -p ksa64-host --bin phase5_telemetry -- target/phase5-nominal.kst5 }
Invoke-Gate { python -B phase5/reference/verify_telemetry.py target/phase5-nominal.kst5 --check }
Invoke-Gate { python -B phase5/reference/analyze_campaign.py --ksc phase5/examples/ksa5-reference.ksc5 --ksr phase5/examples/ksa5-reference.ksr5 --output phase5/reference-campaign-analysis.json --check }
Invoke-Gate { cargo run -p ksa64-host --bin phase5_history -- target/phase5-baseline.kph5 }
Invoke-Gate { python -B phase5/reference/verify_history.py --input target/phase5-baseline.kph5 --check }
Invoke-Gate { cargo fmt --all -- --check }
Invoke-Gate { cargo check --workspace --all-targets --features fixtures }
Invoke-Gate { cargo clippy --workspace --all-targets --features fixtures -- -D warnings -A clippy::result-unit-err -A clippy::manual-is-multiple-of -A clippy::manual-flatten -A clippy::needless-range-loop -A clippy::drop-non-drop -A clippy::too-many-arguments }
Invoke-Gate { cargo test --workspace --features fixtures }
if (-not $SkipMos) {
    $rustWrapper = Join-Path (Get-Location) "tools/toolchains/rust-mos.ps1"
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . cargo build --release `
            --target mos-sim-none --features sim `
            -Z build-std=core `
            -Z build-std-features=compiler-builtins-mem `
            --bin ksa64-phase5-spatial-sim
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . sh -lc `
            "mos-sim target/mos-sim-none/release/ksa64-phase5-spatial-sim"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . cargo build --profile c64 `
            --target mos-sim-none --features sim `
            -Z build-std=core `
            -Z build-std-features=compiler-builtins-mem `
            --bin ksa64-phase5-rigid-spherical-sim `
            --bin ksa64-phase5-rigid-asymmetric-sim
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . sh -lc `
            "mos-sim target/mos-sim-none/c64/ksa64-phase5-rigid-spherical-sim"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . sh -lc `
            "mos-sim target/mos-sim-none/c64/ksa64-phase5-rigid-asymmetric-sim"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . cargo build --release `
            --target mos-sim-none --features sim `
            -Z build-std=core `
            -Z build-std-features=compiler-builtins-mem `
            --bin ksa64-phase5-flexible-sim
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . sh -lc `
            "mos-sim target/mos-sim-none/release/ksa64-phase5-flexible-sim"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . cargo build --release `
            --target mos-sim-none --features sim `
            -Z build-std=core `
            -Z build-std-features=compiler-builtins-mem `
            --bin ksa64-phase5-world-sim
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . sh -lc `
            "mos-sim target/mos-sim-none/release/ksa64-phase5-world-sim"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . cargo build --profile c64 `
            --target mos-sim-none --features sim `
            -Z build-std=core `
            -Z build-std-features=compiler-builtins-mem `
            --bin ksa64-phase5-vehicle-sim
    }
    $vehicleProbe = "target/mos-sim-none/c64/ksa64-phase5-vehicle-sim"
    if ((Get-Item -LiteralPath $vehicleProbe).Length -gt 49152) {
        throw "Phase 5 vehicle probe exceeds the 48 KiB stock-profile gate"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . sh -lc `
            "mos-sim target/mos-sim-none/c64/ksa64-phase5-vehicle-sim"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . cargo build --profile c64 `
            --target mos-sim-none --features sim `
            -Z build-std=core `
            -Z build-std-features=compiler-builtins-mem `
            --bin ksa64-phase5-avionics-sim
    }
    $avionicsProbe = "target/mos-sim-none/c64/ksa64-phase5-avionics-sim"
    if ((Get-Item -LiteralPath $avionicsProbe).Length -gt 49152) {
        throw "Phase 5 avionics probe exceeds the 48 KiB stock-profile gate"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . sh -lc `
            "mos-sim target/mos-sim-none/c64/ksa64-phase5-avionics-sim"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . cargo build --profile c64 `
            --target mos-sim-none --features sim `
            -Z build-std=core `
            -Z build-std-features=compiler-builtins-mem `
            --bin ksa64-phase5-guidance-sim
    }
    $guidanceProbe = "target/mos-sim-none/c64/ksa64-phase5-guidance-sim"
    if ((Get-Item -LiteralPath $guidanceProbe).Length -gt 49152) {
        throw "Phase 5 guidance probe exceeds the 48 KiB stock-profile gate"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . sh -lc `
            "mos-sim target/mos-sim-none/c64/ksa64-phase5-guidance-sim"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . cargo build --profile c64 `
            --target mos-sim-none --features sim `
            -Z build-std=core `
            -Z build-std-features=compiler-builtins-mem `
            --bin ksa64-phase5-telemetry-sim
    }
    $telemetryProbe = "target/mos-sim-none/c64/ksa64-phase5-telemetry-sim"
    if ((Get-Item -LiteralPath $telemetryProbe).Length -gt 49152) {
        throw "Phase 5 telemetry probe exceeds the 48 KiB stock-profile gate"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . sh -lc `
            "mos-sim target/mos-sim-none/c64/ksa64-phase5-telemetry-sim"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . cargo build --profile c64 `
            --target mos-sim-none --features sim `
            -Z build-std=core `
            -Z build-std-features=compiler-builtins-mem `
            --bin ksa64-phase5-campaign-sim
    }
    $campaignProbe = "target/mos-sim-none/c64/ksa64-phase5-campaign-sim"
    if ((Get-Item -LiteralPath $campaignProbe).Length -gt 49152) {
        throw "Phase 5 campaign probe exceeds the 48 KiB stock-profile gate"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . sh -lc `
            "mos-sim target/mos-sim-none/c64/ksa64-phase5-campaign-sim"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . cargo build --profile c64 `
            --target mos-sim-none --features sim `
            -Z build-std=core `
            -Z build-std-features=compiler-builtins-mem `
            --bin ksa64-phase5-history-sim
    }
    $historyProbe = "target/mos-sim-none/c64/ksa64-phase5-history-sim"
    if ((Get-Item -LiteralPath $historyProbe).Length -gt 49152) {
        throw "Phase 5 history probe exceeds the 48 KiB stock-profile gate"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . sh -lc `
            "mos-sim target/mos-sim-none/c64/ksa64-phase5-history-sim"
    }
    Invoke-Gate { & .\phase5\history-reu.ps1 }
    Invoke-Gate { & .\phase5\replay.ps1 }
    Invoke-Gate { & .\phase5\timing.ps1 -Runs 3 }
}
