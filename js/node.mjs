// The API of the npm package tailwind.com for Node.js.
//
//   import { run, runWasm, binPath, release } from 'tailwind.com';
//
//   // The native program (a download at the first use):
//   const { status } = await run(['-i', 'app.css', '-o', 'out.css', '--minify']);
//
//   // The same program as WebAssembly, in this process, with no download:
//   const { status, stdout } = await runWasm(['-i', 'app.css'], { cwd: 'project' });
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { command, ensure } from './download.mjs';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

// The release of this package: { version, tag, tailwindcss, sha256 }, or
// null in a checkout that has no runtime/ (scripts/npm.sh makes it).
export function release() {
  try {
    return JSON.parse(fs.readFileSync(path.join(root, 'runtime', 'release.json'), 'utf8'));
  } catch {
    return null;
  }
}

// The path of the WebAssembly module (WASI preview 1).
export const wasmPath = path.join(root, 'runtime', 'tailwind.wasm');

// The path of the program for a browser (the condition "browser"): no
// optimizer and no JavaScript engine.
export const browserWasmPath = path.join(root, 'runtime', 'tailwind-browser.wasm');

// The path of tailwind.com, after a download when it is not in the cache.
export function binPath(options = {}) {
  return ensure(release(), options);
}

// Run tailwind.com with the arguments. options: cwd, env, stdio (default
// "inherit"), signal. The result is { status, signal }.
export async function run(args = [], options = {}) {
  const file = await binPath({ env: options.env ?? process.env, log: options.log });
  const [program, argv] = command(file, args);
  return new Promise((resolve, reject) => {
    const child = spawn(program, argv, {
      cwd: options.cwd,
      env: options.env,
      stdio: options.stdio ?? 'inherit',
      signal: options.signal,
    });
    child.once('error', reject);
    child.once('exit', (status, signal) => resolve({ status, signal }));
  });
}

// The JavaScript files of the program for the engine of tailwind.wasm
// (js/engine.mjs), from the disk when the program asks: next to the
// module (runtime/ of the package), else in the repository.
function assetReader(wasm) {
  const dirs = [path.dirname(path.resolve(wasm)), path.join(root, 'native', 'tailwind', 'assets')];
  const cache = new Map();
  return (name) => {
    if (!cache.has(name)) {
      const file =
        name === 'compat' ? 'compat.js' : name === 'typescript' ? 'typescript.js' : name.startsWith('plugin:@tailwindcss/') ? `plugins/${name.slice('plugin:@tailwindcss/'.length)}.js` : null;
      let text;
      for (const dir of file ? dirs : []) {
        try {
          text = fs.readFileSync(path.join(dir, file), 'utf8');
          break;
        } catch {}
      }
      cache.set(name, text);
    }
    return cache.get(name);
  };
}

// A realm for the plugins: a context of node:vm.
async function vmRealm() {
  const vm = await import('node:vm');
  return () => {
    const context = vm.createContext({});
    return { global: vm.runInContext('globalThis', context), dispose() {} };
  };
}

class Exit {
  constructor(code) {
    this.code = code;
  }
}

// Run the WebAssembly program in this process, with the arguments of the
// command line program. The directory cwd (default: the current
// directory) is the root "/" of the program: it sees no file outside it.
// options: cwd, env (default: empty), input (the text of stdin), wasm
// (the file of the module).
// The result is { status, stdout, stderr } with the output as text.
// --watch does not end: use run() for a watch.
//
// tailwind.wasm runs the plugins of @plugin and @config in QuickJS. The
// program for a browser (browserWasmPath) has no JavaScript engine: with
// it, the plugins run in the JavaScript engine of Node.js, in a context of
// node:vm (js/engine.mjs). That needs JS Promise Integration: Node.js 24
// or later (Node.js 22 with --experimental-wasm-jspi).
export async function runWasm(args = [], options = {}) {
  const { WASI } = await import('node:wasi');
  const cwd = path.resolve(options.cwd ?? '.');
  const wasm = options.wasm ?? wasmPath;
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'tailwind-wasm-'));
  const files = { stdin: path.join(tmp, 'stdin'), stdout: path.join(tmp, 'stdout'), stderr: path.join(tmp, 'stderr') };
  fs.writeFileSync(files.stdin, options.input ?? '');
  const fds = {
    stdin: fs.openSync(files.stdin, 'r'),
    stdout: fs.openSync(files.stdout, 'w'),
    stderr: fs.openSync(files.stderr, 'w'),
  };
  let engine = null;
  try {
    const wasi = new WASI({
      version: 'preview1',
      args: ['tailwind', ...args],
      env: { PWD: '/', ...(options.env ?? {}) },
      preopens: { '/': cwd },
      ...fds,
    });
    const module = await WebAssembly.compile(fs.readFileSync(wasm));
    const imports = wasi.getImportObject();
    // The exit status: proc_exit ends the run with an exception.
    imports.wasi_snapshot_preview1 = {
      ...imports.wasi_snapshot_preview1,
      proc_exit: (code) => {
        throw new Exit(code);
      },
    };
    let instance = null;
    let promising = false;
    const { createEngine, hasPromiseIntegration, needsEngine } = await import('./engine.mjs');
    if (needsEngine(module)) {
      if (hasPromiseIntegration) {
        engine = createEngine(() => instance.exports, assetReader(wasm), { realm: await vmRealm() });
        imports.tailwind_js = engine.imports;
        promising = true;
      } else {
        const missing = () => {
          throw new Error('tailwind.com: @plugin and @config need JS Promise Integration: Node.js 24 or later.');
        };
        imports.tailwind_js = { tw_js_new: missing, tw_js_free: missing, tw_js_eval: missing, tw_js_call: missing, tw_js_call_async: missing };
      }
    }
    instance = await WebAssembly.instantiate(module, imports);
    // node:wasi sets the memory and calls _start. With JS Promise
    // Integration, _start gives a promise: the run ends when it settles.
    let running = null;
    let status = 0;
    try {
      const start = promising ? WebAssembly.promising(instance.exports._start) : instance.exports._start;
      wasi.start({ exports: { memory: instance.exports.memory, _start: () => void (running = start()) } });
      await running;
    } catch (error) {
      if (!(error instanceof Exit)) throw error;
      status = error.code;
    }
    return {
      status,
      stdout: fs.readFileSync(files.stdout, 'utf8'),
      stderr: fs.readFileSync(files.stderr, 'utf8'),
    };
  } finally {
    engine?.dispose();
    for (const fd of Object.values(fds)) fs.closeSync(fd);
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}
