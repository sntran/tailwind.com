#!/usr/bin/env node
// Usage: node gen-property-order.mjs <path of the upstream clone> <output file .rs>
// This script makes native/tailwind/src/property_order.rs from
// packages/tailwindcss/src/property-order.ts.
import fs from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'

let [clone, out] = process.argv.slice(2)
if (!clone || !out) {
  console.error('Usage: node gen-property-order.mjs <path of the upstream clone> <output file .rs>')
  process.exit(2)
}
clone = path.resolve(clone)
let require = createRequire(path.join(clone, 'packages/@tailwindcss-node/package.json'))
let { createJiti } = require('jiti')
let jiti = createJiti(path.join(clone, 'packages/tailwindcss/src/'), { moduleCache: true })
let order = (await jiti.import(path.join(clone, 'packages/tailwindcss/src/property-order.ts'))).default

let text = `//! The order of the properties for the sort of the utilities:
//! \`property-order.ts\` of Tailwind CSS.

use std::collections::HashMap;
use std::sync::OnceLock;

/// The properties, in the order of the output.
pub const GLOBAL_PROPERTY_ORDER: &[&str] = &[
${order.map((name) => `    ${JSON.stringify(name)},\n`).join('')}];

/// The index of a property in \`GLOBAL_PROPERTY_ORDER\`: the first one, when
/// the list has the property two times.
pub fn property_index(property: &str) -> Option<usize> {
    static INDEX: OnceLock<HashMap<&str, usize>> = OnceLock::new();
    INDEX
        .get_or_init(|| {
            let mut map = HashMap::new();
            for (i, p) in GLOBAL_PROPERTY_ORDER.iter().enumerate() {
                map.entry(*p).or_insert(i);
            }
            map
        })
        .get(property)
        .copied()
}
`
fs.writeFileSync(out, text)
console.log(`${out}: ${order.length} properties`)
