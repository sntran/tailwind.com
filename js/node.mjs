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

// Run the WebAssembly program in this process, with the arguments of the
// command line program. The directory cwd (default: the current
// directory) is the root "/" of the program: it sees no file outside it.
// options: cwd, env (default: empty), input (the text of stdin).
// The result is { status, stdout, stderr } with the output as text.
// --watch does not end: use run() for a watch.
export async function runWasm(args = [], options = {}) {
  const { WASI } = await import('node:wasi');
  const cwd = path.resolve(options.cwd ?? '.');
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'tailwind-wasm-'));
  const files = { stdin: path.join(tmp, 'stdin'), stdout: path.join(tmp, 'stdout'), stderr: path.join(tmp, 'stderr') };
  fs.writeFileSync(files.stdin, options.input ?? '');
  const fds = {
    stdin: fs.openSync(files.stdin, 'r'),
    stdout: fs.openSync(files.stdout, 'w'),
    stderr: fs.openSync(files.stderr, 'w'),
  };
  try {
    const wasi = new WASI({
      version: 'preview1',
      args: ['tailwind', ...args],
      env: { PWD: '/', ...(options.env ?? {}) },
      preopens: { '/': cwd },
      returnOnExit: true,
      ...fds,
    });
    const module = await WebAssembly.compile(fs.readFileSync(options.wasm ?? wasmPath));
    const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
    const status = wasi.start(instance);
    for (const fd of Object.values(fds)) fs.closeSync(fd);
    return {
      status,
      stdout: fs.readFileSync(files.stdout, 'utf8'),
      stderr: fs.readFileSync(files.stderr, 'utf8'),
    };
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}
