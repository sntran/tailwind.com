//! The order of classes: `sort.ts` of Tailwind CSS (`getClassOrder` of the
//! design system). The official Prettier plugin sorts the classes of a
//! template with it.

use crate::compile::compile_candidates;
use crate::design_system::DesignSystem;
use crate::hash::HashMap;

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
