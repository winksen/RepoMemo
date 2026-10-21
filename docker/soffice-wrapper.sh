#!/bin/bash
# Runs LibreOffice in the converter container (V0.2.148). The server starts this in place of LibreOffice
# (REPOMEMO_SOFFICE_WRAPPER): it hands the arguments to the converter over a socket on a volume the two
# containers share, prints what LibreOffice printed and exits with its status. The converter has no
# network and sees only the previews volume, so a document that escapes LibreOffice reaches nothing else.
#
#   repomemo-soffice-wrapper soffice --headless --convert-to pdf --outdir <folder> <file>
#
# The first argument is the program name the server puts before LibreOffice's own arguments.

set -u
socket="${REPOMEMO_SOFFICE_SOCKET:-/run/soffice/convert.sock}"
shift

# Arguments travel NUL-separated, so none can be mistaken for another. The converter answers with
# LibreOffice's output and, on a line of its own at the end, EXIT:<status>.
if ! reply=$(printf '%s\0' "$@" | socat -t 150 - "UNIX-CONNECT:${socket}" 2>/dev/null); then
  echo "The converter container did not answer. Start it with COMPOSE_PROFILES=previews." >&2
  exit 70
fi

code="${reply##*EXIT:}"
output="${reply%EXIT:*}"
case "$code" in
  '' | *[!0-9]*) echo "The converter's answer could not be read." >&2; exit 70 ;;
esac
printf '%s' "${output%$'\n'}"
exit "$code"
