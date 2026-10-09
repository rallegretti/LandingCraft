#!/usr/bin/env bash
# Install LandingCraft for the current user: the binary, a desktop entry (so it
# appears in application menus) and its icon.
#
#   ./install.sh                  build if needed, install under ~/.local
#   ./install.sh --prefix DIR     install under DIR instead (e.g. /usr/local, as root)
#   ./install.sh --uninstall      remove what was installed (apps in ~/.craftapps are kept)
set -euo pipefail
source "$(dirname "$(readlink -f "$0")")/scripts/common.sh"

PREFIX="$HOME/.local"
UNINSTALL=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --prefix)    [[ $# -ge 2 ]] || die "--prefix needs a directory"; PREFIX="$2"; shift ;;
        --prefix=*)  PREFIX="${1#--prefix=}" ;;
        --uninstall) UNINSTALL=1 ;;
        -h|--help)   sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *)           die "unknown option: $1 (see --help)" ;;
    esac
    shift
done
[[ "$PREFIX" == /* ]] || die "--prefix must be an absolute path"

BIN="$PREFIX/bin/landingcraft"
# The desktop file's name must match the window's Wayland app ID ("landingcraft"),
# which is how desktops associate the running window with its icon.
DESKTOP="$PREFIX/share/applications/landingcraft.desktop"
ICON="$PREFIX/share/icons/hicolor/scalable/apps/landingcraft.svg"

refresh_caches() {
    # Optional helpers; desktops also notice the new files on their own.
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database -q "$PREFIX/share/applications" 2>/dev/null || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1 && [[ -f "$PREFIX/share/icons/hicolor/index.theme" ]]; then
        gtk-update-icon-cache -q -t "$PREFIX/share/icons/hicolor" 2>/dev/null || true
    fi
}

if [[ "$UNINSTALL" == 1 ]]; then
    for f in "$BIN" "$DESKTOP" "$ICON"; do
        if [[ -e "$f" ]]; then rm -f "$f"; info "Removed $f"; fi
    done
    refresh_caches
    info "LandingCraft uninstalled. Installed Crafting Apps and settings were left in place."
    exit 0
fi

# Build with the same options as the last ./build.sh (native release by default).
choose_target_dir
PROFILE=release
PORTABLE=0
[[ -f "$(build_conf)" ]] && source "$(build_conf)"
[[ "$PROFILE" == release ]] || warn "the last build was a debug build; installing that"
flags=(--quiet "--$PROFILE")
[[ "$PORTABLE" == 1 ]] && flags+=(--portable)
"$PROJECT_DIR/build.sh" "${flags[@]}"

install -Dm755 "$TARGET_DIR/$PROFILE/landingcraft" "$BIN"
install -Dm644 "$PROJECT_DIR/packaging/landingcraft.svg" "$ICON"
# Point Exec at the installed binary, since $PREFIX/bin may not be on the
# desktop session's PATH.
mkdir -p "$(dirname "$DESKTOP")"
exec_value="$BIN"
[[ "$BIN" == *[[:space:]]* ]] && exec_value="\"$BIN\""
sed "s|^Exec=landingcraft$|Exec=$exec_value|" "$PROJECT_DIR/packaging/landingcraft.desktop" > "$DESKTOP"
chmod 644 "$DESKTOP"
refresh_caches

info "Installed $BIN"
info "Installed $DESKTOP"
info "Installed $ICON"
info "LandingCraft is now in your application menu."
