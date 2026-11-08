//! Engine measurements using public graph-editing APIs and built-in nodes.
//! Graph construction, validation, trigger enqueueing and plugin loading are untimed.
//! Active measurements include per-tick telemetry cleanup, as a server consumer would do.

use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use criterion::{
    BenchmarkId, Criterion, SamplingMode, Throughput, criterion_group, criterion_main,
};
use ticknode_core::TickEngine;
use ticknode_shared::{DataType, DataValue, NodeId, NumberValue, PinId};

fn number(value: i64) -> DataValue {
    DataValue::Number(NumberValue::Int(value))
}

fn strings(value: &str) -> DataValue {
    DataValue::List {
        element_type: DataType::String,
        elements: vec![DataValue::String(value.into())],
    }
}

fn node(engine: &mut TickEngine, type_name: &str) -> NodeId {
    let id = NodeId::generate();
    engine.create_node(id, type_name, &[]).unwrap();
    id
}

fn clear_telemetry(engine: &mut TickEngine) {
    engine.host_state.logs.clear();
    engine.executed.clear();
}

/// Each independent automation has exactly four nodes. Alternate numeric and boolean
/// conditions, pure constant and logic sources, and variable-write and console actions.
fn automations(count: usize) -> (TickEngine, Vec<NodeId>) {
    let mut engine = TickEngine::new();
    let mut triggers = Vec::with_capacity(count);
    for index in 0..count {
        let trigger = node(&mut engine, "test/click_button");
        let condition = node(&mut engine, "match");
        let numeric = index % 2 == 0;
        let source = node(&mut engine, if numeric { "const" } else { "logical/not" });
        let action = node(
            &mut engine,
            if numeric { "variable/set" } else { "console" },
        );

        engine
            .set_node_property(
                condition,
                "inputs",
                strings(if numeric { "1:a:number" } else { "1:a:bool" }),
            )
            .unwrap();
        engine
            .set_node_property(
                condition,
                "cases",
                strings(if numeric { "1: a > 0" } else { "1: a = true" }),
            )
            .unwrap();
        if numeric {
            engine
                .set_node_property(source, "value", number(1))
                .unwrap();
            engine
                .set_node_property(
                    action,
                    "var_name",
                    DataValue::String(format!("automation_{index}")),
                )
                .unwrap();
        } else {
            engine
                .set_input_value(source, PinId(0), Some(DataValue::Bool(false)))
                .unwrap();
        }
        engine
            .set_input_value(action, PinId(1), Some(number(1)))
            .unwrap();
        engine
            .connect_exec(trigger, PinId(0), condition, PinId(0))
            .unwrap();
        engine
            .connect_exec(condition, PinId(1), action, PinId(0))
            .unwrap();
        engine
            .connect_data(
                source,
                PinId(if numeric { 0 } else { 2 }),
                condition,
                PinId(1),
            )
            .unwrap();
        triggers.push(trigger);
    }
    (engine, triggers)
}

fn enqueue(engine: &mut TickEngine, triggers: &[NodeId]) {
    for &trigger in triggers {
        engine.trigger_node(trigger, PinId(0));
    }
}

fn drain(engine: &mut TickEngine) {
    while !engine.execution_queue.is_empty() {
        engine.tick();
        clear_telemetry(engine);
    }
}

/// Validate every action and the lazy data pulls once, outside the measured loop.
fn validate_automations(engine: &mut TickEngine, triggers: &[NodeId]) {
    enqueue(engine, triggers);
    let mut ticks = 0;
    let mut executions = 0;
    let mut logs = 0;
    while !engine.execution_queue.is_empty() {
        engine.tick();
        ticks += 1;
        executions += engine.executed.len();
        logs += engine.host_state.logs.len();
        clear_telemetry(engine);
        assert!(ticks <= 3, "automation did not drain in three ticks");
    }
    assert_eq!(ticks, 3);
    assert_eq!(executions, triggers.len() * 4);
    assert_eq!(logs, triggers.len() / 2);
    assert_eq!(engine.variables.len(), triggers.len().div_ceil(2));
    assert!(engine.variables.values().all(|value| *value == number(1)));
}

