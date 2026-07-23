[CmdletBinding()]
param(
    [ValidateRange(1, 10)]
    [int]$Runs = 3
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$versions = Get-Content -Raw -LiteralPath (Join-Path $projectRoot "toolchains\versions.json") | ConvertFrom-Json
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"
$vice = Join-Path $projectRoot $versions.vice.projectRelativeExecutable.Replace("/", "\")
$artifact = Join-Path $projectRoot "target\mos-c64-none\c64\ksa64-phase3-probes-c64"
$output = Join-Path $projectRoot "phase3\timing-v1.json"

function Assert-ExitCode([string]$label) {
    if ($LASTEXITCODE -ne 0) { throw "$label failed with exit code $LASTEXITCODE." }
}

Push-Location $projectRoot
try {
    if (-not (Test-Path -LiteralPath $vice -PathType Leaf)) {
        throw "Pinned VICE is missing. Run tools/toolchains/setup-vice.ps1."
    }
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $vice).Hash.ToLowerInvariant() -ne $versions.vice.executableSha256) {
        throw "VICE executable hash does not match the pinned release."
    }
    & $rustWrapper -ReturnToCaller -WorkingDirectory "." `
        cargo build --profile c64 --target mos-c64-none --features c64 `
        -Z build-std=core -Z build-std-features=compiler-builtins-mem `
        --bin ksa64-phase3-probes-c64
    Assert-ExitCode "Phase 3 C64 probe build"
    & python -B phase3/reference/vice_probes.py `
        --vice $vice --prg $artifact --runs $Runs --timeout 600 --output $output --check
    Assert-ExitCode "Phase 3 finite PAL C64 probes"
    $evidence = Get-Content -Raw -LiteralPath $output | ConvertFrom-Json
    Write-Host "Composed: $([math]::Round($evidence.cycles.composed_per_step, 1)) cycles/step"
    Write-Host "GPS guidance: $([math]::Round($evidence.cycles.gps_guidance_per_step, 1)) cycles/step"
    Write-Host "Projected full PAL time: $([math]::Round($evidence.full_nominal_decision.projected_real_pal_seconds / 60, 1)) minutes"
    Write-Host "Stock RAM fit: $($evidence.artifact.stock_ram_fit); full-run eligible: $($evidence.full_nominal_decision.eligible)"
    Write-Host "PHASE 3 C64 PROBES: PASS"
} finally {
    Pop-Location
}