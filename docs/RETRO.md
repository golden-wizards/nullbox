# Retro: Golden Wizards (team-06)

How the fasthacks hackathon went for us, written right after the event from the HOT scoreboard,
the GitHub history and our Claude Code session logs. The 2-minute version is the Slidev deck in
[`docs/presentation/`](presentation/slides.md).

## Result

**17 of 26 points, second of eight teams.** The event ran from 04:43Z to 07:30Z on 2026-11-14
(167 minutes). The end moved from 08:00Z to 07:30Z during the event. We had 16 points at the
end; a judge added one more for customer links at 07:41Z.

| Rank | Team | Points |
| --- | --- | --- |
| 1 | So | 19 |
| 2 | **Golden Wizards** | **17** |
| 3 | Kansai | 16 |
| 4 | Bubble Unicorns | 14 |
| 5 | Orange Ocelots | 13 |
| 6 | Kitchen Office | 12 |
| 7 | Crystal Owls | 9 |
| 8 | Ultra Pandas | 3 |

Scores are from the public scoreboard at about 08:00Z. Two of our checks were still waiting
for a judge then (single sign-on and the agent demo), so the final number can still move.

| Outcome | Checks | Points |
| --- | --- | --- |
| Scored | 12 | 17 |
| Missed, checked by HOT | 2 | 2 |
| Missed, failed by a judge | 2 | 4 |
| Still waiting for a judge | 2 | 3 |
| **Total** | **18** | **26** |

## Point by point

Every scored upgrade had an issue from 05:52Z, 69 minutes into the event, when issues #7 to #18
were created in one batch from the battle plan. No check was missed for lack of an issue. What
differed was when each one was claimed, and how long it took to get from merged to live to
checked. All times are UTC.

### Scored

| Check | Pts | Issue | Claimed | Merged | Live | Passed |
| --- | --- | --- | --- | --- | --- | --- |
| Deploy | 1 | #7 | 06:20 | #32 at 06:30 | 06:27 | 06:27 |
| SEO: Agentic Browsing 100 | 1 | #16 | 06:31 | #37 at 06:42 | 07:07 | 07:07 |
| AI: MCP server | 2 | #18 | 06:31 | #36 at 06:45 | 07:07 | 07:08 |
| SEO: Score 100 | 1 | #16 | 06:31 | #37 at 06:42 | 07:07 | 07:11 |
| Security: firewall | 1 | #11 | 07:01 | #60 at 07:10 | 07:09 | 07:11 |
| Security: rotate the leaked secret | 1 | #9 | 06:47 | #45 at 06:56 | 07:07 | 07:13 |
| Observability | 2 | #15 | 06:31 | #35 at 06:40 | 07:14 | 07:15 |
| Login: members-only page | 1 | #14 | 06:31 | #41 at 07:12 | 07:16 | 07:17 |
| Payments | 3 | #17 | 06:31 | #38 at 06:42, #62 at 07:11 | 07:12 | 07:20 |
| CI/CD: automatic deployments | 2 | #8 | 06:07 | #30 at 06:26, #66 at 07:18 | 07:21 | 07:22 |
| Security: the secret out of git | 1 | #10 | 07:24 | force-push at 07:25 | 07:25 | 07:26 |
| Security: customer links | 1 | #12 | no claim comment | #53 at 07:10 | 07:19 | 07:41 (judge) |

Notes on the ones that did not go straight through:

- **Deploy.** Done by hand on the server; the PR with the Dockerfiles merged three minutes after
  the check passed. One failed attempt before the pass.
- **SEO: Score 100.** Two failures first. One was PageSpeed failing to reach the site while a
  container was being restarted for the 07:07 deploy.
- **Security: rotate.** The server had a new token from the first deploy at 06:27, and the
  hardcoded default left the code at 06:56. The point needed two more things nobody had done: give
  HOT the new token, then rotate a second time when HOT asked. Both happened at 07:13.
