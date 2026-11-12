#!/bin/sh
# Get tailwind.wasm of each release of tailwind.com, for the site
# (docs/SITE.md).
#
# Usage: sh scripts/releases.sh DIR [COUNT]
#
# For each release, the newest first, the script writes
# DIR/TAG/tailwind.wasm and checks its SHA-256 against the file SHA256SUMS
# of the release. COUNT (default 20) is the most releases that it gets:
# each program is about 9 MB on the site.
#
# A release of tailwind.com has the npm package file
# (tailwind.com-VERSION.tgz). The releases with the name of a version of
# Tailwind CSS (for example v4.3.5) have the same files as a release of
# tailwind.com: the script does not get them.
#
# It needs the GitHub CLI, with GH_TOKEN in the environment.
set -eu
dir=${1:?usage: sh scripts/releases.sh DIR [COUNT]}
count=${2:-20}
repo=${GITHUB_REPOSITORY:-sntran/tailwind.com}
log() { echo "releases.sh: $*" >&2; }

mkdir -p "$dir"
got=0
for tag in $(gh release list --repo "$repo" --exclude-drafts --exclude-pre-releases --limit 100 --json tagName --jq '.[].tagName'); do
    [ "$got" -lt "$count" ] || break
    assets=$(gh release view "$tag" --repo "$repo" --json assets --jq '.assets[].name')
    echo "$assets" | grep -q '^tailwind\.com-.*\.tgz$' || continue
    echo "$assets" | grep -q '^tailwind\.wasm$' || continue
    rm -rf "${dir:?}/$tag"
    mkdir -p "$dir/$tag"
    gh release download "$tag" --repo "$repo" --pattern tailwind.wasm --pattern SHA256SUMS --dir "$dir/$tag"
    (cd "$dir/$tag" && grep ' tailwind\.wasm$' SHA256SUMS | sha256sum -c - > /dev/null) || { log "$tag: the SHA-256 of tailwind.wasm is not the value of SHA256SUMS"; exit 1; }
    rm "$dir/$tag/SHA256SUMS"
    log "$tag: tailwind.wasm"
    got=$((got + 1))
done
log "$got releases in $dir"
