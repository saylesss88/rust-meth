#![allow(clippy::unwrap_used)]
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rust_meth::{
    analyzer::{parse_definition, parse_methods},
    lsp::LspTransport,
    probe::Probe,
};
use serde_json::{Value, json};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Build a fake LSP completion response with `n` method items.
fn make_completion_response(n: usize) -> Value {
    let items: Vec<Value> = (0..n)
        .map(|i| {
            json!({
                "kind": 2,  // KIND_METHOD
                "label": format!("method_{i}(…)"),
                "detail": format!("pub fn method_{i}(&self) -> usize"),
                "documentation": { "kind": "markdown", "value": format!("Docs for method {i}") }
            })
        })
        .collect();

    json!({ "result": { "items": items, "isIncomplete": false } })
}

/// Build a fake LSP definition response (array form).
fn make_definition_response() -> Value {
    json!({
        "result": [{
            "uri": "file:///home/user/.rustup/toolchains/stable/library/core/src/num/uint_macros.rs",
            "range": {
                "start": { "line": 42, "character": 0 },
                "end":   { "line": 42, "character": 20 }
            }
        }]
    })
}

// ── 1. parse_methods ─────────────────────────────────────────────────────────

fn bench_parse_methods(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_methods");

    for size in [10, 50, 100, 300] {
        let response = make_completion_response(size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &response, |b, r| {
            b.iter(|| parse_methods(black_box(r)).unwrap());
        });
    }

    group.finish();
}

// ── 2. parse_definition ──────────────────────────────────────────────────────

fn bench_parse_definition(c: &mut Criterion) {
    let response = make_definition_response();
    c.bench_function("parse_definition", |b| {
        b.iter(|| parse_definition(black_box(&response)));
    });
}

// ── 3. LspTransport message constructors ─────────────────────────────────────

fn bench_lsp_message_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsp_messages");

    group.bench_function("initialize", |b| {
        b.iter(|| LspTransport::initialize(black_box(12345), black_box("file:///tmp/probe")));
    });

    group.bench_function("completion", |b| {
        b.iter(|| {
            LspTransport::completion(
                black_box(3),
                black_box("file:///tmp/probe/src/main.rs"),
                black_box(11),
                black_box(7),
            )
        });
    });

    group.bench_function("definition", |b| {
        b.iter(|| {
            LspTransport::definition(
                black_box(3),
                black_box("file:///tmp/probe/src/main.rs"),
                black_box(11),
                black_box(7),
            )
        });
    });

    group.finish();
}

// ── 4. Probe creation (I/O) ──────────────────────────────────────────────────

fn bench_probe_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("probe_creation");

    // Stdlib type — no deps section written
    group.bench_function("stdlib_type", |b| {
        b.iter(|| {
            let p = Probe::new_with_deps(black_box("Vec<u8>"), None).unwrap();
            // Drop triggers cleanup; we want to measure full create+destroy cycle
            drop(p);
        });
    });

    // Type with dependencies
    group.bench_function("with_deps", |b| {
        b.iter(|| {
            let p = Probe::new_with_deps(
                black_box("serde_json::Value"),
                Some(black_box(r#"serde_json = "1.0""#)),
            )
            .unwrap();
            drop(p);
        });
    });

    group.finish();
}

// ── criterion entry points ────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_parse_methods,
    bench_parse_definition,
    bench_lsp_message_construction,
    bench_probe_creation,
);
criterion_main!(benches);
