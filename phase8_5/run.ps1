[CmdletBinding()]
param(
    [ValidateSet("host", "vice")]
    [string]$Flight = "host",
    [ValidateSet("tui", "summary", "none")]
    [string]$Display = "tui",
    [ValidateSet("fast", "realtime")]
    [string]$Pace = "realtime",
    [switch]$Gimbal,
    [ValidateRange(1, 65535)]
    [int]$ProbeReleases = 8,
    [string]$Record,
    [switch]$NoBuild
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
Push-Location $projectRoot
try {
    Write-Host "KSA64 Phase 8.5 local-ENU avionics"
    Write-Host "  world:           host"
    Write-Host "  flight computer: $Flight"
    Write-Host "  display / pace:  $Display / $Pace"
    Write-Host "  capability:      $(if ($Gimbal) { 'two-axis gimbal derivative' } else { 'monitor-only Firestorm' })"

    if ($Flight -eq "host") {
        $args = @("run", "--quiet", "-p", "ksa64-host", "--bin", "phase8_5_launch", "--target-dir", "target/phase85", "--", "--display", $Display, "--pace", $Pace)
        if ($Gimbal) { $args += "--gimbal" }
        if ($Record) { $args += @("--record", $Record) }
        & cargo @args
        if ($LASTEXITCODE -ne 0) { throw "host mission failed with exit code $LASTEXITCODE" }
        return
    }

    if ($Display -eq "tui") {
        throw "The bounded VICE acceptance probe is noninteractive. Live host/VICE TUI is enabled by the Phase 8.5 bridge after the finite exactness gate."
    }
    $running = Get-Process -Name "x64sc" -ErrorAction SilentlyContinue
    if ($running) { throw "Refusing to launch another VICE instance; close PID(s) $($running.Id -join ', ')." }
    $versions = Get-Content -Raw -LiteralPath "toolchains/versions.json" | ConvertFrom-Json
    $vice = (Resolve-Path -LiteralPath $versions.vice.projectRelativeExecutable).Path
    $prg = Join-Path $projectRoot "target/mos-c64-none/c64/ksa64-phase8-5-mailbox-endpoint-c64"
    $broker = Join-Path $projectRoot "target/phase85/debug/phase8_5_bridge.exe"
    if (-not (Test-Path -LiteralPath $broker)) { $broker = Join-Path $projectRoot "target/phase85/debug/phase8_5_bridge" }
    if (-not $NoBuild) {
        & cargo build -p ksa64-host --bin phase8_5_bridge --target-dir target/phase85
        if ($LASTEXITCODE -ne 0) { throw "host bridge build failed with exit code $LASTEXITCODE" }
        $mosArgs = @("cargo", "build", "--profile", "c64", "--target", "mos-c64-none", "--features", "c64", "-Z", "build-std=core", "-Z", "build-std-features=compiler-builtins-mem", "--bin", "ksa64-phase8-5-mailbox-endpoint-c64")
        & tools/toolchains/rust-mos.ps1 -WorkingDirectory . @mosArgs
        if ($LASTEXITCODE -ne 0) { throw "C64 endpoint build failed with exit code $LASTEXITCODE" }
    }
    if (-not (Test-Path -LiteralPath $prg)) { throw "missing C64 endpoint: $prg" }
    if (-not (Test-Path -LiteralPath $broker)) { throw "missing host bridge: $broker" }
    $output = Join-Path $projectRoot "phase8_5/reference/vice-probe-$ProbeReleases.json"
    & python -B phase8_5/reference/vice_mailbox_bridge.py --vice $vice --prg $prg --broker $broker --max-releases $ProbeReleases --output $output
    if ($LASTEXITCODE -ne 0) { throw "VICE acceptance probe failed with exit code $LASTEXITCODE" }
} finally {
    Pop-Location
}
