//! The command line program: `@tailwindcss/cli` of Tailwind CSS.
//!
//! The options, the messages and the output follow the official program,
//! so this program can replace it.

pub mod args;
pub mod context;
pub mod engine;
pub mod events;
pub mod modules;
pub mod optimize_css;
pub mod remapping;
pub mod resolve;
pub mod source_maps;

use crate::index::{compile, CompileOptions, Root};
use crate::{Error, Result};
use args::{Parsed, Value};
use context::{errln, outln};
use engine::{AnyCompiler, Selection};
use source_maps::SourceMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};
use tailwindcss_oxide::{PublicSourceEntry, Scanner};

/// The version of this program.
pub const PROGRAM_VERSION: &str = env!("CARGO_PKG_VERSION");

const DEFAULT_POLL_INTERVAL_MS: u64 = 250;
/// The interval of the watch when the system gives file events: the
/// events wake the watch at once, and the interval finds the changes that
/// have no event (as the file watcher of BEAM.com).
const EVENTS_INTERVAL_MS: u64 = 500;
const DEFAULT_INPUT: &str = "@import 'tailwindcss';";
/// The time that the stamps of the input files must stay the same before
/// a full build of the watch: see `wait_until_stable`.
const STABLE_MS: u64 = 10;

/// The text styles of the messages. They are empty when stderr is not a
/// terminal, or when `NO_COLOR` is set.
struct Style {
    on: bool,
}

