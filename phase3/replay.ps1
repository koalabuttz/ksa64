[CmdletBinding()]
param()
$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$versions = Get-Content -Raw -LiteralPath (Join-Path $projectRoot "toolchains\versions.json") | ConvertFrom-Json
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"
$vice = Join-Path $projectRoot $versions.vice.projectRelativeExecutable.Replace("/", "\")
$artifact = Join-Path $projectRoot "target\mos-c64-none\c64\ksa64-phase3-replay-c64"
function Assert-ExitCode([string]$label) { if ($LASTEXITCODE -ne 0) { throw "$label failed with exit code $LASTEXITCODE." } }
Push-Location $projectRoot
try {
    & $rustWrapper -ReturnToCaller -WorkingDirectory "." `
        cargo build --profile c64 --target mos-c64-none --features c64 `
        -Z build-std=core -Z build-std-features=compiler-builtins-mem `
        --bin ksa64-phase3-replay-c64
    Assert-ExitCode "Phase 3 replay build"
    $json = & python -B phase3/reference/vice_replay.py --vice $vice --prg $artifact --timeout 180
    Assert-ExitCode "Phase 3 replay validation"
    $result = ($json -join "`n") | ConvertFrom-Json
    $bytes = (Get-Item -LiteralPath $artifact).Length
    $accepted = Get-Content -Raw -LiteralPath (Join-Path $projectRoot "phase3\replay-v1.json") | ConvertFrom-Json
    $artifactHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $artifact).Hash.ToLowerInvariant()
    $tapeHash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $projectRoot "phase3\examples\ksa3-nominal.krp3")).Hash.ToLowerInvariant()
    $load = [BitConverter]::ToUInt16([IO.File]::ReadAllBytes($artifact), 0)
    $end = $load + $bytes - 2
    if (-not $result.passed -or $end -gt 0xC000 -or $bytes -ne [long]$accepted.replay_prg_bytes -or $artifactHash -ne $accepted.replay_prg_sha256 -or $tapeHash -ne $accepted.tape_sha256) { throw "Phase 3 replay differs from frozen replay-v1 evidence." }
    Write-Host $result.rows.'0'
    Write-Host $result.rows.'2'
    Write-Host $result.rows.'6'
    Write-Host $result.rows.'24'
    Write-Host "Replay PRG: $bytes bytes, load end 0x$($end.ToString('X4'))"
    Write-Host "PHASE 3 C64 REPLAY: PASS"
} finally { Pop-Location }