[CmdletBinding()]
param(
    [string]$RepositoryRoot = "",
    [switch]$AllowDirty,
    [switch]$VerifyOnly
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Get-Sha256([string]$Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
        $digest = $algorithm.ComputeHash($stream)
        return [System.BitConverter]::ToString($digest).Replace("-", "").ToLowerInvariant()
    }
    finally {
        $algorithm.Dispose()
        $stream.Dispose()
    }
}

function Read-ExactSingleManifest([string]$BinariesDirectory) {
    $items = @(Get-ChildItem -LiteralPath $BinariesDirectory `
        -Filter "ksa64_viewer_bridge-*.manifest.json" -File -ErrorAction SilentlyContinue)
    if ($items.Count -ne 1) {
        throw "Expected exactly one staged bridge manifest in '$BinariesDirectory'; found $($items.Count)."
    }
    return $items[0]
}

if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    $RepositoryRoot = Split-Path -Parent $PSScriptRoot
}
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$cargoToml = Join-Path $root "Cargo.toml"
$headerSource = Join-Path $root "viewer-bridge\ksa64_viewer_bridge.h"
$catalogPath = Join-Path $root "phase11_5\product-catalog-v1.json"
$pluginRoot = Join-Path $root "foundry\Ksa64MissionFoundry\Plugins\Ksa64Bridge"
$binaries = Join-Path $pluginRoot "Binaries\Win64"
$includeDirectory = Join-Path $pluginRoot "Source\ThirdParty\ViewerBridge\include"

foreach ($required in @($cargoToml, $headerSource, $catalogPath, $pluginRoot)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "Required Phase 12A bridge input does not exist: $required"
    }
}

if ($VerifyOnly) {
    $manifestFile = Read-ExactSingleManifest $binaries
    $manifest = Get-Content -LiteralPath $manifestFile.FullName -Raw | ConvertFrom-Json
    if ($manifest.schema -ne "ksa64.viewer-bridge-manifest.v1") {
        throw "Unexpected bridge manifest schema '$($manifest.schema)'."
    }
    $dll = Join-Path $binaries $manifest.dll_filename
    if (-not (Test-Path -LiteralPath $dll)) {
        throw "Staged bridge DLL is missing: $dll"
    }
    if ((Get-Sha256 $dll) -ne $manifest.dll_sha256) {
        throw "Staged bridge DLL SHA-256 does not match its manifest."
    }
    $header = Join-Path $includeDirectory $manifest.header_filename
    if (-not (Test-Path -LiteralPath $header) -or (Get-Sha256 $header) -ne $manifest.header_sha256) {
        throw "Staged bridge header SHA-256 does not match its manifest."
    }
    Write-Host "Verified $($manifest.dll_filename)"
    Write-Host "  commit: $($manifest.source_commit)"
    Write-Host "  DLL SHA-256: $($manifest.dll_sha256)"
    Write-Host "  catalog SHA-256: $($manifest.catalog_sha256)"
    return
}

$commit = (& git -C $root rev-parse --verify HEAD).Trim().ToLowerInvariant()
if ($LASTEXITCODE -ne 0 -or $commit -notmatch "^[0-9a-f]{40}$") {
    throw "Could not resolve a full source commit identity."
}
$dirtyLines = @(& git -C $root status --porcelain=v1 --untracked-files=normal)
if ($LASTEXITCODE -ne 0) {
    throw "Could not determine the source-tree state."
}
$sourceTreeClean = $dirtyLines.Count -eq 0
if (-not $sourceTreeClean -and -not $AllowDirty) {
    throw "The bridge source tree is dirty. Commit it before staging, or use -AllowDirty for a non-acceptance diagnostic build."
}

$headerText = Get-Content -LiteralPath $headerSource -Raw
$abiMatch = [regex]::Match($headerText, "#define\s+KSA64_VIEWER_ABI_VERSION\s+([0-9]+)u")
$buildMatch = [regex]::Match($headerText, "#define\s+KSA64_VIEWER_BUILD_IDENTITY\s+0x([0-9A-Fa-f]+)u")
if (-not $abiMatch.Success -or -not $buildMatch.Success) {
    throw "Could not extract the viewer ABI/build identities from the checked C header."
}
$abiVersion = [uint32]::Parse($abiMatch.Groups[1].Value)
$buildIdentity = [Convert]::ToUInt32($buildMatch.Groups[1].Value, 16)

$catalog = Get-Content -LiteralPath $catalogPath -Raw | ConvertFrom-Json
if ($catalog.schema -ne "ksa64.product-catalog.v1" -or $catalog.experiences.Count -ne 13) {
    throw "The source catalog is not the accepted 13-entry ksa64.product-catalog.v1 snapshot."
}
$catalogHash = Get-Sha256 $catalogPath
if ($catalogHash -ne "b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13") {
    throw "The source catalog SHA-256 is not the accepted Phase 11.5 identity."
}

$shortCommit = $commit.Substring(0, 12)
$targetDirectoryRelative = "target\viewer-bridge-staging\$shortCommit"
$targetDirectory = Join-Path $root $targetDirectoryRelative
$stagedHeader = Join-Path $includeDirectory "ksa64_viewer_bridge.h"
if ((Test-Path -LiteralPath $stagedHeader) -and
    (Get-Sha256 $stagedHeader) -ne (Get-Sha256 $headerSource)) {
    throw "The checked Unreal C header mirror differs from viewer-bridge/ksa64_viewer_bridge.h. Synchronize and commit it before staging."
}

Push-Location $root
try {
    & cargo build --locked --target x86_64-pc-windows-msvc --target-dir $targetDirectoryRelative --profile viewer --package ksa64-viewer-bridge
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo failed to build the viewer bridge."
    }
}
finally {
    Pop-Location
}

$builtDll = Join-Path $targetDirectory "x86_64-pc-windows-msvc\viewer\ksa64_viewer_bridge.dll"
if (-not (Test-Path -LiteralPath $builtDll)) {
    throw "Cargo completed without the expected MSVC DLL: $builtDll"
}

New-Item -ItemType Directory -Path $binaries -Force | Out-Null
New-Item -ItemType Directory -Path $includeDirectory -Force | Out-Null

# Keep the staging directory unambiguous. Only files owned by this script are
# replaced; UnrealBuildTool never performs this operation.
Get-ChildItem -LiteralPath $binaries -File -ErrorAction SilentlyContinue |
    Where-Object {
        $_.Name -like "ksa64_viewer_bridge-*.dll" -or
        $_.Name -like "ksa64_viewer_bridge-*.manifest.json"
    } |
    ForEach-Object { Remove-Item -LiteralPath $_.FullName -Force }

$qualifiedBase = "ksa64_viewer_bridge-$($shortCommit)-$($buildIdentity.ToString('x8'))"
$dllFilename = "$qualifiedBase.dll"
$manifestFilename = "$qualifiedBase.manifest.json"
$stagedDll = Join-Path $binaries $dllFilename
$manifestPath = Join-Path $binaries $manifestFilename

Copy-Item -LiteralPath $builtDll -Destination $stagedDll
Copy-Item -LiteralPath $headerSource -Destination $stagedHeader -Force

$postBuildDirtyLines = @(& git -C $root status --porcelain=v1 --untracked-files=normal)
if ($sourceTreeClean -and $postBuildDirtyLines.Count -ne 0) {
    throw "The source tree changed during bridge staging; refusing to qualify the artifact as clean."
}

$manifest = [ordered]@{
    schema = "ksa64.viewer-bridge-manifest.v1"
    abi_version = $abiVersion
    build_identity = $buildIdentity
    source_commit = $commit
    source_tree_clean = $sourceTreeClean
    target_triple = "x86_64-pc-windows-msvc"
    cargo_profile = "viewer"
    build_command = "cargo build --locked --target x86_64-pc-windows-msvc --target-dir target/viewer-bridge-staging/$shortCommit --profile viewer --package ksa64-viewer-bridge"
    dll_filename = $dllFilename
    dll_sha256 = Get-Sha256 $stagedDll
    header_filename = "ksa64_viewer_bridge.h"
    header_sha256 = Get-Sha256 $stagedHeader
    catalog_schema = "ksa64.product-catalog.v1"
    catalog_count = 13
    catalog_sha256 = $catalogHash
    structure_sizes = [ordered]@{
        abi_info = 132
        span = 24
        owned_buffer = 32
        event = 24
        snapshot = 184
    }
}
$json = $manifest | ConvertTo-Json -Depth 5
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)
[System.IO.File]::WriteAllText($manifestPath, "$json`n", $utf8NoBom)

if (-not $sourceTreeClean) {
    Write-Warning "Staged a dirty diagnostic build. The Unreal runtime intentionally rejects it."
}

& $PSCommandPath -RepositoryRoot $root -VerifyOnly
if ($LASTEXITCODE -ne 0) {
    throw "Staged bridge verification failed."
}
