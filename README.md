# rust-meth

`rust-meth` is a type-first method discovery tool for Rust.

[![Crates.io](https://img.shields.io/crates/v/rust-meth.svg)](https://crates.io/crates/rust-meth)
[![Nix Flake](https://img.shields.io/badge/Nix_Flake-Geared-dddd00?logo=nixos&logoColor=white)](https://nixos.org/manual/nix/stable/command-ref/new-cli/nix3-flake.html)
[![Documentation](https://docs.rs/rust-meth/badge.svg)](https://docs.rs/rust-meth)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Discover the methods available on any Rust type. With fuzzy filtering, inline
docs, interactive selection, and go-to-definition into the standard library
source. Powered by `rust-analyzer`.

<p align="center">
  <img src="https://raw.githubusercontent.com/saylesss88/rust-meth/main/assets/serde_json.png" width="600" alt="rustmeth-serde_json example">
</p>

> Think of it as "method completion for any Rust type, anywhere in your terminal."

> [!IMPORTANT]
> - **Standard Toolchain:** Fully supported (`std`, `core`, and `alloc`).
> - **External Crates:** Supported via the `--deps` flag (see [3rd party crates](#3rd-party-crates)).


## Workspace Structure

`rust-meth` is a Cargo workspace. The CLI (`rust-meth`) is the user-facing binary;
the core analysis logic lives in the [`rust-meth-lib`](./lib/README.md) library crate.

| Crate | Path | Description |
|-------|------|-------------|
| `rust-meth` | `cli/` | CLI binary: argument parsing, output, interactive UI |
| `rust-meth-lib` | `lib/` | Library: rust-analyzer integration, type resolution, method lookup |

## Highlights

- Inspect any type's methods and full signatures
- Fuzzy-filter results with partial or typo-ridden input
- Show doc comments inline with `--doc`
- Browse methods interactively with `-i`
- Jump to the stdlib source of any method with `--gd`
- Open that definition directly in your `$EDITOR` with `--open`
- Open official documentation in your browser with `--open-doc`
- Query 3rd party crate types with `--deps`
- Colorized output, JSON output, and call snippets
- Explain any method's full documentation with `--explain`
- Syntax Highlighting of ```rust blocks in output from `--explain`
- Three-tier persistent cache: results (~0.0s), probe (skips Cargo resolution),
  and daemon workspaces (warm sessions at ~0.1s)

## Requirements

- A Rust toolchain (stable or nightly)
- `rust-analyzer` on your `PATH`:
  ```sh
  rustup component add rust-analyzer
  ```
- `rust-src` (for go-to-definition):
  ```sh
  rustup component add rust-src
  ```

## Installation

```bash
cargo install rust-meth
```

## Usage

```sh
rust-meth <type> [filter] [flags]
```

```bash
rust-meth u8 wrapping
rust-meth '&str' splt               # fuzzy, finds split_*
rust-meth 'Vec<u8>' -i              # interactive picker
rust-meth u8 strict_shr --doc       # inline documentation
rust-meth u8 --gd checked_add       # go-to-definition
rust-meth u8 --gd isqrt --open      # open in $EDITOR
rust-meth u8 --gd isqrt --open-doc  # open in browser
rust-meth 'serde_json::Value'       # 3rd party crate
rust-meth u8 wrapping --snippet     # print as a paste-ready call site
rust-meth u8 wrapping --json        # machine-friendly output
```

<p align="center">
  <img src="https://raw.githubusercontent.com/saylesss88/rust-meth/main/assets/rustmeth-open.gif" width="600" alt="rustmeth-open demo">
</p>

For full flag reference and detailed examples, see the sections below.

## Table of Contents

- [Fuzzy filter](#fuzzy-filter)
- [Inline documentation](#inline-documentation)
- [Interactive picker](#interactive-picker)
- [Go-to-definition](#go-to-definition)
- [Open in browser](#open-in-browser)
- [Explain a method](#explain-a-method)
- [3rd party crates](#3rd-party-crates)
- [Daemon mode](#daemon-mode)
- [Caching](#caching)
- [Call snippets](#call-snippets)
- [JSON output](#json-output)
- [Syntax highlighting](#syntax-highlighting)
- [How it works](#how-it-works)
- [License](#license)

<a id="fuzzy-filter"></a>
## Fuzzy filter

The filter argument uses fuzzy matching, typos and partials work:

```sh
rust-meth u8 wrapng        # finds all wrapping_* methods
rust-meth '&str' splt      # finds split_*
```

Results are sorted by match quality, best first.

<a id="inline-documentation"></a>
## Inline documentation

Pass `--doc` / `-d` to print the doc comment below each method:

```sh
rust-meth u8 strict_shr --doc
rust-meth '&str' split_once --doc
rust-meth 'HashMap<String, u32>' entry --doc
```

Also works in interactive mode.

<a id="interactive-picker"></a>
## Interactive picker

Pass `-i` / `--interactive` to get a live fuzzy selector:

```sh
rust-meth u8 -i
rust-meth 'HashMap<String, u32>' -i
rust-meth u8 -i --doc   # also shows doc for selected method
```

Type to narrow, arrow keys to move, Enter to select, Esc to quit.

<a id="go-to-definition"></a>
## Go to definition

Pass `--gd <method>` to find where a method is defined in the stdlib source:

```sh
rust-meth u8 --gd checked_add
# u8::checked_add  library/core/src/num/uint_macros.rs:902

rust-meth u8 --gd checked_add --open
# opens uint_macros.rs at line 902 in $EDITOR
```

Supports `hx`, `nvim`, `vim`, `emacs`, and `code`. Requires `$EDITOR` to be set.

<a id="open-in-browser"></a>
## Open in browser

Pass `--open-doc` with `--gd` to open the official docs in your browser:

```sh
rust-meth u8 --gd isqrt --open-doc
rust-meth 'Vec<u8>' --gd push --open-doc
rust-meth 'HashMap<String, u32>' --gd get --open-doc
```

<p align="center">
  <img src="https://raw.githubusercontent.com/saylesss88/rust-meth/main/assets/rustmeth-browse.gif" width="600" alt="rustmeth-browse demo">
</p>

> [!NOTE]
> `--open` and `--open-doc` are mutually exclusive.
> `--gd` currently only works for standard library types.

<a id="3rd-party-crates"></a>
## 3rd party crates

For types where the crate name matches the type path prefix, `--deps` can be omitted:

```sh
rust-meth 'serde_json::Value'
rust-meth 'serde_json::Value' -i
rust-meth 'anyhow::Error'
```

Use `--deps` to pin a version or add features:

```sh
rust-meth 'serde_json::Value' --deps 'serde_json = "1.0"'
rust-meth 'reqwest::Client' --deps 'reqwest = "0.11" tokio = { version = "1", features = ["net"] }'
```

> [!NOTE]
> First-time queries may take 5–10 seconds as `rust-analyzer` downloads and
> indexes dependencies. Subsequent queries are cached and skip Cargo resolution,
> making them much faster.


## Example: querying a third-party crate

Enumerate methods on a type from any crate, including its optional features:

```bash
rust-meth 'tokio::net::TcpStream' --deps 'tokio = { version = "*", features = ["net"] }'
```

```bash
rust-meth: methods on `tokio::net::TcpStream`
  as_ref              fn(&self) -> &T
  async_io            async fn(&self, Interest, impl FnMut() -> Result<R, Error>) -> Result<R, Error>
  into                fn(self) -> T
  into_split          fn(self) -> (OwnedReadHalf, OwnedWriteHalf)
  ...
  writable            async fn(&self) -> Result<(), Error>
32 method(s)
```

`--deps` accepts any valid `Cargo.toml` dependency line, so you can pin versions,
enable features, or point at git/path dependencies the same way you would in a
real project.

> [!NOTE]
> Very large crates with deep dependency trees (GUI frameworks, async runtimes,
> etc.) will always be fairly slow as `rust-analyzer` indexes it.

<a id="explain-a-method"></a>
## Explain a method

Pass `--explain <method>` to print the full documentation for a specific method
directly in the terminal — no browser required:

```sh
rust-meth 'Result<u8, String>' --explain unwrap_unchecked
rust-meth u8 --explain checked_add
rust-meth 'Vec<u8>' --explain retain
```

Output includes the method signature and its complete rustdoc comment, untruncated:

```
  method: unwrap_unchecked
  sig:    unsafe fn(self) -> T

    │ Returns the contained `Ok` value, consuming the `self` value,
    │ without checking that the value is not an `Err`.
    │
    │ # Safety
    │
    │ Calling this method on an `Err` value is undefined behavior.
    │
    │ # Examples
    │ ...
```

Pass `--browser` to open the official documentation page instead:

```sh
rust-meth 'Result<u8, String>' --explain unwrap_unchecked --browser
rust-meth u8 --explain checked_add --browser
```

> [!NOTE]
> Unlike `--open-doc`, `--explain --browser` does not require `--gd` and skips
> the definition lookup step, making it faster for quick doc lookups.


---

<a id="daemon"></a>
## Daemon mode

For interactive sessions where you query many types from the same crate,
`rust-meth` includes a persistent daemon that keeps a warm `rust-analyzer`
session alive between queries. Instead of spawning and tearing down a
subprocess for every invocation, the daemon reuses the session via
`textDocument/didChange`, cutting repeated queries from ~3s to ~0.1s.

```sh
# Start the daemon in the background
rust-meth --daemon start &

# Queries now use the warm session automatically
rust-meth 'RefCell<u8>'          # ~3s first query (new session)
rust-meth 'Cell<u8>'             # ~0.1s (warm session, same deps)
rust-meth 'Mutex<u8>'            # ~0.1s

# Third-party types get their own session keyed by dep set
rust-meth 'serde_json::Value' --deps 'serde_json = "1.0"'
rust-meth 'serde_json::Map<String, serde_json::Value>' --deps 'serde_json = "1.0"'  # ~0.1s

# Check status
rust-meth --daemon status

# Stop when done
rust-meth --daemon stop
```

The daemon falls back gracefully, if it isn't running, `rust-meth` behaves
exactly as before, using the file-based results cache and standard LSP sessions.

Sessions are keyed by dependency set, so stdlib types share one session,
`serde_json` types share another, and so on. Sessions expire after 10 minutes
of inactivity (configurable with `--ttl`):

```sh
rust-meth --daemon start --ttl 300   # 5 minute TTL
```

> [!NOTE]
> The daemon is most useful for exploratory sessions, browsing an unfamiliar
> crate, comparing methods across related types, or running many queries in
> quick succession. For one-off queries the results cache already handles
> repeated lookups instantly.

---


<a id="caching"></a>
## Caching

`rust-meth` caches at three levels to minimize repeated work:

| Cache | Location | What it eliminates |
|---|---|---|
| Results | `$XDG_CACHE_HOME/rust-meth/results/` | Entire LSP session (~0.0s on hit) |
| Probe | `$XDG_CACHE_HOME/rust-meth/probes/` | Cargo resolution for third-party types |
| Daemon workspaces | `$XDG_CACHE_HOME/rust-meth/workspaces/` | Per-session rust-analyzer index |

Cache keys include the `rust-analyzer` version and `rust-meth` version —
upgrading either automatically invalidates stale entries.

### Query timing at each cache level

| Path | Time | Notes |
|---|---|---|
| First query (cold) | `~3–9s` | Full rust-analyzer session |
| Probe cache hit | `~3–4s` | Skips Cargo resolution |
| Daemon warm session | `~0.1s` | Reuses live rust-analyzer process |
| Results cache hit | `~0.0s` | Reads a JSON file, no LSP involved |


To clear the caches:

```sh
rm -rf ~/.cache/rust-meth/
```

Or selectively:

```sh
rm -rf ~/.cache/rust-meth/results/     # clear method results only
rm -rf ~/.cache/rust-meth/probes/      # clear probe directories
rm -rf ~/.cache/rust-meth/workspaces/  # clear daemon workspaces
```

---

<a id="call-snippets"></a>
## Call snippets

Pass `--snippet` to print each method as a ready-to-paste call site:

```bash
rust-meth u8 wrapping --snippet
#   const fn(self, u8) -> u8
#   → x.wrapping_add(u8)
#
#   const fn(self) -> u8
#   → x.wrapping_neg()
```

<a id="json-output"></a>
## JSON output

Pass `--json` for machine-friendly output (pipe into `jq`, scripts, etc.):

```bash
rust-meth u8 wrapping --json
```

Each item includes `name`, `detail` (full signature), and `documentation`.

<a id="how-it-works"></a>
## How it works

See [ARCHITECTURE.md](./ARCHITECTURE.md) and the [lib crate README](./lib/README.md).

## Syntax Highlighting

The `--explain` flag renders full documentation with syntax-highlighted ```rust
blocks:

```bash
rust-meth 'Result<u8, String>' --explain unwrap_unchecked
```

<p align="center">
  <img src="https://raw.githubusercontent.com/saylesss88/rust-meth/main/assets/syntax-highlighting.cleaned.gif" width="600" alt="rustmeth syntax-highlighting">
</p>

## License

[Apache-2.0](https://github.com/saylesss88/rust-meth/blob/main/LICENSE)
