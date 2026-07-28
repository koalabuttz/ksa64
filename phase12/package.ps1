[CmdletBinding()]
param(
    [string]$RepositoryRoot = "",
    [string]$UnrealRoot = "D:\Games\UE_5.8",
    [string]$DerivedDataCache = "E:\Unreal\DDC",
    [string]$ArchiveDirectory = "",
    [switch]$SkipBuild
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

if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    $RepositoryRoot = Split-Path -Parent $PSScriptRoot
}
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$commit = (& git -C $root rev-parse --verify HEAD).Trim().ToLowerInvariant()
if ($LASTEXITCODE -ne 0 -or $commit -notmatch '^[0-9a-f]{40}$') {
    throw "Could not resolve the source commit."
}
$project = Join-Path $root "foundry\Ksa64MissionFoundry\Ksa64MissionFoundry.uproject"
$runUat = Join-Path $UnrealRoot "Engine\Build\BatchFiles\RunUAT.bat"
$ddc = [IO.Path]::GetFullPath($DerivedDataCache)

if (-not (Test-Path -LiteralPath $project -PathType Leaf)) {
    throw "Unreal project does not exist: $project"
}
if (-not (Test-Path -LiteralPath $runUat -PathType Leaf)) {
    throw "RunUAT does not exist: $runUat"
}
$unrealProcesses = @(Get-Process -Name UnrealEditor, UnrealEditor-Cmd -ErrorAction SilentlyContinue)
if ($unrealProcesses.Count -ne 0) {
    throw "Close Unreal Editor process(es) before the Phase 12A package gate: $($unrealProcesses.Id -join ', ')"
}
$missionFoundryProcesses = @(Get-Process -Name Ksa64MissionFoundry -ErrorAction SilentlyContinue)
if ($missionFoundryProcesses.Count -ne 0) {
    throw "Close packaged Mission Foundry process(es) before the Phase 12A package gate: $($missionFoundryProcesses.Id -join ', ')"
}

if ([string]::IsNullOrWhiteSpace($ArchiveDirectory)) {
    $ArchiveDirectory = Join-Path $root "target\phase12a-package"
}
$archive = [IO.Path]::GetFullPath($ArchiveDirectory)
if (Test-Path -LiteralPath $archive) {
    throw "Archive destination already exists. Select a new empty path: $archive"
}
New-Item -ItemType Directory -Path (Split-Path -Parent $archive) -Force | Out-Null
New-Item -ItemType Directory -Path $ddc -Force | Out-Null
[Environment]::SetEnvironmentVariable("UE-LocalDataCachePath", $ddc, "Process")

$uatArgs = @(
    "BuildCookRun",
    "-project=$project",
    "-noP4",
    "-platform=Win64",
    "-clientconfig=Development"
)
if (-not $SkipBuild) {
    $uatArgs += "-build"
}
$uatArgs += @(
    "-cook",
    "-stage",
    "-pak",
    "-archive",
    "-archivedirectory=$archive",
    '-AdditionalCookerOptions=-DisablePlugins=ModelContextProtocol,ToolsetRegistry,AllToolsets,PythonScriptPlugin -SkipZenStore',
    "-utf8output"
)

& $runUat @uatArgs
if ($LASTEXITCODE -ne 0) {
    throw "Unreal BuildCookRun failed with exit code $LASTEXITCODE."
}

$gameRoot = Join-Path $archive "Windows\Ksa64MissionFoundry"
$gameExe = Join-Path $gameRoot "Binaries\Win64\Ksa64MissionFoundry.exe"
$bridgeDirectory = Join-Path $gameRoot "Plugins\Ksa64Bridge\Binaries\Win64"
$bridgeManifests = @(Get-ChildItem -LiteralPath $bridgeDirectory -Filter "ksa64_viewer_bridge-*.manifest.json" -File -ErrorAction SilentlyContinue)
if ($bridgeManifests.Count -ne 1) {
    throw "Expected one packaged bridge manifest; found $($bridgeManifests.Count)."
}
$bridgeManifest = Get-Content -LiteralPath $bridgeManifests[0].FullName -Raw | ConvertFrom-Json
if ($bridgeManifest.schema -eq "ksa64.viewer-bridge-manifest.v1") {
    $bridgeFileName = $bridgeManifest.dll_filename
    $bridgeExpectedSha256 = $bridgeManifest.dll_sha256
    $bridgeCatalogIdentity = $bridgeManifest.catalog_sha256
}
elseif ($bridgeManifest.schema -eq "ksa64.viewer-bridge-artifact.v2") {
    $bridgeFileName = $bridgeManifest.library_file
    $bridgeExpectedSha256 = $bridgeManifest.sha256
    $bridgeCatalogIdentity = $bridgeManifest.catalog_identity
    if ($bridgeManifest.target_triple -ne "x86_64-pc-windows-msvc" -or
        $bridgeManifest.operating_system -ne "windows" -or
        $bridgeManifest.architecture -ne "x86_64") {
        throw "Packaged portable bridge manifest does not describe the required Win64 artifact."
    }
}
else {
    throw "Unsupported packaged bridge manifest schema '$($bridgeManifest.schema)'."
}
if ([string]::IsNullOrWhiteSpace($bridgeFileName) -or
    [IO.Path]::IsPathRooted($bridgeFileName) -or
    [IO.Path]::GetFileName($bridgeFileName) -ne $bridgeFileName) {
    throw "Packaged bridge manifest contains an invalid library filename."
}
if ($bridgeManifest.source_commit -ne $commit) {
    throw "Packaged bridge source commit does not match the package source commit."
}
$bridgeDll = Join-Path $bridgeDirectory $bridgeFileName
if (-not (Test-Path -LiteralPath $gameExe -PathType Leaf)) {
    throw "Packaged game executable is missing: $gameExe"
}
if (-not (Test-Path -LiteralPath $bridgeDll -PathType Leaf)) {
    throw "Packaged bridge DLL is missing: $bridgeDll"
}
if ((Get-Sha256 $bridgeDll) -ne $bridgeExpectedSha256) {
    throw "Packaged bridge DLL does not match its qualified manifest."
}

