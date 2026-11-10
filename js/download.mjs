// The tailwind.com of this package: the file of the GitHub release that
// the package names, in the cache, with the SHA-256 that the package holds
// (runtime/release.json, from scripts/npm.sh). The package does not hold
// the file: a program that only uses the WebAssembly module never
// downloads it. This is the method of the npm package beam.com.
//
// - TAILWIND_COM: the path of a tailwind.com to use. Then nothing is
//   downloaded.
// - TAILWIND_COM_DOWNLOAD: the URL of the directory of the file, in place
//   of the GitHub release (a mirror, or a test).
// - TAILWIND_COM_CACHE: the cache directory.
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export const REPOSITORY = 'sntran/tailwind.com';

// The time with no bytes that stops a download (ms).
const IDLE = 60000;

export function cacheDir(env = process.env) {
  if (env.TAILWIND_COM_CACHE) return env.TAILWIND_COM_CACHE;
  if (env.XDG_CACHE_HOME) return path.join(env.XDG_CACHE_HOME, 'tailwind.com');
  if (env.LOCALAPPDATA) return path.join(env.LOCALAPPDATA, 'tailwind.com');
  return path.join(env.HOME ?? os.homedir(), '.cache', 'tailwind.com');
}

// The path of the file in the cache. Windows runs an APE file as a PE
// executable with the name .com or .exe.
export function cachePath({ tag, sha256 }, env = process.env) {
  return path.join(cacheDir(env), 'npm', `${tag}-${sha256.slice(0, 12)}`, 'tailwind.com');
}

export function downloadUrl({ tag }, env = process.env) {
  const base = env.TAILWIND_COM_DOWNLOAD || `https://github.com/${REPOSITORY}/releases/download/${tag}`;
  return `${base.replace(/\/$/, '')}/tailwind.com`;
}

// The path of a tailwind.com that runs. A file is in the cache only after
// the check of its SHA-256.
// idle: the download stops when no bytes come in this time (ms).
export async function ensure(release, { env = process.env, fetch = globalThis.fetch, log = () => {}, idle = IDLE } = {}) {
  if (env.TAILWIND_COM) return env.TAILWIND_COM;
  if (!release?.tag || !/^[0-9a-f]{64}$/.test(release?.sha256 ?? '')) {
    throw new Error('this package has no release of tailwind.com (runtime/release.json): set TAILWIND_COM to the path of a tailwind.com');
  }
  const file = cachePath(release, env);
  if (fs.existsSync(file)) return file;
  const url = downloadUrl(release, env);
  log(`tailwind.com: downloading ${url}`);
  const ac = new AbortController();
  let timer;
  const wait = () => {
    clearTimeout(timer);
    timer = setTimeout(() => ac.abort(new Error(`tailwind.com: ${url}: no bytes in ${idle / 1000} s`)), idle);
  };
  wait();
  let response;
  try {
    response = await fetch(url, { signal: ac.signal });
  } catch (e) {
    clearTimeout(timer);
    throw ac.signal.aborted ? ac.signal.reason : e;
  }
  if (!response.ok) {
    clearTimeout(timer);
    throw new Error(`tailwind.com: ${url}: status ${response.status}`);
  }
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const part = `${file}.${process.pid}.part`;
  const out = fs.createWriteStream(part, { mode: 0o755 });
  // An error of the file (a full disk): no drain comes after it.
  const failed = new Promise((_, reject) => out.once('error', reject));
  failed.catch(() => {});
  const hash = createHash('sha256');
  try {
    for await (const chunk of response.body) {
      wait();
      hash.update(chunk);
      if (!out.write(chunk)) await Promise.race([new Promise((r) => out.once('drain', r)), failed]);
    }
    await new Promise((resolve, reject) => out.end((e) => (e ? reject(e) : resolve())));
    const got = hash.digest('hex');
    if (got !== release.sha256) {
      throw new Error(`tailwind.com: ${url}: the SHA-256 is ${got}, and this package expects ${release.sha256}`);
    }
    fs.chmodSync(part, 0o755);
    fs.renameSync(part, file);
  } catch (e) {
    throw ac.signal.aborted ? ac.signal.reason : e;
  } finally {
    clearTimeout(timer);
    out.destroy();
    fs.rmSync(part, { force: true });
  }
  return file;
}

// The command that starts the file. tailwind.com is an Actually Portable
// Executable (its first bytes are "MZ"): Windows runs it as a program, and
// a Unix shell runs the shell script at its start. A file of another type
// (TAILWIND_COM can name a program for this system only) starts directly.
export function command(file, args, platform = process.platform) {
  if (platform === 'win32' || !isApe(file)) return [file, args];
  return ['/bin/sh', [file, ...args]];
}

function isApe(file) {
  try {
    const fd = fs.openSync(file, 'r');
    const head = Buffer.alloc(2);
    fs.readSync(fd, head, 0, 2, 0);
    fs.closeSync(fd);
    return head.toString('latin1') === 'MZ';
  } catch {
    return false;
  }
}
