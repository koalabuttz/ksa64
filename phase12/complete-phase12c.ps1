[CmdletBinding()]
param(
    [switch]$SkipWorkspace,
    [switch]$SkipExactRuns,
    [switch]$SkipHarness,
    [switch]$SkipWeb,
    [switch]$RunUnrealBuild,
    [switch]$RunUnrealAutomation,
    [switch]$RunPackage,
    [switch]$RunBrowserEvidence,
    [string]$UnrealRoot = "D:\Games\UE_5.8",
    [string]$DerivedDataCache = "E:\Unreal\DDC",
    [string]$PackageArchive = "",
    [string]$BrowserEvidenceManifest = "",
    [string]$NativeHarnessEvidenceManifest = "",
    [string]$UnrealEvidenceManifest = "",
    [string]$RuntimeEvidenceManifest = "",
    [string]$CompletionEvidenceDirectory = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$targetRoot = Join-Path $projectRoot "target"
$auditRoot = Join-Path $targetRoot ("phase12c-audit-" + [Guid]::NewGuid().ToString("N"))
$unrealProject = Join-Path $projectRoot "foundry\Ksa64MissionFoundry\Ksa64MissionFoundry.uproject"
$unrealConfigDirectory = Join-Path $projectRoot "foundry\Ksa64MissionFoundry\Config"
$unrealEngineConfig = Join-Path $unrealConfigDirectory "DefaultEngine.ini"
$unrealInputConfig = Join-Path $unrealConfigDirectory "DefaultInput.ini"
$unrealEngineConfigSnapshot = $null
$unrealInputConfigSnapshot = $null
$unrealInputConfigExisted = $false
$unrealConfigSnapshotCaptured = $false

$entryCommit = "eb666cbaf3b8950218656a7ad7fe135b05385813"
$catalogSha256 = "b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13"
$gnssKsb11Sha256 = "7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4"
$phase12b5CompletionSha256 = "6309a4a33adf39a10037909c6c7a34f9215b543ad1f08a042dff4bf34eceabd5"
$phase12bAuditSha256 = "d1af9ad02ad1e1d4ff1c35c0cfe7fdd8a4a288f05a2b2a469fba14da805218f5"

$frozenNominal = [ordered]@{
    "phase10/evidence/ksa-g10r-nominal.ktt10" = "a50b4b32b1c0feb44a54fc9041c40833717b9032ce127af67a9d34c3488e824a"
    "phase10/evidence/ksa-g10r-nominal.kph10" = "cd664e8b72eff7aff1e3c4a5b7fb6859bb9d5178d3b6b6d4c2c06f2c61ed9cf2"
    "phase10/evidence/ksa-g10r-nominal.ksr10" = "9e8691933789ce6d870d561218d6888f65acb04ef24e02796be33a704c8678aa"
}

function Check {
    if ($LASTEXITCODE -ne 0) {
        throw "command failed with exit code $LASTEXITCODE"
    }
}

function Gate([string]$Label, [scriptblock]$Action) {
    Write-Host ""
    Write-Host "=== $Label ==="
    $global:LASTEXITCODE = 0
    & $Action
    Check
}

function Invoke-ExactCargoTest([string[]]$Arguments) {
    $output = @(& cargo @Arguments 2>&1)
    $exitCode = $LASTEXITCODE
    foreach ($line in $output) { Write-Host $line }
    if ($exitCode -ne 0) {
        throw "exact Cargo test failed with exit code $exitCode"
    }
    $runningOne = @($output | Select-String -Pattern "running 1 test").Count
    $passedOne = @($output | Select-String -Pattern "test result: ok\. 1 passed; 0 failed").Count
    if ($runningOne -lt 1 -or $passedOne -lt 1) {
        throw "exact Cargo gate did not execute exactly one passing test"
    }
}

function Get-Sha256([string]$Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
        return ([System.BitConverter]::ToString($algorithm.ComputeHash($stream))).Replace("-", "").ToLowerInvariant()
    }
    finally {
        $algorithm.Dispose()
        $stream.Dispose()
    }
}

function Assert-Sha256([string]$RelativePath, [string]$Expected) {
    $path = Join-Path $projectRoot $RelativePath
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "required frozen artifact is missing: $RelativePath"
    }
    $actual = Get-Sha256 $path
    if ($actual -ne $Expected) {
        throw "frozen artifact changed: $RelativePath expected=$Expected actual=$actual"
    }
}

