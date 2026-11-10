#!/bin/sh
# The generated part of the npm package tailwind.com (package.json at the
# root of the repository), in runtime/: the WebAssembly module, and the
# release of tailwind.com for "npx tailwind.com" (js/download.mjs). Then
# "npm pack" or "npm publish" in the root makes the package.
#
#   sh scripts/npm.sh TAILWIND_COM TAILWIND_WASM TAG
#
# TAILWIND_COM and TAILWIND_WASM: the files of the release. TAG: the tag of
# the GitHub release that has the file, for example v4.3.5.
set -eu
[ $# -eq 3 ] || { echo "usage: $0 TAILWIND_COM TAILWIND_WASM TAG" >&2; exit 2; }
com=$1 wasm=$2 tag=$3
root=$(cd "$(dirname "$0")/.." && pwd)
# shellcheck disable=SC1091
. "$root/UPSTREAM"

version=$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' "$root/package.json")
[ -n "$version" ] || { echo "$0: no version in package.json" >&2; exit 1; }
if command -v sha256sum > /dev/null; then
    sha=$(sha256sum "$com" | cut -d' ' -f1)
else
    sha=$(shasum -a 256 "$com" | cut -d' ' -f1)
fi

out=$root/runtime
mkdir -p "$out"
cp "$wasm" "$out/tailwind.wasm"
printf '{"version": "%s", "tag": "%s", "tailwindcss": "%s", "sha256": "%s"}\n' \
    "$version" "$tag" "$TAILWINDCSS_VERSION" "$sha" > "$out/release.json"
echo "$0: wrote $out (package $version, release $tag, Tailwind CSS $TAILWINDCSS_VERSION)"
