//! The command `canonicalize`: `commands/canonicalize` of the official
//! program. It gives the canonical form of groups of candidates, sorted in
//! the order of the classes:
//!
//! ```text
//! tailwindcss canonicalize "py-3 p-1 px-3"           p-3
//! tailwindcss canonicalize --css app.css < groups.txt
//! tailwindcss canonicalize --stream --format jsonl
//! ```
//!
//! The algorithm is the algorithm of the newest version of Tailwind CSS,
//! with the utilities, the variants and the theme of the selected version
//! (`crate::canonicalize`).

use super::{args, context, engine, header, modules, resolve, Style};
use crate::canonicalize::CanonicalizeOptions;
use crate::design_system::DesignSystem;
use crate::utils::compare::compare;
use crate::utils::segment::segment;
use crate::{CompileOptions, Error};
use std::io::BufRead;
use std::path::Path;
use std::rc::Rc;

/// The usage lines of the command.
pub const USAGE: [&str; 3] = [
    "tailwindcss canonicalize [classes...]",
    "tailwindcss canonicalize --css input.css [classes...]",
    "tailwindcss canonicalize --stream [--css input.css]",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Text,
    Json,
    Jsonl,
}

/// The result of one group: `CandidateGroupResult`.
struct GroupResult {
    input: String,
    output: String,
    changed: bool,
}

impl GroupResult {
    /// `JSON.stringify(result)`.
    fn json(&self) -> String {
        format!("{{\"input\":{},\"output\":{},\"changed\":{}}}", text(&self.input), text(&self.output), self.changed)
    }

    /// `JSON.stringify(result, null, 2)`.
    fn pretty_json(&self) -> String {
        format!("{{\n  \"input\": {},\n  \"output\": {},\n  \"changed\": {}\n}}", text(&self.input), text(&self.output), self.changed)
    }
}

/// A JSON string.
fn text(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

fn help_text(style: &Style) -> String {
    let mut out = header(style);
    out.push_str("\n\n");
    out.push_str(&style.dim("Usage:"));
    out.push('\n');
    for usage in USAGE {
        out.push_str(&format!("  {usage}\n"));
    }
    out.push('\n');
    out.push_str(&style.dim("Options:"));
    out.push('\n');
    for (flags, description) in [
        ("    --css", "CSS entry file used to load the Tailwind design system (defaults to `@import \"tailwindcss\";`)"),
        ("    --format", "Output format: `text`, `json` or `jsonl` [default: `text`]"),
        ("    --stream", "Read candidate groups from stdin line by line and write results to stdout"),
        ("-h, --help", "Display usage information"),
    ] {
        out.push_str(&format!("  {flags:<16} {description}\n"));
    }
    out
}

/// Run the command with the arguments after `canonicalize`. The result
/// is the exit status. As upstream, the result goes to the standard
/// output with a line end, and an error to the standard error.
pub fn run(argv: Vec<String>) -> i32 {
    let style = Style::new();
    let (status, stdout, stderr) = run_command_line(&argv, &style);
    if !stdout.is_empty() {
        context::out(&format!("{stdout}\n"));
    }
    if !stderr.is_empty() {
        context::err(&format!("{stderr}\n"));
    }
    status
}

fn stdin_is_terminal() -> bool {
    use std::io::IsTerminal;
    !context::embedded() && std::io::stdin().is_terminal()
}

/// True when the run has a standard input: the process, or a context
/// with an input.
fn has_stdin() -> bool {
    !context::embedded() || context::has_stdin()
}

fn stdout_is_terminal() -> bool {
    use std::io::IsTerminal;
    !context::embedded() && std::io::stdout().is_terminal()
}

/// `runCommandLine`: the exit status, the standard output and the
/// standard error.
fn run_command_line(argv: &[String], style: &Style) -> (i32, String, String) {
    let parsed = args::parse(argv);
    let result = (|| -> Result<(i32, String, String), String> {
        let format = match parsed.text("format", None).as_deref().unwrap_or("text") {
            "text" => Format::Text,
            "json" => Format::Json,
            "jsonl" => Format::Jsonl,
            other => return Err(format!("Invalid value for --format: {other}")),
        };

        if (stdout_is_terminal() && argv.is_empty()) || parsed.flag("help", Some("h")) {
            return Ok((0, help_text(style).trim_end().to_string(), String::new()));
        }

        let css = parsed.get("css", None).map(|_| parsed.text("css", None).unwrap_or_default());

        if parsed.flag("stream", None) {
            let design_system = load_design_system(css.as_deref())?;
            stream(&design_system, format)?;
            return Ok((0, String::new(), String::new()));
        }

        let inputs: Vec<String> = if !parsed.rest.is_empty() { parsed.rest.clone() } else { read_candidate_groups() };
        if inputs.is_empty() {
            return Ok((1, help_text(style).trim_end().to_string(), "No candidate groups provided".into()));
        }

        let design_system = load_design_system(css.as_deref())?;
        let mut results = Vec::with_capacity(inputs.len());
        for input in inputs {
            results.push(group_result(&design_system, &input)?);
        }
        Ok((0, format_results(&results, format), String::new()))
    })();
    result.unwrap_or_else(|message| (1, String::new(), message))
}

/// `readCandidateGroups`: the lines of the standard input with text,
/// with no whitespace at the two ends. A terminal gives no groups.
fn read_candidate_groups() -> Vec<String> {
    if stdin_is_terminal() || !has_stdin() {
        return Vec::new();
    }
    let mut text = String::new();
    let _ = std::io::Read::read_to_string(&mut context::Stdin, &mut text);
    split_candidate_groups(&text)
}

fn split_candidate_groups(input: &str) -> Vec<String> {
    input.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line)).map(js_trim).filter(|l| !l.is_empty()).map(String::from).collect()
}

