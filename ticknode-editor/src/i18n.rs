//! Editor internationalization powered by Mozilla Fluent (two locales: `uk` and `en`).
//! Translation files are embedded into WASM at compile time (`include_str!`), while the active
//! locale is persisted in `localStorage` and provided via Leptos context.

use std::cell::Cell;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use leptos::prelude::*;
use ticknode_shared::condition::{CompareOp, Join, MatchCase, Operand};
use ticknode_shared::*;
use unic_langid::LanguageIdentifier;

const LOCALE_KEY: &str = "ticknode.locale";

pub const UK_UI_FTL: &str = include_str!("../locales/uk/ui.ftl");
pub const UK_NODES_FTL: &str = include_str!("../locales/uk/nodes.ftl");
pub const EN_UI_FTL: &str = include_str!("../locales/en/ui.ftl");
pub const EN_NODES_FTL: &str = include_str!("../locales/en/nodes.ftl");

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

fn build_bundle(code: &str, sources: &[&str]) -> FluentBundle<FluentResource> {
    let langid: LanguageIdentifier = code.parse().expect("коректний ідентифікатор мови");
    let mut bundle = FluentBundle::new(vec![langid]);
    // Disable Unicode bidi isolation characters (U+2068/U+2069) around interpolated values
    bundle.set_use_isolating(false);
    for src in sources {
        let res = FluentResource::try_new((*src).to_string()).expect("коректний FTL-ресурс");
        bundle.add_resource(res).expect("FTL-ресурси без дублікатів ключів");
    }
    bundle
}

