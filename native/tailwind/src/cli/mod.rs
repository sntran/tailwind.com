//! The command line program: `@tailwindcss/cli` of Tailwind CSS.
//!
//! The options, the messages and the output follow the official program,
//! so this program can replace it.

pub mod args;
pub mod optimize_css;
pub mod resolve;

use crate::index::{compile, CompileOptions, Compiler, Root};
use crate::{Error, Result};
use args::{Parsed, Value};
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use tailwindcss_oxide::{PublicSourceEntry, Scanner};

/// The version of this program.
pub const PROGRAM_VERSION: &str = env!("CARGO_PKG_VERSION");

const DEFAULT_POLL_INTERVAL_MS: u64 = 250;
const DEFAULT_INPUT: &str = "@import 'tailwindcss';";

/// The text styles of the messages. They are empty when stderr is not a
/// terminal, or when `NO_COLOR` is set.
struct Style {
    on: bool,
}

impl Style {
    fn new() -> Self {
        let forced = std::env::var_os("FORCE_COLOR").is_some_and(|v| v != "0");
        let on = forced || (std::env::var_os("NO_COLOR").is_none() && std::io::stderr().is_terminal());
        Self { on }
    }
    fn wrap(&self, open: &str, close: &str, text: &str) -> String {
        if self.on {
            format!("\x1b[{open}m{text}\x1b[{close}m")
        } else {
            text.to_string()
        }
    }
    fn dim(&self, text: &str) -> String {
        self.wrap("2", "22", text)
    }
    fn red(&self, text: &str) -> String {
        self.wrap("31", "39", text)
    }
    fn blue(&self, text: &str) -> String {
        self.wrap("34", "39", text)
    }
    fn highlight(&self, text: &str) -> String {
        format!("{}{}{}", self.dim(&self.blue("`")), self.blue(text), self.dim(&self.blue("`")))
    }
}

/// The first line of the output: the name and the version of Tailwind CSS.
/// The Elixir package `tailwind` reads the version from this line.
fn header(style: &Style) -> String {
    format!("{} tailwindcss {}", style.blue("\u{2248}"), style.blue(&format!("v{}", crate::index::VERSION)))
}

fn help_text(style: &Style) -> String {
    let mut out = String::new();
    out.push_str(&header(style));
    out.push_str("\n\n");
    out.push_str(&style.dim("Usage:"));
    out.push_str("\n  tailwindcss [--input input.css] [--output output.css] [--watch] [--poll=ms] [options…]\n");
    out.push_str("  tailwindcss build [--input input.css] [--output output.css] [--watch] [--poll=ms] [options…]\n\n");
    out.push_str(&style.dim("Options:"));
    out.push('\n');
    for (flags, description) in [
        ("-i, --input", "Input file"),
        ("-o, --output", "Output file [default: `-`]"),
        ("-w, --watch", "Watch for changes and rebuild as needed, and use `always` to keep watching when stdin is closed"),
        ("    --poll", "The interval of the watch in milliseconds [default: `250`]"),
        ("-m, --minify", "Optimize and minify the output"),
        ("    --optimize", "Optimize the output without minifying"),
        ("    --cwd", "The current working directory [default: `.`]"),
        ("    --silent", "Suppress non-error output"),
        ("-h, --help", "Display usage information"),
        ("-v, --version", "Display the versions of this program and of Tailwind CSS"),
    ] {
        out.push_str(&format!("  {flags:<16} {description}\n"));
    }
    out
}

/// A duration as the official program prints it: the largest unit that
/// gives a whole number of at least 1.
pub fn format_duration(duration: Duration) -> String {
    let mut ns = duration.as_nanos();
    for (unit, size) in [("ns", 1000), ("µs", 1000), ("ms", 1000), ("s", 60), ("m", 60), ("h", 24)] {
        if ns < size {
            return format!("{ns}{unit}");
        }
        ns /= size;
    }
    format!("{ns}d")
}

/// An error in the box of the official program.
fn format_error(style: &Style, error: &str) -> String {
    let mut lines = vec![style.red("Error:"), style.dim("\u{250c}")];
    for line in error.split('\n') {
        lines.push(format!("{} {line}", style.dim("\u{2502}")));
    }
    lines.push(style.dim("\u{2514}"));
    lines.join("\n")
}

