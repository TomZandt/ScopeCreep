#!/usr/bin/env bash
# Install or uninstall ScopeCreep: the app and the Sender (the OpenFX plugin for DaVinci Resolve).
#   ./install.sh              install
#   ./install.sh uninstall    remove
# Where things go (override with environment variables):
#   SCOPECREEP_BIN_DIR   the app (default /usr/local/bin when run as root, else ~/.local/bin)
#   SCOPECREEP_OFX_DIR   the Sender (default /usr/OFX/Plugins, which needs admin rights)
#   SCOPECREEP_SUDO      the command used to get admin rights (default sudo)
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
if [[ $EUID -eq 0 ]]; then default_bin=/usr/local/bin; else default_bin="$HOME/.local/bin"; fi
bin_dir=${SCOPECREEP_BIN_DIR:-$default_bin}
ofx_dir=${SCOPECREEP_OFX_DIR:-/usr/OFX/Plugins}
sudo_cmd=${SCOPECREEP_SUDO:-sudo}
bundle=scopecreep_sender.ofx.bundle

# as_owner_or_admin DIR CMD...: runs CMD as you when DIR (or the nearest folder above it) is writable,
# and with admin rights, after saying so, when it is not.
as_owner_or_admin() {
  local dir=$1
  shift
  local probe=$dir
  while [[ ! -e $probe ]]; do probe=$(dirname "$probe"); done
  if [[ -w $probe || $EUID -eq 0 ]]; then
    "$@"
  else
    echo "Admin rights are needed to change $dir. You will be asked for your password."
    "$sudo_cmd" "$@"
  fi
}

install_all() {
  [[ -f $here/scopecreep && -d $here/$bundle ]] || { echo "install.sh: the app or the Sender is missing next to this script" >&2; exit 1; }
  as_owner_or_admin "$bin_dir" mkdir -p "$bin_dir"
  as_owner_or_admin "$bin_dir" install -m755 "$here/scopecreep" "$bin_dir/scopecreep"
  echo "Installed the app: $bin_dir/scopecreep"
  as_owner_or_admin "$ofx_dir" mkdir -p "$ofx_dir"
  as_owner_or_admin "$ofx_dir" rm -rf "$ofx_dir/$bundle"
  as_owner_or_admin "$ofx_dir" cp -r "$here/$bundle" "$ofx_dir/$bundle"
  echo "Installed the Sender: $ofx_dir/$bundle"
  echo "Restart DaVinci Resolve: it only looks for OpenFX plugins when it starts."
}

uninstall_all() {
  as_owner_or_admin "$bin_dir" rm -f "$bin_dir/scopecreep"
  as_owner_or_admin "$ofx_dir" rm -rf "$ofx_dir/$bundle"
  echo "Removed ScopeCreep. Restart DaVinci Resolve so it forgets the Sender."
}

case "${1:-install}" in
  install) install_all ;;
  uninstall) uninstall_all ;;
  *) echo "usage: $0 [install|uninstall]" >&2; exit 2 ;;
esac
