[CmdletBinding()]
param()
$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$versions = Get-Content -Raw -LiteralPath (Join-Path $projectRoot "toolchains\versions.json") | ConvertFrom-Json
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"
$vice = Join-Path $projectRoot $versions.vice.projectRelativeExecutable.Replace("/", "\")
$artifact = Join-Path $projectRoot "target\mos-c64-none\c64\ksa64-phase4-reu-probe-c64"
$output = Join-Path $projectRoot "phase4\reu-matrix-v1.json"
function Assert-ExitCode([string]$label) { if ($LASTEXITCODE -ne 0) { throw "$label failed with exit code $LASTEXITCODE." } }
Push-Location $projectRoot
try {
    & $rustWrapper -ReturnToCaller -WorkingDirectory "." `
        cargo build --profile c64 --target mos-c64-none --features c64 `
        -Z build-std=core -Z build-std-features=compiler-builtins-mem `
        --bin ksa64-phase4-reu-probe-c64
    Assert-ExitCode "Phase 4 REU probe build"
    $json = & python -B phase4/reference/vice_reu.py --vice $vice --prg $artifact --timeout 180 --output $output
    Assert-ExitCode "Phase 4 REU matrix"
    $result = ($json -join "`n") | ConvertFrom-Json
    foreach ($case in $result.cases) {
        Write-Host ("REU {0,5} KiB summaries={1,4} full={2,3} compact={3,3} DMA={4}/{5}/{6}" -f `
            $case.capacity_kib, $case.summary_slots, $case.full_histories, $case.compact_histories, `
            $case.dma_total_cycles.'64', $case.dma_total_cycles.'160', $case.dma_total_cycles.'256')
    }
    Write-Host "PHASE 4 REU MATRIX: PASS"
} finally { Pop-Location }