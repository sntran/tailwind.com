# The scanner of Tailwind CSS

These are the three crates of the scanner of Tailwind CSS: the code that
finds the candidates (the class names) in the source files of a project.
The program uses it for `@source` and for the automatic source detection.

| Directory | From Tailwind CSS | Our crate | Name in the code |
|---|---|---|---|
| `oxide` | `crates/oxide` | `tailwind-com-scanner` | `tailwindcss_oxide` |
| `classification-macros` | `crates/classification-macros` | `tailwind-com-classification-macros` | `classification_macros` |
| `ignore` | `crates/ignore` (a fork of the crate `ignore` of ripgrep) | `tailwind-com-ignore` | `ignore` |

Each file is a copy of the tag of the version in `UPSTREAM`, with no
change, except the three `Cargo.toml` files. Those are ours: crates.io
needs a version for each dependency, and does not take a dependency on a
git repository. Tailwind CSS does not publish these crates. The names of
our crates start with `tailwind-com-`, and each crate keeps the name of
its library, so the code needs no change.

The manifests of Tailwind CSS are in `manifests/`. A difference between
two versions there (a new dependency, a new version of a dependency)
needs the same change in our manifest, by hand. One change is ours:
`classification-macros` asks for the feature `full` of `syn`. Its code
uses that feature, and in the workspace of Tailwind CSS another crate
turns it on; a crate of crates.io builds alone.

`sh scripts/scanner.sh UPSTREAM_CLONE` makes the copy again from a clone
of Tailwind CSS. The job "test" of `.github/workflows/build.yml` runs it
and fails when the copy is not the same as the tag. `sh
scripts/sync-upstream.sh apply VERSION` runs it for a new version.

The licenses: `oxide` and `classification-macros` have the MIT license of
Tailwind CSS (`LICENSE` in each directory). `ignore` has the licenses of
ripgrep (`COPYING`, `LICENSE-MIT`, `UNLICENSE`).

## Tests

The tests of the three crates are the tests of Tailwind CSS. They do not
run with `cargo test` at the root of the repository, which tests only the
compiler (`default-members`). Run them with:

```sh
cargo test --release -p tailwind-com-scanner -p tailwind-com-ignore -p tailwind-com-classification-macros
```

`cargo publish --dry-run` with the same three `-p` options checks that
crates.io takes them. CI runs the two.
