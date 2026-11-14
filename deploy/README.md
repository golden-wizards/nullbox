# Deploy

Production runs on `team-06` (`ssh team-06`) from `/opt/nullbox/deploy` (`compose.yml` + `Caddyfile`).

- `api`: Rust API. SQLite lives on the `data` volume. Config and secrets come from `/etc/nullbox/nullbox.env`
  on the server (root, 0600, **never in git**: `CORS_ALLOWED_ORIGINS`, `PUBLIC_WEB_URL`, `ADMIN_TOKEN`, `RUST_LOG`,
  and for payments `STRIPE_SECRET_KEY`, `STRIPE_WEBHOOK_SECRET`, optional `STRIPE_PUBLISHABLE_KEY`; see `.env.example`).
  Login: optional `SIGNUP_INVITE_CODE`; GitHub SSO: `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET` (both unset = off), optional `GITHUB_ALLOWED_USERS`.
- `web`: Caddy with the built web app. It handles TLS for team-06.fasthacks.cloud, serves `/` (SPA
  fallback) and proxies `/api/*` to `api:3000`. `/inbox*` is members-only: Caddy asks `/api/auth/gate`
  (`forward_auth`) and sends visitors without a session to `/login`. Only this container publishes ports (80/443).

Automatic deploy: `.github/workflows/deploy.yml` runs on every push to `main`. CI runs in parallel with building
and pushing `ghcr.io/golden-wizards/nullbox-{api,web}:<sha>` to GHCR. Once both pass it ships `compose.yml` and
`Caddyfile`, writes the image refs into `/opt/nullbox/deploy/.env`, restarts, and fails unless `/api/healthz`
answers `{"status":"ok"}`. The Dockerfiles are layered for the GitHub Actions cache: a web-only commit reuses the
whole api build, and a Rust change recompiles only the workspace crates (dependencies come from a cargo-chef layer).
It needs the `DEPLOY_SSH_KEY` secret (and optionally a `DEPLOY_KNOWN_HOSTS` variable) in the GitHub `production`
environment. Redeploy by hand with Actions → deploy → Run workflow.

Manual deploy (fallback): build the images where there's CPU, ship them, restart. A manual deploy must clear the
image refs from `.env`, or compose keeps running the last automatic release.

```sh
docker build -f deploy/api.Dockerfile -t nullbox-api:latest .
docker build -f deploy/web.Dockerfile -t nullbox-web:latest .
# one image per save: a multi-image podman archive loads wrongly on Docker
for i in api web; do docker save "nullbox-${i}:latest" | gzip -1 | ssh team-06 "gunzip | sudo docker load"; done
# (podman: images are named localhost/nullbox-*, so retag on the server)
scp deploy/compose.yml deploy/Caddyfile team-06:/tmp/ && ssh team-06 'sudo mv /tmp/compose.yml /tmp/Caddyfile /opt/nullbox/deploy/'
ssh team-06 'cd /opt/nullbox/deploy && sudo docker compose up -d --no-build --force-recreate'
curl -fsS https://team-06.fasthacks.cloud/api/healthz   # {"status":"ok"}
```

Server hardening (#11: debug console off, ufw allowing only 22/80/443) is in `harden.sh`. Safe to re-run;
keep an SSH session open while it runs:

```sh
ssh team-06 'sudo bash -s' < deploy/harden.sh
```