thread_local! {
    static BUNDLES: Bundles = Bundles {
        uk: build_bundle("uk", &[UK_UI_FTL, UK_NODES_FTL]),
        en: build_bundle("en", &[EN_UI_FTL, EN_NODES_FTL]),
    };
    /// Active locale signal for calls outside the component tree (e.g., WebSocket callbacks).
    static LOCALE_SIGNAL: Cell<Option<RwSignal<Locale>>> = const { Cell::new(None) };
    /// Fallback locale for tests and environments without a reactive owner.
    static FALLBACK_LOCALE: Cell<Locale> = const { Cell::new(Locale::Uk) };
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

/// Looks up a message by key in the current locale; returns `None` if the key is missing.
pub fn try_translate(key: &str, args: Option<&FluentArgs<'_>>) -> Option<String> {
    let locale = current_locale();
    BUNDLES.with(|bundles| {
        let bundle = match locale {
            Locale::Uk => &bundles.uk,
            Locale::En => &bundles.en,
        };
        let msg = bundle.get_message(key)?;
        let pattern = msg.value()?;
        let mut errors = Vec::new();
        Some(bundle.format_pattern(pattern, args, &mut errors).into_owned())
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

// --- Built-in node translations by `type_name` ---

fn node_slug(type_name: &str) -> String {
    type_name.replace('/', "-")
}

pub fn node_title(def: &NodeDefinition) -> String {
    // On the canvas, variable nodes include the dynamic variable name in their title (`Get counter`, `Set x`)
    if (def.type_name == "variable/get" && def.title != "Get my_var")
        || (def.type_name == "variable/set" && def.title != "Set my_var")
    {
        return def.title.clone();
    }
    let key = format!("node-{}-title", node_slug(&def.type_name));
    try_translate(&key, None).unwrap_or_else(|| def.title.clone())
}

pub fn node_description(def: &NodeDefinition) -> String {
    let key = format!("node-{}-description", node_slug(&def.type_name));
    try_translate(&key, None).unwrap_or_else(|| def.description.clone())
}

pub fn node_help(def: &NodeDefinition) -> String {
    let key = format!("node-{}-help", node_slug(&def.type_name));
    try_translate(&key, None).unwrap_or_else(|| {
        if def.help.trim().is_empty() {
            node_description(def)
        } else {
            def.help.clone()
        }
    })
}

pub fn node_pin_label(type_name: &str, pin_name: &str) -> String {
    if !pin_name.is_empty() && pin_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        let key = format!("node-{}-pin-{pin_name}", node_slug(type_name));
        if let Some(translated) = try_translate(&key, None) {
            return translated;
        }
    }
    pin_name.to_string()
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
        DataType::Named(name) => types.get(name).map_or_else(|| name.clone(), |d| d.title.clone()),
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
        let ast = fluent_syntax::parser::parse(source).expect("FTL має розбиратися без помилок синтаксису");
        let mut keys = BTreeSet::new();
        for entry in ast.body {
            if let fluent_syntax::ast::Entry::Message(msg) = entry {
                assert!(
                    keys.insert(msg.id.name.to_string()),
                    "дубльований ключ у FTL: {}",
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
            "різні ключі в ui.ftl: лише в uk={only_uk_ui:?}, лише в en={only_en_ui:?}"
        );

        let uk_nodes = extract_ftl_keys(UK_NODES_FTL);
        let en_nodes = extract_ftl_keys(EN_NODES_FTL);
        let only_uk_nodes: Vec<_> = uk_nodes.difference(&en_nodes).collect();
        let only_en_nodes: Vec<_> = en_nodes.difference(&uk_nodes).collect();
        assert!(
            only_uk_nodes.is_empty() && only_en_nodes.is_empty(),
            "різні ключі в nodes.ftl: лише в uk={only_uk_nodes:?}, лише в en={only_en_nodes:?}"
        );

        assert!(
            uk_ui.is_disjoint(&uk_nodes),
            "ключі ui.ftl та nodes.ftl не повинні перетинатися"
        );

        // Verify bundle initialization for both locales
        BUNDLES.with(|b| {
            for key in uk_ui.iter().chain(uk_nodes.iter()) {
                assert!(b.uk.has_message(key), "немає ключа {key} в uk bundle");
                assert!(b.en.has_message(key), "немає ключа {key} в en bundle");
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
        for entry in fs::read_dir(&src_dir).expect("каталог src існує") {
            let path = entry.expect("запис каталогу").path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let content = fs::read_to_string(&path).expect("читання .rs файлу");
            for key in scan_t_calls(&content) {
                BUNDLES.with(|b| {
                    assert!(
                        b.uk.has_message(&key),
                        "ключ '{key}' з файлу {} відсутній у locales/uk/*.ftl",
                        path.display()
                    );
                    assert!(
                        b.en.has_message(&key),
                        "ключ '{key}' з файлу {} відсутній у locales/en/*.ftl",
                        path.display()
                    );
                });
                checked += 1;
            }
        }
        assert!(checked > 0, "сканер має знайти виклики макроса перекладу у вихідному коді");
    }

    #[test]
    fn built_in_nodes_have_translations_and_plugin_nodes_fallback() {
        let builtins = [
            "clock", "console", "const", "math/add", "math/sub", "math/mul", "math/div",
            "math/expression", "logical/and", "logical/or", "logical/not", "variable/get",
            "variable/set", "match", "compare", "for_loop", "sleep", "list/create",
            "list/add", "list/remove", "list/get", "list/length", "object/create",
            "object/set_field", "object/get_field", "object/unpack", "test/click_button",
            "test/debug",
        ];
        for ty in builtins {
            let slug = node_slug(ty);
            for suffix in ["title", "description", "help"] {
                let key = format!("node-{slug}-{suffix}");
                BUNDLES.with(|b| {
                    assert!(b.uk.has_message(&key), "немає {key} в uk");
                    assert!(b.en.has_message(&key), "немає {key} в en");
                });
            }
        }

        // Plugin node (not present in .ftl) falls back to server-provided strings as-is
        let plugin_def = NodeDefinition {
            type_name: "weather/forecast".into(),
            title: "Weather Forecast".into(),
            category: "Weather".into(),
            description: "Отримує прогноз погоди".into(),
            help: "Детальна довідка плагіна".into(),
            pins: Vec::new(),
        };
        with_locale(Locale::En, || {
            assert_eq!(node_title(&plugin_def), "Weather Forecast");
            assert_eq!(node_description(&plugin_def), "Отримує прогноз погоди");
            assert_eq!(node_help(&plugin_def), "Детальна довідка плагіна");
            assert_eq!(node_pin_label("weather/forecast", "City"), "City");
        });
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
