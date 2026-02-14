#!/usr/bin/env bash
set -euo pipefail

die() {
  echo "lstui install: $*" >&2
  exit 1
}

need() {
  command -v "$1" >/dev/null 2>&1 || die "missing command: $1"
}

usage() {
  cat <<'EOF'
Usage: install.sh [--version X|vX] [--repo OWNER/REPO] [--to DIR]

Defaults:
  repo: rocrp/lstui
  to:   ~/.local/bin
  version: latest GitHub release
EOF
}

repo="${LSTUI_REPO:-rocrp/lstui}"
install_dir="${INSTALL_DIR:-$HOME/.local/bin}"
version="${LSTUI_VERSION:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)
      repo="${2:-}"; shift 2 || true
      [[ -n "$repo" ]] || die "--repo requires value"
      ;;
    --to)
      install_dir="${2:-}"; shift 2 || true
      [[ -n "$install_dir" ]] || die "--to requires value"
      ;;
    --version)
      version="${2:-}"; shift 2 || true
      [[ -n "$version" ]] || die "--version requires value"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown arg: $1"
      ;;
  esac
done

need curl
need tar
need uname
need mktemp

os="$(uname -s)"
case "$os" in
  Darwin) os="darwin" ;;
  Linux) os="linux" ;;
  *) die "unsupported OS: $(uname -s)" ;;
esac

arch="$(uname -m)"
case "$arch" in
  x86_64|amd64) arch="amd64" ;;
  arm64|aarch64) arch="arm64" ;;
  *) die "unsupported arch: $(uname -m)" ;;
esac

tag=""
if [[ -n "$version" ]]; then
  if [[ "$version" == v* ]]; then
    tag="$version"
  else
    tag="v$version"
  fi
else
  api="https://api.github.com/repos/${repo}/releases/latest"
  json="$(curl -fsSL "$api")" || die "failed fetching latest release: $api"
  tag="$(printf '%s' "$json" | awk -F'"' '/"tag_name"[[:space:]]*:/{print $4; exit}')"
fi
[[ -n "$tag" ]] || die "could not resolve release tag (try --version)"

asset="lstui-${tag}-${os}-${arch}.tar.gz"
url="https://github.com/${repo}/releases/download/${tag}/${asset}"

tmp="$(mktemp -d)"
cleanup() { rm -rf "$tmp"; }
trap cleanup EXIT

curl -fL "$url" -o "$tmp/$asset" || die "download failed: $url"
tar -xzf "$tmp/$asset" -C "$tmp" || die "extract failed: $asset"

bin="$tmp/lstui"
[[ -f "$bin" ]] || die "archive missing lstui"
chmod +x "$bin"

mkdir -p "$install_dir" || true
dest="${install_dir%/}/lstui"

if [[ -w "$install_dir" ]]; then
  mv "$bin" "$dest"
else
  need sudo
  sudo mkdir -p "$install_dir"
  sudo mv "$bin" "$dest"
fi

echo "installed: $dest"

