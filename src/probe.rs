// Creates a minimal temporary Cargo project containing a single source file
// that declares `let _x: TYPE = todo!();` followed by `_x.` — the dot is the
// completion trigger point.  The project is cleaned up when `Probe` is dropped.

use std::fs;
use std::path::{Path, PathBuf};

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

        // Minimal Cargo.toml — no dependencies so indexing is fast.
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )?;

        // Source file:
        //   line 0: fn main() {
        //   line 1:     let _x: TYPE = todo!();
        //   line 2:     _x.          <-- completion trigger after the dot
        //   line 3: }
        let source = format!("fn main() {{\n    let _x: {type_name} = todo!();\n    _x.\n}}\n");
        let src_path = src_dir.join("main.rs");
        fs::write(&src_path, &source)?;

        // Dot is at line 2, right after "_x." (4 spaces + "_x." = col 7)
        let dot_col = "    _x.".len() as u32;

        Ok(Self {
            dir,
            src_path,
            dot_line: 2,
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
    // Simple file URI — assumes Unix paths (absolute, starts with /).
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}
