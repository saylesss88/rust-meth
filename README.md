# rust-meth

Print every method available on any Rust type, with full signatures. Powered by
`rust-analyzer`.

```bash
rust-meth u8 wrapping
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

More example commands:

```bash
rust-meth 'Vec<u8>'

rust-meth 'HashMap<String, u32>'
```

Because it uses rust-analyzer under the hood, the output reflects your actual
installed toolchain. Including trait methods, blanket impls, deprecated methods,
and nightly-only APIs. Not a static list.
