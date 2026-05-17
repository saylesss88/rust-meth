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
    /// Creates a new probe project without dependencies (for stdlib types).
    pub fn new(type_name: &str) -> std::io::Result<Self> {
        Self::create_probe(type_name, None, None)
    }

    /// Creates a new probe project with optional dependencies (for 3rd party crates).
    ///
    /// # Arguments
    /// * `type_name` - The Rust type to query (e.g., "Vec<u8>", "serde_json::Value")
    /// * `deps` - Optional TOML dependencies section (e.g., "serde_json = \"1.0\"")
    pub fn new_with_deps(type_name: &str, deps: Option<&str>) -> std::io::Result<Self> {
        Self::create_probe(type_name, None, deps)
    }

    /// Creates a probe file with `_x.METHOD_NAME()` for go-to-definition queries.
    /// The cursor position points at the start of the method name.
    pub fn for_definition(type_name: &str, method_name: &str) -> std::io::Result<Self> {
        Self::create_probe(type_name, Some(method_name), None)
    }

    /// Creates a probe file for go-to-definition with custom dependencies.
    pub fn for_definition_with_deps(
        type_name: &str,
        method_name: &str,
        deps: Option<&str>,
    ) -> std::io::Result<Self> {
        Self::create_probe(type_name, Some(method_name), deps)
    }

    /// Internal probe creation logic shared by all constructors.
    ///
    /// # Arguments
    /// * `type_name` - The Rust type to query
    /// * `method_name` - If Some, creates a definition probe; if None, creates a completion probe
    /// * `deps` - Optional TOML dependencies to add to Cargo.toml
    fn create_probe(
        type_name: &str,
        method_name: Option<&str>,
        deps: Option<&str>,
    ) -> std::io::Result<Self> {
        let suffix = method_name.map_or("probe", |_| "probe-def");
        let dir = std::env::temp_dir().join(format!("rust-meth-{suffix}-{}", std::process::id()));
        let src_dir = dir.join("src");
        fs::create_dir_all(&src_dir)?;

        // Build Cargo.toml with optional dependencies
        let cargo_toml = match deps {
            Some(d) => format!(
                "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{}\n",
                d
            ),
            None => {
                "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n".to_string()
            }
        };

        fs::write(dir.join("Cargo.toml"), cargo_toml)?;

        // Source file layout (preamble lines + fn main):
        //
        //   0..N  preamble use statements
        //   N+0:  fn main() {
        //   N+1:      let _x: TYPE = todo!();
        //   N+2:      _x.         <-- completion trigger after the dot (or _x.METHOD() for definition)
        //   N+3:  }
        let preamble_lines =
            u32::try_from(PREAMBLE.lines().count()).expect("Preamble is too long to fit in u32");

        // Generate source based on whether we're doing completion or definition
        let source = match method_name {
            Some(method) => {
                // For definition: _x.METHOD_NAME()
                format!(
                    "{PREAMBLE}fn main() {{\n    let _x: {type_name} = todo!();\n    _x.{method}();\n}}\n"
                )
            }
            None => {
                // For completion: _x.
                format!("{PREAMBLE}fn main() {{\n    let _x: {type_name} = todo!();\n    _x.\n}}\n")
            }
        };

        let src_path = src_dir.join("main.rs");
        fs::write(&src_path, &source)?;

        // Dot is at preamble_lines + 2, col = len("    _x.")
        let dot_line = preamble_lines + 2;
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