/// `path.resolve(base, path)` of Node.js, with no access to the file
/// system: `.` and `..` are removed.
pub fn resolve_path(base: &Path, path: &str) -> PathBuf {
    let joined = if Path::new(path).is_absolute() { PathBuf::from(path) } else { base.join(path) };
    let mut result = PathBuf::new();
    for part in joined.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                result.pop();
            }
            other => result.push(other),
        }
    }
    result
}

/// The path for a message, relative to the current directory when it is
/// in it.
fn relative(path: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    match path.strip_prefix(&cwd) {
        Ok(rest) if !rest.as_os_str().is_empty() => format!("./{}", rest.display()),
        _ => path.display().to_string(),
    }
}

/// The options of a build.
struct Options {
    /// The current directory of the build, as an absolute path.
    base: PathBuf,
    /// The input file. `None` is the default input or stdin.
    input: Option<PathBuf>,
    input_is_stdin: bool,
    /// The output file. `None` is stdout.
    output: Option<PathBuf>,
    minify: bool,
    optimize: bool,
    silent: bool,
    /// `None`: no watch. `Some(always)`: watch; with `always`, also after
    /// the end of stdin.
    watch: Option<bool>,
    poll: Duration,
}

/// The state of the last write.
#[derive(Default)]
struct Previous {
    css: Option<String>,
    optimized_css: String,
    output: Option<String>,
}

/// The mtime and the size of a file, or `None` when the file is not there.
type Stamp = Option<(SystemTime, u64)>;

