//! `@plugin` and `@config` with JavaScript modules from a file system in
//! memory: the loader of modules, the modules of the program, the errors,
//! and the JSON of the tree and of the candidates.

#![cfg(tw_js)]

use flate2::read::GzDecoder;
use proptest::prelude::*;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::rc::Rc;
use tailwind::ast::{at_root, at_rule, comment, context, decl_full, style_rule, AstNode, ContextMap, ContextValue};
use tailwind::compat::js::engine::{Engine, Limits};
use tailwind::compat::js::json::{ast_from_json, ast_to_json, candidate_to_json, LocTable};
use tailwind::compat::NO_LOAD_MODULE;
use tailwind::{compile, CompileOptions, Compiler, Error, MemoryLoader, Stylesheet};

const HEAD: &str = "@tailwind utilities;\n";

fn options(loader: &Rc<MemoryLoader>) -> CompileOptions<'static> {
    CompileOptions { base: "/app/css".to_string(), test_mode: true, module_loader: Some(loader.clone()), ..Default::default() }
}

fn try_compile(css: &str, files: &[(&str, &str)]) -> (Result<Compiler, Error>, Rc<MemoryLoader>) {
    let loader = Rc::new(MemoryLoader::new(files));
    (compile(css, options(&loader)), loader)
}

/// The CSS of the candidates, for a stylesheet and its files.
fn build(css: &str, files: &[(&str, &str)], candidates: &[&str]) -> String {
    let (compiler, _) = try_compile(css, files);
    let candidates: Vec<String> = candidates.iter().map(|c| c.to_string()).collect();
    compiler.unwrap_or_else(|e| panic!("compile failed: {e}")).build(&candidates)
}

fn error(css: &str, files: &[(&str, &str)]) -> Error {
    match try_compile(css, files).0 {
        Ok(_) => panic!("compile gave no error"),
        Err(error) => error,
    }
}

/// A plugin that adds the utility `.name { color: <value> }`.
fn utility(name: &str, value: &str) -> String {
    format!("({{ addUtilities }}) => addUtilities({{ '.{name}': {{ color: {value} }} }})")
}

#[test]
fn a_commonjs_plugin() {
    let plugin = "const plugin = require('tailwindcss/plugin')\nmodule.exports = plugin(function ({ addUtilities }) {\n  addUtilities({ '.one': { color: 'red' } })\n})";
    let css = build(&format!("{HEAD}@plugin './plugin.js';"), &[("/app/css/plugin.js", plugin)], &["one", "hover:one"]);
    assert_eq!(css, ".one {\n  color: red;\n}\n@media (hover: hover) {\n  .hover\\:one:hover {\n    color: red;\n  }\n}\n");
}

#[test]
fn require_of_files_json_directories_and_packages() {
    let files = [
        ("/app/css/plugin.js", "const { color } = require('./lib/color')\nconst data = require('./data.json')\nconst dir = require('./dir')\nconst pkg = require('pkg')\nconst sub = require('@scope/pkg/sub')\nmodule.exports = ({ addUtilities }) => addUtilities({ '.all': { color, margin: data.margin, padding: dir, top: pkg.top, left: sub } })"),
        ("/app/css/lib/color.js", "exports.color = 'red'"),
        ("/app/css/data.json", "{\"margin\": \"1px\"}"),
        ("/app/css/dir/index.js", "module.exports = '2px'"),
        ("/app/node_modules/pkg/package.json", "{\"main\": \"./lib/main.js\"}"),
        ("/app/node_modules/pkg/lib/main.js", "module.exports = { top: require('./top') }"),
        ("/app/node_modules/pkg/lib/top.js", "module.exports = '3px'"),
        ("/app/node_modules/@scope/pkg/package.json", "{\"exports\": {\"./sub\": {\"import\": \"./sub.mjs\", \"require\": \"./sub.cjs\"}}}"),
        ("/app/node_modules/@scope/pkg/sub.cjs", "module.exports = '4px'"),
        ("/app/node_modules/@scope/pkg/sub.mjs", "export default 'wrong'"),
    ];
    let css = build(&format!("{HEAD}@plugin './plugin.js';"), &files, &["all"]);
    assert_eq!(css, ".all {\n  color: red;\n  margin: 1px;\n  padding: 2px;\n  top: 3px;\n  left: 4px;\n}\n");
}

#[test]
fn a_cycle_of_require_gets_the_exports_of_that_time() {
    let files = [
        ("/app/css/plugin.js", "const a = require('./a')\nmodule.exports = ({ addUtilities }) => addUtilities({ '.cycle': { color: a.fromB + '-' + a.own } })"),
        ("/app/css/a.js", "exports.own = 'a'\nconst b = require('./b')\nexports.fromB = b.seen"),
        ("/app/css/b.js", "const a = require('./a')\nexports.seen = 'b saw ' + a.own + ' ' + String(a.fromB)"),
    ];
    let css = build(&format!("{HEAD}@plugin './plugin.js';"), &files, &["cycle"]);
    assert_eq!(css, ".cycle {\n  color: b saw a undefined-a;\n}\n");
}

