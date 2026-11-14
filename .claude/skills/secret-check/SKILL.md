---
name: secret-check
description: Scan staged changes, outgoing commits, or text about to be posted (PR body, issue, comment) for secrets before committing, pushing or publishing. Use before every git commit, git push, PR or issue in this public repo, and whenever the user asks "is this safe to commit/push/post".
---

# /secret-check

This repo is public. Before anything leaves your machine, check it for secrets. **Never print a
secret value while checking:** report the file, line and rule only.

## 1. Pick the scope

- Before a commit: the **staged** changes (`git diff --cached`). If nothing is staged, check
  what the user is about to stage.
- Before a push or PR: the **outgoing** commits, `origin/main..HEAD` (or `@{u}..HEAD`).
- Before posting text (PR body, issue, comment): write the text to a temp file and scan that.
- If the user names a range or path, use that.

## 2. Run gitleaks with the repo rules

The rules live in `.gitleaks.toml` (gitleaks defaults plus `nbx_` admin tokens and the HOT team
code). Use `gitleaks` if it's on PATH, otherwise `nix run nixpkgs#gitleaks --`.

```sh
gitleaks git --staged --config .gitleaks.toml --redact --no-banner -v        # staged
gitleaks git --log-opts="origin/main..HEAD" --config .gitleaks.toml --redact --no-banner -v   # outgoing
gitleaks dir <file> --config .gitleaks.toml --redact --no-banner -v          # text/file
```

Always pass `--redact`. Exit code 1 means leaks were found.

## 3. Check the local denylist of exact values

`.secrets.local` (gitignored, one literal value per line) holds the real team code, deSEC token
and so on. This catches exact values that no regex knows about. **Never `cat` it, and never print
matching lines.**

```sh
[ -f .secrets.local ] && git diff --cached -U0 | grep '^+' | grep -qF -f .secrets.local && echo "DENYLIST HIT (staged)"
[ -f .secrets.local ] && git log -p origin/main..HEAD | grep '^+' | grep -qF -f .secrets.local && echo "DENYLIST HIT (outgoing)"
```

To find where a hit is without printing it, use
`git diff --cached --name-only | xargs grep -lF -f .secrets.local`. If `.secrets.local` is missing,
say so and suggest creating it. The user fills it in themselves, so don't ask them to paste values
into the chat.

## 4. Check for forbidden files

Flag any staged or outgoing file that matches: `.env`, `.env.*` (except `.env.example`), `*.pem`,
`*.key`, `id_rsa*`, `id_ed25519*`, `*.p12`, `.secrets.local`, `*.db`.

```sh
git diff --cached --name-only --diff-filter=A | grep -E '(^|/)(\.env(\..+)?|\.secrets\.local|id_(rsa|ed25519)[^/]*|[^/]+\.(pem|key|p12|db))$' | grep -v '\.env\.example$'
```

## 5. Report

- **Clean:** one line, e.g. "secret-check: clean (staged, gitleaks + denylist + filenames)".
- **Hit, not pushed yet:** stop. List file:line:rule (redacted), unstage or fix, and don't commit
  until a re-run is clean. A test fixture that looks like a secret should get an obviously fake
  value, or a narrow path allowlist in `.gitleaks.toml`. Never allowlist a whole rule.
- **Hit already pushed, or already in a public issue/PR:** the value is burned. Tell the user to
  **rotate it first** (team page / server env / GitHub secret), then tell the team, then clean
  history (see issue #10). Deleting the commit or comment alone isn't enough.
