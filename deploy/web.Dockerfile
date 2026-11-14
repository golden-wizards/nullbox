# Nullbox web: Vite build served by Caddy, which also proxies /api to the API.
# Manifests are copied before sources so the CI layer cache keeps `pnpm install`
# across commits that don't change dependencies.
FROM docker.io/library/node:22-slim AS build
WORKDIR /src
RUN npm install -g pnpm@12.14.0
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/web/package.json apps/web/
RUN pnpm install --frozen-lockfile
COPY apps/web apps/web
# Empty base URL: the bundle calls /api on its own origin.
ENV VITE_API_BASE_URL=
RUN pnpm build

# Retro deck (docs/presentation): a standalone Slidev project, served under /slides/.
FROM docker.io/library/node:22-slim AS slides
WORKDIR /deck
COPY docs/presentation/package.json docs/presentation/.npmrc ./
RUN npm install --no-audit --no-fund
COPY docs/presentation ./
RUN npx slidev build slides.md --base /slides/ --out dist

FROM docker.io/library/caddy:2
COPY deploy/Caddyfile /etc/caddy/Caddyfile
COPY --from=build /src/apps/web/dist /srv
COPY --from=slides /deck/dist /srv/slides