#[test]
fn an_es_module_plugin_with_imports() {
    let files = [
        ("/app/css/plugin.mjs", "import plugin from 'tailwindcss/plugin'\nimport colors from 'tailwindcss/colors'\nimport { shade, name as other } from './lib/shade.mjs'\nimport common, { left } from './common.cjs'\nimport data from './data.json'\nimport * as path from 'node:path'\nexport default plugin(({ addUtilities }) => addUtilities({ '.esm': { color: colors.red[shade], top: common.top, left, margin: data.margin, content: JSON.stringify(path.join('a', other)) } }))"),
        ("/app/css/lib/shade.mjs", "export const shade = 500\nexport const name = 'b'"),
        ("/app/css/common.cjs", "module.exports = { top: '1px', left: '2px' }"),
        ("/app/css/data.json", "{\"margin\": \"3px\"}"),
    ];
    let css = build(&format!("{HEAD}@plugin './plugin.mjs';"), &files, &["esm"]);
    assert_eq!(css, ".esm {\n  color: oklch(63.7% 0.237 25.331);\n  top: 1px;\n  left: 2px;\n  margin: 3px;\n  content: \"a/b\";\n}\n");
}

#[test]
fn the_format_of_a_js_file() {
    // The syntax of an ES module in a `.js` file with no `type`.
    let detected = [("/app/css/plugin.js", "export default ({ addUtilities }) => addUtilities({ '.detected': { color: 'red' } })")];
    assert_eq!(build(&format!("{HEAD}@plugin './plugin.js';"), &detected, &["detected"]), ".detected {\n  color: red;\n}\n");

    // `"type": "module"` in the nearest package.json.
    let typed = [
        ("/app/package.json", "{\"type\": \"module\"}"),
        ("/app/css/plugin.js", "const value = 'blue'\nexport default ({ addUtilities }) => addUtilities({ '.typed': { color: value } })"),
    ];
    assert_eq!(build(&format!("{HEAD}@plugin './plugin.js';"), &typed, &["typed"]), ".typed {\n  color: blue;\n}\n");
}

#[test]
fn require_of_an_es_module() {
    let files = [
        ("/app/css/plugin.cjs", "const esm = require('./values.mjs')\nmodule.exports = ({ addUtilities }) => addUtilities({ '.mixed': { color: esm.color, top: esm.default.top } })"),
        ("/app/css/values.mjs", "export const color = 'green'\nexport default { top: '1px' }"),
    ];
    assert_eq!(build(&format!("{HEAD}@plugin './plugin.cjs';"), &files, &["mixed"]), ".mixed {\n  color: green;\n  top: 1px;\n}\n");
}

#[test]
fn a_commonjs_bundle_of_an_es_module_gives_its_default() {
    let plugin = format!("Object.defineProperty(exports, '__esModule', {{ value: true }})\nexports.default = {}", utility("bundle", "'red'"));
    assert_eq!(build(&format!("{HEAD}@plugin './plugin.js';"), &[("/app/css/plugin.js", &plugin)], &["bundle"]), ".bundle {\n  color: red;\n}\n");
}

#[test]
fn the_modules_of_the_program() {
    let plugin = "const plugin = require('tailwindcss/plugin.js')\nconst colors = require('tailwindcss/colors')\nconst defaultTheme = require('tailwindcss/defaultTheme')\nconst flatten = require('tailwindcss/lib/util/flattenColorPalette')\nconst pkg = require('tailwindcss/package.json')\nconst util = require('node:util')\nmodule.exports = plugin.withOptions((options) => ({ addUtilities }) => addUtilities({ '.builtin': { color: colors.blue[500], 'font-family': defaultTheme.fontFamily.mono[0], top: flatten({ a: { b: '1px' } })['a-b'], content: JSON.stringify(pkg.name + options.suffix), left: typeof util.deprecate } }))";
    let css = build(&format!("{HEAD}@plugin './plugin.js' {{ suffix: '!'; }}"), &[("/app/css/plugin.js", plugin)], &["builtin"]);
    assert_eq!(
        css,
        ".builtin {\n  color: oklch(62.3% 0.214 259.815);\n  font-family: ui-monospace;\n  top: 1px;\n  content: \"tailwindcss!\";\n  left: function;\n}\n"
    );
}

#[test]
fn fs_and_path_read_through_the_loader() {
    let plugin = "const fs = require('fs')\nconst path = require('path')\nmodule.exports = ({ matchUtilities }) => {\n  let dir = path.join(__dirname, '../icons')\n  let values = {}\n  for (let file of fs.readdirSync(dir)) values[path.basename(file, '.svg')] = path.join(dir, file)\n  matchUtilities({ icon: (file) => ({ content: JSON.stringify(fs.readFileSync(file).toString().trim()), top: String(fs.existsSync(file)) + String(fs.existsSync(file + 'x')) + fs.statSync(dir).isDirectory() + fs.statSync(file).isFile(), left: fs.readFileSync(file, 'utf8').length + 'px', color: path.extname(__filename) }) }, { values })\n}";
    let files = [("/app/css/plugin.js", plugin), ("/app/icons/b.svg", "<b/>\n"), ("/app/icons/a.svg", "<a/>\n")];
    let css = build(&format!("{HEAD}@plugin './plugin.js';"), &files, &["icon-a", "icon-b", "icon-c"]);
    assert_eq!(
        css,
        ".icon-a {\n  content: \"<a/>\";\n  top: truefalsetruetrue;\n  left: 5px;\n  color: .js;\n}\n.icon-b {\n  content: \"<b/>\";\n  top: truefalsetruetrue;\n  left: 5px;\n  color: .js;\n}\n"
    );
}

