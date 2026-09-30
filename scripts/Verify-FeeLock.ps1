<#
.SYNOPSIS
  Verify fee lock unit tests + debug exists (does not sync full nodes).
#>
[CmdletBinding()]
param(
  [string]$RepoRoot = ""
)

$ErrorActionPreference = "Stop"
if (-not $RepoRoot) {
  $here = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
  $RepoRoot = (Resolve-Path (Join-Path $here "..")).Path
}

$Vcvars = "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat"
if (-not (Test-Path $Vcvars)) {
  $Vcvars = "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
}
if (-not (Test-Path $Vcvars)) { throw "vcvars64.bat not found" }

$target = Join-Path $RepoRoot "target"
$cmd = 'call "' + $Vcvars + '" && set "CARGO_TARGET_DIR=' + $target + '" && cd /d "' + $RepoRoot + '" && cargo test -p kaspa-stratum-bridge solo_fee --lib && cargo build -p kaspa-stratum-bridge --bin RKstratumKasZkasSolo'
Write-Host "== solo_fee lock tests + bin build =="
cmd /c $cmd
if ($LASTEXITCODE -ne 0) { throw "verify failed ($LASTEXITCODE)" }

$exe = Join-Path $target "debug\RKstratumKasZkasSolo.exe"
if (-not (Test-Path $exe)) { throw "Binary not found: $exe" }
Write-Host "OK binary: $exe"
Write-Host "Fee lock: hardcoded 1% (bridge/src/solo_fee.rs HARDCODED_*)"
Write-Host "Full node E2E: run the exe, wait for NODES READY, point a miner at :7666."
