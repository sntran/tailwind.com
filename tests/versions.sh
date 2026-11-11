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
echo '<p class="flex md:p-4 hover:from-plugin"></p>' > src/a.html

# The version line.
line=$(run --version)
[ "$line" = "tailwind.com v$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' "$root/package.json") (tailwindcss v$native)" ] ||
    fail "--version gives: $line"

# The two engines give the same output for the version of the Rust
# compiler.
run -i app.css -o native.css 2> /dev/null || fail "the native engine failed"
TAILWIND_COM_ENGINE=script run -i app.css -o script.css 2> script.log ||
    { cat script.log >&2; fail "the script engine failed for $native"; }
cmp -s native.css script.css || fail "the script engine and the native engine give different output for $native"
echo "versions.sh: $native: the script engine gives the output of the native engine: pass"

# Each version of PACKAGES.
count=0
for version in $(grep -v '^#' "$root/PACKAGES" | cut -d' ' -f1); do
    [ "$version" = "$native" ] && continue
    line=$(TAILWINDCSS_VERSION=$version run --version) || fail "$version is not in the program"
    case "$line" in
        *"(tailwindcss v$version, script engine)") ;;
        *) fail "$version: --version gives: $line" ;;
    esac
    TAILWINDCSS_VERSION=$version run -i app.css -o out.css 2> out.log || { cat out.log >&2; fail "$version: the build failed"; }
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
echo "versions.sh: $count other versions build the project with their own compiler: pass"

# A version that is not there, and a wrong engine.
if TAILWINDCSS_VERSION=4.999.0 run -i app.css -o out.css 2> out.log; then
    fail "the version 4.999.0 gave no error"
fi
grep -q 'Tailwind CSS v4.999.0 is not in this program' out.log || { cat out.log >&2; fail "the message for 4.999.0"; }
if TAILWINDCSS_VERSION=4.0.0 TAILWIND_COM_ENGINE=native run -i app.css -o out.css 2> out.log; then
    fail "the native engine with 4.0.0 gave no error"
fi
grep -q 'needs the script engine' out.log || { cat out.log >&2; fail "the message for the native engine"; }
echo "versions.sh: a version that is not there, and the wrong engine, are errors: pass"
