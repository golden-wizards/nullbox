# Nullbox: Golden Wizards (team-06), fasthacks

Hackathon repo. We level up Nullbox (Rust/axum API + React/Vite web) toward production and score
upgrades on HOT (https://orga.fasthacks.cloud/team). Read first:

- `docs/BATTLEPLAN.md`: what we're trying to achieve and in what order
- `docs/SITREP.md`: current state, where things are, known problems
- `README.md` → "Extension seams": where each upgrade plugs into the code
- Issues and milestones (M0 Guardrails → M4 Product upgrades) are the task list

## Rule zero: this repo is public

HOT can only read public repos, so **everything here is public**, including issues, PR text,
commit messages and Actions logs. Never commit or post:

- the HOT team login code (four hyphenated words) or anything from the team page marked secret
  (deSEC token, deploy key, Stripe keys)
- SSH private keys, `.env` files, `ADMIN_TOKEN` values (`nbx_…`), the server access code

Secrets live on the team page, in `/etc/nullbox/nullbox.env` on the server (0600), and in the
GitHub `production` environment. Nowhere else. **Run `/secret-check` before every commit, push and
PR.** If a secret leaks anyway: rotate it first, then tell the team, then clean history.

## Workflow

1. **Claim an issue:** assign yourself (or comment "taking this") before you start. No issue? Open one.
2. **Branch from fresh `main`:**
   `git fetch origin && git switch -c <type>/<issue>-<slug> origin/main`
   - types: `feat` `fix` `sec` `ci` `infra` `docs` `chore`
   - e.g. `infra/7-caddy-compose`, `sec/9-rotate-admin-token`
3. **Commit small and often.** Imperative subject (≤72 chars). Reference the issue in the body.
   Claude-assisted commits end with a `Co-Authored-By:` trailer.
4. **Run the checks** for what you touched:
   - Rust: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
   - Web: `pnpm lint`, `pnpm test`, `pnpm build`
   - Always: `/secret-check`
5. **Open a PR to `main`.** Title = what it does. Body: summary, how it was tested, `Closes #N`.
   Keep PRs small, since a 30-minute PR beats a 3-hour one at a hackathon.
6. **Squash-merge** once CI is green; the branch gets deleted. Merge your own PR if nobody objects
   within a reasonable time, and ask in chat for anything risky (server, secrets, history).

### Never

- Push directly to `main` (everything goes through a PR) or force-push `main`. The only planned
  exception is the coordinated `.env` history purge (#10).
- Force-push someone else's branch. Your own branch: `--force-with-lease` only.
- Set a repo-local `user.name`/`user.email`. Use your global git identity.
- Run two Claude sessions in the same clone. Use `git worktree add` for parallel work.

## Server and HOT

- `ssh team-06` (user `deploy`, passwordless sudo). Setup: team page → Server & domain.
- Server changes must be reproducible from the repo (compose files, config templates). If you
  change something by hand, add a line to the SITREP log.
- Don't press **Check** on HOT until you've verified the result from outside the server. After 3
  failures, use **Ask a judge**. Pressing Check is the only write to HOT that counts against us.
- `/sitrep` pulls the scoreboard and syncs `docs/SITREP.md` and the issues.
