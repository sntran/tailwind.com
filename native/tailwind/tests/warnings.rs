//! The warnings of the compiler: the texts that Tailwind CSS gives to
//! `console.warn` for a data type that `--value(…)` or `--modifier(…)` of
//! `@utility` cannot have (`utilities.ts`). The official program prints
//! the same texts for `tests/upstream/cli/warning-value-type`.

use tailwind::{compile, CompileOptions};

fn warnings(css: &str) -> Vec<String> {
    let compiler = compile(css, CompileOptions { test_mode: true, ..Default::default() }).unwrap();
    let warnings = compiler.take_warnings();
    // The compiler gives each warning one time.
    assert!(compiler.take_warnings().is_empty());
    warnings
}

const TYPES: &str = "Only valid data types are: \"number\", \"integer\", \"ratio\", \"percentage\".\n";

#[test]
fn a_data_type_that_a_bare_value_cannot_have_gives_a_warning() {
    let list = warnings("@utility tab-* {\n  tab-size: --value(integer, size, [integer]);\n}\n");
    assert_eq!(
        list,
        [
            format!("Unsupported bare value data type: \"size\".\n{TYPES}"),
            "```css\n--value(integer,size,[integer])\n                ^^^^\n```".to_string(),
        ]
    );

    // Each function and each word has its own warning, in the sequence of
    // the declarations.
    let list = warnings("@utility gap-x-* {\n  column-gap: --value(--spacing-*, float);\n  row-gap: --modifier(number, text, wide);\n}\n");
    assert_eq!(
        list,
        [
            format!("Unsupported bare value data type: \"float\".\n{TYPES}"),
            "```css\n--value(--spacing-*,float)\n                    ^^^^^\n```".to_string(),
            format!("Unsupported bare value data type: \"text\".\n{TYPES}"),
            "```css\n--modifier(number,text,wide)\n                  ^^^^\n```".to_string(),
            format!("Unsupported bare value data type: \"wide\".\n{TYPES}"),
            "```css\n--modifier(number,text,wide)\n                       ^^^^\n```".to_string(),
        ]
    );
}

#[test]
fn valid_arguments_give_no_warning() {
    let css = "@utility example-* {\n  width: --value(--spacing, number, integer, ratio, percentage, [length], [*], 'literal', \"other\");\n  height: --modifier(--default(1), integer);\n  color: red;\n}\n@utility plain {\n  color: blue;\n}\n";
    assert_eq!(warnings(css), Vec::<String>::new());
    assert_eq!(warnings("@theme { --color-red: red; }\n@tailwind utilities;"), Vec::<String>::new());
}

#[test]
fn a_module_loader_gets_each_warning_at_its_time() {
    use std::rc::Rc;
    use tailwind::MemoryLoader;

    let css = "@utility tab-* {\n  tab-size: --value(size);\n}\n";
    let loader = Rc::new(MemoryLoader::default());
    let compiler = compile(css, CompileOptions { test_mode: true, module_loader: Some(loader.clone()), ..Default::default() }).unwrap();
    assert_eq!(loader.console.borrow().len(), 2);
    assert!(loader.console.borrow()[0].starts_with("Unsupported bare value data type: \"size\"."));
    assert!(compiler.take_warnings().is_empty());

    // As upstream gives the text to `console.warn` at once, an error later
    // in the stylesheet does not remove the warning.
    let css = format!("{css}@utility bad {{\n  @apply does-not-exist;\n}}\n@tailwind utilities;\n");
    let loader = Rc::new(MemoryLoader::default());
    let error = compile(&css, CompileOptions { test_mode: true, module_loader: Some(loader.clone()), ..Default::default() }).err().unwrap();
    assert!(error.message.starts_with("Cannot apply unknown utility class `does-not-exist`"));
    assert_eq!(loader.console.borrow().len(), 2, "{:?}", loader.console.borrow());
}
