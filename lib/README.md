# rust-meth-lib

[![Crates.io](https://img.shields.io/crates/v/rust-meth-lib.svg)](https://crates.io/crates/rust-meth-lib)
[![Documentation](https://docs.rs/rust-meth-lib/badge.svg)](https://docs.rs/rust-meth-lib)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Core library powering [`rust-meth`](https://crates.io/crates/rust-meth).
Discovers methods available on any Rust type by driving a `rust-analyzer` LSP
session.

## Overview

`rust-meth-lib` provides the building blocks for programmatically querying
`rust-analyzer`:

- Spinning up and managing an LSP session (`LspTransport`)
- Synthesizing a temporary Rust project and injecting the target type (`Probe`)
- Requesting completions and go-to-definition results from `rust-analyzer`
- Fuzzy-filtering and ranking method results
- Exposing utilities for embedding `rust-meth` workflows in other tools
- Full working [Examples](#rust-meth-lib-examples)

Please file an Issue if any problems arise.

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
└── cli/                     : standalone binary crate that calls into this library
    ├── Cargo.toml
    └── src/
        └── main.rs
```

> UI components, CLI argument parsing, interactive selection, and spinner
> display are now maintained in [`cli/`](../cli/).

## Public API

```toml
[dependencies]
rust-meth-lib = "0.3.0"
```

### Key Types

| Item           | Description                                                                                      |
| -------------- | ------------------------------------------------------------------------------------------------ |
| `LspTransport` | Manages the stdio JSON-RPC channel to a `rust-analyzer` process                                  |
| `Probe`        | Builds a temporary Cargo project that makes the target type available for LSP queries            |
| `analyzer`     | High-level session orchestrator, combines `Probe` + `LspTransport` to return method completions |

### Basic Usage

```toml
[dependencies]
rust-meth-lib = "0.3.0"
```

- [rust_meth_lib docs.rs](https://docs.rs/rust-meth-lib/latest/rust_meth_lib/index.html) 

**Full working Example**:

```rust
use rust_meth_lib::analyzer::{find_rust_analyzer, query_methods};

fn main() -> rust_meth_lib::error::Result<()> {
    let ra_path = find_rust_analyzer()?;
    let methods = query_methods("Result<u8, &'static str>", &ra_path, None)?;

    println!("Methods available for Result<u8, &'static str>");
    for method in &methods {
        println!("{}", method.name);
    }

    Ok(())
}
```

The exact list returned varies by toolchain.

> [!NOTE]  
> The primary APIs are intended for embedding or extending the `rust-meth`
> workflow.  
> The CLI interface is maintained in a separate crate.

### Builder API

[`MethodQuery`](examples/builder_run.rs) provides a chainable interface over
`query_methods` and `filter_methods`. Use `.filter()` to narrow results by
match quality (exact > prefix > substring), and `.run_with_definitions()` to
also resolve source locations.

```rust
// Query + filter
MethodQuery::new("String")
    .filter("push")
    .run(&ra_path)?;

// Query + filter + source locations
MethodQuery::new("serde_json::Value")
    .deps(r#"serde_json = "1.0""#)
    .filter("as_str")
    .run_with_definitions(&ra_path)?;
```

See [`builder_run.rs`](examples/builder_run.rs) and
[`builder_definitions.rs`](examples/builder_definitions.rs) for full examples.

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
| `MissingStdout`            | `stdout` handle wasn't captured                                             |
| `TypeNotFound`            | `rust-analyzer` diagnostic error for the probed type                       |
| `Timeout`                 | `rust-analyzer` failed to produce a usable response within retry budget    |
| `FeatureGated`            | Type exists but requires a feature flag that wasn't enabled for this probe |

All fallible functions in this crate return `rust_meth_lib::Result<T>`. If
you’re embedding the library, you can convert into your own error type via `?`
since `RustMethError` implements `std::error::Error`.

```rust
use rust_meth_lib::{Result, analyzer::{find_rust_analyzer, query_methods}};

fn my_wrapper() -> Result<()> {
    // RustAnalyzerNotFound, Io, Json, etc. all propagate with ?
    let ra_path = find_rust_analyzer()?;
    let _methods = query_methods("Vec<u8>", &ra_path, None)?;
    Ok(())
}
```
# rust-meth-lib examples

Runnable examples covering the main ways to embed `rust-meth-lib` in your own
tooling. Each file is self-contained and documents its own `[dependencies]`
snippet.

## Prerequisites

`rust-analyzer` must be on your `PATH` or installed as a rustup component:

```sh
rustup component add rust-analyzer
```

Clone the repo:

```sh
git clone https://github.com/saylesss88/rust-meth.git
```

## Running the Examples

```sh
cargo run --example basic_query
cargo run --example method_info
cargo run --example third_party_crate   # slow on first run (downloads serde_json)
cargo run --example fuzzy_filter
cargo run --example go_to_definition
cargo run --example custom_error
cargo run --example batch_query
```

## Examples

You can find the examples in the [rust-meth-lib mdBook](https://saylesss88.github.io/rust-meth-lib/)

