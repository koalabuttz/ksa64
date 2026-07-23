[CmdletBinding()]
param(
    [switch]$SkipMos
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"

Push-Location $projectRoot
try {
    & python -B phase2/reference/generate_contract.py --check
    if ($LASTEXITCODE -ne 0) { throw "Phase 2 generated contract is stale." }

    & cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw "Rust formatting failed." }
    & cargo check --workspace --no-default-features
    if ($LASTEXITCODE -ne 0) { throw "no_std workspace check failed." }
    & cargo clippy --workspace --all-targets --features fixtures -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw "Rust lint failed." }
    & cargo test --workspace --features fixtures
    if ($LASTEXITCODE -ne 0) { throw "Native tests failed." }

    if (-not $SkipMos) {
        & $rustWrapper -WorkingDirectory . cargo build --release `
            --target mos-sim-none --features sim `
            -Z build-std=core `
            -Z build-std-features=compiler-builtins-mem `
            --bin ksa64-phase2-contract-sim
        if ($LASTEXITCODE -ne 0) { throw "rust-mos Phase 2 contract build failed." }
        & $rustWrapper -WorkingDirectory . sh -lc `
            "mos-sim target/mos-sim-none/release/ksa64-phase2-contract-sim"
        if ($LASTEXITCODE -ne 0) { throw "rust-mos Phase 2 contract checks failed." }
    }

    Write-Host "PHASE 2 CURRENT GATES: PASS"
} finally {
    Pop-Location
}