$forbiddenPluginBinaries = @(Get-ChildItem -LiteralPath $archive -Recurse -File | Where-Object {
    $_.Name -match '^(ModelContextProtocol|ToolsetRegistry|AllToolsets|PythonScriptPlugin).+\.(dll|exe)$'
})
if ($forbiddenPluginBinaries.Count -ne 0) {
    throw "Editor-only plugin binaries leaked into the package: $($forbiddenPluginBinaries.FullName -join ', ')"
}

$smokeLog = Join-Path $archive "packaged-smoke.log"
$smokeArgs = @(
    "-nullrhi",
    "-nosound",
    "-unattended",
    "-nosplash",
    "-DisablePlugins=ModelContextProtocol,ToolsetRegistry,AllToolsets,PythonScriptPlugin",
    "-abslog=$smokeLog",
    "-ExecCmds=Quit"
)
$process = Start-Process -FilePath $gameExe -ArgumentList $smokeArgs -WorkingDirectory (Split-Path -Parent $gameExe) -WindowStyle Hidden -PassThru
if (-not $process.WaitForExit(120000)) {
    Stop-Process -Id $process.Id -Force
    throw "Packaged smoke process did not honor the explicit Quit command within 120 seconds."
}
$remainingMissionFoundry = @(Get-Process -Name Ksa64MissionFoundry -ErrorAction SilentlyContinue)
if ($remainingMissionFoundry.Count -ne 0) {
    throw "Packaged smoke left Mission Foundry process(es) running: $($remainingMissionFoundry.Id -join ', ')"
}
if ($process.ExitCode -ne 0) {
    throw "Packaged smoke process exited with code $($process.ExitCode)."
}
if (-not (Test-Path -LiteralPath $smokeLog -PathType Leaf)) {
    throw "Packaged smoke log was not produced."
}
$smokeText = Get-Content -LiteralPath $smokeLog -Raw
if ($smokeText -notmatch "KSA64 viewer bridge ready") {
    throw "Packaged runtime did not report a ready KSA64 bridge."
}
if ($smokeText -match "LogKsa64Bridge: Error") {
    throw "Packaged runtime reported a KSA64 bridge error."
}

$files = @(Get-ChildItem -LiteralPath $archive -Recurse -File)
$totalBytes = [int64](($files | Measure-Object -Property Length -Sum).Sum)
$record = [ordered]@{
    schema = "ksa64.phase12a-package-audit.v1"
    source_commit = $commit
    unreal_root = [IO.Path]::GetFullPath($UnrealRoot)
    derived_data_cache = $ddc
    archive_directory = $archive
    file_count = $files.Count
    total_bytes = $totalBytes
    game_executable = $gameExe.Substring($archive.Length + 1).Replace('\', '/')
    game_executable_sha256 = Get-Sha256 $gameExe
    bridge_dll = $bridgeDll.Substring($archive.Length + 1).Replace('\', '/')
    bridge_dll_sha256 = Get-Sha256 $bridgeDll
    bridge_manifest_sha256 = Get-Sha256 $bridgeManifests[0].FullName
    bridge_source_commit = $bridgeManifest.source_commit
    bridge_abi_version = $bridgeManifest.abi_version
    bridge_build_identity = $bridgeManifest.build_identity
    catalog_sha256 = $bridgeCatalogIdentity
    editor_plugin_binaries_packaged = 0
    smoke_exit_code = $process.ExitCode
    smoke_log = $smokeLog.Substring($archive.Length + 1).Replace('\', '/')
    smoke_log_sha256 = Get-Sha256 $smokeLog
}
$auditPath = Join-Path $archive "phase12a-package-audit.json"
$record | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $auditPath -Encoding utf8NoBOM

Write-Host "PHASE 12A PACKAGE AND SMOKE: PASS"
Write-Host "  archive: $archive"
Write-Host "  files: $($files.Count)"
Write-Host "  bytes: $totalBytes"
Write-Host "  bridge: $bridgeFileName"
Write-Host "  audit: $auditPath"
