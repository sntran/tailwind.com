#!/bin/sh
# The half of the file that runs on this system: x86_64 or aarch64.
#
# Usage: sh tests/arch.sh PROGRAM ARCH
#
# ARCH is the CPU of the system, with the names of Node.js: x64 or arm64.
# A plugin puts process.arch into the CSS, and the program gives it the
# CPU of its own code. The file must run the code for the CPU of the
# system. On Windows on ARM64, Windows starts the x86_64 half in its x64
# emulation, and that half starts the aarch64 half with the loader of
# the ZIP of the file (native/tailwind/c_src/main.c). The last line is
# "arch: ARCH: pass".
#
# TAILWIND_RUN, as for tests/cli.sh: a program that starts PROGRAM.
set -eu

[ $# -eq 2 ] || { echo "usage: sh tests/arch.sh PROGRAM ARCH" >&2; exit 2; }
program=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
want=$2

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cat > "$work/arch.js" << 'EOF'
module.exports = function ({ addBase }) {
  addBase({ ":root": { "--arch": process.arch } });
};
EOF
printf '@import "tailwindcss";\n@plugin "./arch.js";\n' > "$work/in.css"

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

cd "$work"
if ! run --input in.css --output out.css 2> stderr.txt; then
    cat stderr.txt >&2
    echo "arch: the build failed" >&2
    exit 1
fi
got=$(tr -d '\r' < out.css | sed -n 's/.*--arch: *\([a-z0-9]*\).*/\1/p' | head -n 1)
if [ "$got" != "$want" ]; then
    echo "arch: the program runs the code for '$got', not for '$want'" >&2
    exit 1
fi
echo "arch: $want: pass"
