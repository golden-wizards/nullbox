# Deploy

Production runs on `team-06` (`ssh team-06`) from `/opt/nullbox/deploy` (`compose.yml` + `Caddyfile`).

- `api`: Rust API. SQLite lives on the `data` volume. Config and secrets come from `/etc/nullbox/nullbox.env`
  on the server (root, 0600, **never in git**: `CORS_ALLOWED_ORIGINS`, `PUBLIC_WEB_URL`, `ADMIN_TOKEN`, `RUST_LOG`,
  and for payments `STRIPE_SECRET_KEY`, `STRIPE_WEBHOOK_SECRET`, optional `STRIPE_PUBLISHABLE_KEY`; see `.env.example`).
  Login: optional `SIGNUP_INVITE_CODE`.
- `web`: Caddy with the built web app. It handles TLS for team-06.fasthacks.cloud, serves `/` (SPA
  fallback) and proxies `/api/*` to `api:3000`. `/inbox*` is members-only: Caddy asks `/api/auth/gate`
  (`forward_auth`) and sends visitors without a session to `/login`. Only this container publishes ports (80/443).

Manual deploy (until CI/CD, #8, does it): build the images where there's CPU, ship them, restart.

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
