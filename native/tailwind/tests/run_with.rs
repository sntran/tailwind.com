//! `cli::run_with`: the program as a part of another program (the NIF of
//! `tailwind_nif`). The working directory, the environment and the output
//! come from the context, and not from the process.

#![cfg(feature = "cli")]

use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tailwind::cli::context::Context;
use tailwind::cli::run_with;

/// Text that a test reads after the run.
#[derive(Clone, Default)]
struct Buffer(Rc<RefCell<Vec<u8>>>);

impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Buffer {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.borrow()).into_owned()
    }
}

/// A new project directory: `app.css` and `index.html`.
fn project(name: &str, html: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tailwind-run-with-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.css"), "@import \"tailwindcss\";\n").unwrap();
    std::fs::write(dir.join("index.html"), html).unwrap();
    dir
}

fn context(dir: &Path, env: &[(&str, Option<&str>)], stdout: &Buffer, stderr: &Buffer, stop: Arc<AtomicBool>) -> Context {
    Context {
        cwd: dir.to_path_buf(),
        env: env.iter().map(|(name, value)| (name.to_string(), value.map(String::from))).collect(),
        stdout: Box::new(stdout.clone()),
        stderr: Box::new(stderr.clone()),
        terminal: false,
        stop,
        stdin: None,
    }
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|arg| arg.to_string()).collect()
}

#[test]
fn a_build_reads_the_working_directory_of_the_context() {
    let dir = project("build", "<p class=\"flex p-4\"></p>\n");
    let before = std::env::current_dir().unwrap();
    let (stdout, stderr) = (Buffer::default(), Buffer::default());
    let status = run_with(args(&["-i", "app.css", "-o", "out.css"]), context(&dir, &[], &stdout, &stderr, Arc::default()));
    assert_eq!(status, 0, "{}", stderr.text());
    let css = std::fs::read_to_string(dir.join("out.css")).unwrap();
    assert!(css.contains(".flex {") && css.contains(".p-4 {"), "{css}");
    // The messages go to the standard error of the context.
    assert!(stderr.text().starts_with("≈ tailwindcss v"), "{}", stderr.text());
    assert!(stderr.text().contains("Done in "), "{}", stderr.text());
    assert_eq!(stdout.text(), "");
    // The process keeps its working directory.
    assert_eq!(std::env::current_dir().unwrap(), before);
}

#[test]
fn the_css_goes_to_the_standard_output_of_the_context() {
    let dir = project("stdout", "<p class=\"underline\"></p>\n");
    let (stdout, stderr) = (Buffer::default(), Buffer::default());
    let status = run_with(args(&["-i", "app.css"]), context(&dir, &[], &stdout, &stderr, Arc::default()));
    assert_eq!(status, 0, "{}", stderr.text());
    assert!(stdout.text().contains(".underline {"), "{}", stdout.text());
}

#[test]
fn the_environment_of_the_context_selects_the_version() {
    let dir = project("env", "");
    let (stdout, stderr) = (Buffer::default(), Buffer::default());
    let env = [("TAILWINDCSS_VERSION", Some("4.9.9"))];
    let status = run_with(args(&["--version"]), context(&dir, &env, &stdout, &stderr, Arc::default()));
    assert_eq!(status, 1);
    assert!(stderr.text().contains("Tailwind CSS v4.9.9 is not in this program"), "{}", stderr.text());
    // The variable is not in the environment of the process.
    assert_eq!(std::env::var("TAILWINDCSS_VERSION").ok(), None);

    // `None` removes a variable: the version of the program.
    let (stdout, stderr) = (Buffer::default(), Buffer::default());
    let env = [("TAILWINDCSS_VERSION", None)];
    let status = run_with(args(&["--version"]), context(&dir, &env, &stdout, &stderr, Arc::default()));
    assert_eq!(status, 0, "{}", stderr.text());
    assert!(stdout.text().contains(&format!("(tailwindcss v{})", tailwind::VERSION)), "{}", stdout.text());
}

