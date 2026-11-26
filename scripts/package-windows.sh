#!/usr/bin/env bash
# Make the Windows Installer from a built app and a built Sender bundle (run in Git Bash on Windows).
#   scripts/package-windows.sh VERSION APP SENDER_BUNDLE OUT_DIR
# Writes OUT_DIR/scopecreep-VERSION-windows-x86_64-setup.exe and OUT_DIR/SHA256SUMS (a line for every
# Installer in OUT_DIR). Fails if an input is missing. The setup .exe is compiled by Inno Setup 6 from
# packaging/windows/scopecreep.iss; SCOPECREEP_ISCC names the compiler (default: ISCC on PATH, then
# Inno Setup 6's own folder).
set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: $0 VERSION APP SENDER_BUNDLE OUT_DIR" >&2
  exit 2
fi
version=$1 app=$2 bundle=$3 out=$4
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
src=$root/packaging/windows

[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.]+)?$ ]] || { echo "package-windows: '$version' is not a version" >&2; exit 1; }
[[ -f $app ]] || { echo "package-windows: the app is missing: $app" >&2; exit 1; }
[[ -d $bundle && -n $(find "$bundle" -name '*.ofx' -type f -print -quit 2>/dev/null) ]] \
  || { echo "package-windows: the Sender bundle is missing or has no .ofx inside: $bundle" >&2; exit 1; }
for f in scopecreep.iss INSTALL.txt; do
  [[ -f $src/$f ]] || { echo "package-windows: $src/$f is missing" >&2; exit 1; }
done
iscc=${SCOPECREEP_ISCC:-}
if [[ -z $iscc ]]; then
  iscc=$(command -v iscc || command -v ISCC || true)
  [[ -n $iscc ]] || iscc="/c/Program Files (x86)/Inno Setup 6/ISCC.exe"
fi
[[ -x $iscc ]] || { echo "package-windows: no Inno Setup compiler at $iscc (set SCOPECREEP_ISCC)" >&2; exit 1; }

name=scopecreep-$version-windows-x86_64-setup
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir -p "$out"
# Everything the script names sits beside it, so ISCC gets no paths to translate between Git Bash and Windows.
cp "$app" "$stage/scopecreep.exe"
cp -r "$bundle" "$stage/scopecreep_sender.ofx.bundle"
cp "$src/scopecreep.iss" "$src/INSTALL.txt" "$stage/"
printf '#define AppVersion "%s"\n#define OutputName "%s"\n' "$version" "$name" > "$stage/version.iss"

(cd "$stage" && "$iscc" scopecreep.iss) >&2
[[ -f $stage/Output/$name.exe ]] || { echo "package-windows: Inno Setup made no Installer ($name.exe)" >&2; exit 1; }

out=$(cd "$out" && pwd)
cp "$stage/Output/$name.exe" "$out/"
(cd "$out" && shopt -s nullglob && sha256sum scopecreep-*.tar.gz scopecreep-*-setup.exe > SHA256SUMS)
echo "$out/$name.exe"
