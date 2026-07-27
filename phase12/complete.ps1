[CmdletBinding()]
param(
    [switch]$SkipLegacy,
    [switch]$SkipMos,
    [switch]$SkipHarness,
    [switch]$RunBridgeStaging,
    [switch]$RunUnrealBuild,
    [switch]$RunUnrealAutomation,
    [switch]$RunPackage,
    [string]$UnrealRoot = "D:\Games\UE_5.8",
    [string]$DerivedDataCache = "E:\Unreal\DDC",
    [string]$PackageArchive = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$auditRoot = Join-Path $projectRoot (".phase12a-audit-" + [Guid]::NewGuid().ToString("N"))
$unrealProject = Join-Path $projectRoot "foundry\Ksa64MissionFoundry\Ksa64MissionFoundry.uproject"

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

function Check {
    if ($LASTEXITCODE -ne 0) {
        throw "command failed: $LASTEXITCODE"
    }
}

function Gate([string]$Label, [scriptblock]$Action) {
    Write-Host ""
    Write-Host "=== $Label ==="
    $global:LASTEXITCODE = 0
    & $Action
    Check
}

function Assert-NoUnrealEditor {
    $processes = @(Get-Process -Name UnrealEditor, UnrealEditor-Cmd -ErrorAction SilentlyContinue)
    if ($processes.Count -ne 0) {
        throw "Close Unreal Editor process(es) before this gate: $($processes.Id -join ', ')"
    }
}

function Assert-JsonContract([string]$Path) {
    $record = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    if (
        $record.schema -ne "ksa64.phase12a.completion-audit.v1" -or
        $record.status -ne "pass" -or
        $record.catalog.count -ne 13 -or
        $record.catalog.sha256 -ne "b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13" -or
        $record.guided_session.ksb11_sha256 -ne "38a3ef2e497b8e24d1cf53a56db85b3d8bea0bdb27586215a02ff75d0ee39dc8" -or
        $record.bridge.dll_sha256 -ne "d1605c4aa9a8b407d8e35ee76d965e404c1e7efcc357d8bd0704b73ade43272d" -or
        $record.unreal.automation.report_sha256 -ne "eeaa21ed3b440b79c54f1f25c631ca971a9c3129f79bfd214a9f8751203131af" -or
        $record.unreal.package.audit_sha256 -ne "899cdd3b98acd528727f90b12ed7f2cf25bed2df2832e0420835c91d43232972" -or
        $record.toolchain_lock_sha256 -ne (Get-Sha256 (Join-Path $projectRoot "phase12\toolchain-lock.toml"))
    ) {
        throw "Phase 12A completion-audit contract changed."
    }
}

function Invoke-UnrealBuild {
    $dotnet = Join-Path $UnrealRoot "Engine\Binaries\ThirdParty\DotNet\10.0\win-x64\dotnet.exe"
    $ubt = Join-Path $UnrealRoot "Engine\Binaries\DotNET\UnrealBuildTool\UnrealBuildTool.dll"
    foreach ($required in @($dotnet, $ubt)) {
        if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
            throw "Required Unreal build tool does not exist: $required"
        }
    }
    $stdout = Join-Path $auditRoot "unreal-build.stdout.log"
    $stderr = Join-Path $auditRoot "unreal-build.stderr.log"
    $arguments = @(
        $ubt,
        "Ksa64MissionFoundryEditor",
        "Win64",
        "Development",
        $unrealProject,
        "-NoHotReloadFromIDE",
        "-NoUBA"
    )
    $process = Start-Process -FilePath $dotnet -ArgumentList $arguments `
        -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru -Wait `
        -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    if ($process.ExitCode -ne 0) {
        throw "Unreal Editor-target build failed with exit code $($process.ExitCode). See $stdout and $stderr."
    }
}

