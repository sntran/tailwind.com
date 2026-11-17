// The default file of the npm package tailwind.com on a CDN (the field
// "jsdelivr" and "unpkg" of package.json). It builds the CSS of the page
// in the browser, as the browser build of Tailwind CSS (the Play CDN) does:
//
//   <script src="https://cdn.jsdelivr.net/npm/tailwind.com@0.4"></script>
//   <style type="text/tailwindcss">
//     @theme { --color-brand: #0d9488; }
//   </style>
//
// - The input CSS is the text of each <style type="text/tailwindcss">.
//   With no @import, `@import "tailwindcss";` comes first.
// - The classes are the classes of each element of the page. A class that
//   goes from the page stays in the CSS, as in the browser build.
// - The output CSS goes in a <style> at the end of <head>.
// - A change of a class or of an element builds again. A change of a
//   <style type="text/tailwindcss"> builds again too.
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
  const script = document.currentScript;
  const source = new URL(script?.src || location.href);
  // The package root of a CDN URL ("/npm/tailwind.com@0.4", with no file),
  // else the directory js/ of this file.
  const found = source.pathname.match(/^(.*\/tailwind\.com(?:@[^/]*)?)(?:\/|$)/);
  const js = found ? new URL(`${found[1]}/js/`, source) : new URL('./', source);

  const output = document.createElement('style');
  const classes = new Set();
  let input = null;
  let api = null;
  let queue = Promise.resolve();
  let pending = false;

  // The text of each <style type="text/tailwindcss">.
  function stylesheet() {
    let css = '';
    for (const style of document.querySelectorAll(`style[type="${TYPE}"]`)) {
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

  async function build() {
    api ??= await import(new URL('browser.mjs', js).href);
    const css = stylesheet();
    const added = collect();
    if (css === input && !added) return;
    input = css;
    // The classes go in a file of the project. The scanner finds them
    // there, as in a template of a project on the disk.
    const result = await api.runWasm(['-i', '-'], {
      input: css,
      files: { 'classes.html': [...classes].join('\n') },
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
      return build().catch((error) => console.error('tailwind.com:', error));
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
    }
  }).observe(document.documentElement, { attributes: true, attributeFilter: ['class'], childList: true, subtree: true });

  schedule();
  document.head.append(output);
})();
