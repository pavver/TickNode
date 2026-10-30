//! Категорії нод у палітрі: порядок (як у Scratch — події й керування першими), українська
//! назва, колір та іконка. Категорія — рядок із `NodeDefinition::category`; невідомі (з плагінів)
//! показуються як є, з кольором із запасної палітри.

use crate::ui::icons;

pub struct Category {
    pub name: &'static str,
    pub label: &'static str,
    pub color: &'static str,
    pub icon: &'static str,
}

const fn cat(name: &'static str, label: &'static str, color: &'static str, icon: &'static str) -> Category {
    Category { name, label, color, icon }
}

const CATEGORIES: [Category; 13] = [
    cat("Events", "Події", "#E6A700", icons::BOLT),
    cat("Control Flow", "Керування", "#F08A24", icons::CALL_SPLIT),
    cat("Time", "Час", "#5C7CFA", icons::SCHEDULE),
    cat("Actions", "Дії", "#4C97FF", icons::SEND),
    cat("Data", "Дані", "#9966FF", icons::STORAGE),
    cat("Math", "Математика", "#4CAF50", icons::CALCULATE),
    cat("Logic", "Логіка", "#26A69A", icons::ACCOUNT_TREE),
    cat("String", "Рядки", "#C25FC2", icons::TEXT_FIELDS),
    cat("Lists", "Списки", "#D9534F", icons::LIST),
    cat("Objects", "Об'єкти", "#E8642C", icons::DATA_OBJECT),
    cat("Network", "Мережа", "#3FA7D6", icons::CLOUD),
    cat("Debug", "Налагодження", "#A0785A", icons::BUG),
    cat("Test", "Тест", "#8A96A6", icons::SCIENCE),
];
/// Для категорій плагінів, яких немає в списку.
const EXTRA_COLORS: [&str; 6] = ["#20A4A4", "#B5651D", "#D6336C", "#7950F2", "#2F9E44", "#1C7ED6"];

fn known(category: &str) -> Option<(usize, &'static Category)> {
    CATEGORIES.iter().enumerate().find(|(_, c)| c.name == category)
}

/// Порядок категорії в палітрі: відомі — за списком, решта — після них за абеткою.
pub fn category_rank(category: &str) -> usize {
    known(category).map_or(CATEGORIES.len(), |(i, _)| i)
}

pub fn category_label(category: &str) -> String {
    known(category).map_or_else(|| category.to_string(), |(_, c)| c.label.to_string())
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
mod tests {
    use super::*;

    #[test]
    fn categories_have_stable_order_colors_and_icons() {
        assert!(category_rank("Events") < category_rank("Math"));
        assert_eq!(category_rank("Плагін"), category_rank("Ще плагін"));
        assert_eq!(category_label("Math"), "Математика");
        assert_eq!(category_label("Weather"), "Weather");
        assert_eq!(category_color("Weather"), category_color("Weather"));
        assert_eq!(category_icon("Weather"), icons::EXTENSION);
    }
}
