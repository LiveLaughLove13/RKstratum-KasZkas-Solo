RKstratum Kas + ZKAS Solo (click-run)
=====================================

One program that:
  - Starts an embedded Kaspa node
  - Starts an embedded ZKAS node
  - Opens a local solo Stratum for your ASICs
  - Permanently keeps a 1% hosting fee (cannot be changed in any config file)

REQUIREMENTS
------------
- SSD with substantial free space (two full-node datadirs)
- First run will sync Kaspa + ZKAS — this can take a long time
- Allow inbound TCP 7666 in the firewall if ASICs are on another machine

WINDOWS
-------
1. Unzip this folder anywhere.
2. Double-click RKstratumKasZkasSolo.exe
3. Leave the window open. Wait until you see "NODES READY".
4. Point your ASIC / miner software at:

     stratum+tcp://YOUR-PC-LAN-IP:7666

   Username: your kaspa: address (optionally .workerName)
   Password: your zkas: address (recommended for ZKAS payouts), or "x"

5. Local dashboard: http://127.0.0.1:3133/

Data: %LOCALAPPDATA%\RKstratumKasZkasSolo\

LINUX
-----
1. Extract the tar.gz.
2. Run:  ./run.sh    (or ./RKstratumKasZkasSolo)
3. Leave the terminal open. Wait until you see "NODES READY".
4. Same ASIC URL / dashboard as above.

Data: ~/.RKstratumKasZkasSolo/

FEE
---
Hosting fee is hardcoded at 1% of job templates (probabilistic over many blocks).
Ops receive addresses are built into the binary. Editing runtime.yaml cannot
disable or redirect the fee.

SUPPORT
-------
Private RKstratum distribution. Do not expect zero-fee operation from this build.
