[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path
$versionsPath = Join-Path $projectRoot "toolchains\versions.json"
$versions = Get-Content -Raw -LiteralPath $versionsPath | ConvertFrom-Json

$rustWrapper = Join-Path $PSScriptRoot "rust-mos.ps1"
$oscarWrapper = Join-Path $PSScriptRoot "oscar64.ps1"

Write-Host "== rust-mos image =="
$repoDigestsJson = & docker image inspect $versions.rustMos.image --format "{{json .RepoDigests}}"

if ($LASTEXITCODE -ne 0) {
    throw "Pinned rust-mos image is not available. Pull $($versions.rustMos.image)."
}

$repoDigests = $repoDigestsJson | ConvertFrom-Json
if ($versions.rustMos.repositoryDigest -notin $repoDigests) {
    throw "The local rust-mos image does not match the pinned repository digest."
}

Write-Host "Image:  $($versions.rustMos.image)"
Write-Host "Digest: $($versions.rustMos.repositoryDigest)"

& $rustWrapper sh -lc "rustc --version --verbose && cargo --version && llvm-config --version && mos-clang --version | head -n 1"
if ($LASTEXITCODE -ne 0) {
    throw "Unable to query the rust-mos toolchain."
}

Write-Host ""
Write-Host "== rust-mos C64 smoke build =="
& $rustWrapper -WorkingDirectory "toolchains/smoke/rust-mos" cargo build --release
if ($LASTEXITCODE -ne 0) {
    throw "rust-mos smoke build failed."
}

$rustArtifact = Join-Path $projectRoot (
    "toolchains\smoke\rust-mos\target\mos-c64-none\release\" +
    "ksa64-rust-mos-smoke"
)
if (-not (Test-Path -LiteralPath $rustArtifact -PathType Leaf)) {
    throw "rust-mos smoke build did not produce the expected C64 artifact."
}

$rustFile = Get-Item -LiteralPath $rustArtifact
$rustHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $rustArtifact).Hash
Write-Host "Artifact: $($rustFile.FullName)"
Write-Host "Bytes:    $($rustFile.Length)"
Write-Host "SHA-256:  $rustHash"

Write-Host ""
Write-Host "== Oscar64 installation =="
$oscarCompiler = Join-Path (
    $projectRoot
) $versions.oscar64.projectRelativeCompiler.Replace("/", "\")

if (-not (Test-Path -LiteralPath $oscarCompiler -PathType Leaf)) {
    throw "Pinned project-local Oscar64 compiler is missing."
}

$oscarFile = Get-Item -LiteralPath $oscarCompiler
$oscarHash = (
    Get-FileHash -Algorithm SHA256 -LiteralPath $oscarCompiler
).Hash.ToLowerInvariant()

if ($oscarHash -ne $versions.oscar64.compilerSha256) {
    throw "Oscar64 compiler hash does not match the pinned release."
}

Write-Host "Compiler: $($oscarFile.FullName)"
Write-Host "Version:  $($oscarFile.VersionInfo.FileVersion)"
Write-Host "SHA-256:  $oscarHash"

Write-Host ""
Write-Host "== Oscar64 C++ C64 smoke build =="
$oscarSource = Join-Path $projectRoot "toolchains\smoke\oscar64\main.cpp"
$oscarOutputDirectory = Join-Path $projectRoot "toolchains\smoke\oscar64\out"
New-Item -ItemType Directory -Path $oscarOutputDirectory -Force | Out-Null
$oscarArtifact = Join-Path $oscarOutputDirectory "ksa64-oscar64-smoke.prg"

& $oscarWrapper "-tm=c64" "-O2" "-o=$oscarArtifact" $oscarSource
if ($LASTEXITCODE -ne 0) {
    throw "Oscar64 smoke build failed."
}

if (-not (Test-Path -LiteralPath $oscarArtifact -PathType Leaf)) {
    throw "Oscar64 smoke build did not produce the expected PRG."
}

$oscarArtifactFile = Get-Item -LiteralPath $oscarArtifact
$oscarArtifactHash = (
    Get-FileHash -Algorithm SHA256 -LiteralPath $oscarArtifact
).Hash
Write-Host "Artifact: $($oscarArtifactFile.FullName)"
Write-Host "Bytes:    $($oscarArtifactFile.Length)"
Write-Host "SHA-256:  $oscarArtifactHash"

Write-Host ""
Write-Host "TOOLCHAIN VERIFICATION: PASS"

