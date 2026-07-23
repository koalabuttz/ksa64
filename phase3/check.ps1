[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path

function Assert-ExitCode([string]$label) {
    if ($LASTEXITCODE -ne 0) { throw "$label failed with exit code $LASTEXITCODE." }
}

function Test-FrozenHashes {
    $sidecars = Get-ChildItem -LiteralPath (Join-Path $projectRoot "phase3") -Filter "*.sha256" -File -Recurse
    foreach ($sidecar in $sidecars) {
        $line = ([IO.File]::ReadAllText($sidecar.FullName)).Trim()
        if ($line -notmatch '^([0-9a-f]{64})  (.+)$') { throw "Malformed SHA-256 sidecar: $($sidecar.FullName)" }
        $artifact = Join-Path $sidecar.DirectoryName $Matches[2]
        if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) { throw "Missing frozen artifact: $artifact" }
        $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $artifact).Hash.ToLowerInvariant()
        if ($actual -ne $Matches[1]) { throw "Frozen artifact hash mismatch: $artifact" }
    }
    Write-Host "Frozen Phase 3 artifact hashes: PASS ($($sidecars.Count) sidecars)"
}

Push-Location $projectRoot
try {
    & python -B phase3/reference/tune_navigation.py --check
    Assert-ExitCode "Phase 3 navigation evidence"
    & python -B phase3/reference/verify_missions.py --check
    Assert-ExitCode "Phase 3 independent mission evidence"
    Test-FrozenHashes

    & cargo fmt --all -- --check
    Assert-ExitCode "Rust formatting"
    & cargo check --workspace --no-default-features
    Assert-ExitCode "no_std workspace check"
    & cargo clippy --workspace --all-targets --features ksa64-core/fixtures -- -D warnings
    Assert-ExitCode "Rust lint"
    & cargo test --workspace --all-targets --features ksa64-core/fixtures
    Assert-ExitCode "Native tests"

    Write-Host "PHASE 3 CURRENT GATES: PASS"
} finally {
    Pop-Location
}
