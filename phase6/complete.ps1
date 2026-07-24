[CmdletBinding()]
param([switch]$SkipMos)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path

function Invoke-PhaseGate([string]$Label, [scriptblock]$Action) {
    Write-Host ""
    Write-Host "============================================================"
    Write-Host $Label
    Write-Host "============================================================"
    & $Action
    if ($LASTEXITCODE -ne 0) { throw "$Label failed with exit code $LASTEXITCODE" }
}

function Assert-NoVice {
    $running = Get-Process -Name "x64sc" -ErrorAction SilentlyContinue
    if ($running) { throw "Refusing to launch another VICE instance; close PID(s) $($running.Id -join ', ')." }
}

function Assert-StockPrg([string]$Path) {
    $raw = [IO.File]::ReadAllBytes($Path)
    if ($raw.Length -lt 3) { throw "Invalid PRG: $Path" }
    $load = [BitConverter]::ToUInt16($raw, 0)
    $end = $load + $raw.Length - 2
    if ($end -gt 0xC000) { throw "$Path ends at 0x$($end.ToString('X4')), beyond the stock endpoint boundary." }
    Write-Host "$([IO.Path]::GetFileName($Path)): $($raw.Length) bytes, 0x$($load.ToString('X4'))-0x$($end.ToString('X4'))"
}

Push-Location $projectRoot
try {
    Invoke-PhaseGate "PHASE 6 NATIVE CONTRACT AND REGRESSION AUDIT" {
        cargo fmt --all -- --check
        cargo check --workspace --all-targets --features fixtures
        cargo clippy --workspace --all-targets --features fixtures -- -D warnings -A clippy::result-unit-err -A clippy::manual-is-multiple-of -A clippy::manual-flatten -A clippy::needless-range-loop -A clippy::drop-non-drop -A clippy::too-many-arguments
        cargo test --workspace --features fixtures
        cargo build -p ksa64-host --bin phase6_bridge
    }

    if (-not $SkipMos) {
        $versions = Get-Content -Raw -LiteralPath "toolchains/versions.json" | ConvertFrom-Json
        $vice = (Resolve-Path -LiteralPath $versions.vice.projectRelativeExecutable).Path
        $rustWrapper = Join-Path $projectRoot "tools/toolchains/rust-mos.ps1"
        Invoke-PhaseGate "PHASE 6 STOCK C64 PACKAGING" {
            & $rustWrapper -WorkingDirectory . cargo build --profile c64 --target mos-c64-none --features c64 -Z build-std=core -Z build-std-features=compiler-builtins-mem --bin ksa64-phase6-endpoint-probe-c64 --bin ksa64-phase6-realtime-timed-c64 --bin ksa64-phase6-mailbox-endpoint-c64 --bin ksa64-phase6-flight-endpoint-c64
            foreach ($name in @("ksa64-phase6-endpoint-probe-c64", "ksa64-phase6-realtime-timed-c64", "ksa64-phase6-mailbox-endpoint-c64", "ksa64-phase6-flight-endpoint-c64")) {
                Assert-StockPrg (Join-Path "target/mos-c64-none/c64" $name)
            }
        }

        Invoke-PhaseGate "PHASE 6 FINITE PAL TIMING AND ENDPOINT PROBES" {
            Assert-NoVice
            python -B phase6/reference/vice_realtime_timing.py --vice $vice --prg target/mos-c64-none/c64/ksa64-phase6-realtime-timed-c64 --runs 3 --output phase6/realtime-timing-v1.json --check
            Assert-NoVice
            python -B phase6/reference/vice_endpoint_probe.py --vice $vice --prg target/mos-c64-none/c64/ksa64-phase6-endpoint-probe-c64 --runs 3 --output phase6/endpoint-probe-v1.json --check
            Assert-NoVice
            python -B phase6/reference/vice_mailbox_smoke.py --warp --vice $vice --prg target/mos-c64-none/c64/ksa64-phase6-mailbox-endpoint-c64
            Assert-NoVice
            powershell -NoProfile -ExecutionPolicy Bypass -File phase6/run.ps1 -World host -Flight vice -MissionControl host -Pace fast -NoBuild -Smoke
            Assert-NoVice
        }

        Invoke-PhaseGate "PHASE 6 FROZEN FULL-FLIGHT EVIDENCE" {
            python -B phase6/reference/validate_evidence.py --evidence phase6/vice-mailbox-v1.json --mailbox-prg target/mos-c64-none/c64/ksa64-phase6-mailbox-endpoint-c64 --flight-prg target/mos-c64-none/c64/ksa64-phase6-flight-endpoint-c64
        }
    }

    Write-Host ""
    if ($SkipMos) { Write-Host "PHASE 6 PARTIAL COMPLETION AUDIT: PASS (MOS/VICE skipped)" }
    else { Write-Host "PHASE 6 SOFTWARE COMPLETION AUDIT: PASS" }
} finally {
    Pop-Location
}
