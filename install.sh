#!/bin/sh
# Install a verified prebuilt release. No sudo, Python or nsh required.
set -eu
repo=huacnlee/sysmon
version=${1:-${SYSMON_VERSION:-latest}}
case $(uname -s) in Darwin) platform=apple-darwin;; Linux) platform=unknown-linux-gnu;; *) echo 'Supported platforms: macOS and Linux' >&2; exit 1;; esac
case $(uname -m) in arm64|aarch64) arch=aarch64;; x86_64|amd64) arch=x86_64;; *) echo 'Unsupported CPU architecture' >&2; exit 1;; esac
fetch() { curl --proto '=https' --tlsv1.2 -fsSL --retry 3 "$@"; }
if [ "$version" = latest ]; then
  location=$(fetch -o /dev/null -w '%{url_effective}' "https://github.com/$repo/releases/latest")
  version=${location##*/}
fi
case "$version" in v[0-9]*) ;; [0-9]*) version=v$version;; *) echo 'Expected a release version such as v0.1.0' >&2; exit 1;; esac
case "$version" in *[!a-zA-Z0-9._-]*) echo 'Invalid version' >&2; exit 1;; esac
archive=sysmon-$version-$arch-$platform.tar.gz
base=https://github.com/$repo/releases/download/$version
scratch=$(mktemp -d)
release=
activated=no
cleanup() {
  status=$?
  trap - EXIT HUP INT TERM
  if [ -n "$release" ] && [ "$activated" = no ]; then
    rm -rf -- "$release"
  fi
  rm -rf -- "$scratch"
  exit "$status"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
fetch "$base/$archive" -o "$scratch/$archive"
fetch "$base/SHA256SUMS" -o "$scratch/SHA256SUMS"
expected=$(awk -v file="$archive" '$2 == file {print $1}' "$scratch/SHA256SUMS")
[ "${#expected}" -eq 64 ] || { echo 'Release checksum is missing' >&2; exit 1; }
if command -v sha256sum >/dev/null 2>&1; then
  actual=$(sha256sum "$scratch/$archive" | awk '{print $1}')
else
  actual=$(shasum -a 256 "$scratch/$archive" | awk '{print $1}')
fi
[ "$expected" = "$actual" ] || { echo 'Checksum verification failed' >&2; exit 1; }
data_home=${XDG_DATA_HOME:-$HOME/.local/share}
install_root=${SYSMON_INSTALL_DIR:-$data_home/sysmon}
bin_dir=${SYSMON_BIN_DIR:-$HOME/.local/bin}
mkdir -p "$install_root" "$bin_dir"
install_root=$(CDPATH= cd -- "$install_root" && pwd -P)
release=$(mktemp -d "$install_root/$version-$arch-$platform.XXXXXX")
tar -xzf "$scratch/$archive" -C "$release"
[ -x "$release/sysmon" ] || { echo 'Release has no sysmon executable' >&2; exit 1; }
# Keep a validated release if activation fails partway through: an entry point
# may already reference it. Historical versions are pruned only after success.
activated=yes
ln -sfn "$release/sysmon" "$bin_dir/sysmon"
if [ "$platform" = apple-darwin ] && [ -d "$release/sysmon.app" ]; then
  apps=${SYSMON_APPS_DIR:-$HOME/Applications}
  mkdir -p "$apps"
  ln -sfn "$release/sysmon.app" "$apps/sysmon.app"
elif [ -f "$release/sysmon.desktop" ]; then
  applications=${XDG_DATA_HOME:-$HOME/.local/share}/applications
  mkdir -p "$applications"
  icons=${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps
  mkdir -p "$icons"
  cp "$release/resources/icon.svg" "$icons/sysmon.svg"
  # Quote the executable path so home directories containing spaces work.
  escaped=$(printf '%s' "$release/sysmon" | sed 's/\\/\\\\/g; s/"/\\"/g; s/`/\\`/g; s/\$/\\$/g')
  sed "s|^Exec=.*|Exec=\"$escaped\"|" "$release/sysmon.desktop" > "$applications/sysmon.desktop"
fi
# Remove only directories created by this installer, across supported targets.
# Never follow symlinks or remove unrelated content in a custom install root.
prune_versions() {
  [ -d "$1" ] || return 0
  root=$(CDPATH= cd -- "$1" && pwd -P)
  for old in "$root"/v*; do
    [ -d "$old" ] && [ ! -L "$old" ] && [ "$old" != "$release" ] || continue
    case "${old##*/}" in
      v[0-9]*-aarch64-apple-darwin.??????|v[0-9]*-x86_64-apple-darwin.??????|v[0-9]*-aarch64-unknown-linux-gnu.??????|v[0-9]*-x86_64-unknown-linux-gnu.??????)
        rm -rf -- "$old";;
    esac
  done
}
prune_versions "$install_root"
if [ -z "${SYSMON_INSTALL_DIR:-}" ]; then
  legacy=$data_home/nsh-sysmon
  if [ -d "$legacy" ] && [ ! -L "$legacy" ]; then
    prune_versions "$legacy"
    if [ -f "$legacy/.DS_Store" ] && [ ! -L "$legacy/.DS_Store" ]; then
      rm -f -- "$legacy/.DS_Store"
    fi
    rmdir "$legacy" 2>/dev/null || :
  fi
fi
printf 'Installed sysmon %s to %s\n' "$version" "$bin_dir/sysmon"
case :${PATH:-}: in *:"$bin_dir":*) ;; *) printf 'Add %s to your PATH, then run sysmon.\n' "$bin_dir";; esac
