#!/usr/bin/env bash
# Upload built files to LandingCraft's GitHub release for the version in Cargo.toml (tag v<version>).
#
#   ./release.sh FILE...             upload to the draft release, creating the draft if needed
#   ./release.sh --publish           publish the draft once every platform has uploaded
#   ./release.sh --publish FILE...   upload, then publish
#
# Each platform builds and uploads its own files from its own machine, at the same commit. The release
# stays a draft, visible only to you, until --publish creates the tag and makes it public.
# Needs the GitHub CLI, signed in once with: gh auth login
set -euo pipefail
source "$(dirname "$(readlink -f "$0")")/scripts/common.sh"

PUBLISH=0
FILES=()
for arg in "$@"; do
    case "$arg" in
        --publish) PUBLISH=1 ;;
        -h|--help) sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        -*)        die "unknown option: $arg (see --help)" ;;
        *)         FILES+=("$arg") ;;
    esac
done
[[ ${#FILES[@]} -gt 0 || "$PUBLISH" == 1 ]] || die "nothing to do; give files to upload and/or --publish (see --help)"

if command -v gh >/dev/null 2>&1; then
    GH="$(command -v gh)"
elif [[ -x "$HOME/.local/bin/gh" ]]; then
    GH="$HOME/.local/bin/gh"
else
    die "the GitHub CLI (gh) isn't installed; see https://cli.github.com"
fi
"$GH" auth status >/dev/null 2>&1 || die "gh isn't signed in to GitHub; run: gh auth login"

cd "$PROJECT_DIR"
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)"
[[ -n "$VERSION" ]] || die "couldn't read the version from Cargo.toml"
TAG="v$VERSION"

# A release must be built from code anyone can check out: committed, and pushed.
[[ -z "$(git status --porcelain)" ]] || die "there are uncommitted changes; commit them (and rebuild) first."
SHA="$(git rev-parse HEAD)"
git fetch --quiet --tags origin
[[ -n "$(git branch -r --contains "$SHA")" ]] || die "this commit isn't on GitHub yet; push it first."

for f in ${FILES[@]+"${FILES[@]}"}; do
    [[ -f "$f" ]] || die "$f not found"
    [[ "$(basename "$f")" == *"$VERSION"* ]] || die "$(basename "$f") isn't named for version $VERSION; is it an old build?"
    if [[ "$OS" == Darwin && "$f" == *.dmg ]]; then
        xcrun stapler validate -q "$f" 2>/dev/null \
            || die "$(basename "$f") isn't notarized; build it with packaging/macos/package.sh --sign … --notarize …"
    fi
done

if state="$("$GH" release view "$TAG" --json isDraft,targetCommitish --jq '"\(.isDraft) \(.targetCommitish)"' 2>/dev/null)"; then
    read -r draft target <<<"$state"
    if [[ "$draft" != true && ${#FILES[@]} -gt 0 ]]; then
        die "$TAG is already published. Bump the version in Cargo.toml for a new release."
    fi
    # Drafts record the commit they'll tag; every platform's files must come from it.
    if [[ "$draft" == true && "$target" != "$SHA" ]]; then
        die "the $TAG draft is for commit ${target:0:12}, but this checkout is at ${SHA:0:12}. Check out that commit and rebuild."
    fi
elif git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
    die "tag $TAG already exists without a release. Bump the version in Cargo.toml."
else
    info "Creating draft release $TAG for commit ${SHA:0:12}"
    "$GH" release create "$TAG" --draft --target "$SHA" --title "LandingCraft $VERSION" --generate-notes
fi

if [[ ${#FILES[@]} -gt 0 ]]; then
    info "Uploading ${#FILES[@]} file(s) to $TAG"
    "$GH" release upload "$TAG" "${FILES[@]}" --clobber
fi

if [[ "$PUBLISH" == 1 ]]; then
    info "Publishing $TAG"
    "$GH" release edit "$TAG" --draft=false --latest >/dev/null
    git fetch --quiet --tags origin
fi

info "$("$GH" release view "$TAG" --json url,isDraft,assets \
    --jq '"\(if .isDraft then "Draft" else "Published" end): \(.url)\n    " + ([.assets[].name] | join("\n    "))')"