#[test]
fn a_watch_builds_again_and_stops_when_the_caller_asks() {
    let dir = project("watch", "<p class=\"flex\"></p>\n");
    let stop = Arc::new(AtomicBool::new(false));
    let watch = {
        let (dir, stop) = (dir.clone(), stop.clone());
        std::thread::spawn(move || {
            let (stdout, stderr) = (Buffer::default(), Buffer::default());
            let argv = args(&["-i", "app.css", "-o", "out.css", "--watch", "--poll=50"]);
            let status = run_with(argv, context(&dir, &[], &stdout, &stderr, stop));
            (status, stderr.text())
        })
    };
    let wait = |needle: &str| {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(20) {
            if std::fs::read_to_string(dir.join("out.css")).is_ok_and(|css| css.contains(needle)) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    };
    assert!(wait(".flex {"), "the first build");
    std::fs::write(dir.join("index.html"), "<p class=\"flex grid\"></p>\n").unwrap();
    assert!(wait(".grid {"), "the build after the change");

    stop.store(true, Ordering::SeqCst);
    let (status, stderr) = watch.join().unwrap();
    assert_eq!(status, 0, "{stderr}");
}

/// A stylesheet whose build takes some seconds: 400,000 candidates.
const SLOW: &str = "@import \"tailwindcss\" source(none);\n@source inline(\"{,hover:,focus:,md:,lg:}{p,m,px,py,mx,my,w,h}-{0..10000}\");\n";

#[test]
fn a_build_stops_soon_when_the_caller_asks() {
    let dir = project("stop", "");
    std::fs::write(dir.join("app.css"), SLOW).unwrap();
    for delay in [0, 50, 300] {
        let stop = Arc::new(AtomicBool::new(false));
        let build = {
            let (dir, stop) = (dir.clone(), stop.clone());
            std::thread::spawn(move || {
                let (stdout, stderr) = (Buffer::default(), Buffer::default());
                let argv = args(&["-i", "app.css", "-o", "out.css"]);
                let status = run_with(argv, context(&dir, &[], &stdout, &stderr, stop));
                (status, stderr.text())
            })
        };
        std::thread::sleep(Duration::from_millis(delay));
        let start = Instant::now();
        stop.store(true, Ordering::SeqCst);
        let (status, stderr) = build.join().unwrap();
        assert_eq!(status, 1, "{stderr}");
        assert!(stderr.contains(tailwind::stop::MESSAGE), "{stderr}");
        assert!(start.elapsed() < Duration::from_millis(500), "the stop after {delay} ms took {:?}", start.elapsed());
        assert!(!dir.join("out.css").exists());
    }
}

#[test]
fn a_plugin_with_an_endless_loop_stops() {
    let dir = project("stop-plugin", "<p class=\"flex\"></p>\n");
    std::fs::write(dir.join("app.css"), "@import \"tailwindcss\";\n@plugin \"./loop.js\";\n").unwrap();
    std::fs::write(dir.join("loop.js"), "module.exports = function () { for (;;) {} }\n").unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let build = {
        let (dir, stop) = (dir.clone(), stop.clone());
        std::thread::spawn(move || {
            let (stdout, stderr) = (Buffer::default(), Buffer::default());
            let status = run_with(args(&["-i", "app.css", "-o", "out.css"]), context(&dir, &[], &stdout, &stderr, stop));
            (status, stderr.text())
        })
    };
    std::thread::sleep(Duration::from_millis(300));
    let start = Instant::now();
    stop.store(true, Ordering::SeqCst);
    let (status, stderr) = build.join().unwrap();
    assert_eq!(status, 1, "{stderr}");
    assert!(start.elapsed() < Duration::from_millis(500), "{:?}", start.elapsed());
    assert!(!dir.join("out.css").exists());
}

/// A store with a package of each version that has only `package.json`:
/// enough for a stylesheet that imports no file of Tailwind CSS.
fn store(name: &str, versions: &[&str]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tailwind-run-with-store-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for version in versions {
        std::fs::create_dir_all(dir.join(version)).unwrap();
        let json = format!("{{\"name\": \"tailwindcss\", \"version\": \"{version}\"}}\n");
        std::fs::write(dir.join(version).join("package.json"), json).unwrap();
    }
    dir
}

#[test]
fn the_input_file_is_a_source_from_4_3_2() {
    // `--duration` has a use only in the value of a theme variable with no
    // use. The scanner finds `--duration` in `var(--duration)` of the input
    // file, and that candidate keeps the theme variable.
    let dir = project("input-source", "<p class=\"p-6\"></p>\n");
    let theme = "@theme {\n  --spacing: 0.25rem;\n  --duration: 0.3s;\n  --animate-fade-in: fade-in var(--duration) linear both;\n}\n";
    let packages = store("input-source", &["4.3.1", "4.3.2"]);
    let packages = packages.to_str().unwrap();
    let build = |input: &str, version: Option<&str>| {
        std::fs::write(dir.join("app.css"), input).unwrap();
        let (stdout, stderr) = (Buffer::default(), Buffer::default());
        let env = [("TAILWINDCSS_VERSION", version), ("TAILWIND_COM_ENGINE", None), ("TAILWIND_COM_PACKAGES", Some(packages))];
        let status = run_with(args(&["-i", "app.css"]), context(&dir, &env, &stdout, &stderr, Arc::default()));
        assert_eq!(status, 0, "{}", stderr.text());
        stdout.text()
    };

    // No source of the stylesheet has the input file: from 4.3.2, the
    // program scans it all the same.
    let no_source = format!("{theme}@source inline(\"p-6\");\n@tailwind utilities source(none);\n");
    for (version, kept) in [(Some("4.3.1"), false), (Some("4.3.2"), true), (None, true)] {
        let css = build(&no_source, version);
        assert!(css.contains(".p-6 {"), "{version:?}: {css}");
        assert_eq!(css.contains("--duration: 0.3s;"), kept, "{version:?}: {css}");
    }

    // The automatic source detection has the input file: each version
    // scans it.
    let auto = format!("{theme}@tailwind utilities;\n");
    for version in [Some("4.3.1"), Some("4.3.2"), None] {
        let css = build(&auto, version);
        assert!(css.contains(".p-6 {") && css.contains("--duration: 0.3s;"), "{version:?}: {css}");
    }
}

#[test]
fn the_command_canonicalize_reads_the_input_of_the_context() {
    let dir = project("canonicalize", "");
    let (stdout, stderr) = (Buffer::default(), Buffer::default());
    let mut ctx = context(&dir, &[], &stdout, &stderr, Arc::default());
    ctx.stdin = Some(Box::new(std::io::Cursor::new(b"py-3 p-1 px-3\nw-[16px] h-[16px]\n".to_vec())));
    let status = run_with(args(&["canonicalize", "--css", "app.css", "--stream"]), ctx);
    assert_eq!(status, 0, "{}", stderr.text());
    assert_eq!(stdout.text(), "p-3\nsize-[16px]\n");

    // With no input, no lists: the help and an error.
    let (stdout, stderr) = (Buffer::default(), Buffer::default());
    let status = run_with(args(&["canonicalize"]), context(&dir, &[], &stdout, &stderr, Arc::default()));
    assert_eq!(status, 1);
    assert_eq!(stderr.text(), "No candidate groups provided\n");
}

#[test]
fn a_channel_input_gives_its_messages_and_ends_with_the_channel() {
    use std::io::Read;
    let (sender, receiver) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let mut input = tailwind::cli::context::ChannelInput::new(receiver, stop.clone());
    sender.send(b"flex\n".to_vec()).unwrap();
    sender.send(b"p-4".to_vec()).unwrap();
    drop(sender);
    let mut text = String::new();
    input.read_to_string(&mut text).unwrap();
    assert_eq!(text, "flex\np-4");

    // A stop ends the input, also with no end of the channel.
    let (_sender, receiver) = std::sync::mpsc::channel::<Vec<u8>>();
    let mut input = tailwind::cli::context::ChannelInput::new(receiver, stop.clone());
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(input.read(&mut [0; 8]).unwrap(), 0);
}
