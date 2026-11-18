//! The text styles and the help text of the official program:
//! `utils/renderer.ts`, `commands/help/index.ts` of `@tailwindcss/cli`, and
//! the package `picocolors` that they use.

use super::context;

/// The colors of `picocolors`. They are on by the rule of its
/// `isColorSupported`: no `NO_COLOR` and no `--no-color`, and one of
/// `FORCE_COLOR`, `--color`, Windows, a standard output that is a terminal
/// (when `TERM` is not `dumb`), or `CI`.
pub struct Style {
    on: bool,
}

impl Style {
    /// `argv`: the arguments of the program.
    pub fn new(argv: &[String]) -> Self {
        let set = |name: &str| context::var(name).is_some_and(|value| !value.is_empty());
        let has = |arg: &str| argv.iter().any(|a| a == arg);
        let off = set("NO_COLOR") || has("--no-color");
        let terminal = context::terminal() && context::var("TERM").as_deref() != Some("dumb");
        let on = set("FORCE_COLOR") || has("--color") || super::modules::windows() || terminal || set("CI");
        Self { on: !off && on }
    }

    /// `formatter(open, close, replace)` of `picocolors`: a style in the
    /// text has the same end, so the text puts `replace` in its place.
    fn format(&self, open: &str, close: &str, replace: &str, input: &str) -> String {
        if !self.on {
            return input.to_string();
        }
        // `string.indexOf(close, open.length)`: the position is in UTF-16
        // units.
        let from = input.char_indices().scan(0, |units, (index, c)| {
            let at = *units;
            *units += c.len_utf16();
            Some((index, at))
        });
        let from = from.into_iter().find(|(_, at)| *at >= open.len()).map_or(input.len(), |(index, _)| index);
        let Some(first) = input[from..].find(close).map(|index| from + index) else {
            return format!("{open}{input}{close}");
        };
        let mut result = String::from(open);
        let mut cursor = 0;
        let mut index = Some(first);
        while let Some(at) = index {
            result.push_str(&input[cursor..at]);
            result.push_str(replace);
            cursor = at + close.len();
            index = input[cursor..].find(close).map(|i| cursor + i);
        }
        result.push_str(&input[cursor..]);
        result.push_str(close);
        result
    }

    pub fn bold(&self, text: &str) -> String {
        self.format("\x1b[1m", "\x1b[22m", "\x1b[22m\x1b[1m", text)
    }
    pub fn dim(&self, text: &str) -> String {
        self.format("\x1b[2m", "\x1b[22m", "\x1b[22m\x1b[2m", text)
    }
    pub fn italic(&self, text: &str) -> String {
        self.format("\x1b[3m", "\x1b[23m", "\x1b[3m", text)
    }
    pub fn red(&self, text: &str) -> String {
        self.format("\x1b[31m", "\x1b[39m", "\x1b[31m", text)
    }
    pub fn green(&self, text: &str) -> String {
        self.format("\x1b[32m", "\x1b[39m", "\x1b[32m", text)
    }
    pub fn yellow(&self, text: &str) -> String {
        self.format("\x1b[33m", "\x1b[39m", "\x1b[33m", text)
    }
    pub fn blue(&self, text: &str) -> String {
        self.format("\x1b[34m", "\x1b[39m", "\x1b[34m", text)
    }
    pub fn gray(&self, text: &str) -> String {
        self.format("\x1b[90m", "\x1b[39m", "\x1b[90m", text)
    }

    /// `highlight` of the renderer: a file name or a value in backticks.
    pub fn highlight(&self, text: &str) -> String {
        format!("{}{}{}", self.dim(&self.blue("`")), self.blue(text), self.dim(&self.blue("`")))
    }

    /// `header` of the renderer: the first line of the output, with the
    /// version of Tailwind CSS. The Elixir package `tailwind` reads the
    /// version from this line.
    pub fn header(&self, version: &str) -> String {
        format!("{} tailwindcss {}", self.italic(&self.bold(&self.blue("\u{2248}"))), self.blue(&format!("v{version}")))
    }
}

/// `dim` and `red` of the command `build`: these styles have no condition.
pub fn always_dim(text: &str) -> String {
    format!("\x1b[2m{text}\x1b[22m")
}

pub fn always_red(text: &str) -> String {
    format!("\x1b[31m{text}\x1b[39m")
}

/// `stripVTControlCharacters(text).length` of Node.js: the UTF-16 length
/// of the text with no escape sequences (`ESC [ … letter`).
fn visible_len(text: &str) -> usize {
    let mut len = 0;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for c in chars.by_ref() {
                if ('@'..='~').contains(&c) {
                    break;
                }
            }
            continue;
        }
        len += c.len_utf16();
    }
    len
}

/// `wordWrap` of the renderer: the words of `text` (split at each space)
/// in lines of at most `width`. A word that is longer than `width` has
/// its own line.
pub fn word_wrap(text: &str, width: i64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut line_len: i64 = 0;
    for word in text.split(' ') {
        let word_len = visible_len(word) as i64;
        if line_len > 0 && line_len + word_len + 1 > width {
            lines.push(std::mem::take(&mut line));
            line_len = 0;
        }
        if line_len > 0 {
            line.push(' ');
        }
        line.push_str(word);
        line_len += word_len + if line_len > 0 { 1 } else { 0 };
    }
    if line_len > 0 {
        lines.push(line);
    }
    lines
}

