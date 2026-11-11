use super::{CompletionItem, CompletionKind};

pub const LOOP_ATTRS: &[&str] = &[
    "changed", "cycle", "depth", "depth0", "first", "index", "index0", "last",
    "length", "nextitem", "previtem", "revindex", "revindex0",
];

pub const TAG_KEYWORDS: &[&str] = &[
    "call", "elif", "else", "endcall", "endfilter", "endfor", "endif",
    "endmacro", "endraw", "endset", "endwith", "filter", "for", "if",
    "macro", "raw", "set", "with",
];

pub const TESTS: &[&str] = &[
    "boolean", "defined", "divisibleby", "endingwith", "eq", "escaped", "even",
    "false", "filter", "float", "ge", "gt", "in", "integer", "iterable", "le",
    "lower", "lt", "mapping", "ne", "none", "number", "odd", "safe", "sameas",
    "sequence", "startingwith", "string", "test", "true", "undefined", "upper",
];

pub const BUILTIN_FILTERS: &[(&str, &str)] = &[
    ("abs", "abs()"),
    ("attr", "attr(name)"),
    ("batch", "batch(size, [fill_with])"),
    ("bool", "bool()"),
    ("capitalize", "capitalize()"),
    ("count", "count()"),
    ("d", "d(default)"),
    ("default", "default(value)"),
    ("dictsort", "dictsort()"),
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
    ("string", "string()"),
    ("sum", "sum([attribute])"),
    ("title", "title()"),
    ("trim", "trim()"),
    ("unique", "unique()"),
    ("upper", "upper()"),
];

pub const FUNCTIONS: &[(&str, &str)] = &[
    ("debug", "debug()"),
    ("dict", "dict(...)"),
    ("namespace", "namespace(...)"),
    ("range", "range([start], stop, [step])"),
];

pub const KEYWORD_OPERATORS: &[&str] = &[
    "and", "else", "false", "if", "in", "is", "none", "not", "or", "true",
];

pub fn loop_attribute_items() -> Vec<CompletionItem> {
    LOOP_ATTRS
        .iter()
        .map(|&attr| CompletionItem {
            label: attr.to_string(),
            kind: CompletionKind::Attribute,
            detail: "loop attribute".to_string(),
            insert: attr.to_string(),
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
        })
        .collect()
}

pub fn filter_items() -> Vec<CompletionItem> {
    let mut items = Vec::with_capacity(1 + BUILTIN_FILTERS.len());
    items.push(CompletionItem {
        label: "text".to_string(),
        kind: CompletionKind::Filter,
        detail: "text".to_string(),
        insert: "text".to_string(),
    });
    for &(name, sig) in BUILTIN_FILTERS {
        items.push(CompletionItem {
            label: name.to_string(),
            kind: CompletionKind::Filter,
            detail: sig.to_string(),
            insert: name.to_string(),
        });
    }
    items
}

pub fn test_items() -> Vec<CompletionItem> {
    TESTS
        .iter()
        .map(|&name| CompletionItem {
            label: name.to_string(),
            kind: CompletionKind::Test,
            detail: "test".to_string(),
            insert: name.to_string(),
        })
        .collect()
}

pub fn function_items() -> Vec<CompletionItem> {
    FUNCTIONS
        .iter()
        .map(|&(name, sig)| CompletionItem {
            label: name.to_string(),
            kind: CompletionKind::Function,
            detail: sig.to_string(),
            insert: format!("{name}()"),
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
        })
        .collect()
}
