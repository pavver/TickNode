use super::kinds::{ValueKind, kind_label};
use super::{CompletionItem, CompletionKind};
use crate::TypeRegistry;

pub const LOOP_ATTRS: &[&str] = &[
    "changed",
    "cycle",
    "depth",
    "depth0",
    "first",
    "index",
    "index0",
    "last",
    "length",
    "nextitem",
    "previtem",
    "revindex",
    "revindex0",
];

pub const TAG_KEYWORDS: &[&str] = &[
    "call",
    "elif",
    "else",
    "endcall",
    "endfilter",
    "endfor",
    "endif",
    "endmacro",
    "endraw",
    "endset",
    "endwith",
    "filter",
    "for",
    "if",
    "macro",
    "raw",
    "set",
    "with",
];

pub const TESTS: &[&str] = &[
    "boolean",
    "defined",
    "divisibleby",
    "endingwith",
    "eq",
    "escaped",
    "even",
    "false",
    "filter",
    "float",
    "ge",
    "gt",
    "in",
    "integer",
    "iterable",
    "le",
    "lower",
    "lt",
    "mapping",
    "ne",
    "none",
    "number",
    "odd",
    "safe",
    "sameas",
    "sequence",
    "startingwith",
    "string",
    "test",
    "true",
    "undefined",
    "upper",
];

pub const BUILTIN_FILTERS: &[(&str, &str)] = &[
    ("abs", "abs()"),
    ("ago", "ago()"),
    ("attr", "attr(name)"),
    ("batch", "batch(size, [fill_with])"),
    ("bool", "bool()"),
    ("capitalize", "capitalize()"),
    ("count", "count()"),
    ("d", "d(default)"),
    ("date", "date([pattern])"),
    ("default", "default(value)"),
    ("dictsort", "dictsort()"),
    ("duration", "duration([pattern])"),
    ("e", "e()"),
    ("escape", "escape()"),
    ("first", "first()"),
    ("float", "float()"),
    ("format", "format(...)"),
    ("groupby", "groupby(attribute)"),
    ("indent", "indent(width)"),
    ("int", "int()"),
    ("items", "items()"),
    ("join", "join(separator)"),
    ("last", "last()"),
    ("length", "length()"),
    ("lines", "lines()"),
    ("list", "list()"),
    ("lower", "lower()"),
    ("map", "map(...)"),
    ("max", "max()"),
    ("min", "min()"),
    ("plural", "plural(forms…)"),
    ("pprint", "pprint()"),
    ("reject", "reject(...)"),
    ("rejectattr", "rejectattr(...)"),
    ("replace", "replace(old, new)"),
    ("reverse", "reverse()"),
    ("round", "round([precision], [method])"),
    ("safe", "safe()"),
    ("select", "select(...)"),
    ("selectattr", "selectattr(...)"),
    ("slice", "slice(slices)"),
    ("sort", "sort()"),
    ("split", "split([separator])"),
    ("strftime", "strftime(format)"),
    ("string", "string()"),
    ("sum", "sum([attribute])"),
    ("title", "title()"),
    ("trim", "trim()"),
    ("unique", "unique()"),
    ("unit", "unit([symbol], digits=, decimals=, units=)"),
    ("upper", "upper()"),
];

pub const FUNCTIONS: &[(&str, &str)] = &[
    ("debug", "debug()"),
    ("dict", "dict(...)"),
    ("namespace", "namespace(...)"),
    ("now", "now() → DateTime"),
    ("range", "range([start], stop, [step])"),
];

pub const KEYWORD_OPERATORS: &[&str] = &[
    "and", "else", "false", "if", "in", "is", "none", "not", "or", "true",
];

const GENERIC_FILTER_NAMES: &[&str] = &["bool", "d", "default", "pprint", "string"];

pub fn filter_accepts(name: &str, input: &ValueKind) -> bool {
    if matches!(input, ValueKind::Any) {
        return true;
    }
    if name == "text" || GENERIC_FILTER_NAMES.contains(&name) {
        return true;
    }
    match name {
        "date" | "strftime" => matches!(input, ValueKind::DateTime | ValueKind::Time),
        "ago" => matches!(input, ValueKind::DateTime),
        "duration" => matches!(input, ValueKind::Duration | ValueKind::Number),
        "unit" => matches!(
            input,
            ValueKind::Number
                | ValueKind::Duration
                | ValueKind::DateTime
                | ValueKind::Time
                | ValueKind::Any
        ),
        "abs" | "round" | "plural" => matches!(input, ValueKind::Number),
        "int" => matches!(
            input,
            ValueKind::Number | ValueKind::String | ValueKind::Bool
        ),
        "float" => matches!(input, ValueKind::Number | ValueKind::String),
        "capitalize" | "lower" | "upper" | "title" | "trim" | "replace" | "indent" | "format"
        | "safe" | "escape" | "e" => matches!(input, ValueKind::String),
        "split" | "lines" => matches!(input, ValueKind::String),
        "first" | "last" | "reverse" => matches!(input, ValueKind::String | ValueKind::List(_)),
        "length" | "count" | "list" => {
            matches!(
                input,
                ValueKind::String | ValueKind::List(_) | ValueKind::Map(_)
            )
        }
        "join" | "batch" | "slice" | "sort" | "unique" | "select" | "reject" | "min" | "max"
        | "map" => matches!(input, ValueKind::List(_)),
        "sum" => match input {
            ValueKind::List(t) => matches!(**t, ValueKind::Number | ValueKind::Any),
            _ => false,
        },
        "groupby" | "selectattr" | "rejectattr" => match input {
            ValueKind::List(t) => matches!(**t, ValueKind::Map(_) | ValueKind::Any),
            _ => false,
        },
        "items" | "dictsort" | "attr" => matches!(input, ValueKind::Map(_)),
        _ => false,
    }
}

