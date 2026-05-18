## How it works

For each query, `rust-meth`:

1. Creates a temporary Cargo project (`Probe`)

A minimal workspace is written to a `rust-meth-probe-<pid>` directory in `/tmp`:

```rust
// preamble (common std imports so types resolve without full qualification)
#![allow(unused_imports)]
use std::collections::*;
use std::sync::*;
use std::cell::*;
use std::rc::Rc;
use std::io::{self, Read, Write, BufRead};
use std::fmt;
use std::ops::*;
use std::path::{Path, PathBuf};

fn main() {
    let _x: TYPE = todo!();
    _x.   // ← LSP completion trigger (or _x.METHOD(); for --gd)
}
```

If `--deps` is provided, the probe's `Cargo.toml` gets a `[dependencies]` block
injected with the raw TOML you supplied.

2. Spawn rust-analyzer

`rust-analyzer` is located via `PATH` (or `rustup` which `rust-analyzer` as a
fallback) and spawned as a subprocess with `stdin`/`stdout` piped. `stderr` is
suppressed.

3. LSP handshake

```text
initialize  →  <wait for result>  →  initialized  →  textDocument/didOpen
```

`procMacro` expansion is disabled in `initializationOptions` to speed up cold
indexing.

4. Wait for indexing


Messages are consumed until one of these signals is received (whichever comes
first):

- `$/progress` with `value.kind == "end"`
- `experimental/serverStatus` with `quiescent == true`
- `textDocument/publishDiagnostics` or `workspace/diagnostic/refresh`
- Hard 10-second timeout

5. Request completions (with retry)

`textDocument/completion` is sent at the dot position. RA can return an empty
list if it isn't fully ready, so the request is retried up to 10 times with
exponential backoff (50 ms → 100 ms → 200 ms → 300 ms).

For `--gd`, `textDocument/definition` is used instead with the same retry loop
(retrying on `-32801 content modified` and `-32800 request cancelled` errors).

6. Parse and filter

Completion items with `kind != 2` (`CompletionItemKind::Method`) are discarded.
Method names are extracted by splitting the label at (, then the list is sorted
alphabetically and deduplicated by name.

7. Shutdown

```text
shutdown  →  <wait for ack>  →  exit
```

The child process is waited on to avoid zombies.

8. Cleanup

The `Probe` struct implements `Drop`: the temporary project directory is removed
automatically whether the query succeeds or fails.

