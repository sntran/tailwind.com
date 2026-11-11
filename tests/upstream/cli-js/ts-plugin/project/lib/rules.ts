export type Rules = Record<string, Record<string, string>>

export enum Level {
  None,
  Half = 50,
  Full = Half * 2,
}

export const rules: Rules = {
  '.ts-one': { color: 'rebeccapurple', 'line-height': String(Level.Full) },
}
