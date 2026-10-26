#!/usr/bin/env bash
# Runs inside a rust:1-bookworm container: /src is the repo (read-only), /out receives the bundles.
set -euo pipefail

apt-get update -qq
apt-get install -y -qq libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev rpm xdg-utils >/dev/null
curl -fsSL https://deb.nodesource.com/setup_22.x | bash - >/dev/null
apt-get install -y -qq nodejs >/dev/null

# Copy without the host's node_modules/target: they hold macOS binaries.
mkdir -p /work
tar -C /src --exclude=node_modules --exclude=src-tauri/target --exclude=src-tauri/gen --exclude=dist -cf - . | tar -xf - -C /work
cd /work
npm ci --silent
# AppImage tooling needs FUSE unless told to extract itself.
APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=true npx tauri build
cp src-tauri/target/release/bundle/deb/*.deb src-tauri/target/release/bundle/rpm/*.rpm \
  src-tauri/target/release/bundle/appimage/*.AppImage /out/
