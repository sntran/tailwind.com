#!/usr/bin/env bash
# Usage: tools/make-all.sh <path of the upstream clone> <scratch directory>
# This script makes all the fixture files again.
# The clone must be at the tag v4.3.5 and must have its dependencies:
#   pnpm install --frozen-lockfile --ignore-scripts
set -euo pipefail

CLONE=$(cd "$1" && pwd)
mkdir -p "$2"
SCRATCH=$(cd "$2" && pwd)
TOOLS=$(cd "$(dirname "$0")" && pwd)
OUT=$(dirname "$TOOLS")

# 0. The JavaScript files of the program that the cases of cli-js need:
# the plugins of the standalone program and the TypeScript transform
# (native/tailwind/assets/plugins/, native/tailwind/assets/typescript.js).
# They need the packages of package.json of this directory.
[ -d "$TOOLS/node_modules/sucrase" ] ||
    (cd "$TOOLS" && npm install --no-audit --no-fund --legacy-peer-deps --ignore-scripts)
node "$TOOLS/build-plugin-bundles.mjs" "$CLONE"

# 1. suite.jsonl.gz
node "$TOOLS/instrument.mjs" "$CLONE"
rm -rf "$SCRATCH/events"
(cd "$CLONE" && TW_ORACLE_DIR="$SCRATCH/events" pnpm vitest run --project tailwindcss)
node "$TOOLS/merge-suite.mjs" "$SCRATCH/events" "$OUT/suite.jsonl.gz"

# 2. classes.jsonl.gz and classes-all.json.gz
node "$TOOLS/gen-classes.mjs" "$CLONE" "$OUT"

# 3. full.json.gz and css/
node "$TOOLS/gen-full.mjs" "$CLONE" "$OUT"

# 4. cli/*/stdout.css and cli/*/exit.txt, and the same for cli-map (with
# files/) and cli-js
node "$TOOLS/run-cli.mjs" "$SCRATCH/cli" "$OUT/cli" 4.3.5
node "$TOOLS/run-cli.mjs" "$SCRATCH/cli" "$OUT/cli-map" 4.3.5
node "$TOOLS/run-cli.mjs" "$SCRATCH/cli" "$OUT/cli-js" 4.3.5

# 5. plugin-suite.jsonl.gz: the unit tests again, with each JavaScript
# module as source text. The script changes plugin.ts of the clone.
node "$TOOLS/gen-plugin-suite.mjs" "$CLONE" "$SCRATCH" "$OUT/plugin-suite.jsonl.gz"

# 6. candidates.jsonl.gz
node "$TOOLS/gen-candidates.mjs" "$CLONE" "$OUT"

# 7. native/tailwind/assets/compat.js: the plugin layer for the program
node "$TOOLS/build-compat-bundle.mjs" "$CLONE"

# 8. native/tailwind/tests/fixtures/node_api.json: the behavior of this
# Node.js for the parts of Node.js that the program gives to a plugin
node "$TOOLS/gen-node-api.mjs"

# 9. Check. source-maps.jsonl.gz uses the records of gen-extra.mjs:
# scripts/sync-upstream.sh makes it after this script.
python3 "$TOOLS/check.py" "$OUT"
