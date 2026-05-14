// Creates a minimal temporary Cargo project containing a single source file
// that declares `let _x: TYPE = todo!();` followed by `_x.` The dot is the
// completion trigger point.  The project is cleaned up when `Probe` is dropped.

use std::fs;
use std::path::{Path, PathBuf};

// Preamble added to every probe file so common std types resolve without
// the user needing to fully qualify them (e.g. `HashMap` not `std::collections::HashMap`).
const PREAMBLE: &str = "\
#![allow(unused_imports)]
use std::collections::*;
use std::sync::*;
use std::cell::*;
use std::rc::Rc;
use std::io::{self, Read, Write, BufRead};
use std::fmt;
use std::ops::*;
use std::path::{Path, PathBuf};
";

pub struct Probe {
    pub dir: PathBuf,
    pub src_path: PathBuf,
    /// LSP position (0-indexed line, character) of the dot trigger.
    pub dot_line: u32,
    pub dot_col: u32,
}

impl Probe {
    pub fn new(type_name: &str) -> std::io::Result<Self> {
        let dir = std::env::temp_dir().join(format!("rust-meth-probe-{}", std::process::id()));
        let src_dir = dir.join("src");
        fs::create_dir_all(&src_dir)?;

        // Minimal Cargo.toml with no dependencies so indexing is fast.
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )?;

        // Source file layout (preamble lines + fn main):
        //
        //   0..N  preamble use statements
        //   N+0:  fn main() {
        //   N+1:      let _x: TYPE = todo!();
        //   N+2:      _x.         <-- completion trigger after the dot
        //   N+3:  }
        // let preamble_lines = PREAMBLE.lines().count() as u32;
        let preamble_lines =
            u32::try_from(PREAMBLE.lines().count()).expect("Preamble is too long to fit in u32");
        let source =
            format!("{PREAMBLE}fn main() {{\n    let _x: {type_name} = todo!();\n    _x.\n}}\n");

        let src_path = src_dir.join("main.rs");
        fs::write(&src_path, &source)?;

        // Dot is at preamble_lines + 2, col = len("    _x.")
        let dot_line = preamble_lines + 2;
        // let dot_col = "    _x.".len() as u32;
        let dot_col = u32::try_from("    _x.".len()).expect("failed");

        Ok(Self {
            dir,
            src_path,
            dot_line,
            dot_col,
        })
    }

    pub fn src_uri(&self) -> String {
        path_to_uri(&self.src_path)
    }

    pub fn root_uri(&self) -> String {
        path_to_uri(&self.dir)
    }

    pub fn source(&self) -> String {
        fs::read_to_string(&self.src_path).unwrap_or_default()
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn path_to_uri(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}
