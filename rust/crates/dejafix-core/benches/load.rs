use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use dejafix_core::testing::{FIXTURE_SEED, write_project};
use dejafix_core::{Clock, EventLog, FsResolver, path_context, read_events_file};
use jiff::tz::TimeZone;

#[expect(
    clippy::expect_used,
    reason = "a benchmark without its fixture must fail loudly"
)]
fn load(criterion: &mut Criterion) {
    let clock = Clock {
        now: jiff::Timestamp::now(),
        zone: TimeZone::UTC,
    };
    let mut group = criterion.benchmark_group("event_log");
    for size in [3_000, 30_000] {
        let dir = tempfile::tempdir().expect("temporary directory");
        write_project(dir.path(), size, FIXTURE_SEED).expect("fixture");
        let mem = dir.path().join(".dejafix");
        let bytes = read_events_file(&mem).expect("fixture log");
        let context = path_context(dir.path()).expect("fixture root");
        group.throughput(Throughput::Bytes(u64::try_from(bytes.len()).unwrap_or(0)));
        group.bench_with_input(BenchmarkId::new("parse", size), &bytes, |bencher, bytes| {
            bencher.iter(|| {
                EventLog::parse(black_box(bytes), &clock, &context, &FsResolver)
                    .events()
                    .len()
            });
        });
        group.bench_with_input(
            BenchmarkId::new("read_and_parse", size),
            &mem,
            |bencher, mem| {
                bencher.iter(|| {
                    read_events_file(black_box(mem)).map(|bytes| {
                        EventLog::parse(&bytes, &clock, &context, &FsResolver)
                            .events()
                            .len()
                    })
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, load);
criterion_main!(benches);
