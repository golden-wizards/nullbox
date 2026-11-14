#!/usr/bin/env bash
# Watch a PR's CI checks until they settle, and print what failed and why.
#
#   scripts/pr-watch.sh <pr-number> [poll-seconds]
#
# Exit codes (the AGENTS.md "watch CI" loop branches on these):
#   0  every check passed (or the PR was merged)
#   1  a check failed: its errors are printed below the summary
#   2  the PR needs you: merge conflict, or closed without merging
#   3  timed out (PR_WATCH_MAX_MIN, default 45 minutes)
#
# It exits on the FIRST failed check instead of waiting for slow ones (CodeQL),
# so you can start fixing right away. Uses only `gh api` (REST) and jq.
set -euo pipefail

PR=${1:?usage: scripts/pr-watch.sh <pr-number> [poll-seconds]}
INTERVAL=${2:-30}
R=${PR_WATCH_REPO:-repos/golden-wizards/nullbox}
deadline=$(($(date +%s) + ${PR_WATCH_MAX_MIN:-45} * 60))
BAD='["failure","cancelled","timed_out","action_required","startup_failure","stale"]'
last=""

show_failure() { # $1 = check-run JSON
  local id name url
  id=$(jq -r .id <<<"$1")
  name=$(jq -r .name <<<"$1")
  url=$(jq -r .html_url <<<"$1")
  echo
  echo "━━ ❌ $name ($(jq -r .conclusion <<<"$1")) $url"
  # Annotations: compiler/linter errors with file:line, when the tool emits them.
  local ann
  if ann=$(gh api "$R/check-runs/$id/annotations?per_page=50" \
    --jq '.[] | select(.annotation_level != "notice") | "  \(.path):\(.start_line) [\(.annotation_level)] \(.message | gsub("\n"; " ") | .[0:300])"' 2>/dev/null); then
    [ -n "$ann" ] && head -30 <<<"$ann"
  fi
  # Log tail: only exists for GitHub Actions jobs, and can take a few seconds to appear.
  local log=""
  for _ in 1 2 3; do
    log=$(gh api "$R/actions/jobs/$id/logs" 2>/dev/null) && break
    log=""
    sleep 5
  done
  if [ -n "$log" ]; then
    echo "  ── log (errors) ──"
    grep -A4 -E '##\[error\]|^error(\[|:)|error: |FAIL|panicked|✖|×|failed|ERROR' <<<"$log" |
      sed -E 's/^[0-9TZ:.-]+ //' | grep -v '^\s*$' | grep -v "^--$" | tail -60 | sed 's/^/  /' || true
  else
    echo "  (no log available via API: open the link above)"
  fi
}

while :; do
  pr=$(gh api "$R/pulls/$PR")
  if [ "$(jq -r .merged <<<"$pr")" = true ]; then echo "PR #$PR is merged ✅"; exit 0; fi
  if [ "$(jq -r .state <<<"$pr")" != open ]; then echo "PR #$PR was closed without merging"; exit 2; fi
  sha=$(jq -r .head.sha <<<"$pr")
  ms=$(jq -r .mergeable_state <<<"$pr")
  if [ "$ms" = dirty ]; then
    echo "PR #$PR has merge conflicts: git fetch origin && git rebase origin/main, resolve, push --force-with-lease"
    exit 2
  fi

  runs=$(gh api "$R/commits/$sha/check-runs?per_page=100" --jq '.check_runs')
  total=$(jq length <<<"$runs")
  pending=$(jq '[.[] | select(.status != "completed")] | length' <<<"$runs")
  failed=$(jq -c --argjson bad "$BAD" '[.[] | select(.status == "completed" and (.conclusion as $c | $bad | index($c)))]' <<<"$runs")
  nfailed=$(jq length <<<"$failed")

  line="PR #$PR @ ${sha:0:7}: $total checks, $pending running, $nfailed failed (mergeable: $ms)"
  [ "$line" != "$last" ] && echo "$(date -u +%H:%M:%S) $line" && last=$line

  if [ "$nfailed" -gt 0 ]; then
    jq -c '.[]' <<<"$failed" | while read -r run; do show_failure "$run"; done
    echo
    echo "Fix, commit, push, then run the watcher again."
    exit 1
  fi
  if [ "$total" -gt 0 ] && [ "$pending" -eq 0 ]; then
    echo "All $total checks passed on ${sha:0:7} ✅ (auto-merge takes it from here)"
    exit 0
  fi
  if [ "$(date +%s)" -ge "$deadline" ]; then echo "Timed out with $pending checks still running"; exit 3; fi
  sleep "$INTERVAL"
done