function Invoke-UnrealAutomation {
    Assert-NoUnrealEditor
    $editorCmd = Join-Path $UnrealRoot "Engine\Binaries\Win64\UnrealEditor-Cmd.exe"
    if (-not (Test-Path -LiteralPath $editorCmd -PathType Leaf)) {
        throw "UnrealEditor-Cmd does not exist: $editorCmd"
    }
    $report = Join-Path $auditRoot "unreal-automation"
    $log = Join-Path $auditRoot "unreal-automation.log"
    New-Item -ItemType Directory -Path $report | Out-Null
    $arguments = @(
        "`"$unrealProject`"",
        "-Unattended",
        "-NullRHI",
        "-NoSplash",
        "-NoSound",
        "-DDC-ForceMemoryCache",
        "-DisablePlugins=ModelContextProtocol,ToolsetRegistry,AllToolsets,PythonScriptPlugin",
        "-ExecCmds=`"Automation RunTests KSA64.Phase12A; Quit`"",
        "-ReportOutputPath=`"$report`"",
        "-TestExit=`"Automation Test Queue Empty`"",
        "-abslog=`"$log`""
    )
    $process = Start-Process -FilePath $editorCmd -ArgumentList $arguments `
        -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru -Wait
    if ($process.ExitCode -ne 0) {
        throw "Unreal automation failed with exit code $($process.ExitCode). See $log."
    }
    $index = Join-Path $report "index.json"
    if (-not (Test-Path -LiteralPath $index -PathType Leaf)) {
        throw "Unreal automation did not produce $index."
    }
    $result = Get-Content -LiteralPath $index -Raw | ConvertFrom-Json
    if ($result.failed -and [int]$result.failed -ne 0) {
        throw "Unreal automation reported failures."
    }
    Assert-NoUnrealEditor
}

New-Item -ItemType Directory -Path $auditRoot | Out-Null
Push-Location $projectRoot
try {
    Gate "frozen Phase 0-11.5 compatibility" {
        & phase11_5/complete.ps1 -SkipLegacy:$SkipLegacy -SkipMos:$SkipMos
    }

    Gate "Phase 12A native and bridge audit" {
        cargo fmt --all -- --check
        Check
        cargo clippy --workspace --all-targets --features fixtures --locked -- -D warnings
        Check
        cargo test --workspace --features fixtures --locked
        Check
        cargo test -p ksa64-viewer-bridge --profile viewer --features panic-probe --locked
        Check
        Assert-JsonContract "phase12/completion-audit.json"
    }

    if (-not $SkipHarness) {
        Gate "native C++ ABI harness and contained panic" {
            & viewer-bridge/harness/build.ps1 -PanicProbe
        }
    }

    if ($RunBridgeStaging) {
        Gate "explicit clean qualified bridge staging" {
            & phase12/build-bridge.ps1 -RepositoryRoot $projectRoot
            & phase12/build-bridge.ps1 -RepositoryRoot $projectRoot -VerifyOnly
        }
    }

    if ($RunUnrealBuild) {
        Gate "explicit Unreal Editor-target build" {
            Assert-NoUnrealEditor
            [Environment]::SetEnvironmentVariable(
                "UE-LocalDataCachePath",
                [IO.Path]::GetFullPath($DerivedDataCache),
                "Process"
            )
            Invoke-UnrealBuild
            Assert-NoUnrealEditor
        }
    }

    if ($RunUnrealAutomation) {
        Gate "explicit headless Unreal automation" {
            [Environment]::SetEnvironmentVariable(
                "UE-LocalDataCachePath",
                [IO.Path]::GetFullPath($DerivedDataCache),
                "Process"
            )
            Invoke-UnrealAutomation
        }
    }

    if ($RunPackage) {
        Gate "explicit Unreal package and packaged bridge smoke" {
            Assert-NoUnrealEditor
            if ([string]::IsNullOrWhiteSpace($PackageArchive)) {
                $PackageArchive = Join-Path $projectRoot (
                    "target\phase12a-package-" + [Guid]::NewGuid().ToString("N")
                )
            }
            & phase12/package.ps1 -RepositoryRoot $projectRoot `
                -UnrealRoot $UnrealRoot `
                -DerivedDataCache $DerivedDataCache `
                -ArchiveDirectory $PackageArchive
            Assert-NoUnrealEditor
        }
    }

    Write-Host ""
    Write-Host "PHASE 12A COMPLETION AUDIT: PASS"
    if (-not ($RunBridgeStaging -or $RunUnrealBuild -or $RunUnrealAutomation -or $RunPackage)) {
        Write-Host "Bridge qualification, Unreal build, automation, packaging, and MCP were not started."
        Write-Host "Use explicit -RunBridgeStaging, -RunUnrealBuild, -RunUnrealAutomation, or -RunPackage switches for live tool gates."
    }
} finally {
    Pop-Location
    $resolved = [IO.Path]::GetFullPath($auditRoot)
    if (-not $resolved.StartsWith($projectRoot + [IO.Path]::DirectorySeparatorChar)) {
        throw "unsafe audit cleanup"
    }
    if (Test-Path -LiteralPath $resolved) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
