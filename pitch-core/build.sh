#!/usr/bin/env bash
# Build pitch-core and install the compiled module where the page loads it:
# ../assets/pitch-core.wasm
#
# Nothing else builds this crate — not the Muse artifact build and not the
# GitHub Pages deploy Action. Both ship the prebuilt .wasm as-is, so after
# any change to src/lib.rs you must run this script and commit the refreshed
# ../assets/pitch-core.wasm, or the change has no effect.
#
# One-time prerequisite: the wasm32 target for your Rust toolchain
#   rustup target add wasm32-unknown-unknown
#
# Usage: ./build.sh   (paths resolve relative to this script, so it can be
# run from any directory)

set -euo pipefail

cd "$(dirname "$0")"

if command -v rustup >/dev/null 2>&1; then
  if ! rustup target list --installed | grep -qx 'wasm32-unknown-unknown'; then
    echo "error: the wasm32-unknown-unknown target is not installed." >&2
    echo "Install it once with:  rustup target add wasm32-unknown-unknown" >&2
    exit 1
  fi
fi

cargo build --release --target wasm32-unknown-unknown

cp target/wasm32-unknown-unknown/release/pitch_core.wasm ../assets/pitch-core.wasm

echo "Installed ../assets/pitch-core.wasm ($(wc -c < ../assets/pitch-core.wasm) bytes)"
echo "Reminder: reload the page and confirm it is still using the WASM detector"
echo "(exports: alloc, dealloc, detect_pitch, last_clarity) — if any export is"
echo "missing, the page silently falls back to its JavaScript jsDetect()."
