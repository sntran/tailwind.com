// Version 4 ignores these options. There is no warning.
module.exports = {
  corePlugins: { float: false, preflight: false },
  separator: '_',
  safelist: ['underline', { pattern: /^bg-red-/ }],
  future: { hoverOnlyWhenSupported: false },
  experimental: { optimizeUniversalDefaults: true },
  variants: { extend: { opacity: ['disabled'] } },
  purge: ['./nothing.html'],
  mode: 'jit',
}
