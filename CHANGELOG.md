# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

# [rust-meth-lib 0.4.0]

### Added

- An mdBook containing the examples and their output shortening the README
  significantly.

- `query` module containing a high-level builder API for querying
  `rust-analyzer` , and a standalone `filter_methods` function.

Example of builder API:

```rs
use rust_meth_lib::query::MethodQuery;
use rust_meth_lib::analyzer::find_rust_analyzer;
let ra_path = find_rust_analyzer().unwrap();
// Simple query with filter
let methods = MethodQuery::new("Vec<u8>")
    .filter("drain")
    .run(&ra_path)
    .unwrap();
// Query with definitions, third-party type
let results = MethodQuery::new("serde_json::Value")
    .deps(r#"serde_json = "1.0""#)
    .filter("as_")
    .run_with_definitions(&ra_path)
    .unwrap();
for r in results {
    if let Some(def) = r.definition {
        println!("{} → {}:{}", r.method.name, def.path, def.line + 1);
    }
}
```

Example of `filter_methods`:

```rs
use rust_meth_lib::query::filter_methods;
use rust_meth_lib::analyzer::{find_rust_analyzer, query_methods};

let ra_path = find_rust_analyzer().unwrap();
let methods = query_methods("HashMap<String, u32>", &ra_path, None).unwrap();
let filtered = filter_methods(&methods, "get");
for m in filtered {
    println!("{}", m.name);
}
```

- `lib/examples/probe_cache.rs`: demonstrates the in-process probe cache; shows
  `cache_entries` and `clear_probe_cache` usage before and after a
  `query_methods_batch` call.
- `cache_entries() -> Vec<CacheEntry>`: returns a snapshot of all probe
  directories currently held in the in-process cache.
- `clear_probe_cache()`: evicts all cache entries; each directory is deleted
  when its `Arc` refcount reaches zero.
- `CacheEntry`: public type exposing `type_name`, `deps`, `method_name`, and
  `dir` for each cached probe.

### Changed

- Split `probe.rs` into `probe/mod.rs` and `probe/cache.rs`: cache logic
  (in-process Arc cache, persistent disk cache, hashing, metadata) is now
  isolated in its own module with `pub(super)` visibility on internal types.
- Extracted `create_probe` into three focused helpers: `from_memory_cache`,
  `from_persistent_cache`, and `create_on_disk`.
- Move unit tests related to caching into `cache.rs`.
- `Probe` is now backed by a global `Arc`-based cache keyed by
  `(type_name, effective_deps, probe_kind)`. Cache hits skip temp-dir creation
  and file writes entirely. The directory is deleted when both the cache entry
  and all borrowing `Probe` instances are dropped.

### Performance

- Persistent probe cache: probe directories are saved to
  `$XDG_CACHE_HOME/rust-meth/probes/` and reused across process restarts.
  Third-party type queries skip Cargo resolution on subsequent invocations.
  Cache keys include the `rust-analyzer` version and `rust-meth-lib` version
  so upgrades automatically invalidate stale entries.
  Measured: `tokio::net::TcpStream` query time 9.0s → 4.2s on second run.
- Probe creation: **~98% reduction** (65µs → 1.1µs) on cache hits.

## [0.7.0] - 2026-08-22

- version bump to use the new `rust-meth-lib` with increased performance

# [rust-meth-lib 0.3.0]

### Added

- `query_methods_batch`: queries multiple types in parallel using
  `std::thread::scope`, one independent `rust-analyzer` subprocess per type.
  Failures are per-type and do not abort the batch. Results are returned in
  input order.
- `lib/examples/`: runnable examples covering the main embedding use cases:
  `basic_query`, `method_info`, `third_party_crate`, `fuzzy_filter`,
  `go_to_definition`, `custom_error`, `batch_query`.

- Examples directory with working examples. To run them, clone the repo, then:

```sh
cargo run --example basic_query
cargo run --example method_info
cargo run --example third_party_crate   # slow on first run (downloads serde_json)
cargo run --example fuzzy_filter
cargo run --example go_to_definition
cargo run --example custom_error
cargo run --example batch_query
```

- Examples README section

### Changed

- `query_methods` is now a thin wrapper over the internal `query_methods_inner`,
  shared with `query_methods_batch`. No change to its public signature or
  behaviour.

### Performance

- Batch queries now run in parallel rather than sequentially. Wall time for a
  3-type batch dropped from ~9.9s to ~5.1s on a 6-core machine.

