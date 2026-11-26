#!/usr/bin/env bash
# Make the Linux Installer from a built app and a built Sender bundle.
#   scripts/package-linux.sh VERSION APP SENDER_BUNDLE OUT_DIR
# Writes OUT_DIR/scopecreep-VERSION-linux-x86_64.tar.gz and OUT_DIR/SHA256SUMS
# (a line for every scopecreep-*.tar.gz in OUT_DIR). Fails if an input is missing.
set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: $0 VERSION APP SENDER_BUNDLE OUT_DIR" >&2
  exit 2
fi
version=$1 app=$2 bundle=$3 out=$4
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
src=$root/packaging/linux

[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.]+)?$ ]] || { echo "package-linux: '$version' is not a version" >&2; exit 1; }
[[ -f $app && -x $app ]] || { echo "package-linux: the app is missing or not executable: $app" >&2; exit 1; }
[[ -d $bundle && -n $(find "$bundle" -name '*.ofx' -type f -print -quit 2>/dev/null) ]] \
  || { echo "package-linux: the Sender bundle is missing or has no .ofx inside: $bundle" >&2; exit 1; }
for f in install.sh INSTALL.txt; do
  [[ -f $src/$f ]] || { echo "package-linux: $src/$f is missing" >&2; exit 1; }
done

name=scopecreep-$version-linux-x86_64
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/$name" "$out"
install -m755 "$app" "$stage/$name/scopecreep"
cp -r "$bundle" "$stage/$name/scopecreep_sender.ofx.bundle"
install -m755 "$src/install.sh" "$stage/$name/install.sh"
install -m644 "$src/INSTALL.txt" "$stage/$name/INSTALL.txt"

out=$(cd "$out" && pwd)
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime='2000-01-01 00:00:00' \
  -C "$stage" -czf "$out/$name.tar.gz" "$name"
(cd "$out" && shopt -s nullglob && sha256sum scopecreep-*.tar.gz scopecreep-*-setup.exe > SHA256SUMS)
echo "$out/$name.tar.gz"
