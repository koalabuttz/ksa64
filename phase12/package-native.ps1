[CmdletBinding()]
param(
    [string]$OutputDirectory = "dist/phase12b5",
    [switch]$SkipBuild,
    [switch]$AllowDirty
)

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

$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $repo
try {
    $qualification = "qualified"
    if ($AllowDirty) {
        $qualification = "unqualified-local"
    } else {
        $dirty = @(& git status --porcelain=v1 --untracked-files=normal)
        if ($LASTEXITCODE -ne 0) { throw "could not inspect source cleanliness" }
        if ($dirty.Count -ne 0) { throw "qualified packaging requires a clean source tree; commit changes or use -AllowDirty for an explicitly unqualified local archive" }
    }

    $rustc = & rustc -vV
    if ($LASTEXITCODE -ne 0) { throw "rustc identity query failed" }
    $targetTriple = (($rustc | Where-Object { $_ -like "host: *" }) -replace "^host: ", "").Trim()
    if (-not $targetTriple) { throw "rustc did not report a host target" }

    if ($targetTriple -match "^x86_64-pc-windows-msvc$") {
        $os = "windows"; $architecture = "x86_64"; $executable = "ksa64.exe"; $library = "ksa64_viewer_bridge.dll"; $extension = "zip"
    } elseif ($targetTriple -match "^x86_64-unknown-linux-gnu$") {
        $os = "linux"; $architecture = "x86_64"; $executable = "ksa64"; $library = "libksa64_viewer_bridge.so"; $extension = "tar.gz"
    } elseif ($targetTriple -match "^aarch64-unknown-linux-gnu$") {
        $os = "linux"; $architecture = "aarch64"; $executable = "ksa64"; $library = "libksa64_viewer_bridge.so"; $extension = "tar.gz"
    } elseif ($targetTriple -match "^aarch64-apple-darwin$") {
        $os = "macos"; $architecture = "aarch64"; $executable = "ksa64"; $library = "libksa64_viewer_bridge.dylib"; $extension = "tar.gz"
    } else {
        throw "unsupported Phase 12B.5 engineering target: $targetTriple"
    }

    if (-not $SkipBuild) {
        & cargo build -p ksa64-host --bin ksa64 --release --locked
        if ($LASTEXITCODE -ne 0) { throw "KSA64 host build failed" }
        & cargo build -p ksa64-viewer-bridge --profile viewer --locked
        if ($LASTEXITCODE -ne 0) { throw "viewer bridge build failed" }
    }

    $commit = (& git rev-parse --short=12 HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or -not $commit) { throw "could not resolve source commit" }
    $qualifiedName = "ksa64-phase12b5-$os-$architecture-$commit"
    if ($qualification -ne "qualified") { $qualifiedName = "$qualifiedName-$qualification" }
    $outputRoot = if ([System.IO.Path]::IsPathRooted($OutputDirectory)) { $OutputDirectory } else { Join-Path $repo $OutputDirectory }
    $stage = Join-Path $outputRoot $qualifiedName
    $resolvedOutputRoot = [System.IO.Path]::GetFullPath($outputRoot).TrimEnd([System.IO.Path]::DirectorySeparatorChar)
    $resolvedStage = [System.IO.Path]::GetFullPath($stage)
    if (-not $resolvedStage.StartsWith($resolvedOutputRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "refusing to prepare a staging directory outside the requested output root"
    }
    if (Test-Path -LiteralPath $resolvedStage) {
        Remove-Item -LiteralPath $resolvedStage -Recurse -Force
    }
    New-Item -ItemType Directory -Path $resolvedStage -Force | Out-Null
    $stage = $resolvedStage

    $executableSource = Join-Path $repo (Join-Path "target/release" $executable)
    $librarySource = Join-Path $repo (Join-Path "target/viewer" $library)
    foreach ($required in @($executableSource, $librarySource)) {
        if (-not (Test-Path -LiteralPath $required -PathType Leaf)) { throw "required build output missing: $required" }
    }
    Copy-Item -LiteralPath $executableSource -Destination (Join-Path $stage $executable) -Force
    Copy-Item -LiteralPath $librarySource -Destination (Join-Path $stage $library) -Force
    Copy-Item -LiteralPath (Join-Path $repo "viewer-bridge/ksa64_viewer_bridge.h") -Destination (Join-Path $stage "ksa64_viewer_bridge.h") -Force

    $libraryHash = Get-Sha256 (Join-Path $stage $library)
    $catalogIdentity = "b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13"
    $manifestJson = & cargo run --locked -p ksa64-viewer-bridge --bin bridge-manifest --quiet -- `
        $commit viewer $library $targetTriple $os $architecture $libraryHash $catalogIdentity
    if ($LASTEXITCODE -ne 0) { throw "bridge manifest generation failed" }
    $manifestJson | Set-Content -LiteralPath (Join-Path $stage "$library.json") -Encoding utf8

    @"
KSA64 Phase 12B.5 engineering archive
Target: $targetTriple
Source: $commit
Qualification: $qualification

This is an unsigned engineering build. Run 'ksa64' for product discovery.
The bridge ABI is described by ksa64_viewer_bridge.h and the adjacent manifest.
No installer, code-signing, notarization, or app-store claim is implied.
"@ | Set-Content -LiteralPath (Join-Path $stage "README.txt") -Encoding utf8

    $sumLines = Get-ChildItem -LiteralPath $stage -File | Sort-Object Name | ForEach-Object {
        "$(Get-Sha256 $_.FullName)  $($_.Name)"
    }
    $sumLines | Set-Content -LiteralPath (Join-Path $stage "SHA256SUMS") -Encoding ascii

    New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
    $archive = Join-Path $outputRoot "$qualifiedName.$extension"
    if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive -Force }
    if ($os -eq "windows") {
        Compress-Archive -LiteralPath $stage -DestinationPath $archive -CompressionLevel Optimal
    } else {
        & tar -C $outputRoot -czf $archive $qualifiedName
        if ($LASTEXITCODE -ne 0) { throw "native archive creation failed" }
    }
    Write-Output $archive
} finally {
    Pop-Location
}
