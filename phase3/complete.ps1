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
    Invoke-PhaseGate "PHASE 2 COMPATIBILITY AND FROZEN EVIDENCE" {
        & .\phase2\check.ps1 -SkipMos
    }
    Invoke-PhaseGate "PHASE 3 NATIVE, INDEPENDENT, AND ARTIFACT AUDIT" {
        & .\phase3\check.ps1
    }
    Invoke-PhaseGate "FINITE PAL C64 PROBES AND FULL-RUN DECISION" {
        & .\phase3\timing.ps1 -Runs 3
    }
    Invoke-PhaseGate "STRICT C64 PETSCII AND SID REPLAY" {
        & .\phase3\replay.ps1
    }

    Write-Host ""
    Write-Host "PHASE 3 COMPLETION AUDIT: PASS"
} finally {
    Pop-Location
}