fn idle_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("idle_tick");
    group.sample_size(50);
    for count in [100, 1_000, 10_000] {
        let (mut engine, _) = automations(count / 4);
        assert_eq!(engine.node_ids().len(), count);
        // Keep these graphs entirely untriggered, including during validation.
        engine.tick();
        assert!(engine.execution_queue.is_empty());
        assert!(engine.executed.is_empty());
        assert!(engine.host_state.logs.is_empty());
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                black_box(&mut engine).tick();
                clear_telemetry(&mut engine);
            });
        });
    }
    group.finish();
}

fn hop(c: &mut Criterion) {
    const HOPS: u64 = 100;
    let mut engine = TickEngine::new();
    let trigger = node(&mut engine, "test/click_button");
    let mut previous = trigger;
    // 100 exec edges: one button followed by 100 variable writes, taking 101 ticks.
    for _ in 0..HOPS {
        let next = node(&mut engine, "variable/set");
        engine
            .set_input_value(next, PinId(1), Some(number(1)))
            .unwrap();
        engine
            .connect_exec(previous, PinId(0), next, PinId(0))
            .unwrap();
        previous = next;
    }
    engine.trigger_node(trigger, PinId(0));
    for _ in 0..=HOPS {
        assert_eq!(engine.execution_queue.len(), 1);
        engine.tick();
        assert_eq!(engine.executed.len(), 1);
        assert!(engine.host_state.logs.is_empty());
        clear_telemetry(&mut engine);
    }
    assert!(engine.execution_queue.is_empty());
    assert_eq!(engine.variables.get("my_var"), Some(&number(1)));

    let mut group = c.benchmark_group("hop");
    group.sample_size(30);
    group.throughput(Throughput::Elements(HOPS));
    group.bench_function("100", |b| {
        b.iter_custom(|iterations| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iterations {
                engine.trigger_node(trigger, PinId(0));
                let start = Instant::now();
                drain(black_box(&mut engine));
                elapsed += start.elapsed();
            }
            elapsed
        });
    });
    group.finish();
}

fn burst(c: &mut Criterion) {
    let mut group = c.benchmark_group("burst");
    // Flat sampling bounds the number of expensive, quadratic large-graph runs.
    group.sample_size(10);
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(5));
    for count in [1_000, 10_000] {
        let (mut engine, triggers) = automations(count);
        validate_automations(&mut engine, &triggers);
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter_custom(|iterations| {
                let mut elapsed = Duration::ZERO;
                for _ in 0..iterations {
                    enqueue(&mut engine, &triggers);
                    let start = Instant::now();
                    drain(black_box(&mut engine));
                    elapsed += start.elapsed();
                }
                elapsed
            });
        });
    }
    group.finish();
}

fn plugin_tick_idle(c: &mut Criterion) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/wasm32-wasip2/debug/math_plugin.wasm");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("{} not found - plugin_tick_idle skipped", path.display());
            return;
        }
        Err(error) => panic!("{}: {error}", path.display()),
    };
    let mut engine = TickEngine::new();
    engine.load_plugin(&bytes, None, |_| vec![]).unwrap();
    for _ in 0..100 {
        node(&mut engine, "math/sin");
    }
    assert_eq!(engine.node_ids().len(), 100);
    engine.tick();
    assert!(engine.execution_queue.is_empty());
    assert!(engine.executed.is_empty());
    assert!(engine.host_state.logs.is_empty());

    let mut group = c.benchmark_group("plugin_tick_idle");
    group.sample_size(30);
    group.bench_function("100", |b| {
        b.iter(|| {
            black_box(&mut engine).tick();
            clear_telemetry(&mut engine);
        });
    });
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets = idle_tick, hop, burst, plugin_tick_idle
}
criterion_main!(benches);
