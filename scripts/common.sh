# Shared setup for build.sh, run.sh, install.sh, release.sh and packaging/*/*.sh. Sourced, not executed.

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OS="$(uname -s)"
# Windows, in Git Bash or MSYS2: uname -s says MINGW64_NT-…, MSYS_NT-… or similar.
case "$OS" in
    MINGW*|MSYS*|CYGWIN*) WINDOWS=1; EXE=.exe ;;
    *)                    WINDOWS=0; EXE= ;;
esac

die()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*" >&2; }
info() { if [[ "${QUIET:-0}" != 1 ]]; then printf '\033[1;34m==>\033[0m %s\n' "$*" >&2; fi; }

# Locate cargo: PATH first, then rustup's default location (rustup was
# installed with --no-modify-path, so ~/.cargo/bin may not be on PATH).
find_cargo() {
    if command -v cargo >/dev/null 2>&1; then
        CARGO="$(command -v cargo)"
    elif [[ -x "${CARGO_HOME:-$HOME/.cargo}/bin/cargo" ]]; then
        CARGO="${CARGO_HOME:-$HOME/.cargo}/bin/cargo"
    else
        die "cargo not found. Install Rust from https://rustup.rs, then re-run."
    fi
}

# Windows with Rust's GNU toolchain: the windows crates need MinGW-w64's dlltool, and the
# icon needs its windres. rustup's own MinGW files lack both (dlltool needs an assembler),
# so use the ones on PATH, else find WinLibs where winget installs it, or MSYS2's.
find_mingw() {
    [[ "$WINDOWS" == 1 ]] || return 0
    [[ "$("$(dirname "$CARGO")/rustc" -vV | sed -n 's/^host: //p')" == *-gnu ]] || return 0
    command -v dlltool >/dev/null 2>&1 && command -v windres >/dev/null 2>&1 && return 0
    local dir local_app_data
    local_app_data="$(cygpath -u "${LOCALAPPDATA:-$USERPROFILE/AppData/Local}")"
    for dir in "$local_app_data"/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.*/mingw64/bin /c/msys64/mingw64/bin; do
        if [[ -x "$dir/dlltool.exe" && -x "$dir/windres.exe" ]]; then
            export PATH="$dir:$PATH"
            return 0
        fi
    done
    die "MinGW-w64 (dlltool and windres) not found. Install it with: winget install BrechtSanders.WinLibs.POSIX.MSVCRT"
}

# Cargo writes thousands of small files; on NTFS/FAT/exFAT (often mounted via
# FUSE) that is slow, so keep build output on the home filesystem instead.
# On Windows, NTFS is the home filesystem. An explicit CARGO_TARGET_DIR always wins.
choose_target_dir() {
    if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
        TARGET_DIR="$CARGO_TARGET_DIR"
        return
    fi
    if [[ "$WINDOWS" == 1 ]]; then
        TARGET_DIR="$PROJECT_DIR/target"
        export CARGO_TARGET_DIR="$TARGET_DIR"
        return
    fi
    local fstype cache="${XDG_CACHE_HOME:-$HOME/.cache}"
    if [[ "$OS" == Darwin ]]; then
        fstype="$(macos_fstype "$PROJECT_DIR")"
        cache="$HOME/Library/Caches"
    else
        fstype="$(findmnt -no FSTYPE -T "$PROJECT_DIR" 2>/dev/null || stat -f -c %T "$PROJECT_DIR")"
    fi
    case "$fstype" in
        fuseblk|fuse*|*fuse|ntfs*|vfat|msdos|exfat)
            TARGET_DIR="$cache/landingcraft/target" ;;
        *)
            TARGET_DIR="$PROJECT_DIR/target" ;;
    esac
    export CARGO_TARGET_DIR="$TARGET_DIR"
}

# macOS: the filesystem type of the volume holding $1 ("apfs", "exfat", "msdos", "macfuse", ...),
# read from mount's "(type, options)" column, since BSD stat can't report it.
macos_fstype() {
    local dev
    dev="$(df -P "$1" 2>/dev/null | awk 'NR == 2 { print $1 }')" || true
    mount | awk -v dev="$dev" 'index($0, dev " on ") == 1 { sub(/.*\(/, ""); sub(/[,)].*/, ""); print; exit }' || true
}

# Remembers the flags of the last build so run.sh rebuilds the same way
# (changing RUSTFLAGS would otherwise trigger a full rebuild).
build_conf() { echo "$TARGET_DIR/landingcraft-build.conf"; }
