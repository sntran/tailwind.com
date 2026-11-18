//! The canonical form of candidates as each version of Tailwind CSS. The
//! algorithm is the one of the newest version, and the utilities and the
//! variants are the ones of the version of the design system
//! (`DesignSystem::version`). For 400 lists of classes
//! (`tests/upstream/cli/canonicalize-random/stdin.txt`) and each version:
//!
//! - each utility of the result is a utility of that version, or a
//!   utility of the input, and a list of valid classes gives valid
//!   classes (the variants too);
//! - the result has the same declarations as the input: for a list whose
//!   classes have the same variants, the utilities of the result and the
//!   utilities of the input have the same signature.
//!
//! The command line cases `canonicalize-4.3.2` and `canonicalize-4.3.4`
//! (`tests/upstream/cli`) compare the result with the official program of
//! those versions.
#![cfg(feature = "canonicalize")]

use tailwind::canonicalize::{signature, CanonicalizeOptions};
use tailwind::index::load_design_system;
use tailwind::{CompileOptions, Stylesheet, Version};

fn load(id: &str, _base: &str) -> tailwind::Result<Stylesheet> {
    let name = if id == "tailwindcss" { "index.css" } else { id.trim_start_matches("tailwindcss/").trim_start_matches("./") };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/upstream/css").join(name);
    let content = std::fs::read_to_string(&path).map_err(|e| tailwind::Error::new(e.to_string()))?;
    Ok(Stylesheet { path: format!("/tailwindcss/{name}"), base: "/tailwindcss".into(), content })
}

fn lists() -> Vec<Vec<String>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/upstream/cli/canonicalize-random/stdin.txt");
    std::fs::read_to_string(path).unwrap().lines().map(|line| line.split(' ').map(String::from).collect()).collect()
}

/// The variants and the utility of a class: the text before and after
/// the last `:` that is not in brackets.
fn split(class: &str) -> (&str, String) {
    let mut depth = 0;
    let mut at = 0;
    for (index, c) in class.char_indices() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ':' if depth == 0 => at = index + 1,
            _ => {}
        }
    }
    (&class[..at], class[at..].to_string())
}

#[test]
fn the_canonical_form_keeps_the_css_of_each_version() {
    let options = CanonicalizeOptions { rem: None, collapse: true, logical_to_physical: true };
    let lists = lists();
    assert_eq!(lists.len(), 400);
    let mut changed_by_version = Vec::new();
    for version in [Version(4, 0, 0), Version(4, 1, 0), Version(4, 2, 2), Version(4, 3, 0), Version::NATIVE] {
        let loader = load;
        let design_system = load_design_system(
            "@import \"tailwindcss\";",
            CompileOptions { base: "/".into(), load_stylesheet: Some(&loader), version: Some(version), ..CompileOptions::default() },
        )
        .unwrap();
        assert_eq!(design_system.version, version);

        let valid = |class: &String| design_system.get_class_order(std::slice::from_ref(class))[0].1.is_some();
        let mut failures = Vec::new();
        let (mut changed, mut compared) = (0, 0);
        for list in &lists {
            let result = design_system.canonicalize_candidates(list, &options);
            if &result != list {
                changed += 1;
            }
            let (variants_before, before): (Vec<&str>, Vec<String>) = list.iter().map(|class| split(class)).unzip();
            let (variants_after, after): (Vec<&str>, Vec<String>) = result.iter().map(|class| split(class)).unzip();
            for utility in &after {
                if !valid(utility) && !before.contains(utility) {
                    failures.push(format!("{list:?}\n  gives {utility:?}, which is not a utility of {version}"));
                }
            }
            // With its variants too: a list of valid classes gives valid
            // classes (the new form of a variant is a variant of the
            // version).
            if list.iter().all(&valid) {
                for class in &result {
                    if !valid(class) {
                        failures.push(format!("{list:?}\n  gives {class:?}, which is not a class of {version}"));
                    }
                }
            }
            // Only the lists with one group of variants: `@apply` gives
            // a rule for each class with variants. A class that is not a
            // class of the version stays as it is, and is not a part of
            // the comparison.
            let group = |variants: &[&str], utilities: &[String]| -> Option<Option<String>> {
                let mut group: Option<String> = None;
                for (variants, utility) in variants.iter().zip(utilities) {
                    if !valid(utility) {
                        continue;
                    }
                    match &group {
                        None => group = Some(variants.to_string()),
                        Some(group) if group == variants => {}
                        Some(_) => return None,
                    }
                }
                Some(group)
            };
            if group(&variants_before, &before).is_none() {
                continue;
            }
            if group(&variants_after, &after).is_none() {
                failures.push(format!("{list:?}\n  gives {result:?}, with more than one group of variants"));
                continue;
            }
            let before: Vec<String> = before.into_iter().filter(|utility| valid(utility)).collect();
            let after: Vec<String> = after.into_iter().filter(|utility| valid(utility)).collect();
            compared += 1;
            if signature(&design_system, &before, &options) != signature(&design_system, &after, &options) {
                failures.push(format!("{list:?}\n  gives {result:?}, with other CSS in {version}"));
            }
        }
        assert!(compared > 300);
        assert!(failures.is_empty(), "{} failures as {version}:\n{}", failures.len(), failures[..failures.len().min(10)].join("\n"));
        changed_by_version.push((version.to_string(), changed));
    }
    // The canonical form does work in each version.
    assert!(changed_by_version.iter().all(|(_, changed)| *changed > 150), "{changed_by_version:?}");
}

#[test]
fn a_replacement_must_be_a_utility_of_the_version() {
    let options = CanonicalizeOptions { rem: None, collapse: true, logical_to_physical: true };
    let canonical = |version: Version, classes: &[&str]| {
        let loader = load;
        let design_system = load_design_system(
            "@import \"tailwindcss\";",
            CompileOptions { base: "/".into(), load_stylesheet: Some(&loader), version: Some(version), ..CompileOptions::default() },
        )
        .unwrap();
        let classes: Vec<String> = classes.iter().map(|class| class.to_string()).collect();
        design_system.canonicalize_candidates(&classes, &options).join(" ")
    };
    // `inset-s-*` is new in 4.2.0, and `wrap-break-word` in 4.1.0.
    assert_eq!(canonical(Version(4, 1, 18), &["start-4"]), "start-4");
    assert_eq!(canonical(Version(4, 2, 0), &["start-4"]), "inset-s-4");
    assert_eq!(canonical(Version(4, 0, 17), &["break-words"]), "break-words");
    assert_eq!(canonical(Version(4, 1, 0), &["break-words"]), "wrap-break-word");
    // The same work in each version.
    for version in [Version(4, 0, 0), Version(4, 2, 2), Version::NATIVE] {
        assert_eq!(canonical(version, &["px-4", "py-4", "w-6", "h-6"]), "p-4 size-6");
        assert_eq!(canonical(version, &["[&>*]:flex", "bg-gradient-to-r"]), "*:flex bg-linear-to-r");
    }
}
