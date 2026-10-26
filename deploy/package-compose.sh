#!/bin/sh
# Packs the files of a container install for a release (V0.4.41): repomemo-compose-<version>.tar.gz, with the files listed in
# deploy/compose-bundle.txt under one folder, repomemo-compose-<version>/. The same bytes every time: sorted names, no owner,
# the time given by SOURCE_DATE_EPOCH (the commit's, by default), and gzip without a name or a time.
#
#   deploy/package-compose.sh 0.4.41 dist        writes dist/repomemo-compose-0.4.41.tar.gz
set -eu

version="${1:-}"
out="${2:-.}"
[ -n "$version" ] || { echo "package-compose: give the version: package-compose.sh 0.4.41 [out-folder]" >&2; exit 1; }
case "$version" in
  *[!0-9.]*|"") echo "package-compose: the version ($version) is digits and dots" >&2; exit 1 ;;
esac

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
list="$here/compose-bundle.txt"
epoch="${SOURCE_DATE_EPOCH:-$(git -C "$root" log -1 --format=%ct 2> /dev/null || echo 0)}"
name="repomemo-compose-$version"

mkdir -p "$out"
out="$(cd "$out" && pwd)"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/$name"

files="$(grep -v '^#' "$list" | grep -v '^$')"
for path in $files; do
  [ -f "$root/$path" ] || { echo "package-compose: $path is in $list but is not there" >&2; exit 1; }
  mkdir -p "$stage/$name/$(dirname "$path")"
  cp "$root/$path" "$stage/$name/$path"
done
printf '%s\n' "$version" > "$stage/$name/VERSION"

(
  cd "$stage"
  find "$name" -type f | LC_ALL=C sort > files.txt
  tar --create --file archive.tar --owner=0 --group=0 --numeric-owner --mtime="@$epoch" --no-recursion --files-from files.txt
  gzip --no-name --best --stdout archive.tar > "$out/$name.tar.gz"
)
echo "package-compose: $out/$name.tar.gz ($(echo "$files" | wc -l | tr -d ' ') files)"
