#!/bin/sh
# Upgrades a container install (V0.4.24): a backup through the API, the new images, a restart, a wait until everything is
# healthy, and the old version put back when it is not.
#
#   REPOMEMO_UPGRADE_TOKEN=<a personal access token with the system scope> deploy/upgrade.sh --version 0.4.24
#   deploy/upgrade.sh --version 0.4.24 --token-file /run/secrets/upgrade-token
#   deploy/upgrade.sh --version 0.4.24 --no-backup           (you have a backup of your own)
#
# Options:
#   --version TAG      the tag to run (REPOMEMO_VERSION in .env); required
#   --env-file FILE    the .env of the install (default .env)
#   --url URL          where the web image answers (default http://127.0.0.1:8080, its loopback port)
#   --no-build         use images that exist or can be pulled; do not build from this checkout
#   --no-backup        skip the backup (the script says so); without it and without a token, it stops
#   --timeout SECONDS  how long to wait for healthy and for the backup (default 900)
#
# What it never does: delete a volume, or touch .env before the backup is done. When the new version does not turn
# healthy, REPOMEMO_VERSION in .env goes back to what it was and the stack is started again on the old images
# (no build). A migration the new version already ran on the database is not undone: the backup is the way back (the
# upgrade guide in mindmap/technical/operations.md).
set -eu

env_file=".env"
url="http://127.0.0.1:8080"
version=""
build=yes
backup=yes
timeout=900
token="${REPOMEMO_UPGRADE_TOKEN:-}"

fail() {
  echo "upgrade: $*" >&2
  exit 1
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version) [ $# -ge 2 ] || fail "--version needs a tag"; version="$2"; shift 2 ;;
    --env-file) [ $# -ge 2 ] || fail "--env-file needs a path"; env_file="$2"; shift 2 ;;
    --url) [ $# -ge 2 ] || fail "--url needs an address"; url="$2"; shift 2 ;;
    --token-file) [ $# -ge 2 ] || fail "--token-file needs a path"; [ -r "$2" ] || fail "cannot read $2"; token="$(tr -d ' \r\n' < "$2")"; shift 2 ;;
    --timeout) [ $# -ge 2 ] || fail "--timeout needs seconds"; timeout="$2"; shift 2 ;;
    --no-build) build=no; shift ;;
    --no-backup) backup=no; shift ;;
    -h|--help) sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) fail "unknown argument: $1 (try --help)" ;;
  esac
done

[ -n "$version" ] || fail "give the tag to run: --version TAG"
case "$version" in
  *[!A-Za-z0-9._-]*) fail "the tag ($version) may hold only letters, digits, dots, underscores and hyphens" ;;
esac
case "$timeout" in *[!0-9]*|"") fail "--timeout is a number of seconds" ;; esac
[ -f "$env_file" ] || fail "$env_file does not exist; this install was not made with deploy/init (or give --env-file)"
command -v docker > /dev/null 2>&1 || fail "docker is not installed or not on the PATH"
command -v curl > /dev/null 2>&1 || fail "curl is not installed (the script asks the server over HTTP)"

compose() {
  docker compose --env-file "$env_file" "$@"
}

current_version() {
  line="$(grep -E '^REPOMEMO_VERSION=' "$env_file" | tail -n 1 || true)"
  value="${line#REPOMEMO_VERSION=}"
  echo "${value:-local}"
}

set_version() {
  # The line is replaced, or added when there is none; the rest of the file is kept as it is.
  if grep -qE '^REPOMEMO_VERSION=' "$env_file"; then
    sed -e "s|^REPOMEMO_VERSION=.*|REPOMEMO_VERSION=$1|" "$env_file" > "$env_file.new"
  else
    { cat "$env_file"; echo "REPOMEMO_VERSION=$1"; } > "$env_file.new"
  fi
  cat "$env_file.new" > "$env_file"
  rm -f "$env_file.new"
}

wait_ready() {
  deadline=$(( $(date +%s) + timeout ))
  while [ "$(date +%s)" -lt "$deadline" ]; do
    if curl --fail --silent "$url/health/ready" > /dev/null 2>&1; then return 0; fi
    sleep 3
  done
  return 1
}

old="$(current_version)"
[ "$old" != "$version" ] || echo "upgrade: $env_file already says $version; the stack is brought up to date with it"

# 1. The backup, through the API, before anything changes.
if [ "$backup" = yes ]; then
  [ -n "$token" ] || fail "no token for the backup. Make a personal access token with the system scope (System > Access) and pass it in REPOMEMO_UPGRADE_TOKEN or --token-file, or pass --no-backup if you have a backup of your own"
  wait_ready || fail "the server does not answer at $url; is the stack running? (docker compose up -d)"
  echo "upgrade: making a backup"
  curl --fail --silent --show-error --request POST --header "Authorization: Bearer $token" "$url/v1/system/backups" > /dev/null \
    || fail "the server refused the backup (is the token valid and of the system scope?)"
  deadline=$(( $(date +%s) + timeout ))
  while :; do
    status="$(curl --fail --silent --show-error --header "Authorization: Bearer $token" "$url/v1/system/backups")" || fail "could not read the backups"
    case "$status" in
      *'"running":true'*) ;;
      *'"succeeded":true'*) break ;;
      *) fail "the backup did not succeed: $status" ;;
    esac
    [ "$(date +%s)" -lt "$deadline" ] || fail "the backup did not finish within $timeout seconds"
    sleep 3
  done
  echo "upgrade: the backup is made"
else
  echo "upgrade: no backup was asked for; going on without one"
fi

# 2. The new version: images, then a restart that waits until every service is healthy.
set_version "$version"
echo "upgrade: $old -> $version"
if [ "$build" = yes ]; then
  compose pull --ignore-buildable > /dev/null 2>&1 || true
  up="up --detach --build --wait --wait-timeout $timeout"
else
  compose pull --ignore-buildable || true
  up="up --detach --no-build --wait --wait-timeout $timeout"
fi

# shellcheck disable=SC2086
if compose $up && wait_ready; then
  running="$(curl --fail --silent "$url/health" || true)"
  echo "upgrade: healthy. The server says: $running"
  exit 0
fi

# 3. It did not turn healthy: the old version goes back.
echo "upgrade: $version did not turn healthy; putting $old back" >&2
compose logs --no-color --tail 40 >&2 || true
set_version "$old"
if compose up --detach --wait --wait-timeout "$timeout" && wait_ready; then
  echo "upgrade: back on $old, running. The backup (if one was made) is untouched." >&2
  exit 1
fi
fail "$old would not start either. The data is untouched: look at 'docker compose logs', and restore the backup if a migration ran"
