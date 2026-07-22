[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path
$versionsPath = Join-Path $projectRoot "toolchains\versions.json"
$versions = Get-Content -Raw -LiteralPath $versionsPath | ConvertFrom-Json
$vice = $versions.vice
$executable = Join-Path $projectRoot $vice.projectRelativeExecutable.Replace("/", "\")

if (Test-Path -LiteralPath $executable -PathType Leaf) {
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $executable).Hash.ToLowerInvariant()
    if ($hash -ne $vice.executableSha256) {
        throw "The existing VICE executable does not match the pinned release."
    }
    Write-Host "VICE $($vice.version) is already installed: $executable"
    return
}

$toolchainRoot = Join-Path $projectRoot ".toolchains\vice"
$versionRoot = Join-Path $toolchainRoot $vice.version
if (Test-Path -LiteralPath $versionRoot) {
    throw "A partial VICE installation exists at $versionRoot. Remove it before retrying."
}

$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) (
    "ksa64-vice-" + [Guid]::NewGuid().ToString("N")
)
$archive = Join-Path $temporaryRoot "vice.zip"
$expanded = Join-Path $temporaryRoot "expanded"

New-Item -ItemType Directory -Path $temporaryRoot | Out-Null
try {
    Write-Host "Downloading VICE $($vice.version)..."
    & curl.exe -L --fail --output $archive $vice.archiveUrl
    if ($LASTEXITCODE -ne 0) { throw "VICE download failed." }

    $archiveHash = (
        Get-FileHash -Algorithm SHA256 -LiteralPath $archive
    ).Hash.ToLowerInvariant()
    if ($archiveHash -ne $vice.archiveSha256) {
        throw "VICE archive hash does not match the pinned release."
    }

    Expand-Archive -LiteralPath $archive -DestinationPath $expanded
    $package = Get-ChildItem -LiteralPath $expanded -Directory | Select-Object -First 1
    if (-not $package) { throw "VICE archive did not contain a package directory." }

    $candidate = Join-Path $package.FullName "bin\x64sc.exe"
    if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
        throw "VICE archive did not contain bin\x64sc.exe."
    }
    $executableHash = (
        Get-FileHash -Algorithm SHA256 -LiteralPath $candidate
    ).Hash.ToLowerInvariant()
    if ($executableHash -ne $vice.executableSha256) {
        throw "VICE executable hash does not match the pinned release."
    }

    New-Item -ItemType Directory -Force -Path $toolchainRoot | Out-Null
    New-Item -ItemType Directory -Path $versionRoot | Out-Null
    Move-Item -LiteralPath $package.FullName -Destination $versionRoot
    Write-Host "Installed VICE $($vice.version): $executable"
} finally {
    $resolvedTemporary = [IO.Path]::GetFullPath($temporaryRoot)
    $systemTemporary = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if (
        $resolvedTemporary.StartsWith($systemTemporary, [StringComparison]::OrdinalIgnoreCase) -and
        (Split-Path -Leaf $resolvedTemporary).StartsWith("ksa64-vice-")
    ) {
        Remove-Item -LiteralPath $resolvedTemporary -Recurse -Force -ErrorAction SilentlyContinue
    }
}
