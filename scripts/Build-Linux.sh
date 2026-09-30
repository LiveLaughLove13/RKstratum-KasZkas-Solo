#!/usr/bin/env bash
# Release-build RKstratumKasZkasSolo for Linux x86_64 (embedded Kaspa + ZKAS).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="${TARGET_DIR:-$ROOT/target-linux}"

need() { command -v "$1" >/dev/null 2>&1 || { echo "missing dependency: $1" >&2; exit 1; }; }
need cargo
need g++

# Prefer a recent linker; rustc will use the system default.
export CARGO_TARGET_DIR="$TARGET_DIR"

cd "$ROOT"
echo "Building RKstratumKasZkasSolo (Linux release)..."
cargo build -p kaspa-stratum-bridge --release --bin RKstratumKasZkasSolo

EXE="$TARGET_DIR/release/RKstratumKasZkasSolo"
DEPS="$TARGET_DIR/release/deps/RKstratumKasZkasSolo"
if [[ -f "$DEPS" ]] && { [[ ! -f "$EXE" ]] || [[ "$DEPS" -nt "$EXE" ]]; }; then
  cp -f "$DEPS" "$EXE" || true
fi
[[ -x "$EXE" ]] || { echo "missing $EXE" >&2; exit 1; }
chmod +x "$EXE"
echo "OK: $EXE ($(stat -c%s "$EXE" 2>/dev/null || wc -c <"$EXE") bytes)"
