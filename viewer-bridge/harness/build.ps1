param([switch]$PanicProbe)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path "$PSScriptRoot\..\..").Path
Push-Location $repo
try {
  $featureArgs = if ($PanicProbe) { @('--features','panic-probe') } else { @() }
  & cargo build -p ksa64-viewer-bridge --profile viewer @featureArgs
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  $commit = (& git rev-parse --short=12 HEAD).Trim()
  if ($LASTEXITCODE -ne 0 -or -not $commit) { throw 'could not resolve bridge commit identity' }
  $source = "$repo\target\viewer\ksa64_viewer_bridge.dll"
  $staged = "$repo\target\viewer\ksa64_viewer_bridge_$commit.dll"
  Copy-Item -LiteralPath $source -Destination $staged -Force
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
  $stream = [System.IO.File]::OpenRead($staged)
  try { $hash = ([System.BitConverter]::ToString($sha256.ComputeHash($stream))).Replace('-', '').ToLowerInvariant() }
  finally { $stream.Dispose(); $sha256.Dispose() }
  [ordered]@{ schema='ksa64.viewer-bridge-artifact.v1'; abi_version=1; commit=$commit; file=(Split-Path $staged -Leaf); sha256=$hash } |
    ConvertTo-Json | Set-Content -LiteralPath "$staged.json" -Encoding utf8
  New-Item -ItemType Directory -Path "$PSScriptRoot\bin" -Force | Out-Null
  & cl /nologo /std:c++20 /EHsc /W4 /WX "$PSScriptRoot\main.cpp" /Fe:"$PSScriptRoot\bin\ksa64_viewer_harness.exe"
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  & "$PSScriptRoot\bin\ksa64_viewer_harness.exe" $staged
  exit $LASTEXITCODE
} finally { Pop-Location }