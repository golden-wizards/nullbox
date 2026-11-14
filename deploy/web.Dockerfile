# Nullbox web: Vite build served by Caddy, which also proxies /api to the API.
FROM docker.io/library/node:22-slim AS build
WORKDIR /src
RUN npm install -g pnpm@12.14.0
COPY . .
RUN pnpm install --frozen-lockfile
# Empty base URL: the bundle calls /api on its own origin.
ENV VITE_API_BASE_URL=
RUN pnpm build

FROM docker.io/library/caddy:2
COPY deploy/Caddyfile /etc/caddy/Caddyfile
COPY --from=build /src/apps/web/dist /srv
