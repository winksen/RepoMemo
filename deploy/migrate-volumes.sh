#!/bin/sh
# Moves the data of a 0.3.x install (V0.4.22) from the one volume it lived in into the three the compose files use now:
#
#   repomemo-data  (workspace-data/main/repomemo.sqlite*, blobs/, backups/, secret.key, logs/)
#     -> repomemo-db       the database file and its write-ahead log
#     -> repomemo-files    the stored files
#     -> repomemo-backups  the backups
#
# The key file, the logs and the previews stay in repomemo-data, where the server still looks for them. Nothing is
# deleted: the old copies stay in repomemo-data until you remove them yourself, after the stack has run on the new
# volumes and you have signed in.
#
#   docker compose down                  # the stack must be stopped
#   deploy/migrate-volumes.sh            # project name "repomemo" (the folder or the `name:` of the compose file)
#   deploy/migrate-volumes.sh myproject  # another project name
#
# It refuses to run when the stack is up, when the old volume holds no database, or when a new volume already holds
# something (a second run would mix two databases).
set -eu

project="${1:-repomemo}"
old="${project}_repomemo-data"
db="${project}_repomemo-db"
files="${project}_repomemo-files"
backups="${project}_repomemo-backups"
helper="${REPOMEMO_MIGRATE_IMAGE:-alpine:3.20}"

fail() {
  echo "migrate-volumes: $*" >&2
  exit 1
}

command -v docker > /dev/null 2>&1 || fail "docker is not installed or not on the PATH"
docker volume inspect "$old" > /dev/null 2>&1 || fail "the volume $old does not exist (is the project called $project? give its name as the first argument)"

running="$(docker ps --quiet --filter "volume=$old")"
[ -z "$running" ] || fail "a container still uses $old; stop the stack first (docker compose down)"

for volume in "$db" "$files" "$backups"; do
  docker volume create "$volume" > /dev/null
  used="$(docker run --rm --volume "$volume:/target:ro" "$helper" sh -c 'ls -A /target | head -n 1')"
  [ -z "$used" ] || fail "the volume $volume already holds data; refusing to mix it with the old one"
done

echo "migrate-volumes: copying $old into $db, $files and $backups"
docker run --rm \
  --volume "$old:/old:ro" --volume "$db:/db" --volume "$files:/files" --volume "$backups:/backups" \
  "$helper" sh -eu -c '
    source=/old/workspace-data/main
    [ -f "$source/repomemo.sqlite" ] || { echo "no database at workspace-data/main/repomemo.sqlite in the old volume" >&2; exit 3; }
    cp -a "$source"/repomemo.sqlite* /db/
    if [ -d "$source/blobs" ]; then cp -a "$source"/blobs/. /files/; fi
    if [ -d "$source/backups" ]; then cp -a "$source"/backups/. /backups/; fi
    # The copy is checked before anyone is told it worked: the same number of files, the same database size.
    [ "$(find "$source/blobs" -type f 2>/dev/null | wc -l)" = "$(find /files -type f | wc -l)" ] || { echo "the number of stored files differs after the copy" >&2; exit 4; }
    [ "$(wc -c < "$source/repomemo.sqlite")" = "$(wc -c < /db/repomemo.sqlite)" ] || { echo "the database differs in size after the copy" >&2; exit 4; }
    echo "copied: $(find /files -type f | wc -l) stored files, $(ls /backups | wc -l) backups, the database"
  ' || fail "the copy did not finish; the new volumes may hold part of it: remove them (docker volume rm $db $files $backups) before trying again"

cat <<EOF
migrate-volumes: done. Start the stack (docker compose up -d), sign in, and look at your data.
The old copies are still in $old, under workspace-data/main (repomemo.sqlite*, blobs/, backups/); they are not used any
more. Remove them when you are satisfied, for example:
  docker run --rm -v $old:/old alpine sh -c 'rm -rf /old/workspace-data/main/repomemo.sqlite* /old/workspace-data/main/blobs /old/workspace-data/main/backups'
EOF
