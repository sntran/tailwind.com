#!/bin/sh
# Get the npm packages `tailwindcss` of the script engine.
#
# Usage: sh scripts/packages.sh DIR [VERSION...]
#        sh scripts/packages.sh --record VERSION...
#
# The first form puts the files of each version in DIR/VERSION. With no
# version, it gets each version of the file PACKAGES. The directory DIR is
# a value for the variable TAILWIND_COM_PACKAGES, and scripts/build.sh puts
# these files into the ZIP directory `tailwindcss/` of tailwind.com.
#
# Each package comes from the npm registry, and its SHA-256 must be the
# value of PACKAGES. A version that is not in PACKAGES is an error.
#
# The second form downloads each version and writes its line into
# PACKAGES. Use it for a new version of Tailwind CSS.
#
# Only the files that the script engine needs stay: package.json, LICENSE,
# the CommonJS files of dist/ (the compiler lib.js, and the modules for
# plugins), and theme.css, preflight.css and utilities.css.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
list=$root/PACKAGES
registry=${NPM_REGISTRY:-https://registry.npmjs.org}

log() { echo "packages.sh: $*" >&2; }

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

# download VERSION FILE
download() {
    curl -fsSL --retry 3 -o "$2" "$registry/tailwindcss/-/tailwindcss-$1.tgz"
}

recorded() {
    grep -E "^$(echo "$1" | sed 's/\./\\./g') " "$list" 2>/dev/null | cut -d' ' -f2
}

if [ "${1:-}" = "--record" ]; then
    shift
    [ $# -gt 0 ] || { log "Usage: sh scripts/packages.sh --record VERSION..."; exit 2; }
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    for version in "$@"; do
        download "$version" "$tmp/$version.tgz"
        sum=$(sha256 "$tmp/$version.tgz")
        grep -v -E "^$(echo "$version" | sed 's/\./\\./g') " "$list" > "$tmp/list" || true
        echo "$version $sum" >> "$tmp/list"
        # The comment lines first, then the versions in sequence.
        { grep '^#' "$tmp/list" || true; grep -v '^#' "$tmp/list" | sort -t. -k1,1n -k2,2n -k3,3n; } > "$list"
        log "recorded $version"
    done
    exit 0
fi

dir=${1:?Usage: sh scripts/packages.sh DIR [VERSION...]}
shift
mkdir -p "$dir/.cache"
dir=$(cd "$dir" && pwd)
if [ $# -eq 0 ]; then
    # shellcheck disable=SC2046
    set -- $(grep -v '^#' "$list" | cut -d' ' -f1)
fi

for version in "$@"; do
    expected=$(recorded "$version")
    [ -n "$expected" ] || { log "Tailwind CSS $version is not in PACKAGES. Add it: sh scripts/packages.sh --record $version"; exit 1; }
    if [ -f "$dir/$version/.sha256" ] && [ "$(cat "$dir/$version/.sha256")" = "$expected" ]; then
        continue
    fi
    file=$dir/.cache/tailwindcss-$version.tgz
    [ -f "$file" ] && [ "$(sha256 "$file")" = "$expected" ] || download "$version" "$file"
    found=$(sha256 "$file")
    if [ "$found" != "$expected" ]; then
        log "tailwindcss-$version.tgz has the SHA-256 $found, and PACKAGES has $expected"
        exit 1
    fi
    rm -rf "$dir/$version" "$dir/.cache/unpack"
    mkdir -p "$dir/.cache/unpack" "$dir/$version/dist"
    tar -xzf "$file" -C "$dir/.cache/unpack"
    from=$dir/.cache/unpack/package
    cp "$from/package.json" "$from/LICENSE" "$from/theme.css" "$from/preflight.css" "$from/utilities.css" "$dir/$version/"
    cp "$from"/dist/*.js "$dir/$version/dist/"
    rm -rf "$dir/.cache/unpack"
    echo "$expected" > "$dir/$version/.sha256"
    log "Tailwind CSS $version"
done
