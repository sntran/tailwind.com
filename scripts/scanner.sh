#!/bin/sh
# Copy the scanner of Tailwind CSS into native/scanner.
#
# Usage: sh scripts/scanner.sh UPSTREAM_CLONE
#
# UPSTREAM_CLONE is a clone of https://github.com/tailwindlabs/tailwindcss.
# The script copies, from the tag of the version of the file UPSTREAM, the
# three crates of the scanner:
#
#   crates/oxide                 -> native/scanner/oxide
#   crates/classification-macros -> native/scanner/classification-macros
#   crates/ignore                -> native/scanner/ignore
#
# Each file is a copy with no change, except Cargo.toml: the manifests
# are ours, with the names that crates.io accepts (native/scanner/README.md).
# The manifest of Tailwind CSS goes to native/scanner/manifests/CRATE.toml,
# so that a change of its dependencies shows in `git diff`: then change
# our manifest by hand. The license of Tailwind CSS goes into the two
# crates that have no license file.
#
# The job "test" of .github/workflows/build.yml runs this script and
# compares the result with the repository. scripts/sync-upstream.sh runs
# it for a new version of Tailwind CSS.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
log() { echo "scanner.sh: $*" >&2; }
clone=${1:?"usage: sh scripts/scanner.sh UPSTREAM_CLONE"}
[ -d "$clone/.git" ] || [ -f "$clone/.git" ] || { log "$clone is not a git clone"; exit 2; }
# shellcheck disable=SC1091
. ./UPSTREAM
tag=v$TAILWINDCSS_VERSION
git -C "$clone" rev-parse -q --verify "refs/tags/$tag^{commit}" > /dev/null ||
    git -C "$clone" -c gc.auto=0 -c maintenance.auto=false fetch -q --depth 1 origin tag "$tag"

work=$(mktemp -d "${TMPDIR:-/tmp}/scanner.XXXXXX")
trap 'rm -rf "$work"' EXIT
git -C "$clone" archive "$tag" crates/oxide crates/classification-macros crates/ignore LICENSE | tar -x -C "$work"

mkdir -p native/scanner/manifests
for crate in oxide classification-macros ignore; do
    to=native/scanner/$crate
    # Keep our manifest; replace each other file.
    keep=$(mktemp "$work/manifest.XXXXXX")
    [ -f "$to/Cargo.toml" ] && cp "$to/Cargo.toml" "$keep"
    rm -rf "$to"
    mkdir -p "$to"
    cp -R "$work/crates/$crate/." "$to/"
    mv "$to/Cargo.toml" native/scanner/manifests/$crate.toml
    if [ -s "$keep" ]; then
        cp "$keep" "$to/Cargo.toml"
    else
        log "$to/Cargo.toml: no manifest of ours; write one (native/scanner/README.md)"
    fi
done
# The fuzz targets of the extractor are for cargo-fuzz, and are not part
# of the crate.
rm -rf native/scanner/oxide/fuzz
cp "$work/LICENSE" native/scanner/oxide/LICENSE
cp "$work/LICENSE" native/scanner/classification-macros/LICENSE
log "native/scanner has the scanner of Tailwind CSS $TAILWINDCSS_VERSION"
