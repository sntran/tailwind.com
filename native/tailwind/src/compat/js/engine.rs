//! The JavaScript engine: safe Rust over `c_src/js.c` (QuickJS).
//!
//! Only UTF-8 text goes to the engine and comes back. JavaScript has no
//! file access and no network access: it calls the host function of the
//! engine (`__host(op, text)`), and the host decides.

use std::ffi::{c_char, c_int, c_void, CStr, CString};

#[repr(C)]
struct TwJs {
    _private: [u8; 0],
}

type HostFn = unsafe extern "C" fn(user: *mut c_void, op: *const c_char, text: *const c_char, is_error: *mut c_int) -> *mut c_char;
type ReleaseFn = unsafe extern "C" fn(user: *mut c_void, text: *mut c_char);

extern "C" {
    fn tw_js_new(memory_limit: usize, stack_limit: usize, host: HostFn, release: ReleaseFn, user: *mut c_void) -> *mut TwJs;
    fn tw_js_free(js: *mut TwJs);
    fn tw_js_free_text(text: *mut c_char);
    fn tw_js_eval(
        js: *mut TwJs,
        source: *const c_char,
        length: usize,
        filename: *const c_char,
        is_module: c_int,
        out: *mut *mut c_char,
        stack: *mut *mut c_char,
    ) -> c_int;
    fn tw_js_call(js: *mut TwJs, name: *const c_char, text: *const c_char, out: *mut *mut c_char, stack: *mut *mut c_char) -> c_int;
    fn tw_js_run_jobs(js: *mut TwJs, out: *mut *mut c_char, stack: *mut *mut c_char) -> c_int;
    fn tw_js_memory_used(js: *mut TwJs) -> usize;
    fn tw_js_platform() -> *const c_char;
}

/// `process.platform` of Node.js for the system that runs the program.
/// `tailwind.com` is one file for all the systems, so the C code finds
/// the system at run time.
pub fn platform() -> String {
    // SAFETY: the function gives static text with a NUL at its end.
    unsafe { CStr::from_ptr(tw_js_platform()) }.to_string_lossy().into_owned()
}

/// An exception of JavaScript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsError {
    /// The `name` of the exception, for example `TypeError`.
    pub name: Option<String>,
    /// The `message` of the exception, or the exception as text.
    pub message: String,
    /// The `stack` of the exception.
    pub stack: Option<String>,
}

impl std::fmt::Display for JsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// The result of a host operation: text, or nothing (`undefined`), or the
/// message of an `Error` that JavaScript gets.
pub type HostResult = Result<Option<String>, String>;

/// The host function: `(op, text)`.
pub type Host = Box<dyn Fn(&str, &str) -> HostResult>;

/// The limits of an engine. 0 is no limit.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// The largest size of the memory of JavaScript, in bytes.
    pub memory: usize,
    /// The largest size of the stack of JavaScript, in bytes.
    pub stack: usize,
}

impl Default for Limits {
    fn default() -> Self {
        // In WebAssembly, the limit is for the stack in the linear memory
        // (1 MB in all for a program of Rust).
        let stack = if cfg!(target_family = "wasm") { 512 * 1024 } else { 1024 * 1024 };
        Self { memory: 1024 * 1024 * 1024, stack }
    }
}

/// One engine: a runtime and a context of QuickJS.
pub struct Engine {
    raw: *mut TwJs,
    /// The host function. The engine has its address.
    host: *mut Host,
}

impl Engine {
    pub fn new(limits: Limits, host: Host) -> Result<Self, JsError> {
        let host = Box::into_raw(Box::new(host));
        // SAFETY: `host` stays valid until `drop`, after `tw_js_free`.
        let raw = unsafe { tw_js_new(limits.memory, limits.stack, host_trampoline, release_trampoline, host.cast()) };
        if raw.is_null() {
            // SAFETY: the pointer comes from `Box::into_raw` above.
            drop(unsafe { Box::from_raw(host) });
            return Err(JsError { name: None, message: "The JavaScript engine has no memory.".to_string(), stack: None });
        }
        Ok(Self { raw, host })
    }

