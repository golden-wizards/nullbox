---
name: sitrep
description: Pull the team's live HOT scoreboard (orga.fasthacks.cloud) and sync docs/SITREP.md and the GitHub issues with what has actually passed, failed or unlocked. Use when the user asks for the score, the status of upgrades, "where are we", or to update the SITREP/board.
---

# /sitrep

Sync the team's shared picture with what HOT actually reports. **Read-only on HOT:** never POST
to `/api/team/checks/*` (pressing Check counts toward the 3-failure limit) unless the user
explicitly asks for that specific check.

## 1. Get the token without leaking it

The HOT team code must come from the environment: `HOT_TOKEN`. If it's unset, ask the user to run
`! export HOT_TOKEN=…`, or to put it in a gitignored local file. Never write it into the repo, an
issue, a PR or a commit message, and never echo it.

## 2. Fetch only safe fields

The `/api/team` response contains secrets (deploy key, deSEC token, Stripe key), so **never print
the raw JSON**. Always filter it through `jq`:

```sh
curl -fsS -H "Authorization: Bearer $HOT_TOKEN" https://orga.fasthacks.cloud/api/team | jq '{
  points,
  checks: [.checks[] | {key, points, decided_by, locked, passed_at, failures, asked_judge,
           last: (.last_run | if . then {status, note, finished_at} else null end)}]
}'
```

Also check the live site yourself:
`curl -s -o /dev/null -w '%{http_code}' https://team-06.fasthacks.cloud/api/healthz`.

## 3. Map checks to issues

| HOT check key(s) | Issue |
|---|---|
| `deploy` | #7 |
| `ci-cd`, `ci-cache`, `tests` | #8 |
| `security-secret-rotation` | #9 |
| `security-secret-removal` | #10 |
| `security-firewall` | #11 |
| `security` (customer links) | #12 |
| `security-audit-log` | #13 |
| `login-protected`, `login-sign-up`, `login-sso` | #14 |
| `observability` | #15 |
| `seo`, `seo-agentic` | #16 |
| `payments` | #17 |
| `ai-mcp`, `ai-agent` | #18 |

If HOT reports a key that isn't in this table, flag it and add it here in the same PR.

## 4. Update the repo, through a PR

- In `docs/SITREP.md`, update the Upgrades table's **Status** column (`passed <time>`,
  `failed ×N: <short note>`, `unlocked`, `locked`) and the TL;DR, bump "Last updated", and add a
  line to the Log. Owners come from the issue assignees (`gh api repos/golden-wizards/nullbox/issues/N`).
- Branch `docs/sitrep-<yyyymmdd-hhmm>` from fresh `origin/main`, commit, run `/secret-check`, and
  open a PR. Never push to `main`.

## 5. Update the issues

- Check newly passed: comment "HOT: `<key>` passed at <time>". Close the issue only when **all**
  of its mapped checks have passed (judge items: when HOT shows them passed).
- New failure: comment with HOT's note (it is safe, it's an error message) and the failure count.
  At 2 failures, warn that one more means asking a judge.
- Run `/secret-check` on any comment text before posting it.

## 6. Report back

Keep it short: score, what changed since the last SITREP, what's unlocked and unowned, and the
best next move according to `docs/BATTLEPLAN.md`.
