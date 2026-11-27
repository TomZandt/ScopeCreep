#!/usr/bin/env bash
# Make the macOS Installer from a built app and a built Sender bundle (run on a Mac: it needs pkgbuild and
# productbuild, which come with macOS).
#   scripts/package-macos.sh VERSION APP SENDER_BUNDLE OUT_DIR
# Writes OUT_DIR/scopecreep-VERSION-macos-arm64.pkg and OUT_DIR/SHA256SUMS (a line for every Installer in
# OUT_DIR). Fails if an input is missing. The .pkg is unsigned: it installs ScopeCreep.app to /Applications, the
# Sender bundle to /Library/OFX/Plugins and INSTALL.txt to /Library/Application Support/ScopeCreep, and refuses
# an Intel Mac (packaging/macos/distribution.xml).
set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: $0 VERSION APP SENDER_BUNDLE OUT_DIR" >&2
  exit 2
fi
version=$1 app=$2 bundle=$3 out=$4
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
src=$root/packaging/macos

[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.]+)?$ ]] || { echo "package-macos: '$version' is not a version" >&2; exit 1; }
[[ -f $app && -x $app ]] || { echo "package-macos: the app is missing or not executable: $app" >&2; exit 1; }
[[ -d $bundle && -n $(find "$bundle" -name '*.ofx' -type f -print -quit 2>/dev/null) ]] \
  || { echo "package-macos: the Sender bundle is missing or has no .ofx inside: $bundle" >&2; exit 1; }
for f in distribution.xml components.plist INSTALL.txt welcome.txt; do
  [[ -f $src/$f ]] || { echo "package-macos: $src/$f is missing" >&2; exit 1; }
done
for tool in pkgbuild productbuild; do
  command -v "$tool" >/dev/null || { echo "package-macos: $tool not found (this script runs on macOS)" >&2; exit 1; }
done

name=scopecreep-$version-macos-arm64
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
payload=$stage/root
mkdir -p "$payload/Applications" "$payload/Library/OFX/Plugins" "$payload/Library/Application Support/ScopeCreep" \
  "$stage/pkgs" "$stage/resources" "$out"

"$root/scripts/package-macos-app.sh" "$version" "$app" "$stage/app" >/dev/null
cp -R "$stage/app/ScopeCreep.app" "$payload/Applications/ScopeCreep.app"
cp -R "$bundle" "$payload/Library/OFX/Plugins/scopecreep_sender.ofx.bundle"
install -m644 "$src/INSTALL.txt" "$payload/Library/Application Support/ScopeCreep/INSTALL.txt"
install -m644 "$src/INSTALL.txt" "$src/welcome.txt" "$stage/resources/"
sed "s/@VERSION@/$version/g" "$src/distribution.xml" > "$stage/distribution.xml"

pkgbuild --root "$payload" --install-location / --identifier io.github.tomzandt.scopecreep.pkg \
  --version "$version" --component-plist "$src/components.plist" --ownership recommended \
  "$stage/pkgs/scopecreep-component.pkg" >&2
[[ -e $stage/pkgs/scopecreep-component.pkg ]] || { echo "package-macos: pkgbuild made no package" >&2; exit 1; }

out=$(cd "$out" && pwd)
productbuild --distribution "$stage/distribution.xml" --package-path "$stage/pkgs" \
  --resources "$stage/resources" "$out/$name.pkg" >&2
[[ -e $out/$name.pkg ]] || { echo "package-macos: productbuild made no Installer ($name.pkg)" >&2; exit 1; }

# macOS has shasum, not sha256sum; both print "HASH  NAME".
sum() { if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi; }
(cd "$out" && shopt -s nullglob && sum scopecreep-*.tar.gz scopecreep-*-setup.exe scopecreep-*.pkg > SHA256SUMS)
echo "$out/$name.pkg"
