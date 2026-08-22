# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

# [rust-meth-lib 0.2.4]

### Added

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
