//! The parts of Node.js that the JavaScript of a plugin or config gets,
//! against the record of Node.js 22 (`tests/fixtures/node_api.json`, from
//! `tests/upstream/tools/gen-node-api.mjs`): `Buffer`, `URL`, the text
//! encoders, the timers, `structuredClone`, `process`, and the modules
//! `os`, `url`, `util`, `assert`, `fs`, `module`. Then the resolver of
//! module names against `require.resolve` and `import.meta.resolve` of
//! Node.js, and the rules: no write access, no network, no process.

#![cfg(tw_js)]

use serde_json::Value;
use std::path::PathBuf;
use std::rc::Rc;
use tailwind::compat::js::resolve::{dirname, resolve, ResolveKind};
use tailwind::{compile, CompileOptions, MemoryLoader};

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn record() -> Value {
    serde_json::from_str(&fixture("node_api.json")).unwrap()
}

/// Run a module in the engine, and give the lines of `console.log`.
fn run(loader: MemoryLoader, module: &str) -> Vec<String> {
    let mut loader = loader;
    loader.files.insert("/app/css/main.mjs".to_string(), module.to_string());
    let loader = Rc::new(loader);
    let options = CompileOptions { base: "/app/css".to_string(), test_mode: true, module_loader: Some(loader.clone()), ..Default::default() };
    if let Err(error) = compile("@plugin './main.mjs';", options) {
        panic!("the module failed: {}: {}", error.name, error.message);
    }
    let lines = loader.console.borrow().clone();
    lines
}

/// One value of JavaScript, as JSON text.
fn eval(expression: &str) -> Value {
    let module = format!(
        "import {{ createRequire }} from 'node:module'\nconst require = createRequire(import.meta.url)\nconsole.log(JSON.stringify(await (async () => ({expression}))()))\nexport default () => {{}}\n"
    );
    let lines = run(MemoryLoader::default(), &module);
    serde_json::from_str(lines.last().expect("no output")).unwrap()
}

#[test]
fn the_apis_of_node_give_the_values_of_node() {
    const DIR: &str = "/app/css/data";
    let mut loader = MemoryLoader::new(&[
        ("/app/css/cases.cjs", &fixture("node_api_cases.cjs")),
        ("/app/css/data/text.txt", "Text with é, € and 𝄞.\n"),
        ("/app/css/data/sub/a.txt", "a"),
        ("/app/css/data/sub/b.json", "{\"list\": [1, 2, 3], \"name\": \"b\"}\n"),
        ("/app/css/data/sub/deep/c.txt", "c"),
    ]);
    // A file that is not UTF-8 text.
    loader.files.insert(format!("{DIR}/bytes.bin"), String::from_utf8_lossy(&[0x00, 0xff, 0x80, 0x41, 0xc3, 0x28, 0x0a]).into_owned());
    loader.binary.insert(format!("{DIR}/bytes.bin"), vec![0x00, 0xff, 0x80, 0x41, 0xc3, 0x28, 0x0a]);
    loader.env = vec![
        ("NODE_API_TEST".to_string(), "value".to_string()),
        ("HOME".to_string(), "/home/test".to_string()),
        ("TMPDIR".to_string(), "/tmp/test/".to_string()),
    ];

    let module = format!(
        "import run from './cases.cjs'\nprocess.noDeprecation = true\nconsole.log(JSON.stringify(await run({DIR:?})))\nexport default () => {{}}\n"
    );
    let lines = run(loader, &module);
    let actual: Value = serde_json::from_str(lines.last().expect("no output")).unwrap();

    let expected = record();
    let expected = expected["results"].as_object().unwrap();
    assert!(expected.len() > 100);
    let mut failed = Vec::new();
    for (name, value) in expected {
        // The record has `<dir>` at the place of the directory.
        let value: Value = serde_json::from_str(&value.to_string().replace("<dir>", DIR)).unwrap();
        let got = actual.get(name).cloned().unwrap_or(Value::Null);
        if got != value {
            failed.push(format!("{name}\n  Node.js: {value}\n  here:    {got}"));
        }
    }
    assert!(failed.is_empty(), "{} of {} cases are different:\n{}", failed.len(), expected.len(), failed.join("\n"));
}

#[test]
fn the_resolver_finds_the_files_that_node_finds() {
    const DIR: &str = "/proj";
    let record = record();
    let resolution = &record["resolution"];
    let files: Vec<(String, String)> = resolution["files"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(path, text)| (format!("{DIR}/{path}"), text.as_str().unwrap().to_string()))
        .collect();
    let files: Vec<(&str, &str)> = files.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect();
    let loader = MemoryLoader::new(&files);

    let questions = resolution["questions"].as_array().unwrap();
    assert!(questions.len() >= 80);
    let mut failed = Vec::new();
    for question in questions {
        let name = question["name"].as_str().unwrap();
        let from = question["from"].as_str().unwrap().replace("<dir>", DIR);
        for (key, kind) in [("require", ResolveKind::Require), ("import", ResolveKind::Import)] {
            let expected = question[key].as_str().map(|path| path.replace("<dir>", DIR));
            let got = resolve(&loader, name, dirname(&from), kind).ok();
            if got != expected {
                failed.push(format!("{key}({name:?}) from {from}\n  Node.js: {expected:?}\n  here:    {got:?}"));
            }
        }
    }
    assert!(failed.is_empty(), "{} questions are different:\n{}", failed.len(), failed.join("\n"));
}

