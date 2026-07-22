[CmdletBinding()]
param(
    [ValidateRange(1, 20)]
    [int]$Runs = 3
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$versions = Get-Content -Raw -LiteralPath (
    Join-Path $projectRoot "toolchains\versions.json"
) | ConvertFrom-Json
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"
$vice = Join-Path $projectRoot $versions.vice.projectRelativeExecutable.Replace("/", "\")
$artifact = Join-Path $projectRoot (
    "target\mos-c64-none\release\ksa64-phase1-production-timed-c64"
)
$palCyclesPerSecond = 985248.0
$targetRateHz = 8.0
$budgetPerStep = $palCyclesPerSecond / $targetRateHz

function Assert-ExitCode([string]$label) {
    if ($LASTEXITCODE -ne 0) { throw "$label failed with exit code $LASTEXITCODE." }
}

Push-Location $projectRoot
try {
    if (-not (Test-Path -LiteralPath $vice -PathType Leaf)) {
        throw "Pinned VICE is missing. Run tools/toolchains/setup-vice.ps1."
    }
    $viceHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $vice).Hash.ToLowerInvariant()
    if ($viceHash -ne $versions.vice.executableSha256) {
        throw "VICE executable hash does not match the pinned release."
    }

    Write-Host "== Phase 1 production timing build =="
    & $rustWrapper -ReturnToCaller -WorkingDirectory "." `
        cargo build --release --target mos-c64-none --features c64 `
        -Z build-std=core `
        -Z build-std-features=compiler-builtins-mem `
        --bin ksa64-phase1-production-timed-c64
    Assert-ExitCode "Production timing build"

    Write-Host ""
    Write-Host "== PAL VICE $($versions.vice.version), CIA1 32-bit timer =="
    $json = & python -B phase1/reference/vice_production_timing.py `
        --vice $vice `
        --prg $artifact `
        --runs $Runs `
        --timeout 240
    Assert-ExitCode "Production VICE timing"
    $payload = ($json -join "`n") | ConvertFrom-Json
    if (-not $payload.stable) { throw "Production timing was not stable." }
    $result = $payload.runs[0]
    $artifactBytes = (Get-Item -LiteralPath $artifact).Length
    if ([double]$result.dynamics_cycles_per_step -gt $budgetPerStep) {
        throw "Checked dynamics exceeded the PAL 8 Hz budget."
    }
    $accepted = Get-Content -Raw -LiteralPath phase1/production-timing-v4.json | ConvertFrom-Json
    if ([long]$result.dynamics_net_cycles -ne [long]$accepted.checked_dynamics.net_cycles `
        -or [long]$result.mission_net_cycles -ne [long]$accepted.dynamics_with_rolling_checksum.net_cycles `
        -or [long]$result.boundary_overhead_cycles -ne [long]$accepted.boundary_overhead_cycles `
        -or $artifactBytes -ne [long]$accepted.timed_prg_bytes) {
        throw "Production timing differs from production-timing-v4.json."
    }

    $rows = @(
        [pscustomobject]@{
            Path = "Checked dynamics"
            Cycles = [long]$result.dynamics_net_cycles
            CyclesPerStep = "{0:N2}" -f [double]$result.dynamics_cycles_per_step
            MarginAt8Hz = "{0:N2}" -f ($budgetPerStep - [double]$result.dynamics_cycles_per_step)
        },
        [pscustomobject]@{
            Path = "Dynamics + checksum"
            Cycles = [long]$result.mission_net_cycles
            CyclesPerStep = "{0:N2}" -f [double]$result.mission_cycles_per_step
            MarginAt8Hz = "{0:N2}" -f ($budgetPerStep - [double]$result.mission_cycles_per_step)
        }
    )
    $rows | Format-Table -AutoSize
    Write-Host ("Checksum delta: {0:N0} cycles ({1:N2}/step)." -f `
        [long]$result.checksum_overhead_cycles, `
        [double]$result.checksum_overhead_per_step)
    Write-Host ("PAL 8 Hz budget: {0:N2} cycles/step." -f $budgetPerStep)
    Write-Host "Timed PRG: $artifactBytes bytes"
    Write-Host "Boundary cost: $($result.boundary_overhead_cycles) cycles"
    Write-Host "Each path was identical across $Runs run(s)."
    Write-Host "PHASE 1 PRODUCTION TIMING GATE: PASS"
} finally {
    Pop-Location
}