#[test]
fn a_plugin_cannot_write_files_and_has_no_other_modules() {
    let write = error(&format!("{HEAD}@plugin './plugin.js';"), &[("/app/css/plugin.js", "require('fs').writeFileSync('/x', '1')\nmodule.exports = () => {}")]);
    assert_eq!(write.message, "fs.writeFileSync is not permitted: the JavaScript of a plugin or config has read-only access to files.");
    // A module for the network loads, and each of its functions throws.
    let network = error(&format!("{HEAD}@plugin './plugin.js';"), &[("/app/css/plugin.js", "const http = require('http')\nmodule.exports = () => { http.get('http://example.com/') }")]);
    assert_eq!(network.message, "http.get is not permitted: the JavaScript of a plugin or config has no network access.");
    let missing = error(&format!("{HEAD}@plugin './plugin.js';"), &[("/app/css/plugin.js", "require('crypto')\nmodule.exports = () => {}")]);
    assert_eq!(missing.message, "Cannot find module 'crypto'\nRequire stack:\n- /app/css/plugin.js");
    let process = error(&format!("{HEAD}@plugin './plugin.js';"), &[("/app/css/plugin.js", "require('child_process').execSync('ls')")]);
    assert_eq!(process.message, "child_process.execSync is not permitted: the JavaScript of a plugin or config cannot start a process.");
}

#[test]
fn the_compiler_reports_the_module_files_and_the_console() {
    let files = [
        ("/app/css/plugin.js", "const lib = require('./lib')\nconst pkg = require('pkg')\nconsole.log('loaded', { a: 1 }.a, lib)\nconsole.warn('careful')\nmodule.exports = () => {}"),
        ("/app/css/lib.js", "module.exports = 'lib'"),
        ("/app/css/tailwind.config.mjs", "import './theme.mjs'\nexport default {}"),
        ("/app/css/theme.mjs", "export default {}"),
        ("/app/node_modules/pkg/index.js", "module.exports = 1"),
    ];
    let (compiler, loader) = try_compile(&format!("{HEAD}@plugin './plugin.js';\n@config './tailwind.config.mjs';"), &files);
    compiler.unwrap();
    let mut dependencies = loader.dependencies.borrow().clone();
    dependencies.sort();
    dependencies.dedup();
    // The files of a package in node_modules are not dependencies.
    assert_eq!(dependencies, ["/app/css/lib.js", "/app/css/plugin.js", "/app/css/tailwind.config.mjs", "/app/css/theme.mjs"]);
    assert_eq!(*loader.console.borrow(), ["loaded 1 lib", "careful"]);
}

#[test]
fn a_config_file() {
    let config = "module.exports = {\n  darkMode: 'selector',\n  theme: { extend: { colors: { brand: { DEFAULT: '#f00', dark: '#900' } }, spacing: { gutter: '13px' } } },\n  plugins: [require('./plugin')],\n}";
    let files = [("/app/css/tailwind.config.js", config), ("/app/css/plugin.js", &format!("module.exports = {}", utility("from-config", "'blue'")) as &str)];
    let css = build(&format!("{HEAD}@config './tailwind.config.js';"), &files, &["bg-brand", "text-brand-dark", "p-gutter", "from-config", "dark:from-config"]);
    assert_eq!(
        css,
        ".bg-brand {\n  background-color: #f00;\n}\n.p-gutter {\n  padding: 13px;\n}\n.from-config {\n  color: blue;\n}\n.text-brand-dark {\n  color: #900;\n}\n.dark\\:from-config:where(.dark, .dark *) {\n  color: blue;\n}\n"
    );
}

