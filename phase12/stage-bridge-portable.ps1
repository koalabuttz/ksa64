[CmdletBinding()]
param(
    [ValidateSet("Win64", "Linux", "Mac")]
    [string]$Platform = "Win64",
    [string]$RepositoryRoot = "",
    [switch]$VerifyOnly
)

# Phase 12B.5 explicit bridge staging. It is intentionally separate from
# UnrealBuildTool: Cargo must never run as an implicit editor/build side effect.
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Get-Sha256([string]$Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
        return ([System.BitConverter]::ToString($algorithm.ComputeHash($stream))).Replace("-", "").ToLowerInvariant()
    } finally {
        $algorithm.Dispose()
        $stream.Dispose()
    }
}

switch ($Platform) {
    "Win64" { $targetTriple = "x86_64-pc-windows-msvc"; $operatingSystem = "windows"; $architecture = "x86_64"; $libraryPrefix = "ksa64_viewer_bridge"; $extension = ".dll" }
    "Linux" { $targetTriple = "x86_64-unknown-linux-gnu"; $operatingSystem = "linux"; $architecture = "x86_64"; $libraryPrefix = "libksa64_viewer_bridge"; $extension = ".so" }
    "Mac" { $targetTriple = "aarch64-apple-darwin"; $operatingSystem = "macos"; $architecture = "aarch64"; $libraryPrefix = "libksa64_viewer_bridge"; $extension = ".dylib" }
}

if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    $RepositoryRoot = Split-Path -Parent $PSScriptRoot
}
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$plugin = Join-Path $root "foundry\Ksa64MissionFoundry\Plugins\Ksa64Bridge"
$binaries = Join-Path $plugin ("Binaries\" + $Platform)
$headerSource = Join-Path $root "viewer-bridge\ksa64_viewer_bridge.h"
$headerDestination = Join-Path $plugin "Source\ThirdParty\ViewerBridgePortable\include\ksa64_viewer_bridge.h"
$catalog = Join-Path $root "phase11_5\product-catalog-v1.json"
$catalogHash = Get-Sha256 $catalog
$acceptedCatalog = "b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13"
if ($catalogHash -ne $acceptedCatalog) { throw "The accepted product catalog identity changed; refusing to stage a bridge." }

if ($VerifyOnly) {
    if (-not (Test-Path -LiteralPath $headerDestination) -or (Get-Sha256 $headerDestination) -ne (Get-Sha256 $headerSource)) { throw "Portable bridge header mirror differs from the canonical header." }
    $manifests = @(Get-ChildItem -LiteralPath $binaries -Filter "*.manifest.json" -File -ErrorAction Stop)
    if ($manifests.Count -ne 1) { throw "Expected exactly one bridge manifest in '$binaries'." }
    $manifest = Get-Content -LiteralPath $manifests[0].FullName -Raw | ConvertFrom-Json
    if ($manifest.schema -eq "ksa64.viewer-bridge-manifest.v1") {
        if ($Platform -ne "Win64" -or $manifest.target_triple -ne $targetTriple) { throw "The archived v1 manifest is Win64-only." }
        $library = Join-Path $binaries $manifest.dll_filename
        $expectedHash = $manifest.dll_sha256
    }
    elseif ($manifest.schema -eq "ksa64.viewer-bridge-artifact.v2") {
        if ($manifest.target_triple -ne $targetTriple -or $manifest.operating_system -ne $operatingSystem -or $manifest.architecture -ne $architecture) { throw "Manifest platform fields do not match $Platform." }
        $library = Join-Path $binaries $manifest.library_file
        $expectedHash = $manifest.sha256
    }
    else { throw "Unsupported bridge manifest schema '$($manifest.schema)'." }
    if (-not (Test-Path -LiteralPath $library) -or (Get-Sha256 $library) -ne $expectedHash) { throw "Bridge library hash verification failed." }
    Write-Host "Verified $(Split-Path $library -Leaf) for $Platform"
    return
}

$commit = (& git -C $root rev-parse --verify HEAD).Trim().ToLowerInvariant()
if ($LASTEXITCODE -ne 0 -or $commit -notmatch "^[0-9a-f]{40}$") { throw "Could not resolve a full source commit identity." }
$dirty = @(& git -C $root status --porcelain=v1 --untracked-files=normal)
if ($LASTEXITCODE -ne 0) { throw "Could not determine source-tree state." }
if ($dirty.Count -ne 0) { throw "Refusing to stage a portable bridge from a dirty source tree." }

$shortCommit = $commit.Substring(0, 12)
$targetDirectoryRelative = "target\viewer-bridge-staging\$shortCommit\$Platform"
$targetDirectory = Join-Path $root $targetDirectoryRelative
Push-Location $root
try {
    & cargo build --locked --target $targetTriple --target-dir $targetDirectoryRelative --profile viewer --package ksa64-viewer-bridge
    if ($LASTEXITCODE -ne 0) { throw "Cargo failed to build the $Platform bridge." }
} finally { Pop-Location }

$built = Join-Path $targetDirectory ($targetTriple + "\viewer\" + $libraryPrefix + $extension)
if (-not (Test-Path -LiteralPath $built)) { throw "Cargo completed without the expected library: $built" }
New-Item -ItemType Directory -Path $binaries -Force | Out-Null
New-Item -ItemType Directory -Path (Split-Path -Parent $headerDestination) -Force | Out-Null
Get-ChildItem -LiteralPath $binaries -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -like "*.manifest.json" -or $_.Extension -in ".dll", ".so", ".dylib" } | Remove-Item -Force

$qualifiedBase = "$libraryPrefix-$shortCommit-120b0001"
$libraryFilename = "$qualifiedBase$extension"
$libraryDestination = Join-Path $binaries $libraryFilename
Copy-Item -LiteralPath $built -Destination $libraryDestination
Copy-Item -LiteralPath $headerSource -Destination $headerDestination -Force
$structureSizes = [ordered]@{
    abi_info = 132; span = 24; owned_buffer = 32; event = 24; snapshot = 184
    start_request_v1 = 48; operational_view_v1 = 208; procedure_view_v1 = 376
    disposition_v1 = 72; action_proposal_v1 = 144; action_receipt_v1 = 80
    timeline_event_v1 = 136; release_sample_v1 = 112; prediction_path_header_v1 = 88
    prediction_path_point_v1 = 56; transport_status_v1 = 96; finish_status_v1 = 64
}
$manifest = [ordered]@{
    schema = "ksa64.viewer-bridge-artifact.v2"
    abi_version = 1
    build_identity = 0x120b0001
    source_commit = $commit
    profile = "viewer"
    library_file = $libraryFilename
    target_triple = $targetTriple
    operating_system = $operatingSystem
    architecture = $architecture
    sha256 = Get-Sha256 $libraryDestination
    catalog_identity = $acceptedCatalog
    structure_sizes = $structureSizes
}
$manifestPath = Join-Path $binaries "$qualifiedBase.manifest.json"
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)
[System.IO.File]::WriteAllText($manifestPath, (($manifest | ConvertTo-Json -Depth 4) + "`n"), $utf8NoBom)
& $PSCommandPath -Platform $Platform -RepositoryRoot $root -VerifyOnly
if ($LASTEXITCODE -ne 0) { throw "Portable bridge staging verification failed." }