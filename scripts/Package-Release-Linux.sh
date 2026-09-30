#!/usr/bin/env bash
# Build + package Linux x86_64 tarball + SHA256 for RKstratum Kas+ZKAS Solo.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="${1:-1.0.0}"
TARGET_DIR="${TARGET_DIR:-$ROOT/target-linux}"

bash "$ROOT/scripts/Build-Linux.sh"

DIST="$ROOT/dist"
STAGE="$DIST/stage-RKstratum-KasZkas-Solo-linux"
rm -rf "$STAGE"
mkdir -p "$STAGE" "$DIST"

cp -f "$TARGET_DIR/release/RKstratumKasZkasSolo" "$STAGE/RKstratumKasZkasSolo"
chmod +x "$STAGE/RKstratumKasZkasSolo"
cp -f "$ROOT/README.txt" "$STAGE/"
cp -f "$ROOT/LICENSE" "$STAGE/" 2>/dev/null || true

cat > "$STAGE/run.sh" <<'EOF'
#!/usr/bin/env bash
cd "$(dirname "$0")"
exec ./RKstratumKasZkasSolo "$@"
EOF
chmod +x "$STAGE/run.sh"

TAR="RKstratum-KasZkas-Solo-linux-x64-v${VERSION}.tar.gz"
rm -f "$DIST/$TAR"
tar -czf "$DIST/$TAR" -C "$STAGE" .
if command -v sha256sum >/dev/null 2>&1; then
  HASH="$(sha256sum "$DIST/$TAR" | awk '{print tolower($1)}')"
else
  HASH="$(shasum -a 256 "$DIST/$TAR" | awk '{print tolower($1)}')"
fi
echo "$HASH  $TAR" > "$DIST/RKstratum-KasZkas-Solo-linux-x64-v${VERSION}.sha256"
echo "Package: $DIST/$TAR"
echo "SHA256:  $HASH"
