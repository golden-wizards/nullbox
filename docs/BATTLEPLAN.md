# BATTLEPLAN: Golden Wizards (team-06)

**Objective:** score as many of the **26 points** on HOT as we can, without leaking a secret
or breaking the live site along the way. Current score: 0. Nothing scores until **Deploy** passes.

State of play is in `docs/SITREP.md`; this file is the plan. Tasks are GitHub issues, grouped
into milestones.

## Principles

1. **Deploy first, everything else second.** Deploy is 1 point but it unlocks the other 25.
2. **Guardrails before speed.** The repo is public. One leaked token costs more than any upgrade.
3. **Cheap points early.** Several 1-pointers are 15-minute jobs once we're deployed.
4. **Ship through PRs, keep `main` deployable.** CI deploys `main`, so a broken merge breaks the score.
5. **Verify from outside before pressing Check.** 3 failures and we have to ask a judge.

## Phases

| Phase | Milestone | Goal | Points | Issues |
| --- | --- | --- | --- | --- |
| 0 | **M0 Guardrails** | Secret hooks + CI scan, GitHub security settings, `main` ruleset, team workflow | — | #2 #3 #4 #5 #6 |
| 1 | **M1 Deploy** | HTTPS on team-06.fasthacks.cloud: web at `/`, API at `/api`, healthz green | 1 | #7 |
| 2 | **M3 quick wins** | Kill the debug console, firewall on, rotate the leaked admin token | 3 | #9 #11 |
| 3 | **M2 CI/CD** | Tests on every PR, cached builds, auto-deploy on merge | 4 | #8 |
| 4 | **M3 Security** | Purge `.env` from history, customer links, audit log | 4 | #10 #12 #13 |
| 5 | **M4 Product** | Login + SSO, observability, SEO, payments, MCP + agent | 14 | #14–#18 |

Phase 0 and phase 1 run **in parallel**: guardrails are mostly settings and config, and deploy is
mostly server work. Phase 2 can start the moment Deploy is green.

## Points map

| Area | Upgrades (pts) | Total | Checked by |
| --- | --- | --- | --- |
| Deploy | Deploy (1) | 1 | HOT |
| CI/CD | auto deploy (2), cache (1), tests (1) | 4 | HOT |
| Login | members-only page (1), username+password (2), SSO (2) | 5 | HOT, judge |
| Security | rotate (1), out of git (1), customer links (1), firewall (1), audit log (2) | 6 | HOT, judge |
| Observability | (2) | 2 | HOT |
| SEO | Score 100 (1), Agentic Browsing 100 (1) | 2 | HOT |
| Payments | (3) | 3 | HOT |
| AI | MCP server (2), agent works a ticket (1) | 3 | HOT, judge |
| | | **26** | |

**Best points per hour after Deploy:** firewall + debug console, rotate, and CI/CD (4 points from
one workflow). Then Login (5), which also unblocks the audit log. Payments (3) and MCP (2) are the
biggest single items in M4.

## Dependencies

```text
Deploy ──┬── firewall, rotate, history purge, CI/CD
         └── Login ── audit log
                  └── members-only page, SSO
CI/CD ── required checks in the main ruleset
Rotate ── history purge (rotate first, purge second)
```

## Risks

| Risk | Mitigation |
| --- | --- |
| A secret lands in the public repo | Hooks + CI gitleaks + GitHub push protection; `/secret-check`; rotate first if it happens |
| A bad merge takes the site down | Required CI checks, smoke test in the deploy job, small PRs |
| The 2 GB server OOMs compiling Rust | Build images in CI, the server only pulls and runs them |
| Docker ports bypass ufw | Only Caddy publishes 80/443; everything else stays internal |
| History rewrite collides with open work | Freeze window, one admin pushes, everyone re-clones |
| Five people on the same thing | Assign the issue before starting; `/sitrep` keeps the board honest |

## Who's on what

The issue assignees are the source of truth. Pick from the lowest open phase first.
