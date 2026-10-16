#!/bin/sh
# Download the latest *binary* package from GitHub Releases.
# Linux: tar.gz (x86_64 or aarch64). macOS: zip (arm64 or x86_64).
# POSIX sh so `curl ... | sh` works (Ubuntu sh is dash; shebang is ignored on a pipe).
# Never clones the repo. Never fetches GitHub "Source code" archives.
set -eu

REPO="LiveLaughLove13/RKstratum-KasZkas-Solo"
DEST="${RKSTRATUM_INSTALL_DIR:-$HOME/RKstratum-KasZkas-Solo}"

die() { echo "install: $*" >&2; exit 1; }

need() {
  command -v "$1" >/dev/null 2>&1 || die "missing dependency: $1"
}

need curl

os="$(uname -s)"
arch="$(uname -m)"
case "$os:$arch" in
  Linux:x86_64|Linux:amd64)
    ASSET_PREFIX="RKstratum-KasZkas-Solo-linux-x64-"
    PKG_EXT="tar.gz"
    ;;
  Linux:aarch64|Linux:arm64)
    ASSET_PREFIX="RKstratum-KasZkas-Solo-linux-arm64-"
    PKG_EXT="tar.gz"
    ;;
  Darwin:arm64)
    ASSET_PREFIX="RKstratum-KasZkas-Solo-macos-arm64-"
    PKG_EXT="zip"
    ;;
  Darwin:x86_64)
    ASSET_PREFIX="RKstratum-KasZkas-Solo-macos-x64-"
    PKG_EXT="zip"
    ;;
  *)
    die "Linux x86_64/aarch64 or macOS arm64/x86_64 only (got ${os}/${arch}). Windows: download the zip from Releases."
    ;;
esac

if [ "$PKG_EXT" = "tar.gz" ]; then
  need tar
else
  need unzip
fi

api_url="https://api.github.com/repos/${REPO}/releases/latest"
api_json="$(curl -fsSL --proto '=https' --tlsv1.2 -H 'Accept: application/vnd.github+json' "$api_url")" \
  || die "failed to read ${api_url}"

tag="$(printf '%s\n' "$api_json" | tr ',' '\n' | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
echo "$tag" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$' \
  || die "could not parse latest tag (got '${tag:-empty}')"

pkg_name="${ASSET_PREFIX}${tag}.${PKG_EXT}"
sha_name="${ASSET_PREFIX}${tag}.sha256"
echo "$pkg_name" | grep -Eq '^RKstratum-KasZkas-Solo-(linux-(x64|arm64)|macos-(arm64|x64))-v[0-9]+\.[0-9]+\.[0-9]+\.(tar\.gz|zip)$' \
  || die "refusing unexpected package name: $pkg_name"
echo "$sha_name" | grep -Eq '^RKstratum-KasZkas-Solo-(linux-(x64|arm64)|macos-(arm64|x64))-v[0-9]+\.[0-9]+\.[0-9]+\.sha256$' \
  || die "refusing unexpected checksum name: $sha_name"

base="https://github.com/${REPO}/releases/download/${tag}"
pkg_url="${base}/${pkg_name}"
sha_url="${base}/${sha_name}"

tmp="$(mktemp -d)"
cleanup() { rm -rf "$tmp"; }
trap cleanup EXIT

echo "install: fetching ${tag} ${os} ${arch} binary package (not source)"
curl -fsSL --proto '=https' --tlsv1.2 -o "${tmp}/${pkg_name}" "$pkg_url" \
  || die "download failed: $pkg_url (no ${ASSET_PREFIX} asset on this release yet?)"
curl -fsSL --proto '=https' --tlsv1.2 -o "${tmp}/${sha_name}" "$sha_url" \
  || die "download failed: $sha_url"

sha_line="$(tr -d '\r' < "${tmp}/${sha_name}" | head -n1)"
sha_hash="${sha_line%% *}"
sha_file="${sha_line##* }"
echo "$sha_hash" | grep -Eq '^[0-9a-fA-F]{64}$' || die "checksum file is not a SHA256"
[ "$sha_file" = "$pkg_name" ] || die "checksum names '${sha_file}', expected '${pkg_name}'"

if command -v sha256sum >/dev/null 2>&1; then
  (
    cd "$tmp"
    printf '%s  %s\n' "$sha_hash" "$pkg_name" | sha256sum -c -
  ) || die "SHA256 mismatch - refusing to install"
else
  actual="$(shasum -a 256 "${tmp}/${pkg_name}" | awk '{print tolower($1)}')"
  [ "$actual" = "$(printf '%s' "$sha_hash" | tr 'A-F' 'a-f')" ] \
    || die "SHA256 mismatch - refusing to install"
fi

if [ "$PKG_EXT" = "tar.gz" ]; then
  tar -tzf "${tmp}/${pkg_name}" >"${tmp}/members" || die "tarball list failed"
else
  unzip -Z1 "${tmp}/${pkg_name}" >"${tmp}/members" || die "zip list failed"
fi
while IFS= read -r member; do
  case "$member" in
    /* | *..*) die "archive has unsafe path: $member" ;;
  esac
done <"${tmp}/members"

mkdir -p "$DEST"
if [ "$PKG_EXT" = "tar.gz" ]; then
  tar -xzf "${tmp}/${pkg_name}" -C "$DEST"
else
  unzip -qo "${tmp}/${pkg_name}" -d "$DEST"
fi
chmod +x "${DEST}/RKstratumKasZkasSolo" "${DEST}/run.sh" 2>/dev/null || true
[ -x "${DEST}/RKstratumKasZkasSolo" ] || die "missing binary after extract"
xattr -dr com.apple.quarantine "$DEST" >/dev/null 2>&1 || true

echo ""
echo "Installed ${tag} to ${DEST}"
echo "This folder is the binary package only (exe + run.sh + license)."
echo "Node data stays in ~/.RKstratumKasZkasSolo/ when you start it."
echo ""
echo "Start:"
echo "  cd \"${DEST}\" && ./run.sh"
echo ""
echo "Wait for NODES READY, then point ASICs at stratum+tcp://THIS-LAN-IP:7666"

if [ "${RKSTRATUM_RUN:-}" = "1" ]; then
  cd "$DEST"
  exec ./run.sh
fi
