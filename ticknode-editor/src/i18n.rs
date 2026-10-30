//! Editor internationalization powered by Mozilla Fluent (two locales: `uk` and `en`).
//!
//! The editor's own texts (`locales/*/ui.ftl`) are embedded into WASM at compile time (`include_str!`).
//! Texts of nodes, data types, categories and plugin settings come from the server: the core's
//! resources and those shipped by enabled plugins arrive in `Init` / `Catalog` (`set_runtime_translations`)
//! and are added to the bundles after `ui.ftl`. Lookup order: active locale, then `en`, then the text
//! from the definition itself (the server's English fallback). The active locale is persisted in
//! `localStorage` and provided via Leptos context.

use std::cell::{Cell, RefCell};

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use leptos::prelude::*;
use ticknode_shared::condition::{CompareOp, Join, MatchCase, Operand};
use ticknode_shared::*;
use unic_langid::LanguageIdentifier;

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
    pub fn code(self) -> &'static str {
        match self {
            Self::Uk => "uk",
            Self::En => "en",
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            Self::Uk => "UK",
            Self::En => "EN",
        }
    }

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

fn node_slug(type_name: &str) -> String {
    slug(type_name)
}

const MAKE_PREFIX: &str = "type/make/";
const BREAK_PREFIX: &str = "type/break/";

/// For the automatic Make/Break nodes of a struct: (`"make"` | `"break"`, the type's name).
fn custom_node_kind(type_name: &str) -> Option<(&'static str, &str)> {
    type_name.strip_prefix(MAKE_PREFIX).map(|t| ("make", t))
        .or_else(|| type_name.strip_prefix(BREAK_PREFIX).map(|t| ("break", t)))
}

/// Translated title of a data type; `None` if the server shipped no translation for it.
fn translated_type_title(type_name: &str) -> Option<String> {
    lookup(&format!("type-{}-title", slug(type_name)))
}

pub fn node_title(def: &NodeDefinition) -> String {
    // On the canvas, variable nodes include the dynamic variable name in their title (`Get counter`, `Set x`)
    if (def.type_name == "variable/get" && def.title != "Get my_var")
        || (def.type_name == "variable/set" && def.title != "Set my_var")
    {
        return def.title.clone();
    }
    if let Some((kind, type_name)) = custom_node_kind(&def.type_name)
        && let Some(title) = translated_type_title(type_name)
    {
        return t!(if kind == "make" { "custom-make-title" } else { "custom-break-title" }, type = title);
    }
    lookup(&format!("node-{}-title", node_slug(&def.type_name))).unwrap_or_else(|| def.title.clone())
}

pub fn node_description(def: &NodeDefinition) -> String {
    if let Some((kind, type_name)) = custom_node_kind(&def.type_name)
        && let Some(title) = translated_type_title(type_name)
    {
        return t!(if kind == "make" { "custom-make-description" } else { "custom-break-description" }, type = title);
    }
    lookup(&format!("node-{}-description", node_slug(&def.type_name))).unwrap_or_else(|| def.description.clone())
}

pub fn node_help(def: &NodeDefinition) -> String {
    if let Some((kind, type_name)) = custom_node_kind(&def.type_name)
        && let Some(title) = translated_type_title(type_name)
    {
        let key = if kind == "make" { "custom-make-help" } else { "custom-break-help" };
        let description = lookup(&format!("type-{}-description", slug(type_name)))
            .or_else(|| TYPES.with(|types| types.borrow().get(type_name).map(|d| d.description.clone())))
            .unwrap_or_default();
        let text = t!(key, type = title, name = type_name);
        return format!("{text}\n\n{description}").trim_end().to_string();
    }
    lookup(&format!("node-{}-help", node_slug(&def.type_name))).unwrap_or_else(|| {
        if def.help.trim().is_empty() {
            node_description(def)
        } else {
            def.help.clone()
        }
    })
}

