#!/bin/sh
# Fetches a release's container files and checks them (V0.4.41): the compose file, the init script and what goes with them,
# from the release page, against the release's SHA256SUMS. Nothing is unpacked when the check fails. It starts nothing.
#
#   sh quickstart.sh                              the latest release, into ./repomemo
#   sh quickstart.sh --version 0.4.41             a release you choose (pin one for a server you keep)
#   sh quickstart.sh --dir /srv/repomemo          another folder (it must be empty or not exist)
#
# Then, in that folder:
#   sh deploy/init.sh --host memo.example.com     makes .env with strong secrets (--no-edge behind a proxy of your own)
#   docker compose pull && docker compose up -d --wait
#
# Options for mirrors and tests: --repo OWNER/NAME (default repomemo/repomemo), --base-url URL (the folder that holds
# v<version>/ with the files, instead of github.com/<repo>/releases/download).
set -eu

repo="repomemo/repomemo"
version=""
dir="repomemo"
base=""

fail() {
  echo "quickstart: $*" >&2
  exit 1
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version) [ $# -ge 2 ] || fail "--version needs a number"; version="${2#v}"; shift 2 ;;
    --dir) [ $# -ge 2 ] || fail "--dir needs a folder"; dir="$2"; shift 2 ;;
    --repo) [ $# -ge 2 ] || fail "--repo needs owner/name"; repo="$2"; shift 2 ;;
    --base-url) [ $# -ge 2 ] || fail "--base-url needs an address"; base="${2%/}"; shift 2 ;;
    -h|--help) sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) fail "unknown argument: $1 (try --help)" ;;
  esac
done

command -v curl > /dev/null 2>&1 || fail "curl is needed"
command -v tar > /dev/null 2>&1 || fail "tar is needed"
case "$repo" in
  */*) ;;
  *) fail "the repository ($repo) is owner/name" ;;
esac

if [ -z "$version" ]; then
  # /releases/latest redirects to /releases/tag/v<version>.
  latest="$(curl --fail --silent --show-error --location --output /dev/null --write-out '%{url_effective}' "https://github.com/$repo/releases/latest")" || fail "could not find the latest release of $repo"
  version="${latest##*/v}"
fi
case "$version" in
  *[!0-9.]*|"") fail "the version ($version) is digits and dots, such as 0.4.41" ;;
esac

[ -n "$base" ] || base="https://github.com/$repo/releases/download"
release="$base/v$version"
name="repomemo-compose-$version"

if [ -e "$dir" ] && [ -n "$(ls -A "$dir" 2> /dev/null || true)" ]; then
  fail "$dir exists and is not empty; not touching it (choose another with --dir)"
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "quickstart: RepoMemo $version from $release"
curl --fail --silent --show-error --location --output "$work/SHA256SUMS" "$release/SHA256SUMS" || fail "could not download SHA256SUMS of $version"
curl --fail --silent --show-error --location --output "$work/$name.tar.gz" "$release/$name.tar.gz" || fail "could not download $name.tar.gz"

line="$(grep "[ *]$name.tar.gz\$" "$work/SHA256SUMS" || true)"
[ -n "$line" ] || fail "SHA256SUMS does not list $name.tar.gz; not trusting the download"
expected="${line%% *}"
if command -v sha256sum > /dev/null 2>&1; then
  actual="$(sha256sum "$work/$name.tar.gz" | cut -d' ' -f1)"
elif command -v shasum > /dev/null 2>&1; then
  actual="$(shasum -a 256 "$work/$name.tar.gz" | cut -d' ' -f1)"
else
  fail "sha256sum (or shasum) is needed to check the download"
fi
[ "$actual" = "$expected" ] || fail "the checksum of $name.tar.gz is $actual, the release says $expected; nothing was unpacked"
echo "quickstart: checksum ok ($actual)"

mkdir -p "$dir"
tar --extract --gzip --file "$work/$name.tar.gz" --directory "$dir" --strip-components=1
# The install is pinned to the release it was fetched from; `latest` would move it at the next pull.
sed "s|^REPOMEMO_VERSION=.*|REPOMEMO_VERSION=$version|" "$dir/.env.example" > "$dir/.env.example.new"
mv "$dir/.env.example.new" "$dir/.env.example"
echo "quickstart: unpacked into $dir, pinned to $version"
cat <<DONE
Next, in $dir:
  sh deploy/init.sh --host <your host name>      (add --no-edge behind a proxy of your own)
  docker compose pull && docker compose up -d --wait
The first-run setup code is in the server's log: docker compose logs server
DONE
