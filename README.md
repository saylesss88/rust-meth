# rust-meth

[![Crates.io](https://img.shields.io/crates/v/rust-meth.svg)](https://crates.io/crates/rust-meth)
[![Nix Flake](https://img.shields.io/badge/Nix_Flake-Geared-dddd00?logo=nixos&logoColor=white)](https://nixos.org/manual/nix/stable/command-ref/new-cli/nix3-flake.html)
[![Documentation](https://docs.rs/rust-meth/badge.svg)](https://docs.rs/rust-meth)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Discover the methods available on any Rust type — with fuzzy filtering, inline
docs, interactive selection, and go-to-definition into the standard library
source. Powered by `rust-analyzer`.

Think of it as "method completion for any Rust type, anywhere in your terminal."

> [!IMPORTANT]
> - **Standard Toolchain:** Fully supported (`std`, `core`, and `alloc`).
> - **External Crates:** Supported via the `--deps` flag (see [3rd party crates](#3rd-party-crates)).


## Highlights

- Inspect any type's methods and full signatures
- Fuzzy-filter results with partial or typo-ridden input
- Show doc comments inline with `--doc`
- Browse methods interactively with `-i`
- Jump to the stdlib source of any method with `--gd`
- Open that definition directly in your `$EDITOR` with `--open`
- Open official documentation directly in your browser with `--open-doc`
- Query 3rd party crate types with `--deps`

## Why it's useful

Rust already gives you editor-side go-to-definition, but that only works inside
an open project. `rust-meth` works anywhere — no project, no editor, no LSP
session. Stay in the terminal while you discover methods, read signatures, and
jump into the stdlib source. Because it uses `rust-analyzer` under the hood, the
output reflects your actual installed toolchain: trait methods, blanket impls,
deprecated methods, and nightly-only APIs. Not a static list.

## Table of Contents

- [Requirements](#requirements)
- [Installation](#installation)
- [Usage](#usage)
  - [Fuzzy filter](#fuzzy-filter)
  - [Inline documentation](#inline-documentation)
  - [Interactive picker](#interactive-picker)
  - [Go-to-definition](#go-to-definition)
  - [Open in browser](#open-in-browser)
  - [3rd party crates](#3rd-party-crates)
  - [Call snippets](#call-snippets)
  - [JSON output](#json-output)
- [How it works](#how-it-works)
- [License](#license)

<a id="requirements"></a>
## Requirements

- A Rust toolchain (stable or nightly)
- `rust-analyzer` on your `PATH`:

```sh
rustup component add rust-analyzer
```

- `rust-src` (Rust standard library source code):

```sh
rustup component add rust-src
```

> [!NOTE]
> While `rust-analyzer` will typically attempt to install the standard library
> source automatically, installing it manually ensures it is present.

---

<a id="installation"></a>
## Installation

```bash
cargo install rust-meth
```

---

<a id="usage"></a>
## Usage

First run may take a few seconds while `rust-analyzer` indexes the toolchain.

```sh
$ rust-meth <type> [filter | -i]
```

```bash
$ rust-meth u8 wrapping
Waiting for rust-analyzer to index… (this may take a moment on first run)
(attempt 1: not ready, retrying…)
rust-meth: methods on `u8` matching "wrapping"
  wrapping_add                const fn(self, u8) -> u8
  wrapping_add_signed         const fn(self, i8) -> u8
  wrapping_div                const fn(self, u8) -> u8
  wrapping_div_euclid         const fn(self, u8) -> u8
  wrapping_mul                const fn(self, u8) -> u8
  wrapping_neg                const fn(self) -> u8
  wrapping_next_power_of_two  const fn(self) -> u8
  wrapping_pow                const fn(self, u32) -> u8
  wrapping_rem                const fn(self, u8) -> u8
  wrapping_rem_euclid         const fn(self, u8) -> u8
  wrapping_shl                const fn(self, u32) -> u8
  wrapping_shr                const fn(self, u32) -> u8
  wrapping_sub                const fn(self, u8) -> u8
  wrapping_sub_signed         const fn(self, i8) -> u8
14 method(s)
```

More examples:

```bash
rust-meth '&str'
rust-meth String
rust-meth f64
rust-meth 'Vec<u8>'
rust-meth 'Option<u8>'
rust-meth 'HashMap<String, u32>'
rust-meth 'Option<Result<Vec<u8>, std::io::Error>>'
```

<a id="fuzzy-filter"></a>
## Fuzzy filter

The filter argument uses fuzzy matching, so typos and partials work:

```sh
$ rust-meth u8 wrapng       # finds all wrapping_* methods
```

```sh
$ rust-meth '&str' splt
Waiting for rust-analyzer to index… (this may take a moment on first run)
rust-meth: methods on `&str` matching "splt"
  split_terminator        fn(&self, P) -> SplitTerminator<'_, P>
  split                   fn(&self, P) -> Split<'_, P>
  split_ascii_whitespace  fn(&self) -> SplitAsciiWhitespace<'_>
  split_at                const fn(&self, usize) -> (&str, &str)
  split_at_checked        const fn(&self, usize) -> Option<(&str, &str)>
  split_at_mut            const fn(&mut self, usize) -> (&mut str, &mut str)
  split_at_mut_checked    const fn(&mut self, usize) -> Option<(&mut str, &mut str)>
  split_inclusive         fn(&self, P) -> SplitInclusive<'_, P>
  split_once              fn(&self, P) -> Option<(&str, &str)>
  split_whitespace        fn(&self) -> SplitWhitespace<'_>
  splitn                  fn(&self, usize, P) -> SplitN<'_, P>
  rsplit_terminator       fn(&self, P) -> RSplitTerminator<'_, P>
  rsplit                  fn(&self, P) -> RSplit<'_, P>
  rsplit_once             fn(&self, P) -> Option<(&str, &str)>
  rsplitn                 fn(&self, usize, P) -> RSplitN<'_, P>
  escape_default          fn(&self) -> EscapeDefault<'_>
16 method(s)
```

Results are sorted by match quality, best first.

---

<a id="inline-documentation"></a>
## Inline documentation

Pass `--doc` / `-d` to print the doc comment below each method signature:

```sh
$ rust-meth u8 strict_shr --doc
rust-meth: methods on `u8` matching "strict_shr"
  strict_shr  const fn(self, u32) -> u8
    Strict shift right. Computes `self >> rhs`, panicking if `rhs` is
    larger than or equal to the number of bits in `self`.
    # Panics
    ## Overflow behavior
1 method(s)
```

Works best combined with a filter:

```sh
$ rust-meth '&str' split_once --doc
$ rust-meth 'HashMap<String, u32>' entry --doc
$ rust-meth u8 checked --doc
```

Also works in interactive mode — select a method and its doc comment prints
below the signature.

<a id="interactive-picker"></a>
## Interactive picker

Pass `-i` / `--interactive` instead of a filter to get a live fuzzy selector:

```sh
$ rust-meth u8 -i
$ rust-meth '&str' -i
```

```sh
$ rust-meth 'HashMap<String, u32>' -i
? Methods on `HashMap<String, u32>` ›
  capacity
  clear
  clone
  ...
```

Type to narrow the list, arrow keys to move, Enter to select. Esc to quit.

Combine with `--doc` to also show the doc comment for the selected method:

```sh
$ rust-meth u8 -i --doc
```

Example output when typing `bitor`:

```markdown
✔ Methods on `u8` · bitor
  bitor  fn(self, Rhs) -> <Self as BitOr<Rhs>>::Output

    Performs the `|` operation.
    
    # Examples
    
    ```rust
    assert_eq!(true | false, true);
```

---

<a id="go-to-definition"></a>
## Go to definition

Pass `--gd <method>` to find where a method is defined in the standard library
source:

```sh
$ rust-meth u8 --gd checked_add
u8::checked_add  library/core/src/num/uint_macros.rs:902
$ rust-meth '&str' --gd split_once
&str::split_once  library/core/src/str/mod.rs:2241
```

Add `--open` / `-o` to jump straight to that line in your `$EDITOR`:

```sh
$ rust-meth u8 --gd checked_add --open
u8::checked_add  library/core/src/num/uint_macros.rs:902
# opens uint_macros.rs at line 902 in $EDITOR
```

Supports `hx` / `helix`, `nvim`, `vim`, `emacs`, and `code`. Any editor that
accepts `+LINE file` on the command line will also work.

Requires the `rust-src` component and `$EDITOR` to be set:

```sh
rustup component add rust-src
export EDITOR=hx  # or nvim, vim, etc.
```

**Discovery Workflow**

Pair it with `-i` to discover first, then open:

```sh
$ rust-meth u8 -i               # pick a method interactively
$ rust-meth u8 --gd isqrt       # find it
$ rust-meth u8 --gd isqrt --open  # open it
u8::isqrt  library/core/src/num/uint_macros.rs:3684
```

<a id="open-in-browser"></a>
### Open in browser

Pass `--open-doc` with `--gd <method>` to open the official documentation for
that method directly in your browser:

```sh
$ rust-meth u8 --gd isqrt --open-doc
u8::isqrt  library/core/src/num/uint_macros.rs:3684
Opening in existing browser session.
```

Works for primitives, stdlib structs, and collections:

```sh
$ rust-meth u8 --gd isqrt --open-doc          # doc.rust-lang.org/std/primitive.u8.html#method.isqrt
$ rust-meth String --gd len --open-doc         # doc.rust-lang.org/std/string/struct.String.html#method.len
$ rust-meth 'Vec<u8>' --gd push --open-doc     # doc.rust-lang.org/std/vec/struct.Vec.html#method.push
$ rust-meth 'HashMap<String, u32>' --gd get --open-doc  # doc.rust-lang.org/std/collections/hash_map/struct.HashMap.html#method.get
```

> [!NOTE]
> `--open` and `--open-doc` are mutually exclusive options
>
> **Limitation:** `--gd` currently only works for standard library types. Go-to-definition
> for 3rd party crate methods is not yet supported.

---

<a id="3rd-party-crates"></a>
## 3rd party crates

Use the `--deps` flag to query methods on types from external crates:

```sh
$ rust-meth 'serde_json::Value' --deps 'serde_json = "1.0"'
✓ Found 37 methods (5.7s)

rust-meth: methods on `serde_json::Value`

  as_array          fn(&self) -> Option<&Vec<Value, Global>>
  as_bool           fn(&self) -> Option<bool>
  as_f64            fn(&self) -> Option<f64>
  as_i64            fn(&self) -> Option<i64>
  as_null           fn(&self) -> Option<()>
  as_object         fn(&self) -> Option<&Map<String, Value>>
  as_str            fn(&self) -> Option<&str>
  as_u64            fn(&self) -> Option<u64>
  ...
37 method(s)
```

**Works with all features:**

```sh
# Interactive mode
$ rust-meth 'serde_json::Value' --deps 'serde_json = "1.0"' -i

# Fuzzy filter
$ rust-meth 'serde_json::Value' as_bool --deps 'serde_json = "1.0"'

# Show documentation
$ rust-meth 'serde_json::Value' --deps 'serde_json = "1.0"' --doc
```

**Multiple dependencies:**

```sh
$ rust-meth 'reqwest::Client' --deps 'reqwest = "0.11"
tokio = { version = "1", features = ["full"] }'
```

**Complex dependency specifications:**

```sh
$ rust-meth 'tokio::net::TcpStream' --deps 'tokio = { version = "1", features = ["net"] }'
```

> [!NOTE]
> The `--deps` flag accepts raw TOML syntax exactly as it would appear in `Cargo.toml`.
> First-time queries with external crates may take longer (5-10 seconds) as `rust-analyzer`
> downloads and indexes the dependencies. Subsequent queries will be faster.

---

<a id="call-snippets"></a>
## Call snippets

Pass `--snippet` to print each method as a ready-to-use call site:

```bash
rust-meth u8 wrapping --snippet
```

Output:

```text
  const fn(self, u8) -> u8
  → x.wrapping_add(u8)

  const fn(self, i8) -> u8
  → x.wrapping_add_signed(i8)

  const fn(self, u8) -> u8
  → x.wrapping_div(u8)

  const fn(self) -> u8
  → x.wrapping_neg()
```

Each block shows:
- the full signature, and
- a call form on the next line prefixed with `→`.

This is useful when you know the method name pattern but not the exact argument forms, or when you want a quick template to paste into your code.

---

<a id="json-output"></a>
## JSON output

Pass `--json` to get machine-friendly output:

```bash
rust-meth u8 wrapping --json
```

Example:

```json
[
  {
    "name": "wrapping_add",
    "detail": "const fn(self, u8) -> u8",
    "documentation": " Wrapping (modular) addition. Computes `self + rhs`,\n wrapping around at the boundary of the type.\n\n # Examples\n\n ```\nassert_eq!(200u8.wrapping_add(55), 255);\nassert_eq!(200u8.wrapping_add(u8::MAX), 199);\n ```"
  },
  {
    "name": "wrapping_add_signed",
    "detail": "const fn(self, i8) -> u8",
    "documentation": " Wrapping (modular) addition with a signed integer. Computes\n `self + rhs`, wrapping around at the boundary of the type.\n\n # Examples\n\n ```\nassert_eq!(1u8.wrapping_add_signed(2), 3);\nassert_eq!(1u8.wrapping_add_signed(-2), u8::MAX);\nassert_eq!((u8::MAX - 2).wrapping_add_signed(4), 1);\n ```"
  }
]
```

Each item includes:
- `name`: method name
- `detail`: full signature as seen in the terminal
- `documentation`: raw doc comment text (newlines preserved)

This mode is ideal for scripting, piping into `jq`, or integrating with other tools.

---

<a id="how-it-works"></a>
## How it works

- See [Architecture](/ARCHITECTURE.md)

---

<a id="license"></a>
## License

[MIT OR Apache-2.0](https://github.com/saylesss88/rust-meth/blob/main/LICENSE)
