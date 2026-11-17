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

// Run the program with the arguments of the command line program, as
// runWasm of js/node.mjs. The files are in memory: options.files is an
// object { path: text or Uint8Array }, with paths from the root "/" of the
// program. The program sees no other file.
//
// options: files, env (default: empty), input (the text of stdin), wasm
// (the URL of tailwind.wasm, default: wasmUrl()).
// The result is { status, stdout, stderr } with the output as text. With
// no -o, the CSS is in stdout. --watch does not end: do not give it.
export async function runWasm(args = [], options = {}) {
  const module = await compile(String(options.wasm ?? wasmUrl()));
  const files = new Files();
  files.mkdir('/project');
  for (const [name, content] of Object.entries(options.files ?? {})) {
    files.write(`/project/${name.replace(/^\/+/, '')}`, content);
  }
  return run(module, {
    args,
    env: options.env ?? {},
    stdin: options.input ?? '',
    files,
    preopens: { '/': '/project' },
  });
}
