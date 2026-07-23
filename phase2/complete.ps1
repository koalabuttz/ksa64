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
    Invoke-PhaseGate "CORE, GENERATED EVIDENCE, AND CROSS-TARGET CORRECTNESS" {
        & .\phase2\check.ps1
    }
    Invoke-PhaseGate "VACUUM INTEGRATOR TARGET TIMING" {
        & .\phase2\vacuum_timing.ps1 -Runs 3
    }
    Invoke-PhaseGate "HOST CAPTURE AND STRICT KST2 INSPECTION" {
        & cargo run -p ksa64-host -- phase2-capture target/phase2-completion.kst2
        if ($LASTEXITCODE -ne 0) { throw "Phase 2 host telemetry capture failed." }
        & cargo run -p ksa64-host -- phase2-inspect target/phase2-completion.kst2
        if ($LASTEXITCODE -ne 0) { throw "Phase 2 host telemetry inspection failed." }
    }
    Invoke-PhaseGate "POWERED TARGET TIMING" {
        & .\phase2\timing.ps1 -Runs 3
    }
    Invoke-PhaseGate "DETERMINISTIC C64 PETSCII AND SID REPLAY" {
        & .\phase2\replay.ps1
    }

    Write-Host ""
    Write-Host "PHASE 2 COMPLETION AUDIT: PASS"
} finally {
    Pop-Location
}
