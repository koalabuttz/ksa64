[CmdletBinding()]
param([ValidateRange(1,10)][int]$Runs=3,[switch]$Update)
$ErrorActionPreference='Stop'
$root=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$versions=Get-Content -Raw -LiteralPath (Join-Path $root 'toolchains\versions.json')|ConvertFrom-Json
$wrapper=Join-Path $root 'tools\toolchains\rust-mos.ps1'
$vice=Join-Path $root $versions.vice.projectRelativeExecutable.Replace('/','\')
$output=Join-Path $root 'phase5\timing-v1.json'
function Assert-Exit([string]$label){if($LASTEXITCODE-ne 0){throw "$label failed with exit code $LASTEXITCODE"}}
Push-Location $root
try{
 if(-not(Test-Path -LiteralPath $vice -PathType Leaf)){throw 'Pinned VICE is missing.'}
 if((Get-FileHash -Algorithm SHA256 -LiteralPath $vice).Hash.ToLowerInvariant()-ne $versions.vice.executableSha256){throw 'Pinned VICE hash mismatch.'}
 & $wrapper -ReturnToCaller -WorkingDirectory . cargo build --profile c64 --target mos-c64-none --features c64 -Z build-std=core -Z build-std-features=compiler-builtins-mem --bin ksa64-phase5-vehicle-timed-c64 --bin ksa64-phase5-avionics-timed-c64 --bin ksa64-phase5-telemetry-timed-c64
 Assert-Exit 'Phase 5 timing build'
 $args=@('phase5/reference/vice_timing.py','--vice',$vice,'--vehicle','target/mos-c64-none/c64/ksa64-phase5-vehicle-timed-c64','--avionics','target/mos-c64-none/c64/ksa64-phase5-avionics-timed-c64','--telemetry','target/mos-c64-none/c64/ksa64-phase5-telemetry-timed-c64','--runs',$Runs,'--timeout','600','--output',$output)
 if(-not $Update){$args+='--check'}
 & python -B @args;Assert-Exit 'Phase 5 finite target timing'
 $e=Get-Content -Raw -LiteralPath $output|ConvertFrom-Json
 Write-Host "Vehicle: $($e.cycles.vehicle.cycles) cycles"
 Write-Host "Avionics: $($e.cycles.avionics.cycles) cycles"
 Write-Host "Telemetry: $($e.cycles.telemetry.cycles) cycles"
 Write-Host "Projected nominal: $([math]::Round($e.full_nominal_decision.conservative_seconds/60,1)) minutes"
 Write-Host "Full-run eligible: $($e.full_nominal_decision.eligible)"
}finally{Pop-Location}