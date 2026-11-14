use serde_json::json;

use super::types::SplitMix64;

const LIBRARIES: [&str; 24] = [
    "bullmq",
    "prisma",
    "next",
    "react",
    "vite",
    "vitest",
    "jest",
    "express",
    "fastify",
    "redis",
    "ioredis",
    "drizzle",
    "zod",
    "tailwindcss",
    "pothos",
    "graphql",
    "nx",
    "bun",
    "typescript",
    "eslint",
    "pydantic",
    "fastapi",
    "sqlalchemy",
    "django",
];
const SUBJECTS: [&str; 16] = [
    "Worker.close()",
    "the connection pool",
    "the build cache",
    "the schema loader",
    "the dev server",
    "the test runner",
    "the migration step",
    "the type generator",
    "the retry loop",
    "the event emitter",
    "the session store",
    "the file watcher",
    "the bundler",
    "the query planner",
    "the config loader",
    "the router",
];
const SYMPTOMS: [&str; 16] = [
    "never resolves when the server is unreachable",
    "silently drops the last batch on shutdown",
    "keeps a stale value after a hot reload",
    "reads environment variables only once at import",
    "rewrites relative paths against the wrong root",
    "times out under parallel test workers",
    "duplicates jobs after a reconnect",
    "ignores the abort signal",
    "leaks file handles on every rebuild",
    "throws on an empty array instead of returning nothing",
    "orders results by insertion instead of the given key",
    "loses precision on large integers",
    "caches the first error and replays it",
    "skips hooks declared in a nested module",
    "requires an explicit flush before exit",
    "fails only when the locale is not English",
];
const REMEDIES: [&str; 12] = [
    "pass an explicit timeout and await it",
    "close the client before the process exits",
    "pin the version until the upstream fix lands",
    "set the option in the root configuration",
    "clear the cache directory before the build",
    "wrap the call in a single transaction",
    "use the stable API instead of the experimental one",
    "register the handler before the first request",
    "convert the value to a string before sending it",
    "run the step sequentially in continuous integration",
    "read the setting at call time, not at import",
    "disable the feature flag in tests",
];
const KEYWORDS: [&str; 20] = [
    "shutdown",
    "reconnect",
    "timeout",
    "hot reload",
    "cache",
    "environment variable",
    "parallel tests",
    "abort signal",
    "file handle leak",
    "empty array",
    "ordering",
    "precision",
    "error replay",
    "nested module",
    "flush",
    "locale",
    "connection pool",
    "migration",
    "type generation",
    "watch mode",
];

#[must_use]
pub fn gotcha_lines(count: usize, seed: u64) -> Vec<String> {
    let mut rng = SplitMix64 { state: seed };
    (0..count)
        .map(|_| {
            let library = rng.pick(&LIBRARIES);
            let keywords: Vec<&str> = (0..2 + rng.below(4)).map(|_| rng.pick(&KEYWORDS)).collect();
            let gotcha = format!(
                "{library}: {} {}; {}.",
                rng.pick(&SUBJECTS),
                rng.pick(&SYMPTOMS),
                rng.pick(&REMEDIES)
            );
            json!({
                "v": 2,
                "id": format!("got_{:012x}", rng.next() & 0xffff_ffff_ffff),
                "library": library,
                "keywords": keywords,
                "versions": null,
                "gotcha": gotcha,
                "source_project": "project-a",
                "discovered": "2026-01-01",
                "reviewed": "2026-01-02",
            })
            .to_string()
        })
        .collect()
}