#[test]
fn the_errors_of_the_loader() {
    let css = format!("{HEAD}@plugin './plugin.js';");
    assert_eq!(error(&css, &[]).message, "Can't resolve './plugin.js' in '/app/css'");
    assert_eq!(error(&format!("{HEAD}@plugin 'some-package';"), &[]).message, "Can't resolve 'some-package' in '/app/css'");

    let typescript = error(&format!("{HEAD}@config './view.tsx';"), &[("/app/css/view.tsx", "export default {}")]);
    assert_eq!(typescript.message, "Cannot load '/app/css/view.tsx': a TypeScript module with JSX (.tsx) is not supported. Use a .ts file.");

    let thrown = error(&css, &[("/app/css/plugin.js", "throw new TypeError('bad module')")]);
    assert_eq!((thrown.message.as_str(), thrown.name.as_ref()), ("bad module", "TypeError"));

    let in_plugin = error(&css, &[("/app/css/plugin.js", "module.exports = () => { throw new Error('bad plugin') }")]);
    assert_eq!((in_plugin.message.as_str(), in_plugin.name.as_ref()), ("bad plugin", "Error"));

    let syntax = error(&css, &[("/app/css/plugin.cjs", ""), ("/app/css/plugin.js", "module.exports = {\n  a: 1\n  b: 2\n}")]);
    assert_eq!(syntax.name.as_ref(), "SyntaxError");
    assert!(syntax.message.contains("/app/css/plugin.js:3"), "{}", syntax.message);

    let missing = error(&css, &[("/app/css/plugin.js", "require('./not-there.js')")]);
    assert_eq!(missing.message, "Cannot find module './not-there.js'\nRequire stack:\n- /app/css/plugin.js");

    let options = error(&format!("{HEAD}@plugin './plugin.js' {{ a: 1; }}"), &[("/app/css/plugin.js", "module.exports = () => {}")]);
    assert_eq!(options.message, "The plugin \"./plugin.js\" does not accept options");

    // With no loader: the error of upstream with no `loadModule`.
    let no_loader = compile(&css, CompileOptions { test_mode: true, ..Default::default() }).err().unwrap();
    assert_eq!(no_loader.message, NO_LOAD_MODULE);
}

#[test]
fn an_exception_of_a_plugin_function_in_a_build() {
    let plugin = "module.exports = ({ matchUtilities }) => matchUtilities({ boom: (value) => { if (value === 'x') throw new RangeError('bad value ' + value); return { color: value } } }, { values: { x: 'x', red: 'red' } })";
    let (compiler, _) = try_compile(&format!("{HEAD}@plugin './plugin.js';"), &[("/app/css/plugin.js", plugin)]);
    let mut compiler = compiler.unwrap();
    assert_eq!(compiler.try_build(&["boom-red".to_string()]).unwrap(), ".boom-red {\n  color: red;\n}\n");
    let error = compiler.try_build(&["boom-x".to_string()]).unwrap_err();
    assert_eq!((error.message.as_str(), error.name.as_ref()), ("bad value x", "RangeError"));
    // `build` gives the CSS without the candidate of the failure.
    assert_eq!(compiler.build(&["boom-x".to_string()]), ".boom-red {\n  color: red;\n}\n");
    // A plugin cannot register a utility in a build.
    let late = "module.exports = (api) => api.matchUtilities({ late: () => { api.addUtilities({ '.x': { color: 'red' } }); return {} } }, { values: { a: 'a' } })";
    let (compiler, _) = try_compile(&format!("{HEAD}@plugin './plugin.js';"), &[("/app/css/plugin.js", late)]);
    let error = compiler.unwrap().try_build(&["late-a".to_string()]).unwrap_err();
    assert_eq!(error.message, "A plugin cannot change the design system during a build.");
}

#[test]
fn the_environment_of_a_module() {
    let plugin = "module.exports = ({ addUtilities }) => addUtilities({ '.env': { content: JSON.stringify([typeof process.env, process.env.HOME === undefined, typeof setTimeout, typeof fetch, typeof globalThis.std, atob('YWJj'), btoa('abc'), typeof structuredClone, __filename, module.id === __filename]) } })";
    let css = build(&format!("{HEAD}@plugin './plugin.js';"), &[("/app/css/plugin.js", plugin)], &["env"]);
    assert_eq!(
        css,
        ".env {\n  content: [\"object\",true,\"function\",\"function\",\"undefined\",\"abc\",\"YWJj\",\"function\",\"/app/css/plugin.js\",true];\n}\n"
    );
}

#[test]
fn one_plugin_file_loads_one_time() {
    let files = [
        ("/app/css/counter.js", "globalThis.count = (globalThis.count ?? 0) + 1\nmodule.exports = () => globalThis.count"),
        ("/app/css/a.js", "const count = require('./counter')\nmodule.exports = ({ addUtilities }) => addUtilities({ '.a': { top: count() + 'px' } })"),
        ("/app/css/b.mjs", "import count from './counter.js'\nexport default ({ addUtilities }) => addUtilities({ '.b': { top: count() + 'px' } })"),
    ];
    let css = build(&format!("{HEAD}@plugin './a.js';\n@plugin './b.mjs';"), &files, &["a", "b"]);
    assert_eq!(css, ".a {\n  top: 1px;\n}\n.b {\n  top: 1px;\n}\n");
}

// ---------------------------------------------------------------------------
// The two paths of the compatibility layer.

fn tailwind_stylesheet(id: &str, _base: &str) -> Result<Stylesheet, Error> {
    let name = match id {
        "tailwindcss" => "index.css",
        id => id.strip_prefix("tailwindcss/").or_else(|| id.strip_prefix("./")).unwrap_or(id),
    };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/upstream/css").join(name);
    let content = std::fs::read_to_string(path).map_err(|e| Error::new(e.to_string()))?;
    Ok(Stylesheet { path: format!("/tailwindcss/{name}"), base: "/tailwindcss".to_string(), content })
}

