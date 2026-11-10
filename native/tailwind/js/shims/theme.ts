// In place of `theme.ts`: the theme is in the host (see `design-system.ts`).
export const enum ThemeOptions {
  NONE = 0,
  INLINE = 1 << 0,
  REFERENCE = 1 << 1,
  DEFAULT = 1 << 2,
  STATIC = 1 << 3,
  USED = 1 << 4,
}

export type Theme = any
export type ThemeKey = `--${string}`