impl Style {
    fn new() -> Self {
        let forced = context::var("FORCE_COLOR").is_some_and(|v| v != "0");
        let on = forced || (context::var("NO_COLOR").is_none() && context::terminal());
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
    format!("{} tailwindcss {}", style.blue("\u{2248}"), style.blue(&format!("v{}", engine::requested_version())))
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
        ("    --poll", "Use polling instead of filesystem events when watching"),
        ("-m, --minify", "Optimize and minify the output"),
        ("    --optimize", "Optimize the output without minifying"),
        ("    --map", "Generate a source map"),
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

/// An error in the box of the official program (`formatError`): the
/// error, and then each cause of its `cause` chain, one level deeper.
fn format_error(style: &Style, error: &Error) -> String {
    // The official program prints the error object of JavaScript, whose
    // text starts with the name of the error: `Error`, `CssSyntaxError`,
    // or the name of an exception of a plugin.
    let first = if error.name.is_empty() { error.message.clone() } else { format!("{}: {}", error.name, error.message) };
    let mut lines = Vec::new();
    for (depth, text) in std::iter::once(&first).chain(error.causes.iter()).enumerate() {
        let indent = "  ".repeat(depth);
        lines.push(format!("{indent}{}", style.red(if depth == 0 { "Error:" } else { "Caused by:" })));
        lines.push(format!("{indent}{}", style.dim("\u{250c}")));
        for line in text.split('\n') {
            // The word wrap of the official program removes the spaces at
            // the start of a line.
            let line = line.trim_start_matches(' ');
            lines.push(format!("{indent}{} {line}", style.dim("\u{2502}")));
        }
        lines.push(format!("{indent}{}", style.dim("\u{2514}")));
    }
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
    let cwd = context::cwd();
    match path.strip_prefix(&cwd) {
        Ok(rest) if !rest.as_os_str().is_empty() => format!("./{}", rest.display()),
        _ => path.display().to_string(),
    }
}

/// `path.relative(from, to)` of Node.js, for two absolute paths with no
/// `.` and `..` parts.
fn relative_path(from: &Path, to: &Path) -> String {
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = from[common..].iter().map(|_| "..".to_string()).collect();
    parts.extend(to[common..].iter().map(|part| part.as_os_str().to_string_lossy().into_owned()));
    parts.join(std::path::MAIN_SEPARATOR_STR)
}

/// The source map of `--map`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum MapOption {
    /// No source map.
    None,
    /// `--map`: the map is in a comment at the end of the output.
    Inline,
    /// `--map FILE`: the map is in this file, as an absolute path.
    File(PathBuf),
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
    /// The name of the input for the optimizer: the path of the input,
    /// `-` for stdin, or `input.css`.
    optimizer_file: String,
    map: MapOption,
    minify: bool,
    optimize: bool,
    silent: bool,
    /// `None`: no watch. `Some(always)`: watch; with `always`, also after
    /// the end of stdin.
    watch: Option<bool>,
    /// The interval of `--poll`. `None`: use the file events of the system
    /// when it has them.
    poll: Option<Duration>,
    /// The engine and the version of Tailwind CSS.
    engine: Selection,
}

/// The state of the last write.
#[derive(Default)]
struct Previous {
    /// The CSS of the compiler that `optimized_css` and `map` are for.
    css: Option<String>,
    optimized_css: String,
    /// The source map of the output.
    map: Option<SourceMap>,
    output: Option<String>,
}

/// The mtime and the size of a file, or `None` when the file is not there.
pub(crate) type Stamp = Option<(SystemTime, u64)>;

pub(crate) fn stamp(path: &Path) -> Stamp {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// A compiler with its scanner, and the files that need a full build when
/// they change: the input and each imported stylesheet.
struct Build {
    compiler: AnyCompiler,
    scanner: Scanner,
    full_rebuild_paths: Vec<(PathBuf, Stamp)>,
}

/// Read the input. The result has the stamp of the input file from before
/// the read: a change of the file after this stamp gives another stamp at
/// the next tick of the watch, so no change is lost (`specs/Watch.tla`).
fn read_input(options: &Options, stdin_text: &Option<String>) -> Result<(String, Stamp)> {
    match (&options.input, options.input_is_stdin) {
        (Some(path), _) => {
            let before = stamp(path);
            let css = std::fs::read_to_string(path).map_err(|e| Error::new(format!("{}: {e}", path.display())))?;
            Ok((css, before))
        }
        (None, true) => Ok((stdin_text.clone().unwrap_or_default(), None)),
        (None, false) => Ok((DEFAULT_INPUT.to_string(), None)),
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

/// The `from` option of the compiler, as the official program gives it.
fn compiler_from(options: &Options) -> String {
    match &options.input {
        Some(path) => path.to_string_lossy().into_owned(),
        None => "stdin.css".to_string(),
    }
}

fn create_build(options: &Options, input: &(String, Stamp)) -> Result<Build> {
    // The compiler keeps the source locations only for a source map: they
    // have a cost. The official program keeps them for each build, and
    // thus the message of a syntax error has the position. On an error,
    // compile again with the locations for that message.
    let from = compiler_from(options);
    // The script engine is the official compiler: it gets the path, as in
    // the official program.
    if options.map != MapOption::None || matches!(options.engine, Selection::Script(_)) {
        return create_build_from(options, input, Some(from));
    }
    match create_build_from(options, input, None) {
        // Only a syntax error has a position. The second compile runs the
        // plugins again, so their messages (`console.log`) stay quiet:
        // daisyUI, for example, prints its name to stdout.
        Err(error) if error.name == "CssSyntaxError" => {
            Err(create_build_with(options, input, Some(from), true).err().unwrap_or(error))
        }
        result => result,
    }
}

fn create_build_from(options: &Options, input: &(String, Stamp), from: Option<String>) -> Result<Build> {
    create_build_with(options, input, from, false)
}

fn create_build_with(options: &Options, (css, input_stamp): &(String, Stamp), from: Option<String>, quiet: bool) -> Result<Build> {
    let input_base = match &options.input {
        Some(path) => path.parent().unwrap_or(Path::new("")).to_path_buf(),
        None => context::cwd(),
    };

    // The resolver gives each path before it reads the file, so each stamp
    // is from before the read.
    let dependencies = std::cell::RefCell::new(Vec::<(PathBuf, Stamp)>::new());
    let load = |id: &str, base: &str| {
        resolve::load_stylesheet(id, base, options.engine.package(), &mut |path| {
            dependencies.borrow_mut().push((path.to_path_buf(), stamp(path)))
        })
    };
    let modules = Rc::new(modules::FileSystem::with_quiet_console(quiet));
    let base = input_base.to_string_lossy().into_owned();
    #[cfg(tw_js)]
    let script = |package: &Rc<engine::DirectoryPackage>, from: Option<String>| {
        crate::script::compile(
            css,
            crate::script::ScriptOptions { base: base.clone(), from, load_stylesheet: Some(&load), module_loader: modules.clone() },
            package.clone(),
        )
        .map(AnyCompiler::Script)
    };
    let compiler = match &options.engine {
        Selection::Native { version, package } => {
            let native = compile(
                css,
                CompileOptions {
                    base: base.clone(),
                    polyfills: crate::POLYFILLS_ALL,
                    load_stylesheet: Some(&load),
                    test_mode: false,
                    from,
                    module_loader: Some(modules.clone()),
                    version: Some(*version),
                },
            );
            match (native, package) {
                // The Rust compiler does not load JavaScript plugins as a
                // version older than `Version::OLDEST_WITH_PLUGINS`: the
                // official compiler of that version does it.
                #[cfg(tw_js)]
                (Err(error), Some(package)) if error.name == crate::version::SCRIPT_ENGINE_NEEDED => {
                    dependencies.borrow_mut().clear();
                    script(package, Some(compiler_from(options)))?
                }
                (native, _) => AnyCompiler::Native(Box::new(native?)),
            }
        }
        #[cfg(tw_js)]
        Selection::Script(package) => script(package, from)?,
        #[cfg(not(tw_js))]
        Selection::Script(_) => return Err(Error::new("This build of the program has no JavaScript engine.")),
    };
    // `console.warn` of the compiler writes to the standard error.
    for warning in compiler.take_warnings() {
        errln!("{warning}");
    }
    ensure_source_detection_root_exists(compiler.root())?;

    let mut sources: Vec<PublicSourceEntry> = Vec::new();
    match compiler.root() {
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
    for source in compiler.sources() {
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

    let mut full_rebuild_paths: Vec<(PathBuf, Stamp)> =
        options.input.iter().map(|path| (path.clone(), *input_stamp)).collect();
    full_rebuild_paths.extend(dependencies.into_inner());
    // The modules of `@plugin` and `@config`.
    full_rebuild_paths.extend(modules.take_dependencies());

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

/// Write the CSS to the output, with its source map for `--map`.
/// `source_map` gives the source map of the CSS. The result is true when
/// the output changed.
fn write(
    css: &str,
    source_map: &dyn Fn() -> Result<SourceMap>,
    options: &Options,
    previous: &mut Previous,
) -> Result<bool> {
    let optimize = options.minify || options.optimize;
    // The optimizer and the map run only for new CSS: the watch calls
    // this function at each tick.
    if (optimize || options.map != MapOption::None) && previous.css.as_deref() != Some(css) {
        let mut map = (options.map != MapOption::None).then(source_map).transpose()?;
        if optimize {
            let optimized = optimize_css::optimize(
                css,
                &options.optimizer_file,
                options.minify,
                map.as_ref().map(|map| map.raw.as_str()),
            )
            .map_err(Error::new)?;
            // The optimizer recovers from an error in the CSS, and gives
            // a warning: the official program prints it.
            if let Some(warnings) = &optimized.warnings {
                errln!("{warnings}");
            }
            previous.optimized_css = optimized.code;
            if let Some(raw) = optimized.map {
                map = Some(source_maps::raw_source_map(raw));
            }
        }
        previous.map = map;
        previous.css = Some(css.to_string());
    }
    let mut output = std::borrow::Cow::Borrowed(if optimize { previous.optimized_css.as_str() } else { css });

    if let Some(map) = &previous.map {
        match &options.map {
            MapOption::None => {}
            MapOption::Inline => {
                let output = output.to_mut();
                output.push('\n');
                output.push_str(&map.inline());
            }
            MapOption::File(file) => {
                let cwd;
                let base_path = match &options.output {
                    Some(output) => output.parent().unwrap_or(Path::new("")),
                    None => {
                        cwd = context::cwd();
                        &cwd
                    }
                };
                let output = output.to_mut();
                output.push('\n');
                output.push_str(&map.comment(&relative_path(base_path, file)));
                output_file(file, &map.raw).map_err(|e| Error::new(format!("{}: {e}", file.display())))?;
            }
        }
    }
    let output: &str = &output;

    let did_change = previous.output.as_deref() != Some(output);
    match &options.output {
        Some(file) => output_file(file, output).map_err(|e| Error::new(format!("{}: {e}", file.display())))?,
        None if did_change => outln!("{output}"),
        None => {}
    }
    previous.output = Some(output.to_string());
    Ok(did_change)
}

fn parse_options(parsed: &Parsed, style: &Style) -> std::result::Result<Options, String> {
    let cwd = context::cwd();
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
    if let Some(input) = input.as_ref().filter(|input| Some(*input) == output.as_ref()) {
        return Err(format!(
            "Specified input file {} and output file {} are identical.",
            style.highlight(&relative(input)),
            style.highlight(&relative(input))
        ));
    }

    // `--map -` is possibly an attempt to get the map in the output.
    let map = match parsed.flag_or_text("map", None) {
        None | Some(Value::Bool(false)) => MapOption::None,
        Some(Value::Bool(true)) => MapOption::Inline,
        Some(Value::Text(t)) if t == "-" => {
            return Err("Use --map without a value to inline the source map.".into());
        }
        // An empty value is not a file, as in JavaScript.
        Some(Value::Text(t)) if t.is_empty() => MapOption::None,
        Some(Value::Text(t)) => MapOption::File(resolve_path(&base, &t)),
    };

    let poll = match parsed.flag_or_text("poll", None) {
        None | Some(Value::Bool(false)) => None,
        Some(Value::Bool(true)) => Some(DEFAULT_POLL_INTERVAL_MS),
        Some(Value::Text(t)) => match t.parse::<u64>() {
            Ok(ms) if ms > 0 => Some(ms),
            _ => return Err("Specified polling interval must be a positive number.".into()),
        },
    };

    let watch = match parsed.flag_or_text("watch", Some("w")) {
        None | Some(Value::Bool(false)) => None,
        Some(Value::Bool(true)) => Some(false),
        Some(Value::Text(t)) => Some(t == "always"),
    };


    let optimizer_file = match (&input, input_is_stdin) {
        (Some(path), _) => path.to_string_lossy().into_owned(),
        (None, true) => "-".to_string(),
        (None, false) => "input.css".to_string(),
    };

    Ok(Options {
        base,
        optimizer_file,
        input,
        input_is_stdin,
        output,
        map,
        minify: parsed.flag("minify", Some("m")),
        optimize: parsed.flag("optimize", None),
        silent: parsed.flag("silent", None),
        watch,
        poll: poll.map(Duration::from_millis),
        engine: engine::select()?,
    })
}

/// Scan, build and write. The result is the CSS before the optimizer.
fn build_once(build: &mut Build, options: &Options, previous: &mut Previous) -> Result<()> {
    let candidates = build.scanner.scan();
    let css = build.compiler.try_build(&candidates)?;
    let compiler = &build.compiler;
    write(&css, &|| Ok(source_maps::to_source_map(&compiler.build_source_map()?)), options, previous)?;
    Ok(())
}

/// Give the paths of a build to the events: the directories of the
/// sources (as `watchDirectories` of the official program; only Windows
/// watches a full tree), the input and the imported stylesheets, the
/// scanned files, and their directories. The directories come first: see
/// `tw_watch_add` in `c_src/watch.c`.
fn watch_paths(build: &mut Build, events: &events::Events) {
    if !events.active() {
        return;
    }
    for source in build.scanner.get_normalized_sources() {
        events.add_tree(Path::new(&source.base));
    }
    let mut files: Vec<PathBuf> = build.full_rebuild_paths.iter().map(|(path, _)| path.clone()).collect();
    files.extend(build.scanner.get_files().into_iter().map(PathBuf::from));
    let mut dirs: std::collections::BTreeSet<&Path> = std::collections::BTreeSet::new();
    for file in &files {
        if let Some(dir) = file.parent() {
            dirs.insert(dir);
        }
    }
    for dir in dirs {
        events.add(dir, true);
    }
    for file in &files {
        events.add(file, false);
    }
}

/// Wait until the stamps of the files stay the same for `STABLE_MS` (at
/// most 10 times). A program can write a file in more than one step: a
/// shell redirect, for example, empties the file first and then writes
/// it. A build from the empty input gives a wrong output, and with no
/// `source(none)` it scans all of the directory of the input.
fn wait_until_stable(paths: &[(PathBuf, Stamp)]) {
    let stamps = || paths.iter().map(|(path, _)| stamp(path)).collect::<Vec<_>>();
    let mut last = stamps();
    for _ in 0..10 {
        std::thread::sleep(Duration::from_millis(STABLE_MS));
        let now = stamps();
        if now == last {
            return;
        }
        last = now;
    }
}

/// The watch. A tick starts at a file event of the system, or at the end
/// of the interval (`--poll`, or a system with no events: only the
/// interval). At each tick, make a full build when the input or an
/// imported stylesheet changed, else scan the changed source files and
/// build when they have new candidates. See `specs/Watch.tla`.
fn watch(mut build: Build, options: &Options, previous: &mut Previous, style: &Style, stdin_text: &Option<String>) -> i32 {
    let stdin_closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    // The end of stdin stops the watch, so a parent process that ends does
    // not leave this process. `--watch=always` keeps the watch.
    // In a run of `run_with`, there is no standard input: the caller stops
    // the watch (`Context::stop`).
    #[cfg(not(target_family = "wasm"))]
    if options.watch == Some(false) && !options.input_is_stdin && !context::embedded() {
        let flag = stdin_closed.clone();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 1024];
            let mut stdin = std::io::stdin();
            loop {
                match stdin.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(_) => {}
                    // A signal can interrupt the read: this is not the end
                    // of the input.
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        });
    }

    let mut events = events::Events::start(options.poll.is_none());
    watch_paths(&mut build, &events);

    loop {
        if stdin_closed.load(std::sync::atomic::Ordering::SeqCst) {
            events.debug("stop (the end of stdin)");
            return 0;
        }
        if context::stopped() {
            events.debug("stop (the caller)");
            return 0;
        }
        let interval = match options.poll {
            Some(interval) => interval,
            None if events.active() => Duration::from_millis(EVENTS_INTERVAL_MS),
            None => Duration::from_millis(DEFAULT_POLL_INTERVAL_MS),
        };
        let woke = events.wait(interval);
        if stdin_closed.load(std::sync::atomic::Ordering::SeqCst) {
            events.debug("stop (the end of stdin)");
            return 0;
        }
        if context::stopped() {
            events.debug("stop (the caller)");
            return 0;
        }

        let start = Instant::now();
        let full = build.full_rebuild_paths.iter().any(|(path, old)| stamp(path) != *old);
        if full {
            wait_until_stable(&build.full_rebuild_paths);
        }

        let result = if full {
            // Keep the old compiler when the new input has an error. Record
            // the new stamps before the read, so the same error is not
            // printed at each tick, and a later change is not lost.
            for (path, old) in build.full_rebuild_paths.iter_mut() {
                *old = stamp(path);
            }
            read_input(options, stdin_text).and_then(|input| create_build(options, &input)).and_then(|mut new_build| {
                let result = build_once(&mut new_build, options, previous);
                build = new_build;
                result.map(|_| true)
            })
        } else {
            let before = previous.output.clone();
            build_once(&mut build, options, previous).map(|_| previous.output != before)
        };

        // New files and directories can be there after a change.
        if woke || full {
            watch_paths(&mut build, &events);
        }

        if matches!(result, Ok(true)) {
            events.debug(if woke { "build (event)" } else { "build (interval)" });
        }
        match result {
            Ok(true) if !options.silent => errln!("Done in {}", format_duration(start.elapsed())),
            Ok(_) => {}
            Err(e) => errln!("{}", format_error(style, &e)),
        }
    }
}

/// `--version`: the version of the program, and the version of Tailwind
/// CSS that the environment selects. With the script engine, the line
/// names it.
fn print_version() -> i32 {
    match engine::select() {
        Ok(Selection::Native { version, .. }) => outln!("tailwind.com v{PROGRAM_VERSION} (tailwindcss v{version})"),
        Ok(selection) => outln!("tailwind.com v{PROGRAM_VERSION} (tailwindcss v{}, {} engine)", selection.version(), selection.engine()),
        Err(message) => {
            errln!("{message}");
            return 1;
        }
    }
    0
}

/// Run the program, as a part of another program: with the working
/// directory, the environment and the output of `context`, and with no
/// standard input. The result is the exit status. A watch (`--watch`)
/// runs until `context.stop` is true.
pub fn run_with(argv: Vec<String>, context: context::Context) -> i32 {
    let _guard = context::enter(context, argv.clone());
    run(argv)
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
        return print_version();
    }
    if parsed.flag("help", Some("h")) {
        context::out(&help_text(&style));
        return 0;
    }
    if let Some(invalid) = parsed.rest.first() {
        outln!("{}\n\n{} {invalid}", header(&style), style.dim("Invalid command:"));
        context::out(help_text(&style).split_once("\n\n").map_or("", |(_, rest)| rest));
        return 1;
    }

    let options = match parse_options(&parsed, &style) {
        Ok(options) => options,
        Err(message) => {
            errln!("{message}");
            return 1;
        }
    };

    if !options.silent {
        errln!("{}\n", header(&style));
    }

    let start = Instant::now();
    let stdin_text = if options.input_is_stdin {
        let mut text = String::new();
        if !context::embedded() {
            let _ = std::io::stdin().read_to_string(&mut text);
        }
        Some(text)
    } else {
        None
    };

    let mut previous = Previous::default();
    let first = read_input(&options, &stdin_text).and_then(|input| create_build(&options, &input)).and_then(|mut build| {
        build_once(&mut build, &options, &mut previous)?;
        Ok(build)
    });
    let build = match first {
        Ok(build) => build,
        Err(e) => {
            errln!("{}", format_error(&style, &e));
            return 1;
        }
    };
    if !options.silent {
        errln!("Done in {}", format_duration(start.elapsed()));
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
