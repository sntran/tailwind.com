#!/bin/sh
# A test of the watch of the program (--watch), on this system.
#
# Usage: sh tests/watch.sh PROGRAM [OPTION...]     (for example --poll=100)
#
# It checks that an edit of a source file, a new file in a new directory,
# an edit of the input, an input with an error and its repair, and an
# edit of a JavaScript plugin each give the correct output, and that the
# end of stdin stops the program. The watch runs with --map out.css.map:
# the test also checks that the source map follows the input.
set -eu
program=$1
shift
case "$program" in /*) ;; *) program=$(pwd)/$program ;; esac
work=$(mktemp -d "${TMPDIR:-/tmp}/tailwind-watch.XXXXXX")
mkdir -p "$work/src"
cd "$work"

cat > app.css <<'CSS'
@import "tailwindcss" source(none);
@source "./src";
CSS
echo '<p class="flex"></p>' > src/a.html

run() {
    case "$(uname -s):$program" in
        MINGW*|MSYS*|CYGWIN*) "$program" "$@" ;;
        *.com) sh "$program" "$@" ;;
        *) "$program" "$@" ;;
    esac
}

# wait_for TEXT: wait at most 15 s until out.css has TEXT.
wait_for() {
    i=0
    while [ $i -lt 150 ]; do
        if grep -qF -- "$1" out.css 2>/dev/null; then
            return 0
        fi
        sleep 0.1
        i=$((i + 1))
    done
    echo "watch.sh: out.css did not get: $1" >&2
    cat log.txt >&2 || true
    return 1
}

# The program reads stdin from a FIFO: when the writer closes it, the
# watch must stop. Where the shell has no FIFO that works (Windows), the
# watch runs with --watch=always, and the last check does not run.
fifo=1
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) fifo=0 ;;
    *) mkfifo stdin.fifo ;;
esac
if [ $fifo = 1 ]; then
    run -i app.css -o out.css --map out.css.map --watch "$@" < stdin.fifo > log.txt 2>&1 &
    pid=$!
    exec 3> stdin.fifo
else
    run -i app.css -o out.css --map out.css.map --watch=always "$@" > log.txt 2>&1 &
    pid=$!
fi
trap 'kill $pid 2>/dev/null || true' EXIT

wait_for '.flex {'
echo "watch.sh: the first build: pass"

echo '<p class="flex underline"></p>' > src/a.html
wait_for '.underline {'
echo "watch.sh: an edit of a source file: pass"

mkdir -p src/deep/er
echo '<p class="italic"></p>' > src/deep/er/b.html
wait_for '.italic {'
echo "watch.sh: a new file in a new directory: pass"

# A replace of the file by a rename, as many editors save a file.
echo '<p class="flex uppercase"></p>' > src/a.html.tmp
mv src/a.html.tmp src/a.html
wait_for '.uppercase {'
echo "watch.sh: a file replaced by a rename: pass"

printf '@import "tailwindcss" source(none);\n@source "./src";\n.one { color: red; }\n' > app.css
wait_for '.one {'
echo "watch.sh: an edit of the input: pass"

# The program writes the map before the output.
if ! grep -qF 'sourceMappingURL=out.css.map' out.css || ! grep -qF '.one { color: red; }' out.css.map; then
    echo "watch.sh: the source map does not have the new input" >&2
    exit 1
fi
echo "watch.sh: the source map of the new input: pass"

# An error keeps the watch alive, and the repair gives a new output.
printf '@import "tailwindcss" source(none);\n.two { @apply does-not-exist; }\n' > app.css
i=0
while [ $i -lt 150 ] && ! grep -q 'does-not-exist' log.txt; do sleep 0.1; i=$((i + 1)); done
grep -q 'does-not-exist' log.txt || { echo "watch.sh: no error message" >&2; cat log.txt >&2; exit 1; }
printf '@import "tailwindcss" source(none);\n@source "./src";\n.three { color: blue; }\n' > app.css
wait_for '.three {'
echo "watch.sh: an error and its repair: pass"

# A JavaScript plugin: a change of the plugin file gives a new compiler.
cat > plugin.js <<'JS'
module.exports = ({ addUtilities }) => addUtilities({ '.from-plugin': { color: 'red' } })
JS
echo '<p class="from-plugin"></p>' > src/a.html
printf '@import "tailwindcss" source(none);\n@source "./src";\n@plugin "./plugin.js";\n' > app.css
wait_for 'color: red'
cat > plugin.js <<'JS'
module.exports = ({ addUtilities }) => addUtilities({ '.from-plugin': { color: 'green' } })
JS
wait_for 'color: green'
echo "watch.sh: an edit of a JavaScript plugin: pass"

# The end of stdin stops the program.
if [ $fifo = 0 ]; then
    echo "watch.sh: the end of stdin: not checked on this system"
    exit 0
fi
exec 3>&-
i=0
while [ $i -lt 100 ] && kill -0 $pid 2>/dev/null; do sleep 0.1; i=$((i + 1)); done
if kill -0 $pid 2>/dev/null; then
    echo "watch.sh: the program did not stop at the end of stdin" >&2
    exit 1
fi
echo "watch.sh: the end of stdin stops the watch: pass"
