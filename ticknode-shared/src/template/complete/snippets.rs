//! Template snippet definitions and tag closure detection for completion.

/// Returns the snippet `(insert, caret)` for a filter name.
/// `caret` is the byte offset inside `insert` where the caret should be placed.
pub fn filter_snippet(name: &str) -> (String, Option<usize>) {
    match name {
        "date" => ("date('')".to_string(), Some(6)),
        "strftime" => ("strftime('')".to_string(), Some(10)),
        "default" => ("default('')".to_string(), Some(9)),
        "replace" => ("replace('', '')".to_string(), Some(9)),
        "join" => ("join('')".to_string(), Some(6)),
        "unit" => ("unit('')".to_string(), Some(6)),
        "duration" => ("duration('')".to_string(), Some(10)),
        "plural" => ("plural('')".to_string(), Some(8)),
        "round" => ("round()".to_string(), Some(6)),
        _ => (name.to_string(), None),
    }
}

/// Returns the snippet `(insert, caret)` for a function name.
pub fn function_snippet(name: &str) -> (String, Option<usize>) {
    let insert = format!("{name}()");
    let caret = if name == "now" {
        None
    } else {
        Some(name.len() + 1)
    };
    (insert, caret)
}

/// Returns true if the tag has already been closed after the caret before the next `{`.
pub fn is_tag_closed_after(after_caret: &str) -> bool {
    if let Some(close_pos) = after_caret.find("%}") {
        if let Some(open_pos) = after_caret.find('{') {
            close_pos < open_pos
        } else {
            true
        }
    } else {
        false
    }
}

/// Returns the snippet `(insert, caret)` for a tag keyword.
pub fn tag_keyword_snippet(kw: &str, is_closed: bool) -> (String, Option<usize>) {
    if is_closed {
        return (kw.to_string(), None);
    }
    match kw {
        "if" => ("if  %}{% endif %}".to_string(), Some(3)),
        "for" => ("for  in  %}{% endfor %}".to_string(), Some(4)),
        "with" => ("with  %}{% endwith %}".to_string(), Some(5)),
        "macro" => ("macro  %}{% endmacro %}".to_string(), Some(6)),
        "filter" => ("filter  %}{% endfilter %}".to_string(), Some(7)),
        "call" => ("call  %}{% endcall %}".to_string(), Some(5)),
        "raw" => ("raw %}{% endraw %}".to_string(), Some(6)),
        _ => (kw.to_string(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_snippets() {
        assert_eq!(filter_snippet("date"), ("date('')".to_string(), Some(6)));
        assert_eq!(
            filter_snippet("strftime"),
            ("strftime('')".to_string(), Some(10))
        );
        assert_eq!(
            filter_snippet("default"),
            ("default('')".to_string(), Some(9))
        );
        assert_eq!(
            filter_snippet("replace"),
            ("replace('', '')".to_string(), Some(9))
        );
        assert_eq!(filter_snippet("join"), ("join('')".to_string(), Some(6)));
        assert_eq!(filter_snippet("unit"), ("unit('')".to_string(), Some(6)));
        assert_eq!(
            filter_snippet("duration"),
            ("duration('')".to_string(), Some(10))
        );
        assert_eq!(
            filter_snippet("plural"),
            ("plural('')".to_string(), Some(8))
        );
        assert_eq!(filter_snippet("round"), ("round()".to_string(), Some(6)));
        assert_eq!(filter_snippet("abs"), ("abs".to_string(), None));
    }

    #[test]
    fn test_function_snippets() {
        assert_eq!(function_snippet("range"), ("range()".to_string(), Some(6)));
        assert_eq!(function_snippet("debug"), ("debug()".to_string(), Some(6)));
        assert_eq!(function_snippet("now"), ("now()".to_string(), None));
    }

    #[test]
    fn test_is_tag_closed_after() {
        assert!(is_tag_closed_after(" %}"));
        assert!(is_tag_closed_after(" %}\n<div>{% endif %}"));
        assert!(!is_tag_closed_after(""));
        assert!(!is_tag_closed_after("\n  <div>\n{% endif %}"));
    }

    #[test]
    fn test_tag_keyword_snippets_unclosed() {
        assert_eq!(
            tag_keyword_snippet("if", false),
            ("if  %}{% endif %}".to_string(), Some(3))
        );
        assert_eq!(
            tag_keyword_snippet("for", false),
            ("for  in  %}{% endfor %}".to_string(), Some(4))
        );
        assert_eq!(
            tag_keyword_snippet("with", false),
            ("with  %}{% endwith %}".to_string(), Some(5))
        );
        assert_eq!(
            tag_keyword_snippet("macro", false),
            ("macro  %}{% endmacro %}".to_string(), Some(6))
        );
        assert_eq!(
            tag_keyword_snippet("filter", false),
            ("filter  %}{% endfilter %}".to_string(), Some(7))
        );
        assert_eq!(
            tag_keyword_snippet("call", false),
            ("call  %}{% endcall %}".to_string(), Some(5))
        );
        assert_eq!(
            tag_keyword_snippet("raw", false),
            ("raw %}{% endraw %}".to_string(), Some(6))
        );
        assert_eq!(
            tag_keyword_snippet("else", false),
            ("else".to_string(), None)
        );
        assert_eq!(tag_keyword_snippet("set", false), ("set".to_string(), None));
    }

    #[test]
    fn test_tag_keyword_snippets_closed() {
        assert_eq!(tag_keyword_snippet("if", true), ("if".to_string(), None));
        assert_eq!(tag_keyword_snippet("for", true), ("for".to_string(), None));
        assert_eq!(tag_keyword_snippet("raw", true), ("raw".to_string(), None));
    }
}
