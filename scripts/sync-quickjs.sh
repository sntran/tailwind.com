#!/bin/sh
# Move the QuickJS submodule (c_src/quickjs) to another commit.
#
# Usage: sh scripts/sync-quickjs.sh COMMIT_OR_TAG
#
# The files of QuickJS are not changed. After the move: build, and run the
# tests. See c_src/QUICKJS.md.
set -eu
ref=${1:?Usage: sh scripts/sync-quickjs.sh COMMIT_OR_TAG}
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
git submodule update --init c_src/quickjs
git -C c_src/quickjs fetch -q origin
git -C c_src/quickjs checkout -q "$ref"
commit=$(git -C c_src/quickjs rev-parse HEAD)
version=$(cat c_src/quickjs/VERSION)
cp c_src/quickjs/LICENSE licenses/quickjs.txt
sed -i.bak "s|^QUICKJS_VERSION=.*|QUICKJS_VERSION=$version|; s|^QUICKJS_COMMIT=.*|QUICKJS_COMMIT=$commit|" UPSTREAM
rm -f UPSTREAM.bak
git add c_src/quickjs UPSTREAM licenses/quickjs.txt
echo "sync-quickjs.sh: QuickJS $version, commit $commit" >&2
echo "sync-quickjs.sh: update the version in c_src/QUICKJS.md and in NOTICE, then build and test" >&2
