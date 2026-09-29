#!/usr/bin/env bash
# Every check of the workspace, in order; the first failure stops the script with its exit code.
#
#   1. cargo fmt --all --check
#   2. cargo clippy --workspace --all-targets -- -D warnings
#   3. cargo test --workspace
#   4. the no_std layer crates for wasm32-unknown-unknown, when that target is installed
#   5. crates/nmtcode-wasm in release mode for wasm32-unknown-unknown, when the crate exists
#   6. no Cargo.toml declares a version of 1.0.0 or above
#   7. cargo deny: licences, bans and sources (deny.toml), then advisories when the RustSec
#      database on github.com is reachable
#   8. report only: runtime dependencies whose licence gives no GPL-2.0-compatible choice
#   9. scripts/check-public-files.sh: every tracked file is allowed in the public repository
#  10. scripts/check-secrets.sh: no tracked file contains a key or token
#
# The last line is the number of passed tests, summed over every "test result:" line; 0 fails.
# The whole script holds the machine-wide lock for heavy commands, so run it without flock.

set -euo pipefail

lock=/tmp/big-heavy.lock
if [[ "${NMTCODE_CHECK_HOLDS_LOCK:-}" != 1 ]]; then
  exec env NMTCODE_CHECK_HOLDS_LOCK=1 flock -w 3600 "$lock" bash "$0" "$@"
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

if [[ ! -f crates/nmtcode-wasm/Cargo.toml ]]; then
  step "skip: crates/nmtcode-wasm does not exist, so its wasm32 release build is not checked"
elif [[ -d "$(rustc --print sysroot)/lib/rustlib/$wasm_target" ]]; then
  step cargo build -p nmtcode-wasm --release --locked --target "$wasm_target"
  cargo build -p nmtcode-wasm --release --locked --target "$wasm_target"
else
  step "skip: target $wasm_target is not installed, so the nmtcode-wasm release build is not checked"
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

if ! command -v cargo-deny >/dev/null 2>&1; then
  printf 'cargo-deny is not installed: cargo install --locked cargo-deny --version 0.20.2\n'
  exit 1
fi
step cargo deny --locked check licenses bans sources
cargo deny --locked check licenses bans sources

if timeout 30 git ls-remote --exit-code https://github.com/rustsec/advisory-db HEAD >/dev/null 2>&1; then
  step cargo deny --locked check advisories
  cargo deny --locked check advisories
else
  step "skip: github.com/rustsec/advisory-db is not reachable, so cargo deny check advisories did not run"
fi

step "report only: runtime dependencies whose licence gives no GPL-2.0-compatible choice"
if command -v python3 >/dev/null 2>&1; then
  metadata=$(mktemp)
  trap 'rm -f "$log" "$metadata"' EXIT
  cargo metadata --format-version 1 --locked --all-features >"$metadata"
  python3 - "$metadata" <<'PY'
import json
import re
import sys

# Licences that can be combined with GPL-2.0 code (FSF list of GPL-compatible licences). The
# LLVM exception to Apache-2.0 exists for GPL-2.0 compatibility; Apache-2.0 alone is not.
COMPATIBLE = {
    "0BSD", "BSD-2-Clause", "BSD-3-Clause", "BSL-1.0", "CC0-1.0", "ISC", "MIT", "MIT-0",
    "Unicode-3.0", "Unicode-DFS-2016", "Unlicense", "Zlib",
    "Apache-2.0 WITH LLVM-exception",
}


def compatible(expression):
    """True when some choice of the SPDX expression uses only GPL-2.0-compatible licences."""
    tokens = re.findall(r"\(|\)|/|[^\s()/]+", expression)
    pos = 0

    def peek():
        return tokens[pos] if pos < len(tokens) else None

    def take():
        nonlocal pos
        pos += 1
        return tokens[pos - 1]

    def primary():
        if peek() == "(":
            take()
            value = any_of()
            if take() != ")":
                raise ValueError(expression)
            return value
        name = take().rstrip("+")
        if peek() is not None and peek().upper() == "WITH":
            take()
            exception = take()
            return f"{name} WITH {exception}" in COMPATIBLE or name in COMPATIBLE
        return name in COMPATIBLE

    def all_of():
        value = primary()
        while peek() is not None and peek().upper() == "AND":
            take()
            right = primary()
            value = value and right
        return value

    def any_of():
        value = all_of()
        while peek() is not None and (peek().upper() == "OR" or peek() == "/"):
            take()
            right = all_of()
            value = value or right
        return value

    value = any_of()
    if pos != len(tokens):
        raise ValueError(expression)
    return value


meta = json.load(open(sys.argv[1], encoding="utf-8"))
packages = {p["id"]: p for p in meta["packages"]}
nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
members = set(meta["workspace_members"])


def proc_macro(package):
    return any("proc-macro" in target["kind"] for target in package["targets"])


# Runtime dependencies: reached from a workspace crate through normal dependencies only (no dev or
# build dependency) and not through a proc-macro crate, which runs inside the compiler.
runtime = set()
stack = list(members)
while stack:
    for dep in nodes[stack.pop()]["deps"]:
        if not any(kind["kind"] is None for kind in dep["dep_kinds"]):
            continue
        child = dep["pkg"]
        if child in runtime or child in members or proc_macro(packages[child]):
            continue
        runtime.add(child)
        stack.append(child)

found = []
for pid in sorted(runtime, key=lambda i: (packages[i]["name"], packages[i]["version"])):
    package = packages[pid]
    licence = package.get("license")
    label = f"{package['name']} {package['version']}"
    if not licence:
        found.append(f"{label} (licence file only)")
        continue
    try:
        if not compatible(licence):
            found.append(f"{label} ({licence})")
    except (ValueError, IndexError):
        found.append(f"{label} (unreadable: {licence})")

print(f"info: {len(runtime)} runtime dependencies; with no GPL-2.0-compatible licence choice: "
      + (", ".join(found) if found else "none"))
PY
else
  step "skip: python3 is not installed, so the GPL-2.0 report did not run"
fi

step bash scripts/check-public-files.sh
bash scripts/check-public-files.sh

step bash scripts/check-secrets.sh
bash scripts/check-secrets.sh

total=$(grep -E '^test result:' "$log" | sed -E 's/.* ([0-9]+) passed.*/\1/' | awk '{ s += $1 } END { print s + 0 }')
if [[ "$total" -eq 0 ]]; then
  printf 'total passed tests: 0 (fail: no test ran)\n'
  exit 1
fi
printf 'total passed tests: %s\n' "$total"
