//! Editor internationalization powered by Mozilla Fluent (two locales: `uk` and `en`).
//!
//! The editor's own texts (`locales/*/ui.ftl`) are embedded into WASM at compile time (`include_str!`).
//! Texts of nodes, data types, categories and plugin settings come from the server: the core's
//! resources and those shipped by enabled plugins arrive in `Init` / `Catalog` (`set_runtime_translations`)
//! and are added to the bundles after `ui.ftl`. Lookup order: active locale, then `en`, then the text
//! from the definition itself (the server's English fallback). The active locale is persisted in
//! `localStorage` and provided via Leptos context.

use std::cell::{Cell, RefCell};

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use leptos::prelude::*;
use ticknode_shared::*;
use unic_langid::LanguageIdentifier;

mod definitions;
mod values;
mod quantities;
pub use quantities::*;

pub use definitions::*;
pub use values::*;

const LOCALE_KEY: &str = "ticknode.locale";

pub const UK_UI_FTL: &str = include_str!("../locales/uk/ui.ftl");
pub const EN_UI_FTL: &str = include_str!("../locales/en/ui.ftl");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    Uk,
    En,
}

impl Locale {
    fn labels(self) -> (&'static str, &'static str) {
        match self {
            Self::Uk => ("uk", "UK"),
            Self::En => ("en", "EN"),
        }
    }

    pub fn code(self) -> &'static str { self.labels().0 }

    pub fn badge(self) -> &'static str { self.labels().1 }

    pub fn toggle(self) -> Self {
        match self {
            Self::Uk => Self::En,
            Self::En => Self::Uk,
        }
    }

    pub fn from_tag(tag: &str) -> Option<Self> {
        let lower = tag.trim().to_ascii_lowercase();
        if lower.starts_with("uk") {
            Some(Self::Uk)
        } else if lower.starts_with("en") {
            Some(Self::En)
        } else {
            None
        }
    }
}

struct Bundles {
    uk: FluentBundle<FluentResource>,
    en: FluentBundle<FluentResource>,
}

/// Builds a bundle: the editor's own `ui.ftl` first (strict), then server-provided resources
/// (lenient: broken entries are skipped and ids already defined keep their first definition).
fn build_bundle(code: &str, ui: &str, runtime: &[(String, String)]) -> FluentBundle<FluentResource> {
    let langid: LanguageIdentifier = code.parse().expect("a valid language identifier");
    let mut bundle = FluentBundle::new(vec![langid]);
    // Disable Unicode bidi isolation characters (U+2068/U+2069) around interpolated values
    bundle.set_use_isolating(false);
    let res = FluentResource::try_new(ui.to_string()).expect("a valid FTL resource");
    bundle.add_resource(res).expect("FTL resources without duplicate ids");
    for (_, source) in runtime.iter().filter(|(l, _)| l == code) {
        // A resource with syntax errors keeps its valid entries (the server rejects broken ones anyway)
        let res = FluentResource::try_new(source.clone()).unwrap_or_else(|(res, _)| res);
        // Conflicting ids are skipped by Fluent (the earlier definition stays), the rest is added
        let _ = bundle.add_resource(res);
    }
    bundle
}

fn build_bundles(runtime: &[(String, String)]) -> Bundles {
    Bundles { uk: build_bundle("uk", UK_UI_FTL, runtime), en: build_bundle("en", EN_UI_FTL, runtime) }
}

thread_local! {
    static BUNDLES: RefCell<Bundles> = RefCell::new(build_bundles(&[]));
    /// Types known to the editor: needed to translate Make/Break node pins (their labels are field titles).
    static TYPES: RefCell<TypeRegistry> = RefCell::new(TypeRegistry::new());
    /// Bumped when server translations change, so reactive texts are re-evaluated.
    static TRANSLATIONS_VERSION: Cell<Option<RwSignal<u32>>> = const { Cell::new(None) };
    /// Active locale signal for calls outside the component tree (e.g., WebSocket callbacks).
    static LOCALE_SIGNAL: Cell<Option<RwSignal<Locale>>> = const { Cell::new(None) };
    /// Fallback locale for tests and environments without a reactive owner.
    static FALLBACK_LOCALE: Cell<Locale> = const { Cell::new(Locale::Uk) };
}

/// Replaces the resources received from the server (core + enabled plugins) and notifies reactive texts.
pub fn set_runtime_translations(translations: &[(String, String)]) {
    BUNDLES.with(|b| *b.borrow_mut() = build_bundles(translations));
    TRANSLATIONS_VERSION.with(|c| {
        if let Some(signal) = c.get() {
            signal.try_update(|v| *v += 1);
        }
    });
}

