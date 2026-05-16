# rust-meth

Discover the methods available on any Rust type, with fuzzy filtering, inline
docs, interactive selection, and go-to-definition into the standard library
source. Powered by `rust-analyzer`.

Works on any type your toolchain knows about: primitives, standard library
types, and generic combinations of them. Support for third-party crate types
(e.g. `serde_json::Value`) is not yet implemented.

## Highlights

- Inspect any type's methods and full signatures
- Fuzzy-filter results with partial or typo-ridden input
- Show doc comments inline with `--doc`
- Browse methods interactively with `-i`
- Jump to the stdlib source of any method with `--gd`
- Open that definition directly in your `$EDITOR` with `--open`

## Why it's useful

Rust already gives you editor-side go-to-definition, but that only works inside
an open project. `rust-meth` works anywhere, no project, no editor, no LSP
session. Stay in the terminal while you discover methods, read signatures, and
jump into the stdlib source. Because it uses `rust-analyzer` under the hood, the
output reflects your actual installed toolchain rather than a static list.
Including nightly-only APIs, deprecated methods, and blanket trait impls.

## Table of Contents

- [Requirements](#requirements)
- [Installation](#installation)
- [Usage](#usage)
  - [Fuzzy filter](#fuzzy-filter)
  - [Inline documentation](#inline-documentation)
  - [Interactive picker](#interactive-picker)
  - [Go-to-definition](#go-to-definition)
- [How it works](#how-it-works)
- [License](#license)

## Requirements

- A Rust toolchain (stable or nightly)
- `rust-analyzer` on your PATH:

```sh
rustup component add rust-analyzer
```

- Go-To-Definition also requires `rust-src`

```sh
rustup component add rust-src
```

---

## Installation

```bash
cargo install rust-meth
```

---

## Usage

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
```

Because it uses rust-analyzer under the hood, the output reflects your actual
installed toolchain. Including trait methods, blanket impls, deprecated methods,
and nightly-only APIs. Not a static list.

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
$ rust-meth '&str' split_once --doc
$ rust-meth 'HashMap<String, u32>' entry --doc
$ rust-meth u8 checked --doc
```

Also works in interactive mode. Select a method and the doc comment prints below
the signature. Examples shown in next section.

### Interactive picker

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

Combine with `--doc` to also show the doc comment for the selected method:

```sh
$ rust-meth u8 -i --doc
```

Example: Output when I type `bitor`:

````bash
✔ Methods on `u8` · bitor
  bitor  fn(self, Rhs) -> <Self as BitOr<Rhs>>::Output

    Performs the `|` operation.

    # Examples

    ```rust
    assert_eq!(true | false, true);
````

---

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

**Why this is useful:** your editor's go-to-definition already does this for
code you're actively editing, but `rust-meth --gd` works anywhere — no open
project, no LSP session, no editor required. When you're exploring an unfamiliar
type or scratching out code in a standalone file, you can jump straight to the
stdlib implementation to understand exactly what a method does, how it handles
edge cases, or what it delegates to.

Pair it with `-i` to first discover the method you want, then open it:

```sh
$ rust-meth u8 -i               # pick a method interactively
$ rust-meth u8 --gd isqrt       # find it
$ rust-meth u8 --gd isqrt --open  # open it
u8::isqrt  library/core/src/num/uint_macros.rs:3684
```

## How it works

<details>
<summary> How it works </summary>

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

</details>

---

### License

- [MIT OR Apache-2.0](https://github.com/saylesss88/rust-meth/blob/main/LICENSE)
