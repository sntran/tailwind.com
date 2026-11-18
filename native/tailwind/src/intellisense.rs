//! The class list and the variants for the editor tools: `intellisense.ts`
//! of Tailwind CSS. The canonical form of candidates uses them.

use crate::design_system::DesignSystem;
use crate::hash::IndexMap;
use crate::utilities::UtilityKind;
use crate::utils::compare::compare;

/// An item of the class list: `ClassItem`.
struct ClassItem {
    name: String,
    utility: String,
    fraction: bool,
    modifiers: Vec<String>,
}

/// `IS_FRACTION`: `/^\d+\/\d+$/`.
fn is_fraction(value: &str) -> bool {
    match value.split_once('/') {
        Some((a, b)) => {
            !a.is_empty() && !b.is_empty() && a.bytes().all(|c| c.is_ascii_digit()) && b.bytes().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

impl DesignSystem {
    /// `getClassList`: each class with its modifiers.
    pub fn get_class_list(&self) -> Vec<(String, Vec<String>)> {
        let mut items: IndexMap<String, ClassItem> = IndexMap::default();
        fn item<'a>(items: &'a mut IndexMap<String, ClassItem>, name: &str) -> &'a mut ClassItem {
            items.entry(name.to_string()).or_insert_with(|| ClassItem {
                name: name.to_string(),
                utility: name.to_string(),
                fraction: false,
                modifiers: Vec::new(),
            })
        }

        // A static utility works only as it is.
        for utility in self.utilities.keys(UtilityKind::Static) {
            if self.utilities.get_completions(utility, self).is_empty() {
                continue;
            }
            let item = item(&mut items, utility);
            item.fraction = false;
            item.modifiers = Vec::new();
        }

        // A functional utility has its own list of completions.
        for utility in self.utilities.keys(UtilityKind::Functional) {
            for group in self.utilities.get_completions(utility, self) {
                for value in &group.values {
                    let fraction = value.as_deref().is_some_and(is_fraction);
                    let name = match value {
                        None => utility.to_string(),
                        Some(value) => format!("{utility}-{value}"),
                    };

                    {
                        let item = item(&mut items, &name);
                        item.utility = utility.to_string();
                        item.fraction |= fraction;
                        item.modifiers.extend(group.modifiers.iter().cloned());
                    }

                    if group.supports_negative {
                        let item = item(&mut items, &format!("-{name}"));
                        item.utility = format!("-{utility}");
                        item.fraction |= fraction;
                        item.modifiers.extend(group.modifiers.iter().cloned());
                    }

                    // No duplicate modifiers. (Upstream does this only for
                    // the class with no `-`: a negative class can keep
                    // duplicates.)
                    let item = item(&mut items, &name);
                    let mut seen = crate::hash::HashSet::default();
                    item.modifiers.retain(|m| seen.insert(m.clone()));
                }
            }
        }

        if items.is_empty() {
            return Vec::new();
        }

        // The sort by the name of the class.
        let mut list: Vec<ClassItem> = items.into_values().collect();
        list.sort_by(|a, b| compare(&a.name, &b.name));

        sort_fractions_last(list)
    }
}

/// `sortFractionsLast`: the fractions of a utility come after its other
/// classes.
fn sort_fractions_last(list: Vec<ClassItem>) -> Vec<(String, Vec<String>)> {
    struct Bucket {
        utility: String,
        items: Vec<ClassItem>,
    }

    // 1. A bucket for each run of a utility. 2. The last bucket of each
    // utility. 3. The fractions of each utility.
    let mut buckets: Vec<Bucket> = Vec::new();
    let mut last_bucket: crate::hash::HashMap<String, usize> = Default::default();
    let mut fractions: IndexMap<String, Vec<ClassItem>> = IndexMap::default();

    for item in list {
        let utility = item.utility.clone();
        if buckets.last().is_none_or(|b| b.utility != utility) {
            buckets.push(Bucket { utility: utility.clone(), items: Vec::new() });
            last_bucket.insert(utility.clone(), buckets.len() - 1);
        }
        if item.fraction {
            fractions.entry(utility).or_default().push(item);
        } else {
            buckets.last_mut().unwrap().items.push(item);
        }
    }

    // 4. The fractions go to the last bucket of their utility.
    for (utility, items) in fractions {
        if let Some(&index) = last_bucket.get(&utility) {
            buckets[index].items.extend(items);
        }
    }

    // 5. One list.
    buckets.into_iter().flat_map(|b| b.items).map(|item| (item.name, item.modifiers)).collect()
}
