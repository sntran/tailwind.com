#!/bin/sh
# The APE loader of the shell header of tailwind.com (scripts/ape_loader.sh,
# issue #28): the program does not use a loader of another program.
#
# Usage: sh tests/ape_loader.sh PROGRAM
#
# PROGRAM is a file of scripts/build.sh. The checks, with an empty
# TMPDIR:
#   - the header names .ape-VERSION-HASH, with the SHA-256 of its loaders;
#   - an other loader at $TMPDIR/.ape-VERSION (the name of cosmocc) and
#     at $TMPDIR/.ape-VERSION-OTHERHASH does not run;
#   - the program extracts its loader one time, and uses it after that;
#   - on Linux, `--assimilate` still makes an ELF file that runs.
# The last line is "ape_loader: ok".
# The patterns are text of the header: $ is not an expansion here.
# shellcheck disable=SC2016
set -eu
program=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
failures=0

fail() {
    echo "ape_loader: $*" >&2
    failures=$((failures + 1))
}

head -c 65536 "$program" > "$tmp/head"
line=$(grep -a '^t="${TMPDIR:-${HOME:-.}}/\.ape-' "$tmp/head" || true)
name=$(echo "$line" | sed 's/.*\///; s/"$//')
case $name in
    .ape-*-????????????) ;;
    *) fail "the header names the loader '$name', with no hash" ;;
esac
version=$(echo "$name" | sed 's/^\.ape-//; s/-.*//')

# The hash: the SHA-256 of the loaders, as gzip -dc gives them.
sed -n 's/^dd if="\$o" skip=\([0-9]*\) *count=\([0-9]*\) *bs=1 .*| gzip -dc.*/\1 \2/p' "$tmp/head" |
    awk '!seen[$0]++' > "$tmp/loaders"
: > "$tmp/all"
while read -r skip count; do
    tail -c +$((skip + 1)) "$program" | head -c "$count" | gzip -dc >> "$tmp/all"
done < "$tmp/loaders"
hash=$(sha256sum < "$tmp/all" | cut -c1-12)
[ "$name" = ".ape-$version-$hash" ] || fail "the header names $name, not .ape-$version-$hash"

# Two other loaders, which fail.
dir=$tmp/tmpdir
mkdir -p "$dir"
for other in ".ape-$version" ".ape-$version-000000000000"; do
    printf '#!/bin/sh\necho "the other loader runs" >&2\nexit 3\n' > "$dir/$other"
    chmod +x "$dir/$other"
done
if ! TMPDIR=$dir sh "$program" --version > "$tmp/out" 2> "$tmp/err"; then
    fail "the program fails with other loaders in TMPDIR: $(cat "$tmp/err")"
fi
grep -q '^tailwind.com v' "$tmp/out" || fail "no version: $(cat "$tmp/out")"
grep -q 'the other loader runs' "$tmp/err" && fail "an other loader runs"
[ -x "$dir/$name" ] || fail "no loader $dir/$name"

# The second start uses the loader of the first start.
if [ -x "$dir/$name" ]; then
    touch -t 200001010000 "$dir/$name"
    before=$(ls -l "$dir/$name")
    TMPDIR=$dir sh "$program" --version > "$tmp/out" || fail "the second start fails"
    [ "$(ls -l "$dir/$name")" = "$before" ] || fail "the second start extracts the loader again"
fi

# --assimilate changes the header of a copy to ELF (Linux only).
if [ "$(uname -s)" = Linux ]; then
    cp "$program" "$tmp/copy.com"
    TMPDIR=$dir sh "$tmp/copy.com" --assimilate || fail "--assimilate fails"
    [ "$(od -An -c -N4 "$tmp/copy.com" | tr -d ' ')" = '177ELF' ] || fail "--assimilate gives no ELF file"
    "$tmp/copy.com" --version > "$tmp/out" || fail "the ELF file of --assimilate fails"
fi

if [ "$failures" != 0 ]; then
    echo "ape_loader: $failures failures"
    exit 1
fi
echo "ape_loader: ok"
