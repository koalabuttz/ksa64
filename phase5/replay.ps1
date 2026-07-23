[CmdletBinding()]
param([switch]$Freeze)
$ErrorActionPreference="Stop"
$root=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$versions=Get-Content -Raw -LiteralPath (Join-Path $root "toolchains/versions.json")|ConvertFrom-Json
$rust=Join-Path $root "tools/toolchains/rust-mos.ps1";$vice=Join-Path $root $versions.vice.projectRelativeExecutable.Replace("/","\")
Push-Location $root
try{
 & $rust -ReturnToCaller -WorkingDirectory "." cargo build --profile c64 --target mos-c64-none --features c64 -Z build-std=core -Z build-std-features=compiler-builtins-mem --bin ksa64-phase5-replay-c64
 if($LASTEXITCODE-ne 0){throw "Phase 5 replay build failed"}
 $prg="target/mos-c64-none/c64/ksa64-phase5-replay-c64";$json=python -B phase5/reference/vice_replay.py --vice $vice --prg $prg --timeout 180;if($LASTEXITCODE-ne 0){throw "Phase 5 replay failed"};$result=($json-join"`n")|ConvertFrom-Json
 $bytes=(Get-Item -LiteralPath $prg).Length;$hash=(Get-FileHash -LiteralPath $prg -Algorithm SHA256).Hash.ToLowerInvariant();$tape=(Get-FileHash -LiteralPath phase5/examples/ksa5-baseline.kph5 -Algorithm SHA256).Hash.ToLowerInvariant();$load=[BitConverter]::ToUInt16([IO.File]::ReadAllBytes((Resolve-Path $prg)),0);$end=$load+$bytes-2
 $evidence=[ordered]@{schema="KSA64 phase5 replay v1";replay_prg_bytes=$bytes;replay_prg_sha256=$hash;load_end=("0x{0:x4}"-f$end);tape_sha256=$tape;screen_sha256=$result.screen_sha256;plot_cells=$result.plot_cells;cue_hash="0x3b2fb64b"}
 $text=$evidence|ConvertTo-Json
 if($Freeze){[IO.File]::WriteAllText((Join-Path $root "phase5/replay-v1.json"),$text+"`n",[Text.UTF8Encoding]::new($false))}else{$accepted=Get-Content -Raw -LiteralPath phase5/replay-v1.json|ConvertFrom-Json;if(($evidence|ConvertTo-Json -Compress)-ne($accepted|ConvertTo-Json -Compress)){throw "Phase 5 replay differs from frozen evidence"}}
 if($end-gt0xc000){throw "Phase 5 replay exceeds stock RAM"};Write-Host $result.rows.'0';Write-Host $result.rows.'1';Write-Host $result.rows.'4';Write-Host $result.rows.'24';Write-Host "Replay PRG: $bytes bytes, load end $($evidence.load_end)";Write-Host "PHASE 5 C64 REPLAY: PASS"
}finally{Pop-Location}