/// A stylesheet with no `@plugin` uses the Rust code for the legacy
/// utilities and for `theme(…)` with a key path. With a plugin, the
/// JavaScript of Tailwind CSS does this work. The two paths must agree.
#[test]
fn the_rust_path_and_the_javascript_path_agree() {
    let input = "@import 'tailwindcss/theme.css' layer(theme);\n@import 'tailwindcss/utilities.css' layer(utilities);\n@theme { --color-brand: #f00; --shadow: 0 1px red; --text-huge: 9rem; --text-huge--line-height: 1; }\n.a {\n  color: theme(colors.red.500);\n  background: theme(colors.red.500 / 50%);\n  margin: theme(spacing.4);\n  padding: theme(spacing[1.5]);\n  width: theme(screens.md);\n  font-family: theme(fontFamily.sans);\n  font-size: theme(fontSize.xl);\n  line-height: theme(fontSize.huge[1].lineHeight);\n  box-shadow: theme(boxShadow.DEFAULT);\n  border-radius: theme(borderRadius.lg);\n  transition: theme(transitionDuration.DEFAULT) theme(transitionTimingFunction.DEFAULT);\n  outline-color: theme(colors.brand);\n  z-index: theme(zIndex.50, 1);\n  inset: theme(--spacing);\n  top: theme(missing.key, 3px);\n  animation: theme(animation.spin);\n  letter-spacing: theme(letterSpacing.tight);\n  border-width: theme(borderWidth.DEFAULT);\n  opacity: theme(opacity.50);\n  max-width: theme(maxWidth.xl);\n  aspect-ratio: theme(aspectRatio.video);\n}\n";
    let candidates: Vec<String> = [
        "bg-gradient-to-r", "bg-left-top", "object-right-bottom", "max-w-screen-md", "overflow-ellipsis", "decoration-slice",
        "flex-shrink", "flex-shrink-0", "flex-grow", "flex-grow-[2]", "order-none", "break-words", "start-auto", "end-full", "-start-px",
        "bg-[theme(colors.blue.500)]", "p-[theme(spacing.2)]", "text-[theme(fontSize.sm)]", "w-[theme(spacing.4/50%)]",
        "shadow-[0_0_theme(spacing.1)_theme(colors.red.500/20%)]", "md:hover:underline", "dark:flex", "container", "bg-red-500/50",
    ]
    .iter()
    .map(|c| c.to_string())
    .collect();

    let rust = compile(input, CompileOptions { load_stylesheet: Some(&tailwind_stylesheet), test_mode: true, ..Default::default() })
        .unwrap()
        .build(&candidates);

    let loader = Rc::new(MemoryLoader::new(&[("/noop.js", "module.exports = () => {}")]));
    let with_plugin = format!("{input}@plugin '/noop.js';\n");
    let javascript = compile(
        &with_plugin,
        CompileOptions { load_stylesheet: Some(&tailwind_stylesheet), test_mode: true, module_loader: Some(loader), ..Default::default() },
    )
    .unwrap()
    .build(&candidates);

    assert!(rust.contains("max-width: var(--breakpoint-md)") && rust.contains("background-position: left top"), "{rust}");
    assert_eq!(rust, javascript);
}

// ---------------------------------------------------------------------------
// The JSON of the candidates.

#[derive(serde::Deserialize)]
struct CandidateRecord {
    candidate: String,
    parsed: serde_json::Value,
}

/// A plugin function gets a candidate as JSON. The JSON must be the JSON of
/// `parseCandidate` of Tailwind CSS (`tests/upstream/candidates.jsonl.gz`).
#[test]
fn the_json_of_a_candidate_is_the_json_of_upstream() {
    let input = "@layer theme, base, components, utilities;\n@import \"tailwindcss/theme.css\" layer(theme);\n@import \"tailwindcss/utilities.css\" layer(utilities);";
    let compiler =
        compile(input, CompileOptions { load_stylesheet: Some(&tailwind_stylesheet), test_mode: true, ..Default::default() }).unwrap();
    let design_system = compiler.design_system();

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/upstream/candidates.jsonl.gz");
    let file = std::fs::File::open(path).unwrap();
    let mut total = 0;
    for line in BufReader::new(GzDecoder::new(file)).lines() {
        let record: CandidateRecord = serde_json::from_str(&line.unwrap()).unwrap();
        let parsed: Vec<serde_json::Value> =
            design_system.parse_candidate(&record.candidate).iter().map(|candidate| candidate_to_json(candidate)).collect();
        assert_eq!(serde_json::Value::Array(parsed), record.parsed, "{}", record.candidate);
        total += 1;
    }
    assert!(total > 10_000, "{total} candidates");
}

// ---------------------------------------------------------------------------
// Properties.

fn text() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z0-9 :.#>&@(),_-]{0,12}",
        // All of Unicode: control characters, quotes, astral characters.
        any::<String>().prop_map(|text| text.replace('\0', "")),
        Just("\"quoted\" \\ back\\slash \n line \u{2028} \u{1F600}".to_string()),
    ]
}

