use super::*;
use std::io::Write;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct LogWriter(Arc<Mutex<Vec<u8>>>);
impl Write for LogWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

#[test]
fn core_c_translation_warnings_name_only_rejected_messages_and_terms() {
    let allowed = AllowedIds::new("example", &["example/node".into()], &[]);
    let writer = LogWriter(Arc::default());
    let output = writer.0.clone();
    let subscriber = tracing_subscriber::fmt().without_time().with_ansi(false).with_writer(move || writer.clone()).finish();
    tracing::subscriber::with_default(subscriber, || {
        let source = "# Documentation\n\nnode-example-node-title = Title\nforeign-title = Foreign\n-example-term = Own\n-foreign-term = Foreign\n";
        let filtered = filter_plugin_resource("example", "en", source, &allowed).unwrap();
        assert_eq!(message_ids(&filtered), ["node-example-node-title"]);
        assert!(filtered.contains("-example-term = Own"));
        assert!(!filtered.contains("Documentation"), "comments are omitted from delivered resources");
        let log = String::from_utf8(output.lock().unwrap().clone()).unwrap();
        assert!(log.contains("foreign-title") && log.contains("-foreign-term"), "{log}");
        assert!(!log.contains("node-example-node-title") && !log.contains("-example-term"), "{log}");
        output.lock().unwrap().clear();
        let filtered = filter_plugin_resource("example", "en", "example-error = Kept\n-example-term = Kept\n", &allowed).unwrap();
        assert_eq!(message_ids(&filtered), ["example-error"]);
        assert!(filtered.contains("-example-term = Kept"));
        assert!(output.lock().unwrap().is_empty(), "valid resources must not produce a rejection warning");
    });
}

#[test]
fn core_c_merged_resources_omit_comments_and_preserve_messages_and_terms() {
    let merged = merge_resources([
        ("en", "# Documentation\n\na = First\n-term = First term\n"),
        ("en", "## More documentation\n\na = Duplicate\n-term = Duplicate\nb = Second\n"),
    ]);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].0, "en");
    let source = &merged[0].1;
    assert_eq!(message_ids(source), ["a", "b"]);
    assert!(source.contains("a = First") && source.contains("b = Second") && source.contains("-term = First term"));
    assert!(!source.contains("Duplicate"));
    assert!(!source.contains('#'), "comments are omitted from delivered resources: {source}");
}

#[test]
fn core_c_locale_validation_requires_a_letter_start_and_a_valid_code() {
    for code in ["12", "-uk", "u", "uk!", "abcdefghijklmnopq"] {
        assert_eq!(normalize_locale(code), None, "{code}");
    }
    assert_eq!(normalize_locale(" EN_us "), Some("en-us".into()));
    assert_eq!(normalize_locale("uk-123"), Some("uk-123".into()));
}
