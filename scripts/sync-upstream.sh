#!/bin/sh
# Bring this repository to another version of Tailwind CSS.
#
# Usage:
#   sh scripts/sync-upstream.sh check
#       Check that each place that names the upstream version agrees with
#       the file UPSTREAM. CI runs this.
#   sh scripts/sync-upstream.sh latest
#       Print the newest release of Tailwind CSS.
#   sh scripts/sync-upstream.sh report VERSION [WORK]
#       Write what changed in Tailwind CSS between the version of UPSTREAM
#       and VERSION: WORK/PORT.md (the list of files of this repository to
#       change), and WORK/diffs/ (one diff for each upstream file).
#       This step changes nothing in the repository.
#   sh scripts/sync-upstream.sh apply VERSION [WORK]
#       Do the mechanical part: the version numbers, the stylesheets, the
#       versions of the scanner and of Lightning CSS, the generated files,
#       and the record of the official compiler (tests/upstream). Then
#       run the tests: the failures are the behavior to port by hand, with
#       WORK/PORT.md as the guide.
#
# WORK is a scratch directory (default: build/sync). The steps need git,
# bash, Node.js 22, pnpm and npm. See docs/SYNC.md.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
# shellcheck disable=SC1091
. ./UPSTREAM
REPO=https://github.com/tailwindlabs/tailwindcss.git

log() { echo "sync-upstream.sh: $*" >&2; }
die() { log "$*"; exit 1; }

# set_key KEY VALUE: change one line of the file UPSTREAM.
set_key() {
    sed -i.bak "s|^$1=.*|$1=$2|" UPSTREAM && rm -f UPSTREAM.bak
}

# expect FILE TEXT: FILE must have TEXT.
expect() {
    if ! grep -qF -- "$2" "$1"; then
        log "$1 does not have: $2"
        status=1
    fi
}

check() {
    status=0
    # shellcheck disable=SC1091
    . ./UPSTREAM
    v=$TAILWINDCSS_VERSION
    expect native/tailwind/src/index.rs "pub const VERSION: &str = \"$v\";"
    # The scanner (native/scanner): the copy of the tag of UPSTREAM. The
    # job "test" of CI compares the files with the tag (scripts/scanner.sh).
    expect native/scanner/README.md "the tag of the version in \`UPSTREAM\`"
    expect native/tailwind/Cargo.toml "lightningcss = { version = \"=$LIGHTNINGCSS_CRATE\""
    expect src/tailwind.erl "-define(LATEST, \"$v\")."
    expect tests/upstream/README.md "\`v$v\`"
    expect tests/upstream/README.md "$TAILWINDCSS_COMMIT"
    # QuickJS: the submodule is at the commit of UPSTREAM.
    if [ ! -f native/tailwind/c_src/quickjs/quickjs.c ]; then
        log "native/tailwind/c_src/quickjs is empty: run git submodule update --init"
        status=1
    elif [ "$(cat native/tailwind/c_src/quickjs/VERSION)" != "$QUICKJS_VERSION" ]; then
        log "native/tailwind/c_src/quickjs has the version $(cat native/tailwind/c_src/quickjs/VERSION), not $QUICKJS_VERSION"
        status=1
    elif [ -e native/tailwind/c_src/quickjs/.git ] && [ "$(git -C native/tailwind/c_src/quickjs rev-parse HEAD)" != "$QUICKJS_COMMIT" ]; then
        log "native/tailwind/c_src/quickjs is not at the commit $QUICKJS_COMMIT"
        status=1
    fi
    # The plugins of the standalone program, and the TypeScript transform:
    # the versions of the tools and of the files in the program.
    expect tests/upstream/tools/package.json "\"@tailwindcss/forms\": \"$STANDALONE_FORMS\""
    expect tests/upstream/tools/package.json "\"@tailwindcss/typography\": \"$STANDALONE_TYPOGRAPHY\""
    expect tests/upstream/tools/package.json "\"@tailwindcss/aspect-ratio\": \"$STANDALONE_ASPECT_RATIO\""
    expect tests/upstream/tools/package.json "\"sucrase\": \"$SUCRASE_VERSION\""
    expect native/tailwind/assets/plugins/forms.js "@tailwindcss/forms $STANDALONE_FORMS "
    expect native/tailwind/assets/plugins/typography.js "@tailwindcss/typography $STANDALONE_TYPOGRAPHY "
    expect native/tailwind/assets/plugins/aspect-ratio.js "@tailwindcss/aspect-ratio $STANDALONE_ASPECT_RATIO "
    expect native/tailwind/assets/typescript.js "sucrase $SUCRASE_VERSION"
    # The packages have one version of their own.
    p=$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' package.json)
    expect mix.exs "@version \"$p\""
    expect src/tailwind.app.src "{vsn, \"$p\"}"
    w=$(awk '/^\[workspace\.package\]/ { on = 1; next } /^\[/ { on = 0 } on && /^version *=/ { gsub(/[" ]/, ""); sub(/version=/, ""); print; exit }' Cargo.toml)
    [ "$w" = "$p" ] || { log "Cargo.toml has the version $w in [workspace.package], not $p"; status=1; }
    for crate in tailwind-com-scanner tailwind-com-classification-macros tailwind-com-ignore; do
        expect Cargo.toml "package = \"$crate\", version = \"$p\""
    done
    [ $status = 0 ] && log "all the files agree with UPSTREAM (Tailwind CSS $v), and the packages have the version $p"
    return $status
}

