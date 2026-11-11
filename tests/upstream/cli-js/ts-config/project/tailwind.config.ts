import type { Config } from 'tailwindcss'
import plugin from 'tailwindcss/plugin'
import badge, { type BadgeOptions } from './plugins/badge'
import { palette, type Palette } from './theme/palette'

// An interface and a type alias: they have no JavaScript.
interface Scale {
  readonly [name: string]: string
}

type Radius = 'card' | 'pill'

// An enum has JavaScript: an object with its members.
enum Size {
  Small = '0.5rem',
  Large = '4rem',
}

const enum Gutter {
  Width = 24,
}

const spacing = {
  gutter: `${Gutter.Width}px`,
  small: Size.Small,
  large: Size.Large,
} as const satisfies Scale

function pick<T extends object, K extends keyof T>(source: T, ...keys: K[]): Pick<T, K> {
  const result = {} as Pick<T, K>
  for (const key of keys) result[key] = source[key]
  return result
}

const radius: Record<Radius, string> = { card: '0.75rem', pill: '9999px' }
const colors: Palette = palette
const options: BadgeOptions = { color: colors.brand.DEFAULT! }

let tabs = <const>[2, 4, 8]

export default {
  content: ['./index.html'],
  theme: {
    extend: {
      colors,
      spacing,
      borderRadius: pick(radius, 'card'),
      fontFamily: { display: ['Georgia', 'serif'] as string[] },
      boxShadow: { soft: '0 2px 8px rgb(0 0 0 / 0.1)' },
    },
  },
  plugins: [
    badge(options),
    plugin(function ({ matchUtilities }): void {
      matchUtilities(
        { tab: (value: string) => ({ tabSize: value }) },
        { values: Object.fromEntries(tabs.map((size: number): [string, string] => [String(size), String(size)])) },
      )
    }),
  ],
} satisfies Config
