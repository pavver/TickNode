use super::*;
use crate::i18n::{property_option_label, with_locale};

#[test]
fn server_locale_option_names_the_engine_language_in_the_ui_language() {
    let settings = |locale: &str| ServerSettings { locale: locale.into(), ..Default::default() };
    set_server_settings(&settings("en"));
    with_locale(Locale::Uk, || {
        assert_eq!(property_option_label("text/to_text", "locale", "server", "Server"), "Сервер (Англійська)");
    });
    set_server_settings(&settings("uk"));
    with_locale(Locale::En, || {
        assert_eq!(property_option_label("text/to_text", "locale", "server", "Server"), "Server (Ukrainian)");
        // Other options keep their own labels.
        assert_ne!(property_option_label("text/to_text", "locale", "uk", "Ukrainian"), "Server (Ukrainian)");
    });
    set_server_settings(&settings("en"));
}
