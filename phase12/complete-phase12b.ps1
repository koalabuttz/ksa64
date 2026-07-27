[CmdletBinding()]
param(
    [switch]$SkipPhase12A,
    [switch]$SkipLegacy,
    [switch]$SkipMos,
    [switch]$SkipHarness,
    [switch]$SkipExtendedRustCases,
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
$auditRoot = Join-Path $projectRoot (".phase12b-audit-" + [Guid]::NewGuid().ToString("N"))
$unrealProject = Join-Path $projectRoot "foundry\Ksa64MissionFoundry\Ksa64MissionFoundry.uproject"
$acceptedKsb11Sha256 = "7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4"
$acceptedKsb11Bytes = 2911464

function Check {
    if ($LASTEXITCODE -ne 0) { throw "command failed: $LASTEXITCODE" }
}

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

function Assert-Phase12bRecord {
    $record = Get-Content -LiteralPath (Join-Path $projectRoot "phase12\phase12b-completion-audit.json") -Raw | ConvertFrom-Json
    if (
        $record.schema -ne "ksa64.phase12b.completion-audit.v1" -or
        $record.full_reference_session.releases -ne 21591 -or
        $record.full_reference_session.elapsed_seconds -ne "674.71875" -or
        $record.full_reference_session.actions -ne 4 -or
        $record.full_reference_session.ksb11.bytes -ne 2911464 -or
        $record.full_reference_session.ksb11.sha256 -ne $acceptedKsb11Sha256 -or
        $record.full_reference_session.ktt10.bytes -ne 175232 -or
        $record.full_reference_session.ktt10.sha256 -ne "456c512825388b7df1d65c1fa8f08a0c086c4be794c6912cc7e1223cd406e2e1" -or
        $record.full_reference_session.kph10.bytes -ne 32896 -or
        $record.full_reference_session.kph10.sha256 -ne "cef09c40f95fd75f52ec7a15f8e9db0e12f9d2ffd12b6c107bbc4c6cfb853223" -or
        $record.full_reference_session.ksr10.bytes -ne 512 -or
        $record.full_reference_session.ksr10.sha256 -ne "6aee34461cc0da65b79ba1954a48a6ad90803d29857bf444a53998ae9de622d1" -or
        $record.disposition.overall -ne "DegradedSuccess" -or
        $record.disposition.objective -ne "PrimaryAchieved" -or
        $record.disposition.vehicle -ne "Nominal" -or
        $record.disposition.procedure -ne "Completed" -or
        $record.disposition.operator -ne "TimelyReference" -or
        $record.disposition.avionics -ne "DegradedOperational" -or
        $record.disposition.evidence -ne "Complete" -or
        -not $record.authority.guided_live_surfaces_truth_filtered -or
        -not $record.authority.completed_ksb11_role_neutral -or
        -not $record.authority.completed_ksb11_crosses_bridge_as_opaque_bytes
    ) { throw "Phase 12B completion-audit contract changed." }
}

function Invoke-OperationsAutomation {
    Assert-NoUnrealEditor
    $editorCmd = Join-Path $UnrealRoot "Engine\Binaries\Win64\UnrealEditor-Cmd.exe"
    if (-not (Test-Path -LiteralPath $editorCmd -PathType Leaf)) {
        throw "UnrealEditor-Cmd does not exist: $editorCmd"
    }
    $report = Join-Path $auditRoot "unreal-operations-automation"
    $log = Join-Path $auditRoot "unreal-operations-automation.log"
    New-Item -ItemType Directory -Path $report | Out-Null
    $q = [char]34
    $arguments = @(
        ($q + $unrealProject + $q),
        "-Unattended", "-NullRHI", "-NoSplash", "-NoSound", "-DDC-ForceMemoryCache",
        "-DisablePlugins=ModelContextProtocol,ToolsetRegistry,AllToolsets,PythonScriptPlugin",
        ("-ExecCmds=" + $q + "Automation RunTests KSA64.Operations; Quit" + $q),
        ("-ReportOutputPath=" + $q + $report + $q),
        ("-TestExit=" + $q + "Automation Test Queue Empty" + $q),
        ("-abslog=" + $q + $log + $q)
    )
    $process = Start-Process -FilePath $editorCmd -ArgumentList $arguments `
        -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru -Wait
    if ($process.ExitCode -ne 0) {
        throw "Unreal Operations automation failed with exit code $($process.ExitCode). See $log."
    }
    $index = Join-Path $report "index.json"
    if (-not (Test-Path -LiteralPath $index -PathType Leaf)) { throw "Unreal automation did not produce $index." }
    $result = Get-Content -LiteralPath $index -Raw | ConvertFrom-Json
    if ($result.failed -and [int]$result.failed -ne 0) { throw "Unreal Operations automation reported failures." }
    Assert-NoUnrealEditor
}

function Invoke-PackagedOperationsAcceptance([string]$ArchiveDirectory) {
    Assert-NoUnrealEditor
    $archive = [IO.Path]::GetFullPath($ArchiveDirectory)
    $gameRoot = Join-Path $archive "Windows\Ksa64MissionFoundry"
    $gameExe = Join-Path $gameRoot "Binaries\Win64\Ksa64MissionFoundry.exe"
    if (-not (Test-Path -LiteralPath $gameExe -PathType Leaf)) {
        throw "Packaged game executable is missing: $gameExe"
    }

    $evidenceDirectory = Join-Path $gameRoot "Saved\KSA64\Evidence"
    if (Test-Path -LiteralPath $evidenceDirectory) {
        $existingEvidence = @(Get-ChildItem -LiteralPath $evidenceDirectory -Filter "*.ksb11" -File)
        if ($existingEvidence.Count -ne 0) {
            throw "Packaged acceptance requires a fresh evidence directory; found $($existingEvidence.Count) KSB11 file(s)."
        }
    }

    $log = Join-Path $archive "packaged-phase12b-acceptance.log"
    $arguments = @(
        "-nullrhi",
        "-nosound",
        "-unattended",
        "-nosplash",
        "-DisablePlugins=ModelContextProtocol,ToolsetRegistry,AllToolsets,PythonScriptPlugin",
        "-Ksa64Phase12bAcceptance",
        "-abslog=$log"
    )
    $process = Start-Process -FilePath $gameExe -ArgumentList $arguments `
        -WorkingDirectory (Split-Path -Parent $gameExe) -WindowStyle Hidden -PassThru
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) {
        throw "Packaged Phase 12B acceptance exited with code $($process.ExitCode). See $log."
    }
    if (-not (Test-Path -LiteralPath $log -PathType Leaf)) {
        throw "Packaged Phase 12B acceptance log was not produced: $log"
    }
    $logText = Get-Content -LiteralPath $log -Raw
    $passMarker = "KSA64_PHASE12B_ACCEPTANCE_PASS release=21591 length=$acceptedKsb11Bytes sha256=$acceptedKsb11Sha256"
    $passCount = [regex]::Matches($logText, [regex]::Escape($passMarker)).Count
    if ($passCount -ne 1 -or $logText.Contains("KSA64_PHASE12B_ACCEPTANCE_FAIL")) {
        throw "Packaged Phase 12B acceptance did not emit exactly one exact PASS marker. See $log."
    }

    $evidence = @(Get-ChildItem -LiteralPath $evidenceDirectory -Filter "*.ksb11" -File -ErrorAction SilentlyContinue)
    if ($evidence.Count -ne 1) {
        throw "Expected exactly one packaged KSB11 artifact; found $($evidence.Count)."
    }
    $evidenceSha = Get-Sha256 $evidence[0].FullName
    if ($evidence[0].Length -ne $acceptedKsb11Bytes -or $evidenceSha -ne $acceptedKsb11Sha256) {
        throw "Packaged KSB11 identity mismatch: $($evidence[0].Length) bytes / $evidenceSha"
    }

    $record = [ordered]@{
        schema = "ksa64.phase12b.package-acceptance.v1"
        release_epoch = 21591
        evidence = [ordered]@{
            path = $evidence[0].FullName.Substring($archive.Length + 1).Replace('\', '/')
            bytes = [int64]$evidence[0].Length
            sha256 = $evidenceSha
        }
        pass_marker = $passMarker
        exit_code = $process.ExitCode
        log = $log.Substring($archive.Length + 1).Replace('\', '/')
        log_sha256 = Get-Sha256 $log
    }
    $record | ConvertTo-Json -Depth 5 | Set-Content `
        -LiteralPath (Join-Path $archive "phase12b-package-acceptance.json") -Encoding utf8NoBOM
    Assert-NoUnrealEditor
}

New-Item -ItemType Directory -Path $auditRoot | Out-Null
if ($RunPackage -and [string]::IsNullOrWhiteSpace($PackageArchive)) {
    $PackageArchive = Join-Path $projectRoot ("target\phase12b-package-" + [Guid]::NewGuid().ToString("N"))
}
Push-Location $projectRoot
try {
    Assert-Phase12bRecord

    if (-not $SkipPhase12A) {
        Gate "frozen Phase 0-12A compatibility" {
            & phase12/complete.ps1 -SkipLegacy:$SkipLegacy -SkipMos:$SkipMos -SkipHarness
        }
    }

    Gate "Phase 12B bounded Rust and ABI tests" {
        cargo test -p ksa64-host --lib --locked phase12b
        Check
        cargo test -p ksa64-viewer-bridge --lib --profile viewer --features panic-probe --locked
        Check
    }

    Gate "Phase 12B accepted full scripted evidence" {
        cargo test -p ksa64-host --lib --locked `
            phase12b_live::tests::scripted_full_mission_seals_exact_evidence_and_succeeds `
            -- --ignored --exact
    }

    Gate "inactive operations preserve the complete Phase 10 mission" {
        cargo test -p ksa64-sim --lib --features fixtures --locked `
            phase10_avionics::tests::inactive_reference_package_matches_frozen_phase10_full_mission_exactly `
            -- --ignored --exact
    }

    if (-not $SkipExtendedRustCases) {
        Gate "Phase 12B no-action and alternate recovery outcomes" {
            cargo test -p ksa64-host --lib --locked `
                phase12b_live::tests::no_action_can_finish_as_degraded_success `
                -- --ignored --exact
            Check
            cargo test -p ksa64-host --lib --locked `
                phase12b_live::tests::safe_recovery_branch_completes_as_contingency_success `
                -- --ignored --exact
            Check
        }
        Gate "Phase 12B role-neutral and application-facade replay parity" {
            cargo test -p ksa64-host --lib --locked `
                phase12b_live::tests::guided_and_scripted_action_transcripts_are_byte_identical `
                -- --ignored --exact
            Check
            cargo test -p ksa64-host --lib --locked `
                phase12b_live::tests::full_authoring_sdk_bundle_matches_direct_full_session `
                -- --ignored --exact
        }
    }

    if (-not $SkipHarness) {
        Gate "frozen and additive native C++ ABI harnesses" {
            & viewer-bridge/harness/build-all.ps1 `
                -Phase12bExpectedSha256 $acceptedKsb11Sha256 -PanicProbe
        }
    }

    if ($RunUnrealBuild -or $RunUnrealAutomation -or $RunPackage) {
        Gate "explicit inherited Phase 12A Unreal gates" {
            $arguments = @("-SkipLegacy", "-SkipMos", "-SkipHarness")
            if ($RunUnrealBuild) { $arguments += "-RunUnrealBuild" }
            if ($RunUnrealAutomation) { $arguments += "-RunUnrealAutomation" }
            if ($RunPackage) { $arguments += "-RunPackage" }
            $arguments += @("-UnrealRoot", $UnrealRoot, "-DerivedDataCache", $DerivedDataCache)
            if (-not [string]::IsNullOrWhiteSpace($PackageArchive)) {
                $arguments += @("-PackageArchive", $PackageArchive)
            }
            & phase12/complete.ps1 @arguments
        }
    }

    if ($RunUnrealAutomation) {
        Gate "explicit KSA64.Operations Unreal automation" {
            [Environment]::SetEnvironmentVariable("UE-LocalDataCachePath", [IO.Path]::GetFullPath($DerivedDataCache), "Process")
            Invoke-OperationsAutomation
        }
    }

    if ($RunPackage) {
        Gate "packaged Phase 12B full-session acceptance" {
            Invoke-PackagedOperationsAcceptance $PackageArchive
        }
    }

    Write-Host ""
    if ($RunUnrealBuild -and $RunUnrealAutomation -and $RunPackage) {
        Write-Host "PHASE 12B IMPLEMENTATION GATES: PASS"
        Write-Host "Product acceptance remains pending until presentation timing, screenshot/semantic, and accessibility evidence are recorded."
    } else {
        Write-Host "PHASE 12B CORE/BRIDGE AUDIT: PASS"
        Write-Host "Full product acceptance remains pending. Use explicit -RunUnrealBuild, -RunUnrealAutomation, and -RunPackage switches for Unreal gates."
    }
}
finally {
    Pop-Location
    $resolved = [IO.Path]::GetFullPath($auditRoot)
    if (-not $resolved.StartsWith($projectRoot + [IO.Path]::DirectorySeparatorChar)) { throw "unsafe audit cleanup" }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
