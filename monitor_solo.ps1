# Solo dual-external stratum monitor (MRR -> Kas+ZKAS).
# Samples http://127.0.0.1:3133/api/stats every 60s; prints a wake line every 5 min.

$ErrorActionPreference = "Continue"
$uri = "http://127.0.0.1:3133/api/stats"
$outDir = Join-Path $PSScriptRoot "logs"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$csv = Join-Path $outDir "solo_monitor_$stamp.csv"
"ts,uptime_s,workers,shares,blocks,blocks_accepted,hr_ghs,stale,invalid,weak" | Set-Content $csv

$tick = 0
while ($true) {
  $tick++
  $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
  try {
    $s = Invoke-RestMethod $uri -TimeoutSec 10
    $hr = 0.0; $stale = 0; $inv = 0; $weak = 0
    foreach ($w in @($s.workers)) {
      $hr += [double]$w.hashrate
      $stale += [int]$w.stale
      $inv += [int]$w.invalid
      $weak += [int]$w.weakShares
    }
    $line = "{0},{1},{2},{3},{4},{5},{6:N1},{7},{8},{9}" -f `
      $ts, [int]$s.bridgeUptime, [int]$s.activeWorkers, [int]$s.totalShares, `
      [int]$s.totalBlocks, [int]$s.totalBlocksAcceptedByNode, $hr, $stale, $inv, $weak
    Add-Content $csv $line

    $workers = (@($s.workers) | ForEach-Object {
      "{0}:{1:N0}GH/s sh={2} blk={3} {4}" -f $_.worker, [double]$_.hashrate, $_.shares, $_.blocks, $_.status
    }) -join " | "

    if (($tick % 5) -eq 0) {
      Write-Output ("AGENT_SOLO_TICK ts={0} up={1}s workers={2} shares={3} blocks={4}/{5} hr_ghs={6:N0} stale={7} inv={8} weak={9} :: {10}" -f `
        $ts, [int]$s.bridgeUptime, [int]$s.activeWorkers, [int]$s.totalShares, `
        [int]$s.totalBlocksAcceptedByNode, [int]$s.totalBlocks, $hr, $stale, $inv, $weak, $workers)
    } else {
      Write-Output ("solo_ok {0} w={1} blk={2} hr={3:N0}ghs" -f $ts, [int]$s.activeWorkers, [int]$s.totalBlocksAcceptedByNode, $hr)
    }
  } catch {
    Write-Output ("AGENT_SOLO_ERR ts={0} {1}" -f $ts, $_.Exception.Message)
  }
  Start-Sleep -Seconds 60
}
