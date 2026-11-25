#!/usr/bin/env bash
# Print the platforms in packaging/platforms.txt, one per line (comments and blanks skipped).
#   scripts/platforms.sh [--json] [FILE]   --json prints a JSON array instead (for a CI matrix)
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
json=0
if [[ ${1:-} == --json ]]; then json=1; shift; fi
file=${1:-$root/packaging/platforms.txt}

names=$(grep -v -E '^[[:space:]]*(#|$)' "$file" || true)
[[ -n $names ]] || { echo "platforms: no platform in $file" >&2; exit 1; }
if [[ $json -eq 1 ]]; then
  printf '%s\n' "$names" | jq -R . | jq -s -c .
else
  printf '%s\n' "$names"
fi
