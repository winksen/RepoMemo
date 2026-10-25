#!/bin/sh
# Installs the unpacked archive as a systemd service (V0.4.32). Run it as root from the unpacked folder:
#
#   sudo sh service/install.sh --generate-secrets --start
#
# What it does, and only this:
#   - makes the system user and group `repomemo` (no login, home /var/lib/repomemo) if they are not there;
#   - makes /var/lib/repomemo (the working folder, owned by that user) and /etc/repomemo (the settings);
#   - copies the release to /opt/repomemo-<version> and points the link /opt/repomemo at it, so a new release is a new
#     link and the old one stays for a way back (V0.4.36);
#   - makes /etc/repomemo/repomemo.env from the example IF IT IS NOT THERE (mode 0600, owned by root); with
#     --generate-secrets it fills in the two secrets; an existing file is never touched;
#   - installs /etc/systemd/system/repomemo.service and enables it; --start also starts it.
# It can be run again: every step checks first, and a run with nothing to change changes nothing.
#
# Options: --prefix DIR (default /opt), --generate-secrets, --start.
set -eu

prefix="/opt"
generate=no
start=no

say() { echo "install: $*"; }
fail() { echo "install: $*" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) [ $# -ge 2 ] || fail "--prefix needs a folder"; prefix="$2"; shift 2 ;;
    --generate-secrets) generate=yes; shift ;;
    --start) start=yes; shift ;;
    -h|--help) sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) fail "unknown argument: $1 (try --help)" ;;
  esac
done

[ "$(id -u)" = "0" ] || fail "run this as root (sudo sh service/install.sh)"
command -v systemctl > /dev/null 2>&1 || fail "systemd is not here (no systemctl); this script is for systemd hosts"
home="$(cd "$(dirname "$0")/.." && pwd)"
[ -x "$home/bin/repomemo-server" ] || fail "$home/bin/repomemo-server is missing: run this from the unpacked archive"
[ -f "$home/VERSION" ] && [ -f "$home/config/repomemo.env.example" ] && [ -f "$home/service/repomemo.service" ] || fail "$home is not an unpacked RepoMemo archive"
version="$(tr -d ' \r\n' < "$home/VERSION")"
case "$version" in *[!A-Za-z0-9._-]*|"") fail "the VERSION file holds something that is not a version" ;; esac

if [ "$prefix" != "/opt" ]; then
  say "note: the unit expects /opt/repomemo; with --prefix $prefix, edit ExecStart and REPOMEMO_WEB_DIR to match"
fi

# The user and the folders.
if ! getent passwd repomemo > /dev/null 2>&1; then
  useradd --system --user-group --home-dir /var/lib/repomemo --no-create-home --shell /usr/sbin/nologin repomemo
  say "made the user repomemo"
fi
install -d -m 0750 -o repomemo -g repomemo /var/lib/repomemo
install -d -m 0755 -o root -g root /etc/repomemo

# The release, and the link to it.
release="$prefix/repomemo-$version"
if [ -f "$release/SHA256SUMS" ] && cmp -s "$release/SHA256SUMS" "$home/SHA256SUMS"; then
  say "release $version is already in $release"
else
  [ ! -e "$release" ] || fail "$release exists and differs from this archive; move it away first (it is never overwritten)"
  install -d -m 0755 "$prefix"
  staging="$prefix/.repomemo-$version.new"
  rm -rf "$staging"
  cp -a "$home" "$staging"
  mv "$staging" "$release"
  say "copied the release to $release"
fi
if [ "$(readlink "$prefix/repomemo" 2> /dev/null || true)" != "$release" ]; then
  [ ! -e "$prefix/repomemo" ] || [ -L "$prefix/repomemo" ] || fail "$prefix/repomemo exists and is not a link; move it away first"
  ln -sfn "$release" "$prefix/repomemo"
  say "$prefix/repomemo now points at $release"
fi

# The settings: made once, never touched again.
env="/etc/repomemo/repomemo.env"
secret() {
  if command -v openssl > /dev/null 2>&1; then openssl rand -hex 32; else head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n'; fi
}
if [ -e "$env" ]; then
  say "$env exists; leaving it as it is"
else
  umask 077
  if [ "$generate" = yes ]; then
    sed -e "s|^REPOMEMO_JWT_SECRET=.*|REPOMEMO_JWT_SECRET=$(secret)|" -e "s|^REPOMEMO_SECRET_KEY=.*|REPOMEMO_SECRET_KEY=$(secret)|" "$home/config/repomemo.env.example" > "$env"
    say "made $env with new secrets: keep a copy of REPOMEMO_SECRET_KEY somewhere safe"
  else
    cp "$home/config/repomemo.env.example" "$env"
    say "made $env from the example: fill in REPOMEMO_JWT_SECRET and REPOMEMO_SECRET_KEY before starting (or run again with --generate-secrets after removing it)"
  fi
  chown root:root "$env"
  chmod 0600 "$env"
fi

# The unit.
unit="/etc/systemd/system/repomemo.service"
if ! cmp -s "$home/service/repomemo.service" "$unit" 2> /dev/null; then
  install -m 0644 -o root -g root "$home/service/repomemo.service" "$unit"
  say "installed $unit"
fi
systemctl daemon-reload
systemctl enable repomemo > /dev/null 2>&1
say "the service is enabled"

if [ "$start" = yes ]; then
  if grep -Eq '^REPOMEMO_(JWT_SECRET|SECRET_KEY)=$' "$env"; then
    fail "not starting: $env still has an empty secret"
  fi
  systemctl restart repomemo
  say "started: systemctl status repomemo, journalctl -u repomemo -f"
else
  say "done. Start it with: systemctl start repomemo"
fi
