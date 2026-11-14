#!/usr/bin/env bash
# Full-history secret scan (#48). Scans every commit with gitleaks and compares the
# findings with the documented known list (.github/secret-scan-known.txt, see #10).
#
#   scripts/secret-scan-history.sh [report.json]
#
# With no argument it runs `gitleaks git` over the whole history (gitleaks must be on
# PATH). With a report file it only evaluates that report (used by the tests).
#
# Exit codes:
#   0  no findings, or only known ones (each is printed as a warning, not hidden)
#   1  a finding that is not on the known list: a new leak, rotate it first
#   2  usage or tool error
#
# Known entries that no longer match anything (e.g. after the purge) are reported as
# stale so the list gets emptied. In GitHub Actions, output uses ::warning/::error
# annotations and a step summary.
set -euo pipefail

KNOWN=${SECRET_SCAN_KNOWN:-.github/secret-scan-known.txt}
CONFIG=${SECRET_SCAN_CONFIG:-.gitleaks.toml}

command -v jq >/dev/null || {
  echo "jq is required" >&2
  exit 2
}

if [ $# -ge 1 ]; then
  report=$1
  [ -f "$report" ] || {
    echo "no such report: $report" >&2
    exit 2
  }
else
  command -v gitleaks >/dev/null || {
    echo "gitleaks is required" >&2
    exit 2
  }
  report=$(mktemp)
  trap 'rm -f "$report"' EXIT
  # gitleaks exits 1 when it finds leaks; we decide pass/fail from the report instead.
  rc=0
  gitleaks git --config "$CONFIG" --redact --no-banner \
    --report-format json --report-path "$report" || rc=$?
  if [ "$rc" -gt 1 ]; then
    echo "gitleaks failed (exit $rc)" >&2
    exit 2
  fi
fi

known=()
if [ -f "$KNOWN" ]; then
  while IFS= read -r line; do
    line=${line%%#*}
    line=$(tr -d '[:space:]' <<<"$line")
    [ -n "$line" ] && known+=("$line")
  done <"$KNOWN"
fi

is_known() {
  local fp
  for fp in "${known[@]}"; do [ "$fp" = "$1" ] && return 0; done
  return 1
}

annotate() { # level, title, message
  if [ -n "${GITHUB_ACTIONS:-}" ]; then
    echo "::$1 title=$2::$3"
  else
    echo "[$1] $2: $3"
  fi
}

summary() { [ -n "${GITHUB_STEP_SUMMARY:-}" ] && echo "$1" >>"$GITHUB_STEP_SUMMARY" || true; }

mapfile -t findings < <(jq -r '.[] | [.Fingerprint, .RuleID, .File, (.StartLine|tostring), .Commit[0:7]] | @tsv' "$report")

new=0 seen=0
found_fps=()
summary "## Full-history secret scan"
summary ""
for row in "${findings[@]}"; do
  IFS=$'\t' read -r fp rule file line commit <<<"$row"
  found_fps+=("$fp")
  if is_known "$fp"; then
    seen=$((seen + 1))
    annotate warning "Known leak in history (#10)" "$rule in $file:$line at $commit. Still public until the history purge."
    summary "- ⚠️ known: \`$rule\` in \`$file:$line\` at \`$commit\` (waiting for #10)"
  else
    new=$((new + 1))
    annotate error "New secret in history" "$rule in $file:$line at $commit. Rotate it first, then tell the team (AGENTS.md)."
    summary "- ❌ **new**: \`$rule\` in \`$file:$line\` at \`$commit\`"
  fi
done

stale=0
for fp in "${known[@]}"; do
  hit=0
  for f in "${found_fps[@]}"; do [ "$f" = "$fp" ] && hit=1 && break; done
  if [ "$hit" = 0 ]; then
    stale=$((stale + 1))
    annotate warning "Stale known finding" "$fp no longer appears in history. Remove it from $KNOWN."
    summary "- 🧹 stale entry in \`$KNOWN\`: \`$fp\`"
  fi
done

echo "full-history scan: ${#findings[@]} finding(s): $new new, $seen known, $stale stale known entr(ies)"
summary ""
summary "${#findings[@]} finding(s): $new new, $seen known, $stale stale."

[ "$new" -eq 0 ]