pub fn filter_output_kind(name: &str, input: &ValueKind) -> ValueKind {
    if matches!(input, ValueKind::Any) {
        return match name {
            "date" | "strftime" | "ago" | "duration" | "unit" | "text" | "string" | "pprint"
            | "safe" | "escape" | "e" | "plural" => ValueKind::String,
            "bool" => ValueKind::Bool,
            _ => ValueKind::Any,
        };
    }

    match name {
        "date" | "strftime" | "ago" | "duration" | "unit" | "text" | "string" | "pprint"
        | "plural" => ValueKind::String,
        "bool" => ValueKind::Bool,
        "default" | "d" => input.clone(),
        "capitalize" | "lower" | "upper" | "title" | "trim" | "replace" | "indent" | "format"
        | "safe" | "escape" | "e" => ValueKind::String,
        "length" | "count" | "int" | "float" | "abs" | "round" | "sum" => ValueKind::Number,
        "split" | "lines" => ValueKind::List(Box::new(ValueKind::String)),
        "join" => ValueKind::String,
        "first" | "last" => match input {
            ValueKind::List(t) => (**t).clone(),
            ValueKind::String => ValueKind::String,
            _ => ValueKind::Any,
        },
        "min" | "max" => match input {
            ValueKind::List(t) => (**t).clone(),
            _ => ValueKind::Any,
        },
        "reverse" => match input {
            ValueKind::String => ValueKind::String,
            ValueKind::List(t) => ValueKind::List(t.clone()),
            _ => ValueKind::Any,
        },
        "sort" | "unique" | "select" | "reject" | "selectattr" | "rejectattr" => match input {
            ValueKind::List(t) => ValueKind::List(t.clone()),
            _ => ValueKind::Any,
        },
        "slice" | "batch" => match input {
            ValueKind::List(t) => ValueKind::List(Box::new(ValueKind::List(t.clone()))),
            _ => ValueKind::Any,
        },
        "map" | "groupby" => ValueKind::List(Box::new(ValueKind::Any)),
        "list" => match input {
            ValueKind::String => ValueKind::List(Box::new(ValueKind::String)),
            ValueKind::List(t) => ValueKind::List(t.clone()),
            ValueKind::Map(_) => ValueKind::List(Box::new(ValueKind::String)),
            _ => ValueKind::Any,
        },
        "items" | "dictsort" => {
            ValueKind::List(Box::new(ValueKind::List(Box::new(ValueKind::Any))))
        }
        "attr" => ValueKind::Any,
        _ => ValueKind::Any,
    }
}

fn make_filter_item(
    name: &str,
    sig: &str,
    input: &ValueKind,
    types: &TypeRegistry,
) -> CompletionItem {
    let out_kind = filter_output_kind(name, input);
    let detail = if out_kind != ValueKind::Any {
        let label = kind_label(&out_kind, types);
        format!("{sig} → {label}")
    } else {
        sig.to_string()
    };
    let help = match name {
        "date" => Some(crate::UiMessage::new(
            "template-help-filter-date",
            "Formats a date-time or time: date('DD.MM.YYYY HH:mm') or a preset; type inside the quotes for format codes",
        )),
        "strftime" => Some(crate::UiMessage::new(
            "template-help-filter-strftime",
            "Formats a date-time or time using strftime format codes",
        )),
        "ago" => Some(crate::UiMessage::new(
            "template-help-filter-ago",
            "Formats relative time, e.g. '5 minutes ago' or 'in 2 hours'",
        )),
        "duration" => Some(crate::UiMessage::new(
            "template-help-filter-duration",
            "Formats a duration using compact units ('short'), words ('long'), or an 'H:mm' pattern",
        )),
        "unit" => Some(crate::UiMessage::new(
            "template-help-filter-unit",
            "Formats a number or physical quantity using project display rules or an explicit unit symbol",
        )),
        "text" => Some(crate::UiMessage::new(
            "template-help-filter-text",
            "Formats a value using its unit and locale rules",
        )),
        "plural" => Some(crate::UiMessage::new(
            "template-help-filter-plural",
            "Selects plural text based on cardinal rules: positional forms follow the formatting language's rules (e.g. one, few, many), keywords zero/one/two/few/many/other, and # is replaced by the formatted number",
        )),
        "default" => Some(crate::UiMessage::new(
            "template-help-filter-default",
            "Fallback value when the input is undefined or empty",
        )),
        _ => None,
    };
    CompletionItem {
        label: name.to_string(),
        kind: CompletionKind::Filter,
        detail,
        insert: name.to_string(),
        help,
    }
}

