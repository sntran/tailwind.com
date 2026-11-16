//! The optimizer with CSS functions and mixins (`docs/MIXINS.md`): the
//! lowering before Lightning CSS, with and without a source map.

#![cfg(feature = "cli")]

use tailwind::cli::optimize_css::optimize;

const INPUT: &str = "@function --gap(--n) {
  --step: 4px;
  @media (width >= 40rem) {
    --step: 8px;
  }
  result: calc(var(--n) * var(--step));
}
@mixin --card(--bg: white, --pad) {
  background: var(--bg);
  padding: var(--pad, 1rem);
  gap: --gap(2);
  &:hover {
    background: black;
  }
  & > p {
    color: var(--bg);
  }
  @contents;
}
.panel {
  @apply --card(red, 2rem) {
    margin: 0;
  }
}
";

#[test]
fn the_optimizer_lowers_functions_and_mixins() {
    let optimized = optimize(INPUT, "input.css", true, None).unwrap();
    assert_eq!(
        optimized.code,
        ".panel{--mixin-1-bg:red;background:red;gap:8px;padding:2rem}@media (min-width:40rem){.panel{gap:16px}}.panel:hover{background:#000}.panel>p{color:var(--mixin-1-bg)}.panel{margin:0}"
    );
    assert_eq!(optimized.warnings, None);
}

#[test]
fn the_source_map_goes_through_the_lowering() {
    // A map of the input to itself: one mapping for each line.
    let lines = INPUT.lines().count();
    let mappings = std::iter::once("AAAA").chain(std::iter::repeat_n("AACA", lines - 1)).collect::<Vec<_>>().join(";");
    let mut map = String::from("{\"version\":3,\"sources\":[\"input.css\"],\"sourcesContent\":[");
    map.push_str(&serde_json::to_string(INPUT).unwrap());
    map.push_str("],\"names\":[],\"mappings\":\"");
    map.push_str(&mappings);
    map.push_str("\"}");
    let optimized = optimize(INPUT, "input.css", false, Some(&map)).unwrap();
    assert!(optimized.code.starts_with(".panel {\n  --mixin-1-bg: red;"), "{}", optimized.code);
    let map = optimized.map.unwrap();
    assert!(map.contains("input.css"), "{map}");
}

#[test]
fn a_stylesheet_with_no_function_and_no_mixin_does_not_change() {
    // `@apply --name` with no `@mixin`: the step does not run, and the
    // rule stays for the browser. The split of the rule is from Lightning
    // CSS, as in the official program.
    let input = ".a {\n  @apply --other;\n  color: red;\n}\n";
    let optimized = optimize(input, "input.css", true, None).unwrap();
    assert_eq!(optimized.code, ".a{@apply --other;}.a{color:red}");
}

#[test]
fn the_compiler_keeps_contents_with_no_block() {
    // A fix of this program: upstream removes `@contents;`, and then the
    // mixin `--wrap`, which has no other node.
    let css = "@mixin --m {\n  color: red;\n  @contents;\n}\n@mixin --wrap {\n  @contents;\n}\n.a {\n  @apply --m {\n    top: 0;\n  }\n}\n";
    let mut compiler = tailwind::compile(css, tailwind::CompileOptions::default()).unwrap();
    let output = compiler.build(&[]);
    // `toCss` of upstream prints an at-rule with no params and no body
    // with a space: `@contents ;`.
    assert!(
        output.ends_with("@mixin --m {\n  color: red;\n  @contents ;\n}\n@mixin --wrap {\n  @contents ;\n}\n.a {\n  @apply --m {\n    top: 0;\n  }\n}\n"),
        "{output}"
    );
    let optimized = optimize(&output, "input.css", true, None).unwrap();
    assert!(optimized.code.ends_with("*/\n.a{color:red;top:0}"), "{}", optimized.code);
}

#[test]
fn the_compiler_keeps_the_conditions_and_the_optimizer_lowers_them() {
    let css = "@function --ink(--mode: light) {\n  result: if(style(--mode: dark): white; else: black);\n}\n.a {\n  color: --ink(dark);\n  padding: if(media(width >= 40rem): 2rem; else: 1rem);\n}\n@when media(hover: hover) {\n  .b {\n    color: red;\n  }\n} @else {\n  .b {\n    color: blue;\n  }\n}\n";
    let mut compiler = tailwind::compile(css, tailwind::CompileOptions::default()).unwrap();
    let output = compiler.build(&[]);
    assert!(output.contains("padding: if(media(width >= 40rem): 2rem; else: 1rem);"), "{output}");
    assert!(output.contains("@when media(hover: hover)") && output.contains("@else {"), "{output}");
    let optimized = optimize(&output, "input.css", true, None).unwrap();
    assert_eq!(
        optimized.code,
        ".a{color:#fff;padding:1rem}@media (min-width:40rem){.a{padding:2rem}}@media (hover:hover){.b{color:red}}@media not all and (hover:hover){.b{color:#00f}}"
    );
    assert_eq!(optimized.warnings, None);
}