    /// Evaluate a script. The result is the value, when it is a string.
    pub fn eval(&self, source: &str, filename: &str) -> Result<Option<String>, JsError> {
        self.eval_as(source, filename, false)
    }

    /// Evaluate an ES module, with its pending jobs.
    pub fn eval_module(&self, source: &str, filename: &str) -> Result<(), JsError> {
        self.eval_as(source, filename, true).map(|_| ())
    }

    fn eval_as(&self, source: &str, filename: &str, is_module: bool) -> Result<Option<String>, JsError> {
        // QuickJS needs a NUL after the source.
        let source = c_string(source);
        let filename = c_string(filename);
        let (mut out, mut stack) = (std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: the pointers are valid for the call, and `out` and
        // `stack` get text from malloc or NULL.
        let status = unsafe {
            tw_js_eval(
                self.raw,
                source.as_ptr(),
                source.as_bytes().len(),
                filename.as_ptr(),
                is_module as c_int,
                &mut out,
                &mut stack,
            )
        };
        finish(status, out, stack)
    }

    /// Call the global function `name` with one string argument. The
    /// result is the value, when it is a string.
    pub fn call(&self, name: &str, text: &str) -> Result<Option<String>, JsError> {
        let name = c_string(name);
        let text = c_string(text);
        let (mut out, mut stack) = (std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: as in `eval_as`.
        let status = unsafe { tw_js_call(self.raw, name.as_ptr(), text.as_ptr(), &mut out, &mut stack) };
        finish(status, out, stack)
    }

    /// Run the pending jobs (promises) until none is left.
    pub fn run_jobs(&self) -> Result<(), JsError> {
        let (mut out, mut stack) = (std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: as in `eval_as`.
        let status = unsafe { tw_js_run_jobs(self.raw, &mut out, &mut stack) };
        finish(status, out, stack).map(|_| ())
    }

    /// The bytes that JavaScript uses now.
    pub fn memory_used(&self) -> usize {
        // SAFETY: `raw` is a valid engine.
        unsafe { tw_js_memory_used(self.raw) }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: `raw` and `host` are valid, and nothing uses them after.
        unsafe {
            tw_js_free(self.raw);
            drop(Box::from_raw(self.host));
        }
    }
}

/// Text for C. A NUL in the text ends it there: JSON text and names have
/// no NUL.
fn c_string(text: &str) -> CString {
    match CString::new(text) {
        Ok(text) => text,
        Err(error) => {
            let end = error.nul_position();
            CString::new(&text.as_bytes()[..end]).unwrap_or_default()
        }
    }
}

/// Take the text of the engine, and free it.
fn take_text(text: *mut c_char) -> Option<String> {
    if text.is_null() {
        return None;
    }
    // SAFETY: the engine gives text with a NUL at its end, from malloc.
    let result = unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();
    // SAFETY: the text comes from the engine, and nothing uses it after.
    unsafe { tw_js_free_text(text) };
    Some(result)
}

fn finish(status: c_int, out: *mut c_char, stack: *mut c_char) -> Result<Option<String>, JsError> {
    let out = take_text(out);
    let stack = take_text(stack);
    if status == 0 {
        Ok(out)
    } else {
        // The engine gives the name, a line end, and the stack.
        let (name, stack) = match stack {
            Some(text) => match text.split_once('\n') {
                Some((name, stack)) => ((!name.is_empty()).then(|| name.to_string()), (!stack.is_empty()).then(|| stack.to_string())),
                None => (None, Some(text)),
            },
            None => (None, None),
        };
        let message = out.unwrap_or_else(|| "JavaScript exception".to_string());
        Err(JsError { name, message: v8_message(&message), stack })
    }
}

/// The text of V8 for the common errors of the engine. The official program
/// runs on V8 (Node.js or Bun), and it prints the message of an exception
/// of a plugin. QuickJS has other words for the same errors: for example
/// `cannot convert to object` for `Object.keys(null)`. A message can be a
/// part of a longer text, so each part changes in place.
pub fn v8_message(message: &str) -> String {
    let mut text = message.replace("cannot convert to object", "Cannot convert undefined or null to object");
    // `cannot read property 'x' of undefined` and the same for null and
    // for a write.
    for (quickjs, v8, verb) in [
        ("cannot read property '", "Cannot read properties of ", "reading"),
        ("cannot set property '", "Cannot set properties of ", "setting"),
    ] {
        while let Some(start) = text.find(quickjs) {
            let rest = &text[start + quickjs.len()..];
            let Some(end) = rest.find("' of ") else { break };
            let name = &rest[..end];
            let after = &rest[end + "' of ".len()..];
            let target = if after.starts_with("undefined") {
                "undefined"
            } else if after.starts_with("null") {
                "null"
            } else {
                break;
            };
            let tail = after[target.len()..].to_string();
            text = format!("{}{v8}{target} ({verb} '{name}'){tail}", &text[..start]);
        }
    }
    for (quickjs, v8) in [
        ("cannot read property of undefined", "Cannot read properties of undefined"),
        ("cannot read property of null", "Cannot read properties of null"),
    ] {
        text = text.replace(quickjs, v8);
    }
    // `'x' is not defined` is `x is not defined` in V8.
    if let Some(rest) = text.strip_prefix('\'') {
        if let Some(name) = rest.strip_suffix("' is not defined") {
            if !name.contains('\'') {
                text = format!("{name} is not defined");
            }
        }
    }
    text
}

#[cfg(test)]
mod v8_message_tests {
    use super::v8_message;

    #[test]
    fn the_messages_of_v8() {
        // The values come from Node.js 22.
        for (quickjs, v8) in [
            ("cannot convert to object", "Cannot convert undefined or null to object"),
            ("cannot read property 'foo' of undefined", "Cannot read properties of undefined (reading 'foo')"),
            ("cannot read property 'bar' of null", "Cannot read properties of null (reading 'bar')"),
            ("cannot set property 'x' of undefined", "Cannot set properties of undefined (setting 'x')"),
            ("cannot set property 'y' of null", "Cannot set properties of null (setting 'y')"),
            ("'notDefinedThing' is not defined", "notDefinedThing is not defined"),
            ("not a function", "not a function"),
            ("Plugin failed: cannot convert to object", "Plugin failed: Cannot convert undefined or null to object"),
        ] {
            assert_eq!(v8_message(quickjs), v8, "{quickjs}");
        }
    }
}

/// The host function for C. The result is text from `CString::into_raw`:
/// the engine gives it back through `release_trampoline`.
unsafe extern "C" fn host_trampoline(user: *mut c_void, op: *const c_char, text: *const c_char, is_error: *mut c_int) -> *mut c_char {
    // SAFETY: `user` is the `Host` of the engine, and `op` and `text` are
    // valid text for the time of the call.
    let (host, op, text) = unsafe { (&*user.cast::<Host>(), CStr::from_ptr(op), CStr::from_ptr(text)) };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host(&op.to_string_lossy(), &text.to_string_lossy())))
        .unwrap_or_else(|_| Err("The host operation failed.".to_string()));
    let (text, failed) = match result {
        Ok(None) => return std::ptr::null_mut(),
        Ok(Some(text)) => (text, false),
        Err(message) => (message, true),
    };
    // SAFETY: `is_error` is a valid address.
    unsafe { *is_error = failed as c_int };
    c_string(&text).into_raw()
}

unsafe extern "C" fn release_trampoline(_user: *mut c_void, text: *mut c_char) {
    if !text.is_null() {
        // SAFETY: the text comes from `CString::into_raw` in `host_trampoline`.
        drop(unsafe { CString::from_raw(text) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn engine(host: impl Fn(&str, &str) -> HostResult + 'static) -> Engine {
        Engine::new(Limits::default(), Box::new(host)).unwrap()
    }

    fn no_host() -> Engine {
        engine(|_, _| Ok(None))
    }

    #[test]
    fn evaluates_a_script() {
        let js = no_host();
        assert_eq!(js.eval("'a' + 1", "test.js").unwrap().as_deref(), Some("a1"));
        assert_eq!(js.eval("1 + 1", "test.js").unwrap(), None);
        assert_eq!(js.eval("'héllo ✓ 😀'", "test.js").unwrap().as_deref(), Some("héllo ✓ 😀"));
    }

    #[test]
    fn calls_a_global_function() {
        let js = no_host();
        js.eval("globalThis.twice = (text) => JSON.stringify(JSON.parse(text) * 2)", "test.js").unwrap();
        assert_eq!(js.call("twice", "21").unwrap().as_deref(), Some("42"));
        js.eval("globalThis.nothing = () => undefined", "test.js").unwrap();
        assert_eq!(js.call("nothing", "").unwrap(), None);
    }

    #[test]
    fn gives_the_message_and_the_stack_of_an_exception() {
        let js = no_host();
        let error = js.eval("function f() { throw new TypeError('bad value') }\nf()", "file.js").unwrap_err();
        assert_eq!(error.message, "bad value");
        assert_eq!(error.name.as_deref(), Some("TypeError"));
        assert!(error.stack.unwrap().contains("file.js:1"));

        let error = js.eval("throw 'text'", "file.js").unwrap_err();
        assert_eq!(error.message, "text");

        let error = js.eval("let x = ;", "file.js").unwrap_err();
        assert!(error.message.contains("unexpected token"), "{}", error.message);

        let error = js.call("missing", "").unwrap_err();
        assert_eq!(error.message, "not a function");

        js.eval("globalThis.fail = (text) => { throw new Error('no ' + text) }", "test.js").unwrap();
        assert_eq!(js.call("fail", "way").unwrap_err().message, "no way");
        // The engine works after an exception.
        assert_eq!(js.eval("'ok'", "test.js").unwrap().as_deref(), Some("ok"));
    }

    #[test]
    fn runs_the_jobs_of_promises() {
        let js = no_host();
        js.eval("globalThis.log = []; Promise.resolve(1).then((v) => log.push(v)).then(() => log.push(2))", "test.js").unwrap();
        assert_eq!(js.eval("JSON.stringify(log)", "test.js").unwrap().as_deref(), Some("[]"));
        js.run_jobs().unwrap();
        assert_eq!(js.eval("JSON.stringify(log)", "test.js").unwrap().as_deref(), Some("[1,2]"));
    }

    #[test]
    fn awaits_a_promise_synchronously() {
        let js = no_host();
        let source = "String(__await((async () => { await null; return 7 })()))";
        assert_eq!(js.eval(source, "test.js").unwrap().as_deref(), Some("7"));
        let error = js.eval("__await(Promise.reject(new Error('rejected')))", "test.js").unwrap_err();
        assert_eq!(error.message, "rejected");
        let error = js.eval("__await(new Promise(() => {}))", "test.js").unwrap_err();
        assert!(error.message.contains("cannot settle"));
        assert_eq!(js.eval("String(__await(5))", "test.js").unwrap().as_deref(), Some("5"));
    }

    #[test]
    fn calls_the_host() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = seen.clone();
        let js = engine(move |op, text| {
            log.borrow_mut().push((op.to_string(), text.to_string()));
            match op {
                "echo" => Ok(Some(format!("<{text}>"))),
                "nothing" => Ok(None),
                _ => Err(format!("unknown operation {op}")),
            }
        });
        assert_eq!(js.eval("__host('echo', 'ä')", "test.js").unwrap().as_deref(), Some("<ä>"));
        assert_eq!(js.eval("String(__host('nothing'))", "test.js").unwrap().as_deref(), Some("undefined"));
        let error = js.eval("__host('other', 'x')", "test.js").unwrap_err();
        assert_eq!(error.message, "unknown operation other");
        assert_eq!(
            js.eval("try { __host('other') } catch (e) { (e instanceof Error) + ':' + e.message }", "test.js").unwrap().as_deref(),
            Some("true:unknown operation other")
        );
        assert_eq!(seen.borrow()[0], ("echo".to_string(), "ä".to_string()));
        assert_eq!(seen.borrow()[1], ("nothing".to_string(), String::new()));
    }

    #[test]
    fn a_panic_of_the_host_is_an_exception() {
        let js = engine(|_, _| panic!("host panic"));
        let error = js.eval("__host('x')", "test.js").unwrap_err();
        assert_eq!(error.message, "The host operation failed.");
    }

    #[test]
    fn the_host_can_call_the_engine_again() {
        let slot: Rc<RefCell<Option<Rc<Engine>>>> = Rc::new(RefCell::new(None));
        let inner = slot.clone();
        let js = Rc::new(engine(move |_, text| {
            let js = inner.borrow().clone().unwrap();
            js.call("double", text).map_err(|e| e.message)
        }));
        *slot.borrow_mut() = Some(js.clone());
        js.eval("globalThis.double = (text) => text + text", "test.js").unwrap();
        assert_eq!(js.eval("__host('again', 'ab')", "test.js").unwrap().as_deref(), Some("abab"));
        *slot.borrow_mut() = None;
    }

    #[test]
    fn loads_es_modules_through_the_host() {
        let js = engine(|op, text| match op {
            "module.resolve" => {
                let name = text.split("\"name\":\"").nth(1).and_then(|rest| rest.split('"').next()).unwrap_or("");
                match name {
                    "./missing.js" => Ok(None),
                    name => Ok(Some(format!("/virtual/{}", name.trim_start_matches("./")))),
                }
            }
            "module.load" => match text {
                "/virtual/a.js" => Ok(Some("import { b } from './b.js'; export const a = 'a' + b; export default 1".to_string())),
                "/virtual/b.js" => Ok(Some("export const b = 'b'".to_string())),
                "/virtual/bad.js" => Ok(Some("export const = 1".to_string())),
                other => Err(format!("Cannot read {other}")),
            },
            _ => Ok(None),
        });
        js.eval_module("import { a } from './a.js'; globalThis.result = a", "/virtual/main.js").unwrap();
        assert_eq!(js.eval("result", "test.js").unwrap().as_deref(), Some("ab"));

        // A dynamic import, and the synchronous wait.
        assert_eq!(js.eval("__await(import('./a.js')).a", "/virtual/x.js").unwrap().as_deref(), Some("ab"));

        let error = js.eval_module("import './missing.js'", "/virtual/main2.js").unwrap_err();
        assert!(error.message.contains("Cannot find module './missing.js'"), "{}", error.message);
        let error = js.eval_module("import './other.js'", "/virtual/main3.js").unwrap_err();
        assert_eq!(error.message, "Cannot read /virtual/other.js");
        assert!(js.eval_module("import './bad.js'", "/virtual/main4.js").is_err());
    }

    #[test]
    fn a_module_with_top_level_await_runs_to_its_end() {
        let js = no_host();
        js.eval_module("globalThis.value = await Promise.resolve('done')", "/m.js").unwrap();
        assert_eq!(js.eval("value", "test.js").unwrap().as_deref(), Some("done"));
        let error = js.eval_module("await null; throw new Error('late')", "/n.js").unwrap_err();
        assert_eq!(error.message, "late");
    }

    #[test]
    fn stops_at_the_memory_limit() {
        let js = Engine::new(Limits { memory: 8 * 1024 * 1024, stack: 512 * 1024 }, Box::new(|_, _| Ok(None))).unwrap();
        let error = js.eval("let all = []; for (;;) all.push(new Array(1000).fill('x'.repeat(100) + all.length))", "test.js").unwrap_err();
        assert!(error.message.contains("out of memory"), "{}", error.message);
        // The engine works after the error.
        assert_eq!(js.eval("'ok'", "test.js").unwrap().as_deref(), Some("ok"));
        assert!(js.memory_used() > 0);
    }

    #[test]
    fn stops_at_the_stack_limit() {
        let js = Engine::new(Limits { memory: 0, stack: 256 * 1024 }, Box::new(|_, _| Ok(None))).unwrap();
        let error = js.eval("function f() { return f() + 1 }\nf()", "test.js").unwrap_err();
        assert!(error.message.contains("stack overflow"), "{}", error.message);
    }

    #[test]
    fn has_no_access_to_files_or_the_network() {
        let js = no_host();
        for name in ["require", "process", "std", "os", "fetch", "XMLHttpRequest", "console", "print"] {
            let source = format!("typeof {name}");
            assert_eq!(js.eval(&source, "test.js").unwrap().as_deref(), Some("undefined"), "{name}");
        }
    }
}