/// An option of the help text (`Arg` of `utils/args.ts`).
pub struct Arg {
    pub flag: &'static str,
    pub alias: Option<&'static str>,
    pub description: &'static str,
    pub default: Option<&'static str>,
    pub values: &'static [&'static str],
}

/// The indent of each line (`UI.indent`).
const INDENT: usize = 2;

/// `help` of the official program: the lines of the text, with no line
/// end after the last line. `invalid`: the invalid command. The width is
/// the width of the terminal of the standard output, else 80.
pub fn help(style: &Style, version: &str, invalid: Option<&str>, usage: &[&str], options: &[Arg]) -> String {
    let width = context::columns(context::Stream::Stdout).unwrap_or(80) as i64;
    let mut lines: Vec<String> = Vec::new();
    let indent = |value: &str, offset: usize| format!("{}{value}", " ".repeat(offset + INDENT));

    lines.push(style.header(version));

    if let Some(invalid) = invalid {
        lines.push(String::new());
        lines.push(format!("{} {invalid}", style.dim("Invalid command:")));
    }

    if !usage.is_empty() {
        lines.push(String::new());
        lines.push(style.dim("Usage:"));
        for (idx, example) in usage.iter().enumerate() {
            let split = example.find('[').unwrap_or(example.len());
            let (command, options) = example.split_at(split);
            let options = dim_brackets(style, options);
            let command_len = visible_len(command);
            let wrapped = word_wrap(&options, width - INDENT as i64 - command_len as i64 - 1);
            if wrapped.len() > 1 && idx != 0 {
                lines.push(String::new());
            }
            let mut wrapped = wrapped.into_iter();
            // `${command}${lines.shift()}`: `undefined` when the example
            // has no options.
            let first = wrapped.next().unwrap_or_else(|| "undefined".to_string());
            lines.push(indent(&format!("{command}{first}"), 0));
            for line in wrapped {
                lines.push(indent(&line, command_len));
            }
        }
    }

    if !options.is_empty() {
        let max_alias = options.iter().filter_map(|arg| arg.alias).map(visible_len).max().unwrap_or(0);
        let strings: Vec<String> = options
            .iter()
            .map(|arg| {
                let mut flag = arg.flag.to_string();
                if !arg.values.is_empty() {
                    flag.push_str(&format!("[={}]", arg.values.join(", ")));
                }
                match arg.alias {
                    Some(alias) => format!("{alias:>max_alias$}, {flag}"),
                    None => format!("{}{flag}", " ".repeat(max_alias + 2)),
                }
            })
            .collect();
        let max_option = strings.iter().map(|option| visible_len(option)).max().unwrap_or(0);

        lines.push(String::new());
        lines.push(style.dim("Options:"));

        const MINIMUM_GAP: usize = 8;
        for (arg, option) in options.iter().zip(&strings) {
            let option_len = visible_len(option);
            let dot_count = MINIMUM_GAP + (max_option - option_len);
            let spaces = 2;
            let available = width - option_len as i64 - dot_count as i64 - spaces - INDENT as i64;
            let text = match arg.default {
                Some(value) => format!("{} {}", arg.description, style.dim(&format!("[default:\u{202f}{}]", style.highlight(value)))),
                None => arg.description.to_string(),
            };
            let mut wrapped = word_wrap(&text, available).into_iter();
            let first = wrapped.next().unwrap_or_else(|| "undefined".to_string());
            let dots = style.dim(&style.gray("\u{b7}")).repeat(dot_count);
            lines.push(indent(&format!("{} {dots} {first}", style.blue(option)), 0));
            for line in wrapped {
                lines.push(indent(&format!("{}{line}", " ".repeat(option_len + dot_count + spaces as usize)), 0));
            }
        }
    }

    lines.join("\n")
}

/// `options.replace(/\[.*?\]/g, (option) => pc.dim(option))`.
fn dim_brackets(style: &Style, text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find(']') else { break };
        out.push_str(&rest[..open]);
        out.push_str(&style.dim(&rest[open..open + close + 1]));
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_wrap_is_the_word_wrap_of_the_renderer() {
        assert_eq!(word_wrap("a b c", 3), ["a b", "c"]);
        assert_eq!(word_wrap("  a", 80), ["a"]);
        assert_eq!(word_wrap("a  b ", 80), ["a  b "]);
        assert_eq!(word_wrap("   ", 80), Vec::<String>::new());
        assert_eq!(word_wrap("long word", 2), ["long", "word"]);
        assert_eq!(word_wrap("\x1b[2m[x]\x1b[22m y", 5), ["\x1b[2m[x]\x1b[22m y"]);
    }

    #[test]
    fn a_style_in_a_style_starts_again_after_its_end() {
        let style = Style { on: true };
        assert_eq!(style.dim(&style.blue("`")), "\x1b[2m\x1b[34m`\x1b[39m\x1b[22m");
        assert_eq!(style.dim(&format!("a{}b", style.dim("c"))), "\x1b[2ma\x1b[2mc\x1b[22m\x1b[2mb\x1b[22m");
        assert_eq!(Style { on: false }.dim("a"), "a");
    }
}
