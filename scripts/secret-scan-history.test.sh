#!/usr/bin/env bash
# Tests for scripts/secret-scan-history.sh, using fixture reports (no gitleaks needed).
#
#   scripts/secret-scan-history.test.sh
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
script="$here/secret-scan-history.sh"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fails=0

finding() { # fingerprint
  local commit=${1%%:*} rest=${1#*:}
  local file=${rest%%:*}
  printf '{"Fingerprint":"%s","RuleID":"nullbox-admin-token","File":"%s","StartLine":7,"Commit":"%s"}' \
    "$1" "$file" "$commit"
}

check() { # name, expected exit, expected output substring, report json, known list
  local name=$1 want=$2 needle=$3
  printf '%s\n' "$4" >"$tmp/report.json"
  printf '%s\n' "$5" >"$tmp/known.txt"
  local out rc=0
  out=$(SECRET_SCAN_KNOWN="$tmp/known.txt" GITHUB_ACTIONS='' GITHUB_STEP_SUMMARY='' \
    "$script" "$tmp/report.json" 2>&1) || rc=$?
  if [ "$rc" != "$want" ] || ! grep -qF -- "$needle" <<<"$out"; then
    echo "FAIL: $name (exit $rc, want $want; looking for '$needle')"
    printf '    %s\n' "${out//$'\n'/$'\n'    }"
    fails=$((fails + 1))
  else
    echo "ok:   $name"
  fi
}

A="aaaaaaa000000000000000000000000000000000:.env:nullbox-admin-token:7"
B="bbbbbbb000000000000000000000000000000000:src/x.rs:nullbox-admin-token:7"

check "clean history passes" 0 "0 finding(s): 0 new" '[]' ''
check "known finding passes with a warning" 0 "[warning] Known leak" "[$(finding "$A")]" "# comment
$A"
check "new finding fails" 1 "[error] New secret" "[$(finding "$A"),$(finding "$B")]" "$A"
check "inline comments and blank lines are ignored" 0 "1 known" "[$(finding "$A")]" "
  $A   # the .env leak
"
check "stale known entry is reported, still passes" 0 "Stale known finding" '[]' "$A"
rc=0
"$script" "$tmp/does-not-exist.json" >/dev/null 2>&1 || rc=$?
if [ "$rc" = 2 ]; then echo "ok:   missing report exits 2"; else
  echo "FAIL: missing report exits 2 (got $rc)"
  fails=$((fails + 1))
fi

# The repo's own known list must parse and only hold fingerprints.
if grep -vE '^\s*(#.*)?$' "$here/../.github/secret-scan-known.txt" |
  grep -vqE '^[0-9a-f]{40}:[^:]+:[a-z0-9-]+:[0-9]+\s*(#.*)?$'; then
  echo "FAIL: .github/secret-scan-known.txt has a line that is not a fingerprint"
  fails=$((fails + 1))
else
  echo "ok:   repo known list is well-formed"
fi

if [ "$fails" -ne 0 ]; then
  echo "$fails test(s) failed"
  exit 1
fi
echo "all tests passed"
