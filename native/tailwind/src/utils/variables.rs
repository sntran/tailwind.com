//! The variables of a value: `utils/variables.ts` of Tailwind CSS.

use crate::value_parser::{self, ValueAstNode};

/// The names of the variables that `raw` uses in `var(…)`, in order:
/// `var(--a, var(--b))` gives `--a` and `--b`.
///
/// Upstream keeps the results in a map. This function has no map.
pub fn extract_used_variables(raw: &str) -> Vec<String> {
    let mut variables = Vec::new();
    // The usual value has no variable.
    if !raw.contains("var(") {
        return variables;
    }
    find_var_functions(value_parser::parse(raw), &mut variables);
    variables
}

fn find_var_functions(ast: Vec<ValueAstNode>, variables: &mut Vec<String>) {
    for node in ast {
        if let ValueAstNode::Function(function) = node {
            if function.value == "var" {
                collect_words(function.nodes, variables);
            } else {
                find_var_functions(function.nodes, variables);
            }
        }
    }
}

/// Collect each word that starts with `--`, at all depths.
fn collect_words(ast: Vec<ValueAstNode>, variables: &mut Vec<String>) {
    for node in ast {
        match node {
            ValueAstNode::Word(word) => {
                if word.value.starts_with("--") {
                    variables.push(word.value);
                }
            }
            ValueAstNode::Function(function) => collect_words(function.nodes, variables),
            ValueAstNode::Separator(_) => {}
        }
    }
}
