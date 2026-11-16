use super::*;

#[tokio::test]
async fn startup_initializes_file_blobs_and_persists_without_a_listener() {
    for usage_stats in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("settings.json"),
            serde_json::json!({
                "tick_ms": 100, "tick_budget": 0, "usage_stats": usage_stats,
            })
            .to_string(),
        )
        .unwrap();
        let config = Config {
            bind: "127.0.0.1:0".into(),
            data_dir: temp.path().to_owned(),
            web_dir: temp.path().join("web"),
            bundled_plugins_dir: None,
        };
        let handle = initialize(&config).unwrap();
        assert!(temp.path().join("blobs").is_dir());
        handle.shutdown().await;
        assert!(temp.path().join("sheets.json").is_file());
    }
}

#[tokio::test]
async fn startup_preserves_corrupt_settings_and_rejects_an_unwritable_data_path() {
    let temp = tempfile::tempdir().unwrap();
    let settings = temp.path().join("settings.json");
    std::fs::write(&settings, "corrupt").unwrap();
    let mut config = Config {
        bind: "127.0.0.1:0".into(),
        data_dir: temp.path().to_owned(),
        web_dir: temp.path().join("web"),
        bundled_plugins_dir: None,
    };
    assert!(initialize(&config).is_err());
    assert_eq!(std::fs::read_to_string(settings).unwrap(), "corrupt");
    config.data_dir = temp.path().join("file");
    std::fs::write(&config.data_dir, "not a directory").unwrap();
    assert!(initialize(&config).is_err());
}
