#!/bin/sh
# Move the QuickJS submodule (native/tailwind/c_src/quickjs) to another commit.
#
# Usage: sh scripts/sync-quickjs.sh COMMIT_OR_TAG
#
# The files of QuickJS are not changed. After the move: build, and run the
# tests. See native/tailwind/c_src/QUICKJS.md.
set -eu
ref=${1:?Usage: sh scripts/sync-quickjs.sh COMMIT_OR_TAG}
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
git submodule update --init native/tailwind/c_src/quickjs
git -C native/tailwind/c_src/quickjs fetch -q origin
git -C native/tailwind/c_src/quickjs checkout -q "$ref"
commit=$(git -C native/tailwind/c_src/quickjs rev-parse HEAD)
version=$(cat native/tailwind/c_src/quickjs/VERSION)
cp native/tailwind/c_src/quickjs/LICENSE licenses/quickjs.txt
sed -i.bak "s|^QUICKJS_VERSION=.*|QUICKJS_VERSION=$version|; s|^QUICKJS_COMMIT=.*|QUICKJS_COMMIT=$commit|" UPSTREAM
rm -f UPSTREAM.bak
git add native/tailwind/c_src/quickjs UPSTREAM licenses/quickjs.txt
echo "sync-quickjs.sh: QuickJS $version, commit $commit" >&2
echo "sync-quickjs.sh: update the version in native/tailwind/c_src/QUICKJS.md and in NOTICE, then build and test" >&2
