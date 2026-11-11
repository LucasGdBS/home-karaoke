#!/usr/bin/env bash
# Downloads a static LGPL ffmpeg as the Tauri sidecar (src-tauri/binaries/ffmpeg-<target-triple>).
set -euo pipefail
cd "$(dirname "$0")/../src-tauri"
mkdir -p binaries
triple=$(rustc -vV | sed -n 's/^host: //p')
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fL https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-lgpl.tar.xz | tar -xJ -C "$tmp"
cp "$tmp"/*/bin/ffmpeg "binaries/ffmpeg-$triple"
chmod +x "binaries/ffmpeg-$triple"
echo "ok: src-tauri/binaries/ffmpeg-$triple"
