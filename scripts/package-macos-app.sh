#!/usr/bin/env bash
# Make ScopeCreep.app from a built app (the first half of the macOS Installer: the .pkg that puts this and the
# Sender in place is scripts/package-macos.sh, #257).
#   scripts/package-macos-app.sh VERSION APP OUT_DIR
# Writes OUT_DIR/ScopeCreep.app (Info.plist with VERSION, the executable, the placeholder icon) and prints its
# path. On a Mac it signs the bundle ad hoc, which Apple Silicon needs to run it; nothing is notarised.
# Fails if an input is missing.
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $0 VERSION APP OUT_DIR" >&2
  exit 2
fi
version=$1 app=$2 out=$3
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
src=$root/packaging/macos

[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.]+)?$ ]] || { echo "package-macos-app: '$version' is not a version" >&2; exit 1; }
[[ -f $app && -x $app ]] || { echo "package-macos-app: the app is missing or not executable: $app" >&2; exit 1; }
for f in Info.plist ScopeCreep.icns; do
  [[ -f $src/$f ]] || { echo "package-macos-app: $src/$f is missing" >&2; exit 1; }
done

mkdir -p "$out"
out=$(cd "$out" && pwd)
bundle=$out/ScopeCreep.app
rm -rf "$bundle"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
install -m755 "$app" "$bundle/Contents/MacOS/scopecreep"
sed "s/@VERSION@/$version/g" "$src/Info.plist" > "$bundle/Contents/Info.plist"
install -m644 "$src/ScopeCreep.icns" "$bundle/Contents/Resources/ScopeCreep.icns"
printf 'APPL????' > "$bundle/Contents/PkgInfo"
if [[ $(uname -s) == Darwin ]] && command -v codesign >/dev/null; then
  codesign --force --deep --sign - "$bundle" >&2
fi
echo "$bundle"
