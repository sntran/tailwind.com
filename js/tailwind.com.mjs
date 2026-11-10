#!/usr/bin/env node
// npx tailwind.com ARGS: tailwind.com of the release of this package, with
// ARGS (download.mjs). For example:
//
//   npx tailwind.com -i app.css -o dist/app.css --minify
//
// The exit status is the exit status of the program. A signal that stops
// this process also stops the program. The standard input goes to the
// program, so a watch (--watch) stops when the input closes.
import { spawn } from 'node:child_process';
import { constants } from 'node:os';
import { command, ensure } from './download.mjs';
import { release } from './node.mjs';

let file;
try {
  file = await ensure(release(), { log: (text) => console.error(text) });
} catch (e) {
  console.error(e.message);
  process.exit(1);
}

const [program, args] = command(file, process.argv.slice(2));
const child = spawn(program, args, { stdio: 'inherit' });
for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
  process.on(signal, () => child.kill(signal));
}
child.once('error', (e) => {
  console.error(`tailwind.com: ${e.message}`);
  process.exit(1);
});
child.once('exit', (status, signal) => {
  // The status of a program that a signal stops: 128 + the signal number.
  process.exit(signal ? 128 + (constants.signals[signal] ?? 0) : status ?? 1);
});
