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

# 1. suite.jsonl.gz
node "$TOOLS/instrument.mjs" "$CLONE"
rm -rf "$SCRATCH/events"
(cd "$CLONE" && TW_ORACLE_DIR="$SCRATCH/events" pnpm vitest run --project tailwindcss)
node "$TOOLS/merge-suite.mjs" "$SCRATCH/events" "$OUT/suite.jsonl.gz"

# 2. classes.jsonl.gz and classes-all.json.gz
node "$TOOLS/gen-classes.mjs" "$CLONE" "$OUT"

# 3. full.json.gz and css/
node "$TOOLS/gen-full.mjs" "$CLONE" "$OUT"

# 4. cli/*/stdout.css and cli/*/exit.txt
node "$TOOLS/run-cli.mjs" "$SCRATCH/cli" "$OUT/cli" 4.3.5
node "$TOOLS/run-cli.mjs" "$SCRATCH/cli" "$OUT/cli-map" 4.3.5

# 5. source-maps.jsonl.gz
node "$TOOLS/gen-source-maps.mjs" "$CLONE" "$OUT"

# 6. Check
python3 "$TOOLS/check.py" "$OUT"
