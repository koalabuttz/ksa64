[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path
$phaseRoot = Join-Path $projectRoot "phase4"

$sidecars = Get-ChildItem -LiteralPath $phaseRoot -Filter "*.sha256" -File
$sidecars += Get-ChildItem -LiteralPath (Join-Path $phaseRoot "examples") -Filter "*.sha256" -File
if ($sidecars.Count -eq 0) {
    throw "Phase 4 stored evidence has no SHA-256 sidecars."
}

foreach ($sidecar in $sidecars) {
    $line = (Get-Content -LiteralPath $sidecar.FullName -Raw).Trim()
    $parts = $line -split "\s+", 2
    if ($parts.Count -ne 2) {
        throw "Malformed sidecar: $($sidecar.FullName)"
    }
    $artifact = Join-Path $sidecar.DirectoryName $parts[1]
    if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) {
        throw "Missing Phase 4 artifact: $artifact"
    }
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $artifact).Hash.ToLowerInvariant()
    if ($actual -ne $parts[0].ToLowerInvariant()) {
        throw "Phase 4 stored evidence changed: $artifact"
    }
}

Get-Content -LiteralPath (Join-Path $phaseRoot "distribution-vectors-v1.json") -Raw |
    ConvertFrom-Json | Out-Null
Get-Content -LiteralPath (Join-Path $phaseRoot "reference-campaign-analysis.json") -Raw |
    ConvertFrom-Json | Out-Null
Get-Content -LiteralPath (Join-Path $phaseRoot "reu-matrix-v1.json") -Raw |
    ConvertFrom-Json | Out-Null

Write-Host "PHASE 4 STORED EVIDENCE AUDIT: PASS ($($sidecars.Count) hashes)"
