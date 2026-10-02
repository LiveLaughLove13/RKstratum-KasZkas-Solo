# RKstratum Kas + ZKAS Solo

**Binary releases only** — click-and-run solo stratum for Kaspa + ZKAS merge-mining.

One program embeds Kaspa + ZKAS nodes, opens a local Stratum for your ASICs, and keeps a **permanent 1% hosting fee** (not configurable).

| | |
|---|---|
| **Latest** | [v1.0.6](../../releases/tag/v1.0.6) |
| Stratum | `stratum+tcp://YOUR-PC-LAN-IP:7666` |
| Dashboard | `http://127.0.0.1:3133/` |

## Download

Get assets from **[Releases](../../releases)**:

| Platform | Asset |
|----------|--------|
| Windows x64 | `RKstratum-KasZkas-Solo-windows-x64-v1.0.6.zip` |
| Linux x64 | `RKstratum-KasZkas-Solo-linux-x64-v1.0.6.tar.gz` |

Each asset has a matching `*.sha256` checksum — verify before running.

### Verify checksums

**Windows (PowerShell):**

```powershell
Get-FileHash .\RKstratum-KasZkas-Solo-windows-x64-v1.0.6.zip -Algorithm SHA256
Get-Content .\RKstratum-KasZkas-Solo-windows-x64-v1.0.6.sha256
```

**Linux:**

```bash
sha256sum -c RKstratum-KasZkas-Solo-linux-x64-v1.0.6.sha256
```

## Windows

1. Unzip the Windows zip.
2. Double-click `RKstratumKasZkasSolo.exe`.
3. Wait for **NODES READY**.
4. Point ASICs at `stratum+tcp://YOUR-LAN-IP:7666`.

   Username: `kaspa:YOUR_ADDRESS`, your DotK name (`dablacksplash` / `dablacksplash.rig1`), or your KNS name (`dablacksplash.kas` / `dablacksplash.kas.rig1`). Bare names are DotK only. Password: your `zkas:` address, or `x`.

Data: `%LOCALAPPDATA%\RKstratumKasZkasSolo\`

## Linux

1. Extract the tar.gz.
2. Open a terminal in that folder and run `./run.sh` (or `./RKstratumKasZkasSolo`). Keep the terminal open.
3. Wait for **NODES READY**.
4. Same ASIC URL / dashboard as above.

Data: `~/.RKstratumKasZkasSolo/`

## Notes

- First sync of Kaspa + ZKAS needs substantial SSD space and time.
- To open the dashboard from another PC: `--web-dashboard-port 0.0.0.0:3133` then `http://THIS-MACHINE-LAN-IP:3133/`.
- Allow inbound TCP **7666** (stratum) and **16811** (ZKAS P2P) if needed.
- Do not run this at the same time as another local Kaspa/ZKAS node on the default RPC ports.
- Hosting fee is hardcoded at 1%. Editing any local config cannot disable it.
- **This repository does not include source code** — binaries and docs only.
