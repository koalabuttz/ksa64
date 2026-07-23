[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$versions = Get-Content -Raw -LiteralPath (Join-Path $projectRoot "toolchains\versions.json") | ConvertFrom-Json
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"
$vice = Join-Path $projectRoot $versions.vice.projectRelativeExecutable.Replace("/", "\")
$artifact = Join-Path $projectRoot "target\mos-c64-none\c64\ksa64-phase2-replay-c64"
$acceptedPath = Join-Path $projectRoot "phase2\replay-v1.json"

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
    & python -B phase2/reference/generate_replay.py --check
    Assert-ExitCode "Phase 2 replay tape generation check"
    & $rustWrapper -ReturnToCaller -WorkingDirectory "." `
        cargo build --profile c64 --target mos-c64-none --features c64 `
        -Z build-std=core -Z build-std-features=compiler-builtins-mem `
        --bin ksa64-phase2-replay-c64
    Assert-ExitCode "Phase 2 C64 replay build"

    $json = & python -B phase2/reference/vice_replay.py `
        --vice $vice --prg $artifact --timeout 90
    Assert-ExitCode "Phase 2 PAL VICE replay"
    $result = ($json -join "`n") | ConvertFrom-Json
    $accepted = Get-Content -Raw -LiteralPath $acceptedPath | ConvertFrom-Json
    $artifactBytes = (Get-Item -LiteralPath $artifact).Length
    $tapeBytes = (Get-Item -LiteralPath phase2/examples/ksa2a-200km.krp2).Length
    $tapeHash = (Get-FileHash -Algorithm SHA256 -LiteralPath phase2/examples/ksa2a-200km.krp2).Hash.ToLowerInvariant()
    if ([long]$result.trajectory_cells -ne [long]$accepted.trajectory_cells `
        -or $result.checksum_and_cue_hash -notlike "*CC57612B*9473FCDB" `
        -or $result.sid_schedule -ne "SID IGN  1 CUT  2 SEP  1 END  1 ALM  0" `
        -or $result.sink_memory -ne "REPLAY SINK        135 BYTES" `
        -or $artifactBytes -ne [long]$accepted.replay_prg_bytes `
        -or $tapeBytes -ne [long]$accepted.tape_bytes `
        -or $tapeHash -ne $accepted.tape_sha256) {
        throw "Phase 2 replay differs from replay-v1.json."
    }
    Write-Host $result.title
    Write-Host "Trajectory cells: $($result.trajectory_cells)"
    Write-Host $result.orbit
    Write-Host $result.checksum_and_cue_hash
    Write-Host $result.sid_schedule
    Write-Host $result.sink_memory
    Write-Host "Replay PRG: $artifactBytes bytes; tape: $tapeBytes bytes"
    Write-Host "PHASE 2 C64 REPLAY: PASS"
} finally {
    Pop-Location
}
