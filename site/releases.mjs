#!/usr/bin/env node
// The programs of the site: the build of the site, and tailwind.wasm of
// each release (scripts/releases.sh).
//
// Usage: node site/releases.mjs SITE RELEASES
//
// It copies RELEASES/TAG/tailwind.wasm to SITE/releases/TAG/tailwind.wasm,
// and writes the list of the programs into SITE/data.json (`programs`).
// The first program is the build of the site (SITE/tailwind.wasm). The
// list replaces the list that is there, so a deploy can add a new release
// to a site that CI built before the release.
import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const compare = (a, b) => {
  const x = a.replace(/^v/, '').split('.').map(Number)
  const y = b.replace(/^v/, '').split('.').map(Number)
  return x[0] - y[0] || x[1] - y[1] || x[2] - y[2]
}

// The name of the build of the site: the branch and the commit.
function buildName() {
  const sha = process.env.GITHUB_SHA ?? (() => {
    try {
      return execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
    } catch {
      return ''
    }
  })()
  // In CI: the branch (also of a pull request), else main.
  const branch = process.env.GITHUB_HEAD_REF || process.env.GITHUB_REF_NAME || 'main'
  return sha ? `${branch} (${sha.slice(0, 7)})` : branch
}

// `name` is the name of the build of the site. A deploy keeps the name
// that site/build.mjs wrote: the commit of the site, not of the deploy.
export function programs(site, releases, name = buildName()) {
  const list = [{ name, wasm: 'tailwind.wasm' }]
  fs.rmSync(path.join(site, 'releases'), { recursive: true, force: true })
  if (releases && fs.existsSync(releases)) {
    const tags = fs
      .readdirSync(releases)
      .filter((tag) => /^v\d+\.\d+\.\d+$/.test(tag) && fs.existsSync(path.join(releases, tag, 'tailwind.wasm')))
      .sort((a, b) => compare(b, a))
    for (const tag of tags) {
      const to = path.join(site, 'releases', tag, 'tailwind.wasm')
      fs.mkdirSync(path.dirname(to), { recursive: true })
      fs.copyFileSync(path.join(releases, tag, 'tailwind.wasm'), to)
      list.push({ name: tag, wasm: `releases/${tag}/tailwind.wasm` })
    }
  }
  return list
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [site, releases] = process.argv.slice(2)
  if (!site || !releases) throw new Error('Usage: node site/releases.mjs SITE RELEASES')
  const file = path.join(site, 'data.json')
  const data = JSON.parse(fs.readFileSync(file, 'utf8'))
  data.programs = programs(site, releases, data.programs?.[0]?.name)
  fs.writeFileSync(file, JSON.stringify(data))
  console.error(`site/releases.mjs: ${data.programs.map((p) => p.name).join(', ')}`)
}
