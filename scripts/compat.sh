#!/bin/sh
# Make the plugin layer of each version that the Rust compiler serves.
#
# Usage: sh scripts/compat.sh UPSTREAM_CLONE
#
# The plugin layer is JavaScript: `packages/tailwindcss/src/compat` of
# Tailwind CSS with the shims of native/tailwind/js, in one file
# (tests/upstream/tools/build-compat-bundle.mjs). This script makes that
# file from the source of each version from Version::OLDEST_WITH_PLUGINS
# (native/tailwind/src/version.rs) to the version of the file UPSTREAM:
#
#   native/tailwind/assets/compat.js              the version of UPSTREAM
#   native/tailwind/assets/script.js              the runtime of the script engine
#   native/tailwind/assets/compat/VERSION.patch   each older version: the
#                                                 difference from the next
#                                                 newer version (tools/twpatch)
#
# UPSTREAM_CLONE is a clone of https://github.com/tailwindlabs/tailwindcss.
# The script reads the tag of each version from it, and gets a tag that is
# not there. It changes no file of the clone.
#
# Run it after a change of native/tailwind/js, after a new version in the
# file UPSTREAM, and after a change of Version::OLDEST_WITH_PLUGINS. It
# needs git, Node.js, cargo, and esbuild (in tests/upstream/tools: npm ci
# --legacy-peer-deps --ignore-scripts). The result does not depend on
# the computer: the
# job "test" of .github/workflows/build.yml makes the files again and
# compares them.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
log() { echo "compat.sh: $*" >&2; }

clone=${1:?"usage: sh scripts/compat.sh UPSTREAM_CLONE"}
[ -d "$clone/.git" ] || [ -f "$clone/.git" ] || { log "$clone is not a git clone"; exit 2; }
# shellcheck disable=SC1091
. ./UPSTREAM
newest=$TAILWINDCSS_VERSION
oldest=$(sed -n 's/.*pub const OLDEST_WITH_PLUGINS: Version = Version(\([0-9]*\), \([0-9]*\), \([0-9]*\));.*/\1.\2.\3/p' native/tailwind/src/version.rs)
[ -n "$oldest" ] || { log "Version::OLDEST_WITH_PLUGINS is not in native/tailwind/src/version.rs"; exit 1; }

# The versions of the file PACKAGES from $oldest to $newest, old to new.
key() { echo "$1" | awk -F. '{ printf "%d%04d%04d", $1, $2, $3 }'; }
versions=
for version in $(grep -v '^#' PACKAGES | cut -d' ' -f1 | sort -t. -k1,1n -k2,2n -k3,3n); do
    if [ "$(key "$version")" -ge "$(key "$oldest")" ] && [ "$(key "$version")" -le "$(key "$newest")" ]; then
        versions="$versions $version"
    fi
done
case "$versions" in
    *" $newest") ;;
    *) log "the version $newest of UPSTREAM is not in PACKAGES"; exit 1 ;;
esac

work=$root/build/compat
rm -rf "$work"
mkdir -p "$work/source" "$work/bundles"
for version in $versions; do
    tag=v$version
    git -C "$clone" rev-parse -q --verify "refs/tags/$tag" > /dev/null ||
        git -C "$clone" fetch -q --depth 1 origin tag "$tag"
    mkdir -p "$work/source/$version" "$work/bundles/$version"
    git -C "$clone" archive "$tag" packages/tailwindcss/src packages/tailwindcss/package.json |
        tar -x -C "$work/source/$version"
    node tests/upstream/tools/build-compat-bundle.mjs "$work/source/$version" "$work/bundles/$version/compat.js" 2> "$work/log" ||
        { cat "$work/log" >&2; log "the plugin layer of $version did not build"; exit 1; }
    # The runtime of the script engine has no code of Tailwind CSS: it is
    # the same file for each version.
    mv "$work/bundles/$version/script.js" "$work/script.js"
    log "$version: $(wc -c < "$work/bundles/$version/compat.js" | tr -d ' ') bytes"
done

cargo build --quiet --release --manifest-path tools/twpatch/Cargo.toml
# shellcheck disable=SC2086
tools/twpatch/target/release/twpatch pack "$work/bundles" "$work/store" $versions >&2

rm -rf native/tailwind/assets/compat
mkdir -p native/tailwind/assets/compat
cp "$work/store/$newest/compat.js" native/tailwind/assets/compat.js
cp "$work/script.js" native/tailwind/assets/script.js
for version in $versions; do
    [ "$version" = "$newest" ] || cp "$work/store/$version.patch" native/tailwind/assets/compat/
done
log "wrote native/tailwind/assets/compat.js, script.js and compat/ ($(du -sk native/tailwind/assets/compat | cut -f1) KB of patches)"
