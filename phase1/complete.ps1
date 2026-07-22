[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path

function Invoke-PhaseGate([string]$label, [scriptblock]$action) {
    Write-Host ""
    Write-Host "============================================================"
    Write-Host $label
    Write-Host "============================================================"
    & $action
    if (-not $?) { throw "$label failed." }
}

Push-Location $projectRoot
try {
    Invoke-PhaseGate "CORE AND CROSS-TARGET CORRECTNESS" { & .\phase1\check.ps1 }
    Invoke-PhaseGate "C64 ACCEPTANCE EXECUTION" { & .\phase1\c64_self_test.ps1 }
    Invoke-PhaseGate "RAW/CHECKSUM TIMING" { & .\phase1\timing.ps1 -Runs 3 }
    Invoke-PhaseGate "CANONICAL TELEMETRY TIMING" { & .\phase1\telemetry_timing.ps1 -Runs 3 }
    Invoke-PhaseGate "HIGH-PRECISION COMPARISON" { & .\phase1\high_precision.ps1 }
    Invoke-PhaseGate "HOST CAPTURE AND INSPECTION" {
        & cargo run -p ksa64-host -- capture target/phase1-completion.kst
        if ($LASTEXITCODE -ne 0) { throw "Host telemetry capture failed." }
        & cargo run -p ksa64-host -- inspect target/phase1-completion.kst
        if ($LASTEXITCODE -ne 0) { throw "Host telemetry inspection failed." }
    }
    Invoke-PhaseGate "C64 STATUS DISPLAY" { & .\phase1\status_display.ps1 }

    Write-Host ""
    Write-Host "PHASE 1 COMPLETION AUDIT: PASS"
} finally {
    Pop-Location
}
