#!/usr/bin/env bash
# Print the workspace version (the first `version = "..."` in Cargo.toml).
#   scripts/workspace-version.sh [CARGO_TOML]   (defaults to the repo's)
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cargo=${1:-$root/Cargo.toml}
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$cargo" | head -1)
[[ -n $version ]] || { echo "workspace-version: no version in $cargo" >&2; exit 1; }
echo "$version"
