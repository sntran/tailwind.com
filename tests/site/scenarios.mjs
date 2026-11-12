// The cases of the tests of the site host (tests/site/host.mjs and
// tests/site/browser.mjs): a project, the arguments and the environment
// of one run of tailwind.wasm. The packages of Tailwind CSS are in
// /packages, in the form of the ZIP directory of tailwind.com.
export const project = {
  'app/index.html': '<article class="prose flex p-4 hover:bg-sky-500 text-lg/7 md:grid-cols-3 shadow-lg ring-2"><input class="form-input"></article>\n',
  'app/app.css': '@import "tailwindcss";\n',
  'app/plugins.css': '@import "tailwindcss";\n@plugin "@tailwindcss/typography";\n@plugin "@tailwindcss/forms";\n',
  'app/local.css': '@import "tailwindcss";\n@plugin "./plugin.js";\n',
  'app/plugin.js': "module.exports = function ({ addUtilities, theme }) {\n  addUtilities({ '.from-plugin': { color: theme('colors.red.500') } })\n}\n",
  'app/bad.css': '@import "tailwindcss";\n.a { @apply no-such-class; }\n',
}

const env = { TAILWIND_COM_PACKAGES: '/packages', NO_COLOR: '1' }

export const scenarios = [
  { name: '--help', args: ['--help'], env },
  { name: '4.3.5', args: ['-i', 'app.css'], env },
  { name: '4.3.5, --minify', args: ['-i', 'app.css', '--minify'], env },
  { name: '4.3.5, output file', args: ['-i', 'app.css', '-o', 'out/app.css'], env, output: 'app/out/app.css' },
  { name: '4.3.5, two plugins', args: ['-i', 'plugins.css'], env },
  { name: '4.3.5, a local plugin', args: ['-i', 'local.css'], env },
  { name: '4.3.5, an error', args: ['-i', 'bad.css'], env },
  { name: '4.1.0', args: ['-i', 'app.css'], env: { ...env, TAILWINDCSS_VERSION: '4.1.0' } },
  { name: '4.0.0', args: ['-i', 'app.css'], env: { ...env, TAILWINDCSS_VERSION: '4.0.0' } },
  { name: '4.0.0, two plugins', args: ['-i', 'plugins.css'], env: { ...env, TAILWINDCSS_VERSION: '4.0.0' } },
  { name: '4.3.5, script engine', args: ['-i', 'app.css'], env: { ...env, TAILWIND_COM_ENGINE: 'script' } },
  { name: '4.0.0, script engine', args: ['-i', 'app.css'], env: { ...env, TAILWINDCSS_VERSION: '4.0.0', TAILWIND_COM_ENGINE: 'script' } },
  { name: '4.0.0, script engine, two plugins', args: ['-i', 'plugins.css'], env: { ...env, TAILWINDCSS_VERSION: '4.0.0', TAILWIND_COM_ENGINE: 'script' } },
]
