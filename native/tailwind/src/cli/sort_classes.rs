//! `--sort-classes`: the sort of the classes of templates, as the official
//! Prettier plugin (`prettier-plugin-tailwindcss`) sorts them, with the
//! design system of the input (its theme, plugins and version).
//!
//! The program reads one request on each line of its standard input, and
//! writes one answer on a line of its standard output, until the end of
//! the input. A request is a JSON object, and each part is optional:
//!
//! ```text
//! {"attributes": ["py-3 p-1 px-3"], "templates": [["px-3 ", " p-1"]],
//!  "lists": [["py-3", "p-1"]], "preserveWhitespace": false,
//!  "preserveDuplicates": false}
//! ```
//!
//! - `attributes`: values of `class` attributes (`sortClassAttributes` of
//!   the sorter API of the plugin).
//! - `templates`: the fixed parts of a string with interpolations, as
//!   `"px-3 #{@a} p-1"` in Elixir (`sortTemplateLiteral` of the plugin).
//! - `lists`: lists of classes (`sortClassLists` of the sorter API).
//!
//! The answer has the same parts, sorted: `{"attributes": ["p-1 px-3
//! py-3"], …}`, or `{"error": "…"}` for a request that is not valid.

use serde_json::{json, Value};

use super::context;
use crate::design_system::DesignSystem;
use crate::sort::{sort_class_attribute, sort_class_list, sort_class_template, SortOptions};

/// Answer the requests of the standard input until its end. The result is
/// the exit status.
pub fn serve(design_system: &DesignSystem) -> i32 {
    let mut line = String::new();
    loop {
        line.clear();
        match context::read_line(&mut line) {
            Ok(0) => return 0,
            Ok(_) => {}
            Err(error) => {
                context::err(&format!("{error}\n"));
                return 1;
            }
        }
        if line.trim().is_empty() {
            continue;
        }
        let answer = answer(design_system, line.trim_end_matches(['\n', '\r']));
        context::out(&format!("{answer}\n"));
    }
}

/// The answer to one request.
pub fn answer(design_system: &DesignSystem, request: &str) -> Value {
    let request: Value = match serde_json::from_str(request) {
        Ok(Value::Object(request)) => Value::Object(request),
        Ok(_) => return json!({ "error": "A request is a JSON object." }),
        Err(error) => return json!({ "error": format!("A request is not valid JSON: {error}") }),
    };
    let preserve_whitespace = request["preserveWhitespace"].as_bool().unwrap_or(false);
    let preserve_duplicates = request["preserveDuplicates"].as_bool().unwrap_or(false);
    let options = SortOptions {
        remove_duplicates: !preserve_duplicates,
        collapse_whitespace: (!preserve_whitespace).then_some((true, true)),
        ..SortOptions::default()
    };

    let mut answer = serde_json::Map::new();
    if let Some(value) = request.get("attributes") {
        let Some(attributes) = strings(value) else { return json!({ "error": "`attributes` is a list of strings." }) };
        let sorted: Vec<String> = attributes.iter().map(|value| sort_class_attribute(design_system, value, options)).collect();
        answer.insert("attributes".into(), json!(sorted));
    }
    if let Some(value) = request.get("templates") {
        let Some(templates) = lists(value) else { return json!({ "error": "`templates` is a list of lists of strings." }) };
        let sorted: Vec<Vec<String>> = templates
            .iter()
            .map(|parts| sort_class_template(design_system, parts, !preserve_duplicates, !preserve_whitespace))
            .collect();
        answer.insert("templates".into(), json!(sorted));
    }
    if let Some(value) = request.get("lists") {
        let Some(classes) = lists(value) else { return json!({ "error": "`lists` is a list of lists of strings." }) };
        let sorted: Vec<Vec<String>> =
            classes.iter().map(|list| sort_class_list(design_system, list, !preserve_duplicates).0).collect();
        answer.insert("lists".into(), json!(sorted));
    }
    Value::Object(answer)
}

fn strings(value: &Value) -> Option<Vec<String>> {
    value.as_array()?.iter().map(|item| item.as_str().map(String::from)).collect()
}

fn lists(value: &Value) -> Option<Vec<Vec<String>>> {
    value.as_array()?.iter().map(strings).collect()
}
