// The API of the npm package tailwind.com in a browser: the condition
// "browser" of package.json gives this file in place of js/node.mjs. It
// runs tailwind.wasm with a file system in memory (js/wasi.mjs).
//
//   import { runWasm } from 'tailwind.com';
//
//   const { status, stdout, stderr } = await runWasm(['-i', 'app.css', '--minify'], {
//     files: { 'app.css': '@import "tailwindcss";', 'index.html': '<div class="flex p-4">' },
//   });
//
// It also runs in a Web Worker, and in Deno and Node.js with no file
// system. A bundler (Vite, webpack, esbuild) copies tailwind.wasm with the
// pattern `new URL('../runtime/tailwind.wasm', import.meta.url)`.
import { createEngine, hasPromiseIntegration, needsEngine } from './engine.mjs';
import { Files, run } from './wasi.mjs';

// The URL of tailwind.wasm, from the URL of a file of the package. A CDN
// URL has the package root in it: "https://cdn.jsdelivr.net/npm/tailwind.com@0.4.0/+esm",
// "https://esm.sh/tailwind.com@0.4.0/es2022/tailwind.com.mjs", or only the
// root for the default file, "https://cdn.jsdelivr.net/npm/tailwind.com@0.5".
// Else, the file is in js/ of the package.
export function wasmUrl(from = import.meta.url) {
  const url = new URL(from);
  const found = url.pathname.match(/^(.*\/tailwind\.com(?:@[^/]*)?)(?:\/|$)/);
  if (found) return new URL(`${found[1]}/runtime/tailwind.wasm`, url).href;
  if (from === import.meta.url) return new URL('../runtime/tailwind.wasm', import.meta.url).href;
  return new URL('../runtime/tailwind.wasm', url).href;
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

// The JavaScript files that a build of `input` and `files` needs, from the
// directory of tailwind.wasm: none with no @plugin and no @config.
async function assets(wasm, input, files) {
  const all = [input, ...Object.values(files).filter((value) => typeof value === 'string')].join('\n');
  if (!/@(?:plugin|config)\b/.test(all)) return {};
  const names = { compat: 'compat.js' };
  // An ES module or a TypeScript file becomes CommonJS with Sucrase.
  if (Object.keys(files).some((name) => /\.[cm]?[jt]s$/.test(name))) names.typescript = 'typescript.js';
  for (const plugin of PLUGINS) {
    if (all.includes(plugin)) names[`plugin:${plugin}`] = `plugins/${plugin.slice('@tailwindcss/'.length)}.js`;
  }
  const entries = await Promise.all(Object.entries(names).map(async ([name, file]) => [name, await text(new URL(file, wasm).href)]));
  return Object.fromEntries(entries);
}

// Run the program with the arguments of the command line program, as
// runWasm of js/node.mjs. The files are in memory: options.files is an
// object { path: text or Uint8Array }, with paths from the root "/" of the
// program. The program sees no other file.
//
// options: files, env (default: empty), input (the text of stdin), wasm
// (the URL of tailwind.wasm, default: wasmUrl()).
//
// tailwind.wasm has no JavaScript engine: for @plugin and @config, it
// runs the plugins in the JavaScript engine of the browser
// (js/engine.mjs). That needs a page (not a worker) and JS Promise
// Integration. The JavaScript files come from the directory of the
// program.
// The result is { status, stdout, stderr } with the output as text. With
// no -o, the CSS is in stdout. --watch does not end: do not give it.
export async function runWasm(args = [], options = {}) {
  // A relative URL is relative to the page.
  const url = new URL(String(options.wasm ?? wasmUrl()), globalThis.location?.href ?? import.meta.url).href;
  // The JavaScript files download while the program compiles.
  const loading = assets(url, options.input ?? '', options.files ?? {});
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
