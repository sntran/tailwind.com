//! Property tests of the full compiler: `compile` and `build`.

use proptest::prelude::*;
use tailwind::{compile, CompileOptions, Stylesheet};

const INPUT: &str = "@layer theme, base, components, utilities;\n@import \"tailwindcss/theme.css\" layer(theme);\n@import \"tailwindcss/utilities.css\" layer(utilities);";

fn load(id: &str, _base: &str) -> tailwind::Result<Stylesheet> {
    let name = id.trim_start_matches("tailwindcss/").trim_start_matches("./");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/upstream/css").join(name);
    let content = std::fs::read_to_string(&path).map_err(|e| tailwind::Error::new(e.to_string()))?;
    Ok(Stylesheet { path: format!("/tailwindcss/{name}"), base: "/tailwindcss".into(), content })
}

fn build(batches: &[Vec<String>]) -> String {
    let loader = load;
    let mut compiler = compile(
        INPUT,
        CompileOptions {
            base: String::new(),
            polyfills: tailwind::POLYFILLS_ALL,
            load_stylesheet: Some(&loader),
            test_mode: true,
            module_loader: None,
        },
    )
    .unwrap();
    let mut css = compiler.build(&[]);
    for batch in batches {
        css = compiler.build(batch);
    }
    css
}

/// A candidate from the parts of real class names. Many are not valid:
/// the compiler must ignore them.
fn candidate() -> impl Strategy<Value = String> {
    let variant = prop::sample::select(vec![
        "hover", "focus", "md", "max-lg", "dark", "group-hover", "peer-checked/name", "not-first", "has-[a]",
        "[&>*]", "[@media(min-width:300px)]", "min-[300px]", "supports-[display:grid]", "data-[x=y]",
        "aria-checked", "nth-3", "*", "**", "@md", "@max-[10rem]", "before", "in-[.a]", "bogus", "",
    ]);
    let root = prop::sample::select(vec![
        "flex", "hidden", "p-4", "-mt-2.5", "w-1/2", "w-[13px]", "bg-red-500", "bg-red-500/50", "bg-[#0088cc]/[35%]",
        "text-sm/6", "text-[length:var(--x)]", "[color:red]", "p-(--my-var)", "grid-cols-[repeat(3,minmax(0,1fr))]",
        "shadow-lg", "shadow-red-500/20", "ring-2", "translate-x-full", "rotate-45", "bg-linear-to-r/oklch",
        "from-blue-500", "via-30%", "font-bold", "underline", "container", "sr-only", "z-99", "aspect-4/3",
        "mask-b-to-50%", "inset-shadow-sm", "duration-150", "w-", "bg-red-500/", "-flex", "w-__", "🙂", "a b",
    ]);
    (prop::collection::vec(variant, 0..3), root, any::<bool>()).prop_map(|(variants, root, important)| {
        let mut parts: Vec<String> = variants.into_iter().map(String::from).collect();
        parts.push(if important { format!("{root}!") } else { root.to_string() });
        parts.join(":")
    })
}

fn candidates() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(candidate(), 0..12)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, failure_persistence: None, ..ProptestConfig::default() })]

    /// The output does not depend on the sequence of the candidates.
    #[test]
    fn the_sequence_of_the_candidates_does_not_change_the_output(list in candidates(), seed in any::<u64>()) {
        let mut shuffled = list.clone();
        // A small shuffle from the seed.
        let mut state = seed | 1;
        for i in (1..shuffled.len()).rev() {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            shuffled.swap(i, (state % (i as u64 + 1)) as usize);
        }
        prop_assert_eq!(build(&[list]), build(&[shuffled]));
    }

    /// A candidate two times is the same as one time.
    #[test]
    fn a_repeated_candidate_does_not_change_the_output(list in candidates()) {
        let mut twice = list.clone();
        twice.extend(list.iter().cloned());
        prop_assert_eq!(build(&[list]), build(&[twice]));
    }

    /// Builds are cumulative: two builds give the output of one build with
    /// all the candidates.
    #[test]
    fn two_builds_are_one_build_of_all_the_candidates(first in candidates(), second in candidates()) {
        let mut all = first.clone();
        all.extend(second.iter().cloned());
        prop_assert_eq!(build(&[first, second]), build(&[all]));
    }

    /// The same build two times gives the same text.
    #[test]
    fn a_build_is_deterministic(list in candidates()) {
        let first = build(std::slice::from_ref(&list));
        prop_assert_eq!(first, build(&[list]));
    }

    /// The output is CSS that the parser reads, and the printer gives the
    /// same text again.
    #[test]
    fn the_output_is_a_fixed_point_of_parse_and_print(list in candidates()) {
        let css = build(&[list]);
        let ast = tailwind::css_parser::parse(&css).unwrap();
        prop_assert_eq!(tailwind::to_css::to_css(&ast), css);
    }

    /// No text makes the compiler fail, as a candidate.
    #[test]
    fn any_text_is_a_safe_candidate(list in prop::collection::vec("\\PC{0,24}", 0..8)) {
        build(&[list]);
    }

    /// No text makes `compile` panic, as a stylesheet: the result is a
    /// compiler or an error.
    #[test]
    fn any_text_is_a_safe_stylesheet(css in "[@a-z{}();:,.&*>\"' \\n\\-\\[\\]/!0-9]{0,80}", extra in prop::sample::select(vec![
        "", "@tailwind utilities;", "@theme { --color-x: red; }", "@utility a-* { color: --value(--color-*); }",
        "@custom-variant b (&:hover);", "@variant b { .c { color: red } }", ".d { @apply flex; }", "@source inline(\"p-{1,2}\");",
    ])) {
        let input = format!("{extra}\n{css}");
        if let Ok(mut compiler) = compile(&input, CompileOptions { base: String::new(), polyfills: tailwind::POLYFILLS_ALL, load_stylesheet: None, test_mode: true, module_loader: None }) {
            compiler.build(&["flex".to_string(), "b:a-x".to_string()]);
        }
    }
}
