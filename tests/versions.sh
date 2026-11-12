#!/bin/sh
# The versions of Tailwind CSS of a program: the script engine with the
# packages that tailwind.com has in its file (docs/VERSIONS.md).
#
# Usage: sh tests/versions.sh PROGRAM
#
# For each version of the file PACKAGES, the program must build a small
# project with the compiler of that version. The test also checks the
# selection: the version line, a version that is not there, and the two
# engines for the version of the Rust compiler.
#
# With TAILWIND_COM_PACKAGES in the environment, the packages come from
# that directory (a program of `cargo build` has none in its file).
# TAILWIND_RUN is a program that starts PROGRAM, as in tests/cli.sh.
set -eu
program=$1
case "$program" in /*) ;; *) program=$(pwd)/$program ;; esac
root=$(cd "$(dirname "$0")/.." && pwd)
# shellcheck disable=SC1091
. "$root/UPSTREAM"
native=$TAILWINDCSS_VERSION
unset TAILWINDCSS_VERSION TAILWIND_COM_ENGINE

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

# choose VERSION ENGINE ARGUMENTS...: run the program with the version
# and the engine in its environment ("" for none). A variable assignment
# before a shell function does not reach the program in each shell (the
# sh of FreeBSD keeps it in the shell), so a subshell exports them.
choose() {
    (
        if [ -n "$1" ]; then TAILWINDCSS_VERSION=$1; export TAILWINDCSS_VERSION; fi
        if [ -n "$2" ]; then TAILWIND_COM_ENGINE=$2; export TAILWIND_COM_ENGINE; fi
        shift 2
        run "$@"
    )
}

fail() { echo "versions.sh: $*" >&2; exit 1; }

work=$(mktemp -d "${TMPDIR:-/tmp}/tailwind-versions.XXXXXX")
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/src"
cd "$work"
cat > app.css <<'CSS'
@import "tailwindcss" source(none);
@source "./src";
@plugin "./plugin.js";
.card { @apply underline; }
CSS
cat > plugin.js <<'JS'
const plugin = require('tailwindcss/plugin')
module.exports = plugin(({ addUtilities, theme }) => {
  addUtilities({ '.from-plugin': { color: theme('colors.red.500') } })
})
JS
echo '<p class="flex md:p-4 hover:from-plugin group-hover:underline blur-sm m-0 m-1 space-x-0"></p>' > src/a.html
# The same with no plugin. The Rust compiler compiles the two as each
# older version: it has the plugin layer of each version.
cat > plain.css <<'CSS'
@import "tailwindcss" source(none);
@source "./src";
.card { @apply underline; }
CSS

# The version line.
line=$(run --version)
[ "$line" = "tailwind.com v$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' "$root/package.json") (tailwindcss v$native)" ] ||
    fail "--version gives: $line"

# The two engines give the same output for the version of the Rust
# compiler.
run -i app.css -o native.css 2> /dev/null || fail "the native engine failed"
line=$(choose "" script --version) || fail "the script engine has no package of $native"
case "$line" in
    *"(tailwindcss v$native, script engine)") ;;
    *) fail "the script engine was not selected: --version gives: $line" ;;
esac
choose "" script -i app.css -o script.css 2> script.log ||
    { cat script.log >&2; fail "the script engine failed for $native"; }
cmp -s native.css script.css || fail "the script engine and the native engine give different output for $native"
echo "versions.sh: $native: the script engine gives the output of the native engine: pass"

# Each version of PACKAGES.
count=0
native_versions=
script_version=
for version in $(grep -v '^#' "$root/PACKAGES" | cut -d' ' -f1); do
    [ "$version" = "$native" ] && continue
    line=$(choose "$version" "" --version) || fail "$version is not in the program"
    # The Rust compiler has some of the older versions (its switches),
    # and the script engine has the other ones.
    case "$line" in
        *"(tailwindcss v$version, script engine)") script_version=$version ;;
        *"(tailwindcss v$version)") native_versions="$native_versions $version" ;;
        *) fail "$version: --version gives: $line" ;;
    esac
    choose "$version" "" -i app.css -o out.css 2> out.log || { cat out.log >&2; fail "$version: the build failed"; }
    grep -q "tailwindcss v$version" out.log || fail "$version: the header has another version"
    [ "$(head -n 1 out.css)" = "/*! tailwindcss v$version | MIT License | https://tailwindcss.com */" ] ||
        fail "$version: the license comment has another version"
    # The form of the rules is not the same in each version: 4.3.3 and
    # later put a media query around a rule, not in it.
    for text in '.flex {' '.md\:p-4' '.hover\:from-plugin' '(hover: hover)' 'text-decoration-line: underline'; do
        grep -qF -- "$text" out.css || fail "$version: the output has no $text"
    done
    count=$((count + 1))
done
echo "versions.sh: $count other versions build the project: pass"

# For a version that the Rust compiler has, the two engines must give the
# same output: the script engine is the official compiler of that version.
for version in $native_versions; do
    for input in plain.css app.css; do
        choose "$version" native -i $input -o native.css 2> /dev/null || fail "$version: the native engine failed for $input"
        choose "$version" script -i $input -o script.css 2> /dev/null || fail "$version: the script engine failed for $input"
        cmp -s native.css script.css || fail "$version: the native engine and the script engine give different output for $input"
    done
done
[ -z "$native_versions" ] || echo "versions.sh: the Rust compiler as$native_versions gives the output of the official compiler: pass"

# A version that is not there, and a wrong engine.
if choose 4.999.0 "" -i app.css -o out.css 2> out.log; then
    fail "the version 4.999.0 gave no error"
fi
grep -q 'Tailwind CSS v4.999.0 is not in this program' out.log || { cat out.log >&2; fail "the message for 4.999.0"; }
# The native engine for a version that only the script engine has. (When
# the Rust compiler has each version of PACKAGES, there is no such
# version.)
if [ -n "$script_version" ]; then
    if choose "$script_version" native -i app.css -o out.css 2> out.log; then
        fail "the native engine with $script_version gave no error"
    fi
    grep -q 'needs the script engine' out.log || { cat out.log >&2; fail "the message for the native engine"; }
fi
echo "versions.sh: a version that is not there, and the wrong engine, are errors: pass"
