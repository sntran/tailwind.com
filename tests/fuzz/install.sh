#!/bin/sh
# Install the official npm packages of a version of Tailwind CSS in DIR,
# for tests/fuzz/fuzz.mjs. The default is the version of UPSTREAM.
#
# Usage: sh tests/fuzz/install.sh DIR [VERSION]
set -eu
dir=${1:?Usage: sh tests/fuzz/install.sh DIR [VERSION]}
root=$(cd "$(dirname "$0")/../.." && pwd)
# shellcheck disable=SC1091
. "$root/UPSTREAM"
native=$TAILWINDCSS_VERSION
TAILWINDCSS_VERSION=${2:-$native}
mkdir -p "$dir"
cd "$dir"
[ -f package.json ] || echo '{"private": true}' > package.json
# The plugins of the fuzzer: the three plugins of the standalone program, at
# the versions of tests/upstream/tools/package.json, and tailwindcss-animate.
plugins=$(node -e '
  const d = require(process.argv[1]).devDependencies
  console.log(Object.entries(d).filter(([n]) => n.startsWith("@tailwindcss/")).map(([n, v]) => n + "@" + v).join(" "))
' "$root/tests/upstream/tools/package.json")
# shellcheck disable=SC2086
npm install --silent --no-audit --no-fund --legacy-peer-deps \
    "tailwindcss@$TAILWINDCSS_VERSION" "@tailwindcss/cli@$TAILWINDCSS_VERSION" \
    $plugins tailwindcss-animate@1.0.7
# The stylesheets of the Tailwind CSS repository, as the standalone program
# and this program have them. The npm package has another index.css (the
# text of theme.css and preflight.css in another format).
# index.css of the repository is the same file in each version 4, and the
# other files of a package are the files of the repository.
if [ "$TAILWINDCSS_VERSION" = "$native" ]; then
    for f in index theme preflight utilities; do
        cp "$root/tests/upstream/css/$f.css" node_modules/tailwindcss/$f.css
    done
else
    cp "$root/tests/upstream/css/index.css" node_modules/tailwindcss/index.css
fi
# The daisyUI bundle of the fixtures (a file of Phoenix 1.8.5).
mkdir -p vendor
cp "$root/tests/upstream/cli-js/daisyui-bundle/project/vendor/daisyui.js" vendor/daisyui.js
echo "install.sh: Tailwind CSS $TAILWINDCSS_VERSION in $dir, with the stylesheets of the repository" >&2
