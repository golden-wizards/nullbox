# Retro: Golden Wizards (team-06)

How the fasthacks hackathon went for us, written right after the event from the HOT scoreboard,
the GitHub history and our Claude Code session logs. The 2-minute version is the Slidev deck in
[`docs/presentation/`](presentation/slides.md).

## Result

**16 points, second of eight teams.** The event ran from 04:43Z to 07:30Z on 2026-11-14 (167
minutes). The end moved from 08:00Z to 07:30Z during the event.

| Rank | Team | Points |
| --- | --- | --- |
| 1 | So | 17 |
| 2 | **Golden Wizards** | **16** |
| 3 | Kansai | 15 |
| 4 | Bubble Unicorns | 14 |
| 5 | Orange Ocelots | 12 |
| 6 | Kitchen Office | 11 |
| 7 | Crystal Owls | 9 |
| 8 | Ultra Pandas | 3 |

Scores are from the public scoreboard a few minutes after the end. Judge-decided checks were
still open then, so the final order can differ.

## What scored

| Time (UTC) | Check | Points | Running total |
| --- | --- | --- | --- |
| 06:27 | Deploy | 1 | 1 |
| 07:07 | SEO: Agentic Browsing 100 | 1 | 2 |
| 07:08 | AI: MCP server | 2 | 4 |
| 07:11 | SEO: Score 100 | 1 | 5 |
| 07:11 | Security: firewall | 1 | 6 |
| 07:13 | Security: rotate the leaked secret | 1 | 7 |
| 07:15 | Observability | 2 | 9 |
| 07:17 | Login: members-only page stays closed | 1 | 10 |
| 07:20 | Payments | 3 | 13 |
| 07:22 | CI/CD: automatic deployments | 2 | 15 |
| 07:26 | Security: the secret out of git | 1 | 16 |

Not scored:

- **CI/CD: tests (1) and cache (1).** Both failed with the same note from HOT: the pipeline for
  HOT's commit did not finish within 300 seconds. Our last 20 pushes took 7 min 55 s in the median.
- **Judge-decided checks (8).** Username and password (2), single sign-on (2), customer links (1),
  audit log (2) and "an agent works a ticket" (1) were all waiting for a judge at the end.

## Our approach

- **A plan first.** `docs/BATTLEPLAN.md` set the order: guardrails, deploy, quick security wins,
  CI/CD, then product upgrades. `docs/SITREP.md` held the shared state.
- **One issue, one claim, one worktree, one PR.** A claim comment on the issue (lowest comment id
  wins) kept four people, each with several Claude Code sessions, from building the same thing.
  It worked: when two sessions claimed the audit log one second apart, the later one backed out.
- **Guardrails before speed.** The repo is public, so secret scanning in CI, a local denylist and
  a `/secret-check` step came before feature work.
- **Agents in parallel.** One workflow claimed five tickets at once (login, payments, MCP,
  observability, SEO). Four of the five were merged within 15 minutes; login followed later.
- **Verify from outside, then press Check.** HOT only awards points after it has run a check
  against the live site.

In numbers: 40 pull requests merged, all between 05:56Z and 07:27Z.

## What tripped us up

1. **Merged is not scored.** Observability, SEO, payments and the MCP server were merged between
   06:40Z and 06:45Z. Production still ran the 06:27Z build, so none of it scored until a manual
   redeploy at 07:07Z. Deploys stayed manual (build here, copy images to the server) until a
   pull-based deployer was started at 07:21Z.
2. **We built against guesses.** The exact check texts were on the HOT team page from the start,
   but the session building the upgrades only read them at 07:05Z. Three upgrades then needed
   rework:
   - Observability: HOT wanted OTLP export to its own endpoint. We had built a Prometheus
     `/metrics` endpoint.
   - Payments: HOT looked for a paid checkout of one specific Stripe product, which only had a
     recurring price. Our checkout sent a one-time payment and Stripe refused it.
   - MCP: the token had to be registered with HOT before the check could connect.
3. **A slow start.** Our first point came 104 minutes into a 167-minute event. Kansai had 10
   points by then. The time went into the team workflow, guardrails and the first deploy.
4. **Our own process got in the way.**
   - A Super Linter workflow added mid-event failed every PR that touched web code, because it
     ran ESLint and Prettier against a repo that uses Biome.
   - A ruleset requiring an approving review was added while PRs were in flight.
   - Auto-merge never switched on: its job failed because `main` had no classic branch
     protection.
   - The history rewrite that removed the leaked token closed open PRs and left every clone on
     the old history, five minutes before the end.
5. **Checks raced with deploys.** A container restart during a PageSpeed run cost one SEO
   failure. Automatic check pressers in two sessions used up failures on checks that were not
   live yet (observability, payments, secret removal).
6. **CI was too slow for the last two points.** With GitHub runners queuing, a push took about
   eight minutes end to end. HOT allows five for tests and four for the cache check.
7. **One agent run was wasted.** The first workflow run stopped within a minute: its agents read
   an unrelated side message as the whole task and declined the ticket work. The second run
   quoted the original request and went through.

## What we would do differently

- **Deploy and automatic deployment first.** Every other point depends on them, and it makes
  "merged" and "live" the same thing.
- **Read the check texts before writing code,** and paste each one into its issue.
- **Make pressing Check the last step of a deploy,** owned by one session.
- **Freeze repo rules during the event.** Linters, rulesets and history rewrites go at the start
  or after the end.
- **Keep CI under five minutes** from the first workflow.
- **One deployer.** Three sessions built and shipped images in the last 20 minutes, and
  production ended on a build that is not exactly `main`.

## Loose ends after the event

- Production runs a hand-built image from before the history rewrite (audit log, OTLP export and
  the Stripe fix are in it). Redeploy from `main` to make the two match.
- Single sign-on is merged but has no GitHub OAuth app configured.
- A `deploycheck` worker account was created on the live site during verification. Delete it.
- Sign-up is open to anyone, because `SIGNUP_INVITE_CODE` is unset.
- Every clone made before 07:25Z is on the old history and needs a fresh clone.

## The deck

```sh
cd docs/presentation
npm install
npm run dev      # present locally
npm run build    # static site in dist/
```

The web image builds the deck too (`deploy/web.Dockerfile`), so every deploy of `main` serves it
at <https://team-06.fasthacks.cloud/slides/>.
