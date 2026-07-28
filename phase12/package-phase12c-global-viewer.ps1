[CmdletBinding()]
param(
    [string]$RepositoryRoot = "",
    [string]$UnrealRoot = "D:\Games\UE_5.8",
    [string]$DerivedDataCache = "E:\Unreal\DDC",
    [string]$ArchiveDirectory = "",
    [switch]$SkipBuild,
    [switch]$UseExistingPackage
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Get-Sha256([string]$Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try { return ([System.BitConverter]::ToString($algorithm.ComputeHash($stream))).Replace("-", "").ToLowerInvariant() }
    finally { $algorithm.Dispose(); $stream.Dispose() }
}

function Get-PngDimensions([string]$Path) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 24 -or $bytes[0] -ne 0x89 -or $bytes[1] -ne 0x50 -or $bytes[2] -ne 0x4e -or $bytes[3] -ne 0x47 -or $bytes[12] -ne 0x49 -or $bytes[13] -ne 0x48 -or $bytes[14] -ne 0x44 -or $bytes[15] -ne 0x52) {
        throw "Screenshot is not a complete PNG with an IHDR chunk: $Path"
    }
    $width = ([uint32]$bytes[16] -shl 24) -bor ([uint32]$bytes[17] -shl 16) -bor ([uint32]$bytes[18] -shl 8) -bor [uint32]$bytes[19]
    $height = ([uint32]$bytes[20] -shl 24) -bor ([uint32]$bytes[21] -shl 16) -bor ([uint32]$bytes[22] -shl 8) -bor [uint32]$bytes[23]
    return [pscustomobject]@{ Width = [int]$width; Height = [int]$height }
}

function Assert-NoRuntime {
    $processes = @(Get-Process -Name UnrealEditor, UnrealEditor-Cmd, Ksa64MissionFoundry -ErrorAction SilentlyContinue)
    if ($processes.Count -ne 0) { throw "Close Unreal/Mission Foundry process(es) before the Phase 12C package gate: $($processes.Id -join ', ')" }
}

if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) { $RepositoryRoot = Split-Path -Parent $PSScriptRoot }
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$commit = (& git -C $root rev-parse --verify HEAD).Trim().ToLowerInvariant()
if ($LASTEXITCODE -ne 0 -or $commit -notmatch '^[0-9a-f]{40}$') { throw "Could not resolve the source commit." }
$status = @(& git -C $root status --porcelain=v1 --untracked-files=all)
if ($LASTEXITCODE -ne 0) { throw "Could not inspect source status." }
if ($status.Count -ne 0) { throw "Source-bound Phase 12C packaged evidence requires a clean committed checkout." }

if ([string]::IsNullOrWhiteSpace($ArchiveDirectory)) { $ArchiveDirectory = Join-Path $root ("target\phase12c-package-" + [Guid]::NewGuid().ToString("N")) }
$archive = [IO.Path]::GetFullPath($ArchiveDirectory)
Assert-NoRuntime
if (-not $UseExistingPackage) {
    $packageScript = Join-Path $PSScriptRoot "package.ps1"
    $parameters = @{ RepositoryRoot = $root; UnrealRoot = $UnrealRoot; DerivedDataCache = $DerivedDataCache; ArchiveDirectory = $archive }
    if ($SkipBuild) { $parameters.SkipBuild = $true }
    & $packageScript @parameters
    if ($LASTEXITCODE -ne 0) { throw "Base Unreal packaging failed with exit code $LASTEXITCODE." }
}
elseif (-not (Test-Path -LiteralPath $archive -PathType Container)) { throw "Existing package archive does not exist: $archive" }
Assert-NoRuntime