fn js_trim(text: &str) -> &str {
    crate::utils::js::js_trim(text)
}

/// `--stream`: one result for each line of the standard input, at once.
fn stream(design_system: &DesignSystem, format: Format) -> Result<(), String> {
    if format == Format::Json {
        context::out("[");
    }
    let mut first = true;
    if has_stdin() {
        let mut reader = std::io::BufReader::new(context::Stdin);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let text = String::from_utf8_lossy(&buffer);
            // A line ends at `\n`, `\r\n` or `\r`, as in `readline`.
            let text = text.strip_suffix('\n').unwrap_or(&text);
            let text = text.strip_suffix('\r').unwrap_or(text);
            for line in text.split('\r') {
                let result = group_result(design_system, line)?;
                match format {
                    Format::Text => context::out(&format!("{}\n", result.output)),
                    Format::Jsonl => context::out(&format!("{}\n", result.json())),
                    Format::Json => {
                        context::out(if first { "\n" } else { ",\n" });
                        context::out(&indent(&result.pretty_json(), 2));
                        first = false;
                    }
                }
            }
        }
    }
    if format == Format::Json {
        context::out(if first { "]" } else { "\n]" });
    }
    Ok(())
}

fn indent(text: &str, size: usize) -> String {
    let prefix = " ".repeat(size);
    text.split('\n').map(|line| format!("{prefix}{line}")).collect::<Vec<_>>().join("\n")
}

/// `formatCandidateResults`.
fn format_results(results: &[GroupResult], format: Format) -> String {
    match format {
        Format::Json if results.is_empty() => "[]".to_string(),
        Format::Json => format!("[\n{}\n]", results.iter().map(|r| indent(&r.pretty_json(), 2)).collect::<Vec<_>>().join(",\n")),
        Format::Jsonl => results.iter().map(GroupResult::json).collect::<Vec<_>>().join("\n"),
        Format::Text => results.iter().map(|r| r.output.as_str()).collect::<Vec<_>>().join("\n"),
    }
}

/// `createCandidateGroupResult`.
fn group_result(design_system: &DesignSystem, input: &str) -> Result<GroupResult, String> {
    let output = canonicalize(design_system, input)?;
    Ok(GroupResult { changed: output != input, input: input.to_string(), output })
}

/// `canonicalize`: the canonical form of the candidates of a group, with
/// `collapse` and `logicalToPhysical`, sorted in the order of the classes.
fn canonicalize(design_system: &DesignSystem, input: &str) -> Result<String, String> {
    let candidates: Vec<String> = {
        let input = js_trim(input);
        if input.is_empty() {
            Vec::new()
        } else {
            segment(input, b' ').into_iter().map(js_trim).filter(|c| !c.is_empty()).map(String::from).collect()
        }
    };
    let options = CanonicalizeOptions { rem: None, collapse: true, logical_to_physical: true };
    let candidates = design_system.canonicalize_candidates(&candidates, &options);
    let order = design_system.get_class_order(&candidates);
    if let Some(error) = design_system.take_js_error() {
        return Err(error.message);
    }
    Ok(default_sort(order).join(" "))
}

/// `defaultSort`: the classes with no order first, then by their order,
/// then by their text.
fn default_sort(mut entries: Vec<(String, Option<u64>)>) -> Vec<String> {
    // The sort of V8: the compare function of an old version is not a
    // consistent order (`intellisense.rs`).
    crate::utils::v8_sort::sort(&mut entries, |(a, order_a), (z, order_z)| {
        (match (order_a, order_z) {
            _ if order_a == order_z => compare(a, z),
            (None, _) => std::cmp::Ordering::Less,
            (_, None) => std::cmp::Ordering::Greater,
            (Some(x), Some(y)) => x.cmp(y).then_with(|| compare(a, z)),
        }) as i64
    });
    entries.into_iter().map(|(candidate, _)| candidate).collect()
}

/// `loadDesignSystem`: the design system of the file of `--css`, or of
/// `@import "tailwindcss";` in the working directory.
fn load_design_system(css_file: Option<&str>) -> Result<DesignSystem, String> {
    let selection = engine::select()?;
    // The Rust compiler has the utilities and the variants of each
    // version. The official compiler of a package (the script engine)
    // has its own canonical form, or none: this command does not run it.
    let version = match &selection {
        engine::Selection::Native { version, .. } => *version,
        engine::Selection::Script(_) => {
            return Err("The canonicalize command uses the Rust compiler. It does not run with the script engine.".into())
        }
    };

    let cwd = context::cwd();
    let (content, base) = match css_file {
        None => ("@import 'tailwindcss';".to_string(), cwd),
        Some(file) => {
            let path = cwd.join(file);
            let content = std::fs::read_to_string(&path)
                .map_err(|_| format!("ENOENT: no such file or directory, open '{}'", path.display()))?;
            (content, path.parent().map(Path::to_path_buf).unwrap_or(cwd))
        }
    };

    let load = |id: &str, base: &str| resolve::load_stylesheet(id, base, selection.package(), &mut |_| {});
    let design_system = crate::index::load_design_system(
        &content,
        CompileOptions {
            base: base.to_string_lossy().into_owned(),
            polyfills: crate::POLYFILLS_ALL,
            load_stylesheet: Some(&load),
            test_mode: false,
            from: None,
            module_loader: Some(Rc::new(modules::FileSystem::with_console_to_stderr())),
            version: Some(version),
        },
    )
    .map_err(|error: Error| error.message)?;
    Ok(design_system)
}
