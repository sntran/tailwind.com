//! The order of classes: `sort.ts` of Tailwind CSS. An editor or a
//! formatter sorts the classes of a template with it, as the official
//! Prettier plugin does.

use crate::compile::compile_candidates;
use crate::design_system::DesignSystem;
use crate::hash::HashMap;
use crate::utils::js::{js_trim_end, js_trim_start};

/// The place of each class in the output: the place of its first rule. A
/// class with no rule (not a class of Tailwind CSS) has `None`. The result
/// has the classes of the input in their sequence, also a class two times.
pub fn get_class_order(design_system: &DesignSystem, classes: &[String]) -> Vec<(String, Option<u64>)> {
    // Make the rules in the order of the output.
    let sorted = compile_candidates(classes, design_system, &mut |_| {}, false);

    // The place of a class: the place of its first rule.
    let mut order: HashMap<&str, u64> = HashMap::default();
    let mut index = 0;
    for node in &sorted {
        if !order.contains_key(node.candidate.as_str()) {
            order.insert(node.candidate.as_str(), index);
            index += 1;
        }
    }

    classes.iter().map(|class| (class.clone(), order.get(class.as_str()).copied())).collect()
}

impl DesignSystem {
    /// `getClassOrder` of the design system: see [`get_class_order`].
    pub fn get_class_order(&self, classes: &[String]) -> Vec<(String, Option<u64>)> {
        get_class_order(self, classes)
    }
}

/// The options of [`sort_class_attribute`]: the options of `sortClasses` of
/// the official Prettier plugin (`prettier-plugin-tailwindcss`).
#[derive(Debug, Clone, Copy)]
pub struct SortOptions {
    /// The first class stays first (a part of a string before an
    /// interpolation).
    pub ignore_first: bool,
    /// The last class stays last.
    pub ignore_last: bool,
    /// Remove a class of Tailwind CSS that is there two times.
    pub remove_duplicates: bool,
    /// One space between the classes, and none at the start (`start`) or at
    /// the end (`end`). `None`: the spaces stay.
    pub collapse_whitespace: Option<(bool, bool)>,
}

impl Default for SortOptions {
    fn default() -> Self {
        SortOptions { ignore_first: false, ignore_last: false, remove_duplicates: true, collapse_whitespace: Some((true, true)) }
    }
}

/// True for the whitespace of the plugin: `[\t\r\f\n ]`.
fn is_space(c: char) -> bool {
    matches!(c, '\t' | '\r' | '\x0c' | '\n' | ' ')
}

/// `sortClassList` of the official Prettier plugin: the classes in the
/// order of Tailwind CSS, with `...` at the end, and with no second copy of
/// a class of Tailwind CSS. The result also has the indexes (in the sorted
/// list) of the classes that it removed.
pub fn sort_class_list(design_system: &DesignSystem, classes: &[String], remove_duplicates: bool) -> (Vec<String>, Vec<usize>) {
    let mut ordered = design_system.get_class_order(classes);
    // `Array.prototype.sort` of V8, with the comparator of the plugin: it
    // is not consistent for two `...`, so the result is the result of V8.
    crate::utils::v8_sort::sort(&mut ordered, |(name_a, a), (name_z, z)| {
        if name_a == "..." || name_a == "\u{2026}" {
            return 1;
        }
        if name_z == "..." || name_z == "\u{2026}" {
            return -1;
        }
        match (a, z) {
            (a, z) if a == z => 0,
            (None, _) => -1,
            (_, None) => 1,
            (Some(a), Some(z)) => if a < z { -1 } else { 1 },
        }
    });

    let mut removed = Vec::new();
    if remove_duplicates {
        let mut seen: HashMap<String, ()> = HashMap::default();
        let mut kept = Vec::with_capacity(ordered.len());
        for (index, (class, order)) in ordered.into_iter().enumerate() {
            if seen.contains_key(&class) {
                removed.push(index);
                continue;
            }
            // Only a class of Tailwind CSS is a duplicate.
            if order.is_some() {
                seen.insert(class.clone(), ());
            }
            kept.push((class, order));
        }
        ordered = kept;
    }
    (ordered.into_iter().map(|(class, _)| class).collect(), removed)
}