/// Pin label. Convention: `node-<type slug>-pin-<name>` where the name is the pin's name as written when it
/// consists of `[A-Za-z0-9_-]`, otherwise its slug (`Delay ms` -> `delay-ms`). The Make/Break nodes of a
/// struct take their pin labels from the type's (translated) title and field titles.
pub fn node_pin_label(type_name: &str, pin_name: &str) -> String {
    if let Some((_, struct_name)) = custom_node_kind(type_name) {
        if let Some(label) = custom_pin_label(struct_name, pin_name) {
            return label;
        }
    }
    let base = format!("node-{}-pin-", node_slug(type_name));
    if !pin_name.is_empty() && pin_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && let Some(label) = lookup(&format!("{base}{pin_name}"))
    {
        return label;
    }
    let pin_slug = slug(pin_name);
    if !pin_slug.is_empty()
        && let Some(label) = lookup(&format!("{base}{pin_slug}"))
    {
        return label;
    }
    pin_name.to_string()
}

/// Make/Break pins are named after the type (pin 0) and the field titles (the rest).
fn custom_pin_label(struct_name: &str, pin_name: &str) -> Option<String> {
    TYPES.with(|types| {
        let types = types.borrow();
        let def = types.get(struct_name)?;
        if pin_name == def.title {
            return translated_type_title(struct_name);
        }
        let field = def.fields().iter().find(|f| if f.title.is_empty() { f.name == pin_name } else { f.title == pin_name })?;
        lookup(&format!("type-{}-field-{}", slug(struct_name), field.name))
    })
}

/// Translated category name for categories without a built-in label (`category-<slug>`).
pub fn plugin_category_label(category: &str) -> Option<String> {
    lookup(&format!("category-{}", slug(category)))
}

/// Title of a data type definition.
pub fn type_def_title(def: &TypeDefinition) -> String {
    translated_type_title(&def.name).unwrap_or_else(|| def.title.clone())
}

/// Title of an enum value in the drop-down (the value itself when it has no title).
pub fn enum_value_title(type_name: &str, value: &EnumValue) -> String {
    lookup(&format!("type-{}-value-{}", slug(type_name), slug(&value.value)))
        .unwrap_or_else(|| if value.title.is_empty() { value.value.clone() } else { value.title.clone() })
}

/// Title of a plugin setting (`setting-<plugin slug>-<key slug>-title`).
pub fn setting_title(plugin: &str, def: &SettingDefinition) -> String {
    lookup(&format!("setting-{}-{}-title", slug(plugin), slug(&def.key))).unwrap_or_else(|| def.title.clone())
}

pub fn setting_description(plugin: &str, def: &SettingDefinition) -> String {
    lookup(&format!("setting-{}-{}-description", slug(plugin), slug(&def.key))).unwrap_or_else(|| def.description.clone())
}

/// Label of a `Select` setting option.
pub fn setting_option_label(plugin: &str, def: &SettingDefinition, value: &str, label: &str) -> String {
    lookup(&format!("setting-{}-{}-option-{}", slug(plugin), slug(&def.key), slug(value))).unwrap_or_else(|| label.to_string())
}

/// Text of a message for the user: the translation of its key (searched in the editor's own and the
/// server-provided texts, arguments substituted), otherwise the English fallback.
pub fn ui_text(message: &UiMessage) -> String {
    if !message.key.is_empty() {
        let mut args = FluentArgs::new();
        // A setting error names the setting by its translated title (`setting-<plugin>-<key>-title`)
        let setting_title = message.args.iter().find(|(n, _)| n == "plugin").zip(message.args.iter().find(|(n, _)| n == "key"))
            .and_then(|((_, plugin), (_, key))| lookup(&format!("setting-{}-{}-title", slug(plugin), slug(key))));
        for (name, value) in &message.args {
            let value = match (&setting_title, name.as_str()) {
                (Some(title), "setting") => title.clone(),
                _ => value.clone(),
            };
            args.set(name.clone(), FluentValue::from(value));
        }
        if let Some(text) = try_translate(&message.key, Some(&args)) {
            return text;
        }
    }
    message.fallback.clone()
}

