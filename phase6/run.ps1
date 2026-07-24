[CmdletBinding()]
param(
    [ValidateSet("host")]
    [string]$World = "host",
    [ValidateSet("host", "vice")]
    [string]$Flight = "host",
    [ValidateSet("host", "disabled")]
    [string]$MissionControl = "host",
    [ValidateSet("fast", "realtime", "step")]
    [string]$Pace = "fast",
    [ValidateSet("adaptive", "tui", "summary", "none")]
    [string]$Display = "adaptive",
    [ValidateSet("si", "dual", "us")]
    [string]$Units = "si",
    [ValidateSet("off", "cues", "cinematic")]
    [string]$Sound = "cues",
    [string]$Record = "auto",
    [switch]$NoBuild,
    [switch]$Smoke
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
Push-Location $projectRoot
try {
    Write-Host "KSA64 Phase 6 deployment"
    Write-Host "  world:           $World"
    Write-Host "  flight computer: $Flight"
    Write-Host "  mission control: $MissionControl"
    Write-Host "  pace:            $Pace"
    Write-Host "  display:         $Display"
    Write-Host "  units / sound:   $Units / $Sound"
    Write-Host "  recording:       $Record"
    Write-Host ""

    if ($Flight -eq "host") {
        $hostArgs = @("run", "--quiet", "-p", "ksa64-host", "--bin", "phase6_launch", "--", "--world", $World, "--flight", "host", "--mission-control", $MissionControl, "--pace", $Pace, "--display", $Display, "--units", $Units, "--sound", $Sound, "--record", $Record)
        & cargo @hostArgs
        if ($LASTEXITCODE -ne 0) { throw "host mission failed with exit code $LASTEXITCODE" }
        return
    }

    $running = Get-Process -Name "x64sc" -ErrorAction SilentlyContinue
    if ($running) { throw "Refusing to launch another VICE instance; close PID(s) $($running.Id -join ', ')." }

    $versions = Get-Content -Raw -LiteralPath "toolchains/versions.json" | ConvertFrom-Json
    $vice = (Resolve-Path -LiteralPath $versions.vice.projectRelativeExecutable).Path
    $prg = Join-Path $projectRoot "target/mos-c64-none/c64/ksa64-phase6-mailbox-endpoint-c64"
    $broker = Join-Path $projectRoot "target/debug/phase6_bridge.exe"
    if (-not (Test-Path -LiteralPath $broker)) { $broker = Join-Path $projectRoot "target/debug/phase6_bridge" }

    if (-not $NoBuild) {
        & cargo build -p ksa64-host --bin phase6_bridge
        if ($LASTEXITCODE -ne 0) { throw "host broker build failed with exit code $LASTEXITCODE" }
        $rustWrapper = Join-Path $projectRoot "tools/toolchains/rust-mos.ps1"
        $mosArgs = @("cargo", "build", "--profile", "c64", "--target", "mos-c64-none", "--features", "c64", "-Z", "build-std=core", "-Z", "build-std-features=compiler-builtins-mem", "--bin", "ksa64-phase6-mailbox-endpoint-c64")
        & $rustWrapper -WorkingDirectory . @mosArgs
        if ($LASTEXITCODE -ne 0) { throw "C64 flight endpoint build failed with exit code $LASTEXITCODE" }
    }

    if (-not (Test-Path -LiteralPath $broker)) { throw "missing host broker: $broker" }
    if (-not (Test-Path -LiteralPath $prg)) { throw "missing C64 flight endpoint: $prg" }
    $pythonArgs = @("-B", "phase6/reference/vice_mailbox_bridge.py", "--vice", $vice, "--prg", $prg, "--broker", $broker, "--mission-control", $MissionControl, "--pace", $Pace, "--display", $Display, "--units", $Units, "--sound", $Sound, "--record", $Record)
    if ($Smoke) { $pythonArgs += @("--max-epochs", "8") }
    & python @pythonArgs
    if ($LASTEXITCODE -ne 0) { throw "VICE mission failed with exit code $LASTEXITCODE" }
} finally {
    Pop-Location
}