/// `sortClasses` of the official Prettier plugin: sort the classes of the
/// value of one `class` attribute, and keep its whitespace as the options
/// say. `sortClassAttributes` of its sorter API calls it with the default
/// options.
pub fn sort_class_attribute(design_system: &DesignSystem, value: &str, options: SortOptions) -> String {
    if value.is_empty() || value.contains("{{") {
        return value.to_string();
    }
    let collapse = options.collapse_whitespace;
    if collapse.is_some() && value.chars().all(is_space) {
        return " ".to_string();
    }

    // `split(/([\t\r\f\n ]+)/)`: the classes at the even places, and the
    // whitespace between them at the odd places.
    let mut classes: Vec<String> = Vec::new();
    let mut whitespace: Vec<String> = Vec::new();
    let mut part = String::new();
    let mut in_space = false;
    for c in value.chars() {
        if is_space(c) != in_space {
            if in_space {
                whitespace.push(std::mem::take(&mut part));
            } else {
                classes.push(std::mem::take(&mut part));
            }
            in_space = !in_space;
        }
        part.push(c);
    }
    if in_space {
        whitespace.push(part);
        classes.push(String::new());
    } else {
        classes.push(part);
    }
    if classes.last().is_some_and(String::is_empty) {
        classes.pop();
    }
    if collapse.is_some() {
        whitespace = whitespace.iter().map(|_| " ".to_string()).collect();
    }

    let mut prefix = String::new();
    if options.ignore_first {
        if !classes.is_empty() {
            prefix.push_str(&classes.remove(0));
        }
        if !whitespace.is_empty() {
            prefix.push_str(&whitespace.remove(0));
        }
    }
    let mut suffix = String::new();
    if options.ignore_last {
        if let Some(space) = whitespace.pop() {
            suffix.push_str(&space);
        }
        if let Some(class) = classes.pop() {
            suffix.push_str(&class);
        }
    }

    let (list, removed) = sort_class_list(design_system, &classes, options.remove_duplicates);

    // The plugin removes the whitespace before each removed class with the
    // indexes of the sorted list: `!removedIndices.has(index + 1)`.
    let whitespace: Vec<String> =
        whitespace.into_iter().enumerate().filter(|(index, _)| !removed.contains(&(index + 1))).map(|(_, space)| space).collect();

    let mut result = String::new();
    for (index, class) in list.iter().enumerate() {
        result.push_str(class);
        if let Some(space) = whitespace.get(index) {
            result.push_str(space);
        }
    }

    if let Some((start, end)) = collapse {
        prefix = replace_end_spaces(&prefix);
        suffix = replace_start_spaces(&suffix);
        let trimmed_start = js_trim_start(&result);
        let had_start = trimmed_start.len() != result.len();
        let mut text = if had_start && !start { format!(" {trimmed_start}") } else { trimmed_start.to_string() };
        let trimmed_end = js_trim_end(&text).len();
        if trimmed_end != text.len() {
            text.truncate(trimmed_end);
            if !end {
                text.push(' ');
            }
        }
        result = text;
    }
    format!("{prefix}{result}{suffix}")
}

/// `sortTemplateLiteral` of the official Prettier plugin: sort the fixed
/// parts of a string with interpolations (`a #{@x} b` in Elixir, `a ${x} b`
/// in JavaScript). A part keeps its first class when it touches the
/// interpolation before it, and its last class when it touches the
/// interpolation after it. The whitespace collapses at the start of the
/// first part and at the end of the last part only.
pub fn sort_class_template(design_system: &DesignSystem, parts: &[String], remove_duplicates: bool, collapse: bool) -> Vec<String> {
    let count = parts.len();
    parts
        .iter()
        .enumerate()
        .map(|(index, part)| {
            let options = SortOptions {
                ignore_first: index > 0 && !part.starts_with(is_js_space),
                ignore_last: index + 1 < count && !part.ends_with(is_js_space),
                remove_duplicates,
                collapse_whitespace: collapse.then_some((index == 0, index + 1 >= count)),
            };
            sort_class_attribute(design_system, part, options)
        })
        .collect()
}

/// `/\s/` of JavaScript.
fn is_js_space(c: char) -> bool {
    crate::utils::js::is_js_whitespace(c) || crate::utils::js::is_js_line_terminator(c)
}

/// `text.replace(/\s+$/g, ' ')`.
fn replace_end_spaces(text: &str) -> String {
    let trimmed = js_trim_end(text);
    if trimmed.len() == text.len() { text.to_string() } else { format!("{trimmed} ") }
}

/// `text.replace(/^\s+/g, ' ')`.
fn replace_start_spaces(text: &str) -> String {
    let trimmed = js_trim_start(text);
    if trimmed.len() == text.len() { text.to_string() } else { format!(" {trimmed}") }
}

/// The sort of the tests of Tailwind CSS (`sort.test.ts`): first the
/// classes with no order, in their sequence, then the other classes by
/// their order. The sort is stable.
pub fn sort_classes(order: Vec<(String, Option<u64>)>) -> Vec<String> {
    let mut order = order;
    order.sort_by(|(_, a), (_, z)| match (a, z) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(a), Some(z)) => a.cmp(z),
    });
    order.into_iter().map(|(class, _)| class).collect()
}
