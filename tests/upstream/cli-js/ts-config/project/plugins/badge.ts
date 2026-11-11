import plugin from 'tailwindcss/plugin'

export interface BadgeOptions {
  color: string
  padding?: string
}

abstract class Shape {
  constructor(
    public readonly name: string,
    protected padding: string,
  ) {}
  abstract rule(color: string): Record<string, string>
}

class Badge extends Shape {
  private static count: number = 0
  declare kind: string
  rule(color: string): Record<string, string> {
    Badge.count++
    return { display: 'inline-flex', padding: this.padding, 'background-color': color, order: String(Badge.count) }
  }
}

export default plugin.withOptions<BadgeOptions>((options) => {
  const { color, padding = '0.25rem 0.5rem' } = options ?? ({} as BadgeOptions)
  const shape: Shape = new Badge('badge', padding)
  return ({ addComponents }) => {
    addComponents({ [`.${shape.name}`]: shape.rule(color) })
  }
})
