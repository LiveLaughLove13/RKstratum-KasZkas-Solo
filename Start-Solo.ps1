# Click-run / local launch of RKstratum Kas+ZKAS Solo (in-process nodes).
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$exe = Join-Path $Root "target\release\RKstratumKasZkasSolo.exe"
if (-not (Test-Path $exe)) {
  Write-Host "Building release binary first..."
  & (Join-Path $Root "scripts\Build.ps1") -RepoRoot $Root
}
& $exe @args
