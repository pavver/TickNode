use super::{Completion, CompletionInput, CompletionItem, CompletionKind};

const DATE_PRESETS: &[(&str, &str)] = &[
    ("datetime", "Standard date and time"),
    ("date", "Date only"),
    ("time", "Time only"),
    (
        "long",
        "Long date with month name in the formatting language",
    ),
    ("iso", "ISO 8601 date and time"),
];

const DATE_TOKENS: &[(&str, &str, &str)] = &[
    ("YYYY", "year4", "Year, 4 digits"),
    ("YY", "year2", "Year, 2 digits"),
    (
        "MMMM",
        "month-name",
        "Month name (with a day in the pattern, the grammatical form used for dates)",
    ),
    ("MMM", "month-short", "Abbreviated month name"),
    ("MM", "month2", "Month, 2 digits (01–12)"),
    ("M", "month1", "Month, 1 or 2 digits (1–12)"),
    ("DD", "day2", "Day of the month, 2 digits (01–31)"),
    ("D", "day1", "Day of the month, 1 or 2 digits (1–31)"),
    ("dddd", "weekday-name", "Full weekday name"),
    ("ddd", "weekday-short", "Abbreviated weekday name"),
    ("HH", "hour24-2", "Hour 00–23, 2 digits"),
    ("H", "hour24-1", "Hour 0–23, 1 or 2 digits"),
    ("hh", "hour12-2", "Hour 01–12, use with A for AM/PM"),
    ("h", "hour12-1", "Hour 1–12, use with A for AM/PM"),
    ("mm", "minute2", "Minute, 2 digits (00–59)"),
    ("m", "minute1", "Minute, 1 or 2 digits (0–59)"),
    ("ss", "second2", "Second, 2 digits (00–59)"),
    ("s", "second1", "Second, 1 or 2 digits (0–59)"),
    ("SSS", "millis", "Milliseconds, 3 digits"),
    ("A", "ampm-upper", "AM or PM, uppercase"),
    ("a", "ampm-lower", "am or pm, lowercase"),
    ("ZZ", "offset-plain", "UTC offset without colon, e.g. +0200"),
    ("Z", "offset-colon", "UTC offset with colon, e.g. +02:00"),
];

const STRFTIME_TOKENS: &[(&str, &str, &str)] = &[
    ("%Y", "upper-y", "Year, 4 digits"),
    ("%y", "y", "Year, 2 digits (00–99)"),
    ("%m", "m", "Month, 2 digits (01–12)"),
    ("%B", "upper-b", "Full month name"),
    ("%b", "b", "Abbreviated month name"),
    ("%d", "d", "Day of the month, 2 digits (01–31)"),
    ("%e", "e", "Day of the month, space-padded ( 1–31)"),
    ("%A", "upper-a", "Full weekday name"),
    ("%a", "a", "Abbreviated weekday name"),
    ("%j", "j", "Day of the year, 3 digits (001–366)"),
    ("%H", "upper-h", "Hour 00–23, 2 digits"),
    ("%I", "upper-i", "Hour 01–12, 2 digits"),
    ("%M", "upper-m", "Minute, 2 digits (00–59)"),
    ("%S", "upper-s", "Second, 2 digits (00–59)"),
    ("%f", "f", "Microseconds, 6 digits"),
    ("%p", "p", "AM or PM designation"),
    ("%z", "z", "UTC offset (+HHMM or -HHMM)"),
    ("%Z", "upper-z", "Time zone name or abbreviation"),
    ("%%", "percent", "Literal % character"),
];

fn is_first_arg_of_filter(before_quote: &str, filter_name: &str) -> bool {
    let s = before_quote.trim_end();
    let Some(without_paren) = s.strip_suffix('(') else {
        return false;
    };
    let without_name = without_paren.trim_end();
    let Some(without_name) = without_name.strip_suffix(filter_name) else {
        return false;
    };
    let without_pipe = without_name.trim_end();
    without_pipe.ends_with('|') && !without_pipe.ends_with("||")
}

