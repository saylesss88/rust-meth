# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-05-13

### Added
- add Fuzzy filter and interactive method picker
- `rust-meth` binary target to `src/main.rs`
- `Probe`: creates a minimal temp Cargo project with a `use std::collections::*` preamble and a dot-trigger source file for any given type; cleans up on drop
- `LspTransport`: framed JSON-RPC reader/writer over rust-analyzer's stdin/stdout, with constructors for all required LSP messages (`initialize`, `initialized`, `didOpen`, `completion`, `shutdown`, `exit`)
- `analyzer`: spawns rust-analyzer, drives the full LSP session, waits for indexing, retries completion until RA returns items, extracts `CompletionItemKind::Method` entries with signatures
- Automatic rust-analyzer discovery: checks `$PATH` then falls back to `rustup which rust-analyzer`
- Optional substring filter as second argument: `rust-meth u8 wrapping`
- Aligned columnar output: method names and signatures padded to the same width
- Non-zero exit code when the type is unresolvable or the filter matches nothing

### Fixed
- Clippy lints
- Indexing wait now treats `textDocument/publishDiagnostics` and `workspace/diagnostic/refresh` as ready signals, fixing a hang on fast/warm projects that skip `$/progress` notifications
- Completion request retries on `isIncomplete: true` with a 500ms backoff, fixing empty results on the first attempt
- Non-prelude std types (`HashMap`, `BTreeMap`, `Mutex`, etc.) now resolve correctly via the preamble imports in the probe file
