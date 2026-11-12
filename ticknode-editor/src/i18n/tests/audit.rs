use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::*;

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
            let prev_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
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
    let mut dirs = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    let mut files = Vec::new();
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).expect("a source directory exists") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
    let mut checked = 0usize;
    for path in files {
        let content = fs::read_to_string(&path).expect("reading a .rs file");
        for key in scan_t_calls(&content) {
            BUNDLES.with(|b| {
                let b = b.borrow();
                assert!(
                    b.uk.has_message(&key),
                    "key '{key}' from {} is missing in locales/uk/ui.ftl",
                    path.display()
                );
                assert!(
                    b.en.has_message(&key),
                    "key '{key}' from {} is missing in locales/en/ui.ftl",
                    path.display()
                );
            });
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "the scanner must find translation macro calls in the source"
    );
}