fn context_map() -> impl Strategy<Value = ContextMap> {
    prop::collection::btree_map(
        "[a-z]{1,6}",
        prop_oneof![text().prop_map(ContextValue::Str), any::<bool>().prop_map(ContextValue::Bool)],
        0..3,
    )
}

fn tree() -> impl Strategy<Value = Vec<AstNode>> {
    let leaf = prop_oneof![
        (text(), prop::option::of(text()), any::<bool>()).prop_map(|(property, value, important)| decl_full(property, value, important)),
        text().prop_map(comment),
    ];
    let node = leaf.prop_recursive(4, 24, 4, |inner| {
        let nodes = prop::collection::vec(inner, 0..4);
        prop_oneof![
            (text(), nodes.clone()).prop_map(|(selector, nodes)| style_rule(selector, nodes)),
            (text(), text(), nodes.clone()).prop_map(|(name, params, nodes)| at_rule(name, params, nodes)),
            (context_map(), nodes.clone()).prop_map(|(map, nodes)| context(map, nodes)),
            nodes.prop_map(at_root),
        ]
    });
    prop::collection::vec(node, 0..4)
}

thread_local! {
    /// An engine with a function that reads JSON and writes it again.
    static ECHO: Engine = {
        let engine = Engine::new(Limits::default(), Box::new(|_, _| Ok(None))).unwrap();
        engine.eval("globalThis.echo = (text) => JSON.stringify(JSON.parse(text))", "echo.js").unwrap();
        engine
    };
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, failure_persistence: None, ..ProptestConfig::default() })]

    /// A tree is the same after the change to JSON and back.
    #[test]
    fn a_tree_is_the_same_after_json(tree in tree()) {
        let json = ast_to_json(&tree, &LocTable::default());
        prop_assert_eq!(ast_from_json(&json, &LocTable::default()).unwrap(), tree.clone());
        // Also as text, as it goes to the engine.
        let parsed: serde_json::Value = serde_json::from_str(&json.to_string()).unwrap();
        prop_assert_eq!(ast_from_json(&parsed, &LocTable::default()).unwrap(), tree);
    }

    /// A tree is the same after a pass through `JSON.parse` and
    /// `JSON.stringify` of the engine.
    #[test]
    fn a_tree_is_the_same_after_the_engine(tree in tree()) {
        let text = ast_to_json(&tree, &LocTable::default()).to_string();
        let echoed = ECHO.with(|engine| engine.call("echo", &text)).unwrap().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&echoed).unwrap();
        prop_assert_eq!(ast_from_json(&parsed, &LocTable::default()).unwrap(), tree);
    }

    /// Text is the same after a pass through the host function.
    #[test]
    fn text_is_the_same_after_the_host(text in text()) {
        let engine = Engine::new(Limits::default(), Box::new(|_, text| Ok(Some(text.to_string())))).unwrap();
        engine.eval("globalThis.through = (text) => __host('echo', text)", "through.js").unwrap();
        prop_assert_eq!(engine.call("through", &text).unwrap().unwrap(), text);
    }
}

/// The source locations of a tree stay the same after the change to JSON
/// and back, also after a pass through the engine. A node that JavaScript
/// makes has no location.
#[test]
fn the_locations_are_the_same_after_json() {
    type Range = Option<(usize, usize)>;
    fn locations(nodes: &[tailwind::ast::AstNode], out: &mut Vec<(Range, Range)>) {
        for node in nodes {
            out.push((node.src().map(|src| (src.start, src.end)), node.dst().map(|dst| (dst.start, dst.end))));
            if let Some(children) = node.children() {
                locations(children, out);
            }
        }
    }

    let css = "@plugin \"./a.js\";\n.a { color: red; @media print { b: c } }\n/* d */\n";
    let tree = tailwind::css_parser::parse_with_source(css, Some("app.css")).unwrap();
    let table = LocTable::default();
    let text = ast_to_json(&tree, &table).to_string();
    let echoed = ECHO.with(|engine| engine.call("echo", &text)).unwrap().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&echoed).unwrap();
    let back = ast_from_json(&parsed, &table).unwrap();

    let (mut before, mut after) = (Vec::new(), Vec::new());
    locations(&tree, &mut before);
    locations(&back, &mut after);
    assert!(before.iter().all(|(src, dst)| src.is_some() && dst.is_some()));
    assert_eq!(before, after);
    // The same source object: the locations are equal.
    assert_eq!(tree[0].src(), back[0].src());

    let made = ast_from_json(&serde_json::json!([{ "kind": "declaration", "property": "a", "value": "b" }]), &table).unwrap();
    assert!(made[0].src().is_none() && made[0].dst().is_none());
}

