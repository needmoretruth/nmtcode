#!/usr/bin/env bash
# Builds the browser version of NMT Code into web/pkg:
#
#   1. the nmtcode-wasm crate for wasm32-unknown-unknown in release mode
#   2. its JavaScript bindings with wasm-bindgen (--target web), which must be version 0.2.129,
#      the version of the wasm-bindgen crate in crates/nmtcode-wasm/Cargo.toml
#
# and prints the size of the .wasm file with and without gzip. wasm-opt is not used.
# The script holds the machine-wide lock for heavy commands, so run it without flock.

set -euo pipefail

lock=/tmp/big-heavy.lock
# scripts/check.sh sets this variable when it already holds the lock.
if [[ "${NMTCODE_CHECK_HOLDS_LOCK:-}" != 1 ]]; then
  exec env NMTCODE_CHECK_HOLDS_LOCK=1 flock -w 3600 "$lock" "$0" "$@"
fi

cd "$(dirname "$0")/.."

bindgen_version=0.2.129
if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "wasm-bindgen is not installed. Install it with:" >&2
  echo "  cargo install wasm-bindgen-cli --version $bindgen_version --locked" >&2
  exit 1
fi
installed=$(wasm-bindgen --version | awk '{ print $2 }')
if [[ "$installed" != "$bindgen_version" ]]; then
  echo "wasm-bindgen $installed is installed, but $bindgen_version is needed. Install it with:" >&2
  echo "  cargo install wasm-bindgen-cli --version $bindgen_version --locked --force" >&2
  exit 1
fi

target=wasm32-unknown-unknown
cargo build -p nmtcode-wasm --target "$target" --release --locked

wasm=${CARGO_TARGET_DIR:-target}/$target/release/nmtcode_wasm.wasm
out=web/pkg
rm -rf "$out"
# The name and producers sections only help debugging; the page does not need them.
wasm-bindgen --target web --no-typescript --remove-name-section --remove-producers-section \
  --out-dir "$out" "$wasm"

built=$out/nmtcode_wasm_bg.wasm
bytes=$(wc -c <"$built")
gzipped=$(gzip -9 -c "$built" | wc -c)
printf '%s: %d bytes, %d bytes with gzip -9\n' "$built" "$bytes" "$gzipped"
