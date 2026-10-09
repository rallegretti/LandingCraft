#!/usr/bin/env bash
# Install LandingCraft.app on this Mac. ./install.sh runs this on macOS.
#
#   ./install.sh                  build for this Mac and install into ~/Applications
#   ./install.sh --prefix DIR     install into DIR instead (e.g. /Applications)
#   ./install.sh --uninstall      remove LandingCraft.app (installed Crafting Apps are kept)
#
# Signed ad hoc unless MACOS_SIGN_IDENTITY names a Developer ID.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/scripts/common.sh"

DEST="$HOME/Applications"
UNINSTALL=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --prefix)    [[ $# -ge 2 ]] || die "--prefix needs a directory"; DEST="$2"; shift ;;
        --prefix=*)  DEST="${1#--prefix=}" ;;
        --uninstall) UNINSTALL=1 ;;
        -h|--help)   sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *)           die "unknown option: $1 (see --help)" ;;
    esac
    shift
done
[[ "$DEST" == /* ]] || die "--prefix must be an absolute path"
APP="$DEST/LandingCraft.app"

if [[ "$UNINSTALL" == 1 ]]; then
    if [[ -d "$APP" ]]; then rm -rf "$APP"; info "Removed $APP"; fi
    info "LandingCraft uninstalled. Installed Crafting Apps and settings were left in place."
    exit 0
fi

"$PROJECT_DIR/packaging/macos/package.sh" --native --app-only --quiet
choose_target_dir
mkdir -p "$DEST"
rm -rf "$APP"
ditto "$TARGET_DIR/macos/LandingCraft.app" "$APP"
info "Installed $APP"
info "LandingCraft is now in Launchpad and Spotlight."