#[test]
fn typescript_modules_lose_their_types() {
    let files = [
        (
            "/app/css/tailwind.config.ts",
            "import type { Config } from 'tailwindcss'\nimport plugin from 'tailwindcss/plugin'\nimport { size, type Size } from './lib/size'\nimport rule from './rule.cts'\ninterface Theme { spacing: Record<string, Size> }\nenum Color { Red = '#f00' }\nnamespace Extra { export const width = '9px'; export function twice<T>(value: T): T[] { return [value, value] } }\nconst theme = { spacing: { big: size } } satisfies Theme\nexport default {\n  theme: { extend: { ...theme, colors: { brand: Color.Red as string } } },\n  plugins: [plugin(({ addUtilities }): void => addUtilities({ '.ts': { ...rule, width: Extra.width, content: JSON.stringify(Extra.twice<number>(1)) } }))],\n} satisfies Config\n",
        ),
        ("/app/css/lib/size.ts", "export type Size = string\nexport const size: Size = '10rem'\n"),
        ("/app/css/rule.cts", "const rule: { [key: string]: string } = { top: '1px' }\nexport = rule\n"),
    ];
    let css = build(&format!("{HEAD}@config './tailwind.config.ts';"), &files, &["p-big", "text-brand", "ts"]);
    assert_eq!(
        css,
        ".ts {\n  top: 1px;\n  width: 9px;\n  content: [1,1];\n}\n.p-big {\n  padding: 10rem;\n}\n.text-brand {\n  color: #f00;\n}\n"
    );

    // A `.ts` file in the CommonJS style, also in a package of ES modules.
    let files = [
        ("/app/css/package.json", "{\"type\": \"module\"}"),
        ("/app/css/plugin.ts", "const value: string = 'red'\nmodule.exports = ({ addUtilities }: any) => addUtilities({ '.cjs-ts': { color: value } })\n"),
        ("/app/css/esm.ts", "const value: string = 'blue'\nexport default ({ addUtilities }: any) => addUtilities({ '.esm-ts': { color: value } })\n"),
        ("/app/css/tailwind.config.js", "import width from './width.ts'\nexport default { theme: { extend: { spacing: { w: width } } } }\n"),
        ("/app/css/width.ts", "export default '7px' as string\n"),
    ];
    let css = build(&format!("{HEAD}@plugin './plugin.ts';@plugin './esm.ts';@config './tailwind.config.js';"), &files, &["cjs-ts", "esm-ts", "m-w"]);
    assert_eq!(css, ".m-w {\n  margin: 7px;\n}\n.cjs-ts {\n  color: red;\n}\n.esm-ts {\n  color: blue;\n}\n");

    // The record of the dependencies has the TypeScript files.
    let (compiler, loader) = try_compile(&format!("{HEAD}@config './tailwind.config.js';"), &files);
    compiler.unwrap();
    assert!(loader.dependencies.borrow().contains(&"/app/css/width.ts".to_string()));

    // A syntax error has the path of the file.
    let syntax = error(&format!("{HEAD}@config './bad.ts';"), &[("/app/css/bad.ts", "export default {\n  theme: { extend: { }\n} satisfies Config\n")]);
    assert_eq!(syntax.name, "SyntaxError");
    assert_eq!(syntax.message, "Unexpected token, expected \",\" (4:1)\n/app/css/bad.ts:4:1");
}

#[test]
fn the_plugins_of_the_standalone_program_are_in_the_program() {
    // No `node_modules` directory.
    let css = build(&format!("{HEAD}@plugin '@tailwindcss/aspect-ratio';"), &[], &["aspect-w-4", "aspect-h-3"]);
    assert!(css.contains(".aspect-h-3 {\n  --tw-aspect-h: 3;\n}"), "{css}");
    let css = build(&format!("{HEAD}@plugin '@tailwindcss/forms' {{ strategy: 'class'; }}"), &[], &["form-input"]);
    assert!(css.contains(".form-input {"), "{css}");
    let css = build(&format!("{HEAD}@plugin '@tailwindcss/typography';"), &[], &["prose"]);
    assert!(css.contains(".prose {") && css.contains(":where(blockquote)"), "{css}");

    // A copy in `node_modules` does not win for `@plugin`, as in the
    // standalone program. `require` in a module finds the copy.
    let files = [
        ("/app/node_modules/@tailwindcss/aspect-ratio/index.js", "module.exports = ({ addUtilities }) => addUtilities({ '.own-copy': { color: 'red' } })"),
        ("/app/css/tailwind.config.js", "module.exports = { plugins: [require('@tailwindcss/aspect-ratio')] }"),
    ];
    let css = build(&format!("{HEAD}@plugin '@tailwindcss/aspect-ratio';"), &files, &["aspect-h-3", "own-copy"]);
    assert_eq!(css, ".aspect-h-3 {\n  --tw-aspect-h: 3;\n}\n");
    let css = build(&format!("{HEAD}@config './tailwind.config.js';"), &files, &["aspect-h-3", "own-copy"]);
    assert_eq!(css, ".own-copy {\n  color: red;\n}\n");

    // `require` of such a plugin with no `node_modules` is an error, as in
    // the standalone program.
    let missing = error(&format!("{HEAD}@config './tailwind.config.js';"), &[files[1]]);
    assert!(missing.message.starts_with("Cannot find module '@tailwindcss/aspect-ratio'"), "{}", missing.message);
    // The other packages of the scope are not in the program.
    assert_eq!(error(&format!("{HEAD}@plugin '@tailwindcss/container-queries';"), &[]).message, "Can't resolve '@tailwindcss/container-queries' in '/app/css'");
}

