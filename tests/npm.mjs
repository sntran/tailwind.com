// The tests of the npm package (js/).
//
// Usage: node --test tests/npm.mjs
//
// TAILWIND is a program to run (default: target/release/tailwind), and
// TAILWIND_WASM the WebAssembly module (default: build/tailwind.wasm). A
// test that needs a file that is not there is skipped.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { after, before, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { cacheDir, cachePath, command, downloadUrl, ensure } from '../js/download.mjs';
import { run, runWasm } from '../js/node.mjs';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const program = path.resolve(process.env.TAILWIND ?? path.join(root, 'target/release/tailwind'));
const wasm = path.resolve(process.env.TAILWIND_WASM ?? path.join(root, 'build/tailwind.wasm'));
const hasProgram = fs.existsSync(program);
const hasWasm = fs.existsSync(wasm);

const body = Buffer.from('MZ a file that stands for tailwind.com\n');
const sha256 = createHash('sha256').update(body).digest('hex');
const release = { version: '0.0.0', tag: 'v0.0.0', sha256 };

let server;
let base;
let requests = 0;
let work;

before(async () => {
  work = fs.mkdtempSync(path.join(os.tmpdir(), 'tailwind-npm-'));
  server = http.createServer((request, response) => {
    requests++;
    if (request.url === '/ok/tailwind.com') {
      response.writeHead(200).end(body);
    } else if (request.url === '/slow/tailwind.com') {
      response.writeHead(200);
      response.write(body.subarray(0, 4)); // and no more bytes
    } else {
      response.writeHead(404).end('no such file');
    }
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  base = `http://127.0.0.1:${server.address().port}`;
});

after(() => {
  server.closeAllConnections();
  server.close();
  fs.rmSync(work, { recursive: true, force: true });
});

function env(name, extra = {}) {
  return { TAILWIND_COM_CACHE: path.join(work, name), ...extra };
}

test('the cache directory follows the environment', () => {
  assert.equal(cacheDir({ TAILWIND_COM_CACHE: '/c' }), '/c');
  assert.equal(cacheDir({ XDG_CACHE_HOME: '/x' }), path.join('/x', 'tailwind.com'));
  assert.equal(cacheDir({ LOCALAPPDATA: '/l' }), path.join('/l', 'tailwind.com'));
  assert.equal(cacheDir({ HOME: '/h' }), path.join('/h', '.cache', 'tailwind.com'));
});

test('the path in the cache has the tag and the start of the SHA-256', () => {
  const file = cachePath(release, { TAILWIND_COM_CACHE: '/c' });
  assert.equal(file, path.join('/c', 'npm', `v0.0.0-${sha256.slice(0, 12)}`, 'tailwind.com'));
});

test('the URL is the GitHub release of the tag, or the mirror', () => {
  assert.equal(downloadUrl(release, {}), 'https://github.com/sntran/tailwind.com/releases/download/v0.0.0/tailwind.com');
  assert.equal(downloadUrl(release, { TAILWIND_COM_DOWNLOAD: 'https://example.com/dir/' }), 'https://example.com/dir/tailwind.com');
});

test('TAILWIND_COM names the file, with no download', async () => {
  const before = requests;
  assert.equal(await ensure(release, { env: { TAILWIND_COM: '/some/tailwind.com' } }), '/some/tailwind.com');
  assert.equal(await ensure(null, { env: { TAILWIND_COM: '/some/tailwind.com' } }), '/some/tailwind.com');
  assert.equal(requests, before);
});

test('a package with no release gives a clear error', async () => {
  await assert.rejects(ensure(null, { env: env('none') }), /has no release of tailwind.com/);
  await assert.rejects(ensure({ tag: 'v1', sha256: 'abc' }, { env: env('none') }), /has no release/);
});

test('a download goes to the cache, and the second use has no download', async () => {
  const e = env('ok', { TAILWIND_COM_DOWNLOAD: `${base}/ok` });
  const logs = [];
  const file = await ensure(release, { env: e, log: (text) => logs.push(text) });
  assert.equal(file, cachePath(release, e));
  assert.deepEqual(fs.readFileSync(file), body);
  if (process.platform !== 'win32') assert.notEqual(fs.statSync(file).mode & 0o111, 0);
  assert.equal(logs.length, 1);
  const count = requests;
  assert.equal(await ensure(release, { env: e }), file);
  assert.equal(requests, count);
});

test('a file with another SHA-256 does not go to the cache', async () => {
  const e = env('bad', { TAILWIND_COM_DOWNLOAD: `${base}/ok` });
  const wrong = { ...release, sha256: '0'.repeat(64) };
  await assert.rejects(ensure(wrong, { env: e }), /the SHA-256 is .* and this package expects 0000/);
  const dir = path.dirname(cachePath(wrong, e));
  assert.deepEqual(fs.existsSync(dir) ? fs.readdirSync(dir) : [], []);
});

test('a status that is not 200 is an error', async () => {
  const e = env('missing', { TAILWIND_COM_DOWNLOAD: `${base}/missing` });
  await assert.rejects(ensure(release, { env: e }), /status 404/);
});

test('a download with no bytes stops after the idle time', async () => {
  const e = env('slow', { TAILWIND_COM_DOWNLOAD: `${base}/slow` });
  await assert.rejects(ensure(release, { env: e, idle: 200 }), /no bytes in 0.2 s/);
  const dir = path.dirname(cachePath(release, e));
  assert.deepEqual(fs.existsSync(dir) ? fs.readdirSync(dir) : [], []);
});

test('the shell starts an APE file, and another file starts directly', async () => {
  const e = env('ok', { TAILWIND_COM_DOWNLOAD: `${base}/ok` });
  const ape = await ensure(release, { env: e });
  assert.deepEqual(command(ape, ['-h'], 'linux'), ['/bin/sh', [ape, '-h']]);
  assert.deepEqual(command(ape, ['-h'], 'win32'), [ape, ['-h']]);
  const other = path.join(work, 'other');
  fs.writeFileSync(other, '\x7fELF');
  assert.deepEqual(command(other, ['-h'], 'linux'), [other, ['-h']]);
});

function project(name) {
  const dir = path.join(work, name);
  fs.mkdirSync(path.join(dir, 'src'), { recursive: true });
  fs.writeFileSync(path.join(dir, 'app.css'), '@import "tailwindcss" source(none);\n@source "./src";\n');
  fs.writeFileSync(path.join(dir, 'src', 'a.html'), '<p class="flex underline"></p>\n');
  return dir;
}

test('run() runs the program', { skip: !hasProgram }, async () => {
  const dir = project('run');
  const result = await run(['-i', 'app.css', '-o', 'out.css', '--minify'], {
    cwd: dir,
    env: { ...process.env, TAILWIND_COM: program },
    stdio: 'ignore',
  });
  assert.equal(result.status, 0);
  assert.match(fs.readFileSync(path.join(dir, 'out.css'), 'utf8'), /\.flex\{display:flex\}/);
});

test('the command gives the exit status of the program', { skip: !hasProgram }, () => {
  const dir = project('bin');
  const bin = path.join(root, 'js', 'tailwind.com.mjs');
  const ok = spawnSync(process.execPath, [bin, '-i', 'app.css'], { cwd: dir, env: { ...process.env, TAILWIND_COM: program } });
  assert.equal(ok.status, 0);
  assert.match(ok.stdout.toString(), /\.underline \{/);
  fs.writeFileSync(path.join(dir, 'app.css'), '.a { @apply does-not-exist; }\n');
  const bad = spawnSync(process.execPath, [bin, '-i', 'app.css'], { cwd: dir, env: { ...process.env, TAILWIND_COM: program } });
  assert.equal(bad.status, 1);
  assert.match(bad.stderr.toString(), /Cannot apply unknown utility class/);
});

test('runWasm() runs the WebAssembly program in this process', { skip: !hasWasm }, async () => {
  const dir = project('wasm');
  const result = await runWasm(['-i', 'app.css'], { cwd: dir, wasm });
  assert.equal(result.status, 0);
  assert.match(result.stdout, /\.flex \{/);
  assert.match(result.stderr, /tailwindcss v/);
  fs.writeFileSync(path.join(dir, 'app.css'), '.a { @apply does-not-exist; }\n');
  const bad = await runWasm(['-i', 'app.css'], { cwd: dir, wasm });
  assert.equal(bad.status, 1);
  assert.match(bad.stderr, /Cannot apply unknown utility class/);
});

test('runWasm() reads the input from options.input', { skip: !hasWasm }, async () => {
  const dir = project('wasm-stdin');
  const result = await runWasm(['-i', '-'], { cwd: dir, wasm, input: '@theme { --color-x: red; }\n.a { color: var(--color-x); }\n' });
  assert.equal(result.status, 0);
  assert.match(result.stdout, /--color-x: red/);
});
