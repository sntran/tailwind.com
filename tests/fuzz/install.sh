#!/bin/sh
# Install the official npm packages of the version of UPSTREAM in DIR, for
# tests/fuzz/fuzz.mjs.
#
# Usage: sh tests/fuzz/install.sh DIR
set -eu
dir=${1:?Usage: sh tests/fuzz/install.sh DIR}
root=$(cd "$(dirname "$0")/../.." && pwd)
# shellcheck disable=SC1091
. "$root/UPSTREAM"
mkdir -p "$dir"
cd "$dir"
[ -f package.json ] || echo '{"private": true}' > package.json
npm install --silent --no-audit --no-fund \
    "tailwindcss@$TAILWINDCSS_VERSION" "@tailwindcss/cli@$TAILWINDCSS_VERSION"
# The stylesheets of the Tailwind CSS repository, as the standalone program
# and this program have them. The npm package has another index.css (the
# text of theme.css and preflight.css in another format).
for f in index theme preflight utilities; do
    cp "$root/tests/upstream/css/$f.css" node_modules/tailwindcss/$f.css
done
echo "install.sh: Tailwind CSS $TAILWINDCSS_VERSION in $dir, with the stylesheets of the repository" >&2
