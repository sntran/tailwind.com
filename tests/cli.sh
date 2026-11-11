#!/bin/sh
# Run the program on the recorded cases of the official program of
# Tailwind CSS, and compare the output.
#
# Usage: sh tests/cli.sh PROGRAM [CASE...]
#        CASES=tests/upstream/cli-js sh tests/cli.sh PROGRAM [CASE...]
#        CASES=tests/upstream/cli-map sh tests/cli.sh PROGRAM [CASE...]
#
# CASES is the directory of the cases (default: tests/upstream/cli).
# tests/upstream/cli-js has the cases with JavaScript plugins and configs.
# tests/upstream/cli-map has the cases for --map.
#
# Each case has project/ (the files), args.txt (one argument for each
# line), stdout.css and exit.txt (the record of the official program),
# and for an error stderr.txt. In stderr.txt, the text <project> is the
# absolute path of the copy of the project. A case with a file
# stderr-contains.txt has an error message that this program cannot
# repeat (a message of the JavaScript parser of Node.js): then only the
# lines of stderr-contains.txt must be in the error output. A case with
# the exit status 0 and a file stderr-contains.txt prints a warning: each
# line of the file must be in the error output of the program, without
# the color codes. A case can have env.txt: the environment variables for
# the program, one NAME=VALUE for each line, with <project> at the place
# of the path of the project. The program
# runs on a copy of project/ outside this repository, as the record was
# made: the .gitignore of this repository must not change the scan. A
# case with a file project.txt has no project/: the file names another
# case, and the case runs on a copy of the project of that case.
#
# A case can also have stdin.txt (the standard input of the program) and
# files/ (each file that the official program made in the project: the
# output file and the source map). A source map has the path of the
# project, so the records have /PROJECT at its place (PROJECT where
# Lightning CSS removes the first slash). The test puts the same text
# into the output of the program before the comparison, also in a source
# map that is in a comment as Base64 text. The cases of cli-map do not run
# on Windows: the paths have another form there.
#
# TAILWIND_RUN in the environment is a program that starts PROGRAM, with
# an absolute path: the APE loader, where the shell cannot start an APE
# file (NetBSD).
set -eu
program=$1
shift
case "$program" in /*) ;; *) program=$(pwd)/$program ;; esac
root=$(cd "$(dirname "$0")/.." && pwd)
cases=${CASES:-$root/tests/upstream/cli}
case "$cases" in /*) ;; *) cases=$(pwd)/$cases ;; esac
work=$(mktemp -d "${TMPDIR:-/tmp}/tailwind-cli.XXXXXX")
# The real path: the program prints paths with no symbolic links.
work=$(cd "$work" && pwd -P)
[ $# -gt 0 ] || set -- $(ls "$cases")

case "$(uname -s):$(basename "$cases")" in
    MINGW*:cli-map|MSYS*:cli-map|CYGWIN*:cli-map)
        echo "cli.sh: the cases of cli-map do not run on Windows"
        exit 0
        ;;
esac

# Base64: the command base64 of the system, or openssl where there is no
# such command (OpenBSD).
if [ "$(printf 'YQ==' | base64 --decode 2> /dev/null)" = a ]; then
    b64_decode() { base64 --decode; }
    b64_encode() { base64; }
elif [ "$(printf 'YQ==' | base64 -d 2> /dev/null)" = a ]; then
    b64_decode() { base64 -d; }
    b64_encode() { base64; }
else
    b64_decode() { openssl base64 -d -A; }
    b64_encode() { openssl base64 -A; }
fi

# Put /PROJECT at the place of the path of the project ($project) in a
# file: normalize FROM TO.
normalize() {
    sed -e "s|$project|/PROJECT|g" -e "s|${project#/}|PROJECT|g" "$1" > "$2"
    # A source map in a comment: decode it, change it, and encode it.
    if grep -q '^/\*# sourceMappingURL=data:application/json;base64,' "$2"; then
        line=$(grep '^/\*# sourceMappingURL=data:application/json;base64,' "$2")
        data=${line#*base64,}
        data=${data% \*/}
        printf "%s" "$data" | b64_decode > "$2.map"
        sed -e "s|$project|/PROJECT|g" -e "s|${project#/}|PROJECT|g" "$2.map" > "$2.map.new"
        data=$(b64_encode < "$2.map.new" | tr -d '\n\r')
        grep -v '^/\*# sourceMappingURL=data:application/json;base64,' "$2" > "$2.rest" || true
        # The comment is the last line with text of the output.
        {
            awk -v n="$(grep -n '^/\*# sourceMappingURL=data:' "$2" | head -1 | cut -d: -f1)" 'NR < n' "$2"
            printf "/*# sourceMappingURL=data:application/json;base64,%s */\n" "$data"
            awk -v n="$(grep -n '^/\*# sourceMappingURL=data:' "$2" | head -1 | cut -d: -f1)" 'NR > n' "$2"
        } > "$2.new"
        mv "$2.new" "$2"
        rm -f "$2.map" "$2.map.new" "$2.rest"
    fi
}

