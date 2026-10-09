#!/usr/bin/env bash
# Run LandingCraft, rebuilding first if the sources changed.
#
#   ./run.sh                 build if needed, then launch
#   ./run.sh --gpu NAME      render on the GPU whose name contains NAME
#   ./run.sh --list-gpus     show the GPUs on this system
#   ./run.sh --no-build      launch the last build as-is
set -euo pipefail
source "$(dirname "$(readlink -f "$0")")/scripts/common.sh"

BUILD=1
while [[ $# -gt 0 ]]; do
    case "$1" in
        --gpu)       [[ $# -ge 2 ]] || die "--gpu needs a device name"
                     export LANDINGCRAFT_GPU="$2"; shift ;;
        --gpu=*)     export LANDINGCRAFT_GPU="${1#--gpu=}" ;;
        --list-gpus)
            if [[ "$OS" == Darwin ]]; then
                system_profiler SPDisplaysDataType 2>/dev/null | awk -F': ' '/Chipset Model/ {print "  " $2}'
                exit 0
            fi
            command -v vulkaninfo >/dev/null 2>&1 \
                || die "vulkaninfo not installed (package vulkan-tools); the app's Settings page also lists devices."
            vulkaninfo --summary 2>/dev/null | awk -F'= ' '/deviceName/ {print "  " $2}'
            exit 0 ;;
        --no-build)  BUILD=0 ;;
        -h|--help)   sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *)           die "unknown option: $1 (see --help)" ;;
    esac
    shift
done

[[ "$OS" == Darwin || -n "${WAYLAND_DISPLAY:-}${DISPLAY:-}" ]] || die "no graphical session found (neither WAYLAND_DISPLAY nor DISPLAY is set)."

choose_target_dir

# Rebuild with the same flags as the last ./build.sh so nothing is recompiled needlessly.
PROFILE=release
PORTABLE=0
[[ -f "$(build_conf)" ]] && source "$(build_conf)"

if [[ "$BUILD" == 1 ]]; then
    flags=(--quiet "--$PROFILE")
    [[ "$PORTABLE" == 1 ]] && flags+=(--portable)
    "$PROJECT_DIR/build.sh" "${flags[@]}"
fi

BIN="$TARGET_DIR/$PROFILE/landingcraft"
[[ -x "$BIN" ]] || die "$BIN not found; run ./build.sh first."

exec "$BIN"