### Fixed

- README is now accurate

# [0.6.2]

### Fixed

- `rust-analyzer`'s default response for non-existant methods or typos

- Timing issue for complex crates

- clippy lints

### Added

- More error types to give better direction to users

- Error type `TypeNotFound` indicating a typo or non-existant type

# [0.6.1]

### Refactor

- Create a `parse` module to further break up analyzer

- Separate analyzer.rs into it's own directory with sub-modules

### Fixed

- Replace panic code with error handling

- Failing doc tests

- Explain why `.expect` calls are always valid in `probe.rs`

# [0.6.0]

### Added

- syntax-highlighting for ```rust blocks in output

- Update dependencies

# [0.5.0]

### Added

- `--explain <method>` functionality which shows the methods full documentation

- update dependencies

# [0.4.0]

### Added

- `--explain <method>` to print the full documentation directly in the terminal.

Example:

```sh
rust-meth 'Result<u8, String>' --explain unwrap_unchecked
```

- Add `--browser` to open the official documentation for said method.

Example:

```sh
rust-meth 'Result<u8, String>' --explain unwrap_unchecked --browser
```

### Fixed

- Unidiomatic patterns

- Separate concerns after workspace refactor, move all CLI related files to cli
  where they belong.

# [0.3.0]

### Added

### Fixed

# [0.2.2] - 2026-05-20

### Added

### Fixed

- Version flag giving the version of the lib after refactor

- Bug in probe: infer_deps was triggering for standard rust-meth u8 commands

# [0.2.1] - 2026-05-20

### Added

- Add custom error handling with `thiserror`
- Refactor into a workspace

### Fixed

# [0.2.0] - 2026-05-18

### Added

- Colorized output
- `owo_colors` as a dependency
- Bench for new function `fn infer_dep`
- Infer dependency from type when `--deps` is omitted
- Demo `.gif` videos
- Dependency inference from type path: `rust-meth 'serde_json::Value'` now works
  without `--deps`, resolving the crate name from the leading path segment at
  the latest version.
- Create an ARCHITECTURE.md

### Fixed

- Failing doc test

- Exclude `assets/` dir

- Change license to Apache-2.0 over MIT OR Apache-2.0 for simplicity

- Use re-export in analyzer.rs rather than full path

# [0.1.6] - 2026-05-17

- Update README

### Added

- Create an `ARCHITECTURE.md`
- Add new benches & tests for 3rd party crates
- add `--snippet` and `--json` output for pipe-friendly/scripting mode
- Add criterion benchmark tests

### Fixed

- Move helper functions out of main into `apps.rs`
- Refactor fn run by creating helper functions.
- Refactor main, slim down main function by creating helper functions.
- Search out and replace un-idiomatic patterns
- Use criterion tests to fix expensive functions (increased perf by up to 70%)

## [0.1.0] - 2026-05-13

### Added

- add Fuzzy filter and interactive method picker
- `rust-meth` binary target to `src/main.rs`
- `Probe`: creates a minimal temp Cargo project with a `use std::collections::*`
  preamble and a dot-trigger source file for any given type; cleans up on drop
- `LspTransport`: framed JSON-RPC reader/writer over rust-analyzer's
  stdin/stdout, with constructors for all required LSP messages (`initialize`,
  `initialized`, `didOpen`, `completion`, `shutdown`, `exit`)
- `analyzer`: spawns rust-analyzer, drives the full LSP session, waits for
  indexing, retries completion until RA returns items, extracts
  `CompletionItemKind::Method` entries with signatures
- Automatic rust-analyzer discovery: checks `$PATH` then falls back to
  `rustup which rust-analyzer`
- Optional substring filter as second argument: `rust-meth u8 wrapping`
- Aligned columnar output: method names and signatures padded to the same width
- Non-zero exit code when the type is unresolvable or the filter matches nothing

### Fixed

- Clippy lints
- Indexing wait now treats `textDocument/publishDiagnostics` and
  `workspace/diagnostic/refresh` as ready signals, fixing a hang on fast/warm
  projects that skip `$/progress` notifications
- Completion request retries on `isIncomplete: true` with a 500ms backoff,
  fixing empty results on the first attempt
- Non-prelude std types (`HashMap`, `BTreeMap`, `Mutex`, etc.) now resolve
  correctly via the preamble imports in the probe file
