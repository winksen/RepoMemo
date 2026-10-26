#!/bin/sh
# Makes the .env of a container install (V0.4.23): the host name and strong secrets, from .env.example. It refuses to
# overwrite a .env that exists, because replacing the secrets of a running install would sign everyone out and make
# the stored provider keys unreadable.
#
#   deploy/init.sh                                  a .env for localhost, with the edge (TLS) profile
#   deploy/init.sh --host memo.example.com          for a public name (Let's Encrypt gets the certificate)
#   deploy/init.sh --no-edge                        no TLS piece: a proxy of your own is in front
#   deploy/init.sh --output /path/to/.env           another file than ./.env
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
example="$here/../.env.example"
host="localhost"
edge=yes
output=".env"

fail() {
  echo "init: $*" >&2
  exit 1
}

while [ $# -gt 0 ]; do
  case "$1" in
    --host) [ $# -ge 2 ] || fail "--host needs a name"; host="$2"; shift 2 ;;
    --no-edge) edge=no; shift ;;
    --edge) edge=yes; shift ;;
    --output) [ $# -ge 2 ] || fail "--output needs a path"; output="$2"; shift 2 ;;
    -h|--help) sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) fail "unknown argument: $1 (try --help)" ;;
  esac
done

[ -f "$example" ] || fail "cannot find .env.example next to this script ($example)"
[ ! -e "$output" ] || fail "$output exists already; not touching it. Remove it first if you mean to start again (that signs everyone out)"
case "$host" in
  *[!A-Za-z0-9.-]*|"") fail "the host name ($host) may hold only letters, digits, dots and hyphens" ;;
esac

random_secret() {
  if command -v openssl > /dev/null 2>&1; then
    openssl rand -hex 32
  else
    head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n'
  fi
}

jwt="$(random_secret)"
key="$(random_secret)"
[ "${#jwt}" -ge 32 ] && [ "${#key}" -ge 32 ] || fail "could not make strong secrets (no openssl and no /dev/urandom?)"
if [ "$edge" = yes ]; then profiles="edge"; else profiles=""; fi

umask 077
sed \
  -e "s|^REPOMEMO_HOST=.*|REPOMEMO_HOST=$host|" \
  -e "s|^REPOMEMO_JWT_SECRET=.*|REPOMEMO_JWT_SECRET=$jwt|" \
  -e "s|^REPOMEMO_SECRET_KEY=.*|REPOMEMO_SECRET_KEY=$key|" \
  -e "s|^COMPOSE_PROFILES=.*|COMPOSE_PROFILES=$profiles|" \
  "$example" > "$output"

cat <<DONE
init: wrote $output for $host (edge: $edge), readable by you alone.
Keep a copy of REPOMEMO_SECRET_KEY somewhere safe: without it the AI provider keys and repository tokens stored in the
database must be entered again. Then:
  docker compose pull && docker compose up -d --wait
and open https://$host (or http://127.0.0.1:8080 without the edge) to finish the first-run setup.
DONE
