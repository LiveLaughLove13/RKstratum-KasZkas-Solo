<#
.SYNOPSIS
  Build + package Windows zip and (when WSL is available) Linux tar.gz.
#>
[CmdletBinding()]
param(
  [string]$RepoRoot = "",
  [string]$Version = "1.0.0",
  [string]$TargetDir = "",
  [switch]$SkipLinux,
  [switch]$SkipWindows
)

$ErrorActionPreference = "Stop"
if (-not $RepoRoot) {
  $here = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
  $RepoRoot = (Resolve-Path (Join-Path $here "..")).Path
}
if (-not $TargetDir) {
  $TargetDir = Join-Path $RepoRoot "target"
}

$distDir = Join-Path $RepoRoot "dist"
New-Item -ItemType Directory -Force -Path $distDir | Out-Null

if (-not $SkipWindows) {
  Write-Host "=== Windows package ==="
  $buildScript = Join-Path $RepoRoot "scripts\Build.ps1"
  & $buildScript -RepoRoot $RepoRoot -TargetDir $TargetDir

  $exeSrc = Join-Path $TargetDir "release\RKstratumKasZkasSolo.exe"
  $stage = Join-Path $distDir "stage-RKstratum-KasZkas-Solo-windows"
  if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
  New-Item -ItemType Directory -Force -Path $stage | Out-Null

  Copy-Item -Force $exeSrc (Join-Path $stage "RKstratumKasZkasSolo.exe")
  Copy-Item -Force (Join-Path $RepoRoot "README.txt") $stage
  Copy-Item -Force (Join-Path $RepoRoot "LICENSE") $stage -ErrorAction SilentlyContinue

  $zipName = "RKstratum-KasZkas-Solo-windows-x64-v$Version.zip"
  $zipPath = Join-Path $distDir $zipName
  if (Test-Path $zipPath) { Remove-Item -Force $zipPath }
  Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zipPath -Force
  $hash = (Get-FileHash -Algorithm SHA256 -Path $zipPath).Hash.ToLowerInvariant()
  $setPath = Join-Path $distDir "RKstratum-KasZkas-Solo-windows-x64-v$Version.sha256"
  "$hash  $zipName" | Set-Content -Path $setPath -Encoding ascii

  Write-Host "Package: $zipPath"
  Write-Host "SHA256:  $hash"
}

if (-not $SkipLinux) {
  Write-Host "=== Linux package ==="
  if (-not (Get-Command wsl -ErrorAction SilentlyContinue)) {
    Write-Warning "WSL not found - Linux package skipped. On a Linux host run: ./scripts/Package-Release-Linux.sh $Version"
  } else {
    $drive = $RepoRoot.Substring(0, 1).ToLowerInvariant()
    $wslRepo = "/mnt/$drive" + ($RepoRoot.Substring(2) -replace '\\', '/')
    # Build bash command without PowerShell interpreting || / &&
    $bashCmd = @(
      "sed -i 's/\r`$//' '$wslRepo/scripts/Build-Linux.sh' '$wslRepo/scripts/Package-Release-Linux.sh'"
      "chmod +x '$wslRepo/scripts/Build-Linux.sh' '$wslRepo/scripts/Package-Release-Linux.sh'"
      "if [ -f `"`$HOME/.cargo/env`" ]; then . `"`$HOME/.cargo/env`"; fi"
      "cd '$wslRepo'"
      "./scripts/Package-Release-Linux.sh $Version"
    ) -join "; "
    wsl -e bash -lc $bashCmd
    if ($LASTEXITCODE -ne 0) { throw "Linux package via WSL failed ($LASTEXITCODE)" }

    $lin = Join-Path $distDir "RKstratum-KasZkas-Solo-linux-x64-v$Version.tar.gz"
    $linSha = Join-Path $distDir "RKstratum-KasZkas-Solo-linux-x64-v$Version.sha256"
    if (-not (Test-Path $lin)) { throw "Missing Linux package: $lin" }
    Write-Host "Linux OK: $lin"
    if (Test-Path $linSha) { Write-Host ("  " + (Get-Content $linSha -Raw).Trim()) }
  }
}

Write-Host ""
Write-Host "Done. Artifacts in: $distDir"
Get-ChildItem $distDir -File |
  Where-Object { $_.Name -like "RKstratum-KasZkas-Solo-windows*" -or $_.Name -like "RKstratum-KasZkas-Solo-linux*" } |
  Format-Table Name, Length -AutoSize
