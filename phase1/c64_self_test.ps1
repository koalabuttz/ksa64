[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$versions = Get-Content -Raw -LiteralPath (
    Join-Path $projectRoot "toolchains\versions.json"
) | ConvertFrom-Json
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"
$vice = Join-Path $projectRoot $versions.vice.projectRelativeExecutable.Replace("/", "\")
$artifact = Join-Path $projectRoot (
    "target\mos-c64-none\release\ksa64-phase1-acceptance-c64"
)

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

    Write-Host "== Phase 1 C64 acceptance build =="
    & $rustWrapper -ReturnToCaller -WorkingDirectory "." `
        cargo build --release --target mos-c64-none --features c64 `
        -Z build-std=core `
        -Z build-std-features=compiler-builtins-mem `
        --bin ksa64-phase1-acceptance-c64
    Assert-ExitCode "C64 acceptance build"

    Write-Host ""
    Write-Host "== PAL VICE C64 acceptance execution =="
    $json = & python -B phase1/reference/vice_c64_self_test.py `
        --vice $vice `
        --prg $artifact `
        --timeout 180
    Assert-ExitCode "C64 acceptance execution"
    $result = ($json -join "`n") | ConvertFrom-Json
    if ($result.failures -ne 0) { throw "C64 acceptance reported failures." }
    Write-Host "Failures: $($result.failures)"
    Write-Host "Pass colors: border=$($result.border_color), background=$($result.background_color)"
    Write-Host "Acceptance PRG: $((Get-Item -LiteralPath $artifact).Length) bytes"
    Write-Host "PHASE 1 C64 ACCEPTANCE: PASS"
} finally {
    Pop-Location
}
