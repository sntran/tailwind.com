#!/bin/sh
# Check the TLA+ models of specs/ with TLC.
#
# Usage: sh scripts/tlc.sh
#
# TLC needs Java 11 or later and tla2tools.jar. The script reads the jar
# from TLA2TOOLS_JAR, then ~/.tla/tla2tools.jar, then build/tla2tools.jar,
# and downloads the release v1.7.4 to build/ when no jar is there.
#
# Each .cfg file of specs/ is one run of the model with the same name
# (the text before the first dot). A file NAME.wrong_*.cfg is a run that
# must FAIL: it shows that the model finds the error that it is for.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
sha=936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88
jar=${TLA2TOOLS_JAR:-}
if [ -z "$jar" ]; then
    jar=$root/build/tla2tools.jar
    [ -f "$HOME/.tla/tla2tools.jar" ] && jar=$HOME/.tla/tla2tools.jar
fi
if [ ! -f "$jar" ]; then
    mkdir -p "$(dirname "$jar")"
    curl -fsSL -o "$jar" https://github.com/tlaplus/tlaplus/releases/download/v1.7.4/tla2tools.jar
fi
if [ "$(sha256sum "$jar" | cut -d' ' -f1)" != "$sha" ]; then
    echo "tlc.sh: $jar is not tla2tools.jar of v1.7.4" >&2
    exit 1
fi
mkdir -p "$root/build/tlc"
cd "$root/specs"
status=0
for cfg in *.cfg; do
    model=${cfg%%.*}
    out=$root/build/tlc/$cfg.txt
    if java -XX:+UseParallelGC -cp "$jar" tlc2.TLC -workers auto \
        -metadir "$root/build/tlc/$cfg.states" -config "$cfg" "$model.tla" > "$out" 2>&1; then
        result=pass
    else
        result=fail
    fi
    case "$cfg" in
        *.wrong_*) expected=fail ;;
        *) expected=pass ;;
    esac
    states=$(grep -o '[0-9,]* distinct states found' "$out" | tail -1)
    if [ "$result" = "$expected" ]; then
        echo "tlc.sh: $cfg: $result, as expected ($states)"
    else
        echo "tlc.sh: $cfg: $result, but expected $expected" >&2
        tail -40 "$out" >&2
        status=1
    fi
done
exit $status
