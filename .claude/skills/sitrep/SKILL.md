---
name: sitrep
description: Pull the team's HOT checks and the public leaderboard (orga.fasthacks.cloud), press Check for changes that are live but not yet scored, and sync docs/SITREP.md and the GitHub issues. Use when the user asks for the score, the leaderboard, the status of upgrades, "where are we", or to update the SITREP/board, and as part of the regular checks in CLAUDE.md.
---

# /sitrep

Sync the team's shared picture with what HOT and the public leaderboard actually report. The
only write to HOT is pressing **Check** (`POST /api/team/checks/<key>`), and only under the rules
in step 2b, because failures count toward the 3-failure limit. Never call any other write endpoint.

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

Then the **public leaderboard** (no token needed, also shown at <https://orga.fasthacks.cloud/>):

```sh
curl -fsS https://orga.fasthacks.cloud/api/scoreboard | jq -r '
  "time left: \(((.ends_at|sub("\\.[0-9]+Z$";"Z")|fromdate) - (.now|sub("\\.[0-9]+Z$";"Z")|fromdate))/60|floor) min",
  (.teams | to_entries[] | "\(.key+1). \(.value.name)  \(.value.points) pts  [\(.value.passed|join(", "))]"),
  "recent:", (.log[0:8][] | "  \(.passed_at[11:16]) \(.team_name): \(.kind) \(.check)")'
```

## 2b. Press Check for what's live (only when it's safe)

When a merged and deployed change should pass a check that hasn't passed yet:

1. Verify it from outside the server yourself (e.g. `curl` the live URL), using what HOT says it
   checks (`.checks[].last_run.note` and the team page describe it).
2. If `failures` for that key is **0 or 1**, press Check:
   `curl -fsS -X POST -H "Authorization: Bearer $HOT_TOKEN" https://orga.fasthacks.cloud/api/team/checks/<key>`
   then re-read `/api/team` (filtered as above) for the result. If `failures` is **2**, don't press
   it: ask the user, because a third failure means asking a judge.
3. Comment the result on the mapped issue (step 5).

## 3. Map checks to issues

| HOT check key(s) | Issue |
| --- | --- |
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
- Keep SITREP markdownlint-clean (Super Linter checks every changed file): table pipes get a space
  on both sides (`| a | b |`, separator `| --- |`), and URLs go in `<…>` or link syntax, never bare.
- Branch `docs/sitrep-<yyyymmdd-hhmm>` from fresh `origin/main`, commit, run `/secret-check`, and
  open a PR. Never push to `main`.

## 5. Update the issues

- Check newly passed: comment `HOT: <key> passed at <time>`. Close the issue only when **all**
  of its mapped checks have passed (judge items: when HOT shows them passed).
- New failure: comment with HOT's note (it is safe, it's an error message) and the failure count.
  At 2 failures, warn that one more means asking a judge.
- Run `/secret-check` on any comment text before posting it.

## 6. Report back

Keep it short: time left, our rank and points against the leader, what changed since the last
SITREP, anything live but not yet checked, what's unlocked and unowned, and the best next move
according to `docs/BATTLEPLAN.md`. Remind the user to pull `main` if it moved.