fn stamp(path: &Path) -> Stamp {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// A compiler with its scanner, and the files that need a full build when
/// they change: the input and each imported stylesheet.
struct Build {
    compiler: Compiler,
    scanner: Scanner,
    full_rebuild_paths: Vec<(PathBuf, Stamp)>,
}

fn read_input(options: &Options, stdin_text: &Option<String>) -> Result<String> {
    match (&options.input, options.input_is_stdin) {
        (Some(path), _) => {
            std::fs::read_to_string(path).map_err(|e| Error::new(format!("{}: {e}", path.display())))
        }
        (None, true) => Ok(stdin_text.clone().unwrap_or_default()),
        (None, false) => Ok(DEFAULT_INPUT.to_string()),
    }
}

/// `ensureSourceDetectionRootExists` of `@tailwindcss/node`.
fn ensure_source_detection_root_exists(root: &Root) -> Result<()> {
    if let Root::Pattern { base, pattern } = root {
        let mut base_path: Vec<&str> = Vec::new();
        for segment in pattern.split('/') {
            if segment.contains('*') || segment.contains('{') {
                break;
            }
            base_path.push(segment);
        }
        if !resolve_path(Path::new(base), &base_path.join("/")).is_dir() {
            return Err(Error::new(format!("The `source({pattern})` does not exist or is not a directory.")));
        }
    }
    Ok(())
}

fn create_build(options: &Options, css: &str) -> Result<Build> {
    let input_base = match &options.input {
        Some(path) => path.parent().unwrap_or(Path::new("")).to_path_buf(),
        None => std::env::current_dir().unwrap_or_default(),
    };

    let dependencies = std::cell::RefCell::new(Vec::<PathBuf>::new());
    let load = |id: &str, base: &str| {
        resolve::load_stylesheet(id, base, &mut |path| dependencies.borrow_mut().push(path.to_path_buf()))
    };
    let compiler = compile(
        css,
        CompileOptions {
            base: input_base.to_string_lossy().into_owned(),
            polyfills: crate::POLYFILLS_ALL,
            load_stylesheet: Some(&load),
            test_mode: false,
        },
    )?;
    ensure_source_detection_root_exists(&compiler.root)?;

    let mut sources: Vec<PublicSourceEntry> = Vec::new();
    match &compiler.root {
        Root::None => {}
        Root::Default => sources.push(PublicSourceEntry {
            base: options.base.to_string_lossy().into_owned(),
            pattern: "**/*".into(),
            negated: false,
        }),
        Root::Pattern { base, pattern } => {
            sources.push(PublicSourceEntry { base: base.clone(), pattern: pattern.clone(), negated: false })
        }
    }
    for source in &compiler.sources {
        sources.push(PublicSourceEntry {
            base: source.base.clone(),
            pattern: source.pattern.clone(),
            negated: source.negated,
        });
    }
    // Do not scan this program itself.
    if let Ok(exe) = std::env::current_exe() {
        if let (Some(dir), Some(name)) = (exe.parent(), exe.file_name()) {
            sources.push(PublicSourceEntry {
                base: dir.to_string_lossy().into_owned(),
                pattern: name.to_string_lossy().into_owned(),
                negated: true,
            });
        }
    }
    // The input file can have classes too.
    if let Some(input) = &options.input {
        if let (Some(dir), Some(name)) = (input.parent(), input.file_name()) {
            sources.push(PublicSourceEntry {
                base: dir.to_string_lossy().into_owned(),
                pattern: name.to_string_lossy().into_owned(),
                negated: false,
            });
        }
    }

    let mut paths: Vec<PathBuf> = options.input.iter().cloned().collect();
    paths.extend(dependencies.into_inner());
    let full_rebuild_paths = paths.into_iter().map(|p| (p.clone(), stamp(&p))).collect();

    Ok(Build { compiler, scanner: Scanner::new(sources), full_rebuild_paths })
}

/// `outputFile` of the official program: do not write a file that has the
/// same content already, and make the parent directories.
fn output_file(file: &Path, contents: &str) -> std::io::Result<()> {
    let is_special = std::fs::metadata(file).is_ok_and(|m| !m.is_file() && !m.is_dir());
    if !is_special {
        if let Ok(current) = std::fs::read_to_string(file) {
            if current == contents {
                return Ok(());
            }
        }
    }
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(file, contents)
}

/// Write the CSS to the output. The result is true when the output changed.
fn write(css: &str, options: &Options, previous: &mut Previous) -> Result<bool> {
    let mut output = css;
    if options.minify || options.optimize {
        if previous.css.as_deref() != Some(css) {
            let file = options.input.as_ref().map_or("input.css".to_string(), |p| p.to_string_lossy().into_owned());
            previous.optimized_css = optimize_css::optimize(css, &file, options.minify).map_err(Error::new)?;
            previous.css = Some(css.to_string());
        }
        output = &previous.optimized_css;
    }

    let did_change = previous.output.as_deref() != Some(output);
    match &options.output {
        Some(file) => output_file(file, output).map_err(|e| Error::new(format!("{}: {e}", file.display())))?,
        None if did_change => {
            let mut stdout = std::io::stdout().lock();
            let _ = writeln!(stdout, "{output}");
            let _ = stdout.flush();
        }
        None => {}
    }
    previous.output = Some(output.to_string());
    Ok(did_change)
}

fn parse_options(parsed: &Parsed, style: &Style) -> std::result::Result<Options, String> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let base = resolve_path(&cwd, &parsed.text("cwd", None).unwrap_or_else(|| ".".into()));

    let output = match parsed.text("output", Some("o")) {
        Some(o) if o != "-" => Some(resolve_path(&base, &o)),
        _ => None,
    };
    let (input, input_is_stdin) = match parsed.text("input", Some("i")) {
        Some(i) if i == "-" => (None, true),
        Some(i) => {
            let path = resolve_path(&base, &i);
            if !path.exists() {
                return Err(format!("Specified input file {} does not exist.", style.highlight(&relative(&path))));
            }
            (Some(path), false)
        }
        None => (None, false),
    };
    if input.is_some() && input == output {
        let input = input.unwrap();
        return Err(format!(
            "Specified input file {} and output file {} are identical.",
            style.highlight(&relative(&input)),
            style.highlight(&relative(&input))
        ));
    }

    let poll = match parsed.flag_or_text("poll", None) {
        None | Some(Value::Bool(_)) => DEFAULT_POLL_INTERVAL_MS,
        Some(Value::Text(t)) => match t.parse::<u64>() {
            Ok(ms) if ms > 0 => ms,
            _ => return Err("Specified polling interval must be a positive number.".into()),
        },
    };

    let watch = match parsed.flag_or_text("watch", Some("w")) {
        None | Some(Value::Bool(false)) => None,
        Some(Value::Bool(true)) => Some(false),
        Some(Value::Text(t)) => Some(t == "always"),
    };

    if parsed.get("map", None).is_some_and(|v| *v != Value::Bool(false)) {
        return Err("This program does not make source maps: remove --map.".into());
    }

    Ok(Options {
        base,
        input,
        input_is_stdin,
        output,
        minify: parsed.flag("minify", Some("m")),
        optimize: parsed.flag("optimize", None),
        silent: parsed.flag("silent", None),
        watch,
        poll: Duration::from_millis(poll),
    })
}

/// Scan, build and write. The result is the CSS before the optimizer.
fn build_once(build: &mut Build, options: &Options, previous: &mut Previous) -> Result<()> {
    let candidates = build.scanner.scan();
    let css = build.compiler.build(&candidates);
    write(&css, options, previous)?;
    Ok(())
}

