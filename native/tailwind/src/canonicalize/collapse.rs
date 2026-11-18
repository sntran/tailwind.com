//! One utility for more utilities: `collapseCandidates` of
//! `canonicalize-candidates.ts`. `mt-1 mb-1` is `my-1`.

use super::signatures::{static_utilities_with, utility_properties, utility_signature, utility_theme_variables, PropertyValues};
use super::utilities::parse_candidate;
use super::InternalOptions;
use crate::candidate::Candidate;
use crate::design_system::DesignSystem;
use crate::hash::{HashMap, IndexMap, IndexSet};
use crate::utilities::UtilityKind;
use crate::utils::infer_data_type::is_valid_spacing_multiplier;
use crate::utils::locale_compare::locale_compare;
use crate::utils::segment::segment;
use std::rc::Rc;

/// The candidates of one group: the same variants and the same important
/// flag. Only these candidates can collapse.
pub fn collapse_candidates(ds: &DesignSystem, options: InternalOptions, candidates: Vec<String>) -> Vec<String> {
    let prefix = match ds.theme.prefix.as_deref().filter(|p| !p.is_empty()) {
        Some(prefix) => format!("{prefix}:"),
        None => String::new(),
    };

    // The groups by the variants, then by the important flag.
    let mut groups: IndexMap<String, IndexMap<String, IndexSet<String>>> = IndexMap::default();
    for candidate in &candidates {
        let mut variants = segment(candidate, b':');
        let mut utility = variants.pop().unwrap_or("").to_string();
        let important = utility.ends_with('!');
        if important {
            utility.pop();
        }
        let before = if variants.is_empty() { String::new() } else { format!("{}:", variants.join(":")) };
        let after = if important { "!" } else { "" };
        groups.entry(before).or_default().entry(after.to_string()).or_default().insert(format!("{prefix}{utility}"));
    }

    let mut result: IndexSet<String> = IndexSet::default();
    for (before, group) in groups {
        for (after, candidates) in group {
            for mut candidate in collapse_group(ds, options, candidates.into_iter().collect()) {
                // The prefix is in the variants: no prefix here.
                if !prefix.is_empty() && candidate.starts_with(&prefix) {
                    candidate = candidate[prefix.len()..].to_string();
                }
                result.insert(format!("{before}{candidate}{after}"));
            }
        }
    }
    result.into_iter().collect()
}

/// `Set.prototype.intersection`: the items of the smaller set, in the
/// sequence of the smaller set (the first set when the sizes are equal).
fn intersection(a: &IndexSet<String>, b: &IndexSet<String>) -> IndexSet<String> {
    if a.len() <= b.len() {
        a.iter().filter(|item| b.contains(*item)).cloned().collect()
    } else {
        b.iter().filter(|item| a.contains(*item)).cloned().collect()
    }
}

/// `combinations`: each combination of 2 or more items, the largest
/// first, in the sequence of Gosper's hack.
fn combinations(items: &[usize]) -> Vec<Vec<usize>> {
    let n = items.len();
    let mut result = Vec::new();
    if n > 63 {
        // Upstream uses big integers. So many linked candidates take too
        // long there too.
        return result;
    }
    let limit: u128 = 1u128 << n;
    for k in (2..=n).rev() {
        let mut mask: u128 = (1u128 << k) - 1;
        while mask < limit {
            result.push((0..n).filter(|i| (mask >> i) & 1 == 1).map(|i| items[i]).collect());
            let carry = mask & mask.wrapping_neg();
            let ripple = mask + carry;
            mask = (((ripple ^ mask) >> 2) / carry) | ripple;
        }
    }
    result
}