function Assert-NoUnrealProcess {
    $processes = @(
        Get-Process -Name UnrealEditor, UnrealEditor-Cmd, Ksa64MissionFoundry -ErrorAction SilentlyContinue
    )
    if ($processes.Count -ne 0) {
        throw "Close Unreal/Mission Foundry process(es) before this gate: $($processes.Id -join ', ')"
    }
}

function Save-UnrealGeneratedConfigSnapshot {
    if (-not (Test-Path -LiteralPath $unrealEngineConfig -PathType Leaf)) {
        throw "required Unreal config is missing: $unrealEngineConfig"
    }
    $script:unrealEngineConfigSnapshot = [IO.File]::ReadAllBytes($unrealEngineConfig)
    $script:unrealInputConfigExisted = Test-Path -LiteralPath $unrealInputConfig -PathType Leaf
    if ($script:unrealInputConfigExisted) {
        $script:unrealInputConfigSnapshot = [IO.File]::ReadAllBytes($unrealInputConfig)
    }
    else {
        $script:unrealInputConfigSnapshot = $null
    }
    $script:unrealConfigSnapshotCaptured = $true
}

function Restore-UnrealGeneratedConfig {
    if (-not $script:unrealConfigSnapshotCaptured) {
        return
    }
    [IO.File]::WriteAllBytes($unrealEngineConfig, $script:unrealEngineConfigSnapshot)
    if ($script:unrealInputConfigExisted) {
        [IO.File]::WriteAllBytes($unrealInputConfig, $script:unrealInputConfigSnapshot)
    }
    elseif (Test-Path -LiteralPath $unrealInputConfig -PathType Leaf) {
        Remove-Item -LiteralPath $unrealInputConfig -Force
    }
}

function Assert-FrozenEntryEvidence {
    & git cat-file -e "$entryCommit^{commit}"
    Check
    & git merge-base --is-ancestor $entryCommit HEAD
    Check

    Assert-Sha256 "phase12/PHASE12B5_COMPLETION.md" $phase12b5CompletionSha256
    Assert-Sha256 "phase12/phase12b-completion-audit.json" $phase12bAuditSha256
    foreach ($entry in $frozenNominal.GetEnumerator()) {
        Assert-Sha256 $entry.Key $entry.Value
    }

    $phase12b = Get-Content -LiteralPath (Join-Path $projectRoot "phase12/phase12b-completion-audit.json") -Raw | ConvertFrom-Json
    if (
        $phase12b.schema -ne "ksa64.phase12b.completion-audit.v1" -or
        $phase12b.status -ne "complete" -or
        $phase12b.accepted_source.catalog_count -ne 13 -or
        $phase12b.accepted_source.catalog_sha256 -ne $catalogSha256 -or
        $phase12b.full_reference_session.releases -ne 21591 -or
        $phase12b.full_reference_session.elapsed_seconds -ne "674.71875" -or
        $phase12b.full_reference_session.actions -ne 4 -or
        $phase12b.full_reference_session.ksb11.bytes -ne 2911464 -or
        $phase12b.full_reference_session.ksb11.sha256 -ne $gnssKsb11Sha256
    ) {
        throw "frozen Phase 12B/12B.5 entry evidence changed"
    }
}

function Invoke-UnrealBuild {
    Assert-NoUnrealProcess
    $dotnet = Join-Path $UnrealRoot "Engine\Binaries\ThirdParty\DotNet\10.0\win-x64\dotnet.exe"
    $ubt = Join-Path $UnrealRoot "Engine\Binaries\DotNET\UnrealBuildTool\UnrealBuildTool.dll"
    foreach ($required in @($dotnet, $ubt, $unrealProject)) {
        if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
            throw "required Unreal build input is missing: $required"
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
        throw "Unreal Editor-target build failed with exit code $($process.ExitCode); see $stdout and $stderr"
    }
    Assert-NoUnrealProcess
}

