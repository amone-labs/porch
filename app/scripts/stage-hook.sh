#!/bin/sh
# Build porch-hook and the porch CLI and place them where Tauri's externalBin
# expects them (src-tauri/binaries/<name>-<target-triple>), so the app bundle
# carries the hook, can install it on its own, and can put `porch` on PATH.
set -e
cd "$(dirname "$0")/../.."
cargo build --release -p porch-hook -p porch-cli
TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
mkdir -p app/src-tauri/binaries
cp target/release/porch-hook "app/src-tauri/binaries/porch-hook-$TRIPLE"
cp target/release/porch "app/src-tauri/binaries/porch-$TRIPLE"
