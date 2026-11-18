// The API of the npm package tailwind.com in a browser: the condition
// "browser" of package.json gives this file in place of js/node.mjs. It
// runs the program for a browser, tailwind-browser.wasm, with a file
// system in memory (js/wasi.mjs).
//
//   import { runWasm } from 'tailwind.com';
//
//   const { status, stdout, stderr } = await runWasm(['-i', 'app.css'], {
//     files: { 'app.css': '@import "tailwindcss";', 'index.html': '<div class="flex p-4">' },
//   });
//
// tailwind-browser.wasm has no Lightning CSS and no JavaScript engine:
// --optimize only lowers the new CSS (functions, mixins, if(), @when, the
// sibling functions, the math functions), --minify gives an error, and the
// plugins run in the JavaScript engine of the browser (js/engine.mjs). For
// --minify, give the URL of tailwind.wasm in options.wasm (about 1.8 MB
// with compression).
//
// It also runs in a Web Worker, and in Deno and Node.js with no file
// system. Vite and webpack copy tailwind-browser.wasm and the JavaScript
// files of runtime/ with the files of the site: each one has the pattern
// `new URL('../runtime/NAME', import.meta.url)`. For a bundler that does
// not copy them (esbuild), copy runtime/ of the package to the site, and
// give its URL in options.runtime.
import { createEngine, hasPromiseIntegration, needsEngine } from './engine.mjs';
import { Files, run } from './wasi.mjs';

// The package root in the URL of a file of the package on a CDN:
// "https://cdn.jsdelivr.net/npm/tailwind.com@0.5.0/+esm",
// "https://esm.sh/tailwind.com@0.5.0/es2022/tailwind.com.mjs", or only the
// root for the default file, "https://cdn.jsdelivr.net/npm/tailwind.com@0.5".
function packageRoot(from) {
  const url = new URL(from);
  const found = url.pathname.match(/^(.*\/tailwind\.com(?:@[^/]*)?)(?:\/|$)/);
  return found ? new URL(`${found[1]}/`, url).href : null;
}

// The files of runtime/ when this file is not on a CDN. A bundler finds
// each pattern `new URL('…', import.meta.url)`: the text of each one must
// stay as it is.
const LOCAL = {
  'tailwind-browser.wasm': () => new URL('../runtime/tailwind-browser.wasm', import.meta.url),
  'compat.js': () => new URL('../runtime/compat.js', import.meta.url),
  'typescript.js': () => new URL('../runtime/typescript.js', import.meta.url),
  'plugins/forms.js': () => new URL('../runtime/plugins/forms.js', import.meta.url),
  'plugins/typography.js': () => new URL('../runtime/plugins/typography.js', import.meta.url),
  'plugins/aspect-ratio.js': () => new URL('../runtime/plugins/aspect-ratio.js', import.meta.url),
};

// The URL of a file of runtime/: in the directory `runtime`, else from the
// package root of a CDN, else the copy of a bundler.
function runtimeUrl(name, runtime) {
  if (runtime) return new URL(name, new URL(String(runtime).replace(/\/?$/, '/'), globalThis.location?.href ?? import.meta.url)).href;
  const root = packageRoot(import.meta.url);
  return root ? new URL(`runtime/${name}`, root).href : LOCAL[name]().href;
}

// The URL of tailwind-browser.wasm, from the URL of a file of the package
// (default: this file).
export function wasmUrl(from = import.meta.url) {
  if (from === import.meta.url) return runtimeUrl('tailwind-browser.wasm');
  const root = packageRoot(from);
  return root ? new URL('runtime/tailwind-browser.wasm', root).href : new URL('../runtime/tailwind-browser.wasm', from).href;
}

// The compiled module of each URL. A failed download can be tried again.
const modules = new Map();

function compile(url) {
  if (!modules.has(url)) {
    const compiled = fetch(url).then(async (response) => {
      if (!response.ok) throw new Error(`${url}: ${response.status} ${response.statusText}`);
      // A server that does not send "application/wasm" cannot stream.
      if (response.headers.get('content-type')?.startsWith('application/wasm')) {
        return WebAssembly.compileStreaming(response);
      }
      return WebAssembly.compile(await response.arrayBuffer());
    });
    compiled.catch(() => modules.delete(url));
    modules.set(url, compiled);
  }
  return modules.get(url);
}

