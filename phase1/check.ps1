[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"

Push-Location $projectRoot
try {
    Write-Host "== Generated Phase 1 inputs =="
    & python -B phase0/reference/generate_numeric_foundation.py --check
    if ($LASTEXITCODE -ne 0) { throw "Numeric-foundation artifacts are stale." }
    & python -B phase1/reference/emit_numeric_bindings.py --check
    if ($LASTEXITCODE -ne 0) { throw "Phase 1 Rust bindings are stale." }
    & python -B phase1/reference/emit_environment_bindings.py --check
    if ($LASTEXITCODE -ne 0) { throw "Phase 1 environment bindings are stale." }
    & python -B phase1/reference/emit_force_bindings.py --check
    if ($LASTEXITCODE -ne 0) { throw "Phase 1 force bindings are stale." }
    & python -B phase1/reference/emit_transition_bindings.py --check
    if ($LASTEXITCODE -ne 0) { throw "Phase 1 transition bindings are stale." }
    & python -B phase1/reference/emit_mission_bindings.py --check
    if ($LASTEXITCODE -ne 0) { throw "Phase 1 mission bindings are stale." }
    & python -B phase1/reference/generate_high_precision.py --check
    if ($LASTEXITCODE -ne 0) { throw "Phase 1 high-precision evidence is stale." }

    Write-Host ""
    Write-Host "== Native production core =="
    & cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw "Production Rust formatting check failed." }
    & cargo check --workspace --no-default-features
    if ($LASTEXITCODE -ne 0) { throw "Production no-default core check failed." }
    & cargo clippy --workspace --all-targets --features fixtures -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw "Production Rust lint check failed." }
    & cargo test --workspace --features fixtures
    if ($LASTEXITCODE -ne 0) { throw "Native production-core tests failed." }

    $mosBuildArguments = @(
        "cargo", "build", "--release",
        "--target", "mos-sim-none",
        "--features", "sim",
        "-Z", "build-std=core",
        "-Z", "build-std-features=compiler-builtins-mem",
        "--bin", "ksa64-phase1-numeric-sim"
    )

    Write-Host ""
    Write-Host "== rust-mos exact core execution =="
    & $rustWrapper -WorkingDirectory "." @mosBuildArguments
    if ($LASTEXITCODE -ne 0) { throw "rust-mos numeric runner build failed." }
    & $rustWrapper -WorkingDirectory "." sh -lc `
        "mos-sim target/mos-sim-none/release/ksa64-phase1-numeric-sim"
    if ($LASTEXITCODE -ne 0) { throw "rust-mos core self-tests failed." }

    Write-Host ""
    Write-Host "== C64 production artifacts =="
    & $rustWrapper -WorkingDirectory "." cargo build --release `
        --target mos-c64-none `
        --features c64 `
        -Z build-std=core `
        -Z build-std-features=compiler-builtins-mem `
        --bin ksa64-phase1-numeric-c64 `
        --bin ksa64-phase1-telemetry-status-c64
    if ($LASTEXITCODE -ne 0) { throw "C64 production artifact build failed." }

    $numericArtifact = Get-Item -LiteralPath `
        (Join-Path $projectRoot "target\mos-c64-none\release\ksa64-phase1-numeric-c64")
    $statusArtifact = Get-Item -LiteralPath `
        (Join-Path $projectRoot "target\mos-c64-none\release\ksa64-phase1-telemetry-status-c64")
    Write-Host "C64 core self-test: $($numericArtifact.Length) bytes"
    Write-Host "C64 status display: $($statusArtifact.Length) bytes"
    Write-Host ""
    Write-Host "PHASE 1 CORE GATES: PASS"
} finally {
    Pop-Location
}
