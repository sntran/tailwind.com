#!/usr/bin/env node
// Usage: node run-cli.mjs <scratch directory> <cli fixture directory> [version] [case...]
// This script runs the official CLI for each case in the cli fixture directory (cli/ or cli-js/).
// It installs the npm packages tailwindcss and @tailwindcss/cli into the scratch directory.
// It copies each project into the scratch directory, so that "tailwindcss" resolves from there.
// It writes stdout.css and exit.txt into each case directory, and stderr.txt for an error.
// With case names, it records only these cases.
//
// A case with a file standalone.txt gets the module interop of the standalone program
// (see bun-interop.mjs).
//
// In stderr.txt, the text <project> replaces the absolute path of the project copy.
import { execFileSync, spawnSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

let [scratch, cliDir, version = '4.3.5', ...only] = process.argv.slice(2)
if (!scratch || !cliDir) {
  console.error('Usage: node run-cli.mjs <scratch directory> <cli fixture directory> [version] [case...]')
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

let interop = pathToFileURL(path.join(path.dirname(fileURLToPath(import.meta.url)), 'bun-interop.mjs')).href

let work = path.join(scratch, 'work')
fs.rmSync(work, { recursive: true, force: true })
// The real path: the CLI prints paths without symbolic links.
fs.mkdirSync(work, { recursive: true })
work = fs.realpathSync(work)

for (let name of fs.readdirSync(cliDir).sort()) {
  let caseDir = path.join(cliDir, name)
  if (!fs.existsSync(path.join(caseDir, 'args.txt'))) continue
  if (only.length > 0 && !only.includes(name)) continue

  let project = path.join(work, name, 'project')
  fs.cpSync(path.join(caseDir, 'project'), project, { recursive: true })

  let args = fs.readFileSync(path.join(caseDir, 'args.txt'), 'utf8').split('\n').filter((line) => line !== '')
  let nodeArgs = fs.existsSync(path.join(caseDir, 'standalone.txt')) ? ['--import', interop] : []
  let result = spawnSync(process.execPath, [...nodeArgs, bin, ...args], {
    cwd: project,
    env: { ...process.env, NODE_ENV: '', NO_COLOR: '1' },
    maxBuffer: 256 * 1024 * 1024,
  })

  fs.writeFileSync(path.join(caseDir, 'stdout.css'), result.stdout)
  fs.writeFileSync(path.join(caseDir, 'exit.txt'), `${result.status}\n`)
  if (result.status !== 0) {
    let stderr = result.stderr.toString('utf8').split(project).join('<project>')
    fs.writeFileSync(path.join(caseDir, 'stderr.txt'), stderr)
  } else {
    fs.rmSync(path.join(caseDir, 'stderr.txt'), { force: true })
  }
  console.log(`${name}: exit ${result.status}, ${result.stdout.length} bytes`)
}
