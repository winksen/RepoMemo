#!/bin/bash
# One request to the converter (V0.2.148): reads LibreOffice's arguments from standard input, NUL-separated,
# runs LibreOffice with them and ends with a line EXIT:<status>. It accepts only what the server sends for a
# version probe or a conversion: LibreOffice's flags, the `pdf` format, and paths inside the previews volume.

set -u
previews=/var/lib/repomemo-previews

args=()
while IFS= read -r -d '' argument; do
  args+=("$argument")
done

for argument in "${args[@]}"; do
  case "$argument" in
    --headless | --norestore | --nolockcheck | --nodefault | --nologo | --version | --convert-to | --outdir | pdf) ;;
    -env:UserInstallation=file://"$previews"/* | -env:UNO_JAVA_JFW_USER_DATA=file://"$previews"/*) ;;
    "$previews"/*)
      case "$argument" in
        *..*) printf '\nEXIT:64\n'; exit 0 ;;
      esac
      ;;
    *) printf '\nEXIT:64\n'; exit 0 ;;
  esac
done

timeout 130 soffice "${args[@]}" 2>/dev/null
status=$?
printf '\nEXIT:%s\n' "$status"
