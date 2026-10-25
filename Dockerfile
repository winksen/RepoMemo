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
# The server binary, in three steps so that a change of code does not build the world again (V0.4.7):
# the dependencies are built from a recipe that holds only the manifests and the lock file, in a layer that
# stays cached until a dependency changes; then the code is built on top of it.
FROM rust:1-trixie@sha256:767c71592f7d1f3f242292000f8efd4e67b6732363da176772f4d220332e264f AS chef
RUN cargo install cargo-chef --locked --version 0.1.71
WORKDIR /build

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
COPY --from=planner /build/recipe.json recipe.json
# The dependencies of the server and nothing else. This layer is the one that is reused.
RUN cargo chef cook --release --locked -p repomemo-server --recipe-path recipe.json
COPY . .
# What this build is called, printed by /health and --version (V0.4.3).
ARG REPOMEMO_BUILD_COMMIT=unknown
ARG REPOMEMO_BUILD_DATE=unknown
ARG REPOMEMO_IMAGE_TAG=unknown
RUN cargo build --release --locked -p repomemo-server \
    && mkdir -p /out \
    && strip --strip-unneeded -o /out/repomemo-server target/release/repomemo-server

# ---------------------------------------------------------------------------
# The server, on Debian 13 (trixie): its git 2.47.3 is a release the remote-clone check
# accepts, where Debian 12's 2.39.5 is refused (`npm run container:check` asserts the pair).
# Readings of documents start this same program again, so it must stay where it is.
# Layout-accurate Office previews need LibreOffice, which is large and parses files strangers uploaded:
# the compose file runs it in the `converter` stage below, a container of its own with no network, and
# reaches it through repomemo-soffice-wrapper. `--build-arg WITH_LIBREOFFICE=true` still puts it in this
# image, for a plain `docker run`.
FROM debian:trixie-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad AS server
ARG WITH_LIBREOFFICE=false
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git socat \
    && if [ "$WITH_LIBREOFFICE" = "true" ]; then \
         apt-get install -y --no-install-recommends libreoffice-core libreoffice-writer libreoffice-calc libreoffice-impress fonts-dejavu-core; \
       fi \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home-dir /var/lib/repomemo --shell /usr/sbin/nologin repomemo \
    && mkdir -p /var/lib/repomemo /var/lib/repomemo-previews /run/soffice \
    && chown repomemo:repomemo /var/lib/repomemo /var/lib/repomemo-previews /run/soffice
COPY --from=build /out/repomemo-server /usr/local/bin/repomemo-server
COPY --chmod=0755 docker/soffice-wrapper.sh /usr/local/bin/repomemo-soffice-wrapper
# Starts as root only to give a volume that belongs to someone else to user 10001 and drop to it (V0.4.10).
COPY --chmod=0755 docker/entrypoint.sh /usr/local/bin/repomemo-entrypoint

# The data folder (the database, the stored files, the key file and the
# backups) is `workspace-data/` under the working directory. Keep it in a volume.
WORKDIR /var/lib/repomemo
VOLUME /var/lib/repomemo
# No USER line: the entrypoint runs as root, fixes the ownership of the data folders and runs the server as
# user 10001 (repomemo). Started with another user, it only checks that the folders can be written.
ENV REPOMEMO_SERVER_ADDR=0.0.0.0:3020 \
    REPOMEMO_SERVER_DATA_DIR=main
EXPOSE 3020
HEALTHCHECK --interval=15s --timeout=5s --start-period=40s --retries=3 \
    CMD ["repomemo-server", "--health"]
# Labels, last so that a new commit does not rebuild what is above (V0.4.9).
ARG REPOMEMO_SOURCE_URL=https://github.com/repomemo/repomemo
ARG REPOMEMO_BUILD_COMMIT=unknown
ARG REPOMEMO_BUILD_DATE=unknown
ARG REPOMEMO_IMAGE_TAG=unknown
LABEL org.opencontainers.image.title="repomemo-server" \
      org.opencontainers.image.description="RepoMemo API: the server, its database and its stored files" \
      org.opencontainers.image.source="${REPOMEMO_SOURCE_URL}" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.version="${REPOMEMO_IMAGE_TAG}" \
      org.opencontainers.image.revision="${REPOMEMO_BUILD_COMMIT}" \
      org.opencontainers.image.created="${REPOMEMO_BUILD_DATE}"
ENTRYPOINT ["repomemo-entrypoint"]

# ---------------------------------------------------------------------------
# The converter (V0.2.148): LibreOffice and nothing else, for the compose profile `previews`. It listens on a
# socket on a volume it shares with the server, sees only the previews volume, and has no network.
# Debian 13's LibreOffice may be older than the fixed releases the server insists on; the server says so in
# System > Overview and converts nothing until it is a release at or past them.
FROM debian:trixie-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad AS converter
RUN apt-get update \
    && apt-get install -y --no-install-recommends bash socat libreoffice-core libreoffice-writer libreoffice-calc libreoffice-impress fonts-dejavu-core \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home-dir /tmp --shell /usr/sbin/nologin repomemo \
    && mkdir -p /var/lib/repomemo-previews /run/soffice \
    && chown repomemo:repomemo /var/lib/repomemo-previews /run/soffice
