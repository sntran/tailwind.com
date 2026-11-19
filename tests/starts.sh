#!/bin/sh
# Many starts of the program, one after the other: a test of the APE
# loader and of the start of the program on each system.
#
# Usage: sh tests/starts.sh PROGRAM [COUNT]
#
# Each start builds the Phoenix project with daisyUI and heroicons
# (tests/upstream/cli-js/phoenix-deps): the shell header, the APE loader,
# the scanner and its threads, and the plugins in QuickJS. COUNT is the
# number of starts (default 2000).
#
# On macOS arm64, the APE loader of cosmocc maps the program at fixed
# addresses with MAP_FIXED. BEAM.com saw two faults there (C51 and C57 of
# its docs/UPSTREAM.md): memory of the malloc of libSystem under the
# program (a rare hang or a crash), and the address space of macOS 27.
# This test counts the starts that fail:
#   - a status that is not 0, or a signal;
#   - a start that takes more than LIMIT seconds (default 60): the test
#     stops it;
#   - CSS that is not the CSS of the first start;
#   - text on the standard error other than the messages of a build.
# The last line is "starts: COUNT starts, 0 failed" when each start
# passes. The exit status is 1 when a start failed.
#
# TAILWIND_RUN, as for tests/cli.sh: a program that starts PROGRAM.
set -eu

[ $# -ge 1 ] || { echo "usage: sh tests/starts.sh PROGRAM [COUNT]" >&2; exit 2; }
program=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
count=${2:-2000}
limit=${LIMIT:-60}
root=$(cd "$(dirname "$0")/.." && pwd)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cp -R "$root/tests/upstream/cli-js/phoenix-deps/project" "$work/project"
cd "$work/project"
export NODE_PATH="$work/project/deps:$work/project/_build/dev"
# The messages have no colors, so the test can compare them.
export NO_COLOR=1
unset CI FORCE_COLOR

run() {
    if [ -n "${TAILWIND_RUN:-}" ]; then
        $TAILWIND_RUN "$program" "$@"
        return
    fi
    case "$(uname -s):$program" in
        MINGW*|MSYS*|CYGWIN*) "$program" "$@" ;;
        *.com) sh "$program" "$@" ;;
        *) "$program" "$@" ;;
    esac
}

# The time in milliseconds. macOS has no %N in date.
now_ms() {
    perl -MTime::HiRes=time -e 'printf("%d\n", time() * 1000)'
}

# One start, with a watchdog. The status is the status of the program, or
# 124 when the watchdog stopped it.
start() {
    rm -f "$work/hung"
    run --input=assets/css/app.css -o "$work/out.css" > "$work/stdout.txt" 2> "$work/stderr.txt" &
    pid=$!
    (
        i=0
        while [ $i -lt $((limit * 10)) ]; do
            sleep 0.1
            kill -0 "$pid" 2> /dev/null || exit 0
            i=$((i + 1))
        done
        : > "$work/hung"
        kill -9 "$pid" 2> /dev/null || true
    ) &
    dog=$!
    status=0
    wait "$pid" || status=$?
    kill "$dog" 2> /dev/null || true
    wait "$dog" 2> /dev/null || true
    [ -f "$work/hung" ] && status=124
    return $status
}

# The messages of a build: the header, an empty line and the time. (The
# line of daisyUI goes to the standard output.)
unexpected_stderr() {
    tr -d '\r' < "$work/stderr.txt" | grep -v -e '^≈ tailwindcss v' -e '^Done in ' -e '^$' || true
}

failed=0
slowest=0
total=0
first=
n=0
while [ $n -lt "$count" ]; do
    n=$((n + 1))
    t0=$(now_ms)
    status=0
    start || status=$?
    t1=$(now_ms)
    ms=$((t1 - t0))
    total=$((total + ms))
    [ $ms -gt $slowest ] && slowest=$ms
    problem=
    if [ $status = 124 ]; then
        problem="stopped after $limit s (a hang)"
    elif [ $status != 0 ]; then
        problem="status $status"
    elif [ -z "$first" ]; then
        first=$work/first.css
        cp "$work/out.css" "$first"
        grep -q -- '--color-base-100' "$first" || problem="the CSS has no theme of daisyUI"
    elif ! cmp -s "$work/out.css" "$first"; then
        problem="the CSS is not the CSS of the first start"
    fi
    if [ -z "$problem" ] && [ -n "$(unexpected_stderr)" ]; then
        problem="unexpected error output"
    fi
    if [ -n "$problem" ]; then
        failed=$((failed + 1))
        echo "starts: start $n: $problem" >&2
        sed 's/^/    /' "$work/stderr.txt" | head -20 >&2
    fi
    if [ $((n % 250)) = 0 ]; then
        echo "starts: $n starts, $failed failed, $((total / n)) ms on average"
    fi
done
echo "starts: the average start took $((total / count)) ms, the slowest $slowest ms"
echo "starts: $count starts, $failed failed"
[ "$failed" = 0 ]
