[CmdletBinding()]
param([switch]$SkipMos)

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

function Test-Phase5Sidecars {
    $sidecars = Get-ChildItem -LiteralPath (Join-Path $projectRoot "phase5") `
        -Recurse -File -Filter "*.sha256" | Sort-Object FullName

    if ($sidecars.Count -eq 0) {
        throw "No Phase 5 SHA-256 sidecars were found."
    }

    foreach ($sidecar in $sidecars) {
        $line = (Get-Content -LiteralPath $sidecar.FullName -Raw).Trim()
        if ($line -notmatch "^([0-9a-fA-F]{64})\s+\*?(.+)$") {
            throw "Malformed SHA-256 sidecar: $($sidecar.FullName)"
        }

        $expected = $Matches[1].ToLowerInvariant()
        $artifactName = $Matches[2].Trim()
        $artifactPath = Join-Path $sidecar.DirectoryName $artifactName
        if (-not (Test-Path -LiteralPath $artifactPath -PathType Leaf)) {
            throw "Missing artifact named by $($sidecar.Name): $artifactPath"
        }

        $actual = (Get-FileHash -LiteralPath $artifactPath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actual -ne $expected) {
            $extension = [IO.Path]::GetExtension($artifactPath).ToLowerInvariant()
            if ($extension -notin @(".json", ".rs", ".md", ".ps1", ".toml")) {
                throw "SHA-256 mismatch for $artifactPath"
            }

            $canonicalText = [IO.File]::ReadAllText($artifactPath).Replace("`r`n", "`n").Replace("`r", "`n")
            $canonicalBytes = [Text.UTF8Encoding]::new($false).GetBytes($canonicalText)
            $hasher = [Security.Cryptography.SHA256]::Create()
            try {
                $canonicalDigest = $hasher.ComputeHash($canonicalBytes)
            } finally {
                $hasher.Dispose()
            }
            $canonicalHash = -join ($canonicalDigest | ForEach-Object { $_.ToString("x2") })
            if ($canonicalHash -ne $expected) {
                throw "Canonical text SHA-256 mismatch for $artifactPath"
            }
        }
    }

    Write-Host "Verified $($sidecars.Count) Phase 5 SHA-256 sidecars."
}

Push-Location $projectRoot
try {
    Invoke-PhaseGate "PHASE 4 COMPATIBILITY AND FROZEN EVIDENCE" {
        python -B phase4/reference/generate_distributions.py --check
        if ($LASTEXITCODE -ne 0) { throw "Phase 4 distribution evidence failed." }
        python -B phase4/reference/analyze_campaign.py `
            --ksc phase4/examples/ksa4-reference.ksc4 `
            --ksr phase4/examples/ksa4-reference.ksr4 `
            --output phase4/reference-campaign-analysis.json `
            --check
        if ($LASTEXITCODE -ne 0) { throw "Phase 4 campaign evidence failed." }
    }

    Invoke-PhaseGate "PHASE 5 NATIVE, INDEPENDENT, AND FINITE TARGET AUDIT" {
        if ($SkipMos) {
            & .\phase5\check.ps1 -SkipMos
        } else {
            & .\phase5\check.ps1
        }
    }

    Invoke-PhaseGate "PHASE 5 FROZEN ARTIFACT HASH AUDIT" {
        Test-Phase5Sidecars
    }

    Write-Host ""
    if ($SkipMos) {
        Write-Host "PHASE 5 PARTIAL COMPLETION AUDIT: PASS (MOS/VICE skipped)"
    } else {
        Write-Host "PHASE 5 COMPLETION AUDIT: PASS"
    }
} finally {
    Pop-Location
}