/// The message and the code of the error of a call that a rule forbids.
fn denied(expression: &str) -> (String, String) {
    let value = eval(&format!(
        "(async () => {{ try {{ await ({expression}); return ['no error', ''] }} catch (error) {{ return [error.message, error.code] }} }})()"
    ));
    (value[0].as_str().unwrap().to_string(), value[1].as_str().unwrap_or("").to_string())
}

#[test]
fn a_write_to_a_file_is_an_error_that_names_the_rule() {
    for call in [
        "require('fs').writeFileSync('/app/css/x.txt', 'x')",
        "require('node:fs').appendFileSync('/app/css/x.txt', 'x')",
        "require('fs').mkdirSync('/app/css/dir')",
        "require('fs').rmSync('/app/css/main.mjs')",
        "require('fs').unlinkSync('/app/css/main.mjs')",
        "require('fs').renameSync('/a', '/b')",
        "require('fs').copyFileSync('/a', '/b')",
        "require('fs').writeFile('/a', 'x', () => {})",
        "require('fs').createWriteStream('/a')",
        "require('fs').openSync('/a', 'w')",
        "require('fs').promises.writeFile('/a', 'x')",
        "require('fs/promises').mkdir('/a')",
    ] {
        let (message, code) = denied(call);
        assert!(message.ends_with("is not permitted: the JavaScript of a plugin or config has read-only access to files."), "{call}: {message}");
        assert!(message.starts_with("fs."), "{call}: {message}");
        assert_eq!(code, "ERR_ACCESS_DENIED", "{call}");
    }
}

#[test]
fn the_network_is_an_error_that_names_the_rule() {
    for call in [
        "fetch('https://example.com/')",
        "new XMLHttpRequest()",
        "new WebSocket('wss://example.com/')",
        "require('http').get('http://example.com/')",
        "require('node:https').request('https://example.com/')",
        "require('net').connect(80, 'example.com')",
        "require('dns').lookup('example.com', () => {})",
        "(await import('node:http')).default.get('http://example.com/')",
    ] {
        let (message, code) = denied(call);
        assert!(message.ends_with("is not permitted: the JavaScript of a plugin or config has no network access."), "{call}: {message}");
        assert_eq!(code, "ERR_ACCESS_DENIED", "{call}");
    }
    assert_eq!(denied("fetch('https://example.com/')").0, "fetch is not permitted: the JavaScript of a plugin or config has no network access.");
}

#[test]
fn a_new_process_is_an_error_that_names_the_rule() {
    for call in [
        "require('child_process').execSync('ls')",
        "require('node:child_process').spawn('ls')",
        "new (require('worker_threads').Worker)('./x.js')",
        "process.exit(0)",
        "process.kill(1)",
        "process.chdir('/')",
    ] {
        let (message, code) = denied(call);
        assert!(message.ends_with("is not permitted: the JavaScript of a plugin or config cannot start a process."), "{call}: {message}");
        assert_eq!(code, "ERR_ACCESS_DENIED", "{call}");
    }
}

#[test]
fn import_meta_has_the_url_the_directory_and_the_file() {
    let lines = run(
        MemoryLoader::default(),
        "console.log(JSON.stringify([import.meta.url, import.meta.dirname, import.meta.filename]))\nexport default () => {}\n",
    );
    assert_eq!(lines, [r#"["file:///app/css/main.mjs","/app/css","/app/css/main.mjs"]"#]);

    let loader = MemoryLoader::new(&[("/app/css/a dir/é.mjs", "export default [import.meta.url, import.meta.dirname]")]);
    let lines = run(loader, "import meta from './a dir/é.mjs'\nconsole.log(JSON.stringify(meta))\nexport default () => {}\n");
    assert_eq!(lines, [r#"["file:///app/css/a%20dir/%C3%A9.mjs","/app/css/a dir"]"#]);
}

#[test]
fn a_timer_that_no_module_waits_for_runs_before_the_end() {
    let lines = run(
        MemoryLoader::default(),
        "setTimeout(() => { console.log('timer 2'); setTimeout(() => console.log('timer 3'), 1) }, 20)\nsetTimeout(() => console.log('timer 1'), 10)\nlet interval = setInterval(() => console.log('interval'), 5)\nconsole.log('module')\nexport default () => { console.log('plugin') }\n",
    );
    assert_eq!(lines, ["module", "plugin", "timer 1", "timer 2", "timer 3"]);
}

#[test]
fn console_formats_as_node() {
    let lines = run(
        MemoryLoader::default(),
        "console.log('%s has %d items', 'list', 2, { a: [1, 'b'] })\nconsole.log({ nested: { deep: { deeper: { deepest: 1 } } } }, [1, 2])\nconsole.warn('warn %o', 'x')\nexport default () => {}\n",
    );
    assert_eq!(lines, ["list has 2 items { a: [ 1, 'b' ] }", "{ nested: { deep: { deeper: [Object] } } } [ 1, 2 ]", "warn 'x'"]);
}

#[test]
fn process_has_the_platform_and_the_arguments_of_the_loader() {
    let value = eval("[process.platform, process.argv, process.arch.length > 0, process.versions.tailwindcss, require('os').EOL, require('os').platform()]");
    assert_eq!(value[0], "linux");
    assert_eq!(value[1], serde_json::json!(["tailwindcss", "tailwindcss"]));
    assert_eq!(value[2], true);
    assert_eq!(value[3], tailwind::index::VERSION);
    assert_eq!(value[4], "\n");
    assert_eq!(value[5], "linux");
}
