//! Node categories in the palette: ordering (Scratch-style — events and control flow first),
//! localized label, color, and icon. A category is the string from `NodeDefinition::category`;
//! unknown ones (from plugins) are shown as-is with a color from the fallback palette.

use crate::i18n::t;
use crate::ui::icons;

pub struct Category {
    pub name: &'static str,
    key: &'static str,
    pub color: &'static str,
    pub icon: &'static str,
}

const fn cat(name: &'static str, key: &'static str, color: &'static str, icon: &'static str) -> Category {
    Category { name, key, color, icon }
}

const CATEGORIES: [Category; 13] = [
    cat("Events", "category-events", "#E6A700", icons::BOLT),
    cat("Control Flow", "category-control-flow", "#F08A24", icons::CALL_SPLIT),
    cat("Time", "category-time", "#5C7CFA", icons::SCHEDULE),
    cat("Actions", "category-actions", "#4C97FF", icons::SEND),
    cat("Data", "category-data", "#9966FF", icons::STORAGE),
    cat("Math", "category-math", "#4CAF50", icons::CALCULATE),
    cat("Logic", "category-logic", "#26A69A", icons::ACCOUNT_TREE),
    cat("String", "category-string", "#C25FC2", icons::TEXT_FIELDS),
    cat("Lists", "category-lists", "#D9534F", icons::LIST),
    cat("Objects", "category-objects", "#E8642C", icons::DATA_OBJECT),
    cat("Network", "category-network", "#3FA7D6", icons::CLOUD),
    cat("Debug", "category-debug", "#A0785A", icons::BUG),
    cat("Test", "category-test", "#8A96A6", icons::SCIENCE),
];
/// Fallback colors for plugin categories not listed above.
const EXTRA_COLORS: [&str; 6] = ["#20A4A4", "#B5651D", "#D6336C", "#7950F2", "#2F9E44", "#1C7ED6"];

fn known(category: &str) -> Option<(usize, &'static Category)> {
    CATEGORIES.iter().enumerate().find(|(_, c)| c.name == category)
}

/// Category sort rank in the palette: known ones in list order, others after them alphabetically.
pub fn category_rank(category: &str) -> usize {
    known(category).map_or(CATEGORIES.len(), |(i, _)| i)
}

pub fn category_label(category: &str) -> String {
    known(category).map_or_else(
        || crate::i18n::plugin_category_label(category).unwrap_or_else(|| category.to_string()),
        |(_, category)| t!(category.key),
    )
}

pub fn category_color(category: &str) -> &'static str {
    known(category).map_or_else(
        || EXTRA_COLORS[category.bytes().fold(0usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize)) % EXTRA_COLORS.len()],
        |(_, c)| c.color,
    )
}

pub fn category_icon(category: &str) -> &'static str {
    known(category).map_or(icons::EXTENSION, |(_, c)| c.icon)
}

#[cfg(test)]
mod tests;
