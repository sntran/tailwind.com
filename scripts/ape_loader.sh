#!/bin/sh
# Give the APE loader of an APE file a name with the SHA-256 of the
# loaders in the file.
#
# Usage: sh scripts/ape_loader.sh FILE
#
# The shell header of an APE file (apelink of cosmocc) runs the APE
# loader $TMPDIR/.ape-1.10 (or ~/.ape-1.10) when that file exists. Else
# it extracts the loader of the file to that path: on macOS arm64 it
# compiles the C source of the loader with cc. The header does not
# examine the contents of a loader that exists. Each APE program writes
# its loader to the same path, also when its loader is different (the
# loader of BEAM.com has a patch for macOS). Thus a program can start
# with the loader of another program, and fail (issue #28).
#
# This script changes the path in the header to .ape-1.10-HASH, with
# HASH the first 12 hexadecimal digits of the SHA-256 of the loaders in
# the file (each one after gzip -dc, in the sequence of the header).
# Thus a program uses only a loader with the same contents as its own.
# The header keeps its size: the script removes the same number of
# spaces from the padding of the numbers of the dd commands. The offsets
# in the file do not change. scripts/build.sh runs it after the link, and
# tests/ape_loader.sh checks the file. It prints the new name of the
# loader.
# The patterns are text of the header: $ is not an expansion here.
# shellcheck disable=SC2016
set -eu
# The header is binary data: the byte locale for grep and sed.
export LC_ALL=C
f=$1
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail() {
    echo "ape_loader.sh: $f: $*" >&2
    exit 1
}

# The shell header is in the first 64 KiB of the file.
head -c 65536 "$f" > "$tmp/head"
line='t="${TMPDIR:-${HOME:-.}}/.ape-'
n=$(grep -c -a -F "$line" "$tmp/head" || true)
[ "$n" = 1 ] || fail "the shell header has $n lines with $line, not 1"
start=$(grep -a -b -F "$line" "$tmp/head" | cut -d: -f1)
end=$(grep -a -b -x 'exit 127' "$tmp/head" | head -n 1 | cut -d: -f1)
if [ -z "$end" ] || [ "$end" -le "$start" ]; then
    fail "the shell header has no line exit 127"
fi
size=$((end - start))
tail -c +$((start + 1)) "$tmp/head" | head -c "$size" > "$tmp/old"
first=$(head -n 1 "$tmp/old")
case $first in
    *'/.ape-'*-*'"') fail "the loader already has a name with a hash: $first" ;;
esac

# The loaders: each "dd if="$o" skip=S count=C bs=1 ... | gzip -dc".
sed -n 's/^dd if="\$o" skip=\([0-9]*\) *count=\([0-9]*\) *bs=1 .*| gzip -dc.*/\1 \2/p' "$tmp/old" |
    awk '!seen[$0]++' > "$tmp/loaders"
[ -s "$tmp/loaders" ] || fail "the shell header has no loader"
: > "$tmp/all"
while read -r skip count; do
    tail -c +$((skip + 1)) "$f" | head -c "$count" | gzip -dc >> "$tmp/all" ||
        fail "the loader at $skip ($count bytes) is not gzip data"
done < "$tmp/loaders"
hash=$(sha256sum < "$tmp/all" | cut -c1-12)
[ ${#hash} = 12 ] || fail "no SHA-256"

# The new path, and the same number of spaces less in the dd commands.
extra=$((${#hash} + 1))
awk -v hash="$hash" -v need="$extra" '
    NR == 1 { sub(/"$/, "-" hash "\"") }
    /^dd / {
        while (need > 0 && (i = index($0, "  ")) > 0) {
            $0 = substr($0, 1, i) substr($0, i + 2)
            need--
        }
    }
    { print }
    END { if (need > 0) exit 1 }
' "$tmp/old" > "$tmp/new" || fail "the dd commands have not enough spaces for the hash"
[ "$(wc -c < "$tmp/new")" -eq "$size" ] || fail "the new shell header has a different size"
dd of="$f" bs=1 seek="$start" conv=notrunc < "$tmp/new" 2>/dev/null
head -n 1 "$tmp/new" | sed 's/.*\///; s/"$//'
