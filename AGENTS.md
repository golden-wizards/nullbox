# Nullbox: Golden Wizards (team-06), fasthacks

Hackathon repo. We level up Nullbox (Rust/axum API + React/Vite web) toward production and score
upgrades on HOT (<https://orga.fasthacks.cloud/team>). Read first:

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

## Workflow: one issue, one claim, one worktree, one PR

Several people (and several Claude sessions per person) work at the same time. This protocol
keeps two of us from building the same thing. **Follow it for every issue, in order.** No issue
for what you want to do? Open one first.

Commands use `gh api` (REST) because it works everywhere, including setups where `gh issue …`
and GraphQL are blocked. Set these once per shell:

```sh
R=repos/golden-wizards/nullbox
ME=$(gh api user --jq .login)
```

### 1. Pick a free issue

An issue is **free** when it is open, has **no assignee** and **no `in-progress` label**. Take
the lowest milestone first (see `docs/BATTLEPLAN.md`). Issues labelled `repo-settings` are for
an org admin.

```sh
gh api "$R/issues?state=open&assignee=none&per_page=100" \
  --jq '.[] | select(.pull_request|not) | select([.labels[].name]|index("in-progress")|not)
        | "#\(.number) [\(.milestone.title // "-")] \(.title)"'
```

If the user named an issue, check that it's free. If it isn't, stop and tell them who holds it.

### 2. Claim it (before writing any code)

Choose the branch name first: `<type>/<issue>-<slug>` (types: `feat` `fix` `sec` `ci` `infra`
`docs` `chore`), e.g. `infra/7-caddy-compose`.

```sh
N=7; B=infra/7-caddy-compose
gh api $R/issues/$N/assignees -f "assignees[]=$ME"
gh api $R/issues/$N/labels -f 'labels[]=in-progress'
CID=$(gh api $R/issues/$N/comments -f body="🔒 Claimed by @$ME, branch \`$B\`." --jq .id)
```

**Then verify you won.** Two sessions can claim at the same moment, and both may be the same
GitHub user. The claim comment with the **lowest id** wins:

```sh
gh api $R/issues/$N/comments --jq '[.[] | select(.body|startswith("🔒 Claimed")) | .id] | min'
```

If that isn't `$CID`, you lost: delete your comment (`gh api -X DELETE $R/issues/comments/$CID`),
leave the assignee and label alone, and pick another issue.

### 3. Work in a dedicated worktree

The main clone stays on `main` and is only used to create worktrees. Never edit in it. Each issue
gets its own worktree next to the clone:

```sh
git fetch origin
git worktree add -b "$B" "../nullbox-worktrees/$N-<slug>" origin/main
cd "../nullbox-worktrees/$N-<slug>"
```

Then do all the work, and run Claude, from inside that worktree.

- Gitignored files are **not** shared between worktrees. Link your secret denylist:
  `ln -s "$(git worktree list | head -1 | cut -d' ' -f1)/.secrets.local" .secrets.local`.
  Run `pnpm install` per worktree (the pnpm store is shared, so it's fast).
- Optional, to save disk: share one Rust build dir with
  `export CARGO_TARGET_DIR=<main clone>/target`. Parallel builds then wait on each other.

### 4. Build it

- Commit small and often. Imperative subject (≤72 chars), `Refs #N` in the body.
  Claude-assisted commits end with a `Co-Authored-By:` trailer.
- Push your branch early (`git push -u origin "$B"`) so the work is visible and backed up.
- Stay in scope. Found something else? Open a new issue, don't fix it here.
- Long task? Comment progress on the issue now and then, so the claim is visibly alive.

### 5. Finish: checks, then a PR

1. Rebase onto the latest main if it moved: `git fetch origin && git rebase origin/main`.
2. Run the checks for what you touched:
   - Rust: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
   - Web: `pnpm lint`, `pnpm test`, `pnpm build`
   - Always: `/secret-check` on the outgoing commits **and** on the PR body
3. Push and open the PR:

   ```sh
   git push -u origin "$B"   # --force-with-lease if you rebased
   gh api $R/pulls -f title="<what it does>" -f head="$B" -f base=main -F body=@/tmp/pr.md
   ```

   Body: summary, how it was tested, and `Closes #N` on its own line (that auto-closes the issue
   on merge). Keep PRs small, since a 30-minute PR beats a 3-hour one at a hackathon.
4. Comment the PR link on the issue. Leave the `in-progress` label on until the PR is merged.
5. **Watch CI and fix it until it's green** (next section). Auto-merge (`auto-merge.yml`)
   squash-merges the PR once the required checks pass. Ask in chat before merging anything
   risky (server, secrets, history), or add the `no-automerge` label.

### 5b. Watch CI until green (the PR author's job, every time)

A PR isn't done when it's opened. It's done when it's green and merged. Right after opening the
PR, **and after every push to it**, start the watcher:

```sh
scripts/pr-watch.sh <PR>      # polls every 30 s; exits on the first failed check
```

Claude sessions: run it as a **background** command (`run_in_background`), so you get notified
when it exits. Don't poll it in a foreground loop. Then act on the exit code:

| Exit | Meaning | Do |
| --- | --- | --- |
| `0` | all checks passed (or already merged) | Done. Auto-merge handles the rest. Clean up (step 6) after the merge. |
| `1` | a check failed. Its annotations and error log lines are printed. | Run the fix loop below. |
| `2` | merge conflict, or PR closed | `git fetch origin && git rebase origin/main`, resolve, `push --force-with-lease`, watch again |
| `3` | timed out (45 min) | Look at the PR's checks page. If a check is stuck, re-run it or ask in chat. |

**Fix loop** (in your worktree, on your branch):

1. Read the failure. If the output isn't enough, open the job link or reproduce it locally with
   the same command the job runs (see `.github/workflows/*.yml`).
2. **Is it your change?** If the same check is also red on `main`
   (`gh api "$R/commits/main/check-runs" --jq '.check_runs[] | "\(.name): \(.conclusion)"'`),
   it isn't your bug. Don't fix it in your PR: comment on the PR, tell the team, and open or point
   to an issue. If it looks flaky, re-run once:
   `gh api -X POST "$R/actions/jobs/<job-id>/rerun"`.
3. Fix the **cause**, run the local checks and `/secret-check`, and commit (`Fix CI: <what>`).
4. `git push` (with `--force-with-lease` if you rebased), then start the watcher again.
5. Repeat until it exits `0`. After **3 failed attempts on the same failure**, stop: comment what
   you tried on the PR and ask a human.

While fixing:

- **Never weaken the checks to get green.** No disabling lint rules, skipping or deleting tests,
  `continue-on-error`, `#[allow]`/`biome-ignore` without a reason, or allowlisting a secret.
  Changing CI config belongs in its own PR, agreed with the team.
- Stay on your own PR. Another person's (or another session's) red PR is theirs: comment, don't push.
- If the fix changed the scope, update the PR body so it still says what the PR does.

### 6. Clean up after merge

```sh
cd <main clone> && git switch main && git pull --ff-only
git worktree remove "../nullbox-worktrees/$N-<slug>"
git branch -D "$B"     # -D because squash merges aren't ancestors of main
gh api -X DELETE "$R/issues/$N/labels/in-progress"
```

### Releasing a claim

Giving up or blocked? Don't sit on it. Comment `🔓 Released by @$ME: <why / what's done>`, remove
yourself as assignee (`gh api -X DELETE $R/issues/$N/assignees -f "assignees[]=$ME"`), remove
the `in-progress` label, and push any useful work on the branch, mentioning it in the comment.

**Stale claims:** if a claimed issue has had no commits, comments or PR for ~2 hours, ask the
holder in chat. Take it over only after they agree (or the team does), and say so on the issue.

### Never

- Push directly to `main` (everything goes through a PR) or force-push `main`. The only planned
  exception is the coordinated `.env` history purge (#10).
- Start coding before your claim has won, or work on an issue someone else holds.
- Force-push someone else's branch. Your own branch: `--force-with-lease` only.
- Set a repo-local `user.name`/`user.email`. Use your global git identity.
- Run two Claude sessions in the same worktree, or edit in the main clone.

## Server and HOT

- `ssh team-06` (user `deploy`, passwordless sudo). Setup: team page → Server & domain.
- Server changes must be reproducible from the repo (compose files, config templates). If you
  change something by hand, add a line to the SITREP log.
- **Points only count once HOT has run the check.** Merging and deploying isn't enough: the
  leaderboard updates only after someone presses **Check** on the team page and it passes.
- Don't press Check until you've verified the result from outside the server. After 3 failures,
  use **Ask a judge**. Pressing Check is the only write to HOT that counts against us.
- `/sitrep` pulls our checks and the public leaderboard and syncs `docs/SITREP.md` and the issues.

## Regular checks (everyone, all day)

The event has a hard end (`ends_at` on the leaderboard). Run this loop **at the start of every
work session, after every merge to `main`, and at least every ~30 minutes:**

1. **Pull `main`.** In the main clone: `git switch main && git pull --ff-only`. In each of your
   worktrees: `git fetch origin && git rebase origin/main` (push with `--force-with-lease`
   afterwards if the branch was already pushed). Small, frequently rebased branches merge cleanly,
   and stale ones cause the conflicts.
2. **Check the leaderboard** at <https://orga.fasthacks.cloud/> (public JSON:
   `curl -fsS https://orga.fasthacks.cloud/api/scoreboard`). Look at our rank and points, the time
   left, and what the leading teams have passed (if they did it, it's doable).
3. **Press Check for anything that's live but not yet scored.** When a PR for a scored issue is
   merged and deployed, the PR author verifies it from outside and then presses Check for its key
   (team page, or `/sitrep`). Note the result on the issue. Don't leave points sitting un-checked.
4. **Run `/sitrep`** (or at least look at the team page) and fix anything stale: claims with no
   activity, failed checks, newly unlocked upgrades nobody owns.
