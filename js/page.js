// The default file of the npm package tailwind.com on a CDN (the field
// "jsdelivr" and "unpkg" of package.json). It builds the CSS of the page
// in the browser, as the browser build of Tailwind CSS (the Play CDN) does.
//
// The input CSS can be a file, as for a build on the disk:
//
//   <link rel="stylesheet" type="text/tailwindcss" href="/assets/app.css">
//   <script src="https://cdn.jsdelivr.net/npm/tailwind.com@0.5"></script>
//
// or a <style>, as for the Play CDN:
//
//   <script src="https://cdn.jsdelivr.net/npm/tailwind.com@0.5"></script>
//   <style type="text/tailwindcss">
//     @theme { --color-brand: #0d9488; }
//   </style>
//
// - A <link rel="stylesheet" type="text/tailwindcss">: the browser does
//   not apply a stylesheet of this type. The script gets the file, and the
//   files of its @import, @reference, @plugin and @config with a path
//   ("./", "../" or "/"), and the files that a JavaScript or TypeScript
//   file of a plugin imports with a path. A path with no extension tries
//   the extensions, as Node.js does. The files go in memory at the path of
//   their URL, so the paths in the CSS find them as on the disk.
// - A <style type="text/tailwindcss">: its text, as a file in the
//   directory of the page. Its paths find the files from the URL of the
//   page. With no <link> and no @import, `@import "tailwindcss";` comes
//   first.
// - Each second, while the page is visible, the script gets the files of
//   this site again (a server with ETag or Last-Modified answers 304), and
//   builds again when one changes. A change of href gets the new files.
// - The classes are the classes of each element of the page. A class that
//   goes from the page stays in the CSS, as in the browser build.
// - The output CSS goes in a <style> after the first such <link>, so it
//   has the place of the file in the cascade. With no <link>, it goes at
//   the end of <head>, as in the browser build.
// - A change of a class or of an element builds again. A change of a
//   <style type="text/tailwindcss"> builds again too.
// - After each build, the document gets the event "tailwind.com:build".
// - The first build waits for the end of the HTML, so that it has all the
//   classes. The downloads start at once.
// - The program has no JavaScript engine: for @plugin and @config, the
//   plugins run in the JavaScript engine of the browser (js/engine.mjs),
//   with JS Promise Integration.
// - The new CSS of the input (@function, @mixin, if(), @when,
//   sibling-index(), round(), ...) is lowered for the browsers of Tailwind
//   CSS, as with --optimize of the command line, with no Lightning CSS.
// - The last CSS of each page stays in localStorage. On the next load, it
//   styles the page at once, and the build replaces it. With no such CSS
//   and the script in <head>, the page stays hidden until the first CSS
//   (at most 3 seconds), so it does not show with no style.
//
// To build on the disk later, remove the script and the attribute type,
// and set href to the output of the build.
//
// It is the same program as the command line: the plugins in the program
// (@tailwindcss/typography, @tailwindcss/forms and
// @tailwindcss/aspect-ratio) work with @plugin. The program
// (tailwind-browser.wasm) is about 0.8 MB with compression, so this script
// is for development, prototypes and editors, not for a production site.
//
// A classic script, not a module: a <script src> with no type loads it,
// as the Play CDN. It loads js/browser.mjs with import().
(() => {
  const TYPE = 'text/tailwindcss';
  const LINKS = `link[rel~="stylesheet"][type="${TYPE}"][href]`;
  const STYLES = `style[type="${TYPE}"]`;
  // The file of the classes of the page, in the root of the project.
  const CLASSES = 'tailwind.com.classes.html';
  // The references to other files in a CSS file, and in a JavaScript file.
  const CSS_REFERENCE = /@(?:import|reference|plugin|config)\s+(?:url\(\s*)?(["'])(.*?)\1/g;
  const JS_REFERENCE = /(?:\bfrom|\bimport|\brequire\s*\(|\bimport\s*\()\s*(["'])((?:\.{1,2})?\/[^"']*)\1/g;
  const JS = /\.[cm]?[jt]s$/i;
  // The files that a path with no extension can be, as Node.js resolves
  // it: a CSS reference, then a reference of JavaScript.
  const CSS_FILES = ['.css', '/index.css'];
  const JS_FILES = ['.js', '.cjs', '.mjs', '.ts', '.cts', '.mts', '/index.js', '/index.cjs', '/index.mjs', '/index.ts'];
  const POLL_MS = 1000;
  // The longest time that the page stays hidden.
  const HIDE_MS = 3000;

  const script = document.currentScript;
  const source = new URL(script?.src || location.href);
  // The package root of a CDN URL ("/npm/tailwind.com@0.5", with no file),
  // else the directory js/ of this file.
  const found = source.pathname.match(/^(.*\/tailwind\.com(?:@[^/]*)?)(?:\/|$)/);
  const js = found ? new URL(`${found[1]}/js/`, source) : new URL('./', source);

  // The program for a browser: no optimizer and no JavaScript engine.
  const wasm = new URL('../runtime/tailwind-browser.wasm', js).href;
  const head = document.head ?? document.documentElement;

  // Start the downloads now: the module of the API, its import, and the
  // program. Else each one starts only when the one before it ends.
  function preload(rel, href, as) {
    const link = document.createElement('link');
    link.rel = rel;
    link.href = href;
    if (as) link.as = as;
    link.crossOrigin = 'anonymous';
    head.append(link);
  }
  preload('modulepreload', new URL('wasi.mjs', js).href);
  preload('modulepreload', new URL('engine.mjs', js).href);
  preload('preload', wasm, 'fetch');
  const loading = import(new URL('browser.mjs', js).href);
  loading.catch(() => {});

  // The CSS of the last build of this page, at once.
  const output = document.createElement('style');
  const KEY = `tailwind.com:${location.pathname}`;
  let cached = null;
  try {
    cached = localStorage.getItem(KEY);
  } catch {}
  if (cached !== null) output.textContent = cached;
  head.append(output);

  // With no such CSS, hide the page until the first CSS. A script at the
  // end of <body> comes after the first paint: then no hide.
  const hide = document.createElement('style');
  if (cached === null && !document.body) {
    hide.textContent = 'html { visibility: hidden !important; }';
    head.append(hide);
    setTimeout(show, HIDE_MS);
  }
  function show() {
    hide.remove();
  }

  const classes = new Set();
  // The text of each file that the script got, by URL.
  const texts = new Map();
  // The URL of the file of a reference with no extension, by the URL of
  // the reference.
  const resolved = new Map();
  // The URLs of the files of the last build.
  let used = new Set();
  // One more for each change of a file.
  let version = 0;
  let input = null;
  let api = null;
  let queue = Promise.resolve();
  let pending = false;
  // The first build waits for the end of the HTML.
  let ready = document.readyState !== 'loading';

  // The path of a URL in the project: the path of the URL.
  const pathOf = (url) => decodeURIComponent(new URL(url).pathname);
  const quote = (text) => `"${text.replace(/[\\"]/g, '\\$&')}"`;

  async function get(url) {
    const response = await fetch(url, { cache: 'no-cache' });
    // A server for a single page can give its HTML for each path.
    if (!response.ok || response.headers.get('content-type')?.startsWith('text/html')) return null;
    return response.text();
  }

  // The URL of the file of a reference: the URL itself when it has an
  // extension, else the first file that is there, as Node.js resolves it.
  async function resolve(url, kind) {
    const path = new URL(url).pathname;
    if (/\.[^./]+$/.test(path)) return url;
    if (!resolved.has(url)) {
      for (const suffix of kind === 'css' ? CSS_FILES : JS_FILES) {
        const candidate = new URL(url);
        candidate.pathname = path.replace(/\/$/, '') + suffix;
        const text = await get(candidate.href);
        if (text !== null) {
          texts.set(candidate.href, text);
          resolved.set(url, candidate.href);
          break;
        }
      }
    }
    if (!resolved.has(url)) throw new Error(`${url}: no such file`);
    return resolved.get(url);
  }

  // Put a file and the files of its references in `files`.
  async function load(url, kind, files) {
    url = await resolve(url, kind);
    const path = pathOf(url).slice(1);
    if (path in files) return;
    files[path] = '';
    used.add(url);
    if (!texts.has(url)) {
      const text = await get(url);
      if (text === null) throw new Error(`${url}: no such file`);
      texts.set(url, text);
    }
    files[path] = texts.get(url);
    await references(files[path], url, path, files);
  }

  // Load the files of the references of a text with a path.
  async function references(text, url, path, files) {
    const loads = [];
    if (/\.css$/i.test(path)) {
      for (const [whole, , target] of text.matchAll(CSS_REFERENCE)) {
        const kind = /^@(?:import|reference)/.test(whole) ? 'css' : 'js';
        if (/^(\.{1,2})?\//.test(target)) loads.push(load(new URL(target, url).href, kind, files));
      }
    } else if (JS.test(path)) {
      for (const [, , target] of text.matchAll(JS_REFERENCE)) loads.push(load(new URL(target, url).href, 'js', files));
    }
    await Promise.all(loads);
  }

  // The files of the project and the input CSS: an @import of each link,
  // then of the text of each <style>, as a file in the directory of the
  // page.
  async function project() {
    used = new Set();
    const files = {};
    const imports = [];
    for (const link of document.querySelectorAll(LINKS)) {
      const url = new URL(link.getAttribute('href'), document.baseURI).href;
      await load(url, 'css', files);
      imports.push(pathOf(await resolve(url, 'css')));
    }
    let text = '';
    for (const [i, style] of [...document.querySelectorAll(STYLES)].entries()) {
      watchStyle(style);
      const url = new URL(`tailwind.com.style.${i}.css`, document.baseURI).href;
      const path = pathOf(url);
      files[path.slice(1)] = style.textContent;
      text += style.textContent;
      await references(style.textContent, url, path, files);
      imports.push(path);
    }
    let css = imports.map((path) => `@import ${quote(`.${path}`)};\n`).join('');
    if (!document.querySelector(LINKS) && !text.includes('@import')) css = `@import "tailwindcss";\n${css}`;
    // Forget the files that the page does not use now.
    for (const url of texts.keys()) if (!used.has(url)) texts.delete(url);
    return { css, files };
  }

  // Get the files of this site again. A change builds again.
  async function poll() {
    if (document.visibilityState === 'visible') {
      for (const url of [...texts.keys()]) {
        if (new URL(url).origin !== location.origin) continue;
        const text = await get(url).catch(() => null);
        if (text !== null && texts.has(url) && text !== texts.get(url)) {
          texts.set(url, text);
          version++;
        }
      }
      if (version !== polled) {
        polled = version;
        schedule();
      }
    }
    setTimeout(poll, POLL_MS);
  }
  let polled = 0;

  // Add the classes of the page. The result is true when a class is new.
  function collect() {
    let added = false;
    for (const element of document.querySelectorAll('[class]')) {
      for (const name of element.classList) {
        if (!classes.has(name)) {
          classes.add(name);
          added = true;
        }
      }
    }
    return added;
  }

  // The place of the output: after the first link, else at the end of
  // <head>.
  function place() {
    const anchor = document.querySelector(LINKS);
    if (anchor) {
      if (anchor.nextSibling !== output) anchor.after(output);
    } else if (output.parentNode !== document.head) {
      document.head.append(output);
    }
  }

  async function build() {
    api ??= await loading;
    const { css, files } = await project();
    const added = collect();
    place();
    const key = `${version}\n${css}\n${JSON.stringify(files)}`;
    if (key === input && !added) return;
    input = key;
    // The classes go in a file of the project. The scanner finds them
    // there, as in a template of a project on the disk.
    // --optimize: the program for a browser lowers the new CSS (functions,
    // mixins, if(), @when, the sibling functions, the math functions) for
    // the browsers of Tailwind CSS. With nothing to lower, the CSS does not
    // change.
    const result = await api.runWasm(['-i', '-', '--optimize'], {
      input: `${css}@source ${quote(`./${CLASSES}`)};\n`,
      files: { ...files, [CLASSES]: [...classes].join('\n') },
      env: { NO_COLOR: '1' },
      wasm,
    });
    // The program writes its version and its time to stderr. Show only the
    // other lines: the warnings and the errors.
    for (const line of result.stderr.split('\n')) {
      if (line.trim() && !line.startsWith('≈ tailwindcss') && !/^Done in /.test(line)) {
        (result.status === 0 ? console.warn : console.error)(`tailwind.com: ${line}`);
      }
    }
    // The CSS of the browser build has no newline at the end.
    if (result.status === 0) {
      const css = result.stdout.replace(/\n$/, '');
      output.textContent = css;
      try {
        localStorage.setItem(KEY, css);
      } catch {}
    }
    show();
    document.dispatchEvent(new CustomEvent('tailwind.com:build', { detail: { status: result.status } }));
  }

  // One build at a time. The changes while a build runs give one more
  // build.
  function schedule() {
    if (!ready || pending) return;
    pending = true;
    queue = queue.then(() => {
      pending = false;
      return build().catch((error) => {
        console.error('tailwind.com:', error);
        show();
        document.dispatchEvent(new CustomEvent('tailwind.com:build', { detail: { status: -1 } }));
      });
    });
  }

  const styles = new MutationObserver(schedule);
  const watched = new WeakSet();
  function watchStyle(style) {
    if (watched.has(style)) return;
    watched.add(style);
    styles.observe(style, { attributes: true, attributeFilter: ['type'], characterData: true, subtree: true, childList: true });
  }

  new MutationObserver((records) => {
    for (const record of records) {
      if (record.type === 'attributes') return schedule();
      for (const node of record.addedNodes) {
        // The realm of the JavaScript engine is a hidden <iframe>.
        if (node !== output && node.nodeType === Node.ELEMENT_NODE && !node.hasAttribute('data-tailwind-com')) return schedule();
      }
      for (const node of record.removedNodes) {
        if (node.nodeType === Node.ELEMENT_NODE && node.matches(`${LINKS}, ${STYLES}`)) return schedule();
      }
    }
  }).observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['class', 'href', 'rel', 'type'],
    childList: true,
    subtree: true,
  });

  if (ready) schedule();
  else {
    document.addEventListener(
      'DOMContentLoaded',
      () => {
        ready = true;
        schedule();
      },
      { once: true },
    );
  }
  setTimeout(poll, POLL_MS);
})();
