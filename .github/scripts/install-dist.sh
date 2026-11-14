#!/usr/bin/env bash
set -euo pipefail

version="0.34.0"
triple="${1:?usage: install-dist.sh <host-triple>}"

case "$triple" in
  aarch64-apple-darwin) expected=d1eec30e63eed23e5a5bd48fe96eae314799811bffca254ec1d2264638ba1890 ;;
  aarch64-unknown-linux-gnu) expected=58efb33edf01342875bfd0c3ce2befa9d73075579ca1b3e5dd75b98c88f92e83 ;;
  aarch64-unknown-linux-musl) expected=aac7d48901f5283b53e136dd0498782c88a3350dfa4a5e88f4c3e0ba7e486eea ;;
  x86_64-apple-darwin) expected=6aa89bb1e7c18f51ae8b148ce2fc6ab00ac46ed8173d4f2ed9afa12101218773 ;;
  x86_64-pc-windows-msvc) expected=7bb53fbc998321f77a3bbb044084a52747c8ed041cf6d6a604802aa57b9c80c6 ;;
  x86_64-unknown-linux-gnu) expected=7fee44ff3ff71ed0064b32d4ec62fee6b58bd37c5887b12a86aedddc5af0c7b5 ;;
  x86_64-unknown-linux-musl) expected=f274b9889e93333a52cdcab86b93ab04b875b8ef4eebc7f8589514e0aff827e6 ;;
  *) echo "install-dist: no pinned hash for $triple" >&2; exit 1 ;;
esac

config="$(cd "$(dirname "$0")/../.." && pwd)/dist-workspace.toml"
wanted="$(sed -n 's/^cargo-dist-version = "\(.*\)"$/\1/p' "$config")"
if [ "$wanted" != "$version" ]; then
  echo "install-dist: dist-workspace.toml wants dist $wanted, this script pins $version; update its version and hashes" >&2
  exit 1
fi

case "$triple" in
  *windows*) archive="cargo-dist-$triple.zip"; exe="dist.exe" ;;
  *) archive="cargo-dist-$triple.tar.xz"; exe="dist" ;;
esac

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$work/$archive" \
  "https://github.com/axodotdev/cargo-dist/releases/download/v$version/$archive"

if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$work/$archive" | cut -d' ' -f1)"
elif command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "$work/$archive" | cut -d' ' -f1)"
else
  echo "install-dist: neither sha256sum nor shasum is available" >&2
  exit 1
fi
if [ "$actual" != "$expected" ]; then
  echo "install-dist: $archive has SHA-256 $actual, expected $expected" >&2
  exit 1
fi

mkdir -p "$work/out"
case "$archive" in
  *.zip)
    if command -v unzip >/dev/null 2>&1; then
      unzip -q "$work/$archive" -d "$work/out"
    else
      powershell -NoProfile -Command "Expand-Archive -LiteralPath '$(cygpath -w "$work/$archive")' -DestinationPath '$(cygpath -w "$work/out")'"
    fi
    ;;
  *) tar -xJf "$work/$archive" -C "$work/out" ;;
esac

found="$(find "$work/out" -type f -name "$exe" | head -n 1)"
if [ -z "$found" ]; then
  echo "install-dist: $archive holds no $exe" >&2
  exit 1
fi
bin="${CARGO_HOME:-$HOME/.cargo}/bin"
mkdir -p "$bin"
cp "$found" "$bin/$exe"
chmod 755 "$bin/$exe"
"$bin/$exe" --version
