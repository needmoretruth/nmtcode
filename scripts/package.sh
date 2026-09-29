#!/usr/bin/env bash
# Builds and packs one release archive into dist/.
#
#   scripts/package.sh <target>   Builds the nmtcode command for <target> (cargo build --release
#                                 --locked -p nmtcode-cli) and packs it with LICENSE-MIT,
#                                 LICENSE-APACHE, README.md and THIRD-PARTY-LICENSES.html into
#                                 dist/nmtcode-<version>-<target>.tar.gz (.zip for a Windows target).
#   scripts/package.sh web        Packs the web page that scripts/build-web.sh built (web/ with
#                                 web/pkg/, without tests) with LICENSE-MIT, LICENSE-APACHE and the
#                                 THIRD-PARTY-LICENSES.html of crates/nmtcode-wasm for wasm32 into
#                                 dist/nmtcode-web-<version>.zip.
#   scripts/package.sh version    Prints the workspace version.
#
# Each archive holds one folder with the archive's name. THIRD-PARTY-LICENSES.html is made by
# cargo-about 0.9.2 from about.toml and about.hbs. On a machine with flock, the script holds the
# machine-wide lock for heavy commands itself, so run it without flock.
#
# Needs: cargo, cargo-about 0.9.2, the Rust target, tar, and zip, 7z or Python for .zip files.

set -euo pipefail

about_version=0.9.2
lock=/tmp/big-heavy.lock

usage() {
  printf 'usage: scripts/package.sh <target> | web | version\n' >&2
  exit 2
}

[[ $# -eq 1 ]] || usage
what="$1"

cd "$(dirname "$0")/.."
root="$(pwd)"

# The version of [workspace.package] in Cargo.toml: one number for every crate.
workspace_version() {
  awk '
    /^[[:space:]]*\[/ { in_package = ($0 ~ /^[[:space:]]*\[workspace\.package\][[:space:]]*$/); next }
    in_package && /^[[:space:]]*version[[:space:]]*=/ {
      value = $0
      sub(/^[^"]*"/, "", value)
      sub(/".*$/, "", value)
      print value
      exit
    }
  ' Cargo.toml
}

version="$(workspace_version)"
if [[ -z "$version" ]]; then
  printf 'package: no version in [workspace.package] of Cargo.toml\n' >&2
  exit 1
fi

if [[ "$what" == version ]]; then
  printf '%s\n' "$version"
  exit 0
fi

if [[ "${NMTCODE_PACKAGE_HOLDS_LOCK:-}" != 1 ]] && command -v flock >/dev/null 2>&1; then
  exec env NMTCODE_PACKAGE_HOLDS_LOCK=1 flock -w 3600 "$lock" bash "$0" "$@"
fi

step() {
  printf '==> %s\n' "$*"
}

step "cargo-about is version $about_version"
found_about="$(cargo about --version 2>/dev/null || true)"
if [[ "$found_about" != "cargo-about $about_version" ]]; then
  printf 'package: need cargo-about %s, found "%s"\n' "$about_version" "$found_about" >&2
  printf 'package: cargo install --locked --features cli cargo-about --version %s\n' "$about_version" >&2
  exit 1
fi

# Makes a zip of folder $2 inside directory $1 at the absolute path $3.
make_zip() {
  local dir="$1" folder="$2" out="$3"
  if command -v zip >/dev/null 2>&1; then
    (cd "$dir" && zip -q -r -X "$out" "$folder")
  elif command -v 7z >/dev/null 2>&1; then
    (cd "$dir" && 7z a -tzip -bso0 -bsp0 "$out" "$folder")
  else
    local python
    python="$(command -v python3 || command -v python || true)"
    if [[ -z "$python" ]]; then
      printf 'package: need zip, 7z or Python to write %s\n' "$out" >&2
      exit 1
    fi
    (cd "$dir" && "$python" -m zipfile -c "$out" "$folder")
  fi
}

notices() {
  local manifest="$1" target="$2" out="$3"
  step cargo about generate --locked --fail -m "$manifest" --target "$target" -o "$out" about.hbs
  cargo about generate --locked --fail -m "$manifest" --target "$target" -c about.toml -o "$out" about.hbs
  if ! grep -q '<section' "$out"; then
    printf 'package: %s lists no licence\n' "$out" >&2
    exit 1
  fi
}

dist="$root/dist"
mkdir -p "$dist"
stage="$(mktemp -d)"
trap 'rm -r -f -- "$stage"' EXIT

if [[ "$what" == web ]]; then
  name="nmtcode-web-$version"
  if [[ -z "$(find web/pkg -name '*.wasm' -print 2>/dev/null | head -n 1)" ]]; then
    printf 'package: web/pkg/ has no .wasm file; run scripts/build-web.sh first\n' >&2
    exit 1
  fi
  step "copy web/ without tests into $name/"
  mkdir -p "$stage/$name"
  # Everything under web/ except test folders, test files and tooling for the tests.
  (
    cd web
    find . -type f \
      ! -path './tests/*' ! -path './test/*' ! -path './e2e/*' ! -path '*/__tests__/*' \
      ! -path './node_modules/*' ! -path '*/.cache/*' \
      ! -name '*.test.*' ! -name '*.spec.*' ! -name 'playwright.config.*' \
      ! -name 'package.json' ! -name 'package-lock.json' ! -name '.gitignore' \
      -print0
  ) | while IFS= read -r -d '' file; do
    mkdir -p "$stage/$name/$(dirname "$file")"
    cp -p "web/$file" "$stage/$name/$file"
  done
  cp LICENSE-MIT LICENSE-APACHE "$stage/$name/"
  notices crates/nmtcode-wasm/Cargo.toml wasm32-unknown-unknown "$stage/$name/THIRD-PARTY-LICENSES.html"
  archive="$dist/$name.zip"
  rm -f -- "$archive"
  step "write $archive"
  make_zip "$stage" "$name" "$archive"
  printf '%s\n' "$archive"
  exit 0
fi

target="$what"
[[ "$target" =~ ^[a-z0-9_]+(-[a-z0-9_.]+)+$ ]] || usage
name="nmtcode-$version-$target"
case "$target" in
  *-windows-*) exe=nmtcode.exe ;;
  *) exe=nmtcode ;;
esac

step cargo build --release --locked -p nmtcode-cli --target "$target"
cargo build --release --locked -p nmtcode-cli --target "$target"
binary="target/$target/release/$exe"
if [[ ! -f "$binary" ]]; then
  printf 'package: the build left no %s\n' "$binary" >&2
  exit 1
fi

step "stage $name/"
mkdir -p "$stage/$name"
cp "$binary" "$stage/$name/$exe"
cp LICENSE-MIT LICENSE-APACHE README.md "$stage/$name/"
notices crates/nmtcode-cli/Cargo.toml "$target" "$stage/$name/THIRD-PARTY-LICENSES.html"

case "$target" in
  *-windows-*)
    archive="$dist/$name.zip"
    rm -f -- "$archive"
    step "write $archive"
    make_zip "$stage" "$name" "$archive"
    ;;
  *)
    archive="$dist/$name.tar.gz"
    rm -f -- "$archive"
    step "write $archive"
    # COPYFILE_DISABLE keeps macOS tar from adding ._ files of extended attributes.
    COPYFILE_DISABLE=1 tar -C "$stage" -czf "$archive" "$name"
    ;;
esac
printf '%s\n' "$archive"
