use super::*;
use std::time::Duration;
use ticknode_core::TickBudget;

#[test]
fn set_settings_validates_and_clamps() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());

    let (reply, _) = mpsc::unbounded_channel();
    let (events, _) = actor
        .apply(
            ClientMessage::SetServerSettings(ServerSettings {
                tick_ms: 0,
                tick_budget: 9_000_000,
                ..Default::default()
            }),
            None,
            &reply,
        )
        .unwrap();

    let expected = ServerSettings {
        tick_ms: 1,
        tick_budget: 1_000_000,
        ..Default::default()
    };
    assert_eq!(
        events,
        vec![(None, ServerMessage::ServerSettingsChanged(expected.clone()))]
    );
    assert_eq!(actor.server_settings, expected);
    assert_eq!(
        actor.engine.host_state.tick_duration,
        Duration::from_millis(1)
    );
    assert_eq!(
        actor.engine.budget,
        TickBudget {
            limit: Some(1_000_000)
        }
    );
}

#[test]
fn set_settings_broadcasts_and_writes_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());

    let (reply, _) = mpsc::unbounded_channel();
    let (events, _) = actor
        .apply(
            ClientMessage::SetServerSettings(ServerSettings {
                tick_ms: 45,
                tick_budget: 120,
                ..Default::default()
            }),
            None,
            &reply,
        )
        .unwrap();

    let expected = ServerSettings {
        tick_ms: 45,
        tick_budget: 120,
        ..Default::default()
    };
    assert_eq!(
        events,
        vec![(None, ServerMessage::ServerSettingsChanged(expected.clone()))]
    );
    assert_eq!(actor.server_settings, expected);

    // Verify settings.json file is written and contains pretty JSON
    let path = actor.storage.settings_json_path();
    assert!(path.exists());
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"tick_ms\": 45"));
    assert!(text.contains("\"tick_budget\": 120"));
    let from_file = actor.storage.load_settings().unwrap();
    assert_eq!(from_file, expected);
}

#[tokio::test]
async fn live_tick_change_speeds_up_actor() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = TickEngine::new();
    let storage = Storage::new(dir.path());
    let loaded = storage.load(&mut engine).unwrap();
    let sheet_id = loaded.sheets[0].id;

    // Start with a very slow tick rate: 1000 ms
    let handle = spawn(
        engine,
        loaded,
        storage,
        Plugins::new(dir.path()),
        ServerSettings {
            tick_ms: 1000,
            tick_budget: 0,
            ..Default::default()
        },
    )
    .unwrap();

    let mut events = handle.subscribe();
    let (reply, _) = mpsc::unbounded_channel();

    // Add a clock node that fires every tick
    handle.send(
        ClientMessage::AddNodeWith {
            type_name: "clock".into(),
            position: Position::default(),
            properties: vec![("interval_ticks".into(), DataValue::from(1))],
        },
        Some(sheet_id),
        reply.clone(),
    );

    // Drain initial events (NodeAdded, etc.)
    tokio::time::sleep(Duration::from_millis(50)).await;
    while events.try_recv().is_ok() {}

    // With 1000ms ticks, in 100ms we shouldn't get many Executed events (at most 1)
    let mut count_before = 0;
    let window_start = tokio::time::Instant::now();
    while window_start.elapsed() < Duration::from_millis(100) {
        if let Ok(Ok((_, bytes))) =
            tokio::time::timeout(Duration::from_millis(20), events.recv()).await
        {
            if let Ok(ServerMessage::Executed(_)) = postcard::from_bytes::<ServerMessage>(&bytes) {
                count_before += 1;
            }
        }
    }
    assert!(
        count_before <= 1,
        "Slow tick rate should yield at most 1 execution in 100ms, got {count_before}"
    );

    // Change settings to fast ticks: 10 ms
    handle.send(
        ClientMessage::SetServerSettings(ServerSettings {
            tick_ms: 10,
            tick_budget: 0,
            ..Default::default()
        }),
        None,
        reply,
    );

    // Collect tick advancement over a 300ms window
    let mut max_tick_seen = 0u64;
    let mut min_tick_seen = u64::MAX;
    let window_start = tokio::time::Instant::now();
    while window_start.elapsed() < Duration::from_millis(300) {
        if let Ok(Ok((_, bytes))) =
            tokio::time::timeout(Duration::from_millis(50), events.recv()).await
        {
            if let Ok(ServerMessage::Executed(traces)) =
                postcard::from_bytes::<ServerMessage>(&bytes)
            {
                for t in traces {
                    min_tick_seen = min_tick_seen.min(t.tick);
                    max_tick_seen = max_tick_seen.max(t.tick);
                }
            }
        }
    }
    assert!(
        min_tick_seen != u64::MAX && max_tick_seen >= min_tick_seen + 10,
        "Fast tick rate should advance tick counter significantly: min {min_tick_seen}, max {max_tick_seen}"
    );

    handle.shutdown().await;
}

#[test]
fn formatting_locale_round_trips_and_updates_engine() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());
    let (reply, _) = mpsc::unbounded_channel();
    for (input, expected) in [("uk", "uk"), ("en", "en"), ("invalid", "en")] {
        let settings = ServerSettings {
            locale: input.into(),
            ..Default::default()
        };
        let (events, _) = actor
            .apply(ClientMessage::SetServerSettings(settings), None, &reply)
            .unwrap();
        assert_eq!(actor.server_settings.locale, expected);
        assert_eq!(actor.engine.host_state.locale, expected);
        assert_eq!(actor.storage.load_settings().unwrap().locale, expected);
        assert!(
            matches!(&events[0].1, ServerMessage::ServerSettingsChanged(s) if s.locale == expected)
        );
    }
}

#[test]
fn display_settings_apply_live_and_invalid_updates_change_nothing() {
    use ticknode_shared::{DisplayRules, QuantityDisplay, UnitStep};
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());
    let (reply, _) = tokio::sync::mpsc::unbounded_channel();
    let mut settings = ServerSettings {
        locale: "uk".into(),
        display: DisplayRules {
            quantities: [(
                "quantity.power".into(),
                QuantityDisplay {
                    digits: 3,
                    steps: vec![UnitStep {
                        from: 0.0,
                        unit: "watt".into(),
                    }],
                    rounding: Default::default(),
                },
            )]
            .into(),
            ..Default::default()
        },
        ..Default::default()
    };
    actor
        .apply(
            ClientMessage::SetServerSettings(settings.clone()),
            None,
            &reply,
        )
        .unwrap();
    assert_eq!(actor.engine.host_state.display, settings.display);
    let saved = std::fs::read(actor.storage.settings_json_path()).unwrap();
    let previous = actor.server_settings.clone();
    settings.tick_ms = 10;
    settings.locale = "en".into();
    settings
        .display
        .quantities
        .get_mut("quantity.power")
        .unwrap()
        .digits = 0;
    let error = actor
        .apply(ClientMessage::SetServerSettings(settings), None, &reply)
        .unwrap_err();
    assert_eq!(error.key, "error-display-digits");
    assert_eq!(actor.server_settings, previous);
    assert_eq!(actor.engine.host_state.locale, "uk");
    assert_eq!(actor.engine.host_state.display, previous.display);
    assert_eq!(
        std::fs::read(actor.storage.settings_json_path()).unwrap(),
        saved
    );
}
