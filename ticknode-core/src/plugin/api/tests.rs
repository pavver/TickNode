use super::*;

fn section(id: u8, data: &[u8]) -> Vec<u8> {
    let mut result = vec![id];
    let mut len = data.len();
    loop {
        let mut byte = (len & 0x7f) as u8;
        len >>= 7;
        if len != 0 {
            byte |= 0x80;
        }
        result.push(byte);
        if len == 0 {
            break;
        }
    }
    result.extend_from_slice(data);
    result
}

fn module(marker: Option<&[u8]>) -> Vec<u8> {
    let mut wasm = b"\0asm\x01\0\0\0".to_vec();
    if let Some(marker) = marker {
        let name = b"ticknode-plugin-api";
        let mut data = vec![name.len() as u8];
        data.extend_from_slice(name);
        data.extend_from_slice(marker);
        wasm.extend(section(0, &data));
    }
    wasm
}

#[test]
fn matching_api_accepts_any_sdk_version() {
    let marker = format!("ticknode-plugin-api/1 {PLUGIN_API} 9.8.7");
    assert_eq!(check_plugin_api(&module(Some(marker.as_bytes()))), Ok(()));
}

#[test]
fn nested_core_module_marker_is_found() {
    let marker = format!("ticknode-plugin-api/1 {PLUGIN_API} 0.1.0");
    let mut component = b"\0asm\x0d\0\x01\0".to_vec();
    component.extend(section(1, &module(Some(marker.as_bytes()))));
    assert_eq!(check_plugin_api(&component), Ok(()));
}

#[test]
fn mismatch_has_translatable_details() {
    let api = if PLUGIN_API == "0123456789abcdef" {
        "fedcba9876543210"
    } else {
        "0123456789abcdef"
    };
    let marker = format!("ticknode-plugin-api/1 {api} 0.0.9");
    let error = check_plugin_api(&module(Some(marker.as_bytes()))).unwrap_err();
    assert_eq!(error.key, "error-plugin-api-mismatch");
    assert_eq!(
        error.args,
        vec![
            ("plugin_api".into(), api[..8].into()),
            ("plugin_sdk".into(), "0.0.9".into()),
            ("server_api".into(), PLUGIN_API[..8].into()),
            ("server_version".into(), SERVER_VERSION.into()),
        ]
    );
    assert!(
        error
            .fallback
            .contains("Rebuild the plugin with the current SDK.")
    );
    assert!(
        error
            .fallback
            .contains(&format!("{} (SDK 0.0.9)", &api[..8]))
    );
    assert!(
        error
            .fallback
            .contains(&format!("{} (TickNode {SERVER_VERSION})", &PLUGIN_API[..8]))
    );
}

#[test]
fn missing_or_unparseable_marker_requests_rebuild() {
    let malformed: &[&[u8]] = &[
        b"",
        b"\xff",
        b"ticknode-plugin-api/2 0123456789abcdef 0.1.0",
        b"ticknode-plugin-api/1 short 0.1.0",
        b"ticknode-plugin-api/1 0123456789abcdeg 0.1.0",
        b"ticknode-plugin-api/1 0123456789abcdef",
        b"ticknode-plugin-api/1 0123456789abcdef 0.1.0 extra",
    ];
    for marker in std::iter::once(None).chain(malformed.iter().map(|m| Some(*m))) {
        let error = check_plugin_api(&module(marker)).unwrap_err();
        assert_eq!(error.key, "error-plugin-api-unknown");
        assert_eq!(
            error.args,
            vec![
                ("server_api".into(), PLUGIN_API[..8].into()),
                ("server_version".into(), SERVER_VERSION.into()),
            ]
        );
        assert!(error.fallback.contains("predates plugin API checks"));
    }
}

#[test]
fn garbage_keeps_a_plain_parse_error() {
    for wasm in [b"not wasm".as_slice(), b"\0asm\x01\0\0\0\0\x7f"] {
        let error = check_plugin_api(wasm).unwrap_err();
        assert!(error.key.is_empty());
        assert!(error.fallback.starts_with("Invalid component:"));
    }
}
