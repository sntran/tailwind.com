# tailwind

A Tailwind CSS 4 compiler in Rust: the compiler of
[tailwind.com](https://github.com/sntran/tailwind.com). It follows
Tailwind CSS 4.3.5 file by file, and it can compile as each older version
of Tailwind CSS 4, from 4.0.0. Its output is the output of the official
compiler of that version, byte for byte: the tests of the repository
compare the two.

tailwind.com is not a product of Tailwind Labs. Tailwind CSS is a
trademark of Tailwind Labs Inc.

## Use

```toml
[dependencies]
tailwind = "0.2"
```

Compile a stylesheet one time, then build the CSS of a list of
candidates (class names):

```rust
use tailwind::cli::resolve::load_stylesheet;
use tailwind::{compile, CompileOptions};

# fn main() -> tailwind::Result<()> {
// `@import "tailwindcss"` gets the stylesheets of Tailwind CSS that the
// crate has.
let loader = |id: &str, base: &str| load_stylesheet(id, base, None, &mut |_| {});
let options = CompileOptions { load_stylesheet: Some(&loader), ..CompileOptions::default() };
let mut compiler = compile("@import \"tailwindcss\";", options)?;

let css = compiler.build(&["flex".to_string(), "hover:bg-sky-500".to_string()]);
assert!(css.contains(".flex {"));
# Ok(())
# }
```

A compiler keeps its state: a second `build` with more candidates gives
the CSS of all the candidates, as the watch of Tailwind CSS does.

### The candidates of a project

`tailwind::scanner` is the scanner of Tailwind CSS (the crate
`tailwind-scanner`): it finds the candidates in the files of a
project, with the rules of `.gitignore`.

```rust
use tailwind::scanner::{PublicSourceEntry, Scanner};

let mut scanner = Scanner::new(vec![PublicSourceEntry {
    base: "src".to_string(),
    pattern: "**/*".to_string(),
    negated: false,
}]);
let candidates: Vec<String> = scanner.scan();
assert!(!candidates.is_empty());
```

### An older version of Tailwind CSS

`CompileOptions::version` selects the version. Each behavior that changed
in a release of Tailwind CSS is a switch (`tailwind::switches`):

```rust
use tailwind::{compile, CompileOptions, Version};

# fn main() -> tailwind::Result<()> {
let options = CompileOptions { version: Some(Version(4, 0, 0)), ..CompileOptions::default() };
let mut compiler = compile("@theme { --color-brand: #0088cc; }\n@tailwind utilities;", options)?;
let css = compiler.build(&["bg-brand".to_string()]);
assert!(css.contains("background-color: var(--color-brand);"));
# Ok(())
# }
```

For an older version, `@import "tailwindcss"` needs the stylesheets of
that version (`theme.css`, `preflight.css`, `utilities.css`, `index.css`
of the npm package `tailwindcss`): give them with your own
`load_stylesheet`. The stylesheets in the crate are the stylesheets of
4.3.5.

## Features

| Feature | Default | What |
|---|---|---|
| `cli` | yes | The command line program `tailwind`, the scanner, the stylesheets of Tailwind CSS, and Lightning CSS for `--minify` and `--optimize`. |
| `js` | with `cli` | `@plugin` and `@config`: QuickJS (in the crate, compiled with the crate `cc`) and the plugin layer of Tailwind CSS. For `wasm32-wasip1`, it needs a wasi-sdk in `WASI_SDK_PATH`. |
| `ape` | no | The build of the file `tailwind.com` with Cosmopolitan (`scripts/build.sh` of the repository). |

With `default-features = false`, the crate is the compiler only.

## Licenses

The crate has the Apache License 2.0. It is a port of the compiler of
Tailwind CSS (MIT License), and it has QuickJS (MIT License) and the
stylesheets and the plugin layer of Tailwind CSS. The file `NOTICE` of the
repository lists them, with their license texts in `licenses/`.
