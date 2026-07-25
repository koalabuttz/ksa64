[CmdletBinding()]
param([switch]$SkipLegacy, [switch]$SkipMos, [switch]$RunVice)
$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$auditRoot = Join-Path $projectRoot (".phase8-5-audit-" + [Guid]::NewGuid().ToString("N"))
function Check { if ($LASTEXITCODE -ne 0) { throw "command failed: $LASTEXITCODE" } }
function Gate([string]$label, [scriptblock]$action) { Write-Host "
=== $label ==="; $global:LASTEXITCODE = 0; & $action; Check }
function Equal([string]$a, [string]$b) { if ((Get-FileHash -Algorithm SHA256 -LiteralPath $a).Hash -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $b).Hash) { throw "artifact mismatch: $a / $b" } }
function NoVice { if ($p = Get-Process x64sc -ErrorAction SilentlyContinue) { throw "Close VICE PID(s) $($p.Id -join ', ')" } }
New-Item -ItemType Directory -Path $auditRoot | Out-Null
Push-Location $projectRoot
try {
    if (-not $SkipLegacy) { Gate "frozen Phase 0-8 audit" { & phase8/complete.ps1 -SkipMos } }
    Gate "Phase 8.5 native audit" {
        cargo fmt --all -- --check; Check
        cargo clippy --workspace --all-targets --features fixtures --target-dir target/phase85 -- -D warnings; Check
        cargo test -p ksa64-core phase8_5 --features fixtures --target-dir target/phase85; Check
        cargo test -p ksa64-interface phase8_5 --target-dir target/phase85; Check
        cargo test -p ksa64-flight phase8_5 --target-dir target/phase85; Check
        cargo test -p ksa64-sim phase8_5 --features fixtures --target-dir target/phase85; Check
        cargo test -p ksa64-host phase8_5 --target-dir target/phase85
    }
    Gate "campaign reproducibility" {
        $one = Join-Path $auditRoot "one.kas8"; $oneJson = Join-Path $auditRoot "one.json"
        $four = Join-Path $auditRoot "four.kas8"; $fourJson = Join-Path $auditRoot "four.json"
        cargo run -q -p ksa64-host --bin phase8_5_campaign --target-dir target/phase85 -- --workers 1 --output $one --evidence $oneJson; Check
        cargo run -q -p ksa64-host --bin phase8_5_campaign --target-dir target/phase85 -- --workers 4 --output $four --evidence $fourJson; Check
        Equal $one $four; Equal $four phase8_5/campaign-64.kas8
        python -B phase8_5/reference/analyze_campaign.py phase8_5/campaign-64.kas8 --evidence phase8_5/campaign-64.json
    }
    Gate "checked C64 evidence" {
        $timing = Get-Content phase8_5/avionics-timing.json -Raw | ConvertFrom-Json
        if (-not $timing.deadline_pass -or $timing.cycles.aided_cycles -gt $timing.cycles.budget_cycles) { throw "PAL avionics timing gate failed" }
        $stock = Get-Content phase8_5/stock-fit.json -Raw | ConvertFrom-Json
        if ($stock.required_linked_bytes -ne ($stock.flat_region.bytes + $stock.flat_overflow_bytes)) { throw "stock-fit arithmetic mismatch" }
        if ($stock.decision -ne "stop_at_explicit_stock_fit_boundary") { throw "unexpected stock-fit decision" }
        foreach ($name in "vice-probe-8.json", "vice-probe-gimbal-8.json") {
            $probe = Get-Content (Join-Path phase8_5/reference $name) -Raw | ConvertFrom-Json
            if ($probe.releases -ne 8 -or -not $probe.artifact.stock_fit) { throw "invalid checked VICE probe: $name" }
        }
    }
    if (-not $SkipMos) {
        Gate "MOS packaging" {
            & tools/toolchains/rust-mos.ps1 -WorkingDirectory . cargo build --profile c64 --target mos-c64-none --features c64 -Z build-std=core -Z build-std-features=compiler-builtins-mem --bin ksa64-phase8-5-mailbox-endpoint-c64 --bin ksa64-phase8-5-avionics-timed-c64; Check
            $endpoint = "target/mos-c64-none/c64/ksa64-phase8-5-mailbox-endpoint-c64"
            $probe = Get-Content phase8_5/reference/vice-probe-8.json -Raw | ConvertFrom-Json
            if ((Get-FileHash $endpoint -Algorithm SHA256).Hash.ToLower() -ne $probe.artifact.sha256) { throw "endpoint artifact hash mismatch" }
        }
    }
    if ($RunVice) {
        if ($SkipMos) { throw "-RunVice requires MOS artifacts" }
        $versions = Get-Content toolchains/versions.json -Raw | ConvertFrom-Json
        $vice = (Resolve-Path $versions.vice.projectRelativeExecutable).Path
        $endpoint = "target/mos-c64-none/c64/ksa64-phase8-5-mailbox-endpoint-c64"
        $timed = "target/mos-c64-none/c64/ksa64-phase8-5-avionics-timed-c64"
        cargo build -p ksa64-host --bin phase8_5_bridge --target-dir target/phase85; Check
        $broker = "target/phase85/debug/phase8_5_bridge.exe"
        Gate "finite VICE evidence" {
            NoVice
            python -B phase8_5/reference/vice_avionics_timing.py --vice $vice --prg $timed --runs 3 --output (Join-Path $auditRoot "timing.json"); Check
            NoVice
            python -B phase8_5/reference/vice_mailbox_bridge.py --vice $vice --prg $endpoint --broker $broker --max-releases 8 --output (Join-Path $auditRoot "monitor.json"); Check
            NoVice
            python -B phase8_5/reference/vice_mailbox_bridge.py --vice $vice --prg $endpoint --broker $broker --max-releases 8 --gimbal --output (Join-Path $auditRoot "gimbal.json"); Check
            NoVice
        }
    }
    Write-Host "
PHASE 8.5 COMPLETION AUDIT: PASS"
} finally {
    Pop-Location
    $resolved = [IO.Path]::GetFullPath($auditRoot)
    if (-not $resolved.StartsWith($projectRoot + [IO.Path]::DirectorySeparatorChar)) { throw "unsafe audit cleanup" }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
