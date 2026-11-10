#!/bin/sh
# Copy the files of QuickJS that the program uses into c_src/quickjs.
#
# Usage: sh scripts/sync-quickjs.sh DIR_OF_QUICKJS_CLONE
#
# The files are not changed. After a new version: update the version and
# the commit in c_src/quickjs/README.md and in NOTICE, build, and run the
# tests.
set -eu
src=${1:?Usage: sh scripts/sync-quickjs.sh DIR_OF_QUICKJS_CLONE}
root=$(cd "$(dirname "$0")/.." && pwd)
dst=$root/c_src/quickjs
FILES="quickjs.c quickjs.h quickjs-atom.h quickjs-opcode.h
libregexp.c libregexp.h libregexp-opcode.h
libunicode.c libunicode.h libunicode-table.h
cutils.c cutils.h dtoa.c dtoa.h list.h VERSION LICENSE"
mkdir -p "$dst"
for f in $FILES; do
    cp "$src/$f" "$dst/$f"
done
cp "$src/LICENSE" "$root/licenses/quickjs.txt"
echo "sync-quickjs.sh: QuickJS $(cat "$dst/VERSION") is in $dst" >&2
if [ -d "$src/.git" ]; then
    echo "sync-quickjs.sh: commit $(git -C "$src" rev-parse --short HEAD)" >&2
fi
