# syntax=docker/dockerfile:1.7
#
# RepoMemo as containers (roadmap O7): two images from one file.
#
#   docker build --target server -t repomemo-server .   the API, background work and the data folder
#   docker build --target proxy  -t repomemo-proxy  .   Caddy: TLS, the web app and the way to the server
#
# `docker compose up` (see docker-compose.yml) builds and starts both. The
# guide is mindmap/technical/operations.md, "Running it in containers".

# Every base image is pinned by digest (roadmap E17): a tag can move, a digest cannot.
# Dependabot (.github/dependabot.yml) proposes the next digest, and `npm run container:check`
# refuses a FROM line without one.

# ---------------------------------------------------------------------------
# The web app. The API is the page's own host, so it is called by relative
# address (an empty VITE_REPOMEMO_API_URL): the same image works under any name.
FROM node:22-bookworm-slim@sha256:a6385bc33f0f9d6a08482c87f52baee537a9184e2b47dea1ed97caac62f8960c AS web
WORKDIR /build
COPY package.json package-lock.json ./
COPY apps/desktop/package.json apps/desktop/package.json
RUN npm ci --no-audit --no-fund
COPY apps/desktop apps/desktop
ENV VITE_REPOMEMO_API_URL=""
RUN npm run build

# ---------------------------------------------------------------------------
# The server binary.
FROM rust:1-bookworm@sha256:5c1a1732ac3f77635b24d397ceb67b4d9ea61de348f68c0aa54a79fd01e65c69 AS build
WORKDIR /build
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release --locked -p repomemo-server \
    && mkdir -p /out \
    && cp target/release/repomemo-server /out/repomemo-server

# ---------------------------------------------------------------------------
# The server. Readings of documents start this same program again, so it must
# stay where it is. Layout-accurate Office previews need LibreOffice, which is
# large: add it with `--build-arg WITH_LIBREOFFICE=true`.
FROM debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8 AS server
ARG WITH_LIBREOFFICE=false
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl git \
    && if [ "$WITH_LIBREOFFICE" = "true" ]; then \
         apt-get install -y --no-install-recommends libreoffice-core libreoffice-writer libreoffice-calc libreoffice-impress fonts-dejavu-core; \
       fi \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home-dir /var/lib/repomemo --shell /usr/sbin/nologin repomemo \
    && mkdir -p /var/lib/repomemo \
    && chown repomemo:repomemo /var/lib/repomemo
COPY --from=build /out/repomemo-server /usr/local/bin/repomemo-server

# The data folder (the database, the stored files, the key file and the
# backups) is `workspace-data/` under the working directory. Keep it in a volume.
WORKDIR /var/lib/repomemo
VOLUME /var/lib/repomemo
USER repomemo
ENV REPOMEMO_SERVER_ADDR=0.0.0.0:3020 \
    REPOMEMO_SERVER_DATA_DIR=main
EXPOSE 3020
HEALTHCHECK --interval=15s --timeout=5s --start-period=40s --retries=3 \
    CMD curl --fail --silent http://127.0.0.1:3020/health/ready || exit 1
ENTRYPOINT ["repomemo-server"]

# ---------------------------------------------------------------------------
# The front door: Caddy gets the certificate, serves the web app and sends
# /v1 and /health to the server. The content security policy the build wrote
# next to the web app is put into the Caddy configuration, as the guide asks.
FROM caddy:2-alpine@sha256:d8542f48d34a9cf4e4c11a478865229840e87e4c96ea3f439101f31a5d35f75f AS proxy
COPY --from=web /build/apps/desktop/dist /srv/web
COPY docker/Caddyfile /etc/caddy/Caddyfile
RUN printf 'header ?Content-Security-Policy "%s"\n' "$(cat /srv/web/content-security-policy.txt)" > /etc/caddy/csp.caddy
EXPOSE 80 443 443/udp
