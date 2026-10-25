#!/bin/sh
# The server image's entrypoint (V0.4.10). A volume or a bind-mounted folder often belongs to root, or to
# whoever made it on the host, and the server runs as the unprivileged user 10001. Started as root, this
# gives the data folders to that user and drops to it before the server starts; started as another user
# (`user:` in compose, `runAsUser` in Kubernetes), it cannot change anything, so it says clearly if a folder
# is not writable instead of letting the first write fail later.
set -eu

# A line for the container's log: JSON like the server's own lines when REPOMEMO_LOG_FORMAT is json (V0.4.29), so that
# every line a log collector reads parses; plain text otherwise.
say() {
  if [ "$(printf '%s' "${REPOMEMO_LOG_FORMAT:-}" | tr 'A-Z' 'a-z')" = "json" ]; then
    escaped="$(printf '%s' "$2" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g')"
    printf '{"timestamp":"%s","level":"%s","target":"repomemo_entrypoint","message":"%s","fields":""}\n' "$(date -u +%Y-%m-%dT%H:%M:%S.000Z)" "$1" "$escaped" >&2
  else
    printf '%s\n' "$2" >&2
  fi
}

uid=10001
folders="/var/lib/repomemo /var/lib/repomemo-previews ${REPOMEMO_DB_DIR:-} ${REPOMEMO_BLOB_DIR:-} ${REPOMEMO_BACKUP_DIR:-}"

# A folder is wrong when it, or something shallow inside it (the environment folder, the database, the
# key file), belongs to someone else. The stored files below are not walked: that would take minutes on
# a large install at every start.
wrong_owner() {
  [ -n "$(find "$1" -maxdepth 3 ! -user "$uid" -print -quit 2>/dev/null)" ]
}

if [ "$(id -u)" = "0" ]; then
  for dir in $folders; do
    [ -d "$dir" ] || continue
    if wrong_owner "$dir"; then
      say WARN "repomemo: $dir does not belong to user $uid; giving it to that user"
      if ! chown -R "$uid:$uid" "$dir"; then
        say ERROR "repomemo: could not change the owner of $dir (a read-only or network mount?). Make it writable by user $uid, and run the container with --user $uid."
        exit 1
      fi
    fi
  done
  exec setpriv --reuid="$uid" --regid="$uid" --clear-groups repomemo-server "$@"
fi

for dir in $folders; do
  [ -d "$dir" ] || continue
  if [ ! -w "$dir" ]; then
    say ERROR "repomemo: $dir is not writable by user $(id -u). Change its owner to that user (chown -R $(id -u) on the host), or start the container as root so it can do that itself."
    exit 1
  fi
done
exec repomemo-server "$@"
