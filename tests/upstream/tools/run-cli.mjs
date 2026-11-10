#!/usr/bin/env node
// Usage: node run-cli.mjs <scratch directory> <cli fixture directory> [version]
// This script runs the official CLI for each case in the cli fixture directory.
// It installs the npm packages tailwindcss and @tailwindcss/cli into the scratch directory.
// It copies each project into the scratch directory, so that "tailwindcss" resolves from there.
// It writes stdout.css and exit.txt into each case directory.
import { execFileSync, spawnSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'

let [scratch, cliDir, version = '4.3.5'] = process.argv.slice(2)
if (!scratch || !cliDir) {
  console.error('Usage: node run-cli.mjs <scratch directory> <cli fixture directory> [version]')
  process.exit(2)
}
scratch = path.resolve(scratch)
cliDir = path.resolve(cliDir)

let bin = path.join(scratch, 'node_modules/@tailwindcss/cli/dist/index.mjs')
if (!fs.existsSync(bin)) {
  fs.mkdirSync(scratch, { recursive: true })
  if (!fs.existsSync(path.join(scratch, 'package.json'))) {
    fs.writeFileSync(path.join(scratch, 'package.json'), '{"private":true}\n')
  }
  execFileSync('npm', ['install', `tailwindcss@${version}`, `@tailwindcss/cli@${version}`], {
    cwd: scratch,
    stdio: 'inherit',
  })
}

let work = path.join(scratch, 'work')
fs.rmSync(work, { recursive: true, force: true })

for (let name of fs.readdirSync(cliDir).sort()) {
  let caseDir = path.join(cliDir, name)
  if (!fs.existsSync(path.join(caseDir, 'args.txt'))) continue

  let project = path.join(work, name, 'project')
  fs.cpSync(path.join(caseDir, 'project'), project, { recursive: true })

  let args = fs.readFileSync(path.join(caseDir, 'args.txt'), 'utf8').split('\n').filter((line) => line !== '')
  let result = spawnSync(process.execPath, [bin, ...args], {
    cwd: project,
    env: { ...process.env, NODE_ENV: '', NO_COLOR: '1' },
    maxBuffer: 256 * 1024 * 1024,
  })

  fs.writeFileSync(path.join(caseDir, 'stdout.css'), result.stdout)
  fs.writeFileSync(path.join(caseDir, 'exit.txt'), `${result.status}\n`)
  if (result.status !== 0) {
    fs.writeFileSync(path.join(caseDir, 'stderr.txt'), result.stderr)
  } else {
    fs.rmSync(path.join(caseDir, 'stderr.txt'), { force: true })
  }
  console.log(`${name}: exit ${result.status}, ${result.stdout.length} bytes`)
}
