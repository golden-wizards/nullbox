# MCP server: let an AI agent work the inbox

Nullbox speaks the [Model Context Protocol](https://modelcontextprotocol.io) at **`/mcp`**, so an
AI agent (Claude Code, Claude Desktop, the MCP Inspector, your own script) can read tickets, reply,
and resolve them like a support worker. Code: `crates/api/src/http/mcp.rs`.

- Transport: streamable HTTP, stateless. `POST /mcp` with one JSON-RPC 2.0 message (or a batch),
  the answer is `application/json`. Notifications get `202`; `GET`/`DELETE` get `405`.
- Methods: `initialize`, `ping`, `tools/list`, `tools/call`.
- Auth: `Authorization: Bearer $MCP_TOKEN`. Without `MCP_TOKEN` on the server, every request gets
  `401`. Actions are attributed to `X-Actor-Name` (percent-encoded) if sent, else "MCP agent".

| Tool | What it does | Like |
| --- | --- | --- |
| `list_tickets` | active tickets, or those with `status` | `GET /api/tickets` |
| `get_ticket` | ticket + full thread incl. internal notes | `GET /api/tickets/{id}` |
| `reply_to_ticket` | reply to the customer, or `internal: true` note | `POST /api/tickets/{id}/messages` |
| `update_ticket` | `status`, `severity`, `assignee_name` | `PATCH /api/tickets/{id}` |
| `resolve_ticket` | yeet: mark resolved | `POST /api/tickets/{id}/yeet` |
| `create_ticket` | file a ticket for a customer | `POST /api/tickets` |
| `backlog_stats` | open count, beast mood | `GET /api/stats/backlog` |

`id` is a ticket UUID or its number (`"42"`). Tool failures (unknown ticket, invalid status
transition) come back as results with `isError: true`, so the model can read them and recover.

## Demo: an agent picks up a ticket and resolves it

### Scripted (no LLM, shows every MCP call)

```sh
MCP_TOKEN=... DEMO_CREATE=1 scripts/mcp-agent-demo.sh https://team-06.fasthacks.cloud/mcp
```

It runs `initialize`, `tools/list`, then works the oldest open ticket with `tools/call`:
`list_tickets` → `get_ticket` → `update_ticket` (claim: `in_progress`, assigned to the agent) →
`reply_to_ticket` (internal note) → `reply_to_ticket` (answer the customer) → `resolve_ticket` →
`backlog_stats`. `DEMO_CREATE=1` files a demo ticket first so the inbox is never empty. Open the
web app afterwards: the ticket is "Yeeted", with the agent's note and reply in the thread.

### With Claude Code (a real agent)

```sh
claude mcp add --transport http nullbox https://team-06.fasthacks.cloud/mcp \
  --header "Authorization: Bearer $MCP_TOKEN" --header "X-Actor-Name: Claude"
claude "Use the nullbox MCP tools. Pick the most urgent open ticket, claim it (in_progress, \
assigned to you), read the thread, add an internal note with your triage, reply to the customer \
with a helpful answer, then resolve it. Report what you did."
```

Any MCP client works the same way, e.g. the Inspector:

```sh
npx @modelcontextprotocol/inspector --cli https://team-06.fasthacks.cloud/mcp --transport http \
  --header "Authorization: Bearer $MCP_TOKEN" --method tools/list
```

### Raw curl

```sh
curl -sS https://team-06.fasthacks.cloud/mcp \
  -H "Authorization: Bearer $MCP_TOKEN" -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_tickets","arguments":{}}}'
```

## Ops

- `MCP_TOKEN` lives in `/etc/nullbox/nullbox.env` on the server (generate with
  `openssl rand -hex 32`), never in git. Rotate by changing it and restarting the `api` container.
- Caddy proxies `/mcp` to the API (`deploy/Caddyfile`).
