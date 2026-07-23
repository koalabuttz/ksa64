[CmdletBinding()]
param()
$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$versions = Get-Content -Raw -LiteralPath (Join-Path $projectRoot "toolchains\versions.json") | ConvertFrom-Json
$vice = Join-Path $projectRoot $versions.vice.projectRelativeExecutable.Replace("/", "\")
$c1541 = Join-Path (Split-Path -Parent $vice) "c1541.exe"
$examples = (Resolve-Path -LiteralPath (Join-Path $projectRoot "phase4\examples")).Path
$exporter = Join-Path $projectRoot "target\debug\phase4_export.exe"
$joiner = Join-Path $projectRoot "target\debug\phase4_join.exe"
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) ("ksa64-phase4-export-" + [Guid]::NewGuid().ToString("N"))

function Assert-ExitCode([string]$label) {
    if ($LASTEXITCODE -ne 0) { throw "$label failed with exit code $LASTEXITCODE." }
}
function Invoke-ExpectedFailure([string]$label, [scriptblock]$action) {
    & $action
    if ($LASTEXITCODE -eq 0) { throw "$label unexpectedly succeeded." }
}
function Assert-BytesEqual([string]$label, [string]$left, [string]$right) {
    $a = [IO.File]::ReadAllBytes($left)
    $b = [IO.File]::ReadAllBytes($right)
    if ($a.Length -ne $b.Length) {
        throw "$label byte comparison failed."
    }
    for ($index = 0; $index -lt $a.Length; $index++) {
        if ($a[$index] -ne $b[$index]) { throw "$label byte comparison failed." }
    }
}
function Write-HashSidecar([string]$artifact) {
    $item = Get-Item -LiteralPath $artifact
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $item.FullName).Hash.ToLowerInvariant()
    $sidecar = "$($item.FullName).sha256"
    [IO.File]::WriteAllText($sidecar, "$hash  $($item.Name)`n", [Text.UTF8Encoding]::new($false))
}
function New-VolumeDisk([string]$image, [string]$label, [string]$source, [string]$cbmName) {
    if (-not $image.StartsWith($examples, [StringComparison]::OrdinalIgnoreCase) -and
        -not $image.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to create a disk image outside the export roots: $image"
    }
    if (Test-Path -LiteralPath $image) { Remove-Item -LiteralPath $image }
    & $c1541 -format "$label,04" d64 $image -write $source $cbmName -dir
    Assert-ExitCode "Create $label disk"
}

New-Item -ItemType Directory -Path $tempRoot | Out-Null
Push-Location $projectRoot
try {
    cargo build -p ksa64-host --bins
    Assert-ExitCode "Phase 4 export tools build"
    & $exporter $examples
    Assert-ExitCode "Phase 4 artifact generation"

    $stockVolume = Join-Path $examples "ksa4-stock-report.kxv4"
    $stockArchive = Join-Path $examples "ksa4-stock-report.kra4"
    $stockDisk = Join-Path $examples "ksa4-report.d64"
    New-VolumeDisk $stockDisk "KSA4 REPORT" $stockVolume "KSA4REPORT"
    $stockReadback = Join-Path $tempRoot "stock-readback.kxv4"
    & $c1541 $stockDisk -read KSA4REPORT $stockReadback
    Assert-ExitCode "Read stock report disk"
    Assert-BytesEqual "Stock disk" $stockVolume $stockReadback
    $stockJoined = Join-Path $tempRoot "stock-joined.kra4"
    & $joiner $stockJoined $stockReadback
    Assert-ExitCode "Join stock report"
    Assert-BytesEqual "Stock archive" $stockArchive $stockJoined

    $originalVolumes = @()
    $readbackVolumes = @()
    for ($index = 1; $index -le 3; $index++) {
        $tag = $index.ToString("00")
        $volume = Join-Path $examples "ksa4-synthetic-$tag.kxv4"
        $disk = Join-Path $tempRoot "synthetic-$tag.d64"
        $readback = Join-Path $tempRoot "synthetic-$tag-readback.kxv4"
        New-VolumeDisk $disk "KSA4 VOL$tag" $volume "KSA4VOL$tag"
        & $c1541 $disk -read "KSA4VOL$tag" $readback
        Assert-ExitCode "Read synthetic volume $tag"
        Assert-BytesEqual "Synthetic disk $tag" $volume $readback
        $originalVolumes += $volume
        $readbackVolumes += $readback
    }
    $expectedJoined = Join-Path $tempRoot "synthetic-expected.bin"
    $actualJoined = Join-Path $tempRoot "synthetic-actual.bin"
    & $joiner $expectedJoined @originalVolumes
    Assert-ExitCode "Join original synthetic volumes"
    & $joiner $actualJoined @readbackVolumes
    Assert-ExitCode "Join disk-read synthetic volumes"
    Assert-BytesEqual "Synthetic joined archive" $expectedJoined $actualJoined

    Invoke-ExpectedFailure "Missing-volume rejection" {
        & $joiner (Join-Path $tempRoot "missing.bin") $readbackVolumes[0] $readbackVolumes[1]
    }
    Invoke-ExpectedFailure "Reordered-volume rejection" {
        & $joiner (Join-Path $tempRoot "reordered.bin") $readbackVolumes[1] $readbackVolumes[0] $readbackVolumes[2]
    }
    $corrupt = Join-Path $tempRoot "corrupt-volume.kxv4"
    $corruptBytes = [IO.File]::ReadAllBytes($readbackVolumes[1])
    $corruptBytes[64] = $corruptBytes[64] -bxor 0x80
    [IO.File]::WriteAllBytes($corrupt, $corruptBytes)
    Invoke-ExpectedFailure "Corrupt-volume rejection" {
        & $joiner (Join-Path $tempRoot "corrupt.bin") $readbackVolumes[0] $corrupt $readbackVolumes[2]
    }

    $fullDisk = Join-Path $tempRoot "full.d64"
    $tooBig = Join-Path $tempRoot "too-big.bin"
    $stream = [IO.File]::Create($tooBig)
    try { $stream.SetLength(200000) } finally { $stream.Dispose() }
    Invoke-ExpectedFailure "Disk-full rejection" {
        & $c1541 -format "KSA4 FULL,04" d64 $fullDisk -write $tooBig TOOBIG
    }

    foreach ($name in @(
        "ksa4-baseline.kst4",
        "ksa4-stock-report.kra4",
        "ksa4-stock-report.kxv4",
        "ksa4-synthetic-01.kxv4",
        "ksa4-synthetic-02.kxv4",
        "ksa4-synthetic-03.kxv4",
        "ksa4-report.d64"
    )) {
        Write-HashSidecar (Join-Path $examples $name)
    }
    Write-Host "Stock report: 3712 bytes in one validated D64 volume"
    Write-Host "Synthetic report: 3000 bytes joined from three validated D64 volumes"
    Write-Host "Missing, reordered, corrupt, and disk-full cases: rejected"
    Write-Host "PHASE 4 EXPORT: PASS"
} finally {
    Pop-Location
    $resolvedTemp = [IO.Path]::GetFullPath($tempRoot)
    $systemTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if ($resolvedTemp.StartsWith($systemTemp, [StringComparison]::OrdinalIgnoreCase) -and
        (Split-Path -Leaf $resolvedTemp).StartsWith("ksa64-phase4-export-")) {
        Remove-Item -LiteralPath $resolvedTemp -Recurse -Force
    }
}