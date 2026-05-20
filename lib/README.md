# rust-meth-lib

[![Crates.io](https://img.shields.io/crates/v/rust-meth-lib.svg)](https://crates.io/crates/rust-meth-lib)
[![Documentation](https://docs.rs/rust-meth-lib/badge.svg)](https://docs.rs/rust-meth-lib)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Core library powering [`rust-meth`](https://crates.io/crates/rust-meth) —
discovers methods available on any Rust type by driving a `rust-analyzer` LSP
session.

> If you're looking for the CLI tool, see the [root README](../README.md).

## Overview

`rust-meth-lib` provides the building blocks for querying `rust-analyzer` programmatically:

- Spinning up and managing an LSP session (`LspTransport`)
- Synthesizing a temporary Rust project and injecting the target type (`Probe`)
- Requesting completions and go-to-definition results from `rust-analyzer`
- Fuzzy-filtering and ranking method results
- Interactive terminal UI (fuzzy picker, spinner, colorized display)
- Generating call snippets, inline docs, and browser/editor links

## Workspace layout

```
rust-meth/
├── lib/          ← this crate (rust-meth-lib)
│   └── src/
│       ├── lib.rs
│       ├── analyzer.rs    — orchestrates the full LSP session
│       ├── app.rs         — primary application logic entry point
│       ├── lsp.rs         — LSP transport (stdio JSON-RPC)
│       ├── probe.rs       — temporary project scaffolding
│       └── ui/
│           ├── args.rs        — CLI argument types
│           ├── display.rs     — colorized output formatting
│           ├── interactive.rs — fuzzy picker (dialoguer)
│           ├── links.rs       — go-to-definition & browser URL resolution
│           └── spinner.rs     — progress spinner (indicatif)
└── cli/          — thin binary crate that calls into this library
```

## Public API

```toml
[dependencies]
rust-meth-lib = "0.2"
```

### Key types

| Item | Description |
|------|-------------|
| `LspTransport` | Manages the stdio JSON-RPC channel to a `rust-analyzer` process |
| `Probe` | Builds a temporary Cargo project that makes the target type available for LSP queries |
| `analyzer` | High-level session orchestrator — combines `Probe` + `LspTransport` to return method completions |
| `app` | Entry point that ties CLI arguments to the analyzer and UI layers |
| `ui` | Terminal output: display, interactive picker, spinner, and browser links |

### Basic usage

```rust
use rust_meth_lib::{LspTransport, Probe};

// Build a temporary project for the target type
let probe = Probe::new("Vec<u8>", None)?;

// Start rust-analyzer and run the LSP session
let transport = LspTransport::spawn(probe.root())?;

// … see `analyzer::run` for the full query-and-collect flow
```

> [!NOTE]
> The full end-to-end flow is best explored through `analyzer::run` and
> `app::run`, which are what the CLI calls directly. The library is designed
> for embedding or extending the `rust-meth` workflow, not as a stable
> general-purpose LSP client.

## Error Handling

The library uses a single [`RustMethError`](https://docs.rs/rust-meth-lib/latest/rust_meth_lib/error/enum.RustMethError.html)
enum (via [`thiserror`](https://docs.rs/thiserror)) and a `Result<T>` alias over it.

| Variant | Cause |
|---------|-------|
| `Io` | OS / file system errors (`std::io::Error`) |
| `Json` | JSON serialization or deserialization failure |
| `ParseInt` | String-to-integer conversion failure |
| `NoContentLength` | LSP response missing a `Content-Length` header |
| `RecvExhausted { limit }` | Message loop hit `limit` without matching a response |
| `UnexpectedResponseShape` | Structurally valid JSON but missing expected fields |
| `RustAnalyzerNotFound` | `rust-analyzer` not on `PATH` or in component directory |

All fallible functions in this crate return `rust_meth_lib::Result<T>`.
If you're embedding the library, you can convert into your own error type via `?`
since `RustMethError` implements `std::error::Error`.

````rust
use rust_meth_lib::{Result, error::RustMethError};

fn my_wrapper() -> Result<()> {
    // RustAnalyzerNotFound, Io, Json, etc. all propagate with ?
    let probe = Probe::new("u8", None)?;
    Ok(())
}
````



## Requirements

- `rust-analyzer` on `PATH`: `rustup component add rust-analyzer`
- `rust-src` component: `rustup component add rust-src`

## Dependencies

| Crate | Purpose |
|-------|---------|
| `anyhow` | Error handling |
| `serde` / `serde_json` | LSP JSON-RPC serialization |
| `fuzzy-matcher` | Fuzzy-filter and rank method results |
| `dialoguer` | Interactive fuzzy selection |
| `owo-colors` | Colorized terminal output |
| `indicatif` | Progress spinner |
| `thiserror` | Ergonomic custom error enum derivation |

## License

[Apache-2.0](https://github.com/saylesss88/rust-meth/blob/main/LICENSE)