- **Observability.** The first version (#35) was a Prometheus endpoint, which is not what HOT
  checks. HOT wants OTLP export to its own endpoint. That was built between 07:07 and 07:14 and
  deployed from a branch; it reached `main` as #74 at 07:27. One failure was recorded when an
  automatic presser ran the check before the export existed.
- **Payments.** The first deploy at 07:07 could not open a checkout: HOT looks for a purchase of
  one specific Stripe product, which only has a recurring price, and our code sent a one-time
  payment. #62 fixed that. The check then needed a person to complete a test payment in a
  browser. Two failures were recorded before the payment existed.
- **CI/CD: automatic deployments.** The deploy workflow (#66) merged at 07:18 but has never
  worked: every run fails because `DEPLOY_SSH_KEY` is not set in the production environment. The
  point came from a small pull-based deployer started on a development machine at 07:21, which
  put HOT's commit live in about 30 seconds.
- **Security: the secret out of git.** Five failures, all from checks pressed before the history
  was rewritten. The rewrite came five minutes before the end.
- **Security: customer links.** Judge-decided, and passed after the end: a customer token now only
  opens its own ticket.

### Missed

| Check | Pts | Issue | What happened |
| --- | --- | --- | --- |
| CI/CD: tests run automatically | 1 | #8 | Requested at 07:23. HOT's commit did not get through our pipeline within 300 seconds. |
| CI/CD: fast builds with a cache | 1 | #8 | Requested at 07:29, 45 seconds before the end. Same failure: our last 20 pushes took 7 min 55 s in the median, 11 min 8 s at worst. |
| Login: username and password | 2 | #14 | Failed by a judge at 07:42. Sign-up accepted an 11-character password and two known-breached ones. The bar was a 12-character minimum and a Have I Been Pwned check; ours was 8 to 200 characters. |
| Security: audit log | 2 | #13 | Failed by a judge at 07:54 on timing. HOT left its trail at 07:17:37; the audit log went live at 07:25:35, so the trail is not in it. |

Why each one was missed:

- **Tests and cache (2 points).** These could only be requested once automatic deployment
  passed, which was 07:22, eight minutes before the end. HOT runs them one at a time and each
  takes up to five minutes, so there was time for one attempt each and no room for a slow run.
  The pipeline was slow for two reasons: GitHub runners were queuing, and every push ran CodeQL,
  Super Linter and the image builds next to the tests.
- **Username and password (2 points).** The check text says the judge "tries a short password
  and a leaked one, and both are refused at sign-up". The login work was specified at 06:30 from
  the issue text, which did not mention it, and the check texts were first read at 07:05. The
  rest of the feature (argon2id hashes, server-side sessions, logout) was in place.
- **Audit log (2 points).** The issue was the last scored one to be claimed (07:12) because it
  was planned to follow login. The code merged at 07:23 and went live at 07:25. The check had
  been requested at 07:17 to get it into the judge queue, before the audit log existed, and HOT
  kept that first trail. The judge's note says the implementation is strong and that a fresh
  request "then passes", but HOT refuses new requests now that the event has closed.

### Still waiting for a judge

| Check | Pts | Issue | State |
| --- | --- | --- | --- |
| Login: single sign-on | 2 | #14 | The code merged at 07:16 (#67), but no GitHub OAuth app was created, so sign-in with GitHub is switched off on the live site. Expect a fail. |
| AI: an agent works a ticket | 1 | #18 | The MCP server, `docs/MCP.md` and a demo script are live. The judge needs our MCP token to connect. |

### What the timeline says

- **05:52 to 06:31: nothing scored was being built except deploy and CI.** The issues existed
  for 39 minutes before the five product upgrades were claimed in one go.
- **06:40 to 07:07: 8 points were merged but not live.** Observability, SEO, payments and the MCP
  server waited 22 to 27 minutes for a deploy.
- **07:07 to 07:26: 15 of our 17 points.** Once someone was deploying and pressing Check, a check
  passed every two minutes on average.
- **The four security issues were claimed late** (06:47, 07:01, 07:12 and 07:24), although the
  firewall and the rotation were the cheapest points on the board.

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

- The deploy workflow has never run to the end: `DEPLOY_SSH_KEY` is not set in the production
  environment. Until it is, production only changes when someone ships images by hand.
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
