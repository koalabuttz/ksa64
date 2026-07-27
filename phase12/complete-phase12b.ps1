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
    [switch]$RunPresentationEvidence,
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

function Get-PngInspection([string]$Path) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 33) { throw "PNG is shorter than its minimum structure: $Path" }
    $signature = [byte[]](0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a)
    for ($index = 0; $index -lt $signature.Length; $index++) {
        if ($bytes[$index] -ne $signature[$index]) { throw "PNG signature mismatch: $Path" }
    }

    $position = 8
    $chunkIndex = 0
    $seenIhdr = $false
    $seenIend = $false
    while ($position -lt $bytes.Length) {
        if ($bytes.Length - $position -lt 12) { throw "PNG has a truncated chunk header: $Path" }
        $chunkLength =
            ([uint32]$bytes[$position] -shl 24) -bor
            ([uint32]$bytes[$position + 1] -shl 16) -bor
            ([uint32]$bytes[$position + 2] -shl 8) -bor
            [uint32]$bytes[$position + 3]
        $chunkEnd = [int64]$position + 12 + [int64]$chunkLength
        if ($chunkEnd -gt $bytes.Length) { throw "PNG has a truncated chunk payload: $Path" }
        $chunkType = [Text.Encoding]::ASCII.GetString($bytes, $position + 4, 4)
        if ($chunkIndex -eq 0 -and ($chunkType -ne "IHDR" -or $chunkLength -ne 13)) {
            throw "PNG first chunk is not a canonical IHDR: $Path"
        }
        if ($chunkType -eq "IHDR") { $seenIhdr = $true }
        if ($chunkType -eq "IEND") {
            if ($chunkLength -ne 0 -or $chunkEnd -ne $bytes.Length) {
                throw "PNG IEND is malformed or not terminal: $Path"
            }
            $seenIend = $true
        }
        $position = [int]$chunkEnd
        $chunkIndex++
    }
    if (-not $seenIhdr -or -not $seenIend) { throw "PNG is missing IHDR or terminal IEND: $Path" }

    Add-Type -AssemblyName System.Drawing.Common -ErrorAction Stop
    $stream = $null
    $image = $null
    $bitmap = $null
    $reencoded = $null
    try {
        $stream = [System.IO.File]::OpenRead($Path)
        $image = [System.Drawing.Image]::FromStream($stream, $true, $true)
        $bitmap = [System.Drawing.Bitmap]::new($image)
        $reencoded = [System.IO.MemoryStream]::new()
        $bitmap.Save($reencoded, [System.Drawing.Imaging.ImageFormat]::Png)
        if ($reencoded.Length -le 33) { throw "Decoded PNG could not be re-encoded: $Path" }

        $stepX = [Math]::Max(1, [Math]::Floor($bitmap.Width / 64))
        $stepY = [Math]::Max(1, [Math]::Floor($bitmap.Height / 36))
        $minimumLuminance = 255
        $maximumLuminance = 0
        $nonDarkSamples = 0
        $sampledPixels = 0
        $colorBuckets = [Collections.Generic.HashSet[int]]::new()
        for ($y = 0; $y -lt $bitmap.Height; $y += $stepY) {
            for ($x = 0; $x -lt $bitmap.Width; $x += $stepX) {
                $pixel = $bitmap.GetPixel($x, $y)
                $luminance = (54 * [int]$pixel.R + 183 * [int]$pixel.G + 19 * [int]$pixel.B) -shr 8
                $minimumLuminance = [Math]::Min($minimumLuminance, $luminance)
                $maximumLuminance = [Math]::Max($maximumLuminance, $luminance)
                if ($luminance -gt 16) { $nonDarkSamples++ }
                $bucket = (([int]$pixel.R -shr 5) -shl 6) -bor (([int]$pixel.G -shr 5) -shl 3) -bor ([int]$pixel.B -shr 5)
                [void]$colorBuckets.Add($bucket)
                $sampledPixels++
            }
        }
        $luminanceRange = $maximumLuminance - $minimumLuminance
        if (
            $sampledPixels -le 0 -or
            $luminanceRange -lt 24 -or
            $colorBuckets.Count -lt 8 -or
            $nonDarkSamples -lt [Math]::Max(1, [Math]::Floor($sampledPixels / 100))
        ) { throw "Decoded PNG does not contain a visibly nonblank dashboard: $Path" }
        return [pscustomobject]@{
            Width = $bitmap.Width
            Height = $bitmap.Height
            ChunkCount = $chunkIndex
            ReencodedBytes = [int64]$reencoded.Length
            SampledPixels = $sampledPixels
            DistinctColorBuckets = $colorBuckets.Count
            LuminanceRange = $luminanceRange
            NonDarkSamples = $nonDarkSamples
        }
    }
    finally {
        if ($null -ne $reencoded) { $reencoded.Dispose() }
        if ($null -ne $bitmap) { $bitmap.Dispose() }
        if ($null -ne $image) { $image.Dispose() }
        if ($null -ne $stream) { $stream.Dispose() }
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

function Assert-NoMissionFoundryRuntime {
    $processes = @(Get-Process -Name Ksa64MissionFoundry -ErrorAction SilentlyContinue)
    if ($processes.Count -ne 0) {
        throw "Close packaged Mission Foundry process(es) before this gate: $($processes.Id -join ', ')"
    }
}

function Assert-Phase12bRecord {
    $record = Get-Content -LiteralPath (Join-Path $projectRoot "phase12/phase12b-completion-audit.json") -Raw | ConvertFrom-Json
    $expectedGates = @(
        "rust_full_reference",
        "rust_no_action",
        "rust_role_and_action_boundaries",
        "phase12a_frozen_audit",
        "cpp_phase12a_harness",
        "cpp_phase12b_full_mission_harness",
        "unreal_editor_build",
        "unreal_operations_automation",
        "packaged_full_mission",
        "presentation_30_60_144_hz",
        "bridge_frame_latency",
        "screenshot_semantic_accessibility",
        "async_shutdown_and_finalization"
    )
    foreach ($gate in $expectedGates) {
        if ($record.gates.$gate -ne "pass") {
            throw "Phase 12B completion gate is not accepted: $gate"
        }
    }
    if (
        $record.schema -ne "ksa64.phase12b.completion-audit.v1" -or
        $record.status -ne "complete" -or
        $record.accepted_source_commit -ne "423c116cf58632f344d4a48774a97a4487c34113" -or
        $record.accepted_source.bridge_abi_major -ne 1 -or
        $record.accepted_source.bridge_build_identity -ne "0x120B0001" -or
        $record.accepted_source.bridge_dll_sha256 -ne "da6657a46759a028cb8901ce813af093d4d8901c76cb383f0d74601d64f26565" -or
        $record.accepted_source.catalog_count -ne 13 -or
        $record.accepted_source.catalog_sha256 -ne "b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13" -or
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
        $record.unreal_metrics.operations_automation.succeeded -ne 17 -or
        $record.unreal_metrics.operations_automation.failed -ne 0 -or
        $record.unreal_metrics.operations_automation.not_run -ne 0 -or
        $record.unreal_metrics.operations_automation.in_process -ne 0 -or
        $record.unreal_metrics.base_package.editor_plugin_binaries_packaged -ne 0 -or
        $record.unreal_metrics.packaged_full_mission.exit_code -ne 0 -or
        $record.unreal_metrics.packaged_full_mission.release_epoch -ne 21591 -or
        $record.unreal_metrics.packaged_full_mission.ksb11_bytes -ne 2911464 -or
        $record.unreal_metrics.packaged_full_mission.ksb11_sha256 -ne $acceptedKsb11Sha256 -or
        $record.unreal_metrics.presentation.rhi -ne "D3D12" -or
        $record.unreal_metrics.presentation.width -ne 1920 -or
        $record.unreal_metrics.presentation.height -ne 1080 -or
        $record.unreal_metrics.presentation.capture_release_epoch -ne 6080 -or
        $record.unreal_metrics.presentation.screenshot_sha256 -ne "55ea4b4c94a7a50fac29fd4e981197ee53a3e6bc01eb3614959d754f4a687fd0" -or
        $record.unreal_metrics.presentation.semantic_sha256 -ne "557a4d9a83917f539464818f24d44cd142e8b987dc2eebddea0bc6acda4d6bb3" -or
        $record.unreal_metrics.presentation.manifest_sha256 -ne "6c48d17b7ecca8c0f82c4bcd316e88dc2ba9f09aedfde859a1454d78d9921dd8" -or
        $record.unreal_metrics.presentation.p99_ns -ne 258900 -or
        $record.unreal_metrics.presentation.maximum_ns -ne 460000 -or
        $record.unreal_metrics.presentation.p99_limit_ns -ne 1000000 -or
        $record.unreal_metrics.presentation.maximum_limit_ns -ne 2000000 -or
        $record.unreal_metrics.presentation.cadence_hz -ne 60 -or
        $record.unreal_metrics.presentation.measured_frames -ne 600 -or
        $record.unreal_metrics.presentation.releases_advanced -ne 320 -or
        $record.unreal_metrics.presentation.pending_commands -ne 0 -or
        $record.unreal_metrics.presentation.transport_overflow -or
        -not $record.unreal_metrics.presentation.observation_complete -or
        -not $record.unreal_metrics.presentation.reduced_motion -or
        -not $record.unreal_metrics.presentation.high_contrast -or
        $record.unreal_metrics.presentation.text_scale -ne "1.25" -or
        $record.unreal_metrics.presentation.sound_cues_enabled -or
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
    if (
        [int]$result.succeeded -ne 17 -or
        [int]$result.failed -ne 0 -or
        [int]$result.notRun -ne 0 -or
        [int]$result.inProcess -ne 0
    ) { throw "Unreal Operations automation did not report the accepted 17/17 result." }

    if (-not [string]::IsNullOrWhiteSpace($PackageArchive)) {
        $archive = [IO.Path]::GetFullPath($PackageArchive)
        if (-not (Test-Path -LiteralPath $archive -PathType Container)) {
            throw "Cannot preserve Unreal automation evidence because the package archive does not exist: $archive"
        }
        $stableDirectory = Join-Path $archive "phase12b-unreal-automation"
        if (Test-Path -LiteralPath $stableDirectory) {
            throw "Unreal automation evidence requires a fresh destination: $stableDirectory"
        }
        New-Item -ItemType Directory -Path $stableDirectory | Out-Null
        $stableIndex = Join-Path $stableDirectory "index.json"
        $stableHtml = Join-Path $stableDirectory "index.html"
        $stableLog = Join-Path $stableDirectory "unreal-operations-automation.log"
        Copy-Item -LiteralPath $index -Destination $stableIndex
        Copy-Item -LiteralPath (Join-Path $report "index.html") -Destination $stableHtml
        Copy-Item -LiteralPath $log -Destination $stableLog
        $record = [ordered]@{
            schema = "ksa64.phase12b.unreal-automation-validation.v1"
            filter = "KSA64.Operations"
            succeeded = [int]$result.succeeded
            failed = [int]$result.failed
            not_run = [int]$result.notRun
            in_process = [int]$result.inProcess
            duration_seconds = [double]$result.totalDuration
            exit_code = $process.ExitCode
            report = [ordered]@{
                path = $stableIndex.Substring($archive.Length + 1).Replace('\', '/')
                sha256 = Get-Sha256 $stableIndex
            }
            html = [ordered]@{
                path = $stableHtml.Substring($archive.Length + 1).Replace('\', '/')
                sha256 = Get-Sha256 $stableHtml
            }
            log = [ordered]@{
                path = $stableLog.Substring($archive.Length + 1).Replace('\', '/')
                sha256 = Get-Sha256 $stableLog
            }
        }
        $record | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $archive "phase12b-unreal-automation-validation.json") -Encoding utf8NoBOM
    }
    Assert-NoUnrealEditor
}

function Invoke-PackagedOperationsAcceptance([string]$ArchiveDirectory) {
    Assert-NoUnrealEditor
    Assert-NoMissionFoundryRuntime
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
    Assert-NoMissionFoundryRuntime
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
    Assert-NoMissionFoundryRuntime
    Assert-NoUnrealEditor
}

function Invoke-PackagedPresentationEvidence([string]$ArchiveDirectory) {
    Assert-NoUnrealEditor
    Assert-NoMissionFoundryRuntime
    $archive = [IO.Path]::GetFullPath($ArchiveDirectory)
    $gameRoot = Join-Path $archive "Windows\Ksa64MissionFoundry"
    $gameExe = Join-Path $gameRoot "Binaries\Win64\Ksa64MissionFoundry.exe"
    if (-not (Test-Path -LiteralPath $gameExe -PathType Leaf)) {
        throw "Packaged game executable is missing: $gameExe"
    }

    $presentationDirectory = Join-Path $gameRoot "Saved\KSA64\PresentationEvidence"
    $screenshot = Join-Path $presentationDirectory "phase12b-gnss-loss-operations-1920x1080.png"
    $semantic = Join-Path $presentationDirectory "phase12b-gnss-loss-operations-semantic.json"
    $manifestPath = Join-Path $presentationDirectory "phase12b-presentation-evidence.json"
    foreach ($path in @($screenshot, $semantic, $manifestPath)) {
        if (Test-Path -LiteralPath $path) {
            throw "Packaged presentation evidence requires fresh fixed outputs; found $path"
        }
    }

    $log = Join-Path $archive "packaged-phase12b-presentation-evidence.log"
    if (Test-Path -LiteralPath $log) {
        throw "Packaged presentation evidence requires a fresh log: $log"
    }
    $arguments = @(
        "-windowed",
        "-ResX=1920",
        "-ResY=1080",
        "-ForceRes",
        "-RenderOffscreen",
        "-Benchmark",
        "-UseFixedTimeStep",
        "-FPS=60",
        "-nosound",
        "-unattended",
        "-nosplash",
        "-DisablePlugins=ModelContextProtocol,ToolsetRegistry,AllToolsets,PythonScriptPlugin",
        "-Ksa64Phase12bPresentationEvidence",
        "-abslog=$log"
    )
    $process = Start-Process -FilePath $gameExe -ArgumentList $arguments -WorkingDirectory (Split-Path -Parent $gameExe) -WindowStyle Hidden -PassThru
    # This wait is intentionally unbounded. Duration alone is never evidence of
    # failure and must not terminate an otherwise progressing simulation.
    $process.WaitForExit()
    Assert-NoMissionFoundryRuntime
    if ($process.ExitCode -ne 0) {
        throw "Packaged Phase 12B presentation evidence exited with code $($process.ExitCode). See $log."
    }
    foreach ($path in @($log, $screenshot, $semantic, $manifestPath)) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Packaged presentation evidence output is missing: $path"
        }
    }

    $logText = Get-Content -LiteralPath $log -Raw
    $markerPattern = "KSA64_PHASE12B_PRESENTATION_EVIDENCE_PASS release=6080 width=1920 height=1080 frames=600 p99_ns=(?<p99>[0-9]+) max_ns=(?<max>[0-9]+)"
    $markers = [regex]::Matches($logText, $markerPattern)
    if ($markers.Count -ne 1 -or $logText.Contains("KSA64_PHASE12B_PRESENTATION_EVIDENCE_FAIL")) {
        throw "Packaged presentation evidence did not emit exactly one exact PASS marker. See $log."
    }

    $pngInspection = Get-PngInspection $screenshot
    if ($pngInspection.Width -ne 1920 -or $pngInspection.Height -ne 1080) {
        throw "Presentation PNG is $($pngInspection.Width)x$($pngInspection.Height), expected 1920x1080."
    }
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if (
        $manifest.schema -ne "ksa64.phase12b.presentation-evidence.v1" -or
        -not $manifest.pass -or
        $manifest.scenario -ne "ksa-g10r.operations/gnss-loss" -or
        $manifest.role -ne "guided-operator" -or
        -not $manifest.truth_filtered -or
        -not $manifest.async_shutdown_complete -or
        $manifest.screenshot_release_epoch -ne 6080 -or
        $manifest.screenshot.width -ne 1920 -or
        $manifest.screenshot.height -ne 1080 -or
        -not $manifest.screenshot.fully_decoded -or
        $manifest.screenshot.sampled_pixels -le 0 -or
        $manifest.screenshot.distinct_color_buckets -lt 8 -or
        $manifest.screenshot.luminance_range -lt 24 -or
        $manifest.screenshot.non_dark_samples -le 0 -or
        -not $manifest.screenshot.slate_inclusive -or
        -not $manifest.screenshot.real_rhi -or
        $manifest.screenshot.rhi_name -notmatch "D3D12" -or
        $manifest.trajectory.planned_reference_points -le 0 -or
        $manifest.trajectory.onboard_estimate_points -le 0 -or
        $manifest.trajectory.ground_estimate_points -le 0 -or
        $manifest.trajectory.observed_points -le 0 -or
        -not $manifest.trajectory.altitude_plot -or
        -not $manifest.trajectory.ground_track_plot -or
        $manifest.trajectory.display_mode -ne "exact" -or
        $manifest.performance.refresh_hz -ne 60 -or
        $manifest.performance.cadence -ne "simulated-fixed-step" -or
        -not $manifest.performance.fixed_timestep -or
        $manifest.performance.fixed_delta_seconds -ne "0.016666666666666667" -or
        $manifest.performance.warmup_frames -ne 120 -or
        $manifest.performance.measured_frames -ne 600 -or
        $manifest.performance.logical_seconds -ne 10 -or
        $manifest.performance.release_delta -ne 320 -or
        $manifest.performance.expected_release_delta -ne 320 -or
        $manifest.performance.end_release -ne $manifest.performance.start_release + 320 -or
        $manifest.performance.end_publication -le $manifest.performance.start_publication -or
        $manifest.performance.commands_pending -ne 0 -or
        $manifest.performance.transport_overflow -ne 0 -or
        -not $manifest.performance.observation_complete -or
        $manifest.performance.advance_outstanding -or
        $manifest.performance.percentile_method -ne "nearest-rank" -or
        -not $manifest.performance.pass -or
        [int64]$manifest.performance.p99_ns -ge 1000000 -or
        [int64]$manifest.performance.max_ns -ge 2000000 -or
        [int64]$manifest.performance.p99_ns -ne [int64]$markers[0].Groups["p99"].Value -or
        [int64]$manifest.performance.max_ns -ne [int64]$markers[0].Groups["max"].Value
    ) { throw "Phase 12B packaged presentation manifest failed its acceptance contract." }

    $semanticRecord = Get-Content -LiteralPath $semantic -Raw | ConvertFrom-Json
    if ($semanticRecord.schema -ne "ksa64.mission-foundry-semantic-state.v1") {
        throw "Presentation semantic evidence schema mismatch."
    }
    $semanticView = $semanticRecord.view | ConvertFrom-Json
    if (
        $semanticRecord.presentation_pace -ne "PAUSED" -or
        $semanticRecord.text_scale -ne 1.25 -or
        -not $semanticRecord.high_contrast -or
        -not $semanticRecord.reduced_motion -or
        $semanticRecord.sound_cues -or
        $semanticRecord.display_mode -ne "exact" -or
        $semanticRecord.planned_reference_point_count -le 0 -or
        $semanticRecord.onboard_prediction_point_count -le 0 -or
        $semanticRecord.ground_prediction_point_count -le 0 -or
        -not $semanticRecord.dashboard_installed -or
        $semanticRecord.capture_release_epoch -ne 6080 -or
        $semanticView.schema -ne "ksa64.operations-view.v1" -or
        $semanticView.release_epoch -ne 6080 -or
        -not $semanticView.session_open -or
        -not $semanticView.truth_filtered -or
        -not $semanticView.observation_complete -or
        $semanticView.transport_overflow -ne 0 -or
        $semanticView.action_state -ne 1 -or
        $semanticView.action_proposal_identity -eq 0 -or
        $semanticView.procedure_identity -eq 0 -or
        $semanticView.prediction_identity -eq 0
    ) { throw "Presentation semantic evidence is not the accepted paused GNSS-loss action epoch." }

    $record = [ordered]@{
        schema = "ksa64.phase12b.presentation-evidence-validation.v1"
        release_epoch = 6080
        screenshot = [ordered]@{
            path = $screenshot.Substring($archive.Length + 1).Replace('\', '/')
            bytes = [int64](Get-Item -LiteralPath $screenshot).Length
            sha256 = Get-Sha256 $screenshot
            width = $pngInspection.Width
            height = $pngInspection.Height
            chunk_count = $pngInspection.ChunkCount
            reencoded_bytes = $pngInspection.ReencodedBytes
            sampled_pixels = $pngInspection.SampledPixels
            distinct_color_buckets = $pngInspection.DistinctColorBuckets
            luminance_range = $pngInspection.LuminanceRange
            non_dark_samples = $pngInspection.NonDarkSamples
        }
        semantic = [ordered]@{
            path = $semantic.Substring($archive.Length + 1).Replace('\', '/')
            sha256 = Get-Sha256 $semantic
        }
        manifest = [ordered]@{
            path = $manifestPath.Substring($archive.Length + 1).Replace('\', '/')
            sha256 = Get-Sha256 $manifestPath
        }
        performance = [ordered]@{
            cadence = $manifest.performance.cadence
            frames = 600
            releases = [int]$manifest.performance.release_delta
            start_release = [int]$manifest.performance.start_release
            end_release = [int]$manifest.performance.end_release
            start_publication = [int64]$manifest.performance.start_publication
            end_publication = [int64]$manifest.performance.end_publication
            p99_ns = [int64]$manifest.performance.p99_ns
            max_ns = [int64]$manifest.performance.max_ns
            observation_complete = [bool]$manifest.performance.observation_complete
            transport_overflow = [int]$manifest.performance.transport_overflow
        }
        exit_code = $process.ExitCode
        log = $log.Substring($archive.Length + 1).Replace('\', '/')
        log_sha256 = Get-Sha256 $log
    }
    $record | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $archive "phase12b-presentation-evidence-validation.json") -Encoding utf8NoBOM
    Assert-NoMissionFoundryRuntime
    Assert-NoUnrealEditor
}

New-Item -ItemType Directory -Path $auditRoot | Out-Null
if ($RunPresentationEvidence -and -not $RunPackage) {
    throw "-RunPresentationEvidence requires -RunPackage so the real-RHI gate uses a freshly packaged build."
}
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
            $phase12aParameters = @{
                SkipLegacy = $true
                SkipMos = $true
                SkipHarness = $true
                UnrealRoot = $UnrealRoot
                DerivedDataCache = $DerivedDataCache
            }
            if ($RunUnrealBuild) { $phase12aParameters.RunUnrealBuild = $true }
            if ($RunUnrealAutomation) { $phase12aParameters.RunUnrealAutomation = $true }
            if ($RunPackage) { $phase12aParameters.RunPackage = $true }
            if (-not [string]::IsNullOrWhiteSpace($PackageArchive)) {
                $phase12aParameters.PackageArchive = $PackageArchive
            }
            & phase12/complete.ps1 @phase12aParameters
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

    if ($RunPresentationEvidence) {
        Gate "packaged Phase 12B real-RHI presentation evidence" {
            Invoke-PackagedPresentationEvidence $PackageArchive
        }
    }

    Write-Host ""
    if ($RunUnrealBuild -and $RunUnrealAutomation -and $RunPackage -and $RunPresentationEvidence) {
        Write-Host "PHASE 12B PRODUCT ACCEPTANCE GATES: PASS"
    } elseif ($RunUnrealBuild -and $RunUnrealAutomation -and $RunPackage) {
        Write-Host "PHASE 12B IMPLEMENTATION GATES: PASS"
        Write-Host "Canonical Phase 12B acceptance is recorded; this invocation omitted the explicit presentation recheck."
    } else {
        Write-Host "PHASE 12B CORE/BRIDGE AUDIT: PASS"
        Write-Host "Canonical Phase 12B acceptance is recorded; this invocation revalidated only core/bridge gates. Use explicit -RunUnrealBuild, -RunUnrealAutomation, -RunPackage, and -RunPresentationEvidence switches to rerun every live gate."
    }
}
finally {
    Pop-Location
    $resolved = [IO.Path]::GetFullPath($auditRoot)
    if (-not $resolved.StartsWith($projectRoot + [IO.Path]::DirectorySeparatorChar)) { throw "unsafe audit cleanup" }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
