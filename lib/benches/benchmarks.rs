#![allow(clippy::unwrap_used)]
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rust_meth_lib::{
    LspTransport, Probe,
    analyzer::{parse_definition, parse_methods},
};
use serde_json::{Value, json};
use std::hint::black_box;

// ── helpers ───────────────────────────────────────────────────────────────────

fn make_completion_response(n: usize) -> Value {
    let items: Vec<Value> = (0..n)
        .map(|i| {
            json!({
                "kind": 2,
                "label": format!("method_{i}(…)"),
                "detail": format!("pub fn method_{i}(&self) -> usize"),
                "documentation": { "kind": "markdown", "value": format!("Docs for method {i}") }
            })
        })
        .collect();
    json!({ "result": { "items": items, "isIncomplete": false } })
}

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

// ── 1. parse_methods ──────────────────────────────────────────────────────────

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

// ── 2. parse_definition ───────────────────────────────────────────────────────

fn bench_parse_definition(c: &mut Criterion) {
    let response = make_definition_response();
    c.bench_function("parse_definition", |b| {
        b.iter(|| parse_definition(black_box(&response)));
    });
}

// ── 3. LspTransport message constructors ──────────────────────────────────────

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

// ── 4. Probe creation — completion (I/O) ─────────────────────────────────────

fn bench_probe_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("probe_creation");

    group.bench_function("inferred_deps", |b| {
        b.iter(|| {
            let p = Probe::new_with_deps(black_box("serde_json::Value"), None).unwrap();
            drop(p);
        });
    });

    group.bench_function("stdlib_type", |b| {
        b.iter(|| {
            let p = Probe::new_with_deps(black_box("Vec<u8>"), None).unwrap();
            drop(p);
        });
    });

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

    group.bench_function("with_multiple_deps", |b| {
        b.iter(|| {
            let p = Probe::new_with_deps(
                black_box("serde_json::Value"),
                Some(black_box(
                    "serde = { version = \"1.0\", features = [\"derive\"] }\nserde_json = \"1.0\"",
                )),
            )
            .unwrap();
            drop(p);
        });
    });

    group.finish();
}

// ── 5. Probe creation — definition (I/O) ─────────────────────────────────────

fn bench_probe_definition_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("probe_definition_creation");

    group.bench_function("stdlib_no_deps", |b| {
        b.iter(|| {
            let p = Probe::for_definition_with_deps(black_box("Vec<u8>"), black_box("len"), None)
                .unwrap();
            drop(p);
        });
    });

    group.bench_function("stdlib_with_deps", |b| {
        b.iter(|| {
            let p = Probe::for_definition_with_deps(
                black_box("serde_json::Value"),
                black_box("as_str"),
                Some(black_box(r#"serde_json = "1.0""#)),
            )
            .unwrap();
            drop(p);
        });
    });

    group.bench_function("long_method_name_with_deps", |b| {
        b.iter(|| {
            let p = Probe::for_definition_with_deps(
                black_box("serde_json::Value"),
                black_box("as_object_mut"),
                Some(black_box(r#"serde_json = "1.0""#)),
            )
            .unwrap();
            drop(p);
        });
    });

    group.finish();
}

fn bench_probe_cache_hit(c: &mut Criterion) {
    let mut group = c.benchmark_group("probe_cache");

    // Warm the cache first, then measure hit cost
    group.bench_function("memory_cache_hit", |b| {
        let _ = Probe::new_with_deps("Vec<u8>", None).unwrap();
        b.iter(|| Probe::new_with_deps(black_box("Vec<u8>"), None).unwrap());
    });

    group.bench_function("memory_cache_hit_with_deps", |b| {
        let _ = Probe::new_with_deps("serde_json::Value", Some(r#"serde_json = "1.0""#)).unwrap();
        b.iter(|| {
            Probe::new_with_deps(
                black_box("serde_json::Value"),
                Some(black_box(r#"serde_json = "1.0""#)),
            )
            .unwrap()
        });
    });

    group.finish();
}

fn bench_filter_methods(c: &mut Criterion) {
    use rust_meth_lib::analyzer::Method;
    use rust_meth_lib::query::filter_methods;

    let methods: Vec<Method> = (0..200)
        .map(|i| Method {
            name: format!("method_{i}"),
            detail: Some(format!("fn method_{i}(&self) -> usize")),
            documentation: Some(format!("Docs for method {i}")),
        })
        .collect();

    let mut group = c.benchmark_group("filter_methods");

    group.bench_function("exact_match", |b| {
        b.iter(|| filter_methods(black_box(&methods), black_box("method_42")));
    });

    group.bench_function("prefix_match", |b| {
        b.iter(|| filter_methods(black_box(&methods), black_box("method_1")));
    });

    group.bench_function("no_match", |b| {
        b.iter(|| filter_methods(black_box(&methods), black_box("zzz")));
    });

    group.bench_function("empty_query", |b| {
        b.iter(|| filter_methods(black_box(&methods), black_box("")));
    });

    group.finish();
}

fn bench_cache_key_hash(c: &mut Criterion) {
    use rust_meth_lib::probe::cache::cache_key_hash;

    c.bench_function("cache_key_hash", |b| {
        b.iter(|| {
            cache_key_hash(
                black_box("serde_json::Value"),
                black_box(Some(r#"serde_json = "1.0""#)),
                black_box(None),
                black_box("rust-analyzer 1.80.0-nightly"),
            )
        });
    });
}

// ── criterion entry points ────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_parse_methods,
    bench_parse_definition,
    bench_lsp_message_construction,
    bench_probe_creation,
    bench_probe_definition_creation,
    bench_probe_cache_hit,
    bench_filter_methods,
    bench_cache_key_hash,
);
criterion_main!(benches);
