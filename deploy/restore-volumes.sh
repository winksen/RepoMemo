#!/bin/sh
# Puts the archives of deploy/backup-volumes.sh back into the volumes of a stack (V0.4.25): the second stack of a
# test, a new machine, or a stack whose volumes were lost. It fills EMPTY volumes only and refuses anything else,
# so a restore can never mix two installs or overwrite a live one.
#
#   deploy/restore-volumes.sh --from /srv/repomemo-copies --project repomemo-new
#   docker compose --project-name repomemo-new up -d --wait          (the same .env secrets as the copied install)
#
# Only the volumes whose archive is in the folder are filled: a copy made without --all holds the server's backups and
# the key file, and the stack then starts empty and restores one of those backups from System > Backups.
set -eu

project="repomemo"
from=""
helper="${REPOMEMO_HELPER_IMAGE:-alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc}"

fail() {
  echo "restore-volumes: $*" >&2
  exit 1
}

while [ $# -gt 0 ]; do
  case "$1" in
    --project) [ $# -ge 2 ] || fail "--project needs a name"; project="$2"; shift 2 ;;
    --from) [ $# -ge 2 ] || fail "--from needs a folder"; from="$2"; shift 2 ;;
    -h|--help) sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) fail "unknown argument: $1 (try --help)" ;;
  esac
done

[ -n "$from" ] || fail "say where the archives are: --from FOLDER"
command -v docker > /dev/null 2>&1 || fail "docker is not installed or not on the PATH"
[ -f "$from/backup.txt" ] || fail "$from holds no backup.txt: it is not a copy made by deploy/backup-volumes.sh"
from="$(cd "$from" && pwd)"

found=""
for short in repomemo-data repomemo-db repomemo-files repomemo-backups; do
  [ -f "$from/$short.tar.gz" ] || continue
  found="$found $short"
  volume="${project}_${short}"
  [ -z "$(docker ps --quiet --filter "volume=$volume")" ] || fail "a container uses $volume; stop that stack first"
  docker volume create "$volume" > /dev/null
  used="$(docker run --rm --volume "$volume:/target:ro" "$helper" sh -c 'ls -A /target | head -n 1')"
  [ -z "$used" ] || fail "the volume $volume is not empty; refusing to restore over it"
done
[ -n "$found" ] || fail "$from holds no archive of a repomemo volume"

for short in $found; do
  echo "restore-volumes: filling ${project}_${short}"
  docker run --rm --volume "${project}_${short}:/target" --volume "$from:/in:ro" "$helper" \
    tar -C /target -xzf "/in/${short}.tar.gz" || fail "unpacking $short did not finish; remove ${project}_${short} and try again"
done
echo "restore-volumes: done:$found"
