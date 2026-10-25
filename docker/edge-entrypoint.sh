#!/bin/sh
# The edge image's entrypoint (V0.4.18): picks how the certificate is got and starts Caddy.
#
#   /certs/tls.crt and /certs/tls.key present   the certificates the operator supplied are used
#   otherwise                                   Caddy's automatic certificates: Let's Encrypt for a public name,
#                                               its own authority for localhost and addresses
#
# One of the two files without the other is a mistake worth stopping for.
set -eu

snippet=/tmp/tls.caddy
cert=/certs/tls.crt
key=/certs/tls.key

if [ -f "$cert" ] && [ -f "$key" ]; then
  echo "tls $cert $key" > "$snippet"
  echo "repomemo-edge: using the certificates in /certs" >&2
elif [ -f "$cert" ] || [ -f "$key" ]; then
  echo "repomemo-edge: /certs holds only one of tls.crt and tls.key; give both, or neither for automatic certificates" >&2
  exit 1
else
  : > "$snippet"
  echo "repomemo-edge: no certificates in /certs; Caddy gets its own for ${REPOMEMO_HOST:-localhost}" >&2
fi

exec caddy run --config /etc/caddy/Caddyfile --adapter caddyfile