pub(crate) fn try_complete_date_pattern(
    input: &CompletionInput,
    expr_start: usize,
    quote: u8,
    quote_pos: usize,
    caret: usize,
) -> Option<Completion> {
    if quote_pos < expr_start || quote_pos >= input.source.len() {
        return None;
    }
    let before_quote = &input.source[expr_start..quote_pos];

    if is_first_arg_of_filter(before_quote, "date") {
        complete_date_arg(input, quote, quote_pos, caret)
    } else if is_first_arg_of_filter(before_quote, "strftime") {
        complete_strftime_arg(input, quote, quote_pos, caret)
    } else {
        None
    }
}

fn complete_date_arg(
    input: &CompletionInput,
    quote: u8,
    quote_pos: usize,
    caret: usize,
) -> Option<Completion> {
    let s_before = if quote_pos < caret && caret <= input.source.len() {
        &input.source[quote_pos + 1..caret]
    } else {
        ""
    };

    let bytes = input.source.as_bytes();
    let mut start = caret;
    while start > quote_pos + 1 && bytes[start - 1].is_ascii_alphabetic() {
        start -= 1;
    }
    let mut end = caret;
    while end < bytes.len() && bytes[end] != quote && bytes[end].is_ascii_alphabetic() {
        end += 1;
    }
    let range = start..end;
    let partial = &input.source[start..caret];

    let presets_allowed = s_before.chars().all(|c| c.is_ascii_alphabetic());

    let mut items = Vec::new();

    if presets_allowed {
        let partial_lower = partial.to_ascii_lowercase();
        for &(preset, fallback) in DATE_PRESETS {
            if preset.starts_with(&partial_lower) {
                let detail = if let (Some(now), Some(labels)) = (input.now, input.labels) {
                    crate::template::dates::example(preset, now, labels).unwrap_or_default()
                } else {
                    String::new()
                };
                items.push(CompletionItem {
                    label: preset.to_string(),
                    kind: CompletionKind::Format,
                    detail,
                    insert: preset.to_string(),
                    help: Some(crate::UiMessage::new(
                        format!("template-help-date-preset-{preset}"),
                        fallback,
                    )),
                });
            }
        }
    }

    for &(token, slug, fallback) in DATE_TOKENS {
        if token.starts_with(partial) {
            let detail = if let (Some(now), Some(labels)) = (input.now, input.labels) {
                crate::template::dates::example(token, now, labels).unwrap_or_default()
            } else {
                String::new()
            };
            items.push(CompletionItem {
                label: token.to_string(),
                kind: CompletionKind::Format,
                detail,
                insert: token.to_string(),
                help: Some(crate::UiMessage::new(
                    format!("template-help-date-token-{slug}"),
                    fallback,
                )),
            });
        }
    }

    if partial.is_empty() {
        items.push(CompletionItem {
            label: "[text]".to_string(),
            kind: CompletionKind::Format,
            detail: String::new(),
            insert: "[]".to_string(),
            help: Some(crate::UiMessage::new(
                "template-help-date-literal",
                "Text in square brackets is printed as is, e.g. [at]",
            )),
        });
    }

    Some(Completion { range, items })
}

fn complete_strftime_arg(
    input: &CompletionInput,
    quote: u8,
    quote_pos: usize,
    caret: usize,
) -> Option<Completion> {
    let bytes = input.source.as_bytes();
    let mut start = caret;
    while start > quote_pos + 1 && bytes[start - 1].is_ascii_alphabetic() {
        start -= 1;
    }
    let (partial, range) = if start > quote_pos + 1 && bytes[start - 1] == b'%' {
        start -= 1;
        let mut end = caret;
        while end < bytes.len() && bytes[end] != quote && bytes[end].is_ascii_alphabetic() {
            end += 1;
        }
        (&input.source[start..caret], start..end)
    } else {
        ("", caret..caret)
    };

    let mut items = Vec::new();
    for &(token, slug, fallback) in STRFTIME_TOKENS {
        if token.starts_with(partial) {
            let detail = if let Some(now) = input.now {
                let mut out = String::new();
                if std::fmt::Write::write_fmt(&mut out, format_args!("{}", now.format(token)))
                    .is_ok()
                {
                    out
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            items.push(CompletionItem {
                label: token.to_string(),
                kind: CompletionKind::Format,
                detail,
                insert: token.to_string(),
                help: Some(crate::UiMessage::new(
                    format!("template-help-strftime-{slug}"),
                    fallback,
                )),
            });
        }
    }

    Some(Completion { range, items })
}
