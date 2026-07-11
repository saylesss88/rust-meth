# rust-meth-lib

[![Crates.io](https://img.shields.io/crates/v/rust-meth-lib.svg)](https://crates.io/crates/rust-meth-lib)
[![Documentation](https://docs.rs/rust-meth-lib/badge.svg)](https://docs.rs/rust-meth-lib)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Core library powering [`rust-meth`](https://crates.io/crates/rust-meth) —  
discovers methods available on any Rust type by driving a `rust-analyzer` LSP
session.

## Overview

`rust-meth-lib` provides the building blocks for programmatically querying
`rust-analyzer`:

- Spinning up and managing an LSP session (`LspTransport`)
- Synthesizing a temporary Rust project and injecting the target type (`Probe`)
- Requesting completions and go-to-definition results from `rust-analyzer`
- Fuzzy-filtering and ranking method results
- Exposing utilities for embedding `rust-meth` workflows in other tools

## Workspace layout

```
rust-meth/
├── Cargo.toml               — workspace manifest
├── lib/                     — rust-meth-lib (this crate)
│   ├── Cargo.toml
│   ├── README.md
│   ├── benches/
│   │   └── benchmarks.rs
│   └── src/
│       ├── lib.rs
│       ├── error.rs
│       ├── lsp.rs
│       ├── probe.rs
│       └── analyzer/
│           ├── mod.rs
│           ├── discovery.rs
│           ├── parse.rs
│           └── session.rs
└── cli/                     — standalone binary crate that calls into this library
    ├── Cargo.toml
    └── src/
        └── main.rs
```

> UI components, CLI argument parsing, interactive selection, and spinner
> display are now maintained in [`cli/`](../cli/).

## Public API

```toml
[dependencies]
rust-meth-lib = "0.2.1"
```

### Key Types

| Item           | Description                                                                                      |
| -------------- | ------------------------------------------------------------------------------------------------ |
| `LspTransport` | Manages the stdio JSON-RPC channel to a `rust-analyzer` process                                  |
| `Probe`        | Builds a temporary Cargo project that makes the target type available for LSP queries            |
| `analyzer`     | High-level session orchestrator — combines `Probe` + `LspTransport` to return method completions |

### Basic Usage

```rust
use rust_meth_lib::{LspTransport, Probe};

// Build a temporary project for the target type
let probe = Probe::new("Vec<u8>", None)?;

// Start rust-analyzer and run the LSP session
let transport = LspTransport::spawn(probe.root())?;

// Use analyzer methods to run queries, get completions, etc.
```

> [!NOTE]  
> The primary APIs are intended for embedding or extending the `rust-meth`
> workflow.  
> The CLI interface is maintained in a separate crate.

## Error Handling

The library uses a single
[`RustMethError`](https://docs.rs/rust-meth-lib/latest/rust_meth_lib/error/enum.RustMethError.html)
enum (via [`thiserror`](https://docs.rs/thiserror)) and a `Result<T>` alias over
it:

```rs
pub type Result<T> = std::result::Result<T, RustMethError>;
```

| Variant                   | Cause                                                                      |
| ------------------------- | -------------------------------------------------------------------------- |
| `Io`                      | OS / file system errors (`std::io::Error`)                                 |
| `Json`                    | JSON serialization or deserialization failure                              |
| `ParseInt`                | String-to-integer conversion failure                                       |
| `NoContentLength`         | LSP response missing a `Content-Length` header                             |
| `RecvExhausted { limit }` | Message loop hit `limit` without matching a response                       |
| `UnexpectedResponseShape` | Structurally valid JSON but missing expected fields                        |
| `RustAnalyzerNotFound`    | `rust-analyzer` not on `PATH` or in component directory                    |
| `MissingStdin`            | `stdin` handle wasn't captured                                             |
| `TypeNotFound`            | `rust-analyzer` diagnostic error for the probed type                       |
| `Timeout`                 | `rust-analyzer` failed to produce a usable response within retry budget    |
| `FeatureGated`            | Type exists but requires a feature flag that wasn't enabled for this probe |

All fallible functions in this crate return `rust_meth_lib::Result<T>`. If
you’re embedding the library, you can convert into your own error type via `?`
since `RustMethError` implements `std::error::Error`.

```rust
use rust_meth_lib::{Result, error::RustMethError};

fn my_wrapper() -> Result<()> {
    // RustAnalyzerNotFound, Io, Json, etc. all propagate with ?
    let probe = Probe::new("u8", None)?;
    Ok(())
}
```