// --- Data types, field hints, and value parsing ---

pub fn type_name_label(name: &str) -> String {
    match name {
        "exec" => t!("type-exec"),
        "number" => t!("type-number"),
        "string" => t!("type-string"),
        "bool" => t!("type-bool"),
        "time" => t!("type-time"),
        "datetime" => t!("type-datetime"),
        "duration" => t!("type-duration"),
        "color" => t!("type-color"),
        "object" => t!("type-object"),
        "list" => t!("type-list"),
        "any" => t!("type-any"),
        "auto" => t!("type-auto"),
        other => other.to_string(),
    }
}

pub fn data_type_title(types: &TypeRegistry, t: &DataType) -> String {
    match t {
        DataType::Named(name) => types.get(name).map_or_else(|| name.clone(), type_def_title),
        DataType::List(inner) => t!("type-list-of", inner = data_type_title(types, inner)),
        other => type_name_label(&data_type_name(other)),
    }
}

pub fn type_hint(t: &DataType) -> String {
    match t {
        DataType::Time => t!("hint-time"),
        DataType::DateTime => t!("hint-datetime"),
        DataType::Duration => t!("hint-duration"),
        DataType::Color => t!("hint-color"),
        _ => t!("hint-default"),
    }
}

pub fn parse_value_localized(input: &str, t: &DataType) -> Result<DataValue, String> {
    let trimmed = input.trim();
    text::parse_value(input, t).map_err(|_| match t {
        DataType::Number => t!("parse-error-number", value = trimmed),
        DataType::Bool => t!("parse-error-bool", value = trimmed),
        DataType::Time => t!("parse-error-time", value = trimmed),
        DataType::DateTime => t!("parse-error-datetime", value = trimmed),
        DataType::Duration => t!("parse-error-duration", value = trimmed),
        DataType::Color => t!("parse-error-color", value = trimmed),
        _ => t!("parse-error-not-text", type = data_type_name(t)),
    })
}

// --- Match condition operators and formatting ---

pub fn op_symbol(op: CompareOp) -> String {
    match op {
        CompareOp::Eq => t!("match-op-eq"),
        CompareOp::Ne => t!("match-op-ne"),
        CompareOp::Lt => t!("match-op-lt"),
        CompareOp::Le => t!("match-op-le"),
        CompareOp::Gt => t!("match-op-gt"),
        CompareOp::Ge => t!("match-op-ge"),
        CompareOp::Contains => t!("match-op-contains"),
        CompareOp::NotContains => t!("match-op-not-contains"),
        CompareOp::StartsWith => t!("match-op-starts-with"),
        CompareOp::EndsWith => t!("match-op-ends-with"),
        CompareOp::IsEmpty => t!("match-op-is-empty"),
        CompareOp::NotEmpty => t!("match-op-not-empty"),
        CompareOp::IsTrue => t!("match-op-is-true"),
        CompareOp::IsFalse => t!("match-op-is-false"),
        CompareOp::HasField => t!("match-op-has-field"),
        CompareOp::NoField => t!("match-op-no-field"),
        CompareOp::LenEq => t!("match-op-len-eq"),
        CompareOp::LenGt => t!("match-op-len-gt"),
        CompareOp::LenLt => t!("match-op-len-lt"),
        CompareOp::HasValue => t!("match-op-has-value"),
        CompareOp::NoValue => t!("match-op-no-value"),
    }
}

