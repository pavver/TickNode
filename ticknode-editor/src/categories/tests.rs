use super::*;
use crate::i18n::{Locale, with_locale};

#[test]
fn categories_have_stable_order_colors_and_icons() {
    assert!(category_rank("Events") < category_rank("Math"));
    assert_eq!(category_rank("Plugin"), category_rank("Another plugin"));
    with_locale(Locale::Uk, || {
        assert_eq!(category_label("Math"), "Математика");
        assert_eq!(category_label("Weather"), "Weather");
        assert_eq!(category_label("Other Plugin"), "Other Plugin");
    });
    with_locale(Locale::En, || {
        assert_eq!(category_label("Math"), "Math");
        assert_eq!(category_label("String"), "Strings");
        assert_eq!(category_label("Weather"), "Weather");
    });
    // A plugin ships a translation of its own category
    crate::i18n::set_runtime_translations(&[("uk".to_string(), "category-weather = Погода".to_string())]);
    with_locale(Locale::Uk, || assert_eq!(category_label("Weather"), "Погода"));
    with_locale(Locale::En, || assert_eq!(category_label("Weather"), "Weather"));
    assert_eq!(category_color("Weather"), category_color("Weather"));
    assert_eq!(category_icon("Weather"), icons::EXTENSION);
}
