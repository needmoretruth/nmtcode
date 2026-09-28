#!/usr/bin/env bash
# Every check of the workspace, in order; the first failure stops the script with its exit code.
#
#   1. cargo fmt --all --check
#   2. cargo clippy --workspace --all-targets -- -D warnings
#   3. cargo test --workspace
#   4. the no_std layer crates for wasm32-unknown-unknown, when that target is installed
#   5. no Cargo.toml declares a version of 1.0.0 or above
#
# The last line is the number of passed tests, summed over every "test result:" line; 0 fails.
# The whole script holds the machine-wide lock for heavy commands, so run it without flock.

set -euo pipefail

lock=/tmp/big-heavy.lock
if [[ "${NMTCODE_CHECK_HOLDS_LOCK:-}" != 1 ]]; then
  exec env NMTCODE_CHECK_HOLDS_LOCK=1 flock -w 3600 "$lock" "$0" "$@"
fi

cd "$(dirname "$0")/.."

step() {
  printf '==> %s\n' "$*"
}

step cargo fmt --all --check
cargo fmt --all --check

step cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets -- -D warnings

log=$(mktemp)
trap 'rm -f "$log"' EXIT
step cargo test --workspace
cargo test --workspace 2>&1 | tee "$log"

wasm_target=wasm32-unknown-unknown
if [[ -d "$(rustc --print sysroot)/lib/rustlib/$wasm_target" ]]; then
  step cargo build -p nmtcode-core -p nmtcode-ecc -p nmtcode-symbol -p nmtcode-payload \
    --no-default-features --target "$wasm_target"
  cargo build -p nmtcode-core -p nmtcode-ecc -p nmtcode-symbol -p nmtcode-payload \
    --no-default-features --target "$wasm_target"
else
  step "skip: target $wasm_target is not installed, so the no_std build is not checked"
fi

step version check: no Cargo.toml declares 1.0.0 or above
# Checked: the version of [package] and [workspace.package], and the version of every
# dependency on a crate of this workspace (nmtcode, nmtcode-*). Third-party versions are not.
for manifest in Cargo.toml crates/*/Cargo.toml; do
  awk -v file="$manifest" '
    function check(line,    value) {
      if (match(line, /version[[:space:]]*=[[:space:]]*"[^0-9"]*[0-9]+/)) {
        value = substr(line, RSTART, RLENGTH)
        sub(/.*"[^0-9"]*/, "", value)
        if (value + 0 >= 1) {
          printf "%s: version 1.0.0 or above: %s\n", file, line
          bad = 1
        }
      }
    }
    /^[[:space:]]*\[/ { section = $0; next }
    section ~ /^\[(workspace\.)?package\][[:space:]]*$/ && /^[[:space:]]*version[[:space:]]*=/ {
      check($0)
    }
    /^[[:space:]]*nmtcode(-[a-z0-9]+)?[[:space:]]*=/ && /version[[:space:]]*=/ { check($0) }
    END { exit bad }
  ' "$manifest"
done

total=$(grep -E '^test result:' "$log" | sed -E 's/.* ([0-9]+) passed.*/\1/' | awk '{ s += $1 } END { print s + 0 }')
if [[ "$total" -eq 0 ]]; then
  printf 'total passed tests: 0 (fail: no test ran)\n'
  exit 1
fi
printf 'total passed tests: %s\n' "$total"
