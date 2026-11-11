#!/bin/sh
# Run the official standalone program of Tailwind CSS on the cases that
# have its module rules, and compare its output with the record.
#
# Usage: sh tests/standalone.sh PROGRAM [CASE...]
#        sh tests/standalone.sh --download DIRECTORY
#
# PROGRAM is the standalone program of the release of Tailwind CSS that
# UPSTREAM names (for example tailwindcss-linux-x64, from
# https://github.com/tailwindlabs/tailwindcss/releases).
#
# The records of tests/upstream/cli-js come from the npm program on
# Node.js. The standalone program runs on Bun, and has two rules of its
# own (tests/upstream/tools/bun-interop.mjs): the default export of a
# CommonJS bundle, and the three plugins in its file. For a case with a
# file standalone.txt, the record comes from the npm program with a copy
# of these rules. This script shows that the copy is correct: the real
# standalone program must give the recorded stdout.css and exit status.
# A difference means that the record is wrong: record the case again
# after a fix of tests/upstream/tools/bun-interop.mjs.
#
# --download gets tailwindcss-linux-x64 of the release, and checks its
# SHA-256 against STANDALONE_LINUX_X64_SHA256 of UPSTREAM. It prints the
# path of the file. (Linux x86_64 with glibc only.)
#
# CASES is the directory of the cases (default: tests/upstream/cli-js).
# With ALL=1, the script runs each case of the directory, also the cases
# with no standalone.txt: the npm program and the standalone program can
# differ there, so this is for a look by hand.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)

if [ "${1:-}" = --download ]; then
    dir=${2:?"--download needs a directory"}
    # shellcheck disable=SC1091
    . "$root/UPSTREAM"
    mkdir -p "$dir"
    file=$dir/tailwindcss-linux-x64
    curl -fsSL -o "$file" \
        "https://github.com/tailwindlabs/tailwindcss/releases/download/v$TAILWINDCSS_VERSION/tailwindcss-linux-x64"
    sum=$(sha256sum "$file" | cut -d' ' -f1)
    if [ "$sum" != "$STANDALONE_LINUX_X64_SHA256" ]; then
        echo "standalone.sh: tailwindcss-linux-x64 of v$TAILWINDCSS_VERSION has the SHA-256 $sum, not $STANDALONE_LINUX_X64_SHA256" >&2
        rm -f "$file"
        exit 1
    fi
    chmod +x "$file"
    echo "$file"
    exit 0
fi

program=${1:?"Usage: sh tests/standalone.sh PROGRAM [CASE...]"}
shift
case "$program" in /*) ;; *) program=$(pwd)/$program ;; esac
cases=${CASES:-$root/tests/upstream/cli-js}
case "$cases" in /*) ;; *) cases=$(pwd)/$cases ;; esac
work=$(mktemp -d "${TMPDIR:-/tmp}/tailwind-standalone.XXXXXX")
work=$(cd "$work" && pwd -P)
[ $# -gt 0 ] || set -- $(ls "$cases")

failed=0
count=0
for name in "$@"; do
    [ -f "$cases/$name/args.txt" ] || continue
    [ "${ALL:-}" = 1 ] || [ -f "$cases/$name/standalone.txt" ] || continue
    count=$((count + 1))
    mkdir -p "$work/$name"
    source=$cases/$name/project
    [ -f "$cases/$name/project.txt" ] && source=$cases/$(tr -d "\r\n" < "$cases/$name/project.txt")/project
    cp -R "$source" "$work/$name/project"
    project=$(cd "$work/$name/project" && pwd -P)
    set --
    while IFS= read -r arg; do
        arg=$(printf "%s" "$arg" | tr -d "\r")
        [ -n "$arg" ] && set -- "$@" "$arg"
    done < "$cases/$name/args.txt"
    status=0
    (
        cd "$project"
        if [ -f "$cases/$name/env.txt" ]; then
            while IFS= read -r line || [ -n "$line" ]; do
                line=$(printf "%s" "$line" | tr -d "\r" | sed "s|<project>|$project|g")
                case "$line" in *=*) export "${line%%=*}=${line#*=}" ;; esac
            done < "$cases/$name/env.txt"
        fi
        NO_COLOR=1 "$program" "$@" > "$work/$name/stdout.css" 2> "$work/$name/stderr.txt" < /dev/null
    ) || status=$?
    expected=$(cat "$cases/$name/exit.txt")
    ok=1
    if [ "$status" != "$expected" ]; then
        echo "standalone.sh: $name: exit status $status, the record has $expected" >&2
        ok=0
    fi
    if ! cmp -s "$work/$name/stdout.css" "$cases/$name/stdout.css"; then
        echo "standalone.sh: $name: the output of the standalone program is not the record:" >&2
        diff "$cases/$name/stdout.css" "$work/$name/stdout.css" | head -40 >&2 || true
        ok=0
    fi
    if [ $ok = 1 ]; then
        echo "standalone.sh: $name: the record is the output of the standalone program"
    else
        failed=$((failed + 1))
        sed 's/^/    /' "$work/$name/stderr.txt" | head -10 >&2
    fi
done
echo "standalone.sh: $count cases, $failed with a difference"
if [ "$count" = 0 ]; then
    echo "standalone.sh: no case has a file standalone.txt" >&2
    exit 1
fi
[ "$failed" = 0 ]
