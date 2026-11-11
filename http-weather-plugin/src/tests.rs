use super::*;

#[test]
fn city_encoding_preserves_only_unreserved_bytes() {
    assert_eq!(
        encode_component("Kyiv&units=imperial /Київ"),
        "Kyiv%26units%3Dimperial%20%2F%D0%9A%D0%B8%D1%97%D0%B2"
    );
    assert_eq!(encode_component("azAZ09-._~"), "azAZ09-._~");
}
#[test]
fn responses_trim_fields_and_keep_commas_in_description() {
    assert_eq!(
        parse(" 22.5 , Sunny, warm \n"),
        Some((22.5, "Sunny, warm".into()))
    );
    assert_eq!(parse("0,"), Some((0.0, String::new())));
    for invalid in ["22.5", "bad,Sunny", "", ",Sunny"] {
        assert_eq!(parse(invalid), None);
    }
}

#[test]
fn temperature_pin_declares_canonical_celsius() {
    let node = WeatherNode {
        url: DEFAULT_URL.into(),
        pending: None,
    };
    let definition = node.definition();
    assert_eq!(
        definition
            .pins
            .iter()
            .find(|p| p.id == TEMP)
            .unwrap()
            .pin_type,
        PinType::Data(DataType::Named("quantity.temperature".into()))
    );
    let mut node = node;
    let out = node.complete(
        Ok(TaskOutput::Http(HttpResponse {
            status: 200,
            headers: vec![],
            body: b"-12.5,Cold".to_vec(),
        })),
        &mut Http::default(),
    );
    assert!(out.contains(&Output::Data(TEMP, (-12.5).into())));
}
#[test]
fn only_current_request_completes_and_success_sets_data_before_pulsing() {
    let mut node = WeatherNode {
        url: DEFAULT_URL.into(),
        pending: Some(7),
    };
    let result = Ok(TaskOutput::Http(HttpResponse {
        status: 200,
        headers: vec![],
        body: b"22,Sunny".to_vec(),
    }));
    assert!(
        node.execute(
            Trigger::Task {
                id: 6,
                result: result.clone()
            },
            &Inputs::default()
        )
        .is_empty()
    );
    assert_eq!(node.pending, Some(7));
    assert_eq!(
        node.execute(Trigger::Task { id: 7, result }, &Inputs::default()),
        vec![
            Output::Data(TEMP, 22i64.into()),
            Output::Data(DESCRIPTION, "Sunny".into()),
            Output::Exec(SUCCESS)
        ]
    );
    assert_eq!(node.pending, None);
    for result in [
        Ok(TaskOutput::Delay),
        Ok(TaskOutput::Http(HttpResponse {
            status: 200,
            headers: vec![],
            body: b"bad".to_vec(),
        })),
    ] {
        node.pending = Some(8);
        assert_eq!(
            node.execute(Trigger::Task { id: 8, result }, &Inputs::default()),
            vec![Output::Exec(FAILURE)]
        );
        assert_eq!(node.pending, None);
    }
    node.set_property("url", "https://example.test/{city}".into());
    node.set_property("url", false.into());
    assert_eq!(
        node.properties(),
        vec![("url".into(), "https://example.test/{city}".into())]
    );
}

#[derive(Default)]
struct Http {
    calls: Vec<String>,
    fail: bool,
}
impl Client for Http {
    fn http(&mut self, request: HttpRequest) -> Result<TaskId, String> {
        assert_eq!(request.method, "GET");
        assert!(request.headers.is_empty() && request.body.is_empty());
        self.calls.push(request.url);
        if self.fail {
            Err("offline".into())
        } else {
            Ok(42)
        }
    }
    fn cancel(&mut self, task: TaskId) {
        self.calls.push(format!("cancel {task}"));
    }
    fn error(&mut self, error: &str) {
        self.calls.push(format!("error {error}"));
    }
}
#[test]
fn requests_cancel_previous_work_and_encode_city_or_default_to_kyiv() {
    let mut node = WeatherNode {
        url: DEFAULT_URL.into(),
        pending: Some(7),
    };
    let mut http = Http::default();
    let inputs = [(CITY, "Kyiv&x=1".into())].into_iter().collect();
    assert!(node.request(&inputs, &mut http).is_empty());
    assert_eq!(
        http.calls,
        ["cancel 7", "http://localhost/weather?city=Kyiv%26x%3D1"]
    );
    assert_eq!(node.pending, Some(42));
    http.calls.clear();
    http.fail = true;
    assert_eq!(
        node.request(&Inputs::default(), &mut http),
        vec![Output::Exec(FAILURE)]
    );
    assert_eq!(
        http.calls,
        [
            "cancel 42",
            "http://localhost/weather?city=Kyiv",
            "error offline"
        ]
    );
    assert_eq!(node.pending, None);
}
#[test]
fn network_and_http_errors_log_the_reason_and_emit_failure() {
    for (result, log) in [
        (Err("offline".into()), "error offline"),
        (
            Ok(TaskOutput::Http(HttpResponse {
                status: 503,
                headers: vec![],
                body: b"22,Sunny".to_vec(),
            })),
            "error HTTP 503",
        ),
    ] {
        let mut node = WeatherNode {
            url: DEFAULT_URL.into(),
            pending: Some(7),
        };
        let mut http = Http::default();
        assert_eq!(
            node.complete(result, &mut http),
            vec![Output::Exec(FAILURE)]
        );
        assert_eq!(http.calls, [log]);
        assert_eq!(node.pending, None);
    }
}
