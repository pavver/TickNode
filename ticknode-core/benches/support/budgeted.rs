use super::*;

/// Measure tick latency separately from trigger enqueueing and validate the complete burst.
fn drain_budgeted(engine: &mut TickEngine) -> (usize, Duration) {
    let mut ticks = 0;
    let mut maximum = Duration::ZERO;
    while !engine.execution_queue.is_empty() {
        let start = Instant::now();
        engine.tick();
        maximum = maximum.max(start.elapsed());
        ticks += 1;
        clear_telemetry(engine);
    }
    (ticks, maximum)
}

pub(super) fn burst_budgeted(c: &mut Criterion) {
    let count = 10_000;
    let (mut engine, triggers) = automations(count);
    engine.budget.limit = Some(1_000);
    enqueue(&mut engine, &triggers);
    let (ticks, maximum) = drain_budgeted(&mut engine);
    assert_eq!(ticks, 40, "four work units per automation, including pure pulls");
    assert_eq!(engine.variables.len(), count.div_ceil(2));
    assert!(engine.variables.values().all(|value| *value == number(1)));
    eprintln!("burst_budgeted/10000: {ticks} ticks, max single tick {maximum:?} (validation run)");

    let mut group = c.benchmark_group("burst_budgeted");
    group.sample_size(10);
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(5));
    group.throughput(Throughput::Elements(count as u64));
    let mut measured_maximum = Duration::ZERO;
    group.bench_function("10000", |b| {
        b.iter_custom(|iterations| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iterations {
                enqueue(&mut engine, &triggers);
                let start = Instant::now();
                let result = drain_budgeted(black_box(&mut engine));
                elapsed += start.elapsed();
                measured_maximum = measured_maximum.max(result.1);
                black_box(result);
            }
            elapsed
        });
    });
    group.finish();
    eprintln!("burst_budgeted/10000: {ticks} ticks per burst, max measured single tick {measured_maximum:?}");
}
