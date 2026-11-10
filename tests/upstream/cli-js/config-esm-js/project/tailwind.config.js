// ESM syntax in a .js file. There is no package.json with "type": "module".
import shared from './shared.js'

export default {
  theme: { extend: { colors: { esm: shared.color } } },
}
