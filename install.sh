#!/bin/sh
# Download the latest Linux x86_64 *binary* package from GitHub Releases.
# POSIX sh so `curl ... | sh` works (Ubuntu sh is dash; shebang is ignored on a pipe).
# Never clones the repo. Never fetches GitHub "Source code" archives.
set -eu

REPO="LiveLaughLove13/RKstratum-KasZkas-Solo"
# Only these two asset names are fetched (version filled from the latest tag).
# Do not add /archive/ or git clone - that is how source would leak.
ASSET_TAR_PREFIX="RKstratum-KasZkas-Solo-linux-x64-"
DEST="${RKSTRATUM_INSTALL_DIR:-$HOME/RKstratum-KasZkas-Solo}"

die() { echo "install: $*" >&2; exit 1; }

need() {
  command -v "$1" >/dev/null 2>&1 || die "missing dependency: $1"
}

need curl
need tar
need sha256sum

os="$(uname -s)"
arch="$(uname -m)"
if [ "$os" != "Linux" ] || { [ "$arch" != "x86_64" ] && [ "$arch" != "amd64" ]; }; then
  die "Linux x86_64 only (got ${os}/${arch}). Windows: download the zip from Releases."
fi

api_url="https://api.github.com/repos/${REPO}/releases/latest"
api_json="$(curl -fsSL --proto '=https' --tlsv1.2 -H 'Accept: application/vnd.github+json' "$api_url")" \
  || die "failed to read ${api_url}"

tag="$(printf '%s\n' "$api_json" | tr ',' '\n' | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
echo "$tag" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$' \
  || die "could not parse latest tag (got '${tag:-empty}')"

tar_name="${ASSET_TAR_PREFIX}${tag}.tar.gz"
sha_name="${ASSET_TAR_PREFIX}${tag}.sha256"
echo "$tar_name" | grep -Eq '^RKstratum-KasZkas-Solo-linux-x64-v[0-9]+\.[0-9]+\.[0-9]+\.tar\.gz$' \
  || die "refusing unexpected tarball name: $tar_name"
echo "$sha_name" | grep -Eq '^RKstratum-KasZkas-Solo-linux-x64-v[0-9]+\.[0-9]+\.[0-9]+\.sha256$' \
  || die "refusing unexpected checksum name: $sha_name"

base="https://github.com/${REPO}/releases/download/${tag}"
tar_url="${base}/${tar_name}"
sha_url="${base}/${sha_name}"

tmp="$(mktemp -d)"
cleanup() { rm -rf "$tmp"; }
trap cleanup EXIT

echo "install: fetching ${tag} Linux binary package (not source)"
curl -fsSL --proto '=https' --tlsv1.2 -o "${tmp}/${tar_name}" "$tar_url" \
  || die "download failed: $tar_url"
curl -fsSL --proto '=https' --tlsv1.2 -o "${tmp}/${sha_name}" "$sha_url" \
  || die "download failed: $sha_url"

# Checksum file must name only the tarball we requested.
sha_line="$(tr -d '\r' < "${tmp}/${sha_name}" | head -n1)"
sha_hash="${sha_line%% *}"
sha_file="${sha_line##* }"
echo "$sha_hash" | grep -Eq '^[0-9a-fA-F]{64}$' || die "checksum file is not a SHA256"
[ "$sha_file" = "$tar_name" ] || die "checksum names '${sha_file}', expected '${tar_name}'"

(
  cd "$tmp"
  printf '%s  %s\n' "$sha_hash" "$tar_name" | sha256sum -c -
) || die "SHA256 mismatch - refusing to install"

tar -tzf "${tmp}/${tar_name}" >"${tmp}/members" || die "tarball list failed"
while IFS= read -r member; do
  case "$member" in
    /* | *..*) die "tarball has unsafe path: $member" ;;
  esac
done <"${tmp}/members"

mkdir -p "$DEST"
tar -xzf "${tmp}/${tar_name}" -C "$DEST"
chmod +x "${DEST}/RKstratumKasZkasSolo" "${DEST}/run.sh" 2>/dev/null || true
[ -x "${DEST}/RKstratumKasZkasSolo" ] || die "missing binary after extract"

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
