# Minimal-downtime promote for dual-external (Kas+ZKAS stratum :6666).
#
# Build completes into target-promote/ while the live process keeps serving.
# Downtime is only the stop → copy → start window (seconds).
#
# Usage:
#   .\scripts\Promote-DualExternal.ps1              # build + swap + start
#   .\scripts\Promote-DualExternal.ps1 -BuildOnly   # stage binary, leave live running
#   .\scripts\Promote-DualExternal.ps1 -RestartOnly # swap staged binary + start
#   .\scripts\Promote-DualExternal.ps1 -FromZip path\to\stratum-bridge-dual-windows-amd64.zip

[CmdletBinding()]
param(
    [switch]$BuildOnly,
    [switch]$RestartOnly,
    [string]$FromZip = "",
    [string]$ConfigPath = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$liveTarget = Join-Path $root "target"
$stageTarget = Join-Path $root "target-promote"
$liveExe = Join-Path $liveTarget "release\stratum-bridge.exe"
$stageExe = Join-Path $stageTarget "release\stratum-bridge.exe"
$backupExe = Join-Path $liveTarget "release\stratum-bridge.exe.prev"
$config = if ($ConfigPath) { $ConfigPath } else { Join-Path $root "bridge\config.yaml" }

if ($BuildOnly -and $RestartOnly) {
    throw "Use only one of -BuildOnly / -RestartOnly"
}
if (-not (Test-Path $config)) {
    throw "Config not found: $config (pass -ConfigPath if it lives elsewhere)"
}

function Get-DualBridgeProcesses {
    Get-CimInstance Win32_Process -Filter "Name = 'stratum-bridge.exe'" -ErrorAction SilentlyContinue |
        Where-Object {
            $_.CommandLine -match '--node-mode\s+external' -or
            $_.CommandLine -match [regex]::Escape($config) -or
            $_.CommandLine -match 'config\.yaml'
        }
}

function Stop-DualBridge {
    $procs = @(Get-DualBridgeProcesses)
    if ($procs.Count -eq 0) {
        Write-Host "No dual-external stratum-bridge process found (ok if already stopped)."
        return
    }
    foreach ($p in $procs) {
        Write-Host "Stopping PID $($p.ProcessId)..."
        Stop-Process -Id $p.ProcessId -Force -ErrorAction Stop
    }
    $deadline = (Get-Date).AddSeconds(15)
    while ((Get-Date) -lt $deadline) {
        if (@(Get-DualBridgeProcesses).Count -eq 0) { break }
        Start-Sleep -Milliseconds 200
    }
    if (@(Get-DualBridgeProcesses).Count -gt 0) {
        throw "Timed out waiting for dual-external process to exit"
    }
}

function Start-DualBridge {
    Write-Host "Starting dual-external (config=$config)..."
    $env:CARGO_TARGET_DIR = $liveTarget
    Start-Process -FilePath $liveExe -ArgumentList @("--config", $config, "--node-mode", "external") -WorkingDirectory $root
}

function Ensure-LiveDir {
    $dir = Split-Path -Parent $liveExe
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
}

function Install-StagedBinary {
    if (-not (Test-Path $stageExe)) {
        throw "Staged binary missing: $stageExe (run without -RestartOnly first, or -FromZip)"
    }
    Ensure-LiveDir
    if (Test-Path $liveExe) {
        Copy-Item -Force $liveExe $backupExe
        Write-Host "Backed up live binary -> $backupExe"
    }
    Copy-Item -Force $stageExe $liveExe
    Write-Host "Installed staged binary -> $liveExe"
}

# --- Stage from GH zip (optional) ---
if ($FromZip) {
    if (-not (Test-Path $FromZip)) { throw "Zip not found: $FromZip" }
    $extract = Join-Path $stageTarget "from-zip"
    if (Test-Path $extract) { Remove-Item -Recurse -Force $extract }
    New-Item -ItemType Directory -Path (Split-Path -Parent $stageExe) -Force | Out-Null
    Expand-Archive -Path $FromZip -DestinationPath $extract -Force
    $found = Get-ChildItem -Path $extract -Recurse -Filter "stratum-bridge.exe" | Select-Object -First 1
    if (-not $found) { throw "No stratum-bridge.exe inside $FromZip" }
    Copy-Item -Force $found.FullName $stageExe
    Write-Host "Staged from zip: $stageExe"
    if ($BuildOnly) { return }
    $RestartOnly = $true
}

# --- Build into side target (live process untouched) ---
if (-not $RestartOnly) {
    Write-Host "Building release into $stageTarget (live binary untouched)..."
    $env:CARGO_TARGET_DIR = $stageTarget
    & cargo build -p kaspa-stratum-bridge --release --bin stratum-bridge
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed ($LASTEXITCODE)" }
    if (-not (Test-Path $stageExe)) { throw "Build succeeded but missing $stageExe" }
    Write-Host "Staged: $stageExe"
    if ($BuildOnly) {
        Write-Host "Build-only done. When ready: .\scripts\Promote-DualExternal.ps1 -RestartOnly"
        return
    }
}

# --- Brief downtime window ---
Write-Host "=== promote window (stop -> install -> start) ==="
Stop-DualBridge
Install-StagedBinary
Start-DualBridge
Write-Host "Promote complete. Cap settle / nodes left alone."
Write-Host "Rollback if needed: stop bridge, copy $backupExe over $liveExe, start again."
