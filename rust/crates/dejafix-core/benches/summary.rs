use std::hint::black_box;
use std::time::Instant;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use dejafix_core::testing::{FIXTURE_SEED, write_project};
use dejafix_core::{
    Clock, EventLog, FsResolver, Project, Settings, SummaryInput, path_context, read_events_file,
    regenerate, render_summary,
};
use jiff::tz::TimeZone;

#[expect(
    clippy::expect_used,
    reason = "a benchmark without its fixture must fail loudly"
)]
fn summary(criterion: &mut Criterion) {
    let clock = Clock {
        now: jiff::Timestamp::now(),
        zone: TimeZone::UTC,
    };
    let mut group = criterion.benchmark_group("summary");
    for size in [3_000, 30_000] {
        let dir = tempfile::tempdir().expect("temporary directory");
        let home = tempfile::tempdir().expect("temporary home");
        write_project(dir.path(), size, FIXTURE_SEED).expect("fixture");
        let project = Project::open(
            dir.path(),
            Settings {
                dejafix_home: home.path().to_path_buf(),
                redact: true,
                clock: clock.clone(),
            },
        )
        .expect("project");
        let started = Instant::now();
        regenerate(&project).expect("first regeneration");
        println!("summary/first_regenerate/{size}: {:?}", started.elapsed());
        group.bench_with_input(
            BenchmarkId::new("regenerate", size),
            &project,
            |bencher, project| {
                bencher.iter(|| regenerate(black_box(project)).expect("regeneration"));
            },
        );
        let bytes = read_events_file(&dir.path().join(".dejafix")).expect("fixture log");
        let context = path_context(dir.path()).expect("fixture root");
        let log = EventLog::parse(&bytes, &clock, &context, &FsResolver);
        group.bench_with_input(BenchmarkId::new("render", size), &log, |bencher, log| {
            bencher.iter(|| {
                render_summary(
                    black_box(log),
                    &SummaryInput {
                        project_name: "bench",
                        today: jiff::civil::date(2026, 11, 12),
                        purpose: None,
                    },
                )
                .len()
            });
        });
    }
    group.finish();
}

criterion_group!(benches, summary);
criterion_main!(benches);