/// `collapseGroup`.
fn collapse_group(ds: &DesignSystem, options: InternalOptions, candidates: Vec<String>) -> Vec<String> {
    let signature_options = options.signature_options;
    let properties = |candidate: &str| -> Rc<PropertyValues> { utility_properties(ds, signature_options, candidate) };

    // The properties and the values of each candidate: `mt-1` is
    // `margin-top: 0.25rem`.
    let candidate_properties: Vec<Rc<PropertyValues>> = candidates.iter().map(|c| properties(c)).collect();

    // When a candidate sets `line-height` and another sets `font-size`,
    // the `text-*` utilities with this line height are pre-computed.
    if candidate_properties.iter().any(|p| p.contains_key("line-height")) {
        let font_size_names = ds.theme.keys_in_namespaces(&["--text"]);
        if !font_size_names.is_empty() {
            let mut interesting: IndexSet<String> = IndexSet::default();
            let mut seen_line_heights: crate::hash::HashSet<String> = Default::default();
            for pairs in &candidate_properties {
                let Some(line_heights) = pairs.get("line-height") else { continue };
                for line_height in line_heights {
                    if !seen_line_heights.insert(line_height.clone()) {
                        continue;
                    }
                    if let Some(bare) = super::utilities::spacing_multiplier(ds, line_height) {
                        let bare_text = crate::utils::js::js_number_to_string(bare);
                        if bare >= 0.0 && bare % 0.25 == 0.0 {
                            interesting.insert(bare_text.clone());
                            for name in &font_size_names {
                                properties(&format!("text-{name}/{bare_text}"));
                            }
                        } else {
                            interesting.insert(line_height.clone());
                            for name in &font_size_names {
                                properties(&format!("text-{name}/[{line_height}]"));
                            }
                        }
                    }
                }
            }

            let mut seen_font_sizes: crate::hash::HashSet<String> = Default::default();
            for pairs in &candidate_properties {
                let Some(font_sizes) = pairs.get("font-size") else { continue };
                for font_size in font_sizes {
                    if !seen_font_sizes.insert(font_size.clone()) {
                        continue;
                    }
                    for line_height in &interesting {
                        if is_valid_spacing_multiplier(line_height) {
                            properties(&format!("text-[{font_size}]/{line_height}"));
                        } else {
                            properties(&format!("text-[{font_size}]/[{line_height}]"));
                        }
                    }
                }
            }
        }
    }

    // The functional utilities with the value of a candidate, for each
    // property of the candidate (`dynamicUtilities`).
    let mut dynamic: HashMap<String, Rc<HashMap<String, IndexMap<String, IndexSet<String>>>>> = HashMap::default();
    let mut dynamic_utilities = |candidate: &str| -> Rc<HashMap<String, IndexMap<String, IndexSet<String>>>> {
        if let Some(found) = dynamic.get(candidate) {
            return found.clone();
        }
        let mut result: HashMap<String, IndexMap<String, IndexSet<String>>> = HashMap::default();
        let relevant: IndexSet<String> = properties(candidate).keys().cloned().collect();
        if !relevant.is_empty() {
            for parsed in parse_candidate(ds, candidate) {
                let Candidate::Functional(functional) = &parsed else { continue };
                if functional.value.is_none() {
                    continue;
                }
                for root in ds.utilities.keys(UtilityKind::Functional) {
                    if root == functional.root {
                        continue;
                    }
                    let mut other = parsed.clone();
                    if let Candidate::Functional(o) = &mut other {
                        o.root = root.to_string();
                    }
                    let replacement = unprefixed(ds, &ds.print_candidate(&other));
                    for (property, values) in properties(&replacement).iter() {
                        if !relevant.contains(property) {
                            continue;
                        }
                        for value in values {
                            result.entry(property.clone()).or_default().entry(value.clone()).or_default().insert(replacement.clone());
                        }
                    }
                }
                break;
            }
        }
        let result = Rc::new(result);
        dynamic.insert(candidate.to_string(), result.clone());
        result
    };

    // For each property, the other utilities that set this property with
    // this value; for more properties, the intersection.
    // `margin-top` is `mt-1`, `my-1`, `m-1`.
    let mut other_utilities: Vec<IndexSet<String>> = Vec::with_capacity(candidates.len());
    for (idx, property_values) in candidate_properties.iter().enumerate() {
        let mut result: Option<IndexSet<String>> = None;
        for (property, values) in property_values.iter() {
            let mut others: IndexSet<String> = static_utilities_with(ds, signature_options, property).into_iter().collect();
            for value in values {
                let dynamic = dynamic_utilities(&candidates[idx]);
                if let Some(found) = dynamic.get(property).and_then(|values| values.get(value)) {
                    others.extend(found.iter().cloned());
                }
            }
            result = Some(match result {
                None => others,
                Some(previous) => intersection(&previous, &others),
            });
            if result.as_ref().unwrap().is_empty() {
                break;
            }
        }
        other_utilities.push(result.unwrap_or_default());
    }

    // A replacement can have the signature of a combination only when the
    // candidates of the combination set each of its properties. A
    // replacement of only one candidate can link no candidates.
    let mut seen: crate::hash::HashSet<String> = Default::default();
    let mut shared: crate::hash::HashSet<String> = Default::default();
    for replacements in &other_utilities {
        for replacement in replacements {
            if !seen.insert(replacement.clone()) {
                shared.insert(replacement.clone());
            }
        }
    }
    let mut covered: HashMap<String, crate::hash::HashSet<String>> = HashMap::default();
    for (idx, replacements) in other_utilities.iter().enumerate() {
        for replacement in replacements {
            if !shared.contains(replacement) {
                continue;
            }
            let covered = covered.entry(replacement.clone()).or_default();
            for property in candidate_properties[idx].keys() {
                covered.insert(property.clone());
            }
        }
    }
    let mut possible: HashMap<String, bool> = HashMap::default();
    let other_utilities: Vec<IndexSet<String>> = other_utilities
        .into_iter()
        .map(|replacements| {
            replacements
                .into_iter()
                .filter(|replacement| {
                    if !shared.contains(replacement) {
                        return false;
                    }
                    if let Some(found) = possible.get(replacement) {
                        return *found;
                    }
                    let empty = Default::default();
                    let covered = covered.get(replacement).unwrap_or(&empty);
                    let result = properties(replacement).keys().all(|property| covered.contains(property));
                    possible.insert(replacement.clone(), result);
                    result
                })
                .collect()
        })
        .collect();

    // Link the candidates that another utility can link: `mt-1` and `mb-1`
    // through `my-1`.
    let mut linked: IndexMap<usize, IndexSet<usize>> = IndexMap::default();
    for i in 0..other_utilities.len() {
        for j in i + 1..other_utilities.len() {
            if other_utilities[i].iter().any(|p| other_utilities[j].contains(p)) {
                linked.entry(i).or_insert_with(|| IndexSet::from_iter([i])).insert(j);
                linked.entry(j).or_insert_with(|| IndexSet::from_iter([j])).insert(i);
            }
        }
    }
    if linked.is_empty() {
        return candidates;
    }

    // The unique combinations of linked candidates.
    let mut unique: IndexSet<Vec<usize>> = IndexSet::default();
    for group in linked.values() {
        let mut sorted: Vec<usize> = group.iter().copied().collect();
        sorted.sort_unstable();
        unique.insert(sorted);
    }

    // The position of each candidate in the CSS: the last one wins.
    // No candidate that throws: a utility of a plugin with the same name
    // can throw for this value.
    let mut orderable: Vec<String> = Vec::new();
    for idx in linked.keys() {
        ds.get_class_order(std::slice::from_ref(&candidates[*idx]));
        if ds.take_js_error().is_none() {
            orderable.push(candidates[*idx].clone());
        }
    }
    let class_order: HashMap<String, Option<u64>> = ds.get_class_order(&orderable).into_iter().collect();
    ds.take_js_error();
    let compare_order = |a: Option<u64>, z: Option<u64>| -> std::cmp::Ordering {
        match (a, z) {
            _ if a == z => std::cmp::Ordering::Equal,
            (None, _) => std::cmp::Ordering::Greater,
            (_, None) => std::cmp::Ordering::Less,
            (Some(a), Some(z)) => a.cmp(&z),
        }
    };

    // Now the collapse.
    let mut result: IndexSet<String> = candidates.iter().cloned().collect();
    let mut drop: IndexSet<String> = IndexSet::default();

    for idxs in &unique {
        for combo in combinations(idxs) {
            if combo.iter().any(|idx| drop.contains(&candidates[*idx])) {
                continue;
            }

            let mut potential = other_utilities[combo[0]].clone();
            for idx in &combo[1..] {
                potential = intersection(&potential, &other_utilities[*idx]);
            }
            // With nothing to compare, no signature of the combination.
            if potential.is_empty() {
                continue;
            }

            let mut names: Vec<&String> = combo.iter().map(|idx| &candidates[*idx]).collect();
            crate::utils::v8_sort::sort(&mut names, |a, z| locale_compare(a, z) as i64);
            let collapsed = names.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ");
            let collapsed_signature = utility_signature(ds, signature_options, &collapsed);

            // First the candidates of the combination, the one that wins
            // in the CSS first, then the other utilities.
            let mut members: Vec<&String> = combo.iter().map(|idx| &candidates[*idx]).collect();
            crate::utils::v8_sort::sort(&mut members, |a, z| {
                compare_order(class_order.get(*z).copied().flatten(), class_order.get(*a).copied().flatten()) as i64
            });
            let mut replacements: IndexSet<String> = members.into_iter().cloned().collect();
            replacements.extend(potential.into_iter());

            for replacement in replacements {
                let signature = utility_signature(ds, signature_options, &replacement);
                if signature != collapsed_signature {
                    continue;
                }
                // Equal signatures can come from other theme variables
                // with the same default value: `mt-card mb-card` and
                // `my-4` are both `1rem`.
                let a = utility_theme_variables(ds, signature_options, &collapsed);
                let b = utility_theme_variables(ds, signature_options, &replacement);
                if !uses_same_theme_variables(a.as_deref(), b.as_deref()) {
                    continue;
                }

                result.insert(replacement.clone());
                // The replacement replaces each item of the combination. A
                // replacement in the combination stays.
                for item in &combo {
                    if candidates[*item] != replacement {
                        drop.insert(candidates[*item].clone());
                    }
                }
                break;
            }
        }
    }

    for item in &drop {
        result.shift_remove(item);
    }
    result.into_iter().collect()
}

/// `usesSameThemeVariables`: a declaration with no theme variables can
/// take a replacement with theme variables.
fn uses_same_theme_variables(candidates: Option<&Vec<String>>, replacement: Option<&Vec<String>>) -> bool {
    let (Some(candidates), Some(replacement)) = (candidates, replacement) else { return false };
    candidates.len() == replacement.len() && candidates.iter().zip(replacement).all(|(variables, other)| variables.is_empty() || variables == other)
}

/// The text of a candidate with no prefix: `printUnprefixedCandidate`.
fn unprefixed(ds: &DesignSystem, text: &str) -> String {
    match ds.theme.prefix.as_deref().filter(|p| !p.is_empty()) {
        Some(prefix) if text.starts_with(&format!("{prefix}:")) => text[prefix.len() + 1..].to_string(),
        _ => text.to_string(),
    }
}
