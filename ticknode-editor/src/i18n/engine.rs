//! Engine formatting language for property labels, independent of the editor locale.

use std::cell::Cell;
use ticknode_shared::ServerSettings;

use super::{Locale, t};

thread_local! {
    static ENGINE_LOCALE: Cell<Locale> = const { Cell::new(Locale::En) };
}

/// Update formatting labels and display rules from Init and live settings broadcasts.
pub fn set_server_settings(settings: &ServerSettings) {
    ENGINE_LOCALE.with(|locale| locale.set(Locale::from_tag(&settings.locale).unwrap_or(Locale::En)));
    // This also notifies reactive labels through the shared translations version.
    super::set_display_rules(&settings.display);
}

pub(super) fn server_locale_label() -> String {
    let language = ENGINE_LOCALE.with(|locale| match locale.get() {
        Locale::En => t!("language-en"),
        Locale::Uk => t!("language-uk"),
    });
    t!("property-locale-server", language = language)
}

#[cfg(test)]
mod tests;
