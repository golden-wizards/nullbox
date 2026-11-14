#!/usr/bin/env bash
# Press HOT "Check" for upgrades that are ready to be scored, with guard rails.
#
#   HOT_TOKEN=... scripts/hot-check.sh            # every eligible check
#   HOT_TOKEN=... scripts/hot-check.sh seo payments   # only these keys (if eligible)
#   DRY_RUN=1 ...                                  # show what would be pressed
#
# Eligible = decided by HOT (not judge-only), unlocked, not passed yet, not handed
# to a judge, and fewer than HOT_MAX_FAILURES failures (default 1: automation only
# presses checks that never failed, so it costs at most one failure per check and
# leaves the other tries to humans). A third failure
# means "ask a judge".
#
# Never prints the token. Only non-secret fields of /api/team are read.
set -euo pipefail

: "${HOT_TOKEN:?set HOT_TOKEN (the team login code) in the environment}"
API=${HOT_API:-https://orga.fasthacks.cloud/api}
MAX_FAILURES=${HOT_MAX_FAILURES:-1}
WAIT=${HOT_WAIT_SECONDS:-60}
OUT=${GITHUB_STEP_SUMMARY:-/dev/null}

status() {
  curl -fsS -H "Authorization: Bearer $HOT_TOKEN" "$API/team" |
    jq -c '{points, checks: [.checks[] | {key, points, decided_by, locked, passed_at, failures, asked_judge,
            last: (.last_run | if . then {status, note} else null end)}]}'
}

before=$(status)
want=$(printf '%s\n' "$@" | jq -R . | jq -sc 'map(select(length > 0))')
eligible=$(jq -r --argjson max "$MAX_FAILURES" --argjson want "$want" '
  .checks[]
  | select(.decided_by != "judge" and (.locked | not) and .passed_at == null and (.asked_judge | not))
  | select(.failures < $max)
  | select(($want | length) == 0 or (.key as $k | $want | index($k)))
  | select(.last.status != "running" and .last.status != "queued")
  | .key' <<<"$before")

echo "Score before: $(jq .points <<<"$before") pts"
if [ -z "$eligible" ]; then
  echo "Nothing eligible to press."
  exit 0
fi

for key in $eligible; do
  if [ -n "${DRY_RUN:-}" ]; then
    echo "would press: $key"
    continue
  fi
  code=$(curl -sS -o /dev/null -w '%{http_code}' -X POST -H "Authorization: Bearer $HOT_TOKEN" "$API/team/checks/$key")
  echo "pressed: $key (HTTP $code)"
done
[ -n "${DRY_RUN:-}" ] && exit 0

# Checks run asynchronously: wait for them to settle, then report.
deadline=$(($(date +%s) + WAIT))
while :; do
  after=$(status)
  busy=$(jq '[.checks[] | select(.last.status == "running" or .last.status == "queued")] | length' <<<"$after")
  if [ "$busy" -eq 0 ] || [ "$(date +%s)" -ge "$deadline" ]; then break; fi
  sleep 5
done

{
  echo "### HOT checks"
  echo
  echo "Score: $(jq .points <<<"$before") → $(jq .points <<<"$after") pts"
  echo
  echo "| Check | Result | Failures | Note |"
  echo "| --- | --- | --- | --- |"
  for key in $eligible; do
    jq -r --arg k "$key" '.checks[] | select(.key == $k)
      | "| \(.key) | \(if .passed_at then "✅ passed" else (.last.status // "?") end) | \(.failures) | \(.last.note // "" | gsub("\\|"; "/") | .[0:160]) |"' <<<"$after"
  done
} | tee -a "$OUT"