/// Remembers the data types declared by plugins (for Make/Break pin labels).
pub fn set_known_types(types: &TypeRegistry) {
    TYPES.with(|t| *t.borrow_mut() = types.clone());
}

/// Detects the initial locale: saved in `localStorage`, otherwise browser language (`uk*` → `uk`, else `en`).
pub fn detect_locale() -> Locale {
    if let Some(saved) = window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(LOCALE_KEY).ok().flatten())
        && let Some(locale) = Locale::from_tag(&saved)
    {
        return locale;
    }
    match window().navigator().language() {
        Some(lang) if lang.trim().to_ascii_lowercase().starts_with("uk") => Locale::Uk,
        _ => Locale::En,
    }
}

/// Persists the locale choice in `localStorage` and updates `<html lang="...">`. Storage errors are ignored.
pub fn persist_locale(locale: Locale) {
    FALLBACK_LOCALE.with(|c| c.set(locale));
    if let Some(root) = document().document_element() {
        let _ = root.set_attribute("lang", locale.code());
    }
    if let Some(storage) = window().local_storage().ok().flatten() {
        let _ = storage.set_item(LOCALE_KEY, locale.code());
    }
}

/// Provides the locale signal in Leptos context and registers it for non-reactive callbacks.
pub fn provide_locale(initial: Locale) -> RwSignal<Locale> {
    let signal = RwSignal::new(initial);
    FALLBACK_LOCALE.with(|c| c.set(initial));
    LOCALE_SIGNAL.with(|c| c.set(Some(signal)));
    provide_context(signal);
    TRANSLATIONS_VERSION.with(|c| c.set(Some(RwSignal::new(0))));
    signal
}

pub fn use_locale() -> RwSignal<Locale> {
    expect_context::<RwSignal<Locale>>()
}

/// Current locale (with reactive subscription when called inside a reactive context).
pub fn current_locale() -> Locale {
    if let Some(sig) = use_context::<RwSignal<Locale>>().or_else(|| LOCALE_SIGNAL.with(Cell::get))
        && let Some(locale) = sig.try_get()
    {
        return locale;
    }
    FALLBACK_LOCALE.with(Cell::get)
}

/// Runs a closure with an explicitly set locale (for unit tests).
#[cfg(test)]
pub fn with_locale<R>(locale: Locale, f: impl FnOnce() -> R) -> R {
    let prev_sig = LOCALE_SIGNAL.with(|c| c.replace(None));
    let prev_fb = FALLBACK_LOCALE.with(|c| c.replace(locale));
    let res = f();
    FALLBACK_LOCALE.with(|c| c.set(prev_fb));
    LOCALE_SIGNAL.with(|c| c.set(prev_sig));
    res
}

/// Looks up a message by key: in the current locale, then in `en`; `None` if neither has it.
pub fn try_translate(key: &str, args: Option<&FluentArgs<'_>>) -> Option<String> {
    let locale = current_locale();
    // Subscribe reactive texts to changes of server translations
    TRANSLATIONS_VERSION.with(|c| {
        if let Some(signal) = c.get() {
            let _ = signal.try_get();
        }
    });
    BUNDLES.with(|bundles| {
        let bundles = bundles.borrow();
        let format = |bundle: &FluentBundle<FluentResource>| {
            let pattern = bundle.get_message(key)?.value()?;
            let mut errors = Vec::new();
            Some(bundle.format_pattern(pattern, args, &mut errors).into_owned())
        };
        let (primary, secondary) = match locale {
            Locale::Uk => (&bundles.uk, &bundles.en),
            Locale::En => (&bundles.en, &bundles.en),
        };
        format(primary).or_else(|| format(secondary))
    })
}

/// Translates a string by key; returns the key itself if not found.
pub fn translate(key: &str, args: Option<&FluentArgs<'_>>) -> String {
    try_translate(key, args).unwrap_or_else(|| key.to_string())
}

/// Localization macro: `t!("key")` or `t!("key", count = n, name = s)`.
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::translate($key, None)
    };
    ($key:expr, $($arg_name:ident = $arg_val:expr),+ $(,)?) => {{
        let mut args = ::fluent_bundle::FluentArgs::new();
        $(
            args.set(stringify!($arg_name), ::fluent_bundle::FluentValue::from($arg_val));
        )+
        $crate::i18n::translate($key, Some(&args))
    }};
}

pub(crate) use t;

// --- Texts of nodes, categories and data types: from server translations, falling back to the definition ---

/// Looks up a message that has no arguments.
fn lookup(key: &str) -> Option<String> {
    try_translate(key, None)
}

/// Fluent-safe fragment of a name (`math/add` -> `math-add`), same as the server uses for keys.
fn slug(name: &str) -> String {
    fluent_slug(name)
}

#[cfg(test)]
mod tests;
