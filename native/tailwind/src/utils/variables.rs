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
    // Upstream walks the tree with no recursion, and thus reads a value
    // of any depth. This loop does the same: each level of the tree is an
    // iterator on the stack, and the tree drops one level at a time.
    // The flag is true in `var(…)`: there, each word that starts with
    // `--` is a variable, at all depths.
    let mut stack: Vec<(std::vec::IntoIter<ValueAstNode>, bool)> = vec![(value_parser::parse_deep(raw).into_iter(), false)];
    while let Some((nodes, in_var)) = stack.last_mut() {
        let in_var = *in_var;
        match nodes.next() {
            None => {
                stack.pop();
            }
            Some(ValueAstNode::Word(word)) => {
                if in_var && word.value.starts_with("--") {
                    variables.push(word.value);
                }
            }
            Some(ValueAstNode::Function(function)) => {
                let in_var = in_var || function.value == "var";
                stack.push((function.nodes.into_iter(), in_var));
            }
            Some(ValueAstNode::Separator(_)) => {}
        }
    }
    variables
}