function Invoke-UnrealAutomation {
    Assert-NoUnrealProcess
    $editor = Join-Path $UnrealRoot "Engine\Binaries\Win64\UnrealEditor-Cmd.exe"
    if (-not (Test-Path -LiteralPath $editor -PathType Leaf)) {
        throw "UnrealEditor-Cmd does not exist: $editor"
    }
    $report = Join-Path $auditRoot "unreal-phase12c-automation"
    $log = Join-Path $auditRoot "unreal-phase12c-automation.log"
    New-Item -ItemType Directory -Path $report | Out-Null
    $q = [char]34
    $arguments = @(
        ($q + $unrealProject + $q),
        "-Unattended",
        "-NullRHI",
        "-NoSplash",
        "-NoSound",
        "-NoIniChanges",
        "-NoAutoUpdate",
        "-DDC-ForceMemoryCache",
        "-DisablePlugins=ModelContextProtocol,ToolsetRegistry,AllToolsets,PythonScriptPlugin",
        ("-ExecCmds=" + $q + "Automation RunTests KSA64.Phase12C; Quit" + $q),
        ("-ReportOutputPath=" + $q + $report + $q),
        ("-TestExit=" + $q + "Automation Test Queue Empty" + $q),
        ("-abslog=" + $q + $log + $q)
    )
    $process = Start-Process -FilePath $editor -ArgumentList $arguments `
        -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru -Wait
    if ($process.ExitCode -ne 0) {
        throw "Unreal Phase 12C automation failed with exit code $($process.ExitCode); see $log"
    }
    $index = Join-Path $report "index.json"
    if (-not (Test-Path -LiteralPath $index -PathType Leaf)) {
        throw "Unreal automation did not produce $index"
    }
    $result = Get-Content -LiteralPath $index -Raw | ConvertFrom-Json
    if (
        [int]$result.succeeded -lt 9 -or
        [int]$result.failed -ne 0 -or
        [int]$result.notRun -ne 0 -or
        [int]$result.inProcess -ne 0
    ) {
        throw "Unreal Phase 12C automation did not pass every discovered test"
    }
    Assert-NoUnrealProcess
}

function Get-CompletionEvidenceFileRecord([string]$EvidenceDirectory, [string]$Path) {
    $relative = [IO.Path]::GetRelativePath($EvidenceDirectory, $Path).Replace('\', '/')
    return [ordered]@{
        path = $relative
        bytes = [int64](Get-Item -LiteralPath $Path).Length
        sha256 = Get-Sha256 $Path
    }
}

function Preserve-UnrealAutomationEvidence([string]$SourceCommit) {
    if (-not $RunUnrealAutomation) {
        throw "cannot preserve Unreal automation evidence without -RunUnrealAutomation"
    }
    if ($SourceCommit -notmatch '^[0-9a-f]{40}$') {
        throw "completion evidence requires a full lower-case source commit"
    }

    $head = (& git rev-parse HEAD).Trim().ToLowerInvariant()
    Check
    if ($head -ne $SourceCommit) {
        throw "completion evidence source commit changed during the audit"
    }

    if ([string]::IsNullOrWhiteSpace($CompletionEvidenceDirectory)) {
        $completionRoot = Join-Path $targetRoot ("phase12c-completion-" + $SourceCommit)
    }
    else {
        $completionRoot = [IO.Path]::GetFullPath($CompletionEvidenceDirectory)
    }
    $stableDirectory = Join-Path $completionRoot "unreal-automation"
    if (Test-Path -LiteralPath $stableDirectory) {
        throw "completion automation evidence requires a fresh destination: $stableDirectory"
    }

    $report = Join-Path $auditRoot "unreal-phase12c-automation"
    $log = Join-Path $auditRoot "unreal-phase12c-automation.log"
    $index = Join-Path $report "index.json"
    $html = Join-Path $report "index.html"
    foreach ($required in @($report, $log, $index, $html)) {
        if (-not (Test-Path -LiteralPath $required)) {
            throw "cannot preserve missing Unreal automation evidence: $required"
        }
    }

    $result = Get-Content -LiteralPath $index -Raw | ConvertFrom-Json
    if (
        [int]$result.succeeded -lt 9 -or
        [int]$result.failed -ne 0 -or
        [int]$result.notRun -ne 0 -or
        [int]$result.inProcess -ne 0
    ) {
        throw "cannot preserve failed or incomplete Unreal automation evidence"
    }

    New-Item -ItemType Directory -Path $completionRoot -Force | Out-Null
    $stagingDirectory = Join-Path $completionRoot (".unreal-automation-" + [Guid]::NewGuid().ToString("N") + ".staging")
    $published = $false
    try {
        New-Item -ItemType Directory -Path $stagingDirectory | Out-Null
        $stagedReport = Join-Path $stagingDirectory "report"
        $stagedLogs = Join-Path $stagingDirectory "logs"
        New-Item -ItemType Directory -Path $stagedReport, $stagedLogs | Out-Null
        Get-ChildItem -LiteralPath $report -Force | ForEach-Object {
            Copy-Item -LiteralPath $_.FullName -Destination $stagedReport -Recurse
        }
        Copy-Item -LiteralPath $log -Destination (Join-Path $stagedLogs "unreal-phase12c-automation.log")
        foreach ($buildLogName in @("unreal-build.stdout.log", "unreal-build.stderr.log")) {
            $buildLog = Join-Path $auditRoot $buildLogName
            if (Test-Path -LiteralPath $buildLog -PathType Leaf) {
                Copy-Item -LiteralPath $buildLog -Destination (Join-Path $stagedLogs $buildLogName)
            }
        }

        $files = @(
            Get-ChildItem -LiteralPath $stagingDirectory -File -Recurse |
                Sort-Object { [IO.Path]::GetRelativePath($stagingDirectory, $_.FullName).Replace('\', '/') } |
                ForEach-Object { Get-CompletionEvidenceFileRecord $stagingDirectory $_.FullName }
        )
        $manifest = [ordered]@{
            schema = "ksa64.phase12c.unreal-automation-evidence.v1"
            source = [ordered]@{
                commit = $SourceCommit
                filter = "KSA64.Phase12C"
            }
            result = [ordered]@{
                succeeded = [int]$result.succeeded
                failed = [int]$result.failed
                not_run = [int]$result.notRun
                in_process = [int]$result.inProcess
                duration_seconds = [double]$result.totalDuration
            }
            files = $files
        }
        $manifestPath = Join-Path $stagingDirectory "phase12c-unreal-automation-evidence.json"
        $temporaryManifest = $manifestPath + ".tmp"
        try {
            $manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $temporaryManifest -Encoding utf8NoBOM
            [IO.File]::Move($temporaryManifest, $manifestPath)
        }
        finally {
            if (Test-Path -LiteralPath $temporaryManifest -PathType Leaf) {
                Remove-Item -LiteralPath $temporaryManifest -Force
            }
        }

        $validated = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
        if (
            $validated.schema -ne "ksa64.phase12c.unreal-automation-evidence.v1" -or
            $validated.source.commit -ne $SourceCommit -or
            $validated.result.succeeded -ne [int]$result.succeeded -or
            $validated.result.failed -ne 0 -or
            $validated.result.not_run -ne 0 -or
            $validated.result.in_process -ne 0
        ) {
            throw "staged Unreal automation evidence manifest is invalid"
        }
        foreach ($file in @($validated.files)) {
            if (
                [string]::IsNullOrWhiteSpace($file.path) -or
                $file.path.StartsWith("/") -or
                $file.path.Contains("..") -or
                $file.path.Contains("\")
            ) {
                throw "staged Unreal automation evidence manifest contains an unsafe path"
            }
            $candidate = Join-Path $stagingDirectory ($file.path.Replace("/", [IO.Path]::DirectorySeparatorChar))
            if (-not (Test-Path -LiteralPath $candidate -PathType Leaf) -or (Get-Sha256 $candidate) -ne $file.sha256) {
                throw "staged Unreal automation evidence manifest does not match its payload"
            }
        }
        if (Test-Path -LiteralPath $stableDirectory) {
            throw "completion automation evidence destination appeared during staging: $stableDirectory"
        }
        [IO.Directory]::Move($stagingDirectory, $stableDirectory)
        $published = $true
    }
    finally {
        if (-not $published -and (Test-Path -LiteralPath $stagingDirectory)) {
            Remove-Item -LiteralPath $stagingDirectory -Recurse -Force
        }
    }
    $publishedManifest = Join-Path $stableDirectory "phase12c-unreal-automation-evidence.json"
    Write-Host "Preserved Unreal automation completion evidence at $stableDirectory"
    return $publishedManifest
}

function Assert-BrowserEvidence([string]$Path) {
    if ([string]::IsNullOrWhiteSpace($Path)) {
        throw "-RunBrowserEvidence requires -BrowserEvidenceManifest"
    }
    $resolved = (Resolve-Path -LiteralPath $Path).Path
    $head = (& git rev-parse HEAD).Trim().ToLowerInvariant()
    Check
    $record = Get-Content -LiteralPath $resolved -Raw | ConvertFrom-Json
    if (
        $record.source.commit -ne $head -or
        $record.schema -ne "ksa64.phase12c.browser-evidence.v1" -or
        -not $record.pass -or
        $record.source.dirty -or
        $record.source.commit -notmatch '^[0-9a-f]{40}$' -or
        $record.source.tree_sha256 -notmatch '^[0-9a-f]{64}$' -or
        $record.production_dist.schema -ne "ksa64.phase12c.web-distribution-identity.v1" -or
        $record.production_dist.measurement -ne "production web/dist payload excluding its identity record" -or
        @($record.production_dist.excluded).Count -ne 1 -or
        $record.production_dist.excluded[0] -ne "phase12c-dist-identity.json" -or
        [int64]$record.production_dist.bytes -le 0 -or
        [int]$record.production_dist.file_count -le 1 -or
        $record.production_dist.tree_sha256 -notmatch '^[0-9a-f]{64}$' -or
        $record.renderer_origin.change_count -ne 1 -or
        [int]$record.renderer_origin.rendered_sample_count -lt 6 -or
        [double]$record.renderer_origin.max_reconstructed_delta_km -lt 0.0 -or
        [double]$record.renderer_origin.max_reconstructed_delta_km -gt 0.001 -or
        -not $record.renderer_origin.rendered_continuity -or
        -not $record.renderer_origin.semantic_continuity -or
        @($record.semantic_milestones).Count -ne 9 -or
        $record.backends.webgpu.status -ne "rendered" -or
        $record.backends.webgl2.status -ne "rendered" -or
        $record.backends.two_d.status -ne "rendered"
    ) {
        throw "rendered-browser producer manifest is incomplete or failed"
    }
}

function Assert-RuntimeEvidence([string]$Path, [string]$ExpectedSourceCommit) {
    $resolved = (Resolve-Path -LiteralPath $Path).Path
    $record = Get-Content -LiteralPath $resolved -Raw | ConvertFrom-Json
    $expectedReleases = @(29, 1920, 3579, 8124, 12669, 15255, 15257, 20929, 22014)
    $actualReleases = @($record.nominal.milestones | ForEach-Object { [int]$_.release_epoch })
    if (
        $record.schema -ne "ksa64.phase12c.cross-renderer-evidence.v2" -or
        -not $record.pass -or
        $record.producer.kind -ne "strict-source-bound-parity-comparator" -or
        $record.producer.source_commit -ne $ExpectedSourceCommit -or
        $record.producer.script.sha256 -notmatch '^[0-9a-f]{64}$' -or
        $record.inputs.native.sha256 -notmatch '^[0-9a-f]{64}$' -or
        $record.inputs.unreal.sha256 -notmatch '^[0-9a-f]{64}$' -or
        $record.inputs.unreal.package.sha256 -notmatch '^[0-9a-f]{64}$' -or
        $record.inputs.unreal.executable.sha256 -notmatch '^[0-9a-f]{64}$' -or
        [int64]$record.inputs.unreal.packaged_directory.bytes -le [int64]$record.inputs.unreal.executable.bytes -or
        [int]$record.inputs.unreal.packaged_directory.file_count -le 1 -or
        $record.inputs.unreal.packaged_directory.tree_sha256 -notmatch '^[0-9a-f]{64}$' -or
        $record.inputs.unreal.packaged_directory.inventory.sha256 -notmatch '^[0-9a-f]{64}$' -or
        $record.inputs.browser.sha256 -notmatch '^[0-9a-f]{64}$' -or
        [int64]$record.inputs.browser.production_dist.bytes -le 0 -or
        [int64]$record.storage.nominal_replay_display.nominal_replay_bytes -le 0 -or
        [int64]$record.storage.nominal_replay_display.exact_active_window_path.serialized_bytes -le 0 -or
        -not $record.renderer_origins.semantic_continuity -or
        [int]$record.renderer_origins.unreal.rendered_sample_count -lt 8 -or
        -not $record.renderer_origins.unreal.rendered_continuity -or
        [int]$record.renderer_origins.browser.rendered_sample_count -lt 6 -or
        -not $record.renderer_origins.browser.rendered_continuity -or
        @($actualReleases).Count -ne 9 -or
        @(Compare-Object $expectedReleases $actualReleases -SyncWindow 0).Count -ne 0 -or
        $record.catalog_sha256 -ne $catalogSha256 -or
        $record.nominal.releases -ne 22015 -or
        $record.nominal.first_release -ne 0 -or
        $record.nominal.last_release -ne 22014 -or
        $record.nominal.transition_count -ne 4 -or
        $record.nominal.terminal_disposition -ne 1 -or
        $record.nominal.source_availability.sim_director -ne 11 -or
        $record.nominal.source_availability.guided_operator -ne 3 -or
        $record.operational_milestones.status -ne "compared" -or
        $record.operational_milestones.count -ne 6 -or
        $record.performance.unreal.resolution -ne "1920x1080" -or
        [double]$record.performance.unreal.frames_per_second -lt 60.0 -or
        [int64]$record.performance.unreal.display_publication_p99_ns -ge 1000000 -or
        [int64]$record.performance.bridge.availability_p99_ns -ge 1000000 -or
        [int64]$record.performance.bridge.range_p99_ns -ge 1000000 -or
        [double]$record.performance.babylon.webgpu_frames_per_second -lt 30.0 -or
        [double]$record.performance.babylon.webgl2_frames_per_second -lt 30.0 -or
        -not $record.performance.babylon.context_loss_fallback
    ) {
        throw "strict source-bound renderer parity/runtime evidence does not satisfy Phase 12C"
    }
}

function Invoke-CrossRendererEvidence([string]$OutputPath) {
    if ([string]::IsNullOrWhiteSpace($NativeHarnessEvidenceManifest)) {
        throw "completion requires -NativeHarnessEvidenceManifest (the actual C++ GlobalDisplay harness JSON)"
    }
    if ([string]::IsNullOrWhiteSpace($UnrealEvidenceManifest)) {
        throw "completion requires -UnrealEvidenceManifest (the actual packaged Unreal evidence JSON)"
    }
    if ([string]::IsNullOrWhiteSpace($BrowserEvidenceManifest)) {
        throw "completion requires -BrowserEvidenceManifest (the actual rendered-browser evidence JSON)"
    }
    $head = (& git rev-parse HEAD).Trim().ToLowerInvariant()
    Check
    if ($head -notmatch '^[0-9a-f]{40}$') { throw "could not resolve a full source commit" }
    & git diff --quiet
    Check
    & git diff --cached --quiet
    Check
    $comparatorArguments = @(
        "phase12/compare-phase12c-renderers.mjs",
        "--native", [IO.Path]::GetFullPath($NativeHarnessEvidenceManifest),
        "--unreal", [IO.Path]::GetFullPath($UnrealEvidenceManifest),
        "--browser", [IO.Path]::GetFullPath($BrowserEvidenceManifest),
        "--output", [IO.Path]::GetFullPath($OutputPath),
        "--source-commit", $head
    )
    & node @comparatorArguments
    Check
    Assert-RuntimeEvidence $OutputPath $head
    return $head
}

if ($RunPackage -and [string]::IsNullOrWhiteSpace($PackageArchive)) {
    $PackageArchive = Join-Path $projectRoot ("target\phase12c-package-" + [Guid]::NewGuid().ToString("N"))
}
if ([string]::IsNullOrWhiteSpace($NativeHarnessEvidenceManifest)) {
    $NativeHarnessEvidenceManifest = Join-Path $projectRoot "viewer-bridge\harness\bin\phase12c-global-display-evidence.json"
}
if (($RunUnrealAutomation -or $RunPackage) -and -not $RunUnrealBuild) {
    Write-Host "Unreal automation/package will use the currently built target because -RunUnrealBuild was not supplied."
}

New-Item -ItemType Directory -Path $targetRoot -Force | Out-Null
New-Item -ItemType Directory -Path $auditRoot | Out-Null
if ($RunUnrealAutomation -or $RunPackage) {
    Save-UnrealGeneratedConfigSnapshot
}
$auditSucceeded = $false
Push-Location $projectRoot
try {
    Gate "frozen Phase 12B.5 entry evidence" {
        Assert-FrozenEntryEvidence
    }

    if (-not $SkipWorkspace) {
        Gate "workspace formatting, Clippy, and native tests" {
            cargo fmt --all -- --check
            Check
            cargo clippy --workspace --all-targets --features fixtures --locked -- -D warnings
            Check
            cargo test --workspace --features fixtures --locked
        }
    }

    Gate "GlobalDisplay protocol, bridge, session, broker, and WASM contracts" {
        cargo test -p ksa64-presentation --locked
        Check
        cargo test -p ksa64-session --lib --locked global_display
        Check
        cargo test -p ksa64-session-broker --all-features --locked
        Check
        cargo test -p ksa64-session-wasm --locked
        Check
        cargo test -p ksa64-viewer-bridge --lib --profile viewer --features panic-probe --locked global_display
    }

    if (-not $SkipExactRuns) {
        Gate "frozen and current Phase 10 nominal lineage audit" {
            Invoke-ExactCargoTest @(
                "test", "-p", "ksa64-session", "--lib", "--locked",
                "phase10_nominal_compat::tests::current_reexecution_matches_reviewed_lineage",
                "--", "--ignored", "--exact")
        }
        Gate "exact 22,015-release nominal global-display replay" {
            Invoke-ExactCargoTest @(
                "test", "-p", "ksa64-session", "--lib", "--locked",
                "global_display::exact_replay_tests::exact_nominal_replay_reproduces_frozen_release_boundaries",
                "--", "--ignored", "--exact")
        }
        Gate "native bridge exact nominal path parity" {
            Invoke-ExactCargoTest @(
                "test", "-p", "ksa64-viewer-bridge", "--lib", "--locked",
                "global_display::tests::nominal_direct_and_bridge_path_products_match",
                "--", "--ignored", "--exact")
        }
        Gate "exact 21,591-release GNSS-loss session" {
            Invoke-ExactCargoTest @(
                "test", "-p", "ksa64-session", "--lib", "--locked",
                "phase12b_live::tests::scripted_full_mission_seals_exact_evidence_and_succeeds",
                "--", "--ignored", "--exact")
        }
        Gate "native bridge terminal guided path parity" {
            Invoke-ExactCargoTest @(
                "test", "-p", "ksa64-viewer-bridge", "--lib", "--locked",
                "global_display::tests::completed_guided_direct_and_bridge_path_products_match",
                "--", "--ignored", "--exact")
        }
        Gate "strict GNSS-loss replay and normalized display stream" {
            Invoke-ExactCargoTest @(
                "test", "-p", "ksa64-session", "--lib", "--locked",
                "presentation_replay::tests::accepted_ksb11_replays_as_sequential_truth_filtered_kps1",
                "--", "--ignored", "--exact")
        }
    }

    if (-not $SkipHarness) {
        Gate "portable C/C++ ABI and contained-panic harnesses" {
            & viewer-bridge/harness/build-all.ps1 `
                -Phase12bExpectedSha256 $gnssKsb11Sha256 -PanicProbe
        }
    }

    if (-not $SkipWeb) {
        Gate "Babylon/React protocol, semantic, renderer, and production-build tests" {
            if (-not (Test-Path -LiteralPath "web/node_modules" -PathType Container)) {
                throw "web/node_modules is missing; run npm ci explicitly before this offline audit"
            }
            Push-Location web
            try {
                npm test
                Check
                npm run build
            }
            finally {
                Pop-Location
            }
        }
    }

    if ($RunUnrealBuild -or $RunPackage) {
        Gate "explicit clean portable Win64 bridge staging" {
            & phase12/stage-bridge-portable.ps1 -Platform Win64 -RepositoryRoot $projectRoot
            Check
            & phase12/stage-bridge-portable.ps1 -Platform Win64 -RepositoryRoot $projectRoot -VerifyOnly
        }
    }

    if ($RunUnrealBuild) {
        Gate "explicit Unreal Editor-target build" {
            [Environment]::SetEnvironmentVariable(
                "UE-LocalDataCachePath",
                [IO.Path]::GetFullPath($DerivedDataCache),
                "Process"
            )
            Invoke-UnrealBuild
        }
    }

    if ($RunUnrealAutomation) {
        Gate "explicit KSA64.Phase12C Unreal automation" {
            [Environment]::SetEnvironmentVariable(
                "UE-LocalDataCachePath",
                [IO.Path]::GetFullPath($DerivedDataCache),
                "Process"
            )
            try {
                Invoke-UnrealAutomation
            }
            finally {
                Restore-UnrealGeneratedConfig
            }
        }
    }

    if ($RunPackage) {
        Gate "explicit Unreal package and packaged bridge smoke" {
            Assert-NoUnrealProcess
            try {
                & phase12/package-phase12c-global-viewer.ps1 -RepositoryRoot $projectRoot `
                    -UnrealRoot $UnrealRoot `
                    -DerivedDataCache $DerivedDataCache `
                    -ArchiveDirectory $PackageArchive
                Check
                $packagedEvidence = Join-Path $PackageArchive "Windows\Ksa64MissionFoundry\Saved\KSA64\GlobalViewerEvidence\phase12c-global-viewer-evidence.json"
                if (-not (Test-Path -LiteralPath $packagedEvidence -PathType Leaf)) {
                    throw "specialized Phase 12C package did not produce its bound Unreal evidence manifest"
                }
                if ([string]::IsNullOrWhiteSpace($script:UnrealEvidenceManifest)) {
                    $script:UnrealEvidenceManifest = $packagedEvidence
                }
                elseif ([IO.Path]::GetFullPath($script:UnrealEvidenceManifest) -ne [IO.Path]::GetFullPath($packagedEvidence)) {
                    throw "-UnrealEvidenceManifest must name the evidence produced under -PackageArchive when -RunPackage is selected"
                }
            }
            finally {
                Restore-UnrealGeneratedConfig
                Assert-NoUnrealProcess
            }
        }
    }

    if ($RunBrowserEvidence) {
        Gate "explicit rendered-browser WebGPU/WebGL2/2-D evidence" {
            Assert-BrowserEvidence $BrowserEvidenceManifest
        }
    }

    $allCompletionGates = (
        -not $SkipWorkspace -and
        -not $SkipExactRuns -and
        -not $SkipHarness -and
        -not $SkipWeb -and
        $RunUnrealBuild -and
        $RunUnrealAutomation -and
        $RunPackage -and
        $RunBrowserEvidence -and
        -not [string]::IsNullOrWhiteSpace($NativeHarnessEvidenceManifest) -and
        -not [string]::IsNullOrWhiteSpace($UnrealEvidenceManifest) -and
        -not [string]::IsNullOrWhiteSpace($RuntimeEvidenceManifest)
    )

    Write-Host ""
    if ($allCompletionGates) {
        Gate "strict source-bound cross-renderer parity and runtime evidence" {
            $recomputedEvidence = Join-Path $auditRoot "phase12c-cross-renderer-recomputed.json"
            Invoke-CrossRendererEvidence $recomputedEvidence | Out-Null
            $runtimeEvidencePath = [IO.Path]::GetFullPath($RuntimeEvidenceManifest)
            if (Test-Path -LiteralPath $runtimeEvidencePath -PathType Container) {
                throw "-RuntimeEvidenceManifest names a directory"
            }
            if (-not (Test-Path -LiteralPath $runtimeEvidencePath -PathType Leaf)) {
                $runtimeEvidenceDirectory = [IO.Path]::GetDirectoryName($runtimeEvidencePath)
                [IO.Directory]::CreateDirectory($runtimeEvidenceDirectory) | Out-Null
                $runtimeEvidenceTemporary = Join-Path $runtimeEvidenceDirectory (([IO.Path]::GetFileName($runtimeEvidencePath)) + "." + [Guid]::NewGuid().ToString("N") + ".tmp")
                try {
                    [IO.File]::Copy($recomputedEvidence, $runtimeEvidenceTemporary, $false)
                    [IO.File]::Move($runtimeEvidenceTemporary, $runtimeEvidencePath)
                }
                finally {
                    if (Test-Path -LiteralPath $runtimeEvidenceTemporary -PathType Leaf) {
                        Remove-Item -LiteralPath $runtimeEvidenceTemporary -Force
                    }
                }
                Write-Host "Recorded strict runtime/parity evidence at $runtimeEvidencePath"
            }
            $recorded = (Resolve-Path -LiteralPath $runtimeEvidencePath).Path
            if ((Get-Sha256 $recorded) -ne (Get-Sha256 $recomputedEvidence)) {
                throw "recorded runtime/parity manifest is not the deterministic output of the current raw producer artifacts"
            }
        }
        Gate "preserve Unreal automation completion evidence" {
            $sourceCommit = (& git rev-parse HEAD).Trim().ToLowerInvariant()
            Check
            Preserve-UnrealAutomationEvidence $sourceCommit | Out-Null
        }
        Write-Host "PHASE 12C COMPLETION AUDIT: PASS"
    }
    else {
        Write-Host "PHASE 12C PORTABLE/CONTRACT AUDIT: PASS"
        Write-Host "Phase completion is NOT claimed by this invocation."
        Write-Host "Completion additionally requires all non-skip defaults plus explicit Unreal build, automation, package, rendered-browser, and runtime/parity evidence."
    }
    $auditSucceeded = $true
}
finally {
    Restore-UnrealGeneratedConfig
    Pop-Location
    $resolved = [IO.Path]::GetFullPath($auditRoot)
    if (-not $resolved.StartsWith($projectRoot + [IO.Path]::DirectorySeparatorChar)) {
        throw "unsafe Phase 12C audit cleanup target"
    }
    if ($auditSucceeded) {
        if (Test-Path -LiteralPath $resolved) {
            Remove-Item -LiteralPath $resolved -Recurse -Force
        }
    }
    else {
        Write-Warning "Phase 12C audit failed; diagnostic evidence retained at $resolved"
    }
}
