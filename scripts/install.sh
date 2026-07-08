#!/usr/bin/env bash
# Build (if `--latest` or no bundle exists yet) and install AnyLeft into
# /Applications. Strips the Gatekeeper quarantine xattr so the ad-hoc-signed
# dev bundle can launch without the "is damaged" prompt on first open.
#
# Usage:
#   pnpm app:install         install an already-built bundle
#   pnpm app:install --latest  rebuild then install

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: app:install is macOS-only (this app is a menu-bar accessory)." >&2
  exit 1
fi

APP_NAME="AnyLeft"
APP_BUNDLE="src-tauri/target/release/bundle/macos/${APP_NAME}.app"
INSTALL_PATH="/Applications/${APP_NAME}.app"

# `--latest` always rebuilds; otherwise build only when there's no bundle yet
# (so a fresh checkout still works). `tauri build` is incremental, so a
# no-op rebuild is cheap when nothing changed.
if [[ "$*" == *--latest* || ! -d "$APP_BUNDLE" ]]; then
  echo ">> pnpm app:build"
  pnpm app:build
fi

if [[ ! -d "$APP_BUNDLE" ]]; then
  echo "error: build succeeded but ${APP_BUNDLE} is missing." >&2
  exit 1
fi

# A running instance locks its bundle; kill it before overwriting.
if pgrep -x anyleft >/dev/null 2>&1; then
  echo ">> stopping running ${APP_NAME}"
  pkill -x anyleft || true
  for _ in 1 2 3 4 5; do
    pgrep -x anyleft >/dev/null 2>&1 || break
    sleep 0.2
  done
fi

if [[ -d "$INSTALL_PATH" ]]; then
  echo ">> removing existing ${INSTALL_PATH}"
  rm -rf "$INSTALL_PATH"
fi

echo ">> installing to ${INSTALL_PATH}"
cp -R "$APP_BUNDLE" /Applications/

# Gatekeeper quarantines ad-hoc-signed bundles on first open; strip it so the
# app launches normally. Fine for local dev — not a substitute for notarization.
echo ">> stripping quarantine xattr"
xattr -dr com.apple.quarantine "$INSTALL_PATH" 2>/dev/null || true

echo "ok installed ${APP_NAME}"
echo "   launch:  open ${INSTALL_PATH}"
