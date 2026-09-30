<#
.SYNOPSIS
  Release-build RKstratumKasZkasSolo (embedded Kaspa + ZKAS) with MSVC env.
#>
[CmdletBinding()]
param(
  [string]$RepoRoot = "",
  [string]$TargetDir = ""
)

$ErrorActionPreference = "Stop"
if (-not $RepoRoot) {
  $here = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
  $RepoRoot = (Resolve-Path (Join-Path $here "..")).Path
}
if (-not $TargetDir) {
  $TargetDir = Join-Path $RepoRoot "target"
}

$Vcvars = "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat"
if (-not (Test-Path $Vcvars)) {
  $Vcvars = "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
}
if (-not (Test-Path $Vcvars)) {
  $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
  if (Test-Path $vswhere) {
    $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($vs) { $Vcvars = Join-Path $vs "VC\Auxiliary\Build\vcvars64.bat" }
  }
}
if (-not (Test-Path $Vcvars)) { throw "vcvars64.bat not found - install VS C++ tools" }

$cmd = 'call "' + $Vcvars + '" && set "CARGO_TARGET_DIR=' + $TargetDir + '" && cd /d "' + $RepoRoot + '" && cargo build -p kaspa-stratum-bridge --release --bin RKstratumKasZkasSolo'
Write-Host "Building RKstratumKasZkasSolo (release)..."
cmd /c $cmd
if ($LASTEXITCODE -ne 0) { throw "cargo build failed ($LASTEXITCODE)" }

$exe = Join-Path $TargetDir "release\RKstratumKasZkasSolo.exe"
if (-not (Test-Path $exe)) { throw "Missing $exe" }
Write-Host "OK: $exe ($((Get-Item $exe).Length) bytes)"
