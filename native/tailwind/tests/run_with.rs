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