$gameRoot = Join-Path $archive "Windows\Ksa64MissionFoundry"
$gameExe = Join-Path $gameRoot "Binaries\Win64\Ksa64MissionFoundry.exe"
$bridgeDirectory = Join-Path $gameRoot "Plugins\Ksa64Bridge\Binaries\Win64"
$bridgeManifests = @(Get-ChildItem -LiteralPath $bridgeDirectory -Filter "*.manifest.json" -File -ErrorAction Stop)
if ($bridgeManifests.Count -ne 1) { throw "Expected one packaged bridge manifest; found $($bridgeManifests.Count)." }
$bridgeManifest = Get-Content -LiteralPath $bridgeManifests[0].FullName -Raw | ConvertFrom-Json
if ($bridgeManifest.schema -eq "ksa64.viewer-bridge-manifest.v1") { $bridgeFile = Join-Path $bridgeDirectory $bridgeManifest.dll_filename; $bridgeHash = $bridgeManifest.dll_sha256 }
elseif ($bridgeManifest.schema -eq "ksa64.viewer-bridge-artifact.v2") { $bridgeFile = Join-Path $bridgeDirectory $bridgeManifest.library_file; $bridgeHash = $bridgeManifest.sha256 }
else { throw "Unsupported packaged bridge manifest schema '$($bridgeManifest.schema)'." }
foreach ($required in @($gameExe, $bridgeFile)) { if (-not (Test-Path -LiteralPath $required -PathType Leaf)) { throw "Packaged runtime input is missing: $required" } }
if ($bridgeManifest.source_commit -ne $commit -or (Get-Sha256 $bridgeFile) -ne $bridgeHash) { throw "Packaged bridge is not bound to the clean source commit." }
$packageAuditPath = Join-Path $archive "phase12a-package-audit.json"
$packageAudit = Get-Content -LiteralPath $packageAuditPath -Raw | ConvertFrom-Json
if ($packageAudit.source_commit -ne $commit) { throw "Base package audit is not bound to the clean source commit." }
$gameSha256 = Get-Sha256 $gameExe
$gameBytes = [int64](Get-Item -LiteralPath $gameExe).Length
$gameRelativePath = $gameExe.Substring($archive.Length + 1).Replace('\', '/')
$packageAuditSha256 = Get-Sha256 $packageAuditPath

$evidenceDirectory = Join-Path $gameRoot "Saved\KSA64\GlobalViewerEvidence"
$manifestGameRelativePath = [IO.Path]::GetRelativePath($evidenceDirectory, $gameExe).Replace('\', '/')
$runtimeManifestPath = Join-Path $evidenceDirectory "phase12c-global-viewer-evidence.json"
$validationPath = Join-Path $archive "phase12c-global-viewer-evidence-validation.json"
$logPath = Join-Path $archive "packaged-phase12c-global-viewer-evidence.log"
foreach ($path in @($runtimeManifestPath, $validationPath, $logPath)) { if (Test-Path -LiteralPath $path) { throw "Phase 12C global-viewer evidence requires fresh fixed outputs; found $path" } }

$arguments = @("-windowed", "-ResX=1920", "-ResY=1080", "-ForceRes", "-RenderOffscreen", "-Benchmark", "-UseFixedTimeStep", "-FPS=60", "-nosound", "-unattended", "-nosplash", "-DisablePlugins=ModelContextProtocol,ToolsetRegistry,AllToolsets,PythonScriptPlugin", "-Ksa64Phase12cGlobalEvidence", "-Ksa64SourceCommit=$commit", "-Ksa64ExecutableRelativePath=$manifestGameRelativePath", "-Ksa64ExecutableBytes=$gameBytes", "-Ksa64ExecutableSha256=$gameSha256", "-Ksa64PackageAuditSha256=$packageAuditSha256", "-abslog=$logPath")
$process = Start-Process -FilePath $gameExe -ArgumentList $arguments -WorkingDirectory (Split-Path -Parent $gameExe) -WindowStyle Hidden -PassThru
# Intentionally unbounded: duration alone is not evidence that this run failed.
$process.WaitForExit()
Assert-NoRuntime
if ($process.ExitCode -ne 0) { throw "Packaged Phase 12C global-viewer evidence exited with code $($process.ExitCode). See $logPath." }
foreach ($required in @($runtimeManifestPath, $logPath)) { if (-not (Test-Path -LiteralPath $required -PathType Leaf)) { throw "Packaged Phase 12C evidence output is missing: $required" } }
$logText = Get-Content -LiteralPath $logPath -Raw
$markerPattern = "KSA64_PHASE12C_GLOBAL_EVIDENCE_PASS captures=9 guided=6 actions=4 release=21591 width=1920 height=1080 frames=600 fps=(?<fps>[0-9]+(?:\.[0-9]+)?) p99_ns=(?<p99>[0-9]+) max_ns=(?<max>[0-9]+)"
$markers = [regex]::Matches($logText, $markerPattern)
if ($markers.Count -ne 1 -or $logText.Contains("KSA64_PHASE12C_GLOBAL_EVIDENCE_FAIL")) { throw "Packaged global-viewer evidence did not emit exactly one PASS marker. See $logPath." }

$manifest = Get-Content -LiteralPath $runtimeManifestPath -Raw | ConvertFrom-Json
$expectedMilestones = @(
    [pscustomobject]@{ label = "enu-to-ecef"; release = 29; frame = 2; segment = 2 },
    [pscustomobject]@{ label = "burnout"; release = 1920; frame = 2; segment = 2 },
    [pscustomobject]@{ label = "ecef-to-gcrf"; release = 3579; frame = 3; segment = 3 },
    [pscustomobject]@{ label = "apogee"; release = 8124; frame = 3; segment = 3 },
    [pscustomobject]@{ label = "gcrf-to-ecef"; release = 12669; frame = 2; segment = 4 },
    [pscustomobject]@{ label = "recovery-enu"; release = 15255; frame = 1; segment = 5 },
    [pscustomobject]@{ label = "drogue"; release = 15257; frame = 1; segment = 5 },
    [pscustomobject]@{ label = "main"; release = 20929; frame = 1; segment = 5 },
    [pscustomobject]@{ label = "landing"; release = 22014; frame = 1; segment = 5 }
)
$expectedOperationalMilestones = @(
    [pscustomobject]@{ kind = "gnss-fault-begins"; release = 5760; gnss = 2; receipt = 0 },
    [pscustomobject]@{ kind = "gnss-fault-qualified"; release = 5824; gnss = 3; receipt = 0 },
    [pscustomobject]@{ kind = "ground-update-stage"; release = 6080; gnss = 3; receipt = 1 },
    [pscustomobject]@{ kind = "ground-update-commit"; release = 6240; gnss = 3; receipt = 2 },
    [pscustomobject]@{ kind = "branch-stage"; release = 6560; gnss = 3; receipt = 1 },
    [pscustomobject]@{ kind = "branch-commit"; release = 6720; gnss = 3; receipt = 2 }
)
if ($manifest.schema -ne "ksa64.phase12c.unreal-global-evidence.v1" -or
    -not $manifest.pass -or
    $manifest.source_commit -ne $commit -or
    $manifest.scenario -ne "ksa-g10r.global/nominal" -or
    $manifest.role -ne "sim-director-read-only" -or
    $manifest.guided_scenario -ne "ksa-g10r.operations/gnss-loss" -or
    $manifest.guided_role -ne "guided-operator" -or
    -not $manifest.accepted_exact -or
    -not $manifest.nominal_truth_permitted -or
    $manifest.nominal_truth_visible -or
    $manifest.guided_truth_permitted -or
    $manifest.guided_truth_visible -or
    $manifest.nominal_terminal_release_epoch -ne 22014 -or
    $manifest.nominal_terminal_disposition -ne 1 -or
    $manifest.guided_terminal_release_epoch -ne 21591 -or
    $manifest.guided_terminal_disposition -ne 2 -or
    $manifest.package.path -ne $manifestGameRelativePath -or
    [int64]$manifest.package.bytes -ne $gameBytes -or
    $manifest.package.sha256 -ne $gameSha256 -or
    $manifest.package_binding.package_audit_sha256 -ne $packageAuditSha256 -or
    $manifest.frozen_reference.releases -ne 22015 -or
    $manifest.frozen_reference.elapsed_seconds -ne "687.9375" -or
    $manifest.frozen_reference.ktt10_sha256 -ne "a50b4b32b1c0feb44a54fc9041c40833717b9032ce127af67a9d34c3488e824a" -or
    $manifest.frozen_reference.kph10_sha256 -ne "cd664e8b72eff7aff1e3c4a5b7fb6859bb9d5178d3b6b6d4c2c06f2c61ed9cf2" -or
    $manifest.frozen_reference.ksr10_sha256 -ne "9e8691933789ce6d870d561218d6888f65acb04ef24e02796be33a704c8678aa" -or
    -not $manifest.renderer.d3d12 -or
    $manifest.renderer.width -ne 1920 -or
    $manifest.renderer.height -ne 1080 -or
    -not $manifest.renderer.fixed_timestep -or
    $manifest.renderer.fixed_delta_seconds -ne "0.016666666666666667" -or
    $manifest.renderer.refresh_hz -ne 60 -or
    [double]$manifest.renderer.frames_per_second -lt 60.0 -or
    -not $manifest.renderer.packaged_runtime -or
    $manifest.renderer.editor_required -or
    $manifest.renderer.mcp_required -or
    $manifest.renderer.python_required -or
    $manifest.captures.Count -ne 9 -or
    $manifest.operational_milestones.Count -ne 6 -or
    $manifest.guided_completed_evidence.actions -ne 4 -or
    [int64]$manifest.guided_completed_evidence.bytes -ne 2911464 -or
    $manifest.guided_completed_evidence.sha256 -ne "7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4" -or
    -not $manifest.guided_completed_evidence.observation_complete -or
    $manifest.guided_completed_evidence.gnss_reacquired -or
    -not $manifest.performance.pass -or
    $manifest.performance.warmup_frames -ne 120 -or
    $manifest.performance.measured_frames -ne 600 -or
    $manifest.performance.release_delta -ne 320 -or
    $manifest.performance.expected_release_delta -ne 320 -or
    [double]$manifest.performance.actual_render_frames_per_second -lt 60.0 -or
    [Math]::Abs([double]$manifest.performance.actual_render_frames_per_second - [double]$manifest.renderer.frames_per_second) -gt 0.000001 -or
    [Math]::Abs([double]$manifest.performance.actual_render_frames_per_second - [double]$markers[0].Groups["fps"].Value) -gt 0.001 -or
    [int64]$manifest.performance.p99_ns -ge 1000000 -or
    [int64]$manifest.performance.max_ns -ge 2000000 -or
    [int64]$manifest.performance.p99_ns -ne [int64]$markers[0].Groups["p99"].Value -or
    [int64]$manifest.performance.max_ns -ne [int64]$markers[0].Groups["max"].Value) {
    throw "Phase 12C runtime manifest failed its source, package, renderer, mission, operational, or performance contract."
}

$validatedCaptures = @()
for ($index = 0; $index -lt $expectedMilestones.Count; ++$index) {
    $expected = $expectedMilestones[$index]
    $capture = $manifest.captures[$index]
    if ($capture.label -ne $expected.label -or $capture.release_epoch -ne $expected.release -or $capture.frame_identity -ne $expected.frame -or $capture.segment_identity -ne $expected.segment -or $capture.source_mask -ne 11 -or $capture.transition_markers -lt 4 -or $capture.planned_path_points -le 0 -or $capture.onboard_path_points -le 0 -or $capture.observed_path_points -le 0 -or $capture.width -ne 1920 -or $capture.height -ne 1080 -or $capture.sampled_pixels -le 0 -or $capture.distinct_color_buckets -lt 8 -or $capture.luminance_range -lt 24 -or $capture.non_dark_samples -le 0) { throw "Milestone capture '$($expected.label)' failed semantic or image validation." }
    $semanticPath = Join-Path $evidenceDirectory $capture.semantic_file
    $screenshotPath = Join-Path $evidenceDirectory $capture.screenshot_file
    foreach ($path in @($semanticPath, $screenshotPath)) { if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Milestone evidence is missing: $path" } }
    $semantic = Get-Content -LiteralPath $semanticPath -Raw | ConvertFrom-Json
    if ($semantic.schema -ne "ksa64.unreal-global-viewer-semantic.v1" -or $semantic.release_epoch -ne $expected.release -or $semantic.frame_identity -ne $expected.frame -or $semantic.segment_identity -ne $expected.segment -or $semantic.source_mask -ne 11 -or -not $semantic.scene_ready -or -not $semantic.acceptance_eligible -or -not $semantic.exact_snap -or -not $semantic.truth_permitted -or $semantic.truth_visible -or $semantic.overall_disposition -ne 1 -or $semantic.evidence_disposition -ne 1) { throw "Milestone semantic snapshot '$($expected.label)' failed exactness or role validation." }
    $dimensions = Get-PngDimensions $screenshotPath
    if ($dimensions.Width -ne 1920 -or $dimensions.Height -ne 1080) { throw "Milestone screenshot '$($expected.label)' is not 1920x1080." }
    $validatedCaptures += [ordered]@{
        label = $expected.label
        release_epoch = $expected.release
        semantic = [ordered]@{ path = $semanticPath.Substring($archive.Length + 1).Replace('\', '/'); sha256 = Get-Sha256 $semanticPath }
        screenshot = [ordered]@{ path = $screenshotPath.Substring($archive.Length + 1).Replace('\', '/'); sha256 = Get-Sha256 $screenshotPath; bytes = [int64](Get-Item -LiteralPath $screenshotPath).Length; width = $dimensions.Width; height = $dimensions.Height }
    }
}

$validatedOperationalMilestones = @()
for ($index = 0; $index -lt $expectedOperationalMilestones.Count; ++$index) {
    $expected = $expectedOperationalMilestones[$index]
    $record = $manifest.operational_milestones[$index]
    if ($record.kind -ne $expected.kind -or
        $record.label -ne $expected.kind -or
        $record.release_epoch -ne $expected.release -or
        $record.selected_release_epoch -ne $expected.release -or
        $record.frame_identity -ne 3 -or
        $record.segment_identity -ne 3 -or
        $record.source_mask -ne 3 -or
        $record.truth_permitted -or
        $record.truth_visible -or
        $record.gnss_state -ne $expected.gnss -or
        $record.gnss_reacquired -or
        ($expected.receipt -ne 0 -and ($record.action_receipt_state -ne $expected.receipt -or $record.action_receipt_accepted -ne 1 -or $record.action_proposal_identity -eq 0))) {
        throw "Operational milestone '$($expected.kind)' failed release, frame, source, truth, action, or disposition semantics."
    }
    $semanticPath = Join-Path $evidenceDirectory $record.semantic_file
    if (-not (Test-Path -LiteralPath $semanticPath -PathType Leaf)) { throw "Operational semantic evidence is missing: $semanticPath" }
    $semantic = Get-Content -LiteralPath $semanticPath -Raw | ConvertFrom-Json
    $viewerSemantic = $semantic.viewer_semantic_json | ConvertFrom-Json
    if ($semantic.schema -ne "ksa64.phase12c.unreal-guided-semantic.v1" -or
        $semantic.label -ne $expected.kind -or
        $semantic.release_epoch -ne $expected.release -or
        $semantic.frame_identity -ne 3 -or
        $semantic.segment_identity -ne 3 -or
        $semantic.source_mask -ne 3 -or
        $semantic.truth_permitted -or
        $semantic.truth_visible -or
        $semantic.gnss_state -ne $expected.gnss -or
        $semantic.gnss_reacquired -or
        $semantic.overall_disposition -ne $record.overall_disposition -or
        $semantic.objective_disposition -ne $record.objective_disposition -or
        $semantic.vehicle_disposition -ne $record.vehicle_disposition -or
        $semantic.procedure_disposition -ne $record.procedure_disposition -or
        $semantic.operator_disposition -ne $record.operator_disposition -or
        $semantic.avionics_disposition -ne $record.avionics_disposition -or
        $semantic.evidence_disposition -ne $record.evidence_disposition -or
        $viewerSemantic.release_epoch -ne $expected.release -or
        $viewerSemantic.frame_identity -ne 3 -or
        $viewerSemantic.segment_identity -ne 3 -or
        $viewerSemantic.source_mask -ne 3 -or
        $viewerSemantic.truth_permitted -or
        $viewerSemantic.truth_visible -or
        -not $viewerSemantic.scene_ready -or
        -not $viewerSemantic.acceptance_eligible -or
        $viewerSemantic.overall_disposition -ne $record.overall_disposition -or
        $viewerSemantic.evidence_disposition -ne $record.evidence_disposition) {
        throw "Operational semantic file '$($expected.kind)' failed normalized renderer parity validation."
    }
    $validatedOperationalMilestones += [ordered]@{
        kind = $expected.kind
        release_epoch = $expected.release
        frame_identity = [int]$record.frame_identity
        segment_identity = [int]$record.segment_identity
        source_mask = [int]$record.source_mask
        truth_visible = [bool]$record.truth_visible
        overall_disposition = [int]$record.overall_disposition
        evidence_disposition = [int]$record.evidence_disposition
        semantic = [ordered]@{ path = $semanticPath.Substring($archive.Length + 1).Replace('\', '/'); sha256 = Get-Sha256 $semanticPath }
    }
}

$validation = [ordered]@{
    schema = "ksa64.phase12c.unreal-global-evidence-validation.v1"
    pass = $true
    source_commit = $commit
    packaged_without_editor_mcp_or_python = $true
    package = [ordered]@{ path = $gameRelativePath; bytes = $gameBytes; sha256 = $gameSha256 }
    package_audit = [ordered]@{ path = $packageAuditPath.Substring($archive.Length + 1).Replace('\', '/'); sha256 = $packageAuditSha256 }
    bridge = [ordered]@{ source_commit = $bridgeManifest.source_commit; manifest_path = $bridgeManifests[0].FullName.Substring($archive.Length + 1).Replace('\', '/'); manifest_sha256 = Get-Sha256 $bridgeManifests[0].FullName; library_path = $bridgeFile.Substring($archive.Length + 1).Replace('\', '/'); library_sha256 = Get-Sha256 $bridgeFile }
    runtime_manifest = [ordered]@{ path = $runtimeManifestPath.Substring($archive.Length + 1).Replace('\', '/'); sha256 = Get-Sha256 $runtimeManifestPath }
    captures = $validatedCaptures
    operational_milestones = $validatedOperationalMilestones
    guided_completed_evidence = [ordered]@{ actions = 4; bytes = 2911464; sha256 = $manifest.guided_completed_evidence.sha256; observation_complete = [bool]$manifest.guided_completed_evidence.observation_complete }
    performance = [ordered]@{ resolution = "1920x1080"; refresh_hz = 60; frames_per_second = [double]$manifest.renderer.frames_per_second; scope = $manifest.performance.scope; frames = [int]$manifest.performance.measured_frames; start_release = [int]$manifest.performance.start_release; end_release = [int]$manifest.performance.end_release; p99_ns = [int64]$manifest.performance.p99_ns; max_ns = [int64]$manifest.performance.max_ns }
    log = [ordered]@{ path = $logPath.Substring($archive.Length + 1).Replace('\', '/'); sha256 = Get-Sha256 $logPath }
    exit_code = $process.ExitCode
}
[System.IO.File]::WriteAllText($validationPath, (($validation | ConvertTo-Json -Depth 8) + [Environment]::NewLine), [System.Text.UTF8Encoding]::new($false))

Write-Host "PHASE 12C PACKAGED D3D12 GLOBAL VIEWER EVIDENCE: PASS"
Write-Host "  archive: $archive"
Write-Host "  source: $commit"
Write-Host "  captures: $($validatedCaptures.Count)"
Write-Host "  operational milestones: $($validatedOperationalMilestones.Count)"
Write-Host "  render fps: $($manifest.renderer.frames_per_second)"
Write-Host "  p99 ns: $($manifest.performance.p99_ns)"
Write-Host "  validation: $validationPath"
