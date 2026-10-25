#!/bin/sh
# The web image's entrypoint (V0.4.16). It decides, from the environment, where the API is, writes that into a
# small Caddy snippet in /tmp and starts Caddy:
#
#   REPOMEMO_API_UPSTREAM=api:3020             same-origin mode: /v1 and /health are forwarded there, with the live event
#                                              streams unbuffered, so one address serves the app and the API
#   (unset)                                    the image serves the app alone, and says so on /v1 and /health
#
# The upstream is host:port, or http(s)://host:port. Anything else is refused here, before it reaches the
# configuration, so a typo cannot turn into a different rule.
set -eu

snippet=/tmp/api.caddy
upstream="${REPOMEMO_API_UPSTREAM:-}"

# Cross-origin mode (V0.4.17): when the app is told to call an API on another address (REPOMEMO_WEB_API_URL), that
# address's origin joins connect-src in the content security policy. It must be http(s)://host[:port][/path] with
# nothing that could widen the policy; an empty value means the page's own host and adds nothing.
api_url="${REPOMEMO_WEB_API_URL:-}"
REPOMEMO_WEB_API_ORIGIN=""
if [ -n "$api_url" ]; then
  case "$api_url" in
    *[!]A-Za-z0-9.:/_[-]*)
      echo "repomemo-web: REPOMEMO_WEB_API_URL ($api_url) may hold only letters, digits and . : / _ - [ ]" >&2
      exit 1
      ;;
    http://*|https://*) ;;
    *) echo "repomemo-web: REPOMEMO_WEB_API_URL ($api_url) must start with http:// or https://" >&2; exit 1 ;;
  esac
  REPOMEMO_WEB_API_ORIGIN=$(printf '%s' "$api_url" | sed -E 's#^(https?://[^/]+).*#\1#')
fi
export REPOMEMO_WEB_API_ORIGIN

if [ -n "$upstream" ]; then
  case "$upstream" in
    *[!]A-Za-z0-9.:/_[-]*)
      echo "repomemo-web: REPOMEMO_API_UPSTREAM ($upstream) may hold only letters, digits and . : / _ - [ ] (host:port or http(s)://host:port)" >&2
      exit 1
      ;;
  esac
  case "$upstream" in
    http://*|https://*) target="$upstream" ;;
    */*) echo "repomemo-web: REPOMEMO_API_UPSTREAM ($upstream) has a path; give host:port or http(s)://host:port" >&2; exit 1 ;;
    *:*) target="$upstream" ;;
    *) echo "repomemo-web: REPOMEMO_API_UPSTREAM ($upstream) has no port; give host:port" >&2; exit 1 ;;
  esac
  cat > "$snippet" <<EOF
reverse_proxy $target {
	# The live event streams arrive as they happen. Caddy adds the client's address to X-Forwarded-For, which the
	# API believes only from an address listed in its REPOMEMO_TRUSTED_PROXIES.
	flush_interval -1
	transport http {
		response_header_timeout 180s
	}
}
EOF
  echo "repomemo-web: forwarding /v1 and /health to $target" >&2
else
  cat > "$snippet" <<'EOF'
respond "This web image has no API behind it (REPOMEMO_API_UPSTREAM is not set)." 502
EOF
  echo "repomemo-web: no REPOMEMO_API_UPSTREAM; serving the app alone" >&2
fi

exec caddy run --config /etc/caddy/Caddyfile --adapter caddyfile
