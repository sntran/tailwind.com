# The site

The site of tailwind.com is one page on GitHub Pages:
<https://sntran.github.io/tailwind.com/>. It runs `tailwind.wasm` in the
browser. A viewer writes markup and CSS, selects a version of Tailwind CSS
from 4.0.0 to the newest, and sees the output CSS and a preview. The page
can also compare:

- **Two versions.** The two outputs, the difference between them line by
  line, and the two previews.
- **The Rust compiler and the official compiler of one version.** The
  file has the two engines ([`VERSIONS.md`](VERSIONS.md)). The page runs
  both, and the difference must be empty.

The page also lists the switches of each release
([`SWITCHES.md`](SWITCHES.md)) with their recorded cases
(`tests/upstream/switches.jsonl`). A case opens with the version before
the change and the version of the change.

The address of the page holds its state: the markup, the CSS, the
versions and the mode. "Copy a link to this page" gives a link that
opens the same state. Nothing goes to a server: the page has no server
code.

The documents stay in Markdown in the repository. The page links to them
on GitHub.

## The files

| File | What |
|---|---|
| `site/index.html`, `site/app.mjs` | The page: the controls, the output, the preview, the list of the changes. |
| `site/style.css` | The CSS of the page. `site/build.mjs` builds it with `tailwind.wasm`. |
| `site/worker.mjs` | A Web Worker that runs `tailwind.wasm`, so that a build does not stop the page. |
| `site/wasi.mjs` | The WASI host (preview 1) with a file system in memory. It has the 22 functions that `tailwind.wasm` imports, and no dependency. It runs in a browser and in Node.js. |
| `site/diff.mjs` | The difference of two texts, line by line (the algorithm of Myers). |
| `site/examples.mjs` | The starter examples. |
| `site/build.mjs` | It writes the site into `build/site`. |

The site has these files:

| File | Size | What |
|---|---:|---|
| `tailwind.wasm` | 9.4 MB, 2.5 MB with gzip | The program. |
| `packages.bin` | 0.95 MB | The packages of the 48 versions: 4.3.5 in full, and each older version as a patch, as in the ZIP directory of `tailwind.com`. |
| `data.json` | 0.16 MB | The versions, the examples, the 195 switches and the recorded cases. |
| The page | 0.06 MB | `index.html`, the modules and `style.css`. |

## How a build runs

Each build is one run of the program, as on the command line:

```
tailwind -i app.css -o out.css [--minify]
```

The worker writes the markup to `index.html` and the CSS to `app.css` in
a directory in memory. Under WASI preview 1, the working directory of a
program is always `/`, so that directory is the root `/` of the program.
The packages are a second directory, `/packages`, that the scanner does
not see. `TAILWIND_COM_PACKAGES=/packages` gives them to the program, and
`TAILWINDCSS_VERSION` and `TAILWIND_COM_ENGINE` select the version and
the engine.

The worker compiles the module one time. Each build makes a new instance
of it. A build with no plugin takes 5 ms to 15 ms in Chromium, and a build
with the typography and forms plugins takes about 170 ms.

The plugins in the program work: `@tailwindcss/typography`,
`@tailwindcss/forms` and `@tailwindcss/aspect-ratio`. Other npm packages
are not there.

### The recorded cases on the page

`site/build.mjs` runs each recorded case on the path of the page, in
Node.js with the same host. That is one element for each candidate in
the markup, and the CSS in `app.css`. A case is on the page only when
the two versions give a different output there. Some cases show their
change only with the compiler API: a list of candidates in a given
sequence, or a CRLF, which a text area changes to LF. For these cases,
the official compiler also gives the same output on the path of the
page. Now 286 of the 301 cases are on the page.

## Build the site

```sh
WASI_SDK_PATH=... make wasm                  # build/tailwind.wasm
node site/build.mjs --wasm build/tailwind.wasm
```

With no `--packages DIR`, the script gets the packages with
`scripts/packages.sh` and makes the patches with `tools/twpatch` in
`build/site-packages`, as `scripts/build.sh` does.

To see the page, serve `build/site` with a web server; a page from
`file://` cannot load a Web Worker module. For example:

```sh
python3 -m http.server -d build/site 8000
```

## The tests

| Test | What |
|---|---|
| `tests/site/browser.mjs` | The WASI host in Chromium against `node:wasi` on a real directory: 13 scenarios (`tests/site/scenarios.mjs`), with the two engines, 4.0.0, 4.1.0 and 4.3.5, `--minify`, plugins and an error. The results must be the same. |
| `tests/site/page.mjs` | The page in Chromium. Each example and each recorded case must build. The Rust compiler and the official compiler must give the same output for each of the 48 versions. Each case on the page must show a difference. |

The two tests need Playwright with Chromium. The job `site` of
`.github/workflows/build.yml` runs them for each push, and the artifact
`site` has the site and a picture of the page.

## The deploy

`.github/workflows/pages.yml` builds the site with `tailwind.wasm` of a
release (from the files of that release, with their SHA-256), runs
`tests/site/page.mjs`, and deploys `build/site` to GitHub Pages. The job
`pages` of `build.yml` calls it after each release. A run by hand deploys
the release of its input `tag`, or the latest release. The first release
with the site is 0.2.0: an older tag has no `site/`.

The repository needs Pages with the source "GitHub Actions": Settings,
Pages, "Build and deployment".
