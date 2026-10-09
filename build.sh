#!/usr/bin/env bash
# Build LandingCraft, tuned for this machine.
#
#   ./build.sh              optimised release build for this CPU
#   ./build.sh --portable   release build without CPU-specific instructions
#   ./build.sh --debug      fast-compiling debug build
#   ./build.sh --clean      remove build output first
set -euo pipefail
source "$(dirname "$(readlink -f "$0")")/scripts/common.sh"

PROFILE=release
PORTABLE=0
CLEAN=0
QUIET=0
for arg in "$@"; do
    case "$arg" in
        --debug)    PROFILE=debug ;;
        --release)  PROFILE=release ;;
        --portable) PORTABLE=1 ;;
        --clean)    CLEAN=1 ;;
        -q|--quiet) QUIET=1 ;;
        -h|--help)  sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *)          die "unknown option: $arg (see --help)" ;;
    esac
done

find_cargo
find_mingw
choose_target_dir
cd "$PROJECT_DIR"

# Vulkan is loaded at runtime, so its absence doesn't stop the build, but say so early.
# (macOS renders with Metal, which every supported Mac has.)
if [[ "$WINDOWS" == 1 ]]; then
    [[ -f "$(cygpath -u "${SYSTEMROOT:-C:/Windows}")/System32/vulkan-1.dll" ]] \
        || warn "vulkan-1.dll not found; the launcher needs a Vulkan driver for your GPU to run (GPU drivers include one)."
elif [[ "$OS" != Darwin ]] && ! ldconfig -p 2>/dev/null | grep 'libvulkan\.so\.1' >/dev/null; then
    warn "libvulkan.so.1 not found; the launcher needs a Vulkan loader and driver to run (e.g. libvulkan1 + mesa-vulkan-drivers)."
fi

if [[ "$CLEAN" == 1 ]]; then
    info "Cleaning $TARGET_DIR"
    "$CARGO" clean
fi

if [[ "$PORTABLE" == 0 ]]; then
    export RUSTFLAGS="${RUSTFLAGS:-} -C target-cpu=native"
fi

args=(build --locked)
[[ "$PROFILE" == release ]] && args+=(--release)
[[ "$QUIET" == 1 ]] && args+=(--quiet)

info "Building $PROFILE ($([[ $PORTABLE == 1 ]] && echo portable || echo "native CPU")) into $TARGET_DIR"
"$CARGO" "${args[@]}"

mkdir -p "$TARGET_DIR"
printf 'PROFILE=%s\nPORTABLE=%s\n' "$PROFILE" "$PORTABLE" > "$(build_conf)"

info "Built $TARGET_DIR/$PROFILE/landingcraft$EXE"
