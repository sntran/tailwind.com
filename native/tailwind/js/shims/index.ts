// In place of `index.ts`: only the constants that the compatibility layer
// uses.
export const enum Features {
  None = 0,
  AtApply = 1 << 0,
  AtImport = 1 << 1,
  JsPluginCompat = 1 << 2,
  ThemeFunction = 1 << 3,
  Utilities = 1 << 4,
  Variants = 1 << 5,
  AtTheme = 1 << 6,
}

export const enum Polyfills {
  None = 0,
  AtProperty = 1 << 0,
  ColorMix = 1 << 1,
  All = AtProperty | ColorMix,
}
