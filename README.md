# RKstratum Kas + ZKAS Solo

**Binary releases only** — click-and-run solo stratum for Kaspa + ZKAS merge-mining.

One program embeds Kaspa + ZKAS nodes, opens a local Stratum for your ASICs, and keeps a **permanent 1% hosting fee** (not configurable).

| | |
|---|---|
| **Latest** | [v2.1.4](../../releases/tag/v2.1.4) |
| Stratum | `stratum+tcp://YOUR-PC-LAN-IP:7666` |
| Dashboard | `http://127.0.0.1:3133/` |

## Linux / macOS one-liner

Downloads the latest **binary** package + SHA256 from Releases (not source). Picks Linux x86_64, Linux ARM64, macOS Apple Silicon, or macOS Intel from `uname`.

```bash
curl -fsSL https://github.com/LiveLaughLove13/RKstratum-KasZkas-Solo/releases/latest/download/install.sh | sh
```

Then `cd ~/RKstratum-KasZkas-Solo && ./run.sh`. Override the folder with `RKSTRATUM_INSTALL_DIR=/path`.

## Download

Get assets from **[Releases](../../releases)**:

| Platform | Asset |
|----------|--------|
| Windows x64 | `RKstratum-KasZkas-Solo-windows-x64-v2.1.4.zip` |
| Linux x64 | `RKstratum-KasZkas-Solo-linux-x64-v2.1.4.tar.gz` |
| Linux ARM64 (Pi 4/5 64-bit, ARM SBCs) | `RKstratum-KasZkas-Solo-linux-arm64-v2.1.4.tar.gz` |
| macOS Apple Silicon | `RKstratum-KasZkas-Solo-macos-arm64-v2.1.4.zip` |
| macOS Intel | `RKstratum-KasZkas-Solo-macos-x64-v2.1.4.zip` |

Each asset has a matching `*.sha256` checksum — verify before running. macOS zips are on the release after that package is built on a Mac.

### Verify checksums

**Windows (PowerShell):**

```powershell
Get-FileHash .\RKstratum-KasZkas-Solo-windows-x64-v2.1.4.zip -Algorithm SHA256
Get-Content .\RKstratum-KasZkas-Solo-windows-x64-v2.1.4.sha256
```

**Linux:**

```bash
sha256sum -c RKstratum-KasZkas-Solo-linux-x64-v2.1.4.sha256
# or, on ARM64:
sha256sum -c RKstratum-KasZkas-Solo-linux-arm64-v2.1.4.sha256
```

## Windows

1. Unzip the Windows zip.
2. Double-click `RKstratumKasZkasSolo.exe`.
3. Wait for **NODES READY**.
4. Point ASICs at `stratum+tcp://YOUR-LAN-IP:7666`.

   Username: `kaspa:YOUR_ADDRESS`, your DotK name (`dablacksplash` / `dablacksplash.rig1`), or your KNS name (`dablacksplash.kas` / `dablacksplash.kas.rig1`). Bare names are DotK only. Password: your `zkas:` address, or `x`.

Data: `%LOCALAPPDATA%\RKstratumKasZkasSolo\`

## macOS

Use the one-liner above, or unzip the matching zip from Releases.

1. Apple Silicon = `macos-arm64`. Intel = `macos-x64`.
2. In Terminal: `cd` into that folder and run `./run.sh`.
3. Wait for **NODES READY**. If Gatekeeper blocks it, right-click the binary → Open, or run `xattr -dr com.apple.quarantine .` in that folder.

Data: `~/.RKstratumKasZkasSolo/`

## Linux

1. Extract the tar.gz.
2. Open a terminal in that folder and run `./run.sh` (or `./RKstratumKasZkasSolo`). Keep the terminal open.
3. Wait for **NODES READY**.
4. Same ASIC URL / dashboard as above.

Data: `~/.RKstratumKasZkasSolo/`

## Notes

- Raspberry Pi needs **64-bit** Raspberry Pi OS and preferably **8 GB RAM** (two full nodes). 32-bit Pi images are not supported.
- First sync of Kaspa + ZKAS needs substantial SSD space and time.
- To open the dashboard from another PC: `--web-dashboard-port 0.0.0.0:3133` then `http://THIS-MACHINE-LAN-IP:3133/`.
- Allow inbound TCP **7666** (stratum) and **16811** (ZKAS P2P) if needed.
- Do not run this at the same time as another local Kaspa/ZKAS node on the default RPC ports.
- Hosting fee is hardcoded at 1%. Editing any local config cannot disable it.
- v2.1.4 shows ZKas network hashrate and ZKas network difficulty on Overview and Node next to the Kaspa tiles. Values stay `-` until the ZKas DAG is synced enough for a reliable window.
- v2.1.4 still keeps IceRiver KS0 Ultra connected through cold-start: VarDiff does not step difficulty down until the worker has sent a first share. Other ASICs are unchanged.
- v2.1.4 still embeds [zkas-v1.0.11](https://github.com/firecash/zkas-rusty/releases/tag/zkas-v1.0.11). Merge mining stays `ZKMM` until ZKas DAA 6,983,500, then switches to [`ZKM1`](https://zkas.info/docs/mining/#zkm1). From that score, BlkZ counts the ZKAS header hash once the block is blue.
- **This repository does not include source code** — binaries and docs only.
