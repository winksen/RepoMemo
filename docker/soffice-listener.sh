#!/bin/bash
# The converter container's only process (V0.2.148): listens on a socket on the shared volume and runs one
# repomemo-soffice-run per connection. Nothing else is reachable here: the container has no network.
set -eu
socket="${REPOMEMO_SOFFICE_SOCKET:-/run/soffice/convert.sock}"
exec socat "UNIX-LISTEN:${socket},fork,mode=0660,unlink-early" EXEC:/usr/local/bin/repomemo-soffice-run
