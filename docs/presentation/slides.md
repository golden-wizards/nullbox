---
theme: default
title: 'Golden Wizards: Nullbox in 167 minutes'
info: Retro for the fasthacks hackathon, team-06. Full report in docs/RETRO.md.
routerMode: hash
colorSchema: light
transition: fade
mdc: true
---

<!-- markdownlint-disable MD025 MD041 -->

# Nullbox in 167 minutes

Golden Wizards, team-06

**16 points. Second of eight teams, one point behind first.**

<div class="pt-8 text-sm opacity-70">
4 people, several Claude Code sessions each, one public repo, 40 merged pull requests
</div>

---

# How we worked

- **A plan before code.** `docs/BATTLEPLAN.md` ordered the work: guardrails, deploy, quick security wins, CI/CD, product upgrades.
- **One issue, one claim, one worktree, one PR.** A claim comment on the issue decided who builds what, so people and agents never built the same thing twice.
- **Agents in parallel.** One workflow claimed five tickets at once: login, payments, MCP, observability and SEO. Four of the five were merged 15 minutes later.
- **Verify from outside, then press Check.** Points only count once HOT has run the check against the live site.

---

# When things landed

<Timeline />

<div class="text-xs opacity-70 pt-2">
Hover a bar or a dot for details. Times are UTC. Pull requests from GitHub, points from the HOT scoreboard.
</div>

---

# What is live

| Upgrade | Try it on `team-06.fasthacks.cloud` |
| --- | --- |
| Login | `/inbox` sends you to `/login`; the worker API answers 401 |
| Payments | `/pay` opens Stripe Checkout in test mode |
| MCP server | `POST /mcp` with a bearer token lists 7 ticket tools |
| Observability | traces go to HOT over OTLP; `/metrics` for Prometheus |
| SEO | `/robots.txt`, `/sitemap.xml`, `/llms.txt` |
| Security | firewall on, admin token rotated twice, leak purged from git history |
| Audit log | `GET /api/audit` |

---

# What tripped us up

1. **Merged is not scored.** Four upgrades were merged by 06:45 and earned nothing until we redeployed at 07:07. Deploys were manual until the last ten minutes.
2. **We built against guesses.** We read HOT's exact check texts only at 07:05. Observability needed OTLP export, not a metrics endpoint. Payments needed one specific Stripe product.
3. **A slow start.** Our first point came 104 minutes in. The leaders had 10 by then.
4. **Our own process got in the way.** A linter that failed every PR, an approval rule added mid-run, auto-merge that never switched on, a history rewrite that closed open PRs.
5. **CI was too slow for the last 2 points.** HOT wanted the pipeline done in 5 minutes. Ours took about 8.

---

# What we would do differently

- **Deploy and automatic deployment first.** Every later point depends on them, and an upgrade is not done until it is live.
- **Read the check texts before writing code.** They were available from the first minute.
- **Press Check as the last step of each deploy,** by one owner, so nothing sits unscored and two sessions never press at once.
- **Freeze repo rules during the event.** Change linters, rulesets and history at the start or after the end.
- **Keep CI under five minutes** from day one.

<div class="pt-6">

Still with the judges: sign-up, single sign-on, customer links, audit log and the agent demo. That is up to 8 more points.

</div>
