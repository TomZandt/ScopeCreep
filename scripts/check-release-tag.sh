#!/usr/bin/env bash
# Refuse a release tag that isn't ready to publish (spec #233, ticket #243).
#   scripts/check-release-tag.sh vX.Y.Z [CARGO_TOML] [CHANGELOG]   (default to the repo's)
# Fails if the tag isn't vX.Y.Z, doesn't match the workspace version, or the changelog
# has no section for it. Prints the version on success.
set -euo pipefail

[[ $# -ge 1 && $# -le 3 ]] || { echo "usage: $0 TAG [CARGO_TOML] [CHANGELOG]" >&2; exit 2; }
tag=$1
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cargo=${2:-$root/Cargo.toml}
changelog=${3:-$root/CHANGELOG.md}

[[ $tag =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]] \
  || { echo "check-release-tag: '$tag' is not a vX.Y.Z tag" >&2; exit 1; }
version=${tag#v}

workspace=$("$root/scripts/workspace-version.sh" "$cargo")
[[ $workspace == "$version" ]] \
  || { echo "check-release-tag: tag $tag does not match the workspace version $workspace" >&2; exit 1; }

"$root/scripts/release-notes.sh" "$version" "$changelog" > /dev/null
echo "$version"
