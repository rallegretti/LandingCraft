# Shared setup for build.sh, run.sh, install.sh and packaging/macos/*.sh. Sourced, not executed.

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OS="$(uname -s)"

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

# Cargo writes thousands of small files; on NTFS/FAT/exFAT (often mounted via
# FUSE) that is slow, so keep build output on the home filesystem instead.
# An explicit CARGO_TARGET_DIR always wins.
choose_target_dir() {
    if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
        TARGET_DIR="$CARGO_TARGET_DIR"
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
