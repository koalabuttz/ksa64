[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path

Push-Location $projectRoot
try {
    & python -B phase1/reference/generate_high_precision.py --check
    if ($LASTEXITCODE -ne 0) { throw "Phase 1 high-precision evidence is stale." }

    $evidence = Get-Content -Raw -LiteralPath phase1/high-precision-v1.json | ConvertFrom-Json
    Write-Host "Fixed minus same-step Decimal:"
    Write-Host "  altitude: $($evidence.fixed_minus_same_step.altitude_m) m"
    Write-Host "  velocity: $($evidence.fixed_minus_same_step.velocity_m_s) m/s"
    Write-Host "Semi-implicit Euler minus refined RK4:"
    Write-Host "  altitude: $($evidence.same_step_minus_rk4_confirmation.altitude_m) m"
    Write-Host "  velocity: $($evidence.same_step_minus_rk4_confirmation.velocity_m_s) m/s"
    Write-Host "Fixed result minus refined RK4:"
    Write-Host "  altitude: $($evidence.fixed_minus_rk4_confirmation.altitude_m) m"
    Write-Host "  velocity: $($evidence.fixed_minus_rk4_confirmation.velocity_m_s) m/s"
    Write-Host "RK4 convergence residual:"
    Write-Host "  altitude: $($evidence.rk4_reference_minus_confirmation.altitude_m) m"
    Write-Host "  velocity: $($evidence.rk4_reference_minus_confirmation.velocity_m_s) m/s"
    Write-Host "PHASE 1 HIGH-PRECISION COMPARISON: PASS"
} finally {
    Pop-Location
}
