export type Shades = { DEFAULT?: string; dark: string; light?: string }
export type Palette = Record<'brand', Shades>

export const palette: Palette = {
  brand: { DEFAULT: '#0ea5e9', dark: '#0369a1' },
}

// A type-only export.
export type { Shades as Tones }
