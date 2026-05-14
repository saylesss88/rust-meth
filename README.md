# rust-meth

Print every method available on any Rust type, with full signatures. Powered by
`rust-analyzer`.

It works on any type your toolchain knows about: primitives, standard library
types, and generic combinations of them. Support for third-party crate types
(e.g. `serde_json::Value`) is not yet implemented.

## Table of Contents

- [Requirements](#requirements)
- [Installation](#installation)
- [Usage](#usage)
  - [Fuzzy filter](#fuzzy-filter)
  - [Inline documentation](#inline-documentation)
  - [Interactive picker](#interactive-picker)
- [How it works](#how-it-works)
- [License](#license)

## Requirements

- A Rust toolchain (stable or nightly)
- `rust-analyzer` on your PATH:

```sh
  rustup component add rust-analyzer
```

---

## Installation

```bash
cargo install rust-meth
```

---

## Usage

```sh
rust-meth <type> [filter | -i]
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
```

Because it uses rust-analyzer under the hood, the output reflects your actual
installed toolchain. Including trait methods, blanket impls, deprecated methods,
and nightly-only APIs. Not a static list.

## Fuzzy filter

The filter argument uses fuzzy matching, so typos and partials work:

```sh
rust-meth u8 wrapng       # finds all wrapping_* methods
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

Works best combined with a filter so you're not scrolling through docs for 200
methods at once:

```sh
rust-meth '&str' split_once --doc
rust-meth 'HashMap<String, u32>' entry --doc
rust-meth u8 checked --doc
```

### Interactive picker

Pass `-i` / `--interactive` instead of a filter to get a live fuzzy selector:

```sh
rust-meth u8 -i
rust-meth '&str' -i
```

```sh
$ rust-meth 'HashMap<String, u32>' -i
? Methods on `HashMap<String, u32>` ›
  capacity
  clear
  clone
  clone_from
  clone_into
  contains_key
  drain
  entry
  eq
  extend
  extend_one
  extend_reserve
  extract_if
  get
  get_disjoint_mut
  get_disjoint_unchecked_mut
  # ---snip---
```

Type to narrow the list, arrow keys to move, Enter to select: prints the method
name and full signature. Esc to quit without selecting.

---

## How it works

For each query, `rust-meth`:

1. Creates a temporary Cargo project in `/tmp` with a probe file:

```rust
   use std::collections::*;
   // ... other common std imports ...
   fn main() {
       let _x: TYPE = todo!();
       _x.  // ← LSP completion trigger
   }
```

2. Spawns `rust-analyzer` as a subprocess

3. Performs the LSP handshake (`initialize` → `initialized` →
   `textDocument/didOpen`)

4. Waits for RA to finish indexing, then sends `textDocument/completion` at the
   dot

5. Filters the response for `CompletionItemKind::Method` items

6. Prints names and signatures, then shuts RA down

The temporary project is cleaned up automatically on exit.

---

### License

- [MIT OR Apache-2.0](https://github.com/saylesss88/rust-meth/blob/main/LICENSE)
