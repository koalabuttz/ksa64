[CmdletBinding()]
param(
    [switch]$SkipLegacy,
    [switch]$SkipMos,
    [switch]$RunVice,
    [switch]$TargetOnly
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$auditRoot = Join-Path $projectRoot (".phase11-5-audit-" + [Guid]::NewGuid().ToString("N"))

function Check {
    if ($LASTEXITCODE -ne 0) {
        throw "command failed: $LASTEXITCODE"
    }
}

function Gate([string]$label, [scriptblock]$action) {
    Write-Host ""
    Write-Host "=== $label ==="
    $global:LASTEXITCODE = 0
    & $action
    Check
}

function NoVice {
    $processes = Get-Process -Name x64sc, x64 -ErrorAction SilentlyContinue
    if ($processes) {
        throw "Close VICE PID(s) $($processes.Id -join ', ')"
    }
}

function Expect-Failure([scriptblock]$action, [string]$label) {
    $global:LASTEXITCODE = 0
    & $action
    if ($LASTEXITCODE -eq 0) {
        throw "$label unexpectedly succeeded"
    }
    $global:LASTEXITCODE = 0
}

function Assert-SameBytes([string]$left, [string]$right, [string]$label) {
    $leftHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $left).Hash
    $rightHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $right).Hash
    if ($leftHash -ne $rightHash) {
        throw "$label differs"
    }
}

New-Item -ItemType Directory -Path $auditRoot | Out-Null
Push-Location $projectRoot
try {
    $phase11Args = @{
        SkipLegacy = $SkipLegacy.IsPresent
        SkipMos = $SkipMos.IsPresent
        RunVice = $RunVice.IsPresent
        TargetOnly = $TargetOnly.IsPresent
    }

    Gate "frozen Phase 0-11 compatibility and target boundary" {
        & phase11/complete.ps1 @phase11Args
    }

    if (-not $TargetOnly) {
        Gate "Phase 11.5 native product audit" {
            cargo fmt --all -- --check
            Check
            cargo clippy -p ksa64-host --all-targets -- -D warnings
            Check
            cargo test -p ksa64-host
            Check
            cargo build -p ksa64-host --release --bin ksa64 --bin ksa64-host --bin phase11 --bin phase11_mission_control
        }

        Gate "deterministic catalog and quick start" {
            $ksa64 = "target/release/ksa64.exe"
            $catalogA = Join-Path $auditRoot "catalog-a.json"
            $catalogB = Join-Path $auditRoot "catalog-b.json"
            $quickA = Join-Path $auditRoot "quick-a.txt"
            $quickB = Join-Path $auditRoot "quick-b.txt"

            & $ksa64 catalog export --historical --output $catalogA
            Check
            & $ksa64 catalog export --historical --output $catalogB
            Check
            Assert-SameBytes $catalogA $catalogB "catalog exports"
            Assert-SameBytes $catalogA "phase11_5/product-catalog-v1.json" "checked catalog snapshot"

            & $ksa64 | Out-File -LiteralPath $quickA -Encoding utf8
            Check
            & $ksa64 | Out-File -LiteralPath $quickB -Encoding utf8
            Check
            Assert-SameBytes $quickA $quickB "quick-start output"
            $quick = Get-Content -LiteralPath $quickA -Raw
            if (
                $quick -notmatch "mission control ksa-g10r.operations --scenario gnss-loss" -or
                $quick -notmatch "Nothing above launches hardware or VICE implicitly"
            ) {
                throw "quick-start contract changed"
            }
        }

        Gate "unified and compatibility entrypoint parity" {
            $source = "phase11/examples/gnss-loss.json"
            $unified = Join-Path $auditRoot "unified.ksb11"
            $legacy = Join-Path $auditRoot "legacy.ksb11"
            & target/release/ksa64.exe project script $source --output $unified
            Check
            & target/release/phase11.exe script $source $legacy
            Check
            Assert-SameBytes $unified $legacy "Phase 11 session bundles"

            $hostCapture = Join-Path $auditRoot "host-capture.kst"
            $ksaCapture = Join-Path $auditRoot "ksa-capture.kst"
            & target/release/ksa64-host.exe capture $hostCapture
            Check
            & target/release/ksa64.exe capture $ksaCapture
            Check
            Assert-SameBytes $hostCapture $ksaCapture "legacy capture aliases"
        }

        Gate "stored target inventory and explicit live boundary" {
            NoVice
            $targets = @(
                "c64.firestorm.vertical",
                "c64.firestorm.spatial-replay",
                "c64.firestorm.advanced-flight",
                "c64.ksa-g10r.global-flight",
                "c64.ksa-g10r.global-replay",
                "c64.ksa-g10r.reference-ops",
                "c64.ksa-g10r.safehold"
            )
            foreach ($target in $targets) {
                & target/release/ksa64.exe target verify $target
                Check
            }
            Expect-Failure {
                & target/release/ksa64.exe target probe c64.ksa-g10r.safehold
            } "target probe without --live"
            NoVice
        }
    }

    Write-Host ""
    Write-Host "PHASE 11.5 COMPLETION AUDIT: PASS"
} finally {
    Pop-Location
    $resolved = [IO.Path]::GetFullPath($auditRoot)
    if (-not $resolved.StartsWith($projectRoot + [IO.Path]::DirectorySeparatorChar)) {
        throw "unsafe audit cleanup"
    }
    if (Test-Path -LiteralPath $resolved) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
