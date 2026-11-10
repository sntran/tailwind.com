#!/bin/sh
# Run the program on the recorded cases of the official program of
# Tailwind CSS (tests/upstream/cli), and compare the output.
#
# Usage: sh tests/cli.sh PROGRAM [CASE...]
#
# Each case has project/ (the files), args.txt (one argument for each
# line), stdout.css and exit.txt (the record of the official program),
# and for an error stderr.txt. The program runs on a copy of project/
# outside this repository, as the record was made: the .gitignore of this
# repository must not change the scan.
set -eu
program=$1
shift
case "$program" in /*) ;; *) program=$(pwd)/$program ;; esac
root=$(cd "$(dirname "$0")/.." && pwd)
cases=$root/tests/upstream/cli
work=$(mktemp -d "${TMPDIR:-/tmp}/tailwind-cli.XXXXXX")
[ $# -gt 0 ] || set -- $(ls "$cases")

# An APE file needs the shell to start it on some systems.
run() {
    case "$program" in
        *.com) sh "$program" "$@" ;;
        *) "$program" "$@" ;;
    esac
}

failed=0
count=0
for name in "$@"; do
    [ -f "$cases/$name/args.txt" ] || continue
    count=$((count + 1))
    mkdir -p "$work/$name"
    cp -R "$cases/$name/project" "$work/$name/project"
    # The arguments: one for each line of args.txt.
    set --
    while IFS= read -r arg; do
        [ -n "$arg" ] && set -- "$@" "$arg"
    done < "$cases/$name/args.txt"
    status=0
    (cd "$work/$name/project" && NO_COLOR=1 run "$@" > "$work/$name/stdout.css" 2> "$work/$name/stderr.txt") || status=$?
    expected=$(cat "$cases/$name/exit.txt")
    ok=1
    if [ "$status" != "$expected" ]; then
        echo "cli.sh: $name: exit status $status, expected $expected" >&2
        ok=0
    fi
    if ! cmp -s "$work/$name/stdout.css" "$cases/$name/stdout.css"; then
        echo "cli.sh: $name: the output is different" >&2
        diff "$cases/$name/stdout.css" "$work/$name/stdout.css" | head -20 >&2 || true
        ok=0
    fi
    # For an error: each message line of the official program (the lines
    # of the box) must be in the output, without the colors.
    if [ -f "$cases/$name/stderr.txt" ]; then
        esc=$(printf '\033')
        sed "s/$esc\[[0-9;]*m//g" "$cases/$name/stderr.txt" | grep '^│' > "$work/$name/expected.txt" || true
        while IFS= read -r line; do
            if ! grep -qF -- "$line" "$work/$name/stderr.txt"; then
                echo "cli.sh: $name: the error output does not have: $line" >&2
                ok=0
            fi
        done < "$work/$name/expected.txt"
    fi
    if [ $ok = 1 ]; then
        echo "cli.sh: $name: pass"
    else
        failed=$((failed + 1))
        sed 's/^/    /' "$work/$name/stderr.txt" | head -10 >&2
    fi
done
echo "cli.sh: $count cases, $failed failed"
[ "$failed" = 0 ]