latest() {
    git ls-remote --tags --refs "$REPO" 'v4.*' | sed 's|.*refs/tags/v||' |
        grep -E '^[0-9]+\.[0-9]+\.[0-9]+$' | sort -t. -k1,1n -k2,2n -k3,3n | tail -1
}

# clone VERSION DIR: the source of Tailwind CSS at a tag.
clone() {
    if [ ! -d "$2/.git" ]; then
        git clone -q --depth 1 --branch "v$1" "$REPO" "$2"
    fi
}

report() {
    new=$1
    old=$TAILWINDCSS_VERSION
    [ "$new" != "$old" ] || die "UPSTREAM has $old already"
    mkdir -p "$work/diffs"
    clone "$old" "$work/old"
    clone "$new" "$work/new"
    out=$work/PORT.md
    {
        echo "# Tailwind CSS $old to $new"
        echo
        echo "Each upstream file that changed, and the file of this repository for it"
        echo "(tests/upstream/files.map). The diffs are in \`diffs/\`."
        echo
        echo "| Upstream file | Lines | Here | How |"
        echo "|---|---|---|---|"
    } > "$out"
    unmapped=
    # Each file that is different, without the tests and the benchmarks.
    for dir in packages/tailwindcss packages/@tailwindcss-cli/src packages/@tailwindcss-node/src \
        packages/@tailwindcss-standalone/src crates pnpm-workspace.yaml; do
        diff -rq "$work/old/$dir" "$work/new/$dir" 2>/dev/null || true
    done | sed -e "s|^Files $work/old/\(.*\) and .* differ$|\1|" \
        -e "s|^Only in $work/new/\(.*\): \(.*\)$|\1/\2|" \
        -e "s|^Only in $work/old/\(.*\): \(.*\)$|\1/\2|" |
        grep -v -E '\.(test|bench)\.[cm]?ts$|__snapshots__|/tests/|/fixtures/|\.snap$|CHANGELOG|package\.json$|README' |
        sort > "$work/changed.txt"
    while IFS= read -r file; do
        [ -n "$file" ] || continue
        # The longest path of the map that is a prefix of the file.
        entry=$(grep -v '^#' tests/upstream/files.map | awk -F'\t' -v f="$file" '
            index(f, $1) == 1 && length($1) > best { best = length($1); line = $0 }
            END { print line }')
        here=$(printf '%s' "$entry" | cut -f2)
        how=$(printf '%s' "$entry" | cut -f3)
        name=$(printf '%s' "$file" | tr '/@' '__')
        diff -u -N "$work/old/$file" "$work/new/$file" > "$work/diffs/$name.diff" 2>/dev/null || true
        lines=$(grep -c '^[+-][^+-]' "$work/diffs/$name.diff" || true)
        if [ -z "$entry" ]; then
            unmapped="$unmapped $file"
            here="?"
            how="**no entry in files.map: decide, and add one**"
        fi
        [ "$how" = skip ] && continue
        echo "| \`$file\` | $lines | \`$here\` | $how |" >> "$out"
    done < "$work/changed.txt"
    {
        echo
        echo "## The changelog of Tailwind CSS"
        echo
        awk -v new="$new" -v old="$old" '
            index($0, "## [" old "]") == 1 { exit }
            index($0, "## [" new "]") == 1 { on = 1 }
            on { print }
        ' "$work/new/CHANGELOG.md"
    } >> "$out"
    log "wrote $out ($(grep -c '^| `' "$out") files to look at)"
    [ -z "$unmapped" ] || log "no entry in tests/upstream/files.map for:$unmapped"
}

apply() {
    new=$1
    report "$new"
    clone=$work/new
    commit=$(git -C "$clone" rev-parse HEAD)
    old=$TAILWINDCSS_VERSION

    log "The version numbers"
    sed -i.bak "s|pub const VERSION: &str = \"$old\";|pub const VERSION: \&str = \"$new\";|" native/tailwind/src/index.rs
    sed -i.bak "s|tailwindcss v$old \||tailwindcss v$new \||" native/tailwind/src/index.rs
    sed -i.bak "s|-define(LATEST, \"$old\").|-define(LATEST, \"$new\").|; s|{version, \"$old\"}|{version, \"$new\"}|" src/tailwind.erl
    sed -i.bak "s|version: \"$old\"|version: \"$new\"|" lib/tailwind.ex README.md
    sed -i.bak "s|v$old|v$new|g; s|@$old|@$new|g; s|$TAILWINDCSS_COMMIT|$commit|" tests/upstream/README.md
    sed -i.bak "s|tag v$old|tag v$new|; s| $old\$| $new|" tests/upstream/tools/make-all.sh
    sed -i.bak "s|version = '$old'|version = '$new'|" tests/upstream/tools/run-cli.mjs
    rm -f README.md.bak tests/upstream/README.md.bak
    find native src lib tests/upstream/tools -name '*.bak' -exec rm -f {} +
    set_key TAILWINDCSS_VERSION "$new"
    set_key TAILWINDCSS_COMMIT "$commit"

    log "The scanner: the crates of the tag v$new (native/scanner)"
    sh scripts/scanner.sh "$clone"
    if ! git diff --quiet -- native/scanner/manifests; then
        log "the manifests of the scanner changed: change native/scanner/*/Cargo.toml in the same way"
        git diff --stat -- native/scanner/manifests >&2
    fi

    log "Lightning CSS: the crate of the npm version that Tailwind CSS uses"
    npm_version=$(sed -n 's/^ *lightningcss: *\([0-9.]*\).*/\1/p' "$clone/pnpm-workspace.yaml" | head -1)
    if [ -n "$npm_version" ] && [ "$npm_version" != "$LIGHTNINGCSS_NPM" ]; then
        crate=$(curl -fsSL "https://raw.githubusercontent.com/parcel-bundler/lightningcss/v$npm_version/Cargo.toml" |
            awk '/^\[package\]/ { on = 1 } on && /^version *=/ { gsub(/[" ]/, "", $0); sub(/version=/, "", $0); print; exit }')
        [ -n "$crate" ] || die "no crate version for lightningcss $npm_version"
        sed -i.bak "s|lightningcss = { version = \"=$LIGHTNINGCSS_CRATE\"|lightningcss = { version = \"=$crate\"|; s|lightningcss $LIGHTNINGCSS_NPM|lightningcss $npm_version|" native/tailwind/Cargo.toml
        rm -f native/tailwind/Cargo.toml.bak
        set_key LIGHTNINGCSS_NPM "$npm_version"
        set_key LIGHTNINGCSS_CRATE "$crate"
        log "Lightning CSS $npm_version is the crate $crate"
    fi

    log "The stylesheets of the repository of Tailwind CSS"
    # The files of the repository, as the standalone program has them. The
    # npm package has another index.css, in another format.
    for f in index theme preflight utilities; do
        cp "$clone/packages/tailwindcss/$f.css" native/tailwind/assets/$f.css
        cp "$clone/packages/tailwindcss/$f.css" tests/upstream/css/$f.css
    done

    log "The dependencies of the clone (pnpm)"
    (cd "$clone" && pnpm install --frozen-lockfile --ignore-scripts > "$work/pnpm.log" 2>&1) ||
        die "pnpm install failed: see $work/pnpm.log"

    log "The plugins of the standalone program"
    tools=tests/upstream/tools
    # The versions of the lock file of Tailwind CSS go to UPSTREAM and to
    # the package.json of the tools. make-all.sh makes the files of the
    # program from these packages (build-plugin-bundles.mjs).
    for item in FORMS:forms TYPOGRAPHY:typography ASPECT_RATIO:aspect-ratio; do
        key=${item%%:*}
        name=${item#*:}
        version=$(sed -n "s|^  '@tailwindcss/$name@\\([0-9][^'(]*\\)':\$|\\1|p" "$clone/pnpm-lock.yaml" | head -1)
        [ -n "$version" ] || die "the lock file of Tailwind CSS has no @tailwindcss/$name"
        sed -i.bak "s|^STANDALONE_$key=.*|STANDALONE_$key=$version|" UPSTREAM
        sed -i.bak "s|\"@tailwindcss/$name\": \"[^\"]*\"|\"@tailwindcss/$name\": \"$version\"|" $tools/package.json
        rm -f UPSTREAM.bak $tools/package.json.bak
    done
    # The checksum of the standalone program, for tests/standalone.sh.
    sum=$(curl -fsSL "https://github.com/tailwindlabs/tailwindcss/releases/download/v$new/sha256sums.txt" |
        sed -n 's|^\([0-9a-f]\{64\}\)  \./tailwindcss-linux-x64$|\1|p')
    [ -n "$sum" ] || die "the release v$new has no checksum for tailwindcss-linux-x64"
    sed -i.bak "s|^STANDALONE_LINUX_X64_SHA256=.*|STANDALONE_LINUX_X64_SHA256=$sum|" UPSTREAM
    rm -f UPSTREAM.bak
    (cd $tools && npm install --no-audit --no-fund --legacy-peer-deps --ignore-scripts > "$work/npm-tools.log" 2>&1) ||
        die "npm install in $tools failed: see $work/npm-tools.log"

    log "The generated files"
    node $tools/gen-property-order.mjs "$clone" native/tailwind/src/property_order.rs
    node $tools/gen-default-theme.mjs "$clone" native/tailwind/src/compat/default_theme.rs
    # make-all.sh, below, also makes the JavaScript files of the program
    # (native/tailwind/assets/compat.js, typescript.js and plugins/) and
    # the plugin records.

    log "The record of the official compiler (this takes some minutes)"
    bash $tools/make-all.sh "$clone" "$work/record"
    node $tools/gen-nodes.mjs "$clone" tests/upstream
    node $tools/gen-extra.mjs "$clone" tests/upstream
    # After gen-extra: the source maps use its records.
    node $tools/gen-source-maps.mjs "$clone" tests/upstream
    node $tools/gen-css-utility.mjs "$clone" native/tailwind/tests/fixtures/css_utility.json
    node $tools/gen-variants.mjs "$clone" native/tailwind/tests/fixtures/variants.json
    node $tools/gen-optimize.mjs "$clone" native/tailwind/tests/fixtures/optimize.json

    # The version before this one is now an older version: its plugin
    # layer becomes a patch file (docs/SWITCHES.md, "The plugin layer").
    log "The plugin layer of each version that the Rust compiler serves"
    sh scripts/compat.sh "$clone"

    log "The official output of the cases of the fuzzer"
    sh tests/fuzz/install.sh "$work/fuzz-official"
    node tests/fuzz/refresh.mjs "$work/fuzz-official"

    check
    log "The tests: each failure is behavior of Tailwind CSS $new to port"
    if cargo test --release --no-fail-fast > "$work/test.log" 2>&1; then
        log "all the tests pass: Tailwind CSS $new needs no change by hand"
    else
        grep -E 'test result: FAILED|^test .* FAILED|records|different' "$work/test.log" | head -40 >&2 || true
        log "some tests fail: see $work/test.log, and port the changes of $work/PORT.md"
    fi
}

command=${1:-}
work=${3:-$root/build/sync}
case "$command" in
    check) check ;;
    latest) latest ;;
    report) [ $# -ge 2 ] || die "report needs a version"; mkdir -p "$work"; work=$(cd "$work" && pwd); report "$2" ;;
    apply) [ $# -ge 2 ] || die "apply needs a version"; mkdir -p "$work"; work=$(cd "$work" && pwd); apply "$2" ;;
    *) sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac
