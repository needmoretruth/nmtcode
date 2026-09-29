#!/usr/bin/env bash
# Fails when git tracks, or has staged, a file that must not be in this public repository:
#
#   - a file that no line of scripts/public-files.txt admits;
#   - whatever that list says: a file named CLAUDE.md, AGENTS.md or GEMINI.md; anything under a
#     notes/, .claude*/ or .playwright-mcp/ folder at any depth; and a key or credential file
#     (*.env, .env.*, *.pem, *.key, id_rsa*, id_ed25519*, credentials*.json).
#
# The files are what `git ls-files` lists: every tracked file plus every staged new file.
# The script prints each refused path with the reason, and never reads the files.

set -euo pipefail

cd "$(dirname "$0")/.."

list=scripts/public-files.txt

# Converts one glob of the list to an anchored extended regular expression:
# `**/` is zero or more folders, `**` is any characters, `*` and `?` stop at `/`.
glob_to_regex() {
  local glob="$1" out='' c
  local -i i=0 n=${#1}
  while ((i < n)); do
    c="${glob:i:1}"
    if [[ "$c" == '*' && "${glob:i+1:1}" == '*' ]]; then
      if [[ "${glob:i+2:1}" == '/' ]]; then
        out+='(.*/)?'
        i+=3
      else
        out+='.*'
        i+=2
      fi
      continue
    fi
    case "$c" in
      '*') out+='[^/]*' ;;
      '?') out+='[^/]' ;;
      '.' | '+' | '^' | '$' | '(' | ')' | '{' | '}' | '|' | '[' | ']' | "\\")
        out+="\\$c" ;;
      *) out+="$c" ;;
    esac
    i+=1
  done
  printf '^%s$' "$out"
}

# Prints why a path is refused whatever the list says, or nothing.
never_public() {
  local path="$1" base lower
  base="${path##*/}"
  lower="${base,,}"
  case "$lower" in
    claude.md | agents.md | gemini.md)
      printf 'instructions for an AI assistant (%s)' "$base"
      return ;;
    *.env | .env.* | *.pem | *.key | id_rsa* | id_ed25519* | credentials*.json)
      printf 'key or credential file (%s)' "$base"
      return ;;
  esac
  if [[ "/$path" == */notes/* ]]; then
    printf 'under a notes/ folder'
  elif [[ "/$path" =~ /\.claude[^/]*/ ]]; then
    printf 'under a .claude*/ folder'
  elif [[ "/$path" == */.playwright-mcp/* ]]; then
    printf 'under a .playwright-mcp/ folder'
  fi
}

if [[ ! -f "$list" ]]; then
  printf 'check-public-files: %s is missing\n' "$list"
  exit 1
fi

regexes=()
negated=()
while IFS= read -r line || [[ -n "$line" ]]; do
  line="${line#"${line%%[![:space:]]*}"}"
  line="${line%"${line##*[![:space:]]}"}"
  [[ -z "$line" || "$line" == \#* ]] && continue
  if [[ "$line" == '!'* ]]; then
    negated+=(1)
    regexes+=("$(glob_to_regex "${line:1}")")
  else
    negated+=(0)
    regexes+=("$(glob_to_regex "$line")")
  fi
done <"$list"

if ((${#regexes[@]} == 0)); then
  printf 'check-public-files: %s has no pattern\n' "$list"
  exit 1
fi

printf '==> public files: every tracked or staged file against %s (%d patterns)\n' \
  "$list" "${#regexes[@]}"

files=0
refused=0
while IFS= read -r -d '' path; do
  files=$((files + 1))
  reason="$(never_public "$path")"
  if [[ -z "$reason" ]]; then
    admitted=0
    for k in "${!regexes[@]}"; do
      if [[ "$path" =~ ${regexes[k]} ]]; then
        if [[ "${negated[k]}" == 1 ]]; then admitted=0; else admitted=1; fi
      fi
    done
    if [[ "$admitted" == 0 ]]; then
      reason="no line of $list admits it"
    fi
  fi
  if [[ -n "$reason" ]]; then
    refused=$((refused + 1))
    printf 'refused: %s: %s\n' "$path" "$reason"
  fi
done < <(git ls-files -z --cached)

if ((files == 0)); then
  printf 'check-public-files: git lists no file (fail: not a git work tree?)\n'
  exit 1
fi
if ((refused > 0)); then
  printf 'public files: %d of %d files refused\n' "$refused" "$files"
  exit 1
fi
printf 'public files: ok, %d files\n' "$files"
