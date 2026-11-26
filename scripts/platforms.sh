#!/usr/bin/env bash
# Print the platforms in packaging/platforms.txt, one per line (comments and blanks skipped).
#   scripts/platforms.sh [--test-builds] [--json] [FILE]
# Each line is `NAME [yes|no]`: whether Releases include that platform (a bare name means yes).
# By default only the Release platforms (yes) are printed; --test-builds prints every platform, since a
# Test build carries them all. --json prints a JSON array instead (for a CI matrix).
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
json=0 all=0
while [[ ${1:-} == --* ]]; do
  case $1 in
    --json) json=1 ;;
    --test-builds) all=1 ;;
    *) echo "platforms: unknown option $1" >&2; exit 2 ;;
  esac
  shift
done
file=${1:-$root/packaging/platforms.txt}

names=()
while read -r name release extra || [[ -n ${name:-} ]]; do
  [[ -z $name || $name == \#* ]] && continue
  release=${release:-yes}
  [[ $release == yes || $release == no ]] && [[ -z $extra ]] \
    || { echo "platforms: '$name $release $extra' in $file: the second word must be yes or no" >&2; exit 1; }
  [[ $all -eq 1 || $release == yes ]] && names+=("$name")
done < "$file"
[[ ${#names[@]} -gt 0 ]] || { echo "platforms: no platform in $file" >&2; exit 1; }
if [[ $json -eq 1 ]]; then
  printf '%s\n' "${names[@]}" | jq -R . | jq -s -c .
else
  printf '%s\n' "${names[@]}"
fi
