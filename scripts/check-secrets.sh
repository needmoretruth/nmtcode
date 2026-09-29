#!/usr/bin/env bash
# Fails when a tracked or staged file contains what looks like a secret:
#
#   - a private-key header (PEM, OpenSSH, PGP);
#   - a GitHub token (ghp_, gho_, ghs_, ghu_, ghr_, github_pat_);
#   - an AWS access key id;
#   - a Slack token or incoming-webhook address;
#   - an assignment of a literal of 16 or more characters to a name containing
#     secret, password or passwd (`secret=...`, `password: "..."`).
#
# Both the working-tree copy and the staged copy of every file that `git ls-files` lists are
# searched; binary files are skipped. The script prints the file, the line number and the kind of
# secret, and never the line or the value.

set -euo pipefail

cd "$(dirname "$0")/.."

# kind | grep options | extended regular expression
rules=(
  'private-key header|-E|-----BEGIN [A-Z0-9 ]*PRIVATE KEY'
  'GitHub token|-E|(ghp|gho|ghs|ghu|ghr)_[A-Za-z0-9]{36}'
  'GitHub fine-grained token|-E|github_pat_[A-Za-z0-9_]{22,}'
  'AWS access key id|-E|(^|[^A-Z0-9])(AKIA|ASIA|ABIA|ACCA|A3T[A-Z0-9])[A-Z0-9]{16}([^A-Z0-9]|$)'
  'Slack token|-E|xox[abposr]-[A-Za-z0-9-]{10,}'
  'Slack webhook|-E|hooks[.]slack[.]com/services/T[A-Za-z0-9]+/B[A-Za-z0-9]+/[A-Za-z0-9]+'
  "secret or password assignment|-E -i|(secret|password|passwd)[A-Za-z0-9_.-]*[\"']?[[:space:]]*(:=|=|:)[[:space:]]*[\"']?[A-Za-z0-9+/_.~@#%^&*=!-]{16,}[\"']?([[:space:],;)]|$)"
)

# Reads one file's text on standard input and prints the numbers of the lines that match.
# The matching lines themselves stay inside this function: only the number before the first
# colon of grep's "number:line" output is printed.
matching_lines() {
  local regex="$1" out status=0
  shift
  out="$(grep -n -I "$@" -e "$regex")" || status=$?
  if ((status > 1)); then
    printf 'check-secrets: grep failed with status %d\n' "$status" >&2
    return 2
  fi
  if [[ -n "$out" ]]; then
    cut -d: -f1 <<<"$out"
  fi
}

# Lists, NUL-separated, the files whose working-tree (no argument) or staged (--cached) copy matches.
candidates() {
  local regex="$1" status=0
  shift
  git grep -z -l -I "$@" -e "$regex" || status=$?
  if ((status > 1)); then
    printf 'check-secrets: git grep failed with status %d\n' "$status" >&2
    return 2
  fi
}

files="$(git ls-files --cached | wc -l)"
if ((files == 0)); then
  printf 'check-secrets: git lists no file (fail: not a git work tree?)\n'
  exit 1
fi

printf '==> secrets: private keys, GitHub, AWS and Slack tokens, secret and password literals in %d tracked or staged files\n' "$files"

found=0
declare -A reported=()
list="$(mktemp)"
trap 'rm -f "$list"' EXIT
for rule in "${rules[@]}"; do
  kind="${rule%%|*}"
  rest="${rule#*|}"
  read -r -a options <<<"${rest%%|*}"
  regex="${rest#*|}"
  for copy in working-tree staged; do
    if [[ "$copy" == working-tree ]]; then
      scope=()
    else
      scope=(--cached)
    fi
    candidates "$regex" "${scope[@]}" "${options[@]}" >"$list"
    while IFS= read -r -d '' path; do
      if [[ "$copy" == working-tree ]]; then
        lines="$(matching_lines "$regex" "${options[@]}" <"$path")"
      else
        lines="$(git show ":$path" | matching_lines "$regex" "${options[@]}")"
      fi
      while IFS= read -r number; do
        [[ -n "$number" ]] || continue
        key="$path:$number:$kind"
        [[ -z "${reported[$key]:-}" ]] || continue
        reported[$key]=1
        found=$((found + 1))
        printf 'secret: %s:%s: %s (%s copy)\n' "$path" "$number" "$kind" "$copy"
      done <<<"$lines"
    done <"$list"
  done
done

if ((found > 0)); then
  printf 'secrets: %d found; remove them, and rotate any real secret\n' "$found"
  exit 1
fi
printf 'secrets: none found\n'
