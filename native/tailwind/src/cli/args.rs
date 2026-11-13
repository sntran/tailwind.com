//! The parser of the command line: `utils/args.ts` of `@tailwindcss/cli`,
//! which uses the package `mri`.

use crate::hash::HashMap;

/// The value of an option.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Text(String),
}

/// The parsed command line: the options by name (with no dashes), and the
/// other arguments.
#[derive(Debug, Default)]
pub struct Parsed {
    pub options: HashMap<String, Value>,
    pub rest: Vec<String>,
}

impl Parsed {
    /// The value of an option, by its long name or its alias.
    pub fn get(&self, name: &str, alias: Option<&str>) -> Option<&Value> {
        self.options.get(name).or_else(|| alias.and_then(|a| self.options.get(a)))
    }

    /// An option of the type `boolean`.
    pub fn flag(&self, name: &str, alias: Option<&str>) -> bool {
        match self.get(name, alias) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Text(t)) => t == "true",
            None => false,
        }
    }

    /// An option of the type `string`.
    pub fn text(&self, name: &str, alias: Option<&str>) -> Option<String> {
        match self.get(name, alias) {
            Some(Value::Bool(b)) => Some(b.to_string()),
            Some(Value::Text(t)) => Some(t.clone()),
            None => None,
        }
    }

    /// An option of the type `boolean | string`.
    pub fn flag_or_text(&self, name: &str, alias: Option<&str>) -> Option<Value> {
        match self.get(name, alias) {
            Some(Value::Text(t)) if t == "true" => Some(Value::Bool(true)),
            Some(Value::Text(t)) if t == "false" => Some(Value::Bool(false)),
            other => other.cloned(),
        }
    }
}

/// Parse the arguments. An option gets the next argument as its value when
/// that argument does not start with `-`. A lone `-` is a value.
pub fn parse(argv: &[String]) -> Parsed {
    let mut parsed = Parsed::default();
    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        i += 1;

        if arg == "--" {
            parsed.rest.extend(argv[i..].iter().cloned());
            break;
        }
        if arg == "-" || !arg.starts_with('-') {
            parsed.rest.push(arg.clone());
            continue;
        }

        let long = arg.starts_with("--");
        let body = arg.trim_start_matches('-');
        let (names, inline) = match body.split_once('=') {
            Some((names, value)) => (names, Some(value.to_string())),
            None => (body, None),
        };

        // `--no-name` is `name: false`.
        if long {
            if let Some(name) = names.strip_prefix("no-") {
                parsed.options.insert(name.to_string(), Value::Bool(false));
                continue;
            }
        }

        // `-abc` is the options `a`, `b` and `c`: the value goes to the last.
        let names: Vec<String> = if long { vec![names.to_string()] } else { names.chars().map(String::from).collect() };
        let Some((last, first)) = names.split_last() else { continue };
        for name in first {
            parsed.options.insert(name.clone(), Value::Bool(true));
        }

        let value = match inline {
            Some(value) => Value::Text(value),
            None => match argv.get(i) {
                Some(next) if next == "-" || !next.starts_with('-') => {
                    i += 1;
                    Value::Text(next.clone())
                }
                _ => Value::Bool(true),
            },
        };
        parsed.options.insert(last.clone(), value);
    }
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Parsed {
        parse(&list.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn long_options() {
        let p = args(&["--input=a.css", "--output", "b.css", "--minify"]);
        assert_eq!(p.text("input", Some("i")).as_deref(), Some("a.css"));
        assert_eq!(p.text("output", Some("o")).as_deref(), Some("b.css"));
        assert!(p.flag("minify", Some("m")));
        assert!(!p.flag("optimize", None));
    }

    #[test]
    fn short_options_and_a_dash_value() {
        let p = args(&["-i", "-", "-mo", "out.css"]);
        assert_eq!(p.text("input", Some("i")).as_deref(), Some("-"));
        assert!(p.flag("minify", Some("m")));
        assert_eq!(p.text("output", Some("o")).as_deref(), Some("out.css"));
    }

    #[test]
    fn an_option_with_or_without_a_value() {
        assert_eq!(args(&["--watch"]).flag_or_text("watch", Some("w")), Some(Value::Bool(true)));
        assert_eq!(args(&["--watch=always"]).flag_or_text("watch", Some("w")), Some(Value::Text("always".into())));
        assert_eq!(args(&["-w", "always"]).flag_or_text("watch", Some("w")), Some(Value::Text("always".into())));
        assert_eq!(args(&["--watch", "--minify"]).flag_or_text("watch", Some("w")), Some(Value::Bool(true)));
        assert_eq!(args(&[]).flag_or_text("watch", Some("w")), None);
    }

    #[test]
    fn the_other_arguments() {
        let p = args(&["build", "--no-minify", "--", "--x"]);
        assert_eq!(p.rest, vec!["build", "--x"]);
        assert!(!p.flag("minify", Some("m")));
    }
}
