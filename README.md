# RKstratum Kas + ZKAS Solo

Private click-and-run **solo** stratum for Kaspa + ZKAS merge-mining.

- **One binary:** embeds Kaspa + ZKAS in-process (`--node-mode inprocess` only)
- **ASICs** connect to `stratum+tcp://<LAN-IP>:7666` (dashboard `:3133`, prom `:3266` — avoids dual-external `:6666` / `:3130` / `:2166`)
- **Hosting fee:** permanently **1%** (compile-time; YAML cannot change it)
- **No user `config.yaml`:** first run writes `%LOCALAPPDATA%\RKstratumKasZkasSolo\runtime.yaml` (Windows) or `~/.RKstratumKasZkasSolo/runtime.yaml` (Linux)

Forked from the dual-external solo bridge. **Never push** this tree to firecash or public upstreams.

## Build both platforms (from Windows + WSL)

```powershell
cd "D:\GITHUB Projects\RKstratum-KasZkas-Solo"
.\scripts\Package-Release.ps1 -Version 1.0.0
```

Produces under `dist\`:

| Artifact | Contents |
|----------|----------|
| `RKstratum-KasZkas-Solo-windows-x64-v*.zip` | `RKstratumKasZkasSolo.exe` + `README.txt` + `LICENSE` |
| `RKstratum-KasZkas-Solo-linux-x64-v*.tar.gz` | `RKstratumKasZkasSolo` + `run.sh` + `README.txt` + `LICENSE` |
| matching `.sha256` files | |

Flags: `-SkipLinux` / `-SkipWindows` if you only want one OS.

Linux alone (native Linux or WSL shell):

```bash
./scripts/Package-Release-Linux.sh 1.0.0
```

Windows alone:

```powershell
.\scripts\Build.ps1
.\scripts\Package-Release.ps1 -SkipLinux
```

## Dev run

```powershell
cargo run -p kaspa-stratum-bridge --release --bin RKstratumKasZkasSolo
```

## Fee lock tests

```powershell
cargo test -p kaspa-stratum-bridge solo_fee --lib
cargo test -p kaspa-stratum-bridge product_runtime --lib
```