// The JavaScript files of the program, by URL.
const texts = new Map();

function text(url) {
  if (!texts.has(url)) {
    const loading = fetch(url).then((response) => {
      if (!response.ok) throw new Error(`${url}: ${response.status} ${response.statusText}`);
      return response.text();
    });
    loading.catch(() => texts.delete(url));
    texts.set(url, loading);
  }
  return texts.get(url);
}

// The plugins in the program, as in the standalone program of Tailwind CSS.
const PLUGINS = ['@tailwindcss/forms', '@tailwindcss/typography', '@tailwindcss/aspect-ratio'];

// The JavaScript files that a build of `input` and `files` needs: none
// with no @plugin and no @config.
async function assets(runtime, input, files) {
  const all = [input, ...Object.values(files).filter((value) => typeof value === 'string')].join('\n');
  if (!/@(?:plugin|config)\b/.test(all)) return {};
  const names = { compat: 'compat.js' };
  // An ES module or a TypeScript file becomes CommonJS with Sucrase.
  if (Object.keys(files).some((name) => /\.[cm]?[jt]s$/.test(name))) names.typescript = 'typescript.js';
  for (const plugin of PLUGINS) {
    if (all.includes(plugin)) names[`plugin:${plugin}`] = `plugins/${plugin.slice('@tailwindcss/'.length)}.js`;
  }
  const entries = await Promise.all(Object.entries(names).map(async ([name, file]) => [name, await text(runtimeUrl(file, runtime))]));
  return Object.fromEntries(entries);
}

// Run the program with the arguments of the command line program, as
// runWasm of js/node.mjs. The files are in memory: options.files is an
// object { path: text or Uint8Array }, with paths from the root "/" of the
// program. The program sees no other file.
//
// options: files, env (default: empty), input (the text of stdin), wasm
// (the URL of the program, default: wasmUrl(), tailwind-browser.wasm),
// runtime (the URL of a copy of runtime/ of the package: the default
// program and the JavaScript files come from there).
//
// tailwind-browser.wasm has no JavaScript engine: for @plugin and
// @config, it runs the plugins in the JavaScript engine of the browser
// (js/engine.mjs). That needs a page (not a worker) and JS Promise
// Integration. The JavaScript files come from runtime/ of the package.
// The result is { status, stdout, stderr } with the output as text. With
// no -o, the CSS is in stdout. --watch does not end: do not give it.
export async function runWasm(args = [], options = {}) {
  // A relative URL is relative to the page.
  const url = new URL(String(options.wasm ?? (options.runtime ? runtimeUrl('tailwind-browser.wasm', options.runtime) : wasmUrl())), globalThis.location?.href ?? import.meta.url).href;
  // The JavaScript files download while the program compiles.
  const loading = assets(options.runtime, options.input ?? '', options.files ?? {});
  loading.catch(() => {});
  const module = await compile(url);
  let imports = {};
  let promising = false;
  let engine = null;
  if (needsEngine(module)) {
    const found = await loading;
    if (found.compat !== undefined) {
      if (!hasPromiseIntegration) {
        throw new Error('tailwind.com: @plugin and @config need JS Promise Integration (WebAssembly.Suspending) in the browser.');
      }
      promising = true;
    }
    // The imports are there also when no build calls them.
    imports = (exports) => {
      if (!promising) return { tailwind_js: missingEngine() };
      engine = createEngine(exports, (name) => found[name]);
      return { tailwind_js: engine.imports };
    };
  }
  const files = new Files();
  files.mkdir('/project');
  for (const [name, content] of Object.entries(options.files ?? {})) {
    files.write(`/project/${name.replace(/^\/+/, '')}`, content);
  }
  try {
    return await run(module, {
      args,
      env: options.env ?? {},
      stdin: options.input ?? '',
      files,
      preopens: { '/': '/project' },
      imports,
      promising,
    });
  } finally {
    engine?.dispose();
  }
}

// The imports of the engine for a build with no @plugin and no @config: the
// program does not call them.
function missingEngine() {
  const missing = () => {
    throw new Error('tailwind.com: this build has no JavaScript engine.');
  };
  return { tw_js_new: missing, tw_js_free: missing, tw_js_eval: missing, tw_js_call: missing, tw_js_call_async: missing };
}
