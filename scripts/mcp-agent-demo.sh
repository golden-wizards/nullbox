#!/usr/bin/env bash
# An agent works a ticket over MCP: picks up the oldest active ticket, claims it,
# leaves an internal note, replies to the customer and resolves it.
# Every step is a JSON-RPC call to the Nullbox MCP server; see docs/MCP.md.
#
#   MCP_TOKEN=... scripts/mcp-agent-demo.sh [https://team-06.fasthacks.cloud/mcp]
#
# Set DEMO_CREATE=1 to file a fresh demo ticket first (so the inbox is never empty).
# Needs curl and jq.
set -euo pipefail

URL="${1:-${MCP_URL:-https://team-06.fasthacks.cloud/mcp}}"
: "${MCP_TOKEN:?set MCP_TOKEN (the bearer token for /mcp)}"
AGENT="${AGENT_NAME:-Claude (MCP agent)}"
ID=0

rpc() { # rpc <method> [params-json]
  ID=$((ID + 1))
  local body params="${2:-}"
  [ -n "$params" ] || params="{}"
  body=$(jq -nc --arg m "$1" --argjson p "$params" --argjson id "$ID" '{jsonrpc:"2.0",id:$id,method:$m,params:$p}')
  curl -fsS "$URL" \
    -H "Authorization: Bearer $MCP_TOKEN" \
    -H 'Content-Type: application/json' \
    -H 'Accept: application/json, text/event-stream' \
    -H 'MCP-Protocol-Version: 2025-06-18' \
    -H "X-Actor-Name: $(jq -rn --arg a "$AGENT" '$a|@uri')" \
    -d "$body"
}

tool() { # tool <name> <arguments-json>; prints structuredContent, fails on isError
  local res
  res=$(rpc tools/call "$(jq -nc --arg n "$1" --argjson a "$2" '{name:$n,arguments:$a}')")
  if [ "$(jq -r '.result.isError // .error != null' <<<"$res")" = "true" ]; then
    echo "tool $1 failed: $(jq -c '.error // .result.content' <<<"$res")" >&2
    exit 1
  fi
  jq -c '.result.structuredContent' <<<"$res"
}

step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }

step "initialize"
rpc initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"mcp-agent-demo","version":"1"}}' |
  jq -c '.result | {protocolVersion, server: .serverInfo.name}'
curl -fsS -o /dev/null "$URL" -H "Authorization: Bearer $MCP_TOKEN" -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"notifications/initialized"}'

step "tools/list"
rpc tools/list | jq -r '.result.tools[] | "  \(.name): \(.description)"'

if [ "${DEMO_CREATE:-0}" = 1 ]; then
  step "create_ticket (demo customer)"
  tool create_ticket '{"subject":"Cannot log in to the VPN","body":"Since this morning the VPN says my password expired.","severity":"p2","customer_name":"Demo Customer"}' |
    jq -c '{number, subject, status}'
fi

step "list_tickets: what is waiting?"
tickets=$(tool list_tickets '{}')
jq -r '.tickets[] | "  #\(.number) [\(.severity)] \(.status): \(.subject)"' <<<"$tickets"
number=$(jq -r '(.tickets | map(select(.status == "open")) | sort_by(.created_at) | .[0].number) // (.tickets | sort_by(.created_at) | .[0].number) // empty' <<<"$tickets")
[ -n "$number" ] || { echo "Inbox Zero, nothing to do (try DEMO_CREATE=1)."; exit 0; }

step "get_ticket #$number: read the thread"
thread=$(tool get_ticket "{\"id\":\"$number\"}")
jq -r '"  subject: \(.ticket.subject)\n  body:    \(.ticket.body)\n  messages: \(.messages | length)"' <<<"$thread"
subject=$(jq -r '.ticket.subject' <<<"$thread")

step "update_ticket #$number: claim it"
tool update_ticket "$(jq -nc --arg id "$number" --arg a "$AGENT" '{id:$id,status:"in_progress",assignee_name:$a}')" |
  jq -c '{number, status, assignee_name}'

step "reply_to_ticket #$number: internal note"
tool reply_to_ticket "$(jq -nc --arg id "$number" '{id:$id,internal:true,body:"Picked up by the MCP agent demo. Triage: known issue, standard fix applies."}')" |
  jq -c '{author_name, internal}'

step "reply_to_ticket #$number: answer the customer"
tool reply_to_ticket "$(jq -nc --arg id "$number" --arg s "$subject" '{id:$id,body:("Thanks for reporting \"" + $s + "\". We have applied the fix on our side; please try again and reply here if it still happens.")}')" |
  jq -c '{author_name, internal}'

step "resolve_ticket #$number"
tool resolve_ticket "{\"id\":\"$number\"}" | jq -c '{number, status, closed_at}'

step "backlog_stats"
tool backlog_stats '{}' | jq -c .