pub fn filter_items_for(input: &ValueKind, types: &TypeRegistry) -> Vec<CompletionItem> {
    let mut specific = Vec::new();
    let mut other_generic = Vec::new();

    for &(name, sig) in BUILTIN_FILTERS {
        if !filter_accepts(name, input) {
            continue;
        }
        let item = make_filter_item(name, sig, input, types);
        if GENERIC_FILTER_NAMES.contains(&name) {
            other_generic.push(item);
        } else {
            specific.push(item);
        }
    }

    let text_item = make_filter_item("text", "text", input, types);

    let mut result = Vec::with_capacity(1 + specific.len() + other_generic.len());
    if matches!(input, ValueKind::Number | ValueKind::Any) {
        result.push(text_item);
        result.extend(specific);
        result.extend(other_generic);
    } else {
        result.extend(specific);
        result.push(text_item);
        result.extend(other_generic);
    }
    result
}

pub fn test_accepts(name: &str, input: &ValueKind) -> bool {
    match name {
        "defined" | "undefined" | "none" | "in" | "sameas" => true,
        "true" | "false" | "boolean" => matches!(input, ValueKind::Bool | ValueKind::Any),
        "odd" | "even" | "divisibleby" => matches!(input, ValueKind::Number | ValueKind::Any),
        "number" | "integer" | "float" => matches!(input, ValueKind::Any),
        "string" => matches!(input, ValueKind::Any),
        "lower" | "upper" | "startingwith" | "endingwith" | "escaped" | "safe" | "filter"
        | "test" => matches!(input, ValueKind::String | ValueKind::Any),
        "sequence" | "iterable" => {
            matches!(
                input,
                ValueKind::List(_) | ValueKind::String | ValueKind::Any
            )
        }
        "mapping" => matches!(input, ValueKind::Map(_) | ValueKind::Any),
        "eq" | "ne" => {
            matches!(
                input,
                ValueKind::Number
                    | ValueKind::String
                    | ValueKind::Bool
                    | ValueKind::DateTime
                    | ValueKind::Time
                    | ValueKind::Duration
                    | ValueKind::Any
            )
        }
        "lt" | "le" | "gt" | "ge" => {
            matches!(
                input,
                ValueKind::Number
                    | ValueKind::String
                    | ValueKind::DateTime
                    | ValueKind::Time
                    | ValueKind::Any
            )
        }
        _ => false,
    }
}

pub fn test_items_for(input: &ValueKind) -> Vec<CompletionItem> {
    TESTS
        .iter()
        .filter(|&&name| test_accepts(name, input))
        .map(|&name| CompletionItem {
            label: name.to_string(),
            kind: CompletionKind::Test,
            detail: "test".to_string(),
            insert: name.to_string(),
            help: None,
        })
        .collect()
}

pub fn loop_attribute_items() -> Vec<CompletionItem> {
    LOOP_ATTRS
        .iter()
        .map(|&attr| CompletionItem {
            label: attr.to_string(),
            kind: CompletionKind::Attribute,
            detail: "loop attribute".to_string(),
            insert: attr.to_string(),
            help: None,
        })
        .collect()
}

pub fn tag_keyword_items() -> Vec<CompletionItem> {
    TAG_KEYWORDS
        .iter()
        .map(|&kw| CompletionItem {
            label: kw.to_string(),
            kind: CompletionKind::Keyword,
            detail: "keyword".to_string(),
            insert: kw.to_string(),
            help: None,
        })
        .collect()
}

pub fn function_items() -> Vec<CompletionItem> {
    FUNCTIONS
        .iter()
        .map(|&(name, sig)| {
            let help = match name {
                "now" => Some(crate::UiMessage::new(
                    "template-help-function-now",
                    "Current date and time as an ISO 8601 / RFC 3339 string",
                )),
                _ => None,
            };
            CompletionItem {
                label: name.to_string(),
                kind: CompletionKind::Function,
                detail: sig.to_string(),
                insert: format!("{name}()"),
                help,
            }
        })
        .collect()
}

pub fn keyword_operator_items() -> Vec<CompletionItem> {
    KEYWORD_OPERATORS
        .iter()
        .map(|&kw| CompletionItem {
            label: kw.to_string(),
            kind: CompletionKind::Keyword,
            detail: "keyword".to_string(),
            insert: kw.to_string(),
            help: None,
        })
        .collect()
}