#[test]
fn a_dynamic_import_in_a_commonjs_file_resolves_from_the_file() {
    let files = [
        ("/app/css/plugins/plugin.js", "module.exports = ({ addUtilities }) => {}\nmodule.exports.ready = import('./lib/value.mjs').then((module) => module.default)"),
        ("/app/css/plugins/lib/value.mjs", "import data from '../data.json' with { type: 'json' }\nexport default data.color"),
        ("/app/css/plugins/data.json", "{\"color\": \"teal\"}"),
        (
            "/app/css/main.mjs",
            "import plugin from './plugins/plugin.js'\nconst color = await plugin.ready\nexport default ({ addUtilities }) => addUtilities({ '.dynamic': { color } })",
        ),
    ];
    let css = build(&format!("{HEAD}@plugin './main.mjs';"), &files, &["dynamic"]);
    assert_eq!(css, ".dynamic {\n  color: teal;\n}\n");
}

#[test]
fn the_imports_of_a_package_json() {
    let files = [
        (
            "/app/package.json",
            "{\"imports\": {\"#theme\": \"./theme/index.js\", \"#lib/*\": \"./lib/*.js\", \"#dep\": \"dep\", \"#kind\": {\"import\": \"./kind/esm.mjs\", \"require\": \"./kind/cjs.cjs\"}}}",
        ),
        ("/app/theme/index.js", "module.exports = { color: 'navy' }"),
        ("/app/lib/size.js", "module.exports = '3px'"),
        ("/app/kind/esm.mjs", "export default 'import'"),
        ("/app/kind/cjs.cjs", "module.exports = 'require'"),
        ("/app/node_modules/dep/index.js", "module.exports = '5px'"),
        (
            "/app/css/plugin.js",
            "const theme = require('#theme')\nmodule.exports = ({ addUtilities }) => addUtilities({ '.imports': { color: theme.color, top: require('#lib/size'), left: require('#dep'), content: JSON.stringify(require('#kind')) } })",
        ),
        ("/app/css/plugin.mjs", "import kind from '#kind'\nimport size from '#lib/size'\nexport default ({ addUtilities }) => addUtilities({ '.imports-esm': { top: size, content: JSON.stringify(kind) } })"),
    ];
    let css = build(&format!("{HEAD}@plugin './plugin.js';@plugin './plugin.mjs';"), &files, &["imports", "imports-esm"]);
    assert_eq!(
        css,
        ".imports {\n  color: navy;\n  top: 3px;\n  left: 5px;\n  content: \"require\";\n}\n.imports-esm {\n  top: 3px;\n  content: \"import\";\n}\n"
    );
    let missing = error(&format!("{HEAD}@plugin './plugin.js';"), &[("/app/package.json", "{\"imports\": {}}"), ("/app/css/plugin.js", "require('#theme')")]);
    assert!(missing.message.starts_with("Cannot find module '#theme'"), "{}", missing.message);
}

#[test]
fn an_error_has_its_chain_of_causes() {
    let css = format!("{HEAD}@plugin './plugin.js';");
    let plugin = "const first = new RangeError('the first cause')\nconst second = new TypeError('the second cause', { cause: first })\nthrow new Error('the module fails', { cause: second })";
    let error = error(&css, &[("/app/css/plugin.js", plugin)]);
    assert_eq!(error.message, "the module fails");
    assert_eq!(error.causes, ["TypeError: the second cause", "RangeError: the first cause"]);

    // A cause that is not an error, and a chain that is a circle.
    let plugin = "const a = new Error('a')\nconst b = new Error('b', { cause: a })\na.cause = b\nmodule.exports = () => { throw new Error('top', { cause: b }) }";
    let error = self::error(&css, &[("/app/css/plugin.js", plugin)]);
    assert_eq!(error.causes, ["Error: b", "Error: a"]);
    let error = self::error(&css, &[("/app/css/plugin.js", "throw new Error('text cause', { cause: 'only text' })")]);
    assert_eq!(error.causes, ["only text"]);
    let error = self::error(&css, &[("/app/css/plugin.js", "throw new Error('no cause')")]);
    assert!(error.causes.is_empty());

    // An exception of a plugin function in a build.
    let plugin = "module.exports = ({ matchUtilities }) => matchUtilities({ boom: () => { throw new Error('in a build', { cause: new Error('the reason') }) } })";
    let (compiler, _) = try_compile(&css, &[("/app/css/plugin.js", plugin)]);
    let error = compiler.unwrap().try_build(&["boom-[1px]".to_string()]).unwrap_err();
    assert_eq!((error.message.as_str(), error.causes.as_slice()), ("in a build", ["Error: the reason".to_string()].as_slice()));
}