/// The watch: at each interval, make a full build when the input or an
/// imported stylesheet changed, else scan the changed source files and
/// build when they have new candidates. See `specs/Watch.tla`.
fn watch(mut build: Build, options: &Options, previous: &mut Previous, style: &Style, stdin_text: &Option<String>) -> i32 {
    let stdin_closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    // The end of stdin stops the watch, so a parent process that ends does
    // not leave this process. `--watch=always` keeps the watch.
    #[cfg(not(target_family = "wasm"))]
    if options.watch == Some(false) && !options.input_is_stdin {
        let flag = stdin_closed.clone();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 1024];
            let mut stdin = std::io::stdin();
            loop {
                match stdin.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        });
    }

    loop {
        if stdin_closed.load(std::sync::atomic::Ordering::SeqCst) {
            return 0;
        }
        std::thread::sleep(options.poll);

        let start = Instant::now();
        let full = build.full_rebuild_paths.iter().any(|(path, old)| stamp(path) != *old);

        let result = if full {
            // Keep the old compiler when the new input has an error. Record
            // the new stamps, so the same error is not printed at each tick.
            for (path, old) in build.full_rebuild_paths.iter_mut() {
                *old = stamp(path);
            }
            read_input(options, stdin_text).and_then(|css| create_build(options, &css)).and_then(|mut new_build| {
                let result = build_once(&mut new_build, options, previous);
                build = new_build;
                result.map(|_| true)
            })
        } else {
            let before = previous.output.clone();
            build_once(&mut build, options, previous).map(|_| previous.output != before)
        };

        match result {
            Ok(true) if !options.silent => eprintln!("Done in {}", format_duration(start.elapsed())),
            Ok(_) => {}
            Err(e) => eprintln!("{}", format_error(style, &e.message)),
        }
    }
}

/// Run the program. The result is the exit status.
pub fn run(mut argv: Vec<String>) -> i32 {
    let style = Style::new();

    let command = argv.first().cloned();
    if command.as_deref() == Some("build") {
        argv.remove(0);
    }
    let parsed = args::parse(&argv);

    if parsed.flag("version", Some("v")) {
        println!("tailwind.com v{PROGRAM_VERSION} (tailwindcss v{})", crate::index::VERSION);
        return 0;
    }
    if parsed.flag("help", Some("h")) {
        print!("{}", help_text(&style));
        return 0;
    }
    if let Some(invalid) = parsed.rest.first() {
        println!("{}\n\n{} {invalid}", header(&style), style.dim("Invalid command:"));
        print!("{}", help_text(&style).split_once("\n\n").map_or("", |(_, rest)| rest));
        return 1;
    }

    let options = match parse_options(&parsed, &style) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            return 1;
        }
    };

    if !options.silent {
        eprintln!("{}\n", header(&style));
    }

    let start = Instant::now();
    let stdin_text = if options.input_is_stdin {
        let mut text = String::new();
        let _ = std::io::stdin().read_to_string(&mut text);
        Some(text)
    } else {
        None
    };

    let mut previous = Previous::default();
    let first = read_input(&options, &stdin_text).and_then(|css| create_build(&options, &css)).and_then(|mut build| {
        build_once(&mut build, &options, &mut previous)?;
        Ok(build)
    });
    let build = match first {
        Ok(build) => build,
        Err(e) => {
            eprintln!("{}", format_error(&style, &e.message));
            return 1;
        }
    };
    if !options.silent {
        eprintln!("Done in {}", format_duration(start.elapsed()));
    }

    if options.watch.is_some() {
        return watch(build, &options, &mut previous, &style, &stdin_text);
    }
    0
}

/// The entry of the program in the one-file executable: `argv` of C.
///
/// # Safety
///
/// `argv` must have `argc` pointers to C strings, as the `main` of C gets.
#[no_mangle]
pub unsafe extern "C" fn tailwind_main(argc: std::os::raw::c_int, argv: *const *const std::os::raw::c_char) -> std::os::raw::c_int {
    let mut args = Vec::new();
    for i in 1..argc.max(1) as usize {
        let arg = unsafe { std::ffi::CStr::from_ptr(*argv.add(i)) };
        args.push(arg.to_string_lossy().into_owned());
    }
    // A panic must not cross into C.
    std::panic::catch_unwind(|| run(args)).unwrap_or(101)
}