COPY --chmod=0755 docker/soffice-listener.sh /usr/local/bin/repomemo-soffice-listener
COPY --chmod=0755 docker/soffice-run.sh /usr/local/bin/repomemo-soffice-run
USER repomemo
ENV HOME=/tmp
# Labels, last so that a new commit does not rebuild what is above (V0.4.9).
ARG REPOMEMO_SOURCE_URL=https://github.com/repomemo/repomemo
ARG REPOMEMO_BUILD_COMMIT=unknown
ARG REPOMEMO_BUILD_DATE=unknown
ARG REPOMEMO_IMAGE_TAG=unknown
LABEL org.opencontainers.image.title="repomemo-converter" \
      org.opencontainers.image.description="RepoMemo converter: LibreOffice for layout-accurate previews, with no network" \
      org.opencontainers.image.source="${REPOMEMO_SOURCE_URL}" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.version="${REPOMEMO_IMAGE_TAG}" \
      org.opencontainers.image.revision="${REPOMEMO_BUILD_COMMIT}" \
      org.opencontainers.image.created="${REPOMEMO_BUILD_DATE}"
ENTRYPOINT ["repomemo-soffice-listener"]

# ---------------------------------------------------------------------------
# The front door: Caddy gets the certificate, serves the web app and sends
# /v1 and /health to the server. The content security policy the build wrote
# next to the web app is put into the Caddy configuration, as the guide asks.
FROM caddy:2-alpine@sha256:d8542f48d34a9cf4e4c11a478865229840e87e4c96ea3f439101f31a5d35f75f AS proxy
COPY --from=web /build/apps/desktop/dist /srv/web
COPY docker/Caddyfile /etc/caddy/Caddyfile
RUN printf 'header ?Content-Security-Policy "%s"\n' "$(cat /srv/web/content-security-policy.txt)" > /etc/caddy/csp.caddy
EXPOSE 80 443 443/udp
# Labels, last so that a new commit does not rebuild what is above (V0.4.9).
ARG REPOMEMO_SOURCE_URL=https://github.com/repomemo/repomemo
ARG REPOMEMO_BUILD_COMMIT=unknown
ARG REPOMEMO_BUILD_DATE=unknown
ARG REPOMEMO_IMAGE_TAG=unknown
LABEL org.opencontainers.image.title="repomemo-proxy" \
      org.opencontainers.image.description="RepoMemo front door: Caddy, the web app and the way to the server" \
      org.opencontainers.image.source="${REPOMEMO_SOURCE_URL}" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.version="${REPOMEMO_IMAGE_TAG}" \
      org.opencontainers.image.revision="${REPOMEMO_BUILD_COMMIT}" \
      org.opencontainers.image.created="${REPOMEMO_BUILD_DATE}"

# ---------------------------------------------------------------------------
# The web client on its own (V0.4.15): the built app behind a small static server, over plain HTTP on 8080, with
# no API and no TLS. A front door brings the certificate; REPOMEMO_WEB_API_URL and REPOMEMO_PRODUCT_NAME say
# what the app is told when it starts (/config.json). Caddy's file capability is taken off so that the image
# also runs with every capability dropped.
FROM caddy:2-alpine@sha256:d8542f48d34a9cf4e4c11a478865229840e87e4c96ea3f439101f31a5d35f75f AS webapp
RUN adduser -S -D -H -u 10001 -h /nonexistent repomemo \
    && apk add --no-cache libcap-setcap \
    && (setcap -r /usr/bin/caddy || true) \
    && apk del libcap-setcap
COPY --from=web /build/apps/desktop/dist /srv/web
COPY docker/Caddyfile.web /etc/caddy/Caddyfile
COPY --chmod=0755 docker/web-entrypoint.sh /usr/local/bin/repomemo-web-entrypoint
RUN printf 'header ?Content-Security-Policy "%s"\n' "$(cat /srv/web/content-security-policy.txt)" > /etc/caddy/csp.caddy
ENV XDG_DATA_HOME=/tmp XDG_CONFIG_HOME=/tmp
USER 10001
EXPOSE 8080
HEALTHCHECK --interval=15s --timeout=5s --start-period=5s --retries=3 \
    CMD ["wget", "--quiet", "--output-document=/dev/null", "http://127.0.0.1:8080/healthz"]
ENTRYPOINT ["repomemo-web-entrypoint"]
# Labels, last so that a new commit does not rebuild what is above (V0.4.9).
ARG REPOMEMO_SOURCE_URL=https://github.com/repomemo/repomemo
ARG REPOMEMO_BUILD_COMMIT=unknown
ARG REPOMEMO_BUILD_DATE=unknown
ARG REPOMEMO_IMAGE_TAG=unknown
LABEL org.opencontainers.image.title="repomemo-web" \
      org.opencontainers.image.description="RepoMemo web client: the built app over plain HTTP, with no API behind it" \
      org.opencontainers.image.source="${REPOMEMO_SOURCE_URL}" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.version="${REPOMEMO_IMAGE_TAG}" \
      org.opencontainers.image.revision="${REPOMEMO_BUILD_COMMIT}" \
      org.opencontainers.image.created="${REPOMEMO_BUILD_DATE}"
