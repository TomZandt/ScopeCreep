#!/usr/bin/env bash
# Print a version's CHANGELOG.md section, then a "Full changelog" link.
#   scripts/release-notes.sh X.Y.Z [CHANGELOG]   (CHANGELOG defaults to the repo's)
# Fails if the changelog has no section for the version.
set -euo pipefail

[[ $# -ge 1 && $# -le 2 ]] || { echo "usage: $0 VERSION [CHANGELOG]" >&2; exit 2; }
version=${1#v}
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
changelog=${2:-$root/CHANGELOG.md}
repo_url=${SCOPECREEP_REPO_URL:-https://github.com/TomZandt/ScopeCreep}

# The lines between the version's heading and the next heading or the link list at the end,
# without the blank lines at either end.
section=$(awk -v h="## [$version]" '
  index($0, h) == 1 { on = 1; next }
  on && (/^## \[/ || /^\[[^]]+\]: /) { exit }
  on { if (NF) { while (blank-- > 0) print ""; blank = 0; print; started = 1 } else if (started) blank++ }
' "$changelog")
[[ -n $section ]] || { echo "release-notes: no section for $version in $changelog" >&2; exit 1; }

printf '%s\n\n' "$section"
echo "Full changelog: $repo_url/blob/v$version/CHANGELOG.md"
