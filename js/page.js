// The default file of the npm package tailwind.com on a CDN (the field
// "jsdelivr" and "unpkg" of package.json). It builds the CSS of the page
// in the browser, as the browser build of Tailwind CSS (the Play CDN) does.
//
// The input CSS can be a file, as for a build on the disk:
//
//   <link rel="stylesheet" type="text/tailwindcss" href="/assets/app.css">
//   <script src="https://cdn.jsdelivr.net/npm/tailwind.com@0.4"></script>
//
// or a <style>, as for the Play CDN:
//
//   <script src="https://cdn.jsdelivr.net/npm/tailwind.com@0.4"></script>
//   <style type="text/tailwindcss">
//     @theme { --color-brand: #0d9488; }
//   </style>
//
// - A <link rel="stylesheet" type="text/tailwindcss">: the browser does
//   not apply a stylesheet of this type. The script gets the file, and the
//   files of its @import, @reference, @plugin and @config with a path
//   ("./", "../" or "/"). The files go in memory at the path of their URL,
//   so the paths in the CSS find them as on the disk. A change of the
//   attribute href (a live reload) gets the files again.
// - A <style type="text/tailwindcss">: its text. With no @import in the
//   input, `@import "tailwindcss";` comes first.
// - The classes are the classes of each element of the page. A class that
//   goes from the page stays in the CSS, as in the browser build.
// - The output CSS goes in a <style> after the first such <link>, so it
//   has the place of the file in the cascade. With no <link>, it goes at
//   the end of <head>, as in the browser build.
// - A change of a class or of an element builds again. A change of a
//   <style type="text/tailwindcss"> builds again too.
// - After each build, the document gets the event "tailwind.com:build".
//
// To build on the disk later, remove the script and the attribute type,
// and set href to the output of the build.
//
// It is the same program as the command line: the plugins in the program
// (@tailwindcss/typography, @tailwindcss/forms and
// @tailwindcss/aspect-ratio) work with @plugin. The program is large (about
// 2 MB with compression), so this script is for development, prototypes
// and editors, not for a production site.
//
// A classic script, not a module: a <script src> with no type loads it,
// as the Play CDN. It loads js/browser.mjs with import().
(() => {
  const TYPE = 'text/tailwindcss';
  const LINKS = `link[rel~="stylesheet"][type="${TYPE}"][href]`;
  const STYLES = `style[type="${TYPE}"]`;
  // The file of the classes of the page, in the root of the project.
  const CLASSES = 'tailwind.com.classes.html';
  // The references to other files in a CSS file.
  const REFERENCE = /@(?:import|reference|plugin|config)\s+(?:url\(\s*)?(["'])(.*?)\1/g;

  const script = document.currentScript;
  const source = new URL(script?.src || location.href);
  // The package root of a CDN URL ("/npm/tailwind.com@0.4", with no file),
  // else the directory js/ of this file.
  const found = source.pathname.match(/^(.*\/tailwind\.com(?:@[^/]*)?)(?:\/|$)/);
  const js = found ? new URL(`${found[1]}/js/`, source) : new URL('./', source);

  const output = document.createElement('style');
  const classes = new Set();
  // The files of the links: { key, paths, files }.
  let linked = { key: null, paths: [], files: {} };
  let input = null;
  let api = null;
  let queue = Promise.resolve();
  let pending = false;

  // The path of a URL in the project: the path of the URL.
  const pathOf = (url) => decodeURIComponent(url.pathname);
  const quote = (text) => `"${text.replace(/[\\"]/g, '\\$&')}"`;

  // Get a file and, for CSS, the files of its references with a path.
  async function load(url, files) {
    const path = pathOf(url);
    if (path.slice(1) in files) return;
    files[path.slice(1)] = '';
    const response = await fetch(url, { cache: 'no-cache' });
    if (!response.ok) throw new Error(`${url.href}: ${response.status} ${response.statusText}`);
    const text = await response.text();
    files[path.slice(1)] = text;
    if (!/\.css$/i.test(path)) return;
    const loads = [];
    for (const [, , target] of text.matchAll(REFERENCE)) {
      if (/^(\.{1,2})?\//.test(target)) loads.push(load(new URL(target, url), files));
    }
    await Promise.all(loads);
  }

  // The files of the links of the page. They load again when the list of
  // the URLs changes.
  async function links() {
    const urls = [...document.querySelectorAll(LINKS)].map((link) => new URL(link.getAttribute('href'), document.baseURI));
    const key = urls.map((url) => url.href).join('\n');
    if (key !== linked.key) {
      const files = {};
      await Promise.all(urls.map((url) => load(url, files)));
      linked = { key, paths: urls.map(pathOf), files };
    }
    return linked;
  }

  // The input CSS: an @import of each link, then the text of each <style>.
  function stylesheet(paths) {
    let css = paths.map((path) => `@import ${quote(`.${path}`)};\n`).join('');
    for (const style of document.querySelectorAll(STYLES)) {
      watchStyle(style);
      css += `${style.textContent}\n`;
    }
    return css.includes('@import') ? css : `@import "tailwindcss";\n${css}`;
  }

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
    api ??= await import(new URL('browser.mjs', js).href);
    const { key, paths, files } = await links();
    const css = stylesheet(paths);
    const added = collect();
    place();
    if (`${key}\n${css}` === input && !added) return;
    input = `${key}\n${css}`;
    // The classes go in a file of the project. The scanner finds them
    // there, as in a template of a project on the disk.
    const result = await api.runWasm(['-i', '-'], {
      input: `${css}@source ${quote(`./${CLASSES}`)};\n`,
      files: { ...files, [CLASSES]: [...classes].join('\n') },
      env: { NO_COLOR: '1' },
    });
    // The program writes its version and its time to stderr. Show only the
    // other lines: the warnings and the errors.
    for (const line of result.stderr.split('\n')) {
      if (line.trim() && !line.startsWith('≈ tailwindcss') && !/^Done in /.test(line)) {
        (result.status === 0 ? console.warn : console.error)(`tailwind.com: ${line}`);
      }
    }
    // The CSS of the browser build has no newline at the end.
    if (result.status === 0) output.textContent = result.stdout.replace(/\n$/, '');
    document.dispatchEvent(new CustomEvent('tailwind.com:build', { detail: { status: result.status } }));
  }

  // One build at a time. The changes while a build runs give one more
  // build.
  function schedule() {
    if (pending) return;
    pending = true;
    queue = queue.then(() => {
      pending = false;
      return build().catch((error) => {
        console.error('tailwind.com:', error);
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
        if (node !== output && node.nodeType === Node.ELEMENT_NODE) return schedule();
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

  schedule();
})();
