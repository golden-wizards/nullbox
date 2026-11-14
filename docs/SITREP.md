# SITREP — Golden Wizards (team-06)

Shared situation report for the team. Update it as you learn things; keep it **secret-free**
(this repo is public — HOT can only see public repos).

_Last updated: 2026-11-14_

## TL;DR

- Server is up and we can SSH in. Onboarding on HOT is **done** (SSH verified).
- Nothing is deployed yet. **Deploy unlocks every other upgrade — do it first.**
- The repo and server ship with planted security problems; each one is a scoring upgrade.

## Where things are

| What | Value |
|---|---|
| Team page / scoreboard (HOT) | https://orga.fasthacks.cloud/team |
| Live URL (what HOT checks) | https://team-06.fasthacks.cloud |
| Server | `team-06.fasthacks.cloud` → `52.195.229.31`, user `deploy` (passwordless sudo) |
| Server spec | 2 vCPU, 2 GB RAM, 58 GB disk, Ubuntu 24.04, Docker + Compose v5, Tokyo (AWS Lightsail) |
| Repo | https://github.com/golden-wizards/nullbox (fork of `tweidinger/nullbox`) |
| HOT's GitHub bot | `fasthacks-bot` — already has push access |
| DNS | deSEC, zone `team-06.fasthacks.cloud`, DNSSEC on. Token is on the team page (Server & domain → Advanced) |

### Getting onto the server

1. Sign in to the team page with the team token (ask a teammate — **never commit it**).
2. Server & domain → copy the one-line setup command → run it on your laptop.
3. `ssh team-06`

Deploy key for GitHub Actions: on the team page under "Keys for deploying". Store it as a
repository **secret**, never in a file.

## Upgrades and points

Checked by HOT (automatic), a judge, or both. Deploy must pass before the rest unlock.

| Upgrade | Pts | Checked by | Owner | Status |
|---|---|---|---|---|
| **Deploy** | 1 | HOT | | not started |
| CI/CD: automatic deployments | 2 | HOT | | locked |
| CI/CD: fast builds with a cache | 1 | HOT | | locked |
| CI/CD: tests run automatically | 1 | HOT | | locked |
| Login: members-only page stays closed | 1 | HOT | | locked |
| Login: username and password | 2 | judge | | locked |
| Login: single sign-on (SSO) | 2 | judge | | locked |
| Security: rotate the leaked secret | 1 | HOT | t-okamoto1122 (#9) | code fix in PR #45; server rotation waits on Deploy |
| Security: the secret out of git | 1 | HOT | | locked |
| Security: customer links | 1 | judge | | locked |
| Security: firewall | 1 | HOT | | locked |
| Security: audit log | 2 | HOT, then judge | | locked |
| Observability | 2 | HOT | | locked |
| SEO: Score 100 | 1 | HOT | | locked |
| SEO: Agentic Browsing 100 | 1 | HOT | | locked |
| Payments | 3 | HOT | | locked |
| AI: MCP server | 2 | HOT | | locked |
| AI: an agent works a ticket | 1 | judge | | locked |

Put your name in **Owner** when you pick something up, to avoid double work.

### Deploy: what HOT checks

> HOT opens `/` on your live URL, and `/api/healthz` answers 200 with `{"status":"ok"}`.

So we need HTTPS on `team-06.fasthacks.cloud` serving the web app at `/` and the API under
`/api` from the same origin (build the web app with `VITE_API_BASE_URL=` empty). Currently
nothing listens on 80/443.

Tip from HOT: if a check fails 3 times, press **Ask a judge** instead of fighting it.

## Known problems (found so far)

1. **Leaked admin token in git history.** Commit `4351700` added `.env` with a real
   `ADMIN_TOKEN`; `e9f15e5` deleted the file, but the value is still in history (and "was shared
   in #general"). Fix = generate a new token (rotate) **and** rewrite history to remove `.env`.
   History rewrite means a force-push to `main`: coordinate before doing it, and everyone
   re-clones afterwards.
2. **Debug console exposed to the internet.** `debug-console.service` runs
   `python3 -m http.server 8888 --bind 0.0.0.0` from `/opt/debug-console`. Confirmed reachable
   from outside (HTTP 200 on `:8888`). Stop/disable it or bind it to localhost.
3. **Firewall off.** ufw is installed but disabled. Note: ports published by Docker
   (`ports:` in compose) **bypass ufw** — bind containers to `127.0.0.1` behind a reverse proxy,
   or handle the `DOCKER-USER` chain.
4. **Worker endpoints have no auth.** `X-Actor-Name` is whatever the client sends;
   `policy::can` always returns `true`. See "Extension seams" in the README.
5. **Customer ticket links** are a bearer token in the URL with no recovery (known MVP limit) —
   likely what "Security: customer links" is about.

Open ports on the server right now: 22 (ssh), 8888 (debug console), 53 on localhost only.

## Codebase pointers

See `README.md` → "Extension seams" for where each upgrade plugs in. Short version:

- API: Rust/axum in `crates/api`, domain rules in `crates/domain`, SQLite via sqlx (`migrations/`, `.sqlx/`).
- Web: React + Vite in `apps/web`; all fetches go through `apps/web/src/api/client.ts`.
- Checks: `cargo test`, `cargo clippy --all-targets`, `cargo fmt --check`, `pnpm test`, `pnpm lint`, `pnpm build`.

## Suggested next steps

1. Deploy: Dockerfile(s) + `docker-compose.yml` + a reverse proxy with automatic TLS
   (e.g. Caddy) on the server. Then press **Check** on Deploy.
2. Quick wins right after: kill the debug console, enable the firewall, rotate the admin token.
3. CI/CD via GitHub Actions using the deploy key as a secret (with cargo/pnpm caching).
4. Split the rest across people.

## Log

- 2026-11-14 — SSH access set up and verified on HOT; repo cloned; this SITREP written.
- 2026-11-14 — `main` ruleset applied (#5): PRs only, required checks `secret-scan`/`rust`/`web`, admins bypass; classic protection removed; merge commits disabled.
- 2026-11-14 — #9: hardcoded leaked `ADMIN_TOKEN` default removed (PR #45). The token on the server still has to be rotated after Deploy.