pub fn op_label_for(op: CompareOp, t: &DataType) -> String {
    let time = matches!(t, DataType::Time | DataType::DateTime);
    match op {
        CompareOp::Lt if time => t!("match-op-lt-time"),
        CompareOp::Le if time => t!("match-op-le-time"),
        CompareOp::Gt if time => t!("match-op-gt-time"),
        CompareOp::Ge if time => t!("match-op-ge-time"),
        CompareOp::Lt if *t == DataType::Duration => t!("match-op-lt-duration"),
        CompareOp::Gt if *t == DataType::Duration => t!("match-op-gt-duration"),
        other => op_symbol(other),
    }
}

pub fn join_symbol(join: Join) -> String {
    match join {
        Join::And => t!("match-join-and"),
        Join::Or => t!("match-join-or"),
    }
}

pub fn format_match_case(case: &MatchCase) -> String {
    if case.conditions.is_empty() {
        return t!("match-always");
    }
    case.conditions
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let cond = match &c.right {
                Some(r) => format!("{} {} {}", c.left.to_text(), op_symbol(c.op), Operand::to_text(r)),
                None => format!("{} {}", c.left.to_text(), op_symbol(c.op)),
            };
            if i == 0 {
                cond
            } else {
                format!("{} {}", join_symbol(c.join), cond)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    fn extract_ftl_keys(source: &str) -> BTreeSet<String> {
        let ast = fluent_syntax::parser::parse(source).expect("FTL must parse without syntax errors");
        let mut keys = BTreeSet::new();
        for entry in ast.body {
            if let fluent_syntax::ast::Entry::Message(msg) = entry {
                assert!(
                    keys.insert(msg.id.name.to_string()),
                    "duplicate key in FTL: {}",
                    msg.id.name
                );
            }
        }
        keys
    }

    #[test]
    fn ftl_files_parse_and_have_identical_keys() {
        let uk_ui = extract_ftl_keys(UK_UI_FTL);
        let en_ui = extract_ftl_keys(EN_UI_FTL);
        let only_uk_ui: Vec<_> = uk_ui.difference(&en_ui).collect();
        let only_en_ui: Vec<_> = en_ui.difference(&uk_ui).collect();
        assert!(
            only_uk_ui.is_empty() && only_en_ui.is_empty(),
            "ui.ftl keys differ: only in uk={only_uk_ui:?}, only in en={only_en_ui:?}"
        );

        // Verify bundle initialization for both locales
        BUNDLES.with(|b| {
            let b = b.borrow();
            for key in &uk_ui {
                assert!(b.uk.has_message(key), "no key {key} in the uk bundle");
                assert!(b.en.has_message(key), "no key {key} in the en bundle");
            }
        });
    }

    /// Finds all translation macro calls in a source file (ignoring comment lines).
    fn scan_t_calls(src: &str) -> Vec<String> {
        let cleaned: String = src
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut keys = Vec::new();
        let bytes = cleaned.as_bytes();
        let mut i = 0;
        while i + 4 < bytes.len() {
            if bytes[i] == b't' && bytes[i + 1] == b'!' && bytes[i + 2] == b'(' {
                let prev_ok =
                    i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
                if prev_ok {
                    let mut j = i + 3;
                    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if j < bytes.len() && bytes[j] == b'"' {
                        let start = j + 1;
                        let mut end = start;
                        while end < bytes.len() && bytes[end] != b'"' {
                            if bytes[end] == b'\\' {
                                end += 2;
                            } else {
                                end += 1;
                            }
                        }
                        if end < bytes.len()
                            && let Ok(key) = std::str::from_utf8(&bytes[start..end])
                            && !key.is_empty()
                        {
                            keys.push(key.to_string());
                        }
                        i = end + 1;
                        continue;
                    }
                }
            }
            i += 1;
        }
        keys
    }

    #[test]
    fn every_t_key_used_in_code_exists_in_bundles() {
        let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut checked = 0usize;
        for entry in fs::read_dir(&src_dir).expect("the src directory exists") {
            let path = entry.expect("a directory entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let content = fs::read_to_string(&path).expect("reading a .rs file");
            for key in scan_t_calls(&content) {
                BUNDLES.with(|b| {
                    let b = b.borrow();
                    assert!(b.uk.has_message(&key), "key '{key}' from {} is missing in locales/uk/ui.ftl", path.display());
                    assert!(b.en.has_message(&key), "key '{key}' from {} is missing in locales/en/ui.ftl", path.display());
                });
                checked += 1;
            }
        }
        assert!(checked > 0, "the scanner must find translation macro calls in the source");
    }

    fn def(type_name: &str) -> NodeDefinition {
        NodeDefinition {
            type_name: type_name.into(),
            title: "Weather Forecast".into(),
            category: "Weather".into(),
            description: "Fetches the forecast".into(),
            help: "Detailed plugin help".into(),
            pins: Vec::new(),
        }
    }

    const PLUGIN_UK: &str = "\
node-weather-forecast-title = Прогноз погоди
node-weather-forecast-description = Отримує прогноз
node-weather-forecast-pin-city = Місто
node-weather-forecast-pin-delay-ms = Затримка
type-weather-state-title = Стан
type-weather-state-field-temp = Температура
type-weather-mode-value-auto = Авто
setting-weather-plugin-url-title = Адреса
weather-plugin-error-no-token = Токен не задано ({ $name })
";

    #[test]
    fn without_translations_the_definition_texts_are_used() {
        let plugin_def = def("weather/forecast");
        with_locale(Locale::En, || {
            assert_eq!(node_title(&plugin_def), "Weather Forecast");
            assert_eq!(node_description(&plugin_def), "Fetches the forecast");
            assert_eq!(node_help(&plugin_def), "Detailed plugin help");
            assert_eq!(node_pin_label("weather/forecast", "City"), "City");
        });
    }

    #[test]
    fn runtime_resources_translate_nodes_types_and_errors() {
        set_runtime_translations(&[("uk".to_string(), PLUGIN_UK.to_string())]);
        let plugin_def = def("weather/forecast");
        with_locale(Locale::Uk, || {
            assert_eq!(node_title(&plugin_def), "Прогноз погоди");
            assert_eq!(node_description(&plugin_def), "Отримує прогноз");
            // No help key: the definition's text stays
            assert_eq!(node_help(&plugin_def), "Detailed plugin help");
            // The pin label is looked up by the name as written, then by its slug
            assert_eq!(node_pin_label("weather/forecast", "city"), "Місто");
            assert_eq!(node_pin_label("weather/forecast", "Delay ms"), "Затримка");
            assert_eq!(node_pin_label("weather/forecast", "Other"), "Other");
            let setting = SettingDefinition::text("url", "URL");
            assert_eq!(setting_title("weather_plugin", &setting), "Адреса");
            assert_eq!(setting_description("weather_plugin", &setting), "");
            let value = EnumValue { value: "auto".into(), title: "Auto".into() };
            assert_eq!(enum_value_title("weather.mode", &value), "Авто");
            assert_eq!(enum_value_title("weather.mode", &EnumValue { value: "manual".into(), title: "Manual".into() }), "Manual");
            let msg = UiMessage::new("weather-plugin-error-no-token", "Token is not set").arg("name", "x");
            assert_eq!(ui_text(&msg), "Токен не задано (x)");
            // An unknown key or no key: the English fallback
            assert_eq!(ui_text(&UiMessage::new("weather-plugin-nope", "Fallback")), "Fallback");
            assert_eq!(ui_text(&UiMessage::plain("Plain")), "Plain");
        });
        // The plugin has no `en` resource: the English locale uses the definition
        with_locale(Locale::En, || {
            assert_eq!(node_title(&plugin_def), "Weather Forecast");
            assert_eq!(ui_text(&UiMessage::new("weather-plugin-error-no-token", "Token is not set")), "Token is not set");
        });
    }

    #[test]
    fn lookup_falls_back_from_the_locale_to_en() {
        set_runtime_translations(&[
            ("en".to_string(), "node-weather-forecast-title = Forecast\nnode-weather-forecast-help = English help".to_string()),
            ("uk".to_string(), "node-weather-forecast-title = Прогноз".to_string()),
        ]);
        let plugin_def = def("weather/forecast");
        with_locale(Locale::Uk, || {
            assert_eq!(node_title(&plugin_def), "Прогноз");
            // Missing in uk: taken from en before the definition's text
            assert_eq!(node_help(&plugin_def), "English help");
            assert_eq!(node_description(&plugin_def), "Fetches the forecast");
        });
        with_locale(Locale::En, || assert_eq!(node_title(&plugin_def), "Forecast"));
    }

    #[test]
    fn the_editor_ui_texts_win_over_runtime_resources() {
        set_runtime_translations(&[("en".to_string(), "toolbar-plugins = HIJACKED\ncategory-extra = Extra".to_string())]);
        with_locale(Locale::En, || {
            assert_eq!(t!("toolbar-plugins"), "Plugins");
            assert_eq!(translate("category-extra", None), "Extra");
        });
    }

    #[test]
    fn broken_runtime_resources_do_not_break_the_bundles() {
        set_runtime_translations(&[("en".to_string(), "good = ok\nthis is = = broken {".to_string())]);
        with_locale(Locale::En, || {
            assert_eq!(translate("good", None), "ok");
            assert_eq!(t!("toolbar-plugins"), "Plugins");
        });
    }

    #[test]
    fn make_and_break_nodes_use_the_translated_type() {
        let mut registry = TypeRegistry::new();
        registry.insert_all(&[TypeDefinition::new_struct(
            "weather.state",
            "State",
            vec![FieldDefinition::new("temp", "Temp", DataType::Number)],
        )]).unwrap();
        set_known_types(&registry);
        set_runtime_translations(&[("uk".to_string(), PLUGIN_UK.to_string())]);
        let make = NodeDefinition { type_name: "type/make/weather.state".into(), title: "Make State".into(), ..def("x") };
        with_locale(Locale::Uk, || {
            assert_eq!(node_title(&make), "Створити Стан");
            assert_eq!(node_description(&make), "Збирає значення «Стан» з полів");
            assert_eq!(node_pin_label("type/make/weather.state", "State"), "Стан");
            assert_eq!(node_pin_label("type/make/weather.state", "Temp"), "Температура");
            assert_eq!(node_pin_label("type/break/weather.state", "Temp"), "Температура");
        });
        // No translated type: the server's text is kept
        with_locale(Locale::En, || assert_eq!(node_title(&make), "Make State"));
    }

    #[test]
    fn plurals_and_arguments_work_in_both_locales() {
        with_locale(Locale::Uk, || {
            assert_eq!(t!("time-seconds-ago", count = 1), "1 секунду тому");
            assert_eq!(t!("time-seconds-ago", count = 2), "2 секунди тому");
            assert_eq!(t!("time-seconds-ago", count = 5), "5 секунд тому");
            assert_eq!(t!("time-seconds-ago", count = 21), "21 секунду тому");
            assert_eq!(t!("group-selected", count = 1), "Виділено 1 ноду");
            assert_eq!(t!("group-selected", count = 3), "Виділено 3 ноди");
            assert_eq!(t!("group-selected", count = 12), "Виділено 12 нод");
        });
        with_locale(Locale::En, || {
            assert_eq!(t!("time-seconds-ago", count = 1), "1 second ago");
            assert_eq!(t!("time-seconds-ago", count = 2), "2 seconds ago");
            assert_eq!(t!("time-seconds-ago", count = 5), "5 seconds ago");
            assert_eq!(t!("group-selected", count = 1), "1 node selected");
            assert_eq!(t!("group-selected", count = 3), "3 nodes selected");
            assert_eq!(t!("status-online", count = 1), "connected · 1 node");
            assert_eq!(t!("status-online", count = 4), "connected · 4 nodes");
        });
    }
}