# An APE file needs the shell to start it, except on Windows, where it is
# a program.
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

failed=0
count=0
for name in "$@"; do
    [ -f "$cases/$name/args.txt" ] || continue
    # A list of paths in the environment (NODE_PATH) has another form on
    # Windows, and the shell changes it: these cases do not run there.
    if [ -f "$cases/$name/env.txt" ]; then
        case "$(uname -s)" in
            MINGW*|MSYS*|CYGWIN*) echo "cli.sh: $name: not on Windows (env.txt)"; continue ;;
        esac
    fi
    count=$((count + 1))
    mkdir -p "$work/$name"
    source=$cases/$name/project
    [ -f "$cases/$name/project.txt" ] && source=$cases/$(tr -d "\r\n" < "$cases/$name/project.txt")/project
    cp -R "$source" "$work/$name/project"
    # The arguments: one for each line of args.txt.
    set --
    while IFS= read -r arg; do
        arg=$(printf "%s" "$arg" | tr -d "\r")
        [ -n "$arg" ] && set -- "$@" "$arg"
    done < "$cases/$name/args.txt"
    status=0
    stdin=/dev/null
    [ -f "$cases/$name/stdin.txt" ] && stdin=$cases/$name/stdin.txt
    project=$(cd "$work/$name/project" && pwd -P)
    (
        cd "$work/$name/project"
        # The environment of the case.
        if [ -f "$cases/$name/env.txt" ]; then
            while IFS= read -r line || [ -n "$line" ]; do
                line=$(printf "%s" "$line" | tr -d "\r" | sed "s|<project>|$project|g")
                case "$line" in *=*) export "${line%%=*}=${line#*=}" ;; esac
            done < "$cases/$name/env.txt"
        fi
        export NO_COLOR=1
        run "$@" > "$work/$name/stdout.raw" 2> "$work/$name/stderr.txt" < "$stdin"
    ) || status=$?
    normalize "$work/$name/stdout.raw" "$work/$name/stdout.css"
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
    # Each file that the official program made must be the same.
    if [ -d "$cases/$name/files" ]; then
        for file in $(cd "$cases/$name/files" && find . -type f | sed 's|^\./||' | sort); do
            if [ ! -f "$work/$name/project/$file" ]; then
                echo "cli.sh: $name: the program did not make the file $file" >&2
                ok=0
                continue
            fi
            mkdir -p "$(dirname "$work/$name/files/$file")"
            normalize "$work/$name/project/$file" "$work/$name/files/$file"
            if ! cmp -s "$work/$name/files/$file" "$cases/$name/files/$file"; then
                echo "cli.sh: $name: the file $file is different" >&2
                cmp "$cases/$name/files/$file" "$work/$name/files/$file" 2>&1 | head -2 >&2 || true
                ok=0
            fi
        done
    fi
    # For an error: each message line of the official program (the lines
    # of the box, or all the lines after the header when there is no box)
    # must be in the output, without the colors.
    esc=$(printf '\033')
    # For a build with no error: each line of the warnings of the official
    # program must be in the error output, without the colors.
    if [ ! -f "$cases/$name/stderr.txt" ] && [ -f "$cases/$name/stderr-contains.txt" ]; then
        sed "s/$esc\[[0-9;]*m//g" "$work/$name/stderr.txt" | tr -d "\r" > "$work/$name/stderr.plain"
        case "$(uname -s)" in
            MINGW*|MSYS*|CYGWIN*) grep -v '<project>' "$cases/$name/stderr-contains.txt" > "$work/$name/expected.txt" || true ;;
            *) sed "s|<project>|$project|g" "$cases/$name/stderr-contains.txt" > "$work/$name/expected.txt" ;;
        esac
        while IFS= read -r line; do
            if ! grep -qF -- "$line" "$work/$name/stderr.plain"; then
                echo "cli.sh: $name: the error output does not have the warning line: $line" >&2
                ok=0
            fi
        done < "$work/$name/expected.txt"
    fi
    if [ -f "$cases/$name/stderr.txt" ]; then
        if [ -f "$cases/$name/stderr-contains.txt" ]; then
            cat "$cases/$name/stderr-contains.txt" > "$work/$name/expected.raw"
        else
            sed "s/$esc\[[0-9;]*m//g" "$cases/$name/stderr.txt" | grep '^│' > "$work/$name/expected.raw" || true
            if [ ! -s "$work/$name/expected.raw" ]; then
                grep -v '^≈' "$cases/$name/stderr.txt" | grep . > "$work/$name/expected.raw" || true
            fi
        fi
        case "$(uname -s)" in
            # On Windows, the program and the shell do not write the path
            # of the project in the same way: compare the text before and
            # after the path.
            MINGW*|MSYS*|CYGWIN*) sed "s|<project>|\n|g" "$work/$name/expected.raw" | sed "s/^│ *$//" | grep -v '^$' > "$work/$name/expected.txt" || true ;;
            *) sed "s|<project>|$project|g" "$work/$name/expected.raw" > "$work/$name/expected.txt" ;;
        esac
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
