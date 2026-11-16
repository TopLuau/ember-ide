#!/usr/bin/env sh
# Ember IDE installer for Linux (x86_64, aarch64/Raspberry Pi) and macOS (Intel, Apple Silicon).
#   curl -fsSL https://your-site/install.sh | EMBER_SITE=https://your-site sh
# Without EMBER_SITE it downloads from GitHub releases. Installs to ~/.local (no sudo).
# Pin a version with EMBER_VERSION=0.1.0 (GitHub mode).
set -eu

REPO="${EMBER_REPO:-topluau/ember-ide}"
BASE="${EMBER_BASE:-}"
if [ -z "$BASE" ] && [ -n "${EMBER_SITE:-}" ]; then BASE="$EMBER_SITE/downloads"; fi
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)  os=linux ;;
  Darwin) os=macos ;;
  *) echo "Unsupported OS: $OS (on Windows use install.ps1 or the Setup .exe)"; exit 1 ;;
esac
case "$ARCH" in
  x86_64|amd64) arch=x86_64 ;;
  aarch64|arm64) arch=aarch64 ;;
  *) echo "Unsupported CPU: $ARCH (32-bit ARM: build from source with cargo)"; exit 1 ;;
esac

if [ -n "$BASE" ]; then
  ver="$(curl -fsSL "$BASE/manifest.json" | sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' | head -n1)"
elif [ -n "${EMBER_VERSION:-}" ]; then
  ver="${EMBER_VERSION#v}"
else
  ver="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | sed -n 's/.*"tag_name": *"v\{0,1\}\([^"]*\)".*/\1/p' | head -n1)"
fi
[ -n "$ver" ] || { echo "Could not find the latest version"; exit 1; }

file="Ember-IDE-${ver}-${os}-${arch}.tar.gz"
if [ -n "$BASE" ]; then url="$BASE/$file"; else url="https://github.com/$REPO/releases/download/v$ver/$file"; fi
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "Downloading $file ..."
curl -fL "$url" -o "$tmp/$file"
tar -xzf "$tmp/$file" -C "$tmp"

prefix="${PREFIX:-$HOME/.local}"
mkdir -p "$prefix/bin" "$prefix/share/ember"
cp "$tmp/ember-ide/ember" "$prefix/bin/ember"
chmod +x "$prefix/bin/ember"
rm -rf "$prefix/share/ember/plugins"
cp -R "$tmp/ember-ide/plugins" "$prefix/share/ember/plugins"

if [ "$os" = linux ]; then
  mkdir -p "$prefix/share/applications" "$prefix/share/icons/hicolor/256x256/apps"
  cp "$tmp/ember-ide/icon.png" "$prefix/share/icons/hicolor/256x256/apps/ember.png"
  sed "s|^Exec=.*|Exec=$prefix/bin/ember %F|" "$tmp/ember-ide/ember.desktop" > "$prefix/share/applications/ember.desktop"
fi

echo "Installed Ember IDE $ver to $prefix/bin/ember"
case ":$PATH:" in *":$prefix/bin:"*) ;; *) echo "Add $prefix/bin to your PATH to run 'ember'." ;; esac
