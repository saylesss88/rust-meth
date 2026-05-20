## Workspace layout

`rust-meth` is a Cargo workspace with two members:

| Crate | Path | Role |
|---|---|---|
| `rust-meth-lib` | `lib/` | All core logic: LSP session, probe generation, parsing, UI, link building |
| `rust-meth` (binary) | `cli/` | Thin entry point — calls `rust_meth_lib::app::run()` |

```
lib/src/
  analyzer.rs   — LSP session orchestration, method/definition querying
  app.rs        — top-level run() entry point, CLI flow control
  error.rs      — RustMethError type
  lsp.rs        — synchronous LSP transport (Content-Length framing over stdio)
  probe.rs      — ephemeral Cargo project generation
  ui/
    args.rs     — CLI argument parsing
    display.rs  — terminal output formatting
    interactive.rs — fuzzy-select interactive mode
    links.rs    — doc URL building, editor/browser launching
    spinner.rs  — progress spinners
```

---

## How it works

For each query, `rust-meth`:

### 1. Creates a temporary Cargo project (`Probe`)

A minimal Cargo project is written to a uniquely named directory in `/tmp`:

```
/tmp/rust-meth-probe-<pid>-<n>/       ← completion queries
/tmp/rust-meth-probe-def-<pid>-<n>/   ← go-to-definition queries (--gd)
```

The counter `<n>` is a global atomic, ensuring uniqueness even across concurrent
calls within the same process.

The generated `src/main.rs` has two forms depending on the query type:

```rust
// preamble — common std imports so types resolve without full qualification
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
    _x.          // ← completion trigger (method listing)
    // or:
    _x.METHOD(); // ← definition trigger (--gd)
}
```

**Dependency injection** — the probe's `Cargo.toml` gets a `[dependencies]` block in two cases:

- `--deps <toml>` was passed explicitly, or
- the type contains `::` and its leading path segment starts with a lowercase
  letter (e.g. `serde_json::Value` → `serde_json = "*"` is auto-inferred).
  Stdlib and primitive types (`u8`, `String`, `Vec<u8>`, `HashMap<…>`, etc.)
  never get an inferred dependency.

### 2. Spawn rust-analyzer

`rust-analyzer` is located via `PATH` (falling back to `rustup which rust-analyzer`)
and spawned as a subprocess with `stdin`/`stdout` piped. `stderr` is suppressed.
The binary path is cached in a `OnceLock` for the lifetime of the process.

### 3. LSP handshake

```text
initialize  →  <wait for result>  →  initialized  →  textDocument/didOpen
```

`procMacro` expansion is disabled in `initializationOptions` to speed up cold
indexing.

### 4. Wait for indexing

Messages are consumed until one of these signals arrives (whichever comes first):

- `$/progress` with `value.kind == "end"`
- `experimental/serverStatus` with `quiescent == true`
- `textDocument/publishDiagnostics` or `workspace/diagnostic/refresh`
- Hard 10-second timeout (gives up and continues anyway)

### 5. Request completions or definition (with retry)

**Method listing:** `textDocument/completion` is sent at the dot position.
RA can return an empty list if it isn't fully ready, so the request is retried
up to 10 times with a 500 ms delay between attempts.

**Go-to-definition (`--gd`):** `textDocument/definition` is sent at the dot
position instead. The same retry loop applies, retrying on `-32801` (content
modified) and `-32800` (request cancelled) errors.

### 6. Parse and filter

**Completions:** items with `kind != 2` (`CompletionItemKind::Method`) are
discarded. Method names are extracted by splitting the label at `(`, then the
list is sorted alphabetically and deduplicated by name.

**Definition:** the first `uri`/`range` entry in the result is extracted. The
`full_path` is the raw filesystem path; `path` is shortened to start at
`library/` (stdlib) or `src/` (other) for display.

### 7. Post-definition actions (`--open` / `--open-doc`)

After a successful `--gd` lookup, one optional action can be taken:

**`--open`** — opens the source file at the definition line in `$EDITOR` (falling
back to `$VISUAL`). Line-jump syntax is adapted per editor:

| Editor | Argument form |
|---|---|
| `hx` / `helix` | `path:line` |
| `code` / `code-insiders` | `--goto path:line` |
| everything else | `+line path` |

**`--open-doc`** — builds a documentation URL and opens it in the system browser
(`xdg-open` on Linux, `open` on macOS, `cmd /C start` on Windows). The URL
destination depends on the definition path:

| Source | URL |
|---|---|
| stdlib (`/library/core/`, `/library/std/`, `/library/alloc/`) | `doc.rust-lang.org/std/…` |
| Cargo registry (`/registry/src/…`) | `docs.rs/<crate>/latest/…` |
| anything else | `docs.rs/releases/search?query=…` |

### 8. Interactive mode (`-i`)

When `-i` / `--interactive` is passed, after methods are fetched the list is
presented as a fuzzy-searchable picker (via `dialoguer`). Selecting a method
prints its signature and documentation.

### 9. Shutdown

```text
shutdown  →  <wait for ack>  →  exit
```

The child process is waited on to avoid zombies.

### 10. Cleanup

`Probe` implements `Drop`: the temporary project directory is removed
automatically whether the query succeeds or fails.
