# A new version of Tailwind CSS

The file [`UPSTREAM`](../UPSTREAM) names the version of Tailwind CSS that
this repository follows. `scripts/sync-upstream.sh` moves the repository
to another version. The script does all the mechanical work, and it
gives a list for the work by hand.

## The steps

```sh
sh scripts/sync-upstream.sh latest          # the newest release
sh scripts/sync-upstream.sh report 4.4.0    # what changed: build/sync/PORT.md
sh scripts/sync-upstream.sh apply 4.4.0     # the mechanical part, then the tests
```

You need git, Node.js 22, pnpm and npm.

### 1. `report`: what changed

`report` clones the two tags and compares them. It changes nothing in
the repository. It writes:

- `build/sync/PORT.md`: one row for each upstream file that changed,
  with the number of changed lines and the file of this repository for
  it, and then the changelog of Tailwind CSS for the versions between.
- `build/sync/diffs/`: one diff for each upstream file.

The relation between the files is in
[`tests/upstream/files.map`](../tests/upstream/files.map). Each Rust
file follows one file of Tailwind CSS, so a diff of 20 lines upstream is
a change of about 20 lines here. The script tells you when an upstream
file has no entry: add one (`port`, `gen`, `copy`, `dep` or `skip`).

### 2. `apply`: the mechanical part

`apply` runs `report`, and then changes the repository:

| What | Where |
|---|---|
| The version and the commit | `UPSTREAM`, `VERSION` in `index.rs`, `src/tailwind.erl`, the docs |
| The scanner of Tailwind CSS | the tag of `tailwindcss-oxide` in `native/tailwind/Cargo.toml` |
| Lightning CSS | the crate version for the npm version of `pnpm-workspace.yaml` of Tailwind CSS |
| The stylesheets | `native/tailwind/assets/*.css`, from the npm package |
| The plugins of the standalone program | their versions in the lock file of Tailwind CSS go to `UPSTREAM` and to `tests/upstream/tools/package.json`, and `tests/upstream/tools/build-plugin-bundles.mjs` makes `native/tailwind/assets/plugins/` again |
| The standalone program, for `tests/standalone.sh` | its SHA-256 from `sha256sums.txt` of the release goes to `UPSTREAM` |
| The generated files | `property_order.rs`, `compat/default_theme.rs`, the JavaScript bundle of the plugin layer (`assets/compat.js`), and the TypeScript transform (`assets/typescript.js`) |
| The record of the official compiler | all of `tests/upstream` and `native/tailwind/tests/fixtures`: the unit tests of the new Tailwind CSS run with a recorder, the official program runs on the fixture projects, and its optimizer gives its warnings |

Then it runs `cargo test`. The tests replay the new record, so each
failure is one behavior of the new version that the Rust code does not
have yet.

### 3. The work by hand

For each row of `PORT.md` with `port`: read the diff, and make the same
change in the Rust file. Run the test of that part (see
[`TESTING.md`](TESTING.md)): `tests/nodes.rs` for utilities and variants,
`tests/optimize.rs` for the last pass, `tests/suite.rs` for all of it.
The work is done when `cargo test`, `tests/cli.sh` and
`sh scripts/sync-upstream.sh check` pass.

A new test file of Tailwind CSS needs no work: the recorder takes each
`compile` call of all the unit tests.

When `packages/@tailwindcss-standalone/src/index.ts` changes (the row
of `PORT.md`): it has the list of the plugins in the standalone program
and the rule for their names. Make the same change in `EMBEDDED` and
`loadModule` of `native/tailwind/js/modules.ts`, in
`tests/upstream/tools/bun-interop.mjs`, and in
`tests/upstream/tools/package.json` for a new plugin.
`build-plugin-bundles.mjs` stops when the two lists are not the same.

After the push, the job `standalone` of CI runs the official standalone
program of the new release on the cases with `standalone.txt`. A
difference there is an error of a record, and not of the Rust code: see
[`TESTING.md`](TESTING.md).

### 4. The release

Tag the commit with the version of Tailwind CSS (`v4.4.0`). See "A
release" in [`BUILDING.md`](BUILDING.md).

## The watch for new versions

The workflow `.github/workflows/upstream.yml` runs each Monday. When
Tailwind CSS has a release that is newer than `UPSTREAM`, it runs
`report` and opens an issue with `PORT.md`.

## The other projects

- **Lightning CSS**: `apply` follows the version that Tailwind CSS uses.
- **Sucrase** (the TypeScript transform): `SUCRASE_VERSION` in
  `UPSTREAM` and the version in `tests/upstream/tools/package.json`.
  After a change, run `npm install` there and
  `build-plugin-bundles.mjs`, and then the cases of
  `tests/upstream/cli-js` with a name that starts with `ts-`.
- **Cosmopolitan**: `COSMOCC_VERSION` and its SHA-256 in
  `scripts/build.sh` and in the workflow.
- **The method for Rust on Cosmopolitan** (`c_src/cosmo_rust.c`,
  `scripts/rust_link.sh`) and **the file events** (`c_src/watch.c`)
  come from BEAM.com: a fix there applies here.
