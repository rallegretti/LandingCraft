#!/usr/bin/env bash
# Build LandingCraft's Linux release files: a tarball and an AppImage.
#
#   packaging/linux/package.sh               build in an Ubuntu 22.04 container (needs Docker or Podman)
#   packaging/linux/package.sh --host        build on this machine instead (runs only on glibc >= this one's)
#   packaging/linux/package.sh --no-appimage stop after the tarball
#
# The container build runs on any distro with glibc 2.35 or newer (Ubuntu 22.04+, Debian 12+), the same
# baseline as the Crafting Apps. Both builds target any CPU of this architecture (no -C target-cpu=native).
# The AppImage is made with appimagetool: $APPIMAGETOOL, then $PATH, else it's downloaded once.
# Output goes to <target dir>/linux/: landingcraft-<version>-linux-<arch>.tar.gz and .AppImage.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/scripts/common.sh"

IMAGE=ubuntu:22.04
HOST=0
APPIMAGE=1
QUIET=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --host)        HOST=1 ;;
        --no-appimage) APPIMAGE=0 ;;
        -q|--quiet)    QUIET=1 ;;
        -h|--help)     sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *)             die "unknown option: $1 (see --help)" ;;
    esac
    shift
done

[[ "$OS" == Linux ]] || die "this script builds the Linux release, so it has to run on Linux."
choose_target_dir
cd "$PROJECT_DIR"

VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)"
[[ -n "$VERSION" ]] || die "couldn't read the version from Cargo.toml"
ARCH="$(uname -m)"
BASE="landingcraft-$VERSION-linux-$ARCH"
OUT="$TARGET_DIR/linux"
mkdir -p "$OUT"

# ---- build -------------------------------------------------------------------------------------
if [[ "$HOST" == 1 ]]; then
    find_cargo
    info "Building release on this machine (needs glibc $(ldd --version | head -n 1 | awk '{print $NF}') or newer to run)"
    (unset RUSTFLAGS; "$CARGO" build --release --locked)
    BIN="$TARGET_DIR/release/landingcraft"
else
    if command -v docker >/dev/null 2>&1; then ENGINE=docker
    elif command -v podman >/dev/null 2>&1; then ENGINE=podman
    else die "neither Docker nor Podman is installed; install one, or build with --host"
    fi
    # The same Rust as this machine when there is one, so both builds match.
    RUST="${LANDINGCRAFT_RUST:-$( (rustc --version || "${CARGO_HOME:-$HOME/.cargo}/bin/rustc" --version) 2>/dev/null | awk '{print $2}')}"
    RUST="${RUST:-stable}"
    # Kept between runs: the toolchain and crates, and a target dir of its own (other glibc).
    mkdir -p "$OUT/cargo-home" "$OUT/target"
    info "Building release in $IMAGE with Rust $RUST ($ENGINE)"
    "$ENGINE" run --rm \
        -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" -e RUST="$RUST" \
        -v "$PROJECT_DIR:/src:ro" -v "$OUT/cargo-home:/cargo-home" -v "$OUT/target:/target" \
        "$IMAGE" bash -euo pipefail -c '
            export DEBIAN_FRONTEND=noninteractive CARGO_HOME=/cargo-home RUSTUP_HOME=/cargo-home/rustup
            export PATH=/cargo-home/bin:$PATH
            apt-get update -qq
            apt-get install -y -qq --no-install-recommends ca-certificates curl gcc libc6-dev >/dev/null
            if ! command -v rustup >/dev/null; then
                curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs \
                    | sh -s -- -y --profile minimal --default-toolchain none --no-modify-path
            fi
            rustup toolchain install "$RUST" --profile minimal
            cd /src
            status=0
            CARGO_TARGET_DIR=/target cargo +"$RUST" build --release --locked || status=$?
            chown -R "$HOST_UID:$HOST_GID" /cargo-home /target
            exit $status'
    BIN="$OUT/target/release/landingcraft"
fi
[[ -x "$BIN" ]] || die "the build produced no binary at $BIN"
if command -v objdump >/dev/null 2>&1; then
    glibc="$(objdump -T "$BIN" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -n 1)"
    info "Built $BIN (needs ${glibc/_/ } or newer)"
fi

# ---- tarball -----------------------------------------------------------------------------------
# A plain bin/ + share/ tree, like the Crafting Apps' tarballs.
stage="$(mktemp -d "$OUT/.stage-XXXXXX")"
trap 'rm -rf "$stage"' EXIT
tree="$stage/$BASE"
install -Dm755 "$BIN" "$tree/bin/landingcraft"
install -Dm644 packaging/landingcraft.desktop "$tree/share/applications/landingcraft.desktop"
install -Dm644 packaging/landingcraft.svg "$tree/share/icons/hicolor/scalable/apps/landingcraft.svg"
install -Dm644 README.md "$tree/share/doc/landingcraft/README.md"
install -Dm644 LICENSE "$tree/share/doc/landingcraft/LICENSE"
if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "$tree/share/applications/landingcraft.desktop"
fi
tar -C "$stage" --owner=0 --group=0 --sort=name -czf "$OUT/$BASE.tar.gz" "$BASE"
info "Built $OUT/$BASE.tar.gz"
[[ "$APPIMAGE" == 1 ]] || exit 0

# ---- AppImage ----------------------------------------------------------------------------------
appdir="$stage/AppDir"
mkdir -p "$appdir/usr"
cp -R "$tree/bin" "$tree/share" "$appdir/usr/"
rm -rf "$appdir/usr/share/doc"
ln -s usr/bin/landingcraft "$appdir/AppRun"
cp packaging/landingcraft.desktop packaging/landingcraft.svg "$appdir/"
ln -s landingcraft.svg "$appdir/.DirIcon"

tool="${APPIMAGETOOL:-$(command -v appimagetool || true)}"
if [[ -z "$tool" ]]; then
    tool="$OUT/appimagetool-$ARCH.AppImage"
    if [[ ! -x "$tool" ]]; then
        info "Downloading appimagetool"
        curl -fsSL -o "$tool.part" \
            "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$ARCH.AppImage"
        chmod +x "$tool.part" && mv "$tool.part" "$tool"
    fi
fi
rm -f "$OUT/$BASE.AppImage"
# Extract-and-run: appimagetool works without FUSE, in containers too. It embeds the static
# type-2 runtime, so the AppImage itself needs no libfuse2 either.
ARCH="$ARCH" APPIMAGE_EXTRACT_AND_RUN=1 "$tool" --no-appstream "$appdir" "$OUT/$BASE.AppImage" >/dev/null
info "Built $OUT/$BASE.AppImage"
