use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use dejafix_core::testing::{FIXTURE_SEED, gotcha_lines, write_project};
use dejafix_core::{
    Clock, Document, EventLog, FieldSpec, FsResolver, Index, RankParams, path_context,
    read_events_file,
};
use jiff::tz::TimeZone;
use serde_json::Value;

const GOTCHA_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "library",
        weight: 3.0,
    },
    FieldSpec {
        name: "keywords",
        weight: 2.0,
    },
    FieldSpec {
        name: "gotcha",
        weight: 1.0,
    },
];

const EVENT_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "summary",
        weight: 2.0,
    },
    FieldSpec {
        name: "location",
        weight: 2.0,
    },
    FieldSpec {
        name: "notes",
        weight: 1.0,
    },
];

struct Gotcha(Value);

impl Document for Gotcha {
    fn id(&self) -> &str {
        self.0.get("id").and_then(Value::as_str).unwrap_or("")
    }

    fn texts<'d>(&'d self, field: usize, visit: &mut dyn FnMut(&'d str)) {
        let name = ["library", "keywords", "gotcha"].get(field).copied();
        match name.and_then(|name| self.0.get(name)) {
            Some(Value::String(text)) => visit(text),
            Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).for_each(visit),
            _ => {}
        }
    }
}

struct EventDoc<'e, 'a>(&'e dejafix_core::Event<'a>);

impl Document for EventDoc<'_, '_> {
    fn id(&self) -> &str {
        self.0.id.as_str()
    }

    fn texts<'d>(&'d self, field: usize, visit: &mut dyn FnMut(&'d str)) {
        match field {
            0 => visit(self.0.summary.as_str()),
            1 => {
                self.0.location.as_deref().into_iter().for_each(&mut *visit);
                self.0.files.iter().map(AsRef::as_ref).for_each(visit);
            }
            _ => self.0.notes.as_deref().into_iter().for_each(visit),
        }
    }
}

#[expect(
    clippy::expect_used,
    reason = "a benchmark without its fixture must fail loudly"
)]
fn rank(criterion: &mut Criterion) {
    let gotchas: Vec<Gotcha> = gotcha_lines(500, FIXTURE_SEED)
        .iter()
        .map(|line| Gotcha(serde_json::from_str(line).expect("generated line")))
        .collect();
    let mut group = criterion.benchmark_group("rank");
    group.bench_function("build_500_gotchas", |bencher| {
        bencher.iter(|| Index::build(GOTCHA_FIELDS, black_box(&gotchas)).len());
    });
    let index = Index::build(GOTCHA_FIELDS, &gotchas);
    group.bench_function("query_500_gotchas", |bencher| {
        bencher.iter(|| {
            index
                .rank(
                    black_box("worker never exits on shutdown"),
                    &RankParams::DEFAULT,
                )
                .len()
        });
    });
    let dir = tempfile::tempdir().expect("temporary directory");
    write_project(dir.path(), 3_000, FIXTURE_SEED).expect("fixture");
    let bytes = read_events_file(&dir.path().join(".dejafix")).expect("fixture log");
    let context = path_context(dir.path()).expect("fixture root");
    let clock = Clock {
        now: jiff::Timestamp::now(),
        zone: TimeZone::UTC,
    };
    let log = EventLog::parse(&bytes, &clock, &context, &FsResolver);
    let events: Vec<EventDoc<'_, '_>> = log.events().iter().map(EventDoc).collect();
    group.bench_function("build_3000_events", |bencher| {
        bencher.iter(|| Index::build(EVENT_FIELDS, black_box(&events)).len());
    });
    let index = Index::build(EVENT_FIELDS, &events);
    group.bench_function("query_3000_events", |bencher| {
        bencher.iter(|| {
            index
                .rank(black_box("token refresh login loop"), &RankParams::DEFAULT)
                .len()
        });
    });
    group.finish();
}

criterion_group!(benches, rank);
criterion_main!(benches);
