#!/usr/bin/env bash
# Build LandingCraft's Windows release file: a zip holding landingcraft.exe, the README and the licence.
#
#   packaging/windows/package.sh    build for this PC's CPU and package it
#
# Runs in Git Bash (or MSYS2) with Rust's GNU toolchain and MinGW-w64 (see the README). The build
# targets any CPU of this architecture (no -C target-cpu=native) and links the C runtime statically,
# so the exe needs only DLLs that come with Windows, plus a Vulkan driver for the GPU.
# Output goes to <target dir>/windows/landingcraft-<version>-windows-<arch>.zip.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/scripts/common.sh"

QUIET=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        -q|--quiet) QUIET=1 ;;
        -h|--help)  sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *)          die "unknown option: $1 (see --help)" ;;
    esac
    shift
done

[[ "$WINDOWS" == 1 ]] || die "this script builds the Windows release, so it has to run on Windows (in Git Bash)."
find_cargo
find_mingw
choose_target_dir
cd "$PROJECT_DIR"

VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)"
[[ -n "$VERSION" ]] || die "couldn't read the version from Cargo.toml"
HOST="$("$(dirname "$CARGO")/rustc" -vV | sed -n 's/^host: //p')"
case "$HOST" in
    x86_64-*)  ARCH=x64; MACHINE=x86-64 ;;
    aarch64-*) ARCH=arm64; MACHINE=aarch64 ;;
    *)         die "no Windows release is made for $HOST" ;;
esac
BASE="landingcraft-$VERSION-windows-$ARCH"
OUT="$TARGET_DIR/windows"
mkdir -p "$OUT"

# ---- build -------------------------------------------------------------------------------------
# The static C runtime is set for the target only, so build scripts aren't built with it. That
# takes an explicit --target, which puts the build in <target dir>/<host>/release.
info "Building release for Windows $ARCH ($HOST)"
flags_var="CARGO_TARGET_$(tr 'a-z-' 'A-Z_' <<<"$HOST")_RUSTFLAGS"
(
    unset RUSTFLAGS
    export "$flags_var=-C target-feature=+crt-static"
    export LANDINGCRAFT_REQUIRE_WINRES=1
    "$CARGO" build --release --locked --target "$HOST"
)
BIN="$TARGET_DIR/$HOST/release/landingcraft.exe"
[[ -f "$BIN" ]] || die "the build produced no binary at $BIN"

# A GUI program (no console window opens with it), for this CPU, needing only DLLs that come with
# Windows: anything named lib*.dll would be MinGW's, missing on other PCs.
headers="$(objdump -p "$BIN")"
grep -q 'Subsystem.*(Windows GUI)' <<<"$headers" || die "$BIN isn't a Windows GUI program"
objdump -f "$BIN" | grep -qi "architecture: .*$MACHINE" || die "$BIN isn't built for $ARCH"
dlls="$(sed -n 's/^[[:space:]]*DLL Name: //p' <<<"$headers" | sort -fu)"
if grep -qi '^lib' <<<"$dlls"; then
    die "$BIN needs MinGW DLLs that Windows doesn't have: $(grep -i '^lib' <<<"$dlls" | tr '\n' ' ')"
fi
info "Built $BIN (uses $(tr '\n' ' ' <<<"$dlls"))"

# ---- zip ---------------------------------------------------------------------------------------
# One top-level folder, like the Crafting Apps' portable zips. Made with the tar that comes with
# Windows (bsdtar), which writes zip files.
stage="$(mktemp -d "$OUT/.stage-XXXXXX")"
trap 'rm -rf "$stage"' EXIT
mkdir "$stage/$BASE"
cp "$BIN" "$stage/$BASE/landingcraft.exe"
cp README.md LICENSE "$stage/$BASE/"
rm -f "$OUT/$BASE.zip"
"$(cygpath -u "${SYSTEMROOT:-C:/Windows}")/System32/tar.exe" -a -c -f "$(cygpath -w "$OUT/$BASE.zip")" \
    -C "$(cygpath -w "$stage")" "$BASE"
info "Built $OUT/$BASE.zip"
