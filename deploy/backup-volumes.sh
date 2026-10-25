#!/bin/sh
# Copies the volumes of a container install into archives you can keep elsewhere (V0.4.25). The server makes its own
# consistent backups (System > Backups, or deploy/upgrade.sh asks for one): they sit in the repomemo-backups volume, and
# this copies that volume off the machine together with repomemo-data (the key file and the logs). That is safe while
# the stack runs.
#
#   deploy/backup-volumes.sh --out /srv/repomemo-copies            the server's backups and the key file
#   deploy/backup-volumes.sh --out /srv/repomemo-copies --all      also the database and the stored files, with the
#                                                                  stack stopped while they are copied, then started
#   deploy/backup-volumes.sh --project myproject --out ...         another compose project name (default: repomemo)
#
# A copy of the database taken while the server writes to it is not a backup, which is why --all stops the stack first.
# Archives are named after the volume (repomemo-backups.tar.gz ...) and a backup.txt says when and from what; a
# second run into the same folder is refused, so a copy is never overwritten. deploy/restore-volumes.sh puts them back.
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
project="repomemo"
out=""
all=no
helper="${REPOMEMO_HELPER_IMAGE:-alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc}"

fail() {
  echo "backup-volumes: $*" >&2
  exit 1
}

while [ $# -gt 0 ]; do
  case "$1" in
    --project) [ $# -ge 2 ] || fail "--project needs a name"; project="$2"; shift 2 ;;
    --out) [ $# -ge 2 ] || fail "--out needs a folder"; out="$2"; shift 2 ;;
    --all) all=yes; shift ;;
    -h|--help) sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) fail "unknown argument: $1 (try --help)" ;;
  esac
done

[ -n "$out" ] || fail "say where to put the archives: --out FOLDER"
command -v docker > /dev/null 2>&1 || fail "docker is not installed or not on the PATH"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
[ ! -e "$out/backup.txt" ] || fail "$out holds a backup already; use a new folder (a copy is never overwritten)"

if [ "$all" = yes ]; then volumes="repomemo-backups repomemo-data repomemo-db repomemo-files"; else volumes="repomemo-backups repomemo-data"; fi
for short in $volumes; do
  docker volume inspect "${project}_${short}" > /dev/null 2>&1 || fail "the volume ${project}_${short} does not exist (is the project called $project? --project NAME)"
done

compose() {
  docker compose --project-name "$project" --file "$here/../docker-compose.yml" "$@"
}

restart=no
if [ "$all" = yes ]; then
  if [ -n "$(compose ps --quiet 2> /dev/null)" ]; then
    echo "backup-volumes: stopping the stack while the database and the files are copied"
    compose stop
    restart=yes
  fi
fi

status=0
for short in $volumes; do
  echo "backup-volumes: copying ${project}_${short}"
  docker run --rm --volume "${project}_${short}:/data:ro" --volume "$out:/out" "$helper" \
    tar -C /data -czf "/out/${short}.tar.gz" . || { status=1; break; }
done

if [ "$restart" = yes ]; then
  echo "backup-volumes: starting the stack again"
  compose start
fi
[ "$status" -eq 0 ] || fail "a copy did not finish; $out may hold part of it: do not use it"

{
  echo "RepoMemo volume copy"
  echo "date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "project: $project"
  echo "volumes: $volumes"
  echo "complete: $all (yes: the database and the stored files were copied with the stack stopped)"
} > "$out/backup.txt"
echo "backup-volumes: done: $out"
