#!/usr/bin/env bash
# Build LandingCraft.app for macOS, sign it, and wrap it in a disk image.
#
#   packaging/macos/package.sh                    universal (Apple silicon + Intel) .app and .dmg, ad-hoc signed
#   packaging/macos/package.sh --sign IDENTITY    sign with a Developer ID, e.g. "Developer ID Application: Name (TEAMID)"
#   packaging/macos/package.sh --notarize PROFILE also notarize with this notarytool keychain profile, then staple
#   packaging/macos/package.sh --native           build for this Mac's CPU only (quicker, for local use)
#   packaging/macos/package.sh --app-only         stop after the .app; don't make a disk image
#   packaging/macos/package.sh --bundle-id ID     bundle identifier (default io.github.rallegretti.landingcraft)
#
# --sign and --notarize can also come from MACOS_SIGN_IDENTITY and MACOS_NOTARY_PROFILE.
# Store a notary profile once with: xcrun notarytool store-credentials PROFILE
# Output goes to <target dir>/macos/.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/scripts/common.sh"

IDENTITY="${MACOS_SIGN_IDENTITY:--}"
PROFILE="${MACOS_NOTARY_PROFILE:-}"
BUNDLE_ID="${LANDINGCRAFT_BUNDLE_ID:-io.github.rallegretti.landingcraft}"
NATIVE=0
APP_ONLY=0
QUIET=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --sign)       [[ $# -ge 2 ]] || die "--sign needs an identity"; IDENTITY="$2"; shift ;;
        --sign=*)     IDENTITY="${1#--sign=}" ;;
        --notarize)   [[ $# -ge 2 ]] || die "--notarize needs a notarytool keychain profile"; PROFILE="$2"; shift ;;
        --notarize=*) PROFILE="${1#--notarize=}" ;;
        --bundle-id)  [[ $# -ge 2 ]] || die "--bundle-id needs an identifier"; BUNDLE_ID="$2"; shift ;;
        --bundle-id=*) BUNDLE_ID="${1#--bundle-id=}" ;;
        --native)     NATIVE=1 ;;
        --app-only)   APP_ONLY=1 ;;
        -q|--quiet)   QUIET=1 ;;
        -h|--help)    sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *)            die "unknown option: $1 (see --help)" ;;
    esac
    shift
done

[[ "$OS" == Darwin ]] || die "this script builds the macOS app, so it has to run on a Mac."
[[ -z "$PROFILE" || "$IDENTITY" != - ]] || die "notarizing needs a Developer ID signature; pass --sign too."
[[ -z "$PROFILE" || "$APP_ONLY" == 0 ]] || die "--notarize submits the disk image, so it can't be used with --app-only."
for tool in codesign lipo hdiutil ditto plutil; do
    command -v "$tool" >/dev/null 2>&1 || die "$tool not found; install the Xcode command line tools (xcode-select --install)."
done

find_cargo
choose_target_dir
cd "$PROJECT_DIR"

# The toolchain's host triple, e.g. aarch64-apple-darwin.
rustc_host() {
    local rustc="$(dirname "$CARGO")/rustc"
    [[ -x "$rustc" ]] || rustc=rustc
    "$rustc" -vV | sed -n 's/^host: //p'
}

# rustup, if Rust was installed with it (it may not be on PATH, like cargo).
find_rustup() {
    command -v rustup 2>/dev/null && return
    local r="${CARGO_HOME:-$HOME/.cargo}/bin/rustup"
    [[ -x "$r" ]] && echo "$r"
}

VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)"
[[ -n "$VERSION" ]] || die "couldn't read the version from Cargo.toml"
# Matches the Info.plist; Rust's own minimum is 10.12 (Intel) and 11.0 (Apple silicon).
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"

# ------------------------------------------------------------------ binary

if [[ "$NATIVE" == 1 ]]; then
    TARGETS=("$(rustc_host)")
    ARCH="$(uname -m | sed 's/^arm64$/aarch64/')"
else
    TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
    ARCH=universal
    if RUSTUP="$(find_rustup)"; then
        installed="$("$RUSTUP" target list --installed)"
        for t in "${TARGETS[@]}"; do
            grep -qx "$t" <<<"$installed" || die "the $t target isn't installed; run: rustup target add $t (or use --native)"
        done
    fi
fi

bins=()
for t in "${TARGETS[@]}"; do
    info "Building release for $t"
    args=(build --locked --release --target "$t")
    [[ "$QUIET" == 1 ]] && args+=(--quiet)
    # Portable code: no target-cpu=native in a build meant for other Macs.
    RUSTFLAGS="" "$CARGO" "${args[@]}"
    bins+=("$TARGET_DIR/$t/release/landingcraft")
done

# ------------------------------------------------------------------ bundle

OUT="$TARGET_DIR/macos"
APP="$OUT/LandingCraft.app"
info "Assembling $APP"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/landingcraft" "${bins[@]}"
sed -e "s/@VERSION@/$VERSION/g" -e "s/@BUNDLE_ID@/$BUNDLE_ID/g" \
    "$PROJECT_DIR/packaging/macos/Info.plist" > "$APP/Contents/Info.plist"
plutil -lint -s "$APP/Contents/Info.plist"
cp "$PROJECT_DIR/packaging/macos/AppIcon.icns" "$APP/Contents/Resources/AppIcon.icns"

# Signs a bundle or disk image. Developer ID signatures get the hardened runtime
# and a secure timestamp, both of which notarization requires.
sign() {
    if [[ "$IDENTITY" == - ]]; then
        codesign --force --sign - "$1"
    else
        codesign --force --options runtime --timestamp --sign "$IDENTITY" "$1"
    fi
}

if [[ "$IDENTITY" == - ]]; then
    info "Signing ad hoc (runs on this Mac; other Macs will refuse it until it's signed with a Developer ID)"
else
    info "Signing with $IDENTITY"
fi
sign "$APP"
codesign --verify --strict --verbose=1 "$APP" 2>&1 | sed 's/^/    /' >&2
info "Built $APP ($(lipo -archs "$APP/Contents/MacOS/landingcraft"))"
[[ "$APP_ONLY" == 1 ]] && exit 0

# ------------------------------------------------------------------ disk image

DMG="$OUT/landingcraft-$VERSION-macos-$ARCH.dmg"
info "Creating $DMG"
stage="$(mktemp -d "$OUT/.dmg-XXXXXX")"
trap 'rm -rf "$stage"' EXIT
ditto "$APP" "$stage/LandingCraft.app"
ln -s /Applications "$stage/Applications"
rm -f "$DMG"
hdiutil create -quiet -volname "LandingCraft" -srcfolder "$stage" -fs HFS+ -format UDZO "$DMG"
[[ "$IDENTITY" == - ]] || sign "$DMG"

if [[ -n "$PROFILE" ]]; then
    info "Submitting to Apple's notary service (profile $PROFILE); this usually takes a few minutes"
    result="$(xcrun notarytool submit "$DMG" --keychain-profile "$PROFILE" --wait --output-format json)" \
        || die "notarytool failed: $result"
    if ! grep -q '"status" *: *"Accepted"' <<<"$result"; then
        id="$(sed -n 's/.*"id" *: *"\([^"]*\)".*/\1/p' <<<"$result")"
        die "notarization wasn't accepted: $result
       See why with: xcrun notarytool log $id --keychain-profile $PROFILE"
    fi
    xcrun stapler staple -q "$DMG"
    spctl --assess --type open --context context:primary-signature --verbose=1 "$DMG" 2>&1 | sed 's/^/    /' >&2
    info "Notarized and stapled"
fi

info "Built $DMG"
